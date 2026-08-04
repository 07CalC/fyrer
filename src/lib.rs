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

pub mod app;
pub mod cli;
pub mod config;
pub mod env;
pub mod error;
pub mod events;
pub mod global;
pub mod graph;
pub mod logger;
pub mod scheduler;
pub mod tasks;
pub mod tui;

pub use config::FyrerConfig;
pub use error::{FyrerError, FyrerResult};
pub use tasks::{Task, TaskId};
