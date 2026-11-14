use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

pub struct ChangedFiles {
    pub base_files: HashMap<String, String>,
    pub head_files: HashMap<String, String>,
}

/// Resolves the changed files between base_ref and head_ref (or working tree).
pub fn get_changed_files(base_ref: &str, head_ref: Option<&str>) -> Result<ChangedFiles> {
    // 1. Get list of changed/added/modified/deleted files
    let mut cmd = Command::new("git");
    cmd.arg("diff").arg("--name-only");

    if let Some(head) = head_ref {
        cmd.arg(format!("{}...{}", base_ref, head));
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

fn read_git_file(git_ref: &str, file_path: &str) -> Result<String> {
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
