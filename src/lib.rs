//! `fyrer` is a declarative, fast, and lightweight monorepo tool.
//!
//! It reads a [`FyrerConfig`] describing projects and their tasks, resolves
//! the dependency graph between tasks, runs each level of the graph
//! concurrently, and streams every task's output through a colorized,
//! prefixed logger. Long-running tasks can be restarted when their watched
//! input files change.
//!
//! # Example
//!
//! ```
//! use fyrer::tasks::TaskId;
//!
//! let id = TaskId::new("web", "build");
//! assert_eq!(id.to_string(), "web:build");
//! ```

pub mod config;
pub mod env;
pub mod error;
pub mod executor;
pub mod global;
pub mod graph;
pub mod logger;
pub mod runner;
pub mod tasks;
pub mod watcher;

pub use config::FyrerConfig;
pub use error::{FyrerError, FyrerResult};
pub use tasks::{Task, TaskId};
