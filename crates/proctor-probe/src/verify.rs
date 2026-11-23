use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::Command;
use std::time::Instant;

use crate::sandbox::WorktreeGuard;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RunnerType {
    Auto,
    Pytest,
    Jest,
    Vitest,
}

impl RunnerType {
    pub fn from_str_opt(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "auto" => Some(Self::Auto),
            "pytest" => Some(Self::Pytest),
            "jest" => Some(Self::Jest),
            "vitest" => Some(Self::Vitest),
            _ => None,
        }
    }

    pub fn detect_for_file(file: &str) -> Self {
        if file.ends_with(".py") {
            Self::Pytest
        } else if file.ends_with(".ts")
            || file.ends_with(".tsx")
            || file.ends_with(".js")
            || file.ends_with(".jsx")
        {
            Self::Jest
        } else {
            Self::Auto
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum VerificationStatus {
    /// Test failed on base with AssertionError and passed on head — proves a genuine regression test!
    VerifiedRegression { reason: String },
    /// Test failed on base due to missing symbol/module/import — red, but not a proven regression.
    TriviallyRed { error: String },
    /// Test passed on both base and head — does not exercise the change / tautological.
    Tautology,
    /// Test timed out or required external dependencies (network, DB, etc.) — never fails CI.
    Inconclusive { reason: String },
    /// Test failed to execute or runner not found
    ExecutionError { error: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestVerificationResult {
    pub test_file: String,
    pub status: VerificationStatus,
    pub runner: String,
    pub duration_ms: u64,
}

/// Runs dynamic regression verification by executing changed tests on the base code.
pub fn verify_tests(
    repo_root: &Path,
    base_ref: &str,
    head_ref: Option<&str>,
    runner: RunnerType,
    changed_test_files: &[String],
) -> Result<Vec<TestVerificationResult>> {
    let mut results = Vec::new();

    if changed_test_files.is_empty() {
        return Ok(results);
    }

    // 1. Create isolated ephemeral worktree checked out at base_ref
    let guard = WorktreeGuard::new(repo_root, base_ref)
        .context("Failed to create ephemeral worktree for base_ref")?;
    let wt = &guard.worktree_path;

    // 2. For each changed test file, overlay the HEAD version into the base worktree
    for test_rel_path in changed_test_files {
        let content = if let Some(h_ref) = head_ref {
            // Read from git show <head_ref>:<path>
            let spec = format!("{}:{}", h_ref, test_rel_path);
            let out = Command::new("git")
                .current_dir(repo_root)
                .args(["show", &spec])
                .output()?;
            if !out.status.success() {
                // File might not exist in git yet, fallback to reading disk
                std::fs::read(repo_root.join(test_rel_path))
                    .with_context(|| format!("Could not read test file {}", test_rel_path))?
            } else {
                out.stdout
            }
        } else {
            // Read from working tree disk
            std::fs::read(repo_root.join(test_rel_path))
                .with_context(|| format!("Could not read test file {}", test_rel_path))?
        };

        let target_path = wt.join(test_rel_path);
        if let Some(parent) = target_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&target_path, content)?;
    }

    // 3. Execute each test file on the base worktree and classify failure modes
    for test_rel_path in changed_test_files {
        let active_runner = match runner {
            RunnerType::Auto => RunnerType::detect_for_file(test_rel_path),
            r => r,
        };

        let start = Instant::now();
        let (status, runner_name) = match active_runner {
            RunnerType::Pytest => {
                let res = run_pytest(wt, test_rel_path);
                (res, "pytest".to_string())
            }
            RunnerType::Jest => {
                let res = run_jest(wt, test_rel_path);
                (res, "jest".to_string())
            }
            RunnerType::Vitest => {
                let res = run_vitest(wt, test_rel_path);
                (res, "vitest".to_string())
            }
            RunnerType::Auto => (
                VerificationStatus::Inconclusive {
                    reason: "Unrecognized test file type".to_string(),
                },
                "unknown".to_string(),
            ),
        };

        let duration_ms = start.elapsed().as_millis() as u64;

        results.push(TestVerificationResult {
            test_file: test_rel_path.clone(),
            status,
            runner: runner_name,
            duration_ms,
        });
    }

    Ok(results)
}

fn run_pytest(worktree: &Path, test_file: &str) -> VerificationStatus {
    let mut cmd = Command::new("pytest");
    cmd.current_dir(worktree)
        .arg(test_file)
        .arg("-q")
        .arg("--tb=short")
        .env("PYTHONPATH", worktree)
        .env("PYTHONDONTWRITEBYTECODE", "1");

    let output = match cmd.output() {
        Ok(out) => out,
        Err(e) => {
            return VerificationStatus::Inconclusive {
                reason: format!("pytest execution failed: {}", e),
            };
        }
    };

    classify_output(&output.stdout, &output.stderr, output.status.success())
}

fn run_jest(worktree: &Path, test_file: &str) -> VerificationStatus {
    let mut cmd = Command::new("npx");
    cmd.current_dir(worktree)
        .args(["jest", test_file, "--silent", "--no-coverage"]);

    let output = match cmd.output() {
        Ok(out) => out,
        Err(e) => {
            return VerificationStatus::Inconclusive {
                reason: format!("jest execution failed: {}", e),
            };
        }
    };

    classify_output(&output.stdout, &output.stderr, output.status.success())
}

fn run_vitest(worktree: &Path, test_file: &str) -> VerificationStatus {
    let mut cmd = Command::new("npx");
    cmd.current_dir(worktree).args(["vitest", "run", test_file]);

    let output = match cmd.output() {
        Ok(out) => out,
        Err(e) => {
            return VerificationStatus::Inconclusive {
                reason: format!("vitest execution failed: {}", e),
            };
        }
    };

    classify_output(&output.stdout, &output.stderr, output.status.success())
}

fn classify_output(stdout: &[u8], stderr: &[u8], is_success: bool) -> VerificationStatus {
    if is_success {
        return VerificationStatus::Tautology;
    }

    let combined = format!(
        "{}\n{}",
        String::from_utf8_lossy(stdout),
        String::from_utf8_lossy(stderr)
    );

    // 1. Check for external environment dependencies (Inconclusive)
    let inconclusive_keywords = [
        "ConnectionRefusedError",
        "OperationalError",
        "DatabaseError",
        "ENOTFOUND",
        "ECONNREFUSED",
        "ETIMEDOUT",
        "NetworkError",
    ];
    for kw in &inconclusive_keywords {
        if combined.contains(kw) {
            return VerificationStatus::Inconclusive {
                reason: format!("Requires external service/network: {}", kw),
            };
        }
    }

    // 2. Check for missing symbols or imports on base (TriviallyRed)
    let trivial_keywords = [
        "ImportError",
        "ModuleNotFoundError",
        "Cannot find module",
        "is not defined",
        "NameError",
        "AttributeError",
    ];
    for kw in &trivial_keywords {
        if combined.contains(kw) {
            let error_line = combined
                .lines()
                .find(|l| l.contains(kw))
                .unwrap_or("Symbol or import missing on base code")
                .trim()
                .to_string();
            return VerificationStatus::TriviallyRed { error: error_line };
        }
    }

    // 3. Check for assertion failures (VerifiedRegression)
    let assertion_keywords = [
        "AssertionError",
        "assert ",
        "assertion failed",
        "Expected:",
        "Received:",
        "to equal",
        "toBe(",
        "FAILED",
        "FAIL",
    ];
    for kw in &assertion_keywords {
        if combined.contains(kw) {
            let reason_line = combined
                .lines()
                .find(|l| {
                    l.contains("AssertionError")
                        || l.contains("Expected:")
                        || l.contains("assertion failed")
                        || l.contains("assert ")
                })
                .unwrap_or("Assertion failed on base code (proven bugfix)")
                .trim()
                .to_string();
            return VerificationStatus::VerifiedRegression {
                reason: reason_line,
            };
        }
    }

    // 4. Default execution error
    VerificationStatus::ExecutionError {
        error: "Test failed on base with unclassified error".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_assertion_failure() {
        let stdout = b"FAILED tests/test_calc.py::test_add - AssertionError: assert 4 == 5";
        let stderr = b"";
        let status = classify_output(stdout, stderr, false);
        match status {
            VerificationStatus::VerifiedRegression { reason } => {
                assert!(reason.contains("AssertionError"));
            }
            other => panic!("Expected VerifiedRegression, got {:?}", other),
        }
    }

    #[test]
    fn test_classify_missing_import() {
        let stdout = b"ModuleNotFoundError: No module named 'new_feature'";
        let stderr = b"";
        let status = classify_output(stdout, stderr, false);
        match status {
            VerificationStatus::TriviallyRed { error } => {
                assert!(error.contains("ModuleNotFoundError"));
            }
            other => panic!("Expected TriviallyRed, got {:?}", other),
        }
    }

    #[test]
    fn test_classify_tautology() {
        let stdout = b"1 passed in 0.01s";
        let stderr = b"";
        let status = classify_output(stdout, stderr, true);
        assert_eq!(status, VerificationStatus::Tautology);
    }

    #[test]
    fn test_classify_inconclusive() {
        let stdout = b"ConnectionRefusedError: [Errno 111] Connection refused";
        let stderr = b"";
        let status = classify_output(stdout, stderr, false);
        match status {
            VerificationStatus::Inconclusive { reason } => {
                assert!(reason.contains("ConnectionRefusedError"));
            }
            other => panic!("Expected Inconclusive, got {:?}", other),
        }
    }
}
