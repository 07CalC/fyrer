use std::path::{Component, Path, PathBuf};

/// resolves a `$WORKSPACE`-prefixed path against the workspace root.
/// workspace root is where the config file is located. If the path does not start with
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
