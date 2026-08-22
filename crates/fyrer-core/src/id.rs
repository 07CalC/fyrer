use std::{fmt::Display, sync::Arc};

/// Task identifier `package:task` — cheap to clone via Arcs.
#[derive(Debug, PartialEq, Eq, Hash)]
pub struct TaskId {
    package: Arc<String>,
    task: Arc<String>,
}

impl TaskId {
    pub fn new(package: &str, task: &str) -> Self {
        Self {
            package: Arc::new(package.to_string()),
            task: Arc::new(task.to_string()),
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        let mut parts = s.splitn(2, ':');
        let p = parts.next()?;
        let t = parts.next()?;
        if p.is_empty() || t.is_empty() || t.contains(':') {
            return None;
        }
        Some(Self::new(p, t))
    }

    pub fn package(&self) -> &str {
        &self.package
    }
    pub fn task(&self) -> &str {
        &self.task
    }
}

impl Clone for TaskId {
    fn clone(&self) -> Self {
        Self {
            package: Arc::clone(&self.package),
            task: Arc::clone(&self.task),
        }
    }
}

impl From<&str> for TaskId {
    fn from(s: &str) -> Self {
        Self::from_str(s).expect("Invalid TaskId format, expected package:task")
    }
}

impl Display for TaskId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.package, self.task)
    }
}

impl serde::Serialize for TaskId {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}
impl<'de> serde::Deserialize<'de> for TaskId {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        Self::from_str(&s).ok_or_else(|| serde::de::Error::custom("expected package:task"))
    }
}

// ---------------------------------------------------------------------------
// Run / Attempt / ExecKey
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RunId(pub u64);

impl RunId {
    pub fn new(v: u64) -> Self {
        Self(v)
    }
}
impl Display for RunId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "run-{}", self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Attempt(pub u32);

impl Attempt {
    pub fn first() -> Self {
        Self(1)
    }
    pub fn next(self) -> Self {
        Self(self.0 + 1)
    }
}
impl Display for Attempt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ExecKey {
    pub run: RunId,
    pub task: TaskId,
    pub attempt: Attempt,
}

impl ExecKey {
    pub fn new(run: RunId, task: TaskId, attempt: Attempt) -> Self {
        Self { run, task, attempt }
    }
}
impl Display for ExecKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}#{}", self.task, self.run.0, self.attempt.0)
    }
}
