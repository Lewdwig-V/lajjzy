use anyhow::{Context, Result, bail};
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::backend::RepoBackend;
use crate::types::GraphData;

/// Backend that shells out to the `jj` CLI.
pub struct JjCliBackend {
    /// Absolute path to the workspace root (from `jj root`).
    workspace_root: PathBuf,
}

impl JjCliBackend {
    /// Create a new backend for the given directory.
    /// Validates that `jj` is installed and the path is inside a jj workspace.
    pub fn new(path: &Path) -> Result<Self> {
        let output = Command::new("jj")
            .arg("root")
            .current_dir(path)
            .output()
            .context("Failed to run `jj root`. Is jj installed?")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            bail!("Not a jj workspace: {}", stderr.trim());
        }

        let root = String::from_utf8(output.stdout)
            .context("jj root output was not valid UTF-8")?
            .trim()
            .to_string();

        Ok(Self {
            workspace_root: PathBuf::from(root),
        })
    }

    /// Returns the workspace root path.
    pub fn workspace_root(&self) -> &Path {
        &self.workspace_root
    }
}

impl RepoBackend for JjCliBackend {
    fn load_graph(&self) -> Result<GraphData> {
        todo!("Implemented in Task 5")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    fn jj_available() -> bool {
        Command::new("jj").arg("--version").output().is_ok()
    }

    #[test]
    fn new_fails_on_non_workspace() {
        if !jj_available() {
            eprintln!("Skipping: jj not in PATH");
            return;
        }
        let tmp = tempfile::tempdir().unwrap();
        let result = JjCliBackend::new(tmp.path());
        assert!(result.is_err());
    }

    #[test]
    fn new_succeeds_on_jj_workspace() {
        if !jj_available() {
            eprintln!("Skipping: jj not in PATH");
            return;
        }
        let tmp = tempfile::tempdir().unwrap();
        let status = Command::new("jj")
            .args(["git", "init"])
            .current_dir(tmp.path())
            .status()
            .unwrap();
        assert!(status.success());

        let backend = JjCliBackend::new(tmp.path()).unwrap();
        assert_eq!(backend.workspace_root(), tmp.path().canonicalize().unwrap());
    }
}
