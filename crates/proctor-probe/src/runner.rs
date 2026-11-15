use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProbeOutcome {
    /// Test failed when mutant was introduced — test successfully asserts on production logic!
    Killed,
    /// Test passed despite mutant — test failed to catch the bug (weak/placebo).
    Survived,
    /// Mutant was never reached/executed during the test run.
    Inconclusive,
}

/// Executes a targeted test run against the ephemeral worktree with sentinel tracking.
pub fn execute_probe_run(
    worktree_path: &Path,
    target_test_file: &str,
    sentinel_path: &Path,
    is_python: bool,
) -> Result<ProbeOutcome> {
    // 1. Ensure sentinel does not exist before run
    if sentinel_path.exists() {
        let _ = std::fs::remove_file(sentinel_path);
    }

    // 2. Build test command
    let mut cmd = if is_python {
        let mut c = Command::new("pytest");
        c.current_dir(worktree_path)
            .arg(target_test_file)
            .arg("-q")
            .arg("--tb=no")
            .env("PYTHONPATH", worktree_path)
            .env("PYTHONDONTWRITEBYTECODE", "1")
            .env("PROCTOR_SENTINEL", sentinel_path);
        c
    } else {
        let mut c = Command::new("npx");
        c.current_dir(worktree_path)
            .arg("jest")
            .arg(target_test_file)
            .arg("--silent")
            .env("PROCTOR_SENTINEL", sentinel_path);
        c
    };

    // 3. Run with execution check
    let output = match cmd.output() {
        Ok(out) => out,
        Err(_) => {
            // Test runner not installed or failed to execute
            return Ok(ProbeOutcome::Inconclusive);
        }
    };

    // 4. Verify sentinel was touched
    let sentinel_touched = sentinel_path.exists();
    if !sentinel_touched {
        return Ok(ProbeOutcome::Inconclusive);
    }

    // Clean up sentinel
    let _ = std::fs::remove_file(sentinel_path);

    // 5. Check exit code:
    // Non-zero exit code means test failed -> mutant was KILLED
    // Zero exit code means test passed -> mutant SURVIVED
    if !output.status.success() {
        Ok(ProbeOutcome::Killed)
    } else {
        Ok(ProbeOutcome::Survived)
    }
}
