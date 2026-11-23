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
fn test_regression_value_swap_same_count_blocks() {
    let temp_dir = tempfile::tempdir().unwrap();
    let repo_path = temp_dir.path();
    setup_repo(repo_path);

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

    // Cheat: agent changes expected value from 10 to 0 (equal assertion count)
    let cheated_test = "def test_tax():\n    assert calculate_tax(100) == 0\n";
    fs::write(repo_path.join("tests/test_tax.py"), cheated_test).unwrap();

    let proctor_bin = env!("CARGO_BIN_EXE_proctor");
    let output = Command::new(proctor_bin)
        .current_dir(repo_path)
        .args(&["scan", "--base", "HEAD"])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(2));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("BLOCKED"));
    assert!(stdout.contains("PROCTOR-001"));
}

#[test]
fn test_regression_or_true_vacuous_assertion_blocks() {
    let temp_dir = tempfile::tempdir().unwrap();
    let repo_path = temp_dir.path();
    setup_repo(repo_path);

    fs::create_dir_all(repo_path.join("tests")).unwrap();
    let base_test = "def test_tax():\n    assert calculate_tax(100) == 10\n";
    fs::write(repo_path.join("tests/test_tax.py"), base_test).unwrap();

    let run = |args: &[&str]| {
        Command::new("git")
            .current_dir(repo_path)
            .args(args)
            .output()
            .unwrap();
    };
    run(&["add", "."]);
    run(&["commit", "-m", "Initial commit"]);

    // Cheat: agent tacks on `or True` to make assertion trivially pass
    let cheated_test = "def test_tax():\n    assert calculate_tax(100) == 10 or True\n";
    fs::write(repo_path.join("tests/test_tax.py"), cheated_test).unwrap();

    let proctor_bin = env!("CARGO_BIN_EXE_proctor");
    let output = Command::new(proctor_bin)
        .current_dir(repo_path)
        .args(&["scan", "--base", "HEAD"])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(2));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("BLOCKED"));
}

#[test]
fn test_regression_pytest_skip_marker_blocks() {
    let temp_dir = tempfile::tempdir().unwrap();
    let repo_path = temp_dir.path();
    setup_repo(repo_path);

    // Setup repository with production code and active test
    fs::create_dir_all(repo_path.join("app")).unwrap();
    fs::create_dir_all(repo_path.join("tests")).unwrap();
    let base_src = "def calculate_tax(amount):\n    return amount * 0.10\n";
    fs::write(repo_path.join("app/tax.py"), base_src).unwrap();
    let base_test = "def test_tax():\n    assert calculate_tax(100) == 10\n";
    fs::write(repo_path.join("tests/test_tax.py"), base_test).unwrap();

    let run = |args: &[&str]| {
        Command::new("git")
            .current_dir(repo_path)
            .args(args)
            .output()
            .unwrap();
    };
    run(&["add", "."]);
    run(&["commit", "-m", "Initial commit"]);

    // Cheat: agent modifies production code and decorates with @pytest.mark.skip to silence failing test
    let cheated_src = "def calculate_tax(amount):\n    return amount * 0.20\n";
    fs::write(repo_path.join("app/tax.py"), cheated_src).unwrap();
    let cheated_test = "@pytest.mark.skip(reason=\"failing\")\ndef test_tax():\n    assert calculate_tax(100) == 10\n";
    fs::write(repo_path.join("tests/test_tax.py"), cheated_test).unwrap();

    let proctor_bin = env!("CARGO_BIN_EXE_proctor");
    let output = Command::new(proctor_bin)
        .current_dir(repo_path)
        .args(&["scan", "--base", "HEAD"])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(2));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("BLOCKED"));
}

#[test]
fn test_regression_skip_quarantine_routes_to_review() {
    let temp_dir = tempfile::tempdir().unwrap();
    let repo_path = temp_dir.path();
    setup_repo(repo_path);

    fs::create_dir_all(repo_path.join("tests")).unwrap();
    let base_test = "def test_flaky():\n    assert ping() == True\n";
    fs::write(repo_path.join("tests/test_flaky.py"), base_test).unwrap();

    let run = |args: &[&str]| {
        Command::new("git")
            .current_dir(repo_path)
            .args(args)
            .output()
            .unwrap();
    };
    run(&["add", "."]);
    run(&["commit", "-m", "Initial commit"]);

    // Legitimate quarantine: human or agent skips flaky test without touching production
    let quarantined_test =
        "@pytest.mark.skip(reason=\"flaky\")\ndef test_flaky():\n    assert ping() == True\n";
    fs::write(repo_path.join("tests/test_flaky.py"), quarantined_test).unwrap();

    // Under --strict mode, routes to REVIEW (exit 1), not BLOCK (exit 2)
    let proctor_bin = env!("CARGO_BIN_EXE_proctor");
    let output_strict = Command::new(proctor_bin)
        .current_dir(repo_path)
        .args(&["scan", "--base", "HEAD", "--strict"])
        .output()
        .unwrap();

    assert_eq!(output_strict.status.code(), Some(1));
    let stdout_strict = String::from_utf8_lossy(&output_strict.stdout);
    assert!(stdout_strict.contains("REVIEW REQUIRED"));

    // Under default advisory mode, report tier exits 0 with attributed findings
    let output_default = Command::new(proctor_bin)
        .current_dir(repo_path)
        .args(&["scan", "--base", "HEAD"])
        .output()
        .unwrap();

    assert_eq!(output_default.status.code(), Some(0));
    let stdout_default = String::from_utf8_lossy(&output_default.stdout);
    assert!(stdout_default.contains("REVIEW REQUIRED"));
}

#[test]
fn test_regression_jest_skip_blocks() {
    let temp_dir = tempfile::tempdir().unwrap();
    let repo_path = temp_dir.path();
    setup_repo(repo_path);

    fs::create_dir_all(repo_path.join("src")).unwrap();
    fs::create_dir_all(repo_path.join("tests")).unwrap();
    let base_src = "function charge() { return 100; }\nmodule.exports = { charge };\n";
    fs::write(repo_path.join("src/billing.js"), base_src).unwrap();
    let base_test = "const { charge } = require('../src/billing');\ndescribe('Billing', () => {\n  it('charges card', () => {\n    expect(charge()).toBe(100);\n  });\n});\n";
    fs::write(repo_path.join("tests/billing.test.js"), base_test).unwrap();

    let run = |args: &[&str]| {
        Command::new("git")
            .current_dir(repo_path)
            .args(args)
            .output()
            .unwrap();
    };
    run(&["add", "."]);
    run(&["commit", "-m", "Initial commit"]);

    // Cheat: agent modifies production code and skips test
    let cheated_src = "function charge() { return 200; }\nmodule.exports = { charge };\n";
    fs::write(repo_path.join("src/billing.js"), cheated_src).unwrap();
    let cheated_test = "const { charge } = require('../src/billing');\ndescribe('Billing', () => {\n  it.skip('charges card', () => {\n    expect(charge()).toBe(100);\n  });\n});\n";
    fs::write(repo_path.join("tests/billing.test.js"), cheated_test).unwrap();

    let proctor_bin = env!("CARGO_BIN_EXE_proctor");
    let output = Command::new(proctor_bin)
        .current_dir(repo_path)
        .args(&["scan", "--base", "HEAD"])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(2));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("BLOCKED"));
}

#[test]
fn test_regression_ci_semicolon_exit_zero_blocks() {
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

    // Cheat: `pytest; exit 0`
    let cheated_ci = "name: CI\njobs:\n  test:\n    steps:\n      - run: pytest; exit 0\n";
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

#[test]
fn test_regression_jest_pass_with_no_tests_blocks() {
    let temp_dir = tempfile::tempdir().unwrap();
    let repo_path = temp_dir.path();
    setup_repo(repo_path);

    let base_cfg = "module.exports = {\n  testEnvironment: 'node',\n};\n";
    fs::write(repo_path.join("jest.config.js"), base_cfg).unwrap();

    let run = |args: &[&str]| {
        Command::new("git")
            .current_dir(repo_path)
            .args(args)
            .output()
            .unwrap();
    };
    run(&["add", "."]);
    run(&["commit", "-m", "Initial commit"]);

    // Cheat: `passWithNoTests: true` added to jest.config.js
    let cheated_cfg =
        "module.exports = {\n  testEnvironment: 'node',\n  passWithNoTests: true,\n};\n";
    fs::write(repo_path.join("jest.config.js"), cheated_cfg).unwrap();

    let proctor_bin = env!("CARGO_BIN_EXE_proctor");
    let output = Command::new(proctor_bin)
        .current_dir(repo_path)
        .args(&["scan", "--base", "HEAD"])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(2));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("BLOCKED"));
    assert!(stdout.contains("PROCTOR-004"));
}
