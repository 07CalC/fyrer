//! `$WORKSPACE`-prefixed path resolution.

use std::path::{Component, Path, PathBuf};

/// Resolve `$WORKSPACE/...` paths against the workspace root (the directory
/// holding the config file). Non-prefixed paths resolve to `None`.
pub trait ResolvePath {
    fn resolve_path(&self, workspace_root: &Path) -> Option<PathBuf>;
}

impl ResolvePath for PathBuf {
    fn resolve_path(&self, workspace_root: &Path) -> Option<PathBuf> {
        let mut components = self.components();
        match components.next() {
            Some(Component::Normal(first)) if first == "$WORKSPACE" => {
                Some(workspace_root.join(components.as_path()))
            }
            _ => None,
        }
    }
}

impl ResolvePath for Path {
    fn resolve_path(&self, workspace_root: &Path) -> Option<PathBuf> {
        self.to_path_buf().resolve_path(workspace_root)
    }
}
