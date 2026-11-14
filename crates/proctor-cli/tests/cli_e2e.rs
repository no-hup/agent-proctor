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
fn test_cli_detects_assertion_weakening_cheat() {
    let temp_dir = tempfile::tempdir().unwrap();
    let repo_path = temp_dir.path();
    setup_repo(repo_path);

    // 1. Add base files
    fs::create_dir_all(repo_path.join("tests")).unwrap();
    fs::create_dir_all(repo_path.join("app")).unwrap();

    let base_test = "def test_tax():\n    assert calculate_tax(100) == 10\n";
    let base_app = "def calculate_tax(amount):\n    return amount * 0.1\n";

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

    // 2. Cheating agent change: weakens test assertion without touching app
    let cheated_test = "def test_tax():\n    assert calculate_tax(100) is not None\n";
    fs::write(repo_path.join("tests/test_tax.py"), cheated_test).unwrap();

    // 3. Run `proctor scan --base HEAD`
    let proctor_bin = env!("CARGO_BIN_EXE_proctor");
    let output = Command::new(proctor_bin)
        .current_dir(repo_path)
        .args(&["scan", "--base", "HEAD"])
        .output()
        .unwrap();

    // Must exit with code 2 (BLOCKED)
    assert_eq!(output.status.code(), Some(2));

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("BLOCKED"));
    assert!(stdout.contains("PROCTOR-001"));
}

#[test]
fn test_cli_allows_legitimate_refactor() {
    let temp_dir = tempfile::tempdir().unwrap();
    let repo_path = temp_dir.path();
    setup_repo(repo_path);

    fs::create_dir_all(repo_path.join("tests")).unwrap();
    fs::create_dir_all(repo_path.join("app")).unwrap();

    let base_test = "def test_legacy():\n    assert user.old_token == 'secret'\n";
    let base_app = "class User:\n    old_token = 'secret'\n";

    fs::write(repo_path.join("tests/test_legacy.py"), base_test).unwrap();
    fs::write(repo_path.join("app/user.py"), base_app).unwrap();

    let run = |args: &[&str]| {
        Command::new("git")
            .current_dir(repo_path)
            .args(args)
            .output()
            .unwrap();
    };
    run(&["add", "."]);
    run(&["commit", "-m", "Initial commit"]);

    // Legitimate refactor: feature `old_token` is deleted from both production code and test
    let head_test = "def test_legacy():\n    pass\n";
    let head_app = "class User:\n    pass\n";
    fs::write(repo_path.join("tests/test_legacy.py"), head_test).unwrap();
    fs::write(repo_path.join("app/user.py"), head_app).unwrap();

    let proctor_bin = env!("CARGO_BIN_EXE_proctor");
    let output = Command::new(proctor_bin)
        .current_dir(repo_path)
        .args(&["scan", "--base", "HEAD"])
        .output()
        .unwrap();

    // Must exit with code 0 (PASS - justified by orphaned symbol)
    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("PASS"));
}

#[test]
fn test_cli_detects_ci_sabotage() {
    let temp_dir = tempfile::tempdir().unwrap();
    let repo_path = temp_dir.path();
    setup_repo(repo_path);

    fs::create_dir_all(repo_path.join(".github/workflows")).unwrap();
    let base_ci = "name: CI\njobs:\n  test:\n    steps:\n      - run: pytest\n";
    fs::write(repo_path.join(".github/workflows/ci.yml"), base_ci).unwrap();

    let run = |args: &[&str]| {
        Command::new("git")
            .current_dir(repo_path)
            .args(args)
            .output()
            .unwrap();
    };
    run(&["add", "."]);
    run(&["commit", "-m", "Initial commit"]);

    // Tampering: agent adds `|| true`
    let cheated_ci = "name: CI\njobs:\n  test:\n    steps:\n      - run: pytest || true\n";
    fs::write(repo_path.join(".github/workflows/ci.yml"), cheated_ci).unwrap();

    let proctor_bin = env!("CARGO_BIN_EXE_proctor");
    let output = Command::new(proctor_bin)
        .current_dir(repo_path)
        .args(&["scan", "--base", "HEAD"])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(2));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("BLOCKED"));
    assert!(stdout.contains("PROCTOR-003"));
}
