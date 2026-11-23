#![allow(clippy::needless_borrows_for_generic_args, clippy::len_zero)]

use std::fs;
use std::process::Command;

fn setup_repo(repo_path: &std::path::Path) {
    let run = |args: &[&str]| {
        let out = Command::new("git")
            .current_dir(repo_path)
            .args(args)
            .output()
            .unwrap();
        assert!(out.status.success(), "Git command failed: {:?}", args);
    };

    run(&["init"]);
    run(&["config", "user.name", "Test Runner"]);
    run(&["config", "user.email", "test@example.com"]);
}

#[test]
fn test_probe_clean_worktree_lifecycle() {
    let temp_dir = tempfile::tempdir().unwrap();
    let repo_path = temp_dir.path();
    setup_repo(repo_path);

    fs::create_dir_all(repo_path.join("tests")).unwrap();
    fs::create_dir_all(repo_path.join("app")).unwrap();

    let base_test = "def test_tax():\n    assert calculate_tax(100) == 10\n";
    let base_app = "def calculate_tax(amount):\n    if amount > 50:\n        return amount * 0.1\n    return 0\n";

    fs::write(repo_path.join("tests/test_tax.py"), base_test).unwrap();
    fs::write(repo_path.join("app/tax.py"), base_app).unwrap();

    let run = |args: &[&str]| {
        Command::new("git")
            .current_dir(repo_path)
            .args(args)
            .output()
            .unwrap();
    };
    run(&["add", "."]);
    run(&["commit", "-m", "Initial commit"]);

    // Head commit touches app/tax.py
    let head_app = "def calculate_tax(amount):\n    if amount > 50:\n        return amount * 0.1\n    return amount * 0.05\n";
    fs::write(repo_path.join("app/tax.py"), head_app).unwrap();
    run(&["add", "."]);
    run(&["commit", "-m", "Update tax logic"]);

    let proctor_bin = env!("CARGO_BIN_EXE_proctor");
    let output = Command::new(proctor_bin)
        .current_dir(repo_path)
        .args(&["probe", "--base", "HEAD~1", "--head", "HEAD"])
        .output()
        .unwrap();

    // Verify git worktrees list has only main worktree remaining (zero leaked temporary worktrees)
    let wt_list = Command::new("git")
        .current_dir(repo_path)
        .args(&["worktree", "list"])
        .output()
        .unwrap();
    let wt_stdout = String::from_utf8_lossy(&wt_list.stdout);
    let worktree_count = wt_stdout.lines().count();
    assert_eq!(
        worktree_count, 1,
        "Expected only 1 worktree remaining, found: {}",
        wt_stdout
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("proctor probe"));
    assert!(stdout.contains("Total Mutants"));
}
