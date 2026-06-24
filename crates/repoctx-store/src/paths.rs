//! Canonical paths for RepoCtx output under a repository root.

use std::path::{Path, PathBuf};

/// Resolved paths for `.repoctx/` output and the embedded index database.
#[derive(Debug, Clone)]
pub struct RepoCtxPaths {
    /// Repository root directory.
    pub root: PathBuf,
    /// `.repoctx/` output directory.
    pub output_dir: PathBuf,
    /// SQLite index database path.
    pub index_db: PathBuf,
}

impl RepoCtxPaths {
    /// Creates path helpers for the given repository root.
    ///
    /// # Arguments
    ///
    /// * `root` - Absolute or relative path to the repository root.
    pub fn new(root: impl AsRef<Path>) -> Self {
        let root = root.as_ref().to_path_buf();
        let output_dir = root.join(".repoctx");
        let index_db = output_dir.join("index.db");
        Self {
            root,
            output_dir,
            index_db,
        }
    }

    /// Returns the path for a named JSON artifact file.
    ///
    /// # Arguments
    ///
    /// * `name` - Artifact base name without extension (e.g. `symbols`).
    pub fn artifact(&self, name: &str) -> PathBuf {
        self.output_dir.join(format!("{name}.json"))
    }
}
