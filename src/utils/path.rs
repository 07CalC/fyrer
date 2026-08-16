use std::path::PathBuf;

pub trait ResolvePath {
    fn resolve_path(&self) -> PathBuf;
}

impl ResolvePath for PathBuf {
    fn resolve_path(&self) -> PathBuf {
        let path = self.to_string_lossy();
        let cwd_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        match path.strip_prefix("$WORKSPACE") {
            Some(stripped) => cwd_dir.join(stripped.trim_start_matches('/')),
            None => self.clone(),
        }
    }
}
