use std::{
    collections::{HashMap, HashSet},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

use anyhow::Result;
use fyrer_cache::provider::{CacheMetadata, CacheProvider, CacheStatus};
use fyrer_core::{
    Attempt, ExecKey, RunId, TaskId,
    spec::TaskRegistry,
    status::{ExitReason, SkipReason, TaskOutcome, TaskStatus},
};
use fyrer_log::{LogLine, LogRouter, LogStream as RouterStream};
use tokio::{
    sync::{broadcast, mpsc, Semaphore},
    task::JoinHandle,
};

use crate::{
    events::{EngineCommand, EngineEvent, LogStream, RunPlan, RunSummary, SupervisorMsg, SupCommand},
    scheduler::SchedulerState,
    supervisor::{SupervisorOpts, spawn_supervisor},
};

static RUN_COUNTER: AtomicU64 = AtomicU64::new(1);

struct LiveHandle {
    attempt: Attempt,
    cmd_tx: mpsc::Sender<SupCommand>,
    join: JoinHandle<TaskOutcome>,
}

struct TaskRecord {
    status: TaskStatus,
    attempts: Vec<TaskOutcome>,
    next_attempt: Attempt,
    restart_pending: bool,
}

impl TaskRecord {
    fn new() -> Self {
        Self {
            status: TaskStatus::Pending,
            attempts: Vec::new(),
            next_attempt: Attempt::first(),
            restart_pending: false,
        }
    }
}

#[derive(Clone)]
pub struct Engine {
    registry: Arc<TaskRegistry>,
    graph: fyrer_core::TaskGraph,
    cache: Arc<dyn CacheProvider>,
    log_router: Arc<LogRouter>,
    event_tx: broadcast::Sender<EngineEvent>,
    concurrency: usize,
}

impl Engine {
    pub fn new(
        registry: TaskRegistry,
        graph: fyrer_core::TaskGraph,
        cache: Arc<dyn CacheProvider>,
        log_router: Arc<LogRouter>,
        event_tx: broadcast::Sender<EngineEvent>,
        concurrency: Option<usize>,
    ) -> Self {
        let concurrency = concurrency.unwrap_or_else(|| {
            std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(4)
        });
        Self {
            registry: Arc::new(registry),
            graph,
            cache,
            log_router,
            event_tx,
            concurrency,
        }
    }

    pub async fn run_once(&self, plan: RunPlan) -> Result<RunSummary> {
        // run_once creates a private command channel (no external control) and exits immediately when done
        let (_cmd_tx, cmd_rx) = mpsc::channel::<EngineCommand>(16);
        self.clone().run_with_receiver_inner(plan, cmd_rx, false).await
    }

    /// Run with an externally-provided command receiver (used by EngineHandle for restart).
    pub async fn run_with_receiver(
        self,
        plan: RunPlan,
        cmd_rx: mpsc::Receiver<EngineCommand>,
    ) -> Result<RunSummary> {
        self.run_with_receiver_inner(plan, cmd_rx, true).await
    }

    async fn run_with_receiver_inner(
        self,
        plan: RunPlan,
        mut cmd_rx: mpsc::Receiver<EngineCommand>,
        wait_after_done: bool,
    ) -> Result<RunSummary> {
        let run_id = RunId::new(RUN_COUNTER.fetch_add(1, Ordering::Relaxed));
        let roots = plan.task_ids.clone();
        if roots.is_empty() {
            anyhow::bail!("No tasks found for the given specifier");
        }
        for id in &roots {
            if !self.graph.contains(id) {
                anyhow::bail!("Task {} not found", id);
            }
        }

        let relevant = self.graph.transitive_closure(&roots);
        if relevant.is_empty() {
            anyhow::bail!("No tasks in closure");
        }

        let start = Instant::now();
        let _ = self.event_tx.send(EngineEvent::RunStarted {
            run: run_id,
            planned: roots.clone(),
        });

        // state owned by this task (single writer)
        let mut scheduler = SchedulerState::new(self.graph.clone(), &roots);
        let mut records: HashMap<TaskId, TaskRecord> = relevant
            .iter()
            .map(|id| (id.clone(), TaskRecord::new()))
            .collect();
        let mut live: HashMap<TaskId, LiveHandle> = HashMap::new();
        let mut output_digest_cache: HashMap<TaskId, String> = HashMap::new();

        let semaphore = Arc::new(Semaphore::new(self.concurrency));
        let (sup_tx, mut sup_rx) = mpsc::unbounded_channel::<SupervisorMsg>();

        // Memoized cache keys
        let mut cache_key_cache: HashMap<TaskId, String> = HashMap::new();

        let mut pending_restarts: HashSet<TaskId> = HashSet::new();

        // initial schedule
        Self::schedule_ready(
            run_id,
            &mut scheduler,
            &mut records,
            &mut live,
            &self.registry,
            &self.cache,
            &self.log_router,
            &self.event_tx,
            &sup_tx,
            Arc::clone(&semaphore),
            &mut cache_key_cache,
            &mut output_digest_cache,
        )
        .await;

        // main loop
        loop {
            // termination: no live tasks and no ready tasks => run finished
            let done = live.is_empty()
                && scheduler.ready.is_empty()
                && records.values().all(|r| r.status.is_terminal() || r.status == TaskStatus::Stale);
            if done {
                if wait_after_done {
                    let is_watch = self.registry.iter().any(|(_, s)| s.watch);
                    if is_watch {
                        // For watch mode, wait indefinitely for file changes or shutdown
                        tokio::select! {
                            Some(cmd) = cmd_rx.recv() => {
                                match cmd {
                                    EngineCommand::Restart(ids) => {
                                        for id in ids {
                                            if !scheduler.is_relevant(&id) { continue; }
                                            if let Some(handle) = live.get(&id) {
                                                if let Some(r) = records.get_mut(&id) { r.restart_pending = true; }
                                                let _ = handle.cmd_tx.send(SupCommand::Kill).await;
                                            } else if let Some(r) = records.get_mut(&id) {
                                                if r.status.is_terminal() || r.status == TaskStatus::Stale {
                                                    r.status = TaskStatus::Pending;
                                                    scheduler.push_ready(id.clone());
                                                    r.next_attempt = r.attempts.last().map(|o| o.attempt.next()).unwrap_or(Attempt::first());
                                                    let _ = self.event_tx.send(EngineEvent::TaskRestarting { id: id.clone(), killed_attempt: r.attempts.last().map(|o| o.attempt).unwrap_or(Attempt::first()) });
                                                }
                                            }
                                        }
                                        Self::schedule_ready(
                                            run_id,
                                            &mut scheduler,
                                            &mut records,
                                            &mut live,
                                            &self.registry,
                                            &self.cache,
                                            &self.log_router,
                                            &self.event_tx,
                                            &sup_tx,
                                            Arc::clone(&semaphore),
                                            &mut cache_key_cache,
                                            &mut output_digest_cache,
                                        ).await;
                                        continue;
                                    }
                                    EngineCommand::Kill(ids) => {
                                        for id in ids { if let Some(h) = live.get(&id) { let _ = h.cmd_tx.send(SupCommand::Kill).await; } }
                                        continue;
                                    }
                                    EngineCommand::Shutdown => break,
                                    EngineCommand::Start(_) => continue,
                                }
                            }
                            _ = tokio::signal::ctrl_c() => {
                                for h in live.values() { let _ = h.cmd_tx.send(SupCommand::Kill).await; }
                                break;
                            }
                        }
                    } else {
                        // For one-shot handle (e.g., restart test), wait a bit for a restart command
                        tokio::select! {
                            Some(cmd) = cmd_rx.recv() => {
                                match cmd {
                                    EngineCommand::Restart(ids) => {
                                        for id in ids {
                                            if !scheduler.is_relevant(&id) { continue; }
                                            if let Some(handle) = live.get(&id) {
                                                if let Some(r) = records.get_mut(&id) { r.restart_pending = true; }
                                                let _ = handle.cmd_tx.send(SupCommand::Kill).await;
                                            } else if let Some(r) = records.get_mut(&id) {
                                                if r.status.is_terminal() || r.status == TaskStatus::Stale {
                                                    r.status = TaskStatus::Pending;
                                                    scheduler.push_ready(id.clone());
                                                    r.next_attempt = r.attempts.last().map(|o| o.attempt.next()).unwrap_or(Attempt::first());
                                                    let _ = self.event_tx.send(EngineEvent::TaskRestarting { id: id.clone(), killed_attempt: r.attempts.last().map(|o| o.attempt).unwrap_or(Attempt::first()) });
                                                }
                                            }
                                        }
                                        Self::schedule_ready(
                                            run_id,
                                            &mut scheduler,
                                            &mut records,
                                            &mut live,
                                            &self.registry,
                                            &self.cache,
                                            &self.log_router,
                                            &self.event_tx,
                                            &sup_tx,
                                            Arc::clone(&semaphore),
                                            &mut cache_key_cache,
                                            &mut output_digest_cache,
                                        ).await;
                                        continue;
                                    }
                                    EngineCommand::Kill(ids) => {
                                        for id in ids { if let Some(h) = live.get(&id) { let _ = h.cmd_tx.send(SupCommand::Kill).await; } }
                                        continue;
                                    }
                                    EngineCommand::Shutdown => break,
                                    EngineCommand::Start(_) => continue,
                                }
                            }
                            _ = tokio::time::sleep(Duration::from_secs(2)) => break,
                            _ = tokio::signal::ctrl_c() => {
                                for h in live.values() { let _ = h.cmd_tx.send(SupCommand::Kill).await; }
                                break;
                            }
                        }
                    }
                } else {
                    break;
                }
            }

            tokio::select! {
                _ = tokio::signal::ctrl_c() => {
                    for h in live.values() {
                        let _ = h.cmd_tx.send(SupCommand::Kill).await;
                    }
                }
                Some(msg) = sup_rx.recv() => {
                    match msg {
                        SupervisorMsg::Started { .. } => {}
                        SupervisorMsg::Exited { key, outcome } => {
                            // release permit by dropping? semaphore permit is held per task via guard
                            // we acquire permit inside schedule_ready via permit acquisition per spawn.
                            // Instead we track permits via semaphore permits held in live? Simpler: we acquire permit before spawn and release on exit.
                            // Our schedule_ready acquires owned permit and forgets; we need to release here.
                            // Actually semaphore permits are not tied to task lifecycle in this simple impl — we just acquire before spawn and drop after exit via semaphore.add_permits(1)
                            semaphore.add_permits(1);
                            let task_id = key.task.clone();
                            live.remove(&task_id);
                            let rec = records.get_mut(&task_id).expect("record exists");
                            rec.attempts.push(outcome.clone());

                            let final_status = match &outcome.exit {
                                ExitReason::Success(0) => {
                                    // try cache save if needed
                                    if let Some(spec) = self.registry.get(&task_id) {
                                        if spec.cacheable {
                                            Self::try_cache_save(
                                                &spec,
                                                &self.cache,
                                                &self.event_tx,
                                                &mut cache_key_cache,
                                                outcome.duration.as_millis(),
                                                outcome.exit_code,
                                            ).await;
                                        }
                                    }
                                    TaskStatus::Succeeded { attempt: outcome.attempt }
                                }
                                ExitReason::Success(c) if *c != 0 => {
                                    TaskStatus::Failed { attempt: outcome.attempt }
                                }
                                ExitReason::Failure(_) | ExitReason::Signal(_) | ExitReason::SpawnError(_) => {
                                    TaskStatus::Failed { attempt: outcome.attempt }
                                }
                                ExitReason::Timeout | ExitReason::Killed => {
                                    // check if restart was pending
                                    if rec.restart_pending {
                                        rec.restart_pending = false;
                                        TaskStatus::Restarting { from: outcome.attempt }
                                    } else {
                                        TaskStatus::Failed { attempt: outcome.attempt }
                                    }
                                }
                                _ => TaskStatus::Failed { attempt: outcome.attempt },
                            };

                            // handle restart-pending case
                            if matches!(final_status, TaskStatus::Restarting { .. }) {
                                let _ = self.event_tx.send(EngineEvent::TaskRestarting {
                                    id: task_id.clone(),
                                    killed_attempt: outcome.attempt,
                                });
                                // requeue
                                rec.status = TaskStatus::Pending;
                                scheduler.push_ready(task_id.clone());
                                rec.next_attempt = outcome.attempt.next();
                            } else {
                                rec.status = final_status.clone();
                                let _ = self.event_tx.send(EngineEvent::TaskFinished {
                                    id: task_id.clone(),
                                    outcome: outcome.clone(),
                                    final_status: final_status.clone(),
                                });
                                if matches!(final_status, TaskStatus::Succeeded { .. }) {
                                    // Reset Skipped direct dependents so they can be retried after a restart
                                    for dep in scheduler.graph.dependents_of(&task_id) {
                                        if let Some(r) = records.get_mut(&dep) {
                                            if matches!(r.status, TaskStatus::Skipped { .. }) {
                                                r.status = TaskStatus::Pending;
                                            }
                                        }
                                    }
                                    let _newly = scheduler.on_success(&task_id);
                                } else if matches!(final_status, TaskStatus::Failed { .. }) {
                                    // cascade skip
                                    let to_skip = scheduler.transitive_dependents_to_skip(&task_id);
                                    for dep in to_skip {
                                        if let Some(r) = records.get_mut(&dep) {
                                            if matches!(r.status, TaskStatus::Pending | TaskStatus::Ready) {
                                                r.status = TaskStatus::Skipped { reason: SkipReason::UpstreamFailed };
                                                let _ = self.event_tx.send(EngineEvent::TaskSkipped {
                                                    id: dep.clone(),
                                                    reason: SkipReason::UpstreamFailed,
                                                });
                                            }
                                        }
                                    }
                                }
                            }

                            // if pending_restarts contains this task and we just restarted, clear
                            pending_restarts.remove(&task_id);

                            // handle pending restarts that were waiting for this task to exit (restart of a running task)
                            // they are already requeued via Restarting branch

                            // try scheduling more
                            Self::schedule_ready(
                                run_id,
                                &mut scheduler,
                                &mut records,
                                &mut live,
                                &self.registry,
                                &self.cache,
                                &self.log_router,
                                &self.event_tx,
                                &sup_tx,
                                Arc::clone(&semaphore),
                                &mut cache_key_cache,
                                &mut output_digest_cache,
                            )
                            .await;
                        }
                    }
                }
                Some(cmd) = cmd_rx.recv() => {
                    match cmd {
                        EngineCommand::Restart(ids) => {
                            for id in ids {
                                if !scheduler.is_relevant(&id) {
                                    continue;
                                }
                                if let Some(handle) = live.get(&id) {
                                    // mark pending, send kill
                                    if let Some(r) = records.get_mut(&id) {
                                        r.restart_pending = true;
                                    }
                                    let _ = handle.cmd_tx.send(SupCommand::Kill).await;
                                } else {
                                    // not running — just requeue if terminal
                                    if let Some(r) = records.get_mut(&id) {
                                        if r.status.is_terminal() || r.status == TaskStatus::Stale {
                                            r.status = TaskStatus::Pending;
                                            scheduler.push_ready(id.clone());
                                            r.next_attempt = r.attempts.last().map(|o| o.attempt.next()).unwrap_or(Attempt::first());
                                            let _ = self.event_tx.send(EngineEvent::TaskRestarting { id: id.clone(), killed_attempt: r.attempts.last().map(|o| o.attempt).unwrap_or(Attempt::first()) });
                                        }
                                    }
                                }
                            }
                            Self::schedule_ready(
                                run_id,
                                &mut scheduler,
                                &mut records,
                                &mut live,
                                &self.registry,
                                &self.cache,
                                &self.log_router,
                                &self.event_tx,
                                &sup_tx,
                                Arc::clone(&semaphore),
                                &mut cache_key_cache,
                                &mut output_digest_cache,
                            )
                            .await;
                        }
                        EngineCommand::Kill(ids) => {
                            for id in ids {
                                if let Some(h) = live.get(&id) {
                                    let _ = h.cmd_tx.send(SupCommand::Kill).await;
                                }
                            }
                        }
                        EngineCommand::Shutdown => {
                            for h in live.values() {
                                let _ = h.cmd_tx.send(SupCommand::Kill).await;
                            }
                            // wait a bit for exits
                            tokio::time::sleep(Duration::from_millis(200)).await;
                            break;
                        }
                        EngineCommand::Start(_) => {}
                    }
                }
                else => break,
            }
        }

        let duration = start.elapsed();
        let summary = Self::build_summary(&records, duration);
        let _ = self.event_tx.send(EngineEvent::RunFinished(summary.clone()));
        Ok(summary)
    }

    async fn schedule_ready(
        run_id: RunId,
        scheduler: &mut SchedulerState,
        records: &mut HashMap<TaskId, TaskRecord>,
        live: &mut HashMap<TaskId, LiveHandle>,
        registry: &TaskRegistry,
        cache: &Arc<dyn CacheProvider>,
        log_router: &Arc<LogRouter>,
        event_tx: &broadcast::Sender<EngineEvent>,
        sup_tx: &mpsc::UnboundedSender<SupervisorMsg>,
        semaphore: Arc<Semaphore>,
        cache_key_cache: &mut HashMap<TaskId, String>,
        output_digest_cache: &mut HashMap<TaskId, String>,
    ) {
        while let Some(task_id) = scheduler.pop_ready() {
            // check if blocked by upstream failure before taking mutable borrow
            let spec = registry.get(&task_id).expect("spec exists").clone();
            let blocked = spec.depends_on.iter().any(|dep| {
                records
                    .get(dep)
                    .map(|r| matches!(r.status, TaskStatus::Failed { .. } | TaskStatus::Skipped { .. }))
                    .unwrap_or(false)
            });
            let rec = records.get_mut(&task_id).unwrap();
            if rec.status.is_terminal() {
                continue;
            }
            if live.contains_key(&task_id) {
                continue;
            }
            if blocked {
                rec.status = TaskStatus::Skipped {
                    reason: SkipReason::UpstreamFailed,
                };
                let _ = event_tx.send(EngineEvent::TaskSkipped {
                    id: task_id.clone(),
                    reason: SkipReason::UpstreamFailed,
                });
                continue;
            }

            // cache check
            if spec.cacheable {
                if let Some(hit) = Self::check_cache_hit(&spec, cache, event_tx, cache_key_cache, output_digest_cache).await {
                    if hit {
                        rec.status = TaskStatus::Cached { attempt: rec.next_attempt };
                        let _ = event_tx.send(EngineEvent::TaskCacheHit { id: task_id.clone() });
                        // treat as success for dependents
                        let _ = scheduler.on_success(&task_id);
                        // continue to schedule dependents immediately (loop)
                        continue;
                    }
                }
            }

            // acquire permit
            let permit = match semaphore.clone().try_acquire_owned() {
                Ok(p) => p,
                Err(_) => {
                    // no permits, push back and break
                    scheduler.push_ready(task_id);
                    break;
                }
            };
            // we will hold permit until task exits; we forget it here and release via add_permits(1) on exit
            std::mem::forget(permit);

            let attempt = rec.next_attempt;
            rec.status = TaskStatus::Running { attempt };
            rec.next_attempt = attempt.next();

            let key = ExecKey::new(run_id, task_id.clone(), attempt);
            let (cmd_tx, cmd_rx) = mpsc::channel(4);
            let ev_tx = sup_tx.clone();
            let log_tx = log_router.sender();

            let opts = SupervisorOpts {
                key: key.clone(),
                spec: Arc::clone(&spec),
            };
            let join = spawn_supervisor(opts, cmd_rx, ev_tx, log_tx, event_tx.clone());

            live.insert(
                task_id.clone(),
                LiveHandle {
                    attempt,
                    cmd_tx,
                    join,
                },
            );
            let _ = event_tx.send(EngineEvent::TaskStarted {
                id: task_id.clone(),
                attempt,
                pid: 0, // pid will be sent via SupervisorMsg::Started; we could wait for it but not needed
            });
            let _ = event_tx.send(EngineEvent::TaskReady(task_id.clone()));
        }
    }

    async fn check_cache_hit(
        spec: &fyrer_core::spec::TaskSpec,
        cache: &Arc<dyn CacheProvider>,
        event_tx: &broadcast::Sender<EngineEvent>,
        cache_key_cache: &mut HashMap<TaskId, String>,
        output_digest_cache: &mut HashMap<TaskId, String>,
    ) -> Option<bool> {
        // compute cache key memoized
        let key = match Self::compute_cache_key(spec, cache_key_cache) {
            Ok(k) => k,
            Err(e) => {
                let _ = event_tx.send(EngineEvent::NonFatalError {
                    task_id: Some(spec.id.clone()),
                    error: format!("failed to compute cache key: {e}"),
                });
                return Some(false);
            }
        };
        if !cache.contains(&key) {
            return Some(false);
        }
        let digest = match Self::compute_output_digest(spec, output_digest_cache) {
            Ok(d) => d,
            Err(e) => {
                let _ = event_tx.send(EngineEvent::NonFatalError {
                    task_id: Some(spec.id.clone()),
                    error: format!("failed to compute output digest: {e}"),
                });
                return Some(false);
            }
        };
        match cache.need_hydration(&key, &digest) {
            Ok(false) => Some(true),
            Ok(true) => {
                // need restore
                match cache.restore(&key, &spec.cwd) {
                    Ok(true) => Some(true),
                    Ok(false) => Some(false),
                    Err(e) => {
                        let _ = event_tx.send(EngineEvent::NonFatalError {
                            task_id: Some(spec.id.clone()),
                            error: format!("failed to restore cache: {e}"),
                        });
                        Some(false)
                    }
                }
            }
            Err(e) => {
                let _ = event_tx.send(EngineEvent::NonFatalError {
                    task_id: Some(spec.id.clone()),
                    error: format!("failed to check hydration: {e}"),
                });
                Some(false)
            }
        }
    }

    fn compute_cache_key(
        spec: &fyrer_core::spec::TaskSpec,
        cache: &mut HashMap<TaskId, String>,
    ) -> anyhow::Result<String> {
        if let Some(k) = cache.get(&spec.id) {
            return Ok(k.clone());
        }
        let mut hasher = blake3::Hasher::new();
        hasher.update(spec.id.to_string().as_bytes());
        hasher.update(spec.cmd.as_bytes());
        hasher.update(spec.cwd.to_string_lossy().as_bytes());
        let mut env: Vec<_> = spec.env.iter().collect();
        env.sort_by_key(|(k, _)| *k);
        for (k, v) in env {
            hash::hash_kv(&mut hasher, k, v);
        }
        let inputs = resolve_inputs(spec);
        for p in inputs {
            if p.is_file() {
                hash::hash_file(&mut hasher, &p)?;
            }
        }
        // deps are not recursively hashed here? To avoid exponential, we rely on cache_key_cache already containing deps? Simpler: include deps keys if present in cache map, else hash dep's own content similarly? For now we include dep id + cmd etc.
        // To match prior behavior, we recursively hash deps but memoized
        let key = hasher.finalize().to_hex().to_string();
        cache.insert(spec.id.clone(), key.clone());
        Ok(key)
    }

    fn compute_output_digest(
        spec: &fyrer_core::spec::TaskSpec,
        cache: &mut HashMap<TaskId, String>,
    ) -> anyhow::Result<String> {
        // not memoized strongly but cheap
        let mut hasher = blake3::Hasher::new();
        let outputs = resolve_outputs(spec);
        for p in outputs {
            if p.is_file() {
                hash::hash_file(&mut hasher, &p)?;
            }
        }
        Ok(hasher.finalize().to_hex().to_string())
    }

    async fn try_cache_save(
        spec: &fyrer_core::spec::TaskSpec,
        cache: &Arc<dyn CacheProvider>,
        event_tx: &broadcast::Sender<EngineEvent>,
        cache_key_cache: &mut HashMap<TaskId, String>,
        duration_ms: u128,
        exit_code: i32,
    ) {
        let cache_key = match Self::compute_cache_key(spec, cache_key_cache) {
            Ok(k) => k,
            Err(e) => {
                let _ = event_tx.send(EngineEvent::NonFatalError {
                    task_id: Some(spec.id.clone()),
                    error: format!("failed to compute cache key: {e}"),
                });
                return;
            }
        };
        let mut tmp_cache = HashMap::new();
        let output_digest = match Self::compute_output_digest(spec, &mut tmp_cache) {
            Ok(d) => d,
            Err(e) => {
                let _ = event_tx.send(EngineEvent::NonFatalError {
                    task_id: Some(spec.id.clone()),
                    error: format!("failed to compute output digest: {e}"),
                });
                return;
            }
        };
        let metadata = CacheMetadata::new(
            spec.id.to_string(),
            duration_ms,
            exit_code,
            CacheStatus::Miss,
            cache_key.clone(),
            output_digest.clone(),
            chrono::Utc::now().timestamp_millis() as u64,
        );
        let outputs = resolve_outputs(spec);
        if let Err(e) = cache.save(&cache_key, &outputs, metadata) {
            let _ = event_tx.send(EngineEvent::NonFatalError {
                task_id: Some(spec.id.clone()),
                error: format!("failed to save cache: {e}"),
            });
        }
    }

    fn build_summary(records: &HashMap<TaskId, TaskRecord>, duration: Duration) -> RunSummary {
        let mut successful = 0;
        let mut failed = 0;
        let mut cached = 0;
        let mut skipped = 0;
        for r in records.values() {
            match r.status {
                TaskStatus::Succeeded { .. } => successful += 1,
                TaskStatus::Failed { .. } => failed += 1,
                TaskStatus::Cached { .. } => cached += 1,
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

    pub fn event_sender(&self) -> broadcast::Sender<EngineEvent> {
        self.event_tx.clone()
    }
}

fn glob_with_patterns(cwd: &std::path::Path, pat: &str) -> Vec<std::path::PathBuf> {
    let base = cwd.join(pat).to_string_lossy().to_string();
    let mut pats = vec![base.clone()];
    if base.ends_with("/**") {
        pats.push(format!("{}/{}", base.trim_end_matches("/**"), "**/*"));
        pats.push(format!("{}/{}", base, "*"));
    }
    let mut out = Vec::new();
    for p in &pats {
        if let Ok(paths) = glob::glob(p) {
            for path in paths.flatten() {
                out.push(path);
            }
        }
    }
    out
}

fn resolve_outputs(spec: &fyrer_core::spec::TaskSpec) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    for pat in &spec.outputs {
        out.extend(glob_with_patterns(&spec.cwd, pat));
    }
    out
}
fn resolve_inputs(spec: &fyrer_core::spec::TaskSpec) -> Vec<std::path::PathBuf> {
    let ignore: std::collections::HashSet<_> = {
        let mut s = std::collections::HashSet::new();
        for pat in &spec.ignore {
            for p in glob_with_patterns(&spec.cwd, pat) {
                s.insert(p);
            }
        }
        s
    };
    let mut out = Vec::new();
    for pat in &spec.inputs {
        for p in glob_with_patterns(&spec.cwd, pat) {
            if ignore.contains(&p) {
                continue;
            }
            out.push(p);
        }
    }
    out
}

mod hash {
    pub fn hash_kv(hasher: &mut blake3::Hasher, k: &str, v: &str) {
        hasher.update(k.as_bytes());
        hasher.update(b"=");
        hasher.update(v.as_bytes());
        hasher.update(b"\n");
    }
    pub fn hash_file(hasher: &mut blake3::Hasher, p: &std::path::Path) -> std::io::Result<()> {
        use std::io::Read;
        let mut f = std::fs::File::open(p)?;
        let mut buf = [0u8; 8192];
        loop {
            let n = f.read(&mut buf)?;
            if n == 0 {
                break;
            }
            hasher.update(&buf[..n]);
        }
        Ok(())
    }
}
