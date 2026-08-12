# fyrer

A declarative, fast and lightweight monorepo tool that runs multiple dev
servers and build tasks concurrently.

`fyrer` reads a `fyrer.yml` file describing the packages in a monorepo and
their tasks, resolves the dependency graph between tasks, runs each level of
the graph concurrently, and streams every task's output through a colorized,
prefixed logger. Long-running dev servers keep running until you quit, and
cached builds are skipped when their inputs haven't changed.

## Installation

### Install using cargo

```bash
cargo install fyrer
```

### Build from source

```bash
git clone https://github.com/07calc/fyrer
cd fyrer
cargo build --release
cargo install --path .
```

## Quick start

A runnable demo monorepo lives in [`examples/acme-corp`](./examples/acme-corp):
four languages (Bun/TS, Rust, Go, Python), shared packages, dev servers,
cached builds, `.env` parsing, `cwd`, and a `timeout` demo.

```bash
cd examples/acme-corp
fyrer list                          # see every package and task
fyrer plan build                    # show the execution plan without running
fyrer run build                     # build everything (second run is cached)
fyrer run dev                       # all dev servers at once; q / Ctrl+C to stop
```

## Usage

`fyrer` looks for a `fyrer.yml` file in the current directory. A full example
is available at [`fyrer.example.yml`](./fyrer.example.yml). Package roots and
task globs are resolved relative to the current working directory, so run
`fyrer` from the directory that contains the config.

```bash
fyrer list                          # list every package and its tasks
fyrer plan <task?>                  # print the topological execution plan
fyrer run <task?>                   # execute tasks and stream their logs
```

The `<task>` specifier comes in three forms:

| Specifier          | Meaning                                            |
| ------------------ | -------------------------------------------------- |
| *(empty)*          | run every task in every package                    |
| `build`            | run the `build` task of **every** package that has one |
| `web:dev`          | run exactly one task, `web:dev` (`package:task`)   |

Flags:

```bash
fyrer --config path/to/fyrer.yml run   # point at a different config file
fyrer run build -n                     # plain (non-TUI) output, for CI/pipes
```

## Configuration

```yaml
version: 1                      # config format version (must be 1)
cache:                          # build cache configuration
  provider: local               # only "local" is available for now
env:                            # variables shared by every package
  NODE_ENV: development
packages:                       # packages that make up the monorepo
  - name: web                   # unique package name
    root: ./apps/web            # relative package root (must exist)
    env:                        # variables shared by every task
      PORT: "3000"
    env_file: .env              # .env file path, relative to root
    tasks:                      # map of task name -> task config
      dev:
        cmd: bun run dev        # shell command ($SHELL -c) to run
        depends_on:             # tasks that must finish first
          - ui:build            #   "package:task" ...
          - check               #   ... or a bare "task" in this package
        inputs:                 # globs of files that define the task's inputs
          - src/**
        outputs:                # globs of files the task produces
          - dist/**
        ignore:                 # globs excluded from inputs/outputs
          - node_modules/**
        cache: false            # allow skipping when inputs haven't changed
        persistent: true        # long-running: keep alive until quit
        watch: true             # watched: restart on input changes (see note)
        timeout: 30s            # kill the task after this duration
        cwd: subdir             # run the command from a subdirectory of root
        env:                    # per-task variables (highest precedence)
          PORT: "8080"
        env_file: .env.local    # per-task .env file, relative to root
```

### Task options

| Option        | Description                                                                  |
| ------------- | ---------------------------------------------------------------------------- |
| `cmd`         | Required. Command run via `sh -c` (Unix) or `cmd /C` (Windows).              |
| `depends_on`  | Task IDs that must finish first. Accepts `package:task` or a bare `task`.    |
| `inputs`      | Globs (relative to the package root) included in the cache key.              |
| `outputs`     | Globs (relative to the package root) of files produced by the task.          |
| `ignore`      | Globs excluded from `inputs`/`outputs`.                                      |
| `cache`       | If `true`, a successful run is cached and skipped on future identical runs.  |
| `persistent`  | Marks a long-running task (used with dev servers). Cannot combine with `cache`. |
| `watch`       | Marks a task that restarts when watched inputs change. Cannot combine with `cache`. |
| `timeout`     | Kill the task (SIGKILL to its process group) after this duration.            |
| `cwd`         | Working directory for the command, relative to the package root.             |
| `env`         | Per-task environment variables (highest precedence).                         |
| `env_file`    | Per-task `.env` file, relative to the package root.                          |

Validation rules: package names and task names must be unique; package roots
must exist and be relative; `env_file` must exist and be relative; `cwd` must
exist inside the package root; `timeout` must be greater than zero; `inputs`,
`outputs` and `ignore` must be valid glob patterns; and `cache` cannot be
combined with `persistent` or `watch`.

### Environment precedence

From lowest to highest:

1. root-level `env`
2. package `env_file`
3. package-level `env`
4. task `env_file`
5. task-level `env`

`.env` files are plain `KEY=VALUE` files; blank lines and lines starting with
`#` are ignored.

## Caching

Tasks with `cache: true` are keyed by a **blake3** hash of the task id,
command, working directory, resolved environment, the contents of every file
matched by `inputs`, and the cache keys of every dependency. Outputs are
archived under `.fyrer/cache/` (a local provider for now).

On a later run, a task whose cache key matches a previous successful run is
reported as `⚡ Cached` instead of executing again. If its `outputs` are
missing or stale, they are restored from the cache before the run is skipped.

## Interactive TUI

By default `fyrer run` opens a full-screen TUI with a task list on the left
and the selected task's logs on the right:

- `q` or `Ctrl+C` — quit (kills all spawned process groups)
- `j` / `k` or `↑` / `↓` — select the previous / next task
- `u` / `d` or `PageUp` / `PageDown` — scroll the log pane
- `g` — jump to the top of the log; `G` — tail (follow)
- mouse wheel — scroll the log
- `Enter` — after the run finishes, browse each task's logs

When a run completes, a summary popup shows success / failure / cached /
skipped counts and total duration. Pass `-n` (`--no-tui`) for plain,
prefixed, colorized log output instead.

## Features

- Run multiple development servers and build tasks concurrently
- Declarative YAML-based configuration with validation
- Dependency-aware topological ordering, with failure propagation (tasks
  whose dependencies failed are skipped)
- Automatic `.env` file parsing at the package and task level
- Content-addressed local build cache with output restore
- Per-task `timeout` and working directory (`cwd`) support
- Colorized, prefixed log output in plain mode, or a full TUI
- Graceful shutdown on `SIGINT`/`SIGTERM`, killing spawned process groups

### Notes and limitations

- Tasks in the same graph level run concurrently, and a level finishes before
  the next one starts. A `persistent` task therefore blocks any later levels,
  so keep dev servers on leaves of the graph.
- The `watch` flag and the `FileChanged`/`RestartRequest` events are part of
  the config schema and UI, but the file-watcher that triggers automatic
  restarts is not wired up yet.

## License

MIT