use std::path::{Path, PathBuf};

use crate::config::Task;

impl Task {
    pub fn get_env_path(&self) -> Option<PathBuf> {
        match (&self.dir, &self.env_path) {
            (Some(dir), Some(path)) => Some(Path::new(&dir).join(path)),
            (Some(dir), None) => Some(Path::new(&dir).join(".env")),
            (None, Some(path)) => Some(Path::new(&path).to_path_buf()),
            (None, None) => None,
        }
    }
}
