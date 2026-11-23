use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::TempDir;

/// RAII Guard managing an ephemeral git worktree.
///
/// On Drop, cleanly detaches and unregisters the worktree so no stale
/// mutants or locks ever contaminate the parent repository.
pub struct WorktreeGuard {
    pub repo_root: PathBuf,
    pub worktree_path: PathBuf,
    _temp_dir: TempDir,
}

impl WorktreeGuard {
    pub fn new(repo_root: &Path, head_ref: &str) -> Result<Self> {
        let temp_dir = tempfile::Builder::new()
            .prefix("proctor-probe-")
            .tempdir()
            .context("Failed to create temporary directory for probe worktree")?;

        let worktree_path = temp_dir.path().to_path_buf();

        // 1. git worktree prune first to clear any dead stale registrations
        let _ = Command::new("git")
            .current_dir(repo_root)
            .args(["worktree", "prune"])
            .output();

        // 2. git worktree add --detach <path> <head_ref>
        let output = Command::new("git")
            .current_dir(repo_root)
            .args([
                "worktree",
                "add",
                "--detach",
                worktree_path.to_str().unwrap(),
                head_ref,
            ])
            .output()
            .context("Failed to run git worktree add")?;

        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("git worktree add failed: {}", err.trim());
        }

        Ok(Self {
            repo_root: repo_root.to_path_buf(),
            worktree_path,
            _temp_dir: temp_dir,
        })
    }
}

impl Drop for WorktreeGuard {
    fn drop(&mut self) {
        // Force-remove worktree from git registration
        let _ = Command::new("git")
            .current_dir(&self.repo_root)
            .args([
                "worktree",
                "remove",
                "--force",
                self.worktree_path.to_str().unwrap(),
            ])
            .output();

        let _ = Command::new("git")
            .current_dir(&self.repo_root)
            .args(["worktree", "prune"])
            .output();
    }
}
