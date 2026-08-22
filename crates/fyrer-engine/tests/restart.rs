use std::{collections::HashMap, sync::Arc};
use fyrer_core::{TaskId, spec::{TaskRegistry, TaskSpec}, graph::TaskGraph};
use fyrer_cache::local::LocalCacheProvider;
use fyrer_log::LogRouter;
use fyrer_engine::{Engine, events::{RunPlan, EngineEvent}};
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
    let mut rx = event_tx.subscribe();

    let engine = Engine::new(registry.clone(), graph.clone(), cache.clone(), log_router.clone(), event_tx.clone(), Some(4));
    let plan = RunPlan::new(vec![a_id.clone(), b_id.clone(), c_id.clone()]);

    // Spawn engine with handle for restart
    let (cmd_tx, cmd_rx) = tokio::sync::mpsc::channel(16);
    let engine_clone = engine.clone();
    let handle = tokio::spawn(async move {
        engine_clone.run_with_receiver(plan, cmd_rx).await
    });

    // Collect events in background (drain)
    let _collector = tokio::spawn(async move {
        let mut evs = Vec::new();
        while let Ok(ev) = rx.recv().await {
            match &ev {
                EngineEvent::RunFinished(_) => {
                    evs.push(ev.clone());
                    break;
                }
                _ => evs.push(ev.clone()),
            }
        }
        evs
    });

    // Wait a bit for first run to complete (a fails, b skipped, c succeeds)
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    // Now create flag so a will succeed on restart
    std::fs::write(&flag, "1").unwrap();

    // Send restart for a
    cmd_tx.send(fyrer_engine::events::EngineCommand::Restart(vec![a_id.clone()])).await.unwrap();

    // Wait for engine to finish (needs to handle restart)
    // The engine's run_with_receiver will not finish until all tasks terminal and no ready.
    // After restart, a will succeed and b should be retried.
    // We need to wait for handle
    let summary = tokio::time::timeout(std::time::Duration::from_secs(10), handle).await.unwrap().unwrap().unwrap();
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
