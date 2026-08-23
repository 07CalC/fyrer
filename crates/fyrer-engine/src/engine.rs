//! Single-writer engine: owns all mutable run state and drives scheduling,
//! supervision and restarts from one event loop.
//!
//! Ownership chain: `Engine` -> supervisor (per attempt) -> child process.
//! Control arrives as [`EngineCommand`]s; observations leave as data-only
//! [`EngineEvent`]s.

use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

use anyhow::Result;
use fyrer_cache::{
    hash::{hash_file, hash_kv},
    provider::{CacheMetadata, CacheProvider, CacheStatus},
};
use fyrer_core::{
    Attempt, ExecKey, RunId, TaskGraph, TaskId,
    spec::{TaskRegistry, TaskSpec},
    status::{ExitReason, SkipReason, TaskOutcome, TaskStatus},
};
use fyrer_log::LogRouter;
use tokio::sync::{Semaphore, broadcast, mpsc};

use crate::{
    events::{EngineCommand, EngineEvent, RunPlan, RunSummary, SupCommand, SupervisorMsg},
    scheduler::SchedulerState,
    supervisor::{SupervisorOpts, spawn_supervisor},
};

static RUN_COUNTER: AtomicU64 = AtomicU64::new(1);

fn default_concurrency() -> usize {
    std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4)
}

/// Handle to one in-flight attempt. The engine registry is the only holder of
/// the command channel — UIs never receive process capabilities.
struct LiveHandle {
    cmd_tx: mpsc::Sender<SupCommand>,
}

/// Everything the engine knows about one task during a run.
struct TaskRecord {
    status: TaskStatus,
    attempts: Vec<TaskOutcome>,
    next_attempt: Attempt,
    /// Set when a Restart command killed the live attempt; the exit handler
    /// then turns a kill/timeout into a requeue instead of a failure.
    restart_pending: bool,
    /// Set by Restart commands: the next scheduling of this task must
    /// actually execute, even if the cache has a matching entry.
    force_run: bool,
}

impl TaskRecord {
    fn new() -> Self {
        Self {
            status: TaskStatus::Pending,
            attempts: Vec::new(),
            next_attempt: Attempt::first(),
            restart_pending: false,
            force_run: false,
        }
    }

    fn last_attempt(&self) -> Attempt {
        self.attempts.last().map(|o| o.attempt).unwrap_or(Attempt::first())
    }
}

/// All mutable state for one run, owned exclusively by the event loop.
struct RunState {
    scheduler: SchedulerState,
    records: HashMap<TaskId, TaskRecord>,
    live: HashMap<TaskId, LiveHandle>,
    cache_keys: HashMap<TaskId, String>,
    output_digests: HashMap<TaskId, String>,
    completed_at: Option<Duration>,
}

impl RunState {
    fn new(scheduler: SchedulerState) -> Self {
        let records = scheduler
            .relevant
            .iter()
            .map(|id| (id.clone(), TaskRecord::new()))
            .collect();
        Self {
            scheduler,
            records,
            live: HashMap::new(),
            cache_keys: HashMap::new(),
            output_digests: HashMap::new(),
            completed_at: None,
        }
    }

    /// True when nothing is running, nothing is queued and every record is
    /// terminal (or stale from an upstream restart).
    fn is_done(&self) -> bool {
        self.live.is_empty()
            && self.scheduler.ready.is_empty()
            && self.records.values().all(|r| r.status.is_terminal() || r.status == TaskStatus::Stale)
    }

    /// Requeue a finished/stale task for its next attempt. Returns false for
    /// tasks that are live or not yet started.
    fn requeue(&mut self, id: &TaskId) -> bool {
        let Some(rec) = self.records.get_mut(id) else {
            return false;
        };
        if !rec.status.is_terminal() && rec.status != TaskStatus::Stale {
            return false;
        }
        rec.status = TaskStatus::Pending;
        rec.next_attempt = rec.last_attempt().next();
        self.scheduler.push_ready(id.clone());
        true
    }
}

#[derive(Clone)]
pub struct Engine {
    registry: Arc<TaskRegistry>,
    graph: TaskGraph,
    cache: Arc<dyn CacheProvider>,
    log_router: Arc<LogRouter>,
    event_tx: broadcast::Sender<EngineEvent>,
    concurrency: usize,
    /// Directory holding the config file; cache entries are stored and
    /// restored relative to this.
    workspace_root: std::path::PathBuf,
}

impl Engine {
    pub fn new(
        registry: TaskRegistry,
        graph: TaskGraph,
        cache: Arc<dyn CacheProvider>,
        log_router: Arc<LogRouter>,
        event_tx: broadcast::Sender<EngineEvent>,
        concurrency: Option<usize>,
        workspace_root: Option<std::path::PathBuf>,
    ) -> Self {
        Self {
            registry: Arc::new(registry),
            graph,
            cache,
            log_router,
            event_tx,
            concurrency: concurrency.unwrap_or_else(default_concurrency),
            workspace_root: workspace_root.unwrap_or_else(|| {
                std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."))
            }),
        }
    }

    /// One-shot run: exits as soon as every task is terminal. No control.
    pub async fn run_once(&self, plan: RunPlan) -> Result<RunSummary> {
        let (_cmd_tx, cmd_rx) = mpsc::channel::<EngineCommand>(16);
        self.clone().run_with_receiver_inner(plan, cmd_rx, false).await
    }

    /// Interactive run: keeps serving commands after completion (TUI browsing,
    /// watch mode). Exits on [`EngineCommand::Shutdown`] or channel close.
    pub async fn run_with_receiver(
        self,
        plan: RunPlan,
        cmd_rx: mpsc::Receiver<EngineCommand>,
    ) -> Result<RunSummary> {
        self.run_with_receiver_inner(plan, cmd_rx, true).await
    }

    pub(crate) async fn run_with_receiver_inner(
        self,
        plan: RunPlan,
        mut cmd_rx: mpsc::Receiver<EngineCommand>,
        wait_after_done: bool,
    ) -> Result<RunSummary> {
        let run_id = RunId::new(RUN_COUNTER.fetch_add(1, Ordering::Relaxed));
        let roots = self.resolve_roots(&plan)?;
        let start = Instant::now();
        let _ = self.event_tx.send(EngineEvent::RunStarted {
            run: run_id,
            planned: roots.clone(),
        });

        let scheduler = SchedulerState::new(self.graph.clone(), &roots);
        let mut st = RunState::new(scheduler);
        let semaphore = Arc::new(Semaphore::new(self.concurrency));
        let (sup_tx, mut sup_rx) = mpsc::unbounded_channel::<SupervisorMsg>();

        self.pump_ready(run_id, &mut st, &sup_tx, &semaphore).await;

        loop {
            // Stamp + announce completion on the fresh done-transition so the
            // TUI summary appears immediately; interactive engines keep
            // serving commands afterwards. Post-run restarts re-trigger this.
            if st.is_done() {
                if st.completed_at.is_none() {
                    st.completed_at = Some(start.elapsed());
                    let duration = st.completed_at.unwrap_or_default();
                    let summary = build_summary(&st.records, duration);
                    let _ = self.event_tx.send(EngineEvent::RunCompleted(summary));
                }
                if !wait_after_done {
                    break;
                }
            }

            tokio::select! {
                maybe_cmd = cmd_rx.recv() => match maybe_cmd {
                    Some(cmd) => {
                        if !self.handle_command(run_id, &mut st, cmd, &sup_tx, &semaphore).await {
                            break;
                        }
                    }
                    None => {
                        // Command channel closed: nobody can control us anymore.
                        self.kill_all(&mut st).await;
                        break;
                    }
                },
                _ = tokio::signal::ctrl_c() => {
                    // Kill children but stay in the loop until their exits
                    // arrive through `sup_rx`.
                    self.kill_all(&mut st).await;
                },
                Some(msg) = sup_rx.recv() => {
                    if let SupervisorMsg::Exited { key, outcome } = msg {
                        // One permit per finished attempt.
                        semaphore.add_permits(1);
                        self.on_task_exit(run_id, &mut st, key.task, outcome, &sup_tx, &semaphore).await;
                    }
                },
            }
        }

        // Prefer the stamped completion time; fall back to exit time when we
        // were shut down mid-run.
        let duration = st.completed_at.unwrap_or_else(|| start.elapsed());
        let summary = build_summary(&st.records, duration);
        let _ = self.event_tx.send(EngineEvent::RunFinished(summary.clone()));
        Ok(summary)
    }

    fn resolve_roots(&self, plan: &RunPlan) -> Result<Vec<TaskId>> {
        let roots = plan.task_ids.clone();
        if roots.is_empty() {
            anyhow::bail!("No tasks found for the given specifier");
        }
        for id in &roots {
            if !self.graph.contains(id) {
                anyhow::bail!("Task {id} not found");
            }
        }
        Ok(roots)
    }

    /// Send Kill to every live attempt. Best-effort: dead channels are ignored.
    async fn kill_all(&self, st: &mut RunState) {
        for handle in st.live.values() {
            let _ = handle.cmd_tx.send(SupCommand::Kill).await;
        }
    }

    /// Apply an external command. Returns false when the engine must stop.
    async fn handle_command(
        &self,
        run_id: RunId,
        st: &mut RunState,
        cmd: EngineCommand,
        sup_tx: &mpsc::UnboundedSender<SupervisorMsg>,
        semaphore: &Arc<Semaphore>,
    ) -> bool {
        match cmd {
            EngineCommand::Restart(ids) => {
                for id in ids {
                    if !st.scheduler.is_relevant(&id) {
                        continue;
                    }
                    let restarted = if let Some(handle) = st.live.get(&id) {
                        // Live attempt: mark it, kill it — the exit handler
                        // requeues instead of failing.
                        if let Some(rec) = st.records.get_mut(&id) {
                            rec.restart_pending = true;
                            rec.force_run = true;
                        }
                        let _ = handle.cmd_tx.send(SupCommand::Kill).await;
                        true
                    } else if st.requeue(&id) {
                        if let Some(rec) = st.records.get_mut(&id) {
                            rec.force_run = true;
                        }
                        let _ = self.event_tx.send(EngineEvent::TaskRestarting {
                            id: id.clone(),
                            killed_attempt: st
                                .records
                                .get(&id)
                                .map(|r| r.last_attempt())
                                .unwrap_or(Attempt::first()),
                        });
                        true
                    } else {
                        false
                    };

                    // Documented default policy (`stale`): finished dependents
                    // of a restarted task are marked stale so reporters can
                    // show that their inputs are outdated. They are not
                    // re-run automatically.
                    if restarted {
                        let stale: Vec<TaskId> = st
                            .scheduler
                            .transitive_dependents_to_skip(&id)
                            .into_iter()
                            .filter(|dep| {
                                st.records.get(dep).is_some_and(|rec| {
                                    matches!(
                                        rec.status,
                                        TaskStatus::Succeeded { .. } | TaskStatus::Cached { .. }
                                    )
                                })
                            })
                            .collect();
                        for dep in &stale {
                            if let Some(rec) = st.records.get_mut(dep) {
                                rec.status = TaskStatus::Stale;
                            }
                        }
                        if !stale.is_empty() {
                            let _ = self.event_tx.send(EngineEvent::DependentsStale { ids: stale });
                        }
                    }
                }
                self.pump_ready(run_id, st, sup_tx, semaphore).await;
                true
            }
            EngineCommand::Kill(ids) => {
                for id in ids {
                    if let Some(handle) = st.live.get(&id) {
                        let _ = handle.cmd_tx.send(SupCommand::Kill).await;
                    }
                }
                true
            }
            EngineCommand::Shutdown => {
                self.kill_all(st).await;
                false
            }
            EngineCommand::Start(_) => true,
        }
    }

    /// Handle a supervisor reporting that its process exited.
    async fn on_task_exit(
        &self,
        run_id: RunId,
        st: &mut RunState,
        task_id: TaskId,
        outcome: TaskOutcome,
        sup_tx: &mpsc::UnboundedSender<SupervisorMsg>,
        semaphore: &Arc<Semaphore>,
    ) {
        st.live.remove(&task_id);
        let Some(rec) = st.records.get_mut(&task_id) else {
            return;
        };
        rec.attempts.push(outcome.clone());

        let restart_pending = rec.restart_pending;
        let final_status = classify_exit(&outcome, restart_pending);

        if matches!(final_status, TaskStatus::Restarting { .. }) {
            // Restart requested while live: requeue for the next attempt.
            rec.restart_pending = false;
            rec.status = TaskStatus::Pending;
            rec.next_attempt = outcome.attempt.next();
            let _ = self.event_tx.send(EngineEvent::TaskRestarting {
                id: task_id.clone(),
                killed_attempt: outcome.attempt,
            });
            st.scheduler.push_ready(task_id.clone());
            self.pump_ready(run_id, st, sup_tx, semaphore).await;
            return;
        }

        rec.status = final_status.clone();
        let _ = self.event_tx.send(EngineEvent::TaskFinished {
            id: task_id.clone(),
            outcome: outcome.clone(),
            final_status: final_status.clone(),
        });

        if matches!(final_status, TaskStatus::Succeeded { .. }) {
            if let Some(spec) = self.registry.get(&task_id) {
                if spec.cacheable {
                    self.save_to_cache(&spec, &mut st.cache_keys, &outcome).await;
                }
            }
            // A late success unblocks dependents; skipped ones may retry now.
            self.unskip_direct_dependents(st, &task_id);
            st.scheduler.on_success(&task_id);
        } else if matches!(final_status, TaskStatus::Failed { .. }) {
            // Cascade: nothing downstream of a failure can run this pass.
            self.cascade_skip(st, &task_id);
        }

        self.pump_ready(run_id, st, sup_tx, semaphore).await;
    }

    /// Reset direct dependents that were skipped earlier (e.g. before a
    /// restart repaired the upstream failure) so they can be scheduled again.
    fn unskip_direct_dependents(&self, st: &mut RunState, task_id: &TaskId) {
        for dep in self.graph.dependents_of(task_id) {
            if let Some(rec) = st.records.get_mut(&dep) {
                if matches!(rec.status, TaskStatus::Skipped { .. }) {
                    rec.status = TaskStatus::Pending;
                }
            }
        }
    }

    fn cascade_skip(&self, st: &mut RunState, failed: &TaskId) {
        for dep in st.scheduler.transitive_dependents_to_skip(failed) {
            if let Some(rec) = st.records.get_mut(&dep) {
                if matches!(rec.status, TaskStatus::Pending | TaskStatus::Ready) {
                    rec.status = TaskStatus::Skipped {
                        reason: SkipReason::UpstreamFailed,
                    };
                    let _ = self.event_tx.send(EngineEvent::TaskSkipped {
                        id: dep,
                        reason: SkipReason::UpstreamFailed,
                    });
                }
            }
        }
    }

    /// Drain the ready queue while concurrency permits are available: skip
    /// blocked tasks, serve cache hits, spawn supervisors for the rest.
    async fn pump_ready(
        &self,
        run_id: RunId,
        st: &mut RunState,
        sup_tx: &mpsc::UnboundedSender<SupervisorMsg>,
        semaphore: &Arc<Semaphore>,
    ) {
        while let Some(task_id) = st.scheduler.pop_ready() {
            let Some(spec) = self.registry.get(&task_id) else {
                continue;
            };
            let spec = spec.clone();

            if self.is_blocked(st, &spec) {
                let Some(rec) = st.records.get_mut(&task_id) else {
                    continue;
                };
                rec.status = TaskStatus::Skipped {
                    reason: SkipReason::UpstreamFailed,
                };
                let _ = self.event_tx.send(EngineEvent::TaskSkipped {
                    id: task_id,
                    reason: SkipReason::UpstreamFailed,
                });
                continue;
            }

            // Already terminal or currently live? Nothing to do.
            if let Some(rec) = st.records.get(&task_id) {
                if rec.status.is_terminal() || st.live.contains_key(&task_id) {
                    continue;
                }
            }

            let force_run = st
                .records
                .get(&task_id)
                .is_some_and(|rec| rec.force_run);

            if spec.cacheable && !force_run && self.try_cache_hit(&spec, st).await {
                let Some(rec) = st.records.get_mut(&task_id) else {
                    continue;
                };
                rec.status = TaskStatus::Cached {
                    attempt: rec.next_attempt,
                };
                let _ = self.event_tx.send(EngineEvent::TaskCacheHit { id: task_id.clone() });
                // A hit counts as success for dependents.
                st.scheduler.on_success(&task_id);
                continue;
            }
            // Either not cacheable or an explicit restart: run for real.
            if force_run {
                if let Some(rec) = st.records.get_mut(&task_id) {
                    rec.force_run = false;
                }
            }

            // No free permit? Put the task back and wait for an exit.
            let Ok(permit) = semaphore.clone().try_acquire_owned() else {
                st.scheduler.push_ready(task_id);
                break;
            };
            // The permit is released via `add_permits(1)` in `on_task_exit`.
            std::mem::forget(permit);

            let attempt = match st.records.get(&task_id) {
                Some(rec) => rec.next_attempt,
                None => continue,
            };
            {
                let Some(rec) = st.records.get_mut(&task_id) else {
                    continue;
                };
                rec.status = TaskStatus::Running { attempt };
                rec.next_attempt = attempt.next();
            }

            let key = ExecKey::new(run_id, task_id.clone(), attempt);
            let (cmd_tx, cmd_rx) = mpsc::channel(4);
            let opts = SupervisorOpts {
                key: key.clone(),
                spec: Arc::clone(&spec),
            };
            spawn_supervisor(opts, cmd_rx, sup_tx.clone(), self.log_router.sender(), self.event_tx.clone());
            st.live.insert(task_id.clone(), LiveHandle { cmd_tx });

            let _ = self.event_tx.send(EngineEvent::TaskStarted {
                id: task_id,
                attempt,
                pid: 0,
            });
        }
    }

    /// A task is blocked when any direct dependency failed or was skipped.
    fn is_blocked(&self, st: &RunState, spec: &TaskSpec) -> bool {
        spec.depends_on.iter().any(|dep| {
            st.records
                .get(dep)
                .is_some_and(|r| matches!(r.status, TaskStatus::Failed { .. } | TaskStatus::Skipped { .. }))
        })
    }

    /// Check the cache for a task. On a hit with stale outputs, restore them
    /// into the task's cwd. Returns true on a usable hit.
    async fn try_cache_hit(&self, spec: &TaskSpec, st: &mut RunState) -> bool {
        let Some(key) = self.cache_key_for(spec, &mut st.cache_keys) else {
            return false;
        };
        if !self.cache.contains(&key) {
            return false;
        }
        let digest = output_digest(spec, &mut st.output_digests);
        match self.cache.need_hydration(&key, &digest) {
            Ok(false) => true,
            Ok(true) => match self.cache.restore(&key, &self.workspace_root) {
                Ok(restored) => {
                    restored
                }
                Err(e) => {
                    self.report_error(&spec.id, format!("failed to restore cache: {e}"));
                    false
                }
            },
            Err(e) => {
                self.report_error(&spec.id, format!("failed to check hydration: {e}"));
                false
            }
        }
    }

    async fn save_to_cache(&self, spec: &TaskSpec, cache_keys: &mut HashMap<TaskId, String>, outcome: &TaskOutcome) {
        let Some(key) = self.cache_key_for(spec, cache_keys) else {
            return;
        };
        let digest = output_digest(spec, &mut HashMap::new());
        let metadata = CacheMetadata::new(
            spec.id.to_string(),
            outcome.duration.as_millis(),
            outcome.exit_code,
            CacheStatus::Miss,
            key.clone(),
            digest,
            chrono::Utc::now().timestamp_millis() as u64,
        );
        if let Err(e) = self
            .cache
            .save(&key, &resolve_outputs(spec), &self.workspace_root, metadata)
        {
            self.report_error(&spec.id, format!("failed to save cache: {e}"));
        }
    }

    /// Memoized blake3 over id + cmd + cwd + env + input file contents.
    fn cache_key_for(&self, spec: &TaskSpec, memo: &mut HashMap<TaskId, String>) -> Option<String> {
        if let Some(key) = memo.get(&spec.id) {
            return Some(key.clone());
        }
        let mut hasher = blake3::Hasher::new();
        hasher.update(spec.id.to_string().as_bytes());
        hasher.update(spec.cmd.as_bytes());
        hasher.update(spec.cwd.to_string_lossy().as_bytes());
        let mut env: Vec<_> = spec.env.iter().collect();
        env.sort_by_key(|(k, _)| *k);
        for (k, v) in env {
            hash_kv(&mut hasher, k, v);
        }
        for path in resolve_inputs(spec) {
            if path.is_file() {
                if let Err(e) = hash_file(&mut hasher, &path) {
                    self.report_error(&spec.id, format!("failed to compute cache key: {e}"));
                    return None;
                }
            }
        }
        let key = hasher.finalize().to_hex().to_string();
        memo.insert(spec.id.clone(), key.clone());
        Some(key)
    }

    fn report_error(&self, task: &TaskId, error: String) {
        let _ = self.event_tx.send(EngineEvent::NonFatalError {
            task_id: Some(task.clone()),
            error,
        });
    }
}

/// Map a process exit to the task status it implies.
fn classify_exit(outcome: &TaskOutcome, restart_pending: bool) -> TaskStatus {
    match &outcome.exit {
        ExitReason::Success(0) => TaskStatus::Succeeded {
            attempt: outcome.attempt,
        },
        // A deliberate kill (restart request) arrives as Timeout/Killed on the
        // control path or Signal(SIGKILL) from the process-group kill.
        ExitReason::Timeout | ExitReason::Killed | ExitReason::Signal(_)
            if restart_pending =>
        {
            TaskStatus::Restarting { from: outcome.attempt }
        }
        _ => TaskStatus::Failed {
            attempt: outcome.attempt,
        },
    }
}

fn build_summary(records: &HashMap<TaskId, TaskRecord>, duration: Duration) -> RunSummary {
    let mut successful = 0;
    let mut cached = 0;
    let mut failed = 0;
    let mut skipped = 0;
    for rec in records.values() {
        match rec.status {
            TaskStatus::Succeeded { .. } => successful += 1,
            TaskStatus::Cached { .. } => cached += 1,
            TaskStatus::Failed { .. } => failed += 1,
            TaskStatus::Skipped { .. } => skipped += 1,
            _ => {}
        }
    }
    RunSummary {
        total: records.len(),
        successful,
        cached,
        failed,
        skipped,
        duration,
    }
}

// --- glob resolution -------------------------------------------------------

/// Expand one glob relative to the task cwd. Rust's glob needs explicit file
/// matching after a bare `/**`, so trailing-`**` patterns gain extra arms —
/// which can overlap, hence the dedupe.
fn glob_with_patterns(cwd: &std::path::Path, pattern: &str) -> Vec<std::path::PathBuf> {
    let base = cwd.join(pattern).to_string_lossy().to_string();
    let mut patterns = vec![base.clone()];
    if base.ends_with("/**") {
        patterns.push(format!("{}/*", base.trim_end_matches("/**")));
        patterns.push(format!("{base}/**/*"));
    }
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for pattern in patterns {
        if let Ok(paths) = glob::glob(&pattern) {
            for path in paths.flatten() {
                if seen.insert(path.clone()) {
                    out.push(path);
                }
            }
        }
    }
    out
}

fn resolve_outputs(spec: &TaskSpec) -> Vec<std::path::PathBuf> {
    spec.outputs.iter().flat_map(|p| glob_with_patterns(&spec.cwd, p)).collect()
}

fn resolve_inputs(spec: &TaskSpec) -> Vec<std::path::PathBuf> {
    let ignored: std::collections::HashSet<_> = spec
        .ignore
        .iter()
        .flat_map(|p| glob_with_patterns(&spec.cwd, p))
        .collect();
    resolve_outputs_like_inputs(spec)
        .into_iter()
        .filter(|p| !ignored.contains(p))
        .collect()
}

fn resolve_outputs_like_inputs(spec: &TaskSpec) -> Vec<std::path::PathBuf> {
    spec.inputs.iter().flat_map(|p| glob_with_patterns(&spec.cwd, p)).collect()
}

/// Memoized blake3 over produced files. Cheap enough not to need caching.
fn output_digest(spec: &TaskSpec, _memo: &mut HashMap<TaskId, String>) -> String {
    let mut hasher = blake3::Hasher::new();
    for path in resolve_outputs(spec) {
        if path.is_file() {
            if let Err(e) = hash_file(&mut hasher, &path) {
                eprintln!("[engine] output digest failed for {}: {e}", spec.id);
            }
        }
    }
    hasher.finalize().to_hex().to_string()
}
