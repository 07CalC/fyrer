use std::{
    collections::HashMap,
    sync::{
        Mutex, MutexGuard, OnceLock, PoisonError,
        atomic::{AtomicBool, AtomicI32, Ordering},
    },
    time::Duration,
};

use tokio::sync::{Notify, mpsc::Sender};

use crate::{
    error::{FyrerError, FyrerResult, StateError},
    graph::TaskGraph,
    logger::LogMessage,
    tasks::{TaskId, TaskMap},
};

/// Exit code reported after a `SIGINT` shutdown.
const EXIT_SIGINT: i32 = 130;

/// Exit code reported after a `SIGTERM` shutdown.
const EXIT_SIGTERM: i32 = 143;

#[derive(Debug)]
pub struct GlobalState {
    /// The resolved task graph.
    pub task_graph: TaskGraph,
    /// The resolved task map.
    pub task_map: TaskMap,
    /// Channel used to stream log messages to the logger task.
    pub log_sender: Sender<LogMessage>,
    running_pids: Mutex<HashMap<TaskId, u32>>,
}

/// Set once a shutdown signal has been received.
static SHUTTING_DOWN: AtomicBool = AtomicBool::new(false);

/// The exit code to use once shutdown completes.
static SHUTDOWN_CODE: AtomicI32 = AtomicI32::new(0);

/// Notified when a shutdown signal has been received.
static SHUTDOWN: OnceLock<Notify> = OnceLock::new();

static GLOBAL_STATE: OnceLock<GlobalState> = OnceLock::new();

pub fn init(
    task_graph: TaskGraph,
    task_map: TaskMap,
    log_sender: Sender<LogMessage>,
) -> FyrerResult<()> {
    GLOBAL_STATE
        .set(GlobalState {
            task_graph,
            task_map,
            log_sender,
            running_pids: Mutex::new(HashMap::new()),
        })
        .map_err(|_| FyrerError::State(StateError::AlreadyInitialized))
}

#[must_use]
pub fn get() -> &'static GlobalState {
    GLOBAL_STATE
        .get()
        .expect("global state must be initialized before use")
}

pub fn register_pid(task_id: TaskId, pid: u32) {
    if let Some(state) = GLOBAL_STATE.get() {
        pids(state).insert(task_id, pid);
    }
}

pub fn unregister_pid(task_id: &TaskId) {
    if let Some(state) = GLOBAL_STATE.get() {
        pids(state).remove(task_id);
    }
}

fn pids(state: &GlobalState) -> MutexGuard<'_, HashMap<TaskId, u32>> {
    state
        .running_pids
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
}

fn running_pids() -> Vec<u32> {
    GLOBAL_STATE
        .get()
        .map(|state| pids(state).values().copied().collect())
        .unwrap_or_default()
}

/// Sends `SIGTERM` and then `SIGKILL` to every running task.
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

#[cfg(unix)]
fn kill_group(pid: u32, signal: libc::c_int) {
    unsafe {
        libc::kill(-pid.cast_signed(), signal);
    }
}

#[must_use]
pub fn is_shutting_down() -> bool {
    SHUTTING_DOWN.load(Ordering::SeqCst)
}

#[must_use]
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

/// Waits for Ctrl-C and returns the matching exit code.
#[cfg(not(unix))]
async fn await_shutdown_signal_code() -> i32 {
    tokio::signal::ctrl_c()
        .await
        .expect("failed to install ctrl-c handler");
    EXIT_SIGINT
}
