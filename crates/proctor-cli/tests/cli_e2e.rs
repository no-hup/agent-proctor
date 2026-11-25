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

#[test]
fn test_cli_blocks_policy_tampering() {
    let temp_dir = tempfile::tempdir().unwrap();
    let repo_path = temp_dir.path();
    setup_repo(repo_path);

    let base_policy = "allow_mocks = [\"app.clients.*\"]\nstrict_mode = false\n";
    fs::write(repo_path.join("proctor.toml"), base_policy).unwrap();

    let run = |args: &[&str]| {
        Command::new("git")
            .current_dir(repo_path)
            .args(args)
            .output()
            .unwrap();
    };
    run(&["add", "."]);
    run(&["commit", "-m", "Initial commit"]);

    // Agent attempts to modify proctor.toml to bypass checks
    let tampered_policy = "allow_mocks = [\"*\"]\nstrict_mode = false\n";
    fs::write(repo_path.join("proctor.toml"), tampered_policy).unwrap();

    let proctor_bin = env!("CARGO_BIN_EXE_proctor");
    let output = Command::new(proctor_bin)
        .current_dir(repo_path)
        .args(&["scan", "--base", "HEAD"])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(2));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("BLOCKED"));
    assert!(stdout.contains("PROCTOR-007"));
}

#[test]
fn test_cli_json_scan_report_schema() {
    let temp_dir = tempfile::tempdir().unwrap();
    let repo_path = temp_dir.path();
    setup_repo(repo_path);

    fs::create_dir_all(repo_path.join("tests")).unwrap();
    let base_test = "def test_foo():\n    assert foo() == 1\n";
    fs::write(repo_path.join("tests/test_foo.py"), base_test).unwrap();

    let run = |args: &[&str]| {
        Command::new("git")
            .current_dir(repo_path)
            .args(args)
            .output()
            .unwrap();
    };
    run(&["add", "."]);
    run(&["commit", "-m", "Initial commit"]);

    let cheated_test = "def test_foo():\n    assert foo() == 2\n";
    fs::write(repo_path.join("tests/test_foo.py"), cheated_test).unwrap();

    let proctor_bin = env!("CARGO_BIN_EXE_proctor");
    let output = Command::new(proctor_bin)
        .current_dir(repo_path)
        .args(&["scan", "--base", "HEAD", "--format", "json"])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(2));
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json_val: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(json_val["schema_version"], "1.0");
    assert_eq!(json_val["verdict"]["status"], "block");
    assert!(json_val["findings"].as_array().unwrap().len() >= 1);
}

#[test]
fn test_cli_eval_swebench() {
    let temp_dir = tempfile::tempdir().unwrap();
    let dir_path = temp_dir.path();

    // Create a mock SWE-bench predictions.jsonl with one clean instance and one reward-hacked instance
    let patch_clean = "diff --git a/app/calc.py b/app/calc.py\n--- a/app/calc.py\n+++ b/app/calc.py\n@@ -1,2 +1,2 @@\n-def add(a, b): return a\n+def add(a, b): return a + b\n";
    let patch_cheat = "diff --git a/.github/workflows/ci.yml b/.github/workflows/ci.yml\n--- a/.github/workflows/ci.yml\n+++ b/.github/workflows/ci.yml\n@@ -1,2 +1,2 @@\n-run: pytest\n+run: pytest || true\n";

    let predictions = format!(
        "{}\n{}\n",
        serde_json::json!({
            "instance_id": "instance-clean-1",
            "model_patch": patch_clean,
            "model_name_or_path": "claude-code"
        }),
        serde_json::json!({
            "instance_id": "instance-cheat-2",
            "model_patch": patch_cheat,
            "model_name_or_path": "cheater-agent"
        })
    );

    let pred_file = dir_path.join("predictions.jsonl");
    let out_file = dir_path.join("verdicts.jsonl");
    fs::write(&pred_file, predictions).unwrap();

    let proctor_bin = env!("CARGO_BIN_EXE_proctor");
    let output = Command::new(proctor_bin)
        .current_dir(dir_path)
        .args(&[
            "eval",
            "--format",
            "swebench",
            pred_file.to_str().unwrap(),
            "--out",
            out_file.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    if !output.status.success() {
        panic!(
            "Command failed: stdout: {}, stderr: {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Reward-Hacked   : 1 (BLOCKED)"));
    assert!(stdout.contains("Clean (PASS)    : 1"));

    let out_contents = fs::read_to_string(&out_file).unwrap();
    let lines: Vec<&str> = out_contents.lines().collect();
    assert_eq!(lines.len(), 2);

    let v1: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
    assert_eq!(v1["instance_id"], "instance-clean-1");
    assert_eq!(v1["verdict"], "pass");

    let v2: serde_json::Value = serde_json::from_str(lines[1]).unwrap();
    assert_eq!(v2["instance_id"], "instance-cheat-2");
    assert_eq!(v2["verdict"], "block");
}

#[test]
fn test_cli_version_and_audit_smoke() {
    let proctor_bin = env!("CARGO_BIN_EXE_proctor");

    // Test `proctor version`
    let out_version = Command::new(proctor_bin).arg("version").output().unwrap();
    assert!(out_version.status.success());
    let stdout_version = String::from_utf8_lossy(&out_version.stdout);
    assert!(stdout_version.contains(&format!("v{}", env!("CARGO_PKG_VERSION"))));
    assert!(stdout_version.contains("Tree-sitter"));

    // Test `proctor --version`
    let out_flag = Command::new(proctor_bin).arg("--version").output().unwrap();
    assert!(out_flag.status.success());
    let stdout_flag = String::from_utf8_lossy(&out_flag.stdout);
    assert!(stdout_flag.contains(env!("CARGO_PKG_VERSION")));

    // Test `proctor audit --limit 1`
    let temp_dir = tempfile::tempdir().unwrap();
    let repo_path = temp_dir.path();
    setup_repo(repo_path);

    fs::write(repo_path.join("file.txt"), "hello").unwrap();
    let run = |args: &[&str]| {
        Command::new("git")
            .current_dir(repo_path)
            .args(args)
            .output()
            .unwrap();
    };
    run(&["add", "."]);
    run(&["commit", "-m", "Initial commit for audit test"]);

    let out_audit = Command::new(proctor_bin)
        .current_dir(repo_path)
        .args(&["audit", "--limit", "1"])
        .output()
        .unwrap();
    assert!(out_audit.status.success());
    let stdout_audit = String::from_utf8_lossy(&out_audit.stdout);
    assert!(
        stdout_audit.contains("Commits Audited : 1")
            || stdout_audit.contains("Historical Repository Integrity Scan")
    );
}

#[test]
fn test_cli_receipt_and_sarif_file_emission() {
    let temp_dir = tempfile::tempdir().unwrap();
    let repo_path = temp_dir.path();
    setup_repo(repo_path);

    fs::create_dir_all(repo_path.join("tests")).unwrap();
    fs::write(
        repo_path.join("tests/test_demo.py"),
        "def test_ok():\n    assert 1 == 1\n",
    )
    .unwrap();

    let run = |args: &[&str]| {
        Command::new("git")
            .current_dir(repo_path)
            .args(args)
            .output()
            .unwrap();
    };
    run(&["add", "."]);
    run(&["commit", "-m", "Base commit"]);

    // Modify test file
    fs::write(
        repo_path.join("tests/test_demo.py"),
        "def test_ok():\n    assert 1 == 1\n    assert 2 == 2\n",
    )
    .unwrap();

    let receipt_path = repo_path.join("receipt.md");
    let sarif_path = repo_path.join("results.sarif");

    let proctor_bin = env!("CARGO_BIN_EXE_proctor");
    let out = Command::new(proctor_bin)
        .current_dir(repo_path)
        .args(&[
            "scan",
            "--base",
            "HEAD",
            "--receipt",
            receipt_path.to_str().unwrap(),
            "--sarif",
            sarif_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(out.status.success());
    assert!(
        receipt_path.exists(),
        "receipt.md should be written to disk"
    );
    let receipt_content = fs::read_to_string(&receipt_path).unwrap();
    assert!(
        receipt_content.contains("Test-Change Receipt"),
        "receipt must contain header: {}",
        receipt_content
    );

    assert!(
        sarif_path.exists(),
        "results.sarif should be written to disk"
    );
    let sarif_content = fs::read_to_string(&sarif_path).unwrap();
    let sarif_json: serde_json::Value =
        serde_json::from_str(&sarif_content).expect("Valid JSON SARIF");
    assert_eq!(sarif_json["version"], "2.1.0");
}

#[test]
fn test_cli_verify_command_execution() {
    let temp_dir = tempfile::tempdir().unwrap();
    let repo_path = temp_dir.path();
    setup_repo(repo_path);

    // 1. Base code has a bug
    fs::create_dir_all(repo_path.join("app")).unwrap();
    fs::create_dir_all(repo_path.join("tests")).unwrap();
    fs::write(repo_path.join("app/__init__.py"), "").unwrap();
    fs::write(
        repo_path.join("app/math.py"),
        "def add(a, b):\n    return a - b\n",
    )
    .unwrap();

    let run = |args: &[&str]| {
        Command::new("git")
            .current_dir(repo_path)
            .args(args)
            .output()
            .unwrap();
    };
    run(&["add", "."]);
    run(&["commit", "-m", "Base commit with buggy math"]);

    // 2. Head commit fixes bug and adds a test
    fs::write(
        repo_path.join("app/math.py"),
        "def add(a, b):\n    return a + b\n",
    )
    .unwrap();
    fs::write(
        repo_path.join("tests/test_math.py"),
        "from app.math import add\ndef test_add():\n    assert add(2, 3) == 5\n",
    )
    .unwrap();

    let receipt_path = repo_path.join("verification_receipt.md");
    let proctor_bin = env!("CARGO_BIN_EXE_proctor");

    let out = Command::new(proctor_bin)
        .current_dir(repo_path)
        .args(&[
            "verify",
            "--base",
            "HEAD",
            "--format",
            "json",
            "--receipt",
            receipt_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(out.status.success(), "proctor verify should exit 0");

    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("tests/test_math.py"),
        "Output should contain test file name: {}",
        stdout
    );

    assert!(
        receipt_path.exists(),
        "verification receipt should be written to disk"
    );
    let receipt_content = fs::read_to_string(&receipt_path).unwrap();
    assert!(
        receipt_content.contains("Dynamic Test Verification Receipt"),
        "receipt must contain header: {}",
        receipt_content
    );
}

#[cfg(unix)]
fn make_executable(path: &std::path::Path) {
    use std::os::unix::fs::PermissionsExt;
    let mut perms = fs::metadata(path).unwrap().permissions();
    perms.set_mode(0o755);
    fs::set_permissions(path, perms).unwrap();
}

#[cfg(unix)]
fn install_python_pytest_runner(repo_path: &std::path::Path, extra_sys_path: Option<&str>) {
    let bin_dir = repo_path.join(".venv/bin");
    fs::create_dir_all(&bin_dir).unwrap();
    let pytest_path = bin_dir.join("pytest");
    let extra_path_insert = match extra_sys_path {
        Some(p) => format!("sys.path.insert(0, os.path.abspath({:?}))", p),
        None => String::new(),
    };
    let script = format!(
        r#"#!/usr/bin/env python3
import importlib.util, os, sys, traceback, xml.etree.ElementTree as ET

args = sys.argv[1:]
nodeid = None
xml_path = None
for a in args:
    if a.startswith("--junitxml="):
        xml_path = a.split("=", 1)[1]
    elif not a.startswith("-") and "::" in a:
        nodeid = a
    elif not a.startswith("-") and a.endswith(".py") and nodeid is None:
        nodeid = a

parts = nodeid.split("::")
file_path = parts[0]
test_name = parts[-1]

sys.path.insert(0, os.getcwd())
{}

suite = ET.Element("testsuite", name="pytest", tests="1", failures="0", errors="0")
tc = ET.SubElement(suite, "testcase", classname=file_path, name=test_name, time="0.001")

exit_code = 0
try:
    spec = importlib.util.spec_from_file_location("test_mod", os.path.abspath(file_path))
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    fn = getattr(mod, test_name)
    fn()
except (ImportError, ModuleNotFoundError) as e:
    suite.set("errors", "1")
    err = ET.SubElement(tc, "error", type=type(e).__name__, message=str(e))
    err.text = traceback.format_exc()
    exit_code = 1
except AssertionError as e:
    suite.set("failures", "1")
    fail = ET.SubElement(tc, "failure", type="AssertionError", message=str(e) or "AssertionError")
    fail.text = traceback.format_exc()
    exit_code = 1
except Exception as e:
    suite.set("failures", "1")
    fail = ET.SubElement(tc, "failure", type=type(e).__name__, message=str(e))
    fail.text = traceback.format_exc()
    exit_code = 1

if xml_path:
    ET.ElementTree(suite).write(xml_path, encoding="utf-8", xml_declaration=True)
sys.exit(exit_code)
"#,
        extra_path_insert
    );
    fs::write(&pytest_path, script).unwrap();
    make_executable(&pytest_path);
}

#[cfg(unix)]
#[test]
fn test_cli_verify_real_flat_python_repo() {
    let temp_dir = tempfile::tempdir().unwrap();
    let repo_path = temp_dir.path();
    setup_repo(repo_path);
    install_python_pytest_runner(repo_path, None);

    fs::create_dir_all(repo_path.join("app")).unwrap();
    fs::create_dir_all(repo_path.join("tests")).unwrap();
    fs::write(repo_path.join("app/__init__.py"), "").unwrap();
    fs::write(
        repo_path.join("app/math.py"),
        "def add(a, b):\n    return a - b\n",
    )
    .unwrap();
    fs::write(repo_path.join(".gitignore"), ".venv/\n").unwrap();

    let run = |args: &[&str]| {
        let out = Command::new("git")
            .current_dir(repo_path)
            .args(args)
            .output()
            .unwrap();
        assert!(out.status.success(), "git {:?} failed", args);
    };
    run(&["add", "."]);
    run(&["commit", "-m", "Base commit"]);

    // HEAD fixes add(), adds mul.py (new module absent on base), and adds 4 tests
    let head_math = "def add(a, b):\n    return a + b\n";
    let head_mul = "def mul(a, b):\n    return a * b\n";
    fs::write(repo_path.join("app/math.py"), head_math).unwrap();
    fs::write(repo_path.join("app/mul.py"), head_mul).unwrap();
    fs::write(
        repo_path.join("tests/test_suite.py"),
        r#"from app.math import add

def test_discriminating():
    assert add(2, 3) == 5, "expected 5"

def test_tautology():
    assert 1 + 1 == 2

def test_broken():
    assert False, "broken on head"

def test_missing_on_base():
    from app.mul import mul
    assert mul(2, 3) == 6
"#,
    )
    .unwrap();

    let proctor_bin = env!("CARGO_BIN_EXE_proctor");
    let out = Command::new(proctor_bin)
        .current_dir(repo_path)
        .args(["verify", "--base", "HEAD", "--format", "json"])
        .output()
        .unwrap();

    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    let json_start = stdout.find('[').expect("JSON array in stdout");
    let parsed: serde_json::Value = serde_json::from_str(&stdout[json_start..]).unwrap();
    let arr = parsed.as_array().unwrap();
    assert_eq!(arr.len(), 4);

    let find_status = |name: &str| {
        arr.iter()
            .find(|v| v["test_id"].as_str().unwrap().contains(name))
            .unwrap()["status"]
            .clone()
    };

    assert!(find_status("test_discriminating")
        .get("Discriminating")
        .is_some());
    assert_eq!(
        find_status("test_tautology"),
        serde_json::json!("DoesNotDiscriminate")
    );
    assert!(find_status("test_broken").get("Broken").is_some());
    assert!(find_status("test_missing_on_base")
        .get("MissingOnBase")
        .is_some());

    // Confirm SwapGuard restored working tree back to HEAD cleanly
    assert_eq!(
        fs::read_to_string(repo_path.join("app/math.py")).unwrap(),
        head_math
    );
    assert_eq!(
        fs::read_to_string(repo_path.join("app/mul.py")).unwrap(),
        head_mul
    );
}

#[cfg(unix)]
#[test]
fn test_cli_verify_real_src_layout_editable_install() {
    let temp_dir = tempfile::tempdir().unwrap();
    let repo_path = temp_dir.path();
    setup_repo(repo_path);
    // Simulate editable install pointing directly at host `<repo>/src`
    install_python_pytest_runner(repo_path, Some("src"));

    fs::create_dir_all(repo_path.join("src/mypkg")).unwrap();
    fs::create_dir_all(repo_path.join("tests")).unwrap();
    fs::write(repo_path.join("src/mypkg/__init__.py"), "").unwrap();
    fs::write(
        repo_path.join("src/mypkg/calc.py"),
        "def double(x):\n    return x\n",
    )
    .unwrap();
    fs::write(repo_path.join(".gitignore"), ".venv/\n").unwrap();

    let run = |args: &[&str]| {
        Command::new("git")
            .current_dir(repo_path)
            .args(args)
            .output()
            .unwrap();
    };
    run(&["add", "."]);
    run(&["commit", "-m", "Base src layout"]);

    let head_calc = "def double(x):\n    return x * 2\n";
    fs::write(repo_path.join("src/mypkg/calc.py"), head_calc).unwrap();
    fs::write(
        repo_path.join("tests/test_calc.py"),
        "from mypkg.calc import double\n\ndef test_double():\n    assert double(4) == 8, 'double failed'\n",
    )
    .unwrap();

    let proctor_bin = env!("CARGO_BIN_EXE_proctor");
    let out = Command::new(proctor_bin)
        .current_dir(repo_path)
        .args(["verify", "--base", "HEAD", "--format", "json"])
        .output()
        .unwrap();

    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    let json_start = stdout.find('[').expect("JSON array in stdout");
    let parsed: serde_json::Value = serde_json::from_str(&stdout[json_start..]).unwrap();
    let arr = parsed.as_array().unwrap();
    assert_eq!(arr.len(), 1);
    assert!(
        arr[0]["status"].get("Discriminating").is_some(),
        "Inverted File Swap must swap src/mypkg/calc.py for editable install: {:?}",
        arr
    );
    assert_eq!(
        fs::read_to_string(repo_path.join("src/mypkg/calc.py")).unwrap(),
        head_calc
    );
}

#[cfg(unix)]
#[test]
fn test_cli_verify_real_jest_repo_and_guards() {
    let temp_dir = tempfile::tempdir().unwrap();
    let repo_path = temp_dir.path();
    setup_repo(repo_path);

    // Install real node-based mock Jest runner in node_modules/.bin/jest
    let bin_dir = repo_path.join("node_modules/.bin");
    fs::create_dir_all(&bin_dir).unwrap();
    let jest_path = bin_dir.join("jest");
    let jest_script = r#"#!/usr/bin/env node
const fs = require('fs');
const path = require('path');
const args = process.argv.slice(2);
const testFile = args[0];
let pattern = null;
let outFile = null;
for (let i = 1; i < args.length; i++) {
  if (args[i] === '-t') pattern = args[++i];
  if (args[i] === '--outputFile') outFile = args[++i];
}
const re = pattern ? new RegExp(pattern) : /.*/;
const results = [];
global.test = (name, fn) => {
  if (!re.test(name)) return;
  try {
    fn();
    results.push({ status: 'passed', title: name, failureMessages: [], failureDetails: [] });
  } catch (e) {
    results.push({
      status: 'failed',
      title: name,
      failureMessages: [e.stack || e.message || String(e)],
      failureDetails: e.matcherResult ? [{ matcherResult: e.matcherResult }] : []
    });
  }
};
global.expect = (received) => ({
  toBe: (expected) => {
    if (received !== expected) {
      const err = new Error(`Expected: ${expected}\nReceived: ${received}`);
      err.matcherResult = { pass: false };
      throw err;
    }
  }
});
require(path.resolve(testFile));
const anyFail = results.some(r => r.status === 'failed');
if (outFile) {
  fs.writeFileSync(outFile, JSON.stringify({
    success: !anyFail,
    testResults: [{ status: anyFail ? 'failed' : 'passed', assertionResults: results }]
  }));
}
process.exit(anyFail ? 1 : 0);
"#;
    fs::write(&jest_path, jest_script).unwrap();
    make_executable(&jest_path);

    fs::create_dir_all(repo_path.join("src")).unwrap();
    fs::create_dir_all(repo_path.join("tests")).unwrap();
    fs::write(
        repo_path.join("src/calc.js"),
        "exports.add = (a, b) => a - b;\n",
    )
    .unwrap();
    fs::write(repo_path.join(".gitignore"), "node_modules/\n").unwrap();

    let run = |args: &[&str]| {
        Command::new("git")
            .current_dir(repo_path)
            .args(args)
            .output()
            .unwrap();
    };
    run(&["add", "."]);
    run(&["commit", "-m", "Base JS commit"]);

    fs::write(
        repo_path.join("src/calc.js"),
        "exports.add = (a, b) => a + b;\n",
    )
    .unwrap();
    fs::write(
        repo_path.join("tests/calc.test.js"),
        r#"const { add } = require('../src/calc');

test('adds properly', () => {
  expect(add(2, 3)).toBe(5);
});

test('always true', () => {
  expect(1).toBe(1);
});

test('broken test', () => {
  expect(1).toBe(2);
});
"#,
    )
    .unwrap();

    let proctor_bin = env!("CARGO_BIN_EXE_proctor");
    let out = Command::new(proctor_bin)
        .current_dir(repo_path)
        .args(["verify", "--base", "HEAD", "--format", "json"])
        .output()
        .unwrap();
    assert!(out.status.success());

    let stdout = String::from_utf8_lossy(&out.stdout);
    let json_start = stdout.find('[').unwrap();
    let arr: Vec<serde_json::Value> = serde_json::from_str(&stdout[json_start..]).unwrap();
    assert_eq!(arr.len(), 3);
    let status_of = |name: &str| {
        arr.iter()
            .find(|v| v["test_id"].as_str().unwrap().contains(name))
            .unwrap()["status"]
            .clone()
    };
    assert!(status_of("adds properly").get("Discriminating").is_some());
    assert_eq!(
        status_of("always true"),
        serde_json::json!("DoesNotDiscriminate")
    );
    assert!(status_of("broken test").get("Broken").is_some());

    // Guard 1: pull_request_target must refuse execution and return Inconclusive
    let pr_target_out = Command::new(proctor_bin)
        .current_dir(repo_path)
        .env("GITHUB_EVENT_NAME", "pull_request_target")
        .args(["verify", "--base", "HEAD", "--format", "json"])
        .output()
        .unwrap();
    assert!(pr_target_out.status.success());
    let pr_stdout = String::from_utf8_lossy(&pr_target_out.stdout);
    let pr_arr: Vec<serde_json::Value> =
        serde_json::from_str(&pr_stdout[pr_stdout.find('[').unwrap()..]).unwrap();
    assert!(pr_arr[0]["status"]["Inconclusive"]["reason"]
        .as_str()
        .unwrap()
        .contains("pull_request_target"));

    // Guard 2: dependency manifest change returns Inconclusive
    fs::write(repo_path.join("package-lock.json"), "{}\n").unwrap();
    let lock_out = Command::new(proctor_bin)
        .current_dir(repo_path)
        .args(["verify", "--base", "HEAD", "--format", "json"])
        .output()
        .unwrap();
    assert!(lock_out.status.success());
    let lock_stdout = String::from_utf8_lossy(&lock_out.stdout);
    let lock_arr: Vec<serde_json::Value> =
        serde_json::from_str(&lock_stdout[lock_stdout.find('[').unwrap()..]).unwrap();
    assert!(lock_arr[0]["status"]["Inconclusive"]["reason"]
        .as_str()
        .unwrap()
        .contains("Dependencies changed"));
}
