use crate::{
    error::{FyrerError, FyrerResult, state::StateError},
    graph::TaskGraph,
    logger::LogMessage,
    tasks::{TaskId, TaskMap},
};
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, AtomicI32, Ordering},
        Mutex, OnceLock,
    },
    time::Duration,
};
use tokio::sync::{Notify, mpsc::Sender};

const EXIT_SIGINT: i32 = 130;
const EXIT_SIGTERM: i32 = 143;

#[derive(Debug)]
pub struct GlobalState {
    pub task_graph: TaskGraph,
    pub task_map: TaskMap,
    pub global_env: HashMap<String, String>,
    pub log_sender: Sender<LogMessage>,
    running_pids: Mutex<HashMap<TaskId, u32>>,
}

static SHUTTING_DOWN: AtomicBool = AtomicBool::new(false);
static SHUTDOWN_CODE: AtomicI32 = AtomicI32::new(0);
static SHUTDOWN: OnceLock<Notify> = OnceLock::new();

pub static GLOBAL_STATE: OnceLock<GlobalState> = OnceLock::new();

pub fn init(
    task_graph: TaskGraph,
    task_map: TaskMap,
    global_env: HashMap<String, String>,
    log_sender: Sender<LogMessage>,
) -> FyrerResult<()> {
    GLOBAL_STATE
        .set(GlobalState {
            task_graph,
            task_map,
            global_env,
            log_sender,
            running_pids: Mutex::new(HashMap::new()),
        })
        .map_err(|_| FyrerError::State(StateError::AlreadyInitialized))
}

pub fn get() -> &'static GlobalState {
    GLOBAL_STATE.get().expect("Global state is not initialized")
}

pub fn register_pid(task_id: TaskId, pid: u32) {
    if let Some(state) = GLOBAL_STATE.get() {
        state.running_pids.lock().unwrap().insert(task_id, pid);
    }
}

pub fn unregister_pid(task_id: &TaskId) {
    if let Some(state) = GLOBAL_STATE.get() {
        state.running_pids.lock().unwrap().remove(task_id);
    }
}

fn running_pids() -> Vec<u32> {
    match GLOBAL_STATE.get() {
        Some(state) => state.running_pids.lock().unwrap().values().copied().collect(),
        None => Vec::new(),
    }
}

#[cfg(unix)]
fn kill_group(pid: u32, signal: libc::c_int) {
    unsafe {
        libc::kill(-(pid as libc::pid_t), signal);
    }
}

pub async fn kill_all_running() {
    let pids = running_pids();
    if pids.is_empty() {
        return;
    }
    #[cfg(unix)]
    {
        for pid in &pids {
            kill_group(*pid, libc::SIGTERM);
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
        for pid in pids {
            kill_group(pid, libc::SIGKILL);
        }
    }
}

pub fn is_shutting_down() -> bool {
    SHUTTING_DOWN.load(Ordering::SeqCst)
}

pub fn shutdown_code() -> i32 {
    SHUTDOWN_CODE.load(Ordering::SeqCst)
}

pub async fn shutdown_notified() {
    if is_shutting_down() {
        return;
    }
    SHUTDOWN.get_or_init(Notify::new).notified().await;
}

pub async fn await_shutdown_signal() {
    let code = await_shutdown_signal_code().await;
    SHUTDOWN_CODE.store(code, Ordering::SeqCst);
    SHUTTING_DOWN.store(true, Ordering::SeqCst);
    SHUTDOWN.get_or_init(Notify::new).notify_waiters();
    kill_all_running().await;
}

#[cfg(unix)]
async fn await_shutdown_signal_code() -> i32 {
    use tokio::signal::unix::{SignalKind, signal};

    let mut sigint = signal(SignalKind::interrupt()).expect("failed to install SIGINT handler");
    let mut sigterm = signal(SignalKind::terminate()).expect("failed to install SIGTERM handler");
    tokio::select! {
        _ = sigint.recv() => EXIT_SIGINT,
        _ = sigterm.recv() => EXIT_SIGTERM,
    }
}

#[cfg(not(unix))]
async fn await_shutdown_signal_code() -> i32 {
    tokio::signal::ctrl_c()
        .await
        .expect("failed to install ctrl-c handler");
    EXIT_SIGINT
}
