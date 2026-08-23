use std::{collections::HashMap, sync::Arc};

use fyrer_cache::local::LocalCacheProvider;
use fyrer_core::{
    TaskId,
    graph::TaskGraph,
    spec::{TaskRegistry, TaskSpec},
};
use fyrer_engine::{
    Engine,
    events::{EngineCommand, EngineEvent, RunPlan},
};
use fyrer_log::LogRouter;
use tokio::sync::broadcast;

#[tokio::test]
async fn test_dynamic_scheduling_and_restart() {
    // Create a simple DAG: a -> b, c independent
    // a will fail first, we restart a and expect b to run after a succeeds
    let tmp = tempfile::tempdir().unwrap();
    let cwd_a = tmp.path().join("a");
    let cwd_b = tmp.path().join("b");
    let cwd_c = tmp.path().join("c");
    std::fs::create_dir_all(&cwd_a).unwrap();
    std::fs::create_dir_all(&cwd_b).unwrap();
    std::fs::create_dir_all(&cwd_c).unwrap();

    // Create a flag file that controls a's success
    let flag = tmp.path().join("flag");
    // a's command: check flag, fail if not exists, succeed if exists
    let a_cmd = format!("if [ -f {} ]; then echo 'a success'; exit 0; else echo 'a fail'; exit 1; fi", flag.display());
    let b_cmd = "echo 'b success'".to_string();
    let c_cmd = "echo 'c success'".to_string();

    let a_id = TaskId::new("pkg", "a");
    let b_id = TaskId::new("pkg", "b");
    let c_id = TaskId::new("pkg", "c");

    let a_spec = Arc::new(TaskSpec::new(
        a_id.clone(),
        HashMap::new(),
        false, false, false, None,
        cwd_a.clone(),
        a_cmd,
        vec![],
        vec![],
        vec![],
        vec![],
    ));
    let b_spec = Arc::new(TaskSpec::new(
        b_id.clone(),
        HashMap::new(),
        false, false, false, None,
        cwd_b.clone(),
        b_cmd,
        vec![a_id.clone()],
        vec![],
        vec![],
        vec![],
    ));
    let c_spec = Arc::new(TaskSpec::new(
        c_id.clone(),
        HashMap::new(),
        false, false, false, None,
        cwd_c.clone(),
        c_cmd,
        vec![],
        vec![],
        vec![],
        vec![],
    ));

    let mut map = HashMap::new();
    map.insert(a_id.clone(), a_spec);
    map.insert(b_id.clone(), b_spec);
    map.insert(c_id.clone(), c_spec);
    let registry = TaskRegistry::new(map);
    let graph = TaskGraph::from_specs(
        registry.iter().map(|(id, s)| (id.clone(), s.depends_on.clone()))
    ).unwrap();
    graph.validate().unwrap();

    let cache = Arc::new(LocalCacheProvider::new(tmp.path().join(".fyrer/cache").to_string_lossy().to_string()));
    let log_router = Arc::new(LogRouter::new(100, None));
    let (event_tx, _) = broadcast::channel(1024);

    let engine = Engine::new(registry.clone(), graph.clone(), cache.clone(), log_router.clone(), event_tx.clone(), Some(4));
    let plan = RunPlan::new(vec![a_id.clone(), b_id.clone(), c_id.clone()]);

    // Spawn engine with handle for restart
    let (cmd_tx, cmd_rx) = tokio::sync::mpsc::channel(16);
    let engine_clone = engine.clone();
    let handle = tokio::spawn(async move {
        engine_clone.run_with_receiver(plan, cmd_rx).await
    });

    // Watch events until `b` finishes successfully (only possible after restart)
    let mut watcher_rx = event_tx.subscribe();
    let b_for_watch = b_id.clone();
    let cmd_tx_for_shutdown = cmd_tx.clone();
    let collector = tokio::spawn(async move {
        loop {
            match watcher_rx.recv().await {
                Ok(EngineEvent::TaskFinished { id, outcome, .. }) => {
                    if id == b_for_watch && outcome.is_success() {
                        // Chain repaired — tell the engine to stop.
                        let _ = cmd_tx_for_shutdown
                            .send(EngineCommand::Shutdown)
                            .await;
                        break;
                    }
                }
                Err(broadcast::error::RecvError::Closed) => break,
                _ => {}
            }
        }
    });

    // Wait a bit for first pass to complete (a fails, b skipped, c succeeds)
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    // Create the flag so `a` will succeed on restart
    std::fs::write(&flag, "1").unwrap();

    // Send restart for a — engine is parked waiting for commands post-run
    cmd_tx.send(EngineCommand::Restart(vec![a_id.clone()])).await.unwrap();

    // Engine exits after Shutdown triggered by the collector
    let summary = tokio::time::timeout(std::time::Duration::from_secs(10), handle).await.unwrap().unwrap().unwrap();
    let _ = collector.await;
    println!("summary: {:?}", summary);
    assert_eq!(summary.failed, 0, "after restart, no failures");
    assert_eq!(summary.successful, 3, "all three should succeed after restart");
}

#[tokio::test]
async fn test_concurrent_independent_tasks() {
    let tmp = tempfile::tempdir().unwrap();
    let cwd1 = tmp.path().join("p1");
    let cwd2 = tmp.path().join("p2");
    std::fs::create_dir_all(&cwd1).unwrap();
    std::fs::create_dir_all(&cwd2).unwrap();

    let id1 = TaskId::new("pkg", "t1");
    let id2 = TaskId::new("pkg", "t2");
    let spec1 = Arc::new(TaskSpec::new(id1.clone(), HashMap::new(), false, false, false, None, cwd1, "sleep 0.2 && echo t1".to_string(), vec![], vec![], vec![], vec![]));
    let spec2 = Arc::new(TaskSpec::new(id2.clone(), HashMap::new(), false, false, false, None, cwd2, "sleep 0.2 && echo t2".to_string(), vec![], vec![], vec![], vec![]));
    let mut map = HashMap::new();
    map.insert(id1.clone(), spec1);
    map.insert(id2.clone(), spec2);
    let registry = TaskRegistry::new(map);
    let graph = TaskGraph::from_specs(registry.iter().map(|(id, s)| (id.clone(), s.depends_on.clone()))).unwrap();
    let cache = Arc::new(LocalCacheProvider::new(tmp.path().join(".fyrer/cache").to_string_lossy().to_string()));
    let log_router = Arc::new(LogRouter::new(100, None));
    let (event_tx, _) = broadcast::channel(1024);
    let engine = Engine::new(registry, graph, cache, log_router, event_tx, Some(4));
    let plan = RunPlan::new(vec![id1, id2]);
    let start = std::time::Instant::now();
    let summary = engine.run_once(plan).await.unwrap();
    let elapsed = start.elapsed();
    println!("elapsed: {:?}", elapsed);
    assert!(elapsed < std::time::Duration::from_millis(350), "tasks should run in parallel, elapsed {:?}", elapsed);
    assert_eq!(summary.successful, 2);
}

#[tokio::test]
async fn test_duration_stops_at_completion_not_shutdown() {
    // Two fast tasks; interactive engine parks after completion.
    // Simulated user browsing must NOT inflate summary.duration.
    let tmp = tempfile::tempdir().unwrap();
    let cwd1 = tmp.path().join("p1");
    let cwd2 = tmp.path().join("p2");
    std::fs::create_dir_all(&cwd1).unwrap();
    std::fs::create_dir_all(&cwd2).unwrap();

    let id1 = TaskId::new("pkg", "t1");
    let id2 = TaskId::new("pkg", "t2");
    let spec1 = Arc::new(TaskSpec::new(id1.clone(), HashMap::new(), false, false, false, None, cwd1, "sleep 0.15 && echo t1".to_string(), vec![], vec![], vec![], vec![]));
    let spec2 = Arc::new(TaskSpec::new(id2.clone(), HashMap::new(), false, false, false, None, cwd2, "sleep 0.15 && echo t2".to_string(), vec![], vec![], vec![], vec![]));
    let mut map = HashMap::new();
    map.insert(id1, spec1);
    map.insert(id2, spec2);
    let registry = TaskRegistry::new(map);
    let graph = TaskGraph::from_specs(registry.iter().map(|(id, s)| (id.clone(), s.depends_on.clone()))).unwrap();
    let cache = Arc::new(LocalCacheProvider::new(tmp.path().join(".fyrer/cache").to_string_lossy().to_string()));
    let log_router = Arc::new(LogRouter::new(100, None));
    let (event_tx, _) = broadcast::channel(1024);

    let engine = Engine::new(registry, graph, cache, log_router, event_tx.clone(), Some(4));
    let (cmd_tx, cmd_rx) = tokio::sync::mpsc::channel(16);
    let handle_join = tokio::spawn(async move {
        engine.run_with_receiver(RunPlan::new(vec![TaskId::new("pkg", "t1"), TaskId::new("pkg", "t2")]), cmd_rx).await
    });

    // Drain events until every task has reached a terminal state.
    // NOTE: RunFinished is only emitted after the engine loop exits, so we
    // can't wait for it here — the engine parks post-completion in
    // interactive mode.
    let mut rx = event_tx.subscribe();
    let mut terminal = 0;
    loop {
        match rx.recv().await {
            Ok(EngineEvent::TaskFinished { .. })
            | Ok(EngineEvent::TaskSkipped { .. })
            | Ok(EngineEvent::TaskCacheHit { .. }) => {
                terminal += 1;
                if terminal >= 2 {
                    break;
                }
            }
            Err(broadcast::error::RecvError::Closed) => break,
            _ => {}
        }
    }

    // Simulate user browsing the TUI for 800ms after completion
    tokio::time::sleep(std::time::Duration::from_millis(800)).await;
    let _ = cmd_tx.send(EngineCommand::Shutdown).await;
    let summary = tokio::time::timeout(std::time::Duration::from_secs(5), handle_join).await.unwrap().unwrap().unwrap();

    println!("summary: {:?}", summary);
    // Tasks take ~150-300ms; if the bug were present the duration would be >= 800ms
    assert!(
        summary.duration < std::time::Duration::from_millis(600),
        "duration should stop at task completion, not shutdown: {:?}",
        summary.duration
    );
}
