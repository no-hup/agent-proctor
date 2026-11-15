use anyhow::{Context, Result};
use proctor_core::Policy;
use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

pub struct ChangedFiles {
    pub base_files: HashMap<String, String>,
    pub head_files: HashMap<String, String>,
}

/// Resolves the changed files between base_ref and head_ref (or working tree).
/// Uses two-dot diff (`base..head`) to avoid shallow-clone merge-base crashes in CI.
pub fn get_changed_files(base_ref: &str, head_ref: Option<&str>) -> Result<ChangedFiles> {
    // 1. Get list of changed/added/modified/deleted files
    let mut cmd = Command::new("git");
    cmd.arg("diff").arg("--name-only").arg("--no-renames");

    if let Some(head) = head_ref {
        cmd.arg(format!("{}..{}", base_ref, head));
    } else {
        cmd.arg(base_ref);
    }

    let output = cmd
        .output()
        .with_context(|| format!("Failed to run git diff against base ref '{}'", base_ref))?;

    if !output.status.success() {
        let err_msg = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("git diff failed: {}", err_msg.trim());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let paths: Vec<String> = stdout
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect();

    let mut base_files = HashMap::new();
    let mut head_files = HashMap::new();

    for path in paths {
        // Read base file content
        if let Ok(base_content) = read_git_file(base_ref, &path) {
            base_files.insert(path.clone(), base_content);
        }

        // Read head file content
        if let Some(head) = head_ref {
            if let Ok(head_content) = read_git_file(head, &path) {
                head_files.insert(path.clone(), head_content);
            }
        } else {
            // Read from working directory
            if Path::new(&path).exists() {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    head_files.insert(path.clone(), content);
                }
            }
        }
    }

    Ok(ChangedFiles {
        base_files,
        head_files,
    })
}

/// Reads a file from a specific git ref using `git show <git_ref>:<file_path>`.
pub fn read_git_file(git_ref: &str, file_path: &str) -> Result<String> {
    let output = Command::new("git")
        .arg("show")
        .arg(format!("{}:{}", git_ref, file_path))
        .output()?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        anyhow::bail!("File not present at ref {}", git_ref)
    }
}

/// Reads proctor.toml strictly from the protected base ref to prevent PR tampering.
/// If the base ref has no policy committed (e.g. initial adoption PR), falls back to Policy::default().
/// NEVER discovers from working tree / head ref to prevent agents from committing self-granting ignore rules.
pub fn load_policy_from_base(base_ref: &str) -> Policy {
    for candidate in &["proctor.toml", ".proctor.toml"] {
        if let Ok(content) = read_git_file(base_ref, candidate) {
            if let Ok(policy) = Policy::from_toml(&content) {
                return policy;
            }
        }
    }
    Policy::default()
}

/// Filters changed files against the repository's configured ignored_paths.
/// Explicitly protects proctor.toml and .proctor.toml so policy files can never ignore themselves out of inspection.
pub fn filter_ignored_paths(changed: &mut ChangedFiles, ignored_patterns: &[String]) {
    let is_ignored = |path: &str| -> bool {
        if path == "proctor.toml" || path == ".proctor.toml" {
            return false;
        }
        for pattern in ignored_patterns {
            let p = pattern.trim();
            if p.is_empty() {
                continue;
            }
            if let Some(prefix) = p.strip_suffix("/**") {
                if path == prefix || path.starts_with(&format!("{}/", prefix)) {
                    return true;
                }
            } else if let Some(prefix) = p.strip_suffix("/*") {
                if path == prefix || path.starts_with(&format!("{}/", prefix)) {
                    return true;
                }
            } else if p.ends_with('*') {
                if path.starts_with(p.trim_end_matches('*')) {
                    return true;
                }
            } else if path == p {
                return true;
            }
        }
        false
    };

    changed.base_files.retain(|path, _| !is_ignored(path));
    changed.head_files.retain(|path, _| !is_ignored(path));
}
