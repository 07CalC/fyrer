#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskStatus {
    Running,
    Success,
    Failed,
    Cached,
    Skipped,
}
