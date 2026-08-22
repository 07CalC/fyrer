# Fyrer Architecture

This document describes the target architecture for fyrer as an extensible,
workspace-based monorepo build tool. It covers the problems with the current
single-crate design, the principles behind the redesign, the crate layout, the
ownership model that makes task restart possible, and a phased migration plan.

---

## 1. Why the current design cannot be extended

### 1.1 Process ownership is split three ways

Today, spawning a task scatters ownership across three components:

| Component                          | What it holds                                    |
| ---------------------------------- | ------------------------------------------------ |
| Tokio task inside `Task::spawn()`  | The `Child`, stdin, timeout deadline             |
| `Scheduler`                        | The `JoinHandle<ProcessResult>` via `JoinSet`    |
| `Orchestrator`                     | The kill channel, received **via an event**      |

`AppEvent::TaskSpawned { command_tx }` broadcasts a live `Sender<TaskCommand>`
over the same bus used for logs and UI input. Events are observations;
channels are capabilities. Every subscriber (including UIs) receives control
power over every process, and nobody is *the* owner of a running task.

Consequences:

- **Restart is structurally impossible.** Once a level's `JoinSet` drains, all
  handles are dropped. There is no component whose job is "keep this task
  running across attempts". `watch` and crash-recovery cannot be implemented.
- **Two sources of truth for status**: `Orchestrator.tasks` and
  `Scheduler.status` can drift.
- **Killing stale tasks silently no-ops** (`try_send` on a dead receiver).

### 1.2 Level-barrier scheduling

`TaskGraph::get_orders` produces `Vec<Vec<TaskId>>`; each level must fully
drain before the next starts. This makes persistent dev servers block all
later levels (documented limitation), prevents starting a task the moment its
own dependencies finish, and has no room for re-scheduling (restart/watch).

### 1.3 Everything shares one broadcast bus

Logs, control flow, UI keypresses, ticks, and process handles all travel
through one `broadcast<AppEvent>`. Lag drops log lines silently; the TUI keeps
unbounded per-task buffers; there is no transcript persistence.

### 1.4 Monolithic crate

Config parsing, glob resolution, hashing, process management, scheduling, and
two UIs live in one binary crate with `pub(crate)` coupling. Nothing can be
reused or swapped: the cache provider trait is sync-blocking inside async
code, there is exactly one executor strategy hard-coded, and adding CLI verbs
(`restart`, `logs`, `graph`) means touching the orchestrator monolith.

Known bugs worth fixing along the way:
`LocalCacheProvider::restore` copies outputs to `std::env::current_dir()`
instead of the task's cwd, and `Task::cache_key` re-hashes the whole
dependency tree without memoization.

---

## 2. Goals

1. **Single owner per process.** Exactly one actor owns each child process for
   exactly one attempt of one task.
2. **Restart as a first-class operation**, enabling watch mode, manual restart
   from the TUI, crash policies, and future daemon/attach workflows.
3. **Streaming DAG execution.** A task starts as soon as its dependencies have
   finished — no level barriers; persistent tasks never block others.
4. **Extensibility at defined seams**: cache backends, executors, reporters,
   watchers, config discovery — all behind traits in small crates.
5. **Library-first core** so the engine can be embedded (tests, daemons,
   language bindings) without the CLI.
6. Keep the existing `fyrer.yml` v1 format working unchanged.

Non-goals (for now): remote/distributed execution, plugins in other
languages, config hot reload, Windows job objects beyond best-effort.

---

## 3. Design principles

1. **Single-writer state machine.** All mutable run state lives in one place
   (the `Engine`), mutated by one event loop. No shared maps, no locks.
2. **Linear ownership chain:** Engine → Supervisor (per attempt) → Child.
   Control flows down dedicated mpsc channels; observations flow up as
   data-only events. Handles are never broadcast.
3. **Attempts, not restarts.** An execution is identified by
   `(RunId, TaskId, Attempt)`. "Restart" simply means spawning attempt N+1
   under a fresh supervisor. Watch mode, retry-on-crash, and manual restart
   are all the same mechanism.
4. **Separate planes:** control plane (commands), observation plane (events),
   data plane (log bytes). Each has its own typed channels.
5. **Crate-per-concern** with strictly downward dependencies; traits at every
   boundary.

---

## 4. Target workspace layout

```
fyrer/
├── Cargo.toml                  # [workspace]
├── crates/
│   ├── fyrer-core/             # domain model. NO tokio, NO filesystem IO
│   │   └── src/
│   │       ├── id.rs           #   PackageId, TaskId, RunId, Attempt, ExecKey
│   │       ├── spec.rs         #   TaskSpec (pure immutable definition)
│   │       ├── graph.rs        #   TaskGraph: deps/dependents, cycle checks, topo utils
│   │       └── status.rs       #   TaskStatus, TaskOutcome, ExitReason
│   ├── fyrer-config/           # discovery + parse + validate → Workspace IR
│   │   └── src/
│   │       ├── yaml.rs         #   current serde schema (v1, kept compatible)
│   │       ├── ir.rs           #   resolved Workspace { packages, env, tasks }
│   │       ├── validate.rs     #   rule checks (split out of config/mod.rs today)
│   │       └── discovery.rs    #   trait for auto-detecting packages (future)
│   ├── fyrer-process/          # OS primitives, sync & unit-testable
│   │   └── src/
│   │       ├── spawn.rs        #   shell resolution, stdio pipes, pgid/job setup
│   │       └── signal.rs       #   group kill (unix), Job Objects note (windows)
│   ├── fyrer-log/              # LogRouter actor + sinks
│   │   └── src/
│   │       ├── router.rs       #   single consumer of all log lines
│   │       ├── buffer.rs       #   bounded ring buffer per task (for TUI replay)
│   │       └── sink.rs         #   Sink trait: file transcript, stdout passthrough…
│   ├── fyrer-cache/            # async CacheProvider + implementations
│   │   └── src/
│   │       ├── provider.rs     #   async trait
│   │       ├── local.rs        #   .fyrer/cache tar+zstd (fixes restore-to-cwd bug)
│   │       └── hash.rs         #   blake3 hashing w/ memoized dep tree
│   ├── fyrer-watch/            # file watching
│   │   └── src/lib.rs          #   notify backend + debounce → RestartIntent
│   ├── fyrer-engine/           # THE orchestration crate
│   │   └── src/
│   │       ├── engine.rs       #   single-writer event loop + EngineState
│   │       ├── scheduler.rs    #   ready-queue policy (in-degree counters)
│   │       ├── supervisor.rs   #   per-attempt process owner actor
│   │       ├── watcher.rs      #   glue: watch events → restart policy
│   │       ├── events.rs       #   EngineCommand / EngineEvent / SupCommand
│   │       └── handle.rs       #   public facade: EngineHandle, RunBuilder
│   ├── fyrer-ui/               # reporters
│   │   └── src/
│   │       ├── reporter.rs     #   Reporter trait consuming EngineEvent refs
│   │       ├── plain.rs        #   line-oriented output (CI/pipes)
│   │       ├── tui.rs          #   ratatui app (adds restart keybind)
│   │       └── json.rs         #   machine-readable stream
│   └── fyrer/                  # the CLI binary (thin!)
│       └── src/main.rs         #   clap wiring only
├── docs/, examples/, npm/ …    # unchanged
```

Dependency rules (arrows may only point downward):

```
        fyrer (cli)
   ┌───────┬────┴─────┬──────────┐
   ▼       ▼          ▼          ▼
 fyrer-ui  fyrer-engine  fyrer-config  fyrer-cache
   │        │  │  │
   │        │  │  └──▶ fyrer-watch
   │        │  └─────▶ fyrer-log
   │        └────────▶ fyrer-process
   ▼
 fyrer-core   ◀── (everything depends on this; it depends on nothing)
```

---

## 5. Domain model (`fyrer-core`)

```rust
pub struct RunId(u64);                       // unique per engine session/run
pub struct Attempt(u32);                     // nth execution within a run

#[derive(Hash, Eq, PartialEq)]
pub struct ExecKey {                         // identifies ONE process slot
    pub run: RunId,
    pub task: TaskId,
    pub attempt: Attempt,
}

pub struct TaskSpec {                        // what today's `Task` should be:
    pub id: TaskId,                          // pure data, zero behavior
    pub cmd: String,
    pub cwd: PathBuf,
    pub env: EnvMap,
    pub timeout: Option<Duration>,
    pub persistent: bool,
    pub watch: bool,
    pub cacheable: bool,
    pub depends_on: Vec<TaskId>,
    pub inputs: Vec<String>,                 // globs
    pub outputs: Vec<String>,
    pub ignore: Vec<String>,
}

pub enum TaskStatus {
    Pending,                                 // not yet schedulable (deps unmet)
    Ready,                                   // queued
    Running { attempt: Attempt },
    Succeeded { outcome: TaskOutcome },
    Failed   { outcome: TaskOutcome },
    Cached   { outcome: TaskOutcome },
    Skipped  { reason: SkipReason },         // upstream failure cascade
    Restarting { from: Attempt },            // old attempt dying, next queued
}

pub struct TaskOutcome {
    pub attempt: Attempt,
    pub exit: ExitReason,                    // Code(i32) | Signal(i32) | Timeout | Killed | SpawnError(String)
    pub started_at: SystemTime,
    pub duration: Duration,
}
```

`TaskGraph` stays roughly as today (deps + dependents + cycle detection) but
gains `transitive_deps`, `dependents_of`, and returns richer cycle errors.

---

## 6. Ownership fix #1 — the Supervisor actor

One supervisor owns one process for one attempt. It is spawned by the engine
and is the *only* code in the system that touches a `Child`.

```rust
// crates/fyrer-engine/src/supervisor.rs

pub struct SupervisorOpts {
    pub key: ExecKey,
    pub spec: Arc<TaskSpec>,
}

pub enum SupCommand {                        // engine → supervisor (mpsc)
    Kill,                                    // graceful→SIGKILL escalation
    Stdin(String),
}

pub enum SupervisorMsg {                     // supervisor → engine (one shared mpsc)
    Started   { key: ExecKey, pid: u32 },
    Exited    { key: ExecKey, outcome: TaskOutcome },
}
```

Lifecycle (all in one tokio task, mirroring today's select loop):

```
spawn(spec) ──▶ pipe stdout/stderr ──▶ select! {
                                        child.wait(),
                                        cmd_rx.recv(),        // Kill / Stdin
                                        timeout deadline,
                                      }
             ──▶ on exit: send SupervisorMsg::Exited, return.
                 Child drops HERE and nowhere else.
```

Rules enforced by construction:

- The command channel is created by the **engine** and passed *into* the
  supervisor. Only the engine's registry (`live: HashMap<TaskId, LiveHandle>`)
  holds a clone — UIs never receive capabilities.
- Log bytes go straight to the **LogRouter** (`fyrer-log`), not onto the
  control/observation bus. No more silent drops from broadcast lag.
- Process-group creation, group kill, and shell selection move into
  `fyrer-process` (pure, testable functions).
- Timeout handling stays here, but emits `ExitReason::Timeout` so policies
  upstream can distinguish it from user kills.

Because a supervisor is bound to `(RunId, TaskId, Attempt)`, restarting is
just: terminate the old supervisor (send `Kill`, await `Exited`), then spawn a
new one with `attempt + 1`. The old logs stay attributable because every log
line carries its `ExecKey`.

---

## 7. Ownership fix #2 — the Engine (single writer)

The engine replaces both today's `Orchestrator` *and* `Scheduler`. It is one
tokio task that owns all mutable run state and consumes two inputs:

```rust
// crates/fyrer-engine/src/events.rs

pub enum EngineCommand {                     // world → engine (mpsc, many producers)
    Start(RunPlan),                          // roots + options
    Restart(Vec<TaskSelector>),              // manual / TUI / API
    Kill(TaskSelector),
    Shutdown,
}

enum InternalMsg {                           // supervisors/watcher → engine (mpsc)
    Supervisor(SupervisorMsg),
    FilesChanged(TaskId, Vec<PathBuf>),
}

pub enum EngineEvent {                       // engine → world (broadcast, DATA ONLY)
    RunStarted { run: RunId, planned: Vec<TaskId> },
    TaskReady(TaskId),
    TaskStarted { id: TaskId, attempt: Attempt, pid: u32 },
    TaskFinished { id: TaskId, outcome: TaskOutcome, final_status: TaskStatus },
    TaskCacheHit { id: TaskId },
    TaskSkipped { id: TaskId, reason: SkipReason },
    TaskRestarting { id: TaskId, killed_attempt: Attempt },
    DependentsStale { ids: Vec<TaskId> },
    RunFinished(RunSummary),
}
```

### State owned by the engine

```rust
struct EngineState {
    run_id: RunId,
    specs: Arc<TaskRegistry>,                // TaskId → Arc<TaskSpec> (immutable)
    graph: TaskGraph,

    pending_deps: HashMap<TaskId, usize>,    // unmet dependency count
    ready: VecDeque<TaskId>,                 // schedulable now
    live: HashMap<TaskId, LiveHandle>,       // ONLY holder of SupCommand senders
    records: HashMap<TaskId, TaskRecord>,    // status + attempt history
    permits: Arc<Semaphore>,                 // concurrency limit
    restart_policy: RestartPolicy,           // see §9
}
```

Note there is now exactly **one** status store. The Orchestrator/Scheduler
drift problem disappears because the second store no longer exists.

### Event loop

```rust
loop {
    tokio::select! {
        Some(cmd) = cmd_rx.recv() => match cmd {
            EngineCommand::Start(plan) => self.begin_run(plan)?,
            EngineCommand::Restart(sel) => self.request_restart(sel)?,
            EngineCommand::Kill(sel)     => self.kill(sel)?,
            EngineCommand::Shutdown      => break self.drain().await?,
        },
        Some(msg) = internal_rx.recv() => match msg {
            InternalMsg::Supervisor(m) => self.on_supervisor_msg(m)?,
            InternalMsg::FilesChanged(t, p) => self.on_file_change(t, p)?,
        },
    }
    self.schedule_ready()?;                  // keep the pipeline full
}
```

### Dynamic scheduling (no more levels)

`begin_run` seeds `pending_deps` from the transitive closure of requested
roots, pushes zero-dependency tasks into `ready`. `schedule_ready` loops while
a semaphore permit is available:

```
pop task from ready
  ├─ cacheable && cache hit?      → emit TaskCacheHit, mark finished(success)
  └─ else spawn Supervisor(attempt = records[task].next_attempt())
       permit acquired; live.insert(task, LiveHandle{ cmd_tx, attempt })
```

When a supervisor reports `Exited`:

```
release permit; live.remove(task);
record outcome;
match exit {
    success | cached-ok => for each dependent: pending_deps -= 1
                            if 0 → push dependent into ready
    failure             => cascade-skip transitive dependents (emit Skipped)
}
schedule_ready()
```

A **persistent** task simply never exits, so nothing waits on it — later work
flows around it. This removes the documented "keep dev servers on leaves"
limitation outright.

---

## 8. Communication topology

```
                 EngineCommand (mpsc)              ┌──────────────────────┐
   CLI ─────────────────────────────────────────▶ │                      │
   TUI ─────────────────────────────────────────▶ │        ENGINE        │
   tests/embeddings ────────────────────────────▶ │  (single tokio task) │
                                                  │                      │
                 SupervisorMsg (shared mpsc)      │  EngineState (owned) │
   Supervisor#1 ────────────────────────────────▶ │                      │
   Supervisor#2 ────────────────────────────────▶ └──┬───────┬───────┬───┘
        ▲                                            │       │       │
        │ SupCommand (per-attempt mpsc,              │       │       │
        │  cloned sender lives ONLY in               │       │       │
        │  engine's live registry)                   │       │       │
        └────────────────────────────────────────────┘       │       │
                                                             │       │
             LogLine (mpsc)                                  │       │
   Supervisor ──────────────▶ LogRouter ──▶ ring buffers ────┘       │
                                │        └──▶ file transcripts       │
                                └──▶ sink trait (stdout/json/…)      │
                                                                     │
                 EngineEvent (broadcast, data-only)                  │
   ENGINE ─────────────────────────────────────▶ PlainUi/Tui/Json ───┘
                                                 (+ future: daemon RPC)
```

Channel inventory:

| Channel                       | Type            | Producers → Consumers        |
| ----------------------------- | --------------- | ---------------------------- |
| `EngineCommand`               | mpsc            | many → engine                |
| `InternalMsg`                 | mpsc            | supervisors/watcher → engine |
| `SupCommand`                  | per-attempt mpsc| engine → one supervisor      |
| `EngineEvent`                 | broadcast       | engine → reporters (read-only data) |
| `LogLine`                     | mpsc            | supervisors → LogRouter      |

---

## 9. Restart & watch semantics

All three triggers converge on one internal operation:

```
request_restart(selector):
  if task is live        → send SupCommand::Kill, set records[task].restart_pending
                           emit TaskRestarting
  else                   → enqueue_spawn(task, attempt+1)
  apply restart_policy to transitive dependents
```

Policies (config, `run.restart.dependents`):

| Policy        | Behavior when `web:dev` restarts                              |
| ------------- | ------------------------------------------------------------- |
| `stale` (default) | Mark dependents `Stale`; UI shows ↻; they are *not* rerun |
| `rerun`       | Re-seed `pending_deps` for dependents and re-enqueue them once |
| `none`        | Ignore                                                         |

Trigger sources:

1. **Watch mode** — `fyrer-watch` subscribes to the union of a task's
   `inputs` globs, debounces (default 300ms), and feeds
   `InternalMsg::FilesChanged`. This finally wires up the dead
   `FileChanged`/`RestartRequest` concepts from the current events module.
2. **Manual** — TUI keybind (`r` on selected task) sends `EngineCommand::Restart`.
3. **Crash policy** (future, config `run.restart.on_crash: {max, backoff}`) —
   engine inspects `ExitReason`; non-user-initiated failures respawn up to N
   times with backoff. Trivial because attempts already exist.

Failure cascades interact sanely: a restarted task clears its own
`Failed/Skipped` state first; dependents follow the policy above.

---

## 10. Subsystems

### 10.1 Logging (`fyrer-log`)

`LogRouter` is a dedicated actor consuming `LogLine { key: ExecKey, stream,
line }` over one mpsc. It maintains:

- a bounded ring buffer per task (TUI replays instantly, memory capped),
- full transcripts written to `.fyrer/logs/<run_id>/<package.task>.<attempt>.log`,
- fan-out to `Sink` implementations (plain printer, JSONL, future syslog).

This fixes silent lag-drop, bounds memory, and enables a future
`fyrer logs <task>` command reading persisted transcripts.

### 10.2 Caching (`fyrer-cache`)

- Trait becomes **async** (`async fn contains/get/save/…`), so remote
  providers (S3/GCS) need no thread-per-call hacks.
- `local.rs` keeps the tar+zstd layout but restores outputs relative to the
  **task cwd** (fixes the current-dir bug).
- Hashing gains a memo table: each task's cache key is computed once per run
  (currently the dep tree is re-hashed exponentially).
- Cache checks happen in the engine before spawning; hits emit
  `EngineEvent::TaskCacheHit` and count as successful completion for
  dependents.

### 10.3 UI (`fyrer-ui`)

```rust
pub trait Reporter: Send {
    fn start(&mut self, rx: broadcast::Receiver<EngineEvent>, logs: LogReplay)
        -> JoinHandle<Result<()>>;
}
```

Reporters consume data-only events plus a read handle into the LogRouter's
ring buffers (the TUI currently re-buffers every line itself). Because the
TUI also receives an `EngineCommand` sender, it gains affordances like
"restart task", "kill task", stdin injection — previously impossible since it
had no legitimate way to influence the run.

### 10.4 Config (`fyrer-config`)

- v1 YAML schema preserved byte-for-byte; `deny_unknown_fields` stays so
  typos still fail loudly.
- New optional keys gate new behavior (all default off): workspace-level
  `concurrency`, `run.restart.*`, per-task `watch.debounce`.
- `discovery.rs` defines a `PackageDiscovery` trait (yaml manifest today;
  package.json workspaces / cargo members / pnpm adapters later). Discovery
  runs before validation and merges into the same IR, which is what turns
  fyrer into a general monorepo tool rather than a hand-written-yaml runner.

### 10.5 Process primitives (`fyrer-process`)

Pure functions: `build_command(spec) -> Command` (shell selection unix/win),
`spawn_with_group(...)`, `kill_group(pid)` (SIGTERM→SIGKILL escalation with a
configurable grace period — today it jumps straight to SIGKILL). Windows Job
Objects remain a tracked limitation, isolated to this crate.

---

## 11. Extension points summary

| Seam              | Trait                     | Shipped today | Planned |
| ----------------- | ------------------------- | ------------- | ------- |
| Cache backend     | `CacheProvider` (async)   | local         | s3/gcs  |
| Output reporting  | `Reporter`                | plain, tui    | json, LSP-ish |
| File watching     | `Watcher`                 | notify        | polling fallback |
| Package discovery | `PackageDiscovery`        | yaml only     | npm/cargo/pnpm |
| Executor          | `Executor` (how `cmd` runs)| shell         | docker, remote agents |

Every one of these is a struct implementing a trait, registered through a
builder in `fyrer-engine/src/handle.rs`:

```rust
let engine = Engine::builder()
    .config(workspace)
    .cache(Arc::new(LocalCache::new(...)))
    .reporter(Box::new(TuiReporter::new()))
    .watcher(Box::new(NotifyWatcher::default()))
    .concurrency(8)
    .build()?;

let handle = engine.run(RunPlan::tasks(["web:dev"])).await?;
handle.restart(selector!("web:dev")).await?;
```

That builder is also the embedding API used by integration tests, replacing
today's end-to-end testing through the binary.

---

## 12. Migration plan

Each phase compiles, passes existing behavior, and ships independently.

**Phase 0 — workspace split (mechanical).**
Create `[workspace]`, move modules into crates per §4 preserving behavior.
`src/task/mod.rs` splits: pure data → `fyrer-core`, spawn logic → temp home
in `fyrer-engine`. No semantic changes. CI green = done.

**Phase 1 — Supervisor actor + capability fix.**
Introduce `supervisor.rs`; delete `AppEvent::TaskSpawned { command_tx }`;
engine's `live` map becomes the only holder of `SupCommand` senders. Logs
move off the event bus onto the LogRouter. `kill_all_tasks` becomes
`EngineCommand::Kill(all)` routed through real owners.

**Phase 2 — Engine replaces Orchestrator+Scheduler.**
Implement `EngineState` + ready-queue loop (§7). Delete level extraction from
`get_orders` usage at runtime (graph util remains for `plan`). Persistent
tasks stop blocking later levels. Single status store achieved.

**Phase 3 — logging hardening.**
Ring buffers + transcripts + bounded memory; TUI reads replay from
LogRouter. Add `-n/--json` reporter while the surface is fresh.

**Phase 4 — attempts/restart plumbing + watch.**
Add `Attempt`/`ExecKey` threading, restart policies, `r` keybind, wire
`fyrer-watch` to `inputs` globs. The `watch: true` flag starts doing what the
config always promised.

**Phase 5 — cache correctness + async trait.**
Async `CacheProvider`, restore-into-cwd fix, memoized hash tree.

**Phase 6 — extensibility round-up.**
`PackageDiscovery` trait + builder API; JSON reporter; crash-restart policy;
(optional) daemon mode storing `RunId` sessions for cross-invocation restart.

---

## 13. Risks & notes

- **TUI rewrite scope**: the ratatui worker already models `Restarting`; it
  needs `Stale` + attempt badges. Moderate, isolated to `crates/fyrer-ui`.
- **Backpressure**: supervisor→engine uses one unbounded mpsc; fine because
  messages are tiny lifecycle facts (not logs). Logs are bounded by the
  router. Document this invariant.
- **Windows**: group-kill semantics stay best-effort (CREATE_NEW_PROCESS_GROUP
  today, Job Objects later) — confined to `fyrer-process` so improvements
  don't ripple.
- **Compat**: `fyrer.yml` v1 untouched until Phase 6 adds opt-in v2 keys;
  `plan` output format preserved for scripts.
