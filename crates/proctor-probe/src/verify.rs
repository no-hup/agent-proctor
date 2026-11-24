use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const MAX_VERIFY_TARGETS: usize = 25;
const DEFAULT_RUN_TIMEOUT: Duration = Duration::from_secs(20);
const TOTAL_VERIFY_BUDGET: Duration = Duration::from_secs(120);

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

    pub fn detect_for_file(file: &str, repo_root: &Path) -> Self {
        if file.ends_with(".py") {
            Self::Pytest
        } else if file.ends_with(".ts")
            || file.ends_with(".tsx")
            || file.ends_with(".js")
            || file.ends_with(".jsx")
            || file.ends_with(".mjs")
            || file.ends_with(".cjs")
            || file.ends_with(".mts")
            || file.ends_with(".cts")
        {
            let has_vitest_config = [
                "vitest.config.ts",
                "vitest.config.js",
                "vitest.config.mts",
                "vitest.config.mjs",
            ]
            .iter()
            .any(|cfg| repo_root.join(cfg).exists());

            if has_vitest_config
                || (repo_root.join("node_modules/.bin/vitest").exists()
                    && !repo_root.join("node_modules/.bin/jest").exists())
            {
                Self::Vitest
            } else {
                Self::Jest
            }
        } else {
            Self::Auto
        }
    }
}

/// Specific test target extracted from the semantic diff.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct VerifyTarget {
    pub file_path: String,
    pub scope_hierarchy: Vec<String>,
    pub test_name: String,
}

impl VerifyTarget {
    /// Stable canonical test identifier (matches pytest nodeid format).
    pub fn canonical_id(&self) -> String {
        if self.scope_hierarchy.is_empty() {
            format!("{}::{}", self.file_path, self.test_name)
        } else {
            format!(
                "{}::{}::{}",
                self.file_path,
                self.scope_hierarchy.join("::"),
                self.test_name
            )
        }
    }

    /// Full display/filter name for Jest/Vitest `-t`.
    pub fn js_filter_name(&self) -> String {
        if self.scope_hierarchy.is_empty() {
            self.test_name.clone()
        } else {
            format!("{} {}", self.scope_hierarchy.join(" "), self.test_name)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum VerificationStatus {
    /// Consistently passes on head and fails on base with an assertion failure in the test body.
    Discriminating { reason: String },
    /// Consistently passes on head and fails on base due to a missing module/symbol/import.
    MissingOnBase { error: String },
    /// Passes on both head and base — does not discriminate between base and head behavior.
    DoesNotDiscriminate,
    /// Fails on head itself — the PR's test does not pass on the head revision.
    Broken { error: String },
    /// Timeout, flakiness, dependency changes, missing runner, setup crash, or security guard.
    Inconclusive { reason: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestVerificationResult {
    pub test_id: String,
    pub test_file: String,
    pub status: VerificationStatus,
    pub runner: String,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum SingleRunOutcome {
    Passed,
    AssertionFailure { error_type: String, message: String },
    MissingSymbol { error_type: String, message: String },
    SetupOrCollectionError { error_type: String, message: String },
    ExternalDependency { reason: String },
    TimedOut,
    OtherFailure { error_type: String, message: String },
}

#[derive(Debug)]
struct TimedOutput {
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    success: bool,
    timed_out: bool,
}

/// RAII guard that swaps non-test production files in `repo_root` and restores
/// exact pre-existing disk contents on `restore()` or `Drop` (even on panic).
struct SwapGuard {
    backups: HashMap<PathBuf, Option<Vec<u8>>>,
    created_dirs: Vec<PathBuf>,
}

impl SwapGuard {
    fn new() -> Self {
        Self {
            backups: HashMap::new(),
            created_dirs: Vec::new(),
        }
    }

    /// Snapshots `abs_path` once (if not already backed up) before modifying it.
    fn record_original(&mut self, abs_path: &Path) {
        if self.backups.contains_key(abs_path) {
            return;
        }
        let existing = std::fs::read(abs_path).ok();
        self.backups.insert(abs_path.to_path_buf(), existing);
    }

    /// Replaces `rel_path` in `repo_root` with `new_bytes`, or deletes it if `None`.
    fn apply_file_state(
        &mut self,
        repo_root: &Path,
        rel_path: &str,
        new_bytes: Option<&[u8]>,
    ) -> Result<()> {
        let abs_path = repo_root.join(rel_path);
        self.record_original(&abs_path);

        if rel_path.ends_with(".py") {
            invalidate_pycache_for(repo_root, rel_path);
        }

        match new_bytes {
            Some(bytes) => {
                if let Some(parent) = abs_path.parent() {
                    if !parent.exists() {
                        std::fs::create_dir_all(parent)?;
                        self.created_dirs.push(parent.to_path_buf());
                    }
                }
                std::fs::write(&abs_path, bytes)?;
            }
            None => {
                if abs_path.exists() {
                    std::fs::remove_file(&abs_path)?;
                }
            }
        }
        Ok(())
    }

    /// Swaps all specified relative paths to their content at `git_ref`.
    fn swap_paths_to_ref(
        &mut self,
        repo_root: &Path,
        git_ref: &str,
        rel_paths: &[String],
    ) -> Result<()> {
        for rel_path in rel_paths {
            let spec = format!("{}:{}", git_ref, rel_path);
            let out = Command::new("git")
                .current_dir(repo_root)
                .args(["show", &spec])
                .output()
                .with_context(|| format!("Failed to inspect git ref {}", spec))?;

            if out.status.success() {
                self.apply_file_state(repo_root, rel_path, Some(&out.stdout))?;
            } else {
                // Path did not exist at git_ref
                self.apply_file_state(repo_root, rel_path, None)?;
            }
        }
        Ok(())
    }

    /// Restores all swapped files back to their pre-verification disk state.
    fn restore(&mut self) {
        for (abs_path, original) in self.backups.drain() {
            if abs_path.extension().and_then(|e| e.to_str()) == Some("py") {
                if let Some(parent) = abs_path.parent() {
                    let cache_dir = parent.join("__pycache__");
                    if cache_dir.is_dir() {
                        if let Some(stem) = abs_path.file_stem().and_then(|s| s.to_str()) {
                            purge_stem_pyc(&cache_dir, stem);
                        }
                    }
                }
            }

            match original {
                Some(bytes) => {
                    if let Some(parent) = abs_path.parent() {
                        let _ = std::fs::create_dir_all(parent);
                    }
                    let _ = std::fs::write(&abs_path, bytes);
                }
                None => {
                    let _ = std::fs::remove_file(&abs_path);
                }
            }
        }
        for dir in self.created_dirs.drain(..).rev() {
            let _ = std::fs::remove_dir(dir);
        }
    }
}

impl Drop for SwapGuard {
    fn drop(&mut self) {
        self.restore();
    }
}

fn invalidate_pycache_for(repo_root: &Path, rel_path: &str) {
    let abs_path = repo_root.join(rel_path);
    if let (Some(parent), Some(stem)) = (
        abs_path.parent(),
        abs_path.file_stem().and_then(|s| s.to_str()),
    ) {
        let cache_dir = parent.join("__pycache__");
        if cache_dir.is_dir() {
            purge_stem_pyc(&cache_dir, stem);
        }
    }
}

fn purge_stem_pyc(cache_dir: &Path, stem: &str) {
    if let Ok(entries) = std::fs::read_dir(cache_dir) {
        let prefix = format!("{}.", stem);
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str.starts_with(&prefix) && name_str.ends_with(".pyc") {
                let _ = std::fs::remove_file(entry.path());
            }
        }
    }
}

/// Returns true if a file is a test file or test support artifact (fixtures, snapshots,
/// conftest, test runner setup) that must remain at HEAD during the base comparison.
pub fn is_test_or_support_file(path: &str) -> bool {
    if proctor_diff::is_test_file(path) {
        return true;
    }

    let p = Path::new(path);
    for component in p.components() {
        let comp = component.as_os_str().to_string_lossy().to_lowercase();
        if matches!(
            comp.as_str(),
            "tests"
                | "test"
                | "__tests__"
                | "__snapshots__"
                | "__fixtures__"
                | "__mocks__"
                | "fixtures"
                | "testdata"
        ) {
            return true;
        }
    }

    let file_name = p
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_lowercase();

    matches!(
        file_name.as_str(),
        "conftest.py"
            | "jest.config.js"
            | "jest.config.ts"
            | "jest.config.mjs"
            | "jest.config.cjs"
            | "jest.setup.js"
            | "jest.setup.ts"
            | "vitest.config.ts"
            | "vitest.config.js"
            | "vitest.config.mts"
            | "vitest.config.mjs"
            | "vitest.setup.ts"
            | "vitest.setup.js"
    )
}

/// Returns the path of the first changed dependency manifest or lockfile, if any.
pub fn find_changed_dependency_manifest(changed_paths: &[String]) -> Option<String> {
    for path in changed_paths {
        let file_name = Path::new(path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_lowercase();

        let is_manifest = matches!(
            file_name.as_str(),
            "package.json"
                | "package-lock.json"
                | "npm-shrinkwrap.json"
                | "yarn.lock"
                | "pnpm-lock.yaml"
                | "bun.lockb"
                | "bun.lock"
                | "pyproject.toml"
                | "poetry.lock"
                | "pipfile"
                | "pipfile.lock"
                | "uv.lock"
                | "setup.py"
                | "setup.cfg"
                | "environment.yml"
                | "conda.yml"
        ) || (file_name.starts_with("requirements")
            && (file_name.ends_with(".txt") || file_name.ends_with(".in")));

        if is_manifest {
            return Some(path.clone());
        }
    }
    None
}

/// Resolves changed paths between `base_ref` and `head_ref` (or working tree if `None`).
fn collect_changed_paths(
    repo_root: &Path,
    base_ref: &str,
    head_ref: Option<&str>,
    extra_paths: &[String],
) -> Vec<String> {
    let mut paths = BTreeSet::new();
    for p in extra_paths {
        paths.insert(p.clone());
    }

    let mut cmd = Command::new("git");
    cmd.current_dir(repo_root)
        .arg("diff")
        .arg("--name-only")
        .arg("--no-renames");
    if let Some(h) = head_ref {
        cmd.arg(format!("{}..{}", base_ref, h));
    } else {
        cmd.arg(base_ref);
    }

    if let Ok(out) = cmd.output() {
        if out.status.success() {
            for line in String::from_utf8_lossy(&out.stdout).lines() {
                let p = line.trim().to_string();
                if !p.is_empty() {
                    paths.insert(p);
                }
            }
        }
    }

    if head_ref.is_none() {
        if let Ok(out) = Command::new("git")
            .current_dir(repo_root)
            .args(["ls-files", "--others", "--exclude-standard"])
            .output()
        {
            if out.status.success() {
                for line in String::from_utf8_lossy(&out.stdout).lines() {
                    let p = line.trim().to_string();
                    if !p.is_empty() {
                        paths.insert(p);
                    }
                }
            }
        }
    }

    paths.into_iter().collect()
}

/// Runs dynamic regression verification using the Inverted File Swap pattern:
/// executes target tests in the host checkout (preserving `.venv` / editable installs /
/// `node_modules`), runs HEAD twice, swaps changed non-test files to `base_ref`, runs BASE
/// twice, and restores original disk state via RAII `SwapGuard`.
pub fn verify_tests(
    repo_root: &Path,
    base_ref: &str,
    head_ref: Option<&str>,
    runner: RunnerType,
    targets: &[VerifyTarget],
    changed_paths_hint: &[String],
) -> Result<Vec<TestVerificationResult>> {
    verify_tests_with_timeout(
        repo_root,
        base_ref,
        head_ref,
        runner,
        targets,
        changed_paths_hint,
        DEFAULT_RUN_TIMEOUT,
    )
}

/// Configurable-timeout entrypoint for testing and custom budgets.
pub fn verify_tests_with_timeout(
    repo_root: &Path,
    base_ref: &str,
    head_ref: Option<&str>,
    runner: RunnerType,
    targets: &[VerifyTarget],
    changed_paths_hint: &[String],
    per_run_timeout: Duration,
) -> Result<Vec<TestVerificationResult>> {
    let mut results = Vec::new();
    if targets.is_empty() {
        return Ok(results);
    }

    // 1. Pre-flight security check: refuse dynamic execution under pull_request_target
    if std::env::var("GITHUB_EVENT_NAME").as_deref() == Ok("pull_request_target") {
        for target in targets {
            results.push(TestVerificationResult {
                test_id: target.canonical_id(),
                test_file: target.file_path.clone(),
                status: VerificationStatus::Inconclusive {
                    reason: "Refused: dynamic verification disabled on pull_request_target"
                        .to_string(),
                },
                runner: runner_label(runner, &target.file_path, repo_root),
                duration_ms: 0,
            });
        }
        return Ok(results);
    }

    // 2. Collect changed paths & check if dependencies/manifests changed
    let all_changed = collect_changed_paths(repo_root, base_ref, head_ref, changed_paths_hint);
    if let Some(manifest) = find_changed_dependency_manifest(&all_changed) {
        for target in targets {
            results.push(TestVerificationResult {
                test_id: target.canonical_id(),
                test_file: target.file_path.clone(),
                status: VerificationStatus::Inconclusive {
                    reason: format!(
                        "Dependencies changed ({}); base and head environments may differ",
                        manifest
                    ),
                },
                runner: runner_label(runner, &target.file_path, repo_root),
                duration_ms: 0,
            });
        }
        return Ok(results);
    }

    // Cap active targets to keep CI runtime bounded
    let (active_targets, overflow_targets) = if targets.len() > MAX_VERIFY_TARGETS {
        (
            &targets[..MAX_VERIFY_TARGETS],
            &targets[MAX_VERIFY_TARGETS..],
        )
    } else {
        (targets, &[][..])
    };

    let mut swap_guard = SwapGuard::new();

    // If caller explicitly passed a head_ref other than current HEAD, stage all changed
    // paths to head_ref before the HEAD pass (and restore on Drop).
    if let Some(h_ref) = head_ref {
        if !refs_point_to_same_commit(repo_root, h_ref, "HEAD") {
            swap_guard.swap_paths_to_ref(repo_root, h_ref, &all_changed)?;
        }
    }

    let total_start = Instant::now();
    let mut target_timings: Vec<Instant> = Vec::with_capacity(active_targets.len());
    let mut head_outcomes: Vec<Option<VerificationStatus>> = vec![None; active_targets.len()];
    let mut resolved_runners: Vec<RunnerType> = Vec::with_capacity(active_targets.len());

    // 3. HEAD phase: each test must pass TWICE on head
    for (idx, target) in active_targets.iter().enumerate() {
        target_timings.push(Instant::now());
        let active_runner = match runner {
            RunnerType::Auto => RunnerType::detect_for_file(&target.file_path, repo_root),
            r => r,
        };
        resolved_runners.push(active_runner);

        if total_start.elapsed() >= TOTAL_VERIFY_BUDGET {
            head_outcomes[idx] = Some(VerificationStatus::Inconclusive {
                reason: "Skipped: exceeded total verification time budget".to_string(),
            });
            continue;
        }

        if active_runner == RunnerType::Auto {
            head_outcomes[idx] = Some(VerificationStatus::Inconclusive {
                reason: "Unrecognized test file extension".to_string(),
            });
            continue;
        }

        let run1 = run_single_test(repo_root, active_runner, target, per_run_timeout);
        match run1 {
            SingleRunOutcome::Passed => {
                let run2 = run_single_test(repo_root, active_runner, target, per_run_timeout);
                match run2 {
                    SingleRunOutcome::Passed => {
                        // Passed twice on HEAD; eligible for BASE verification
                    }
                    SingleRunOutcome::TimedOut => {
                        head_outcomes[idx] = Some(VerificationStatus::Inconclusive {
                            reason: "Timed out on head (run 2)".to_string(),
                        });
                    }
                    _ => {
                        head_outcomes[idx] = Some(VerificationStatus::Inconclusive {
                            reason: "Flaky on head (passed run 1, failed run 2)".to_string(),
                        });
                    }
                }
            }
            SingleRunOutcome::TimedOut => {
                head_outcomes[idx] = Some(VerificationStatus::Inconclusive {
                    reason: format!("Timed out on head (>{}s)", per_run_timeout.as_secs()),
                });
            }
            SingleRunOutcome::ExternalDependency { reason } => {
                head_outcomes[idx] = Some(VerificationStatus::Inconclusive { reason });
            }
            SingleRunOutcome::SetupOrCollectionError { message, .. } => {
                head_outcomes[idx] = Some(VerificationStatus::Inconclusive {
                    reason: format!("Setup/collection error on head: {}", message),
                });
            }
            SingleRunOutcome::AssertionFailure { message, .. }
            | SingleRunOutcome::MissingSymbol { message, .. }
            | SingleRunOutcome::OtherFailure { message, .. } => {
                head_outcomes[idx] = Some(VerificationStatus::Broken { error: message });
            }
        }
    }

    // Check if any target needs BASE verification
    let any_needs_base = head_outcomes.iter().any(|o| o.is_none());
    let mut final_statuses = head_outcomes;

    if any_needs_base {
        // 4. Inverted File Swap: swap ONLY non-test/non-support files to base_ref.
        // Test files, conftest.py, fixtures, and snapshots remain at HEAD.
        let non_test_paths: Vec<String> = all_changed
            .iter()
            .filter(|p| !is_test_or_support_file(p))
            .cloned()
            .collect();

        swap_guard.swap_paths_to_ref(repo_root, base_ref, &non_test_paths)?;

        // 5. BASE phase: run tests that passed twice on HEAD
        for (idx, target) in active_targets.iter().enumerate() {
            if final_statuses[idx].is_some() {
                continue;
            }
            if total_start.elapsed() >= TOTAL_VERIFY_BUDGET {
                final_statuses[idx] = Some(VerificationStatus::Inconclusive {
                    reason: "Skipped: exceeded total verification time budget".to_string(),
                });
                continue;
            }

            let active_runner = resolved_runners[idx];
            let base1 = run_single_test(repo_root, active_runner, target, per_run_timeout);

            let status = match base1 {
                SingleRunOutcome::Passed => {
                    let base2 = run_single_test(repo_root, active_runner, target, per_run_timeout);
                    if base2 == SingleRunOutcome::Passed {
                        VerificationStatus::DoesNotDiscriminate
                    } else {
                        VerificationStatus::Inconclusive {
                            reason: "Flaky on base (passed run 1, failed run 2)".to_string(),
                        }
                    }
                }
                SingleRunOutcome::TimedOut => VerificationStatus::Inconclusive {
                    reason: format!("Timed out on base (>{}s)", per_run_timeout.as_secs()),
                },
                SingleRunOutcome::ExternalDependency { reason } => {
                    VerificationStatus::Inconclusive { reason }
                }
                SingleRunOutcome::SetupOrCollectionError {
                    error_type,
                    message,
                } => VerificationStatus::Inconclusive {
                    reason: format!("{} on base: {}", error_type, message),
                },
                SingleRunOutcome::AssertionFailure {
                    error_type: t1,
                    message: m1,
                } => {
                    let base2 = run_single_test(repo_root, active_runner, target, per_run_timeout);
                    match base2 {
                        SingleRunOutcome::AssertionFailure { error_type: t2, .. } if t1 == t2 => {
                            VerificationStatus::Discriminating { reason: m1 }
                        }
                        SingleRunOutcome::TimedOut => VerificationStatus::Inconclusive {
                            reason: "Timed out on base (run 2)".to_string(),
                        },
                        _ => VerificationStatus::Inconclusive {
                            reason: "Flaky on base (inconsistent failure across runs)".to_string(),
                        },
                    }
                }
                SingleRunOutcome::MissingSymbol {
                    error_type: t1,
                    message: m1,
                } => {
                    let base2 = run_single_test(repo_root, active_runner, target, per_run_timeout);
                    match base2 {
                        SingleRunOutcome::MissingSymbol { error_type: t2, .. } if t1 == t2 => {
                            VerificationStatus::MissingOnBase { error: m1 }
                        }
                        SingleRunOutcome::TimedOut => VerificationStatus::Inconclusive {
                            reason: "Timed out on base (run 2)".to_string(),
                        },
                        _ => VerificationStatus::Inconclusive {
                            reason: "Flaky on base (inconsistent missing-symbol error)".to_string(),
                        },
                    }
                }
                SingleRunOutcome::OtherFailure {
                    error_type,
                    message,
                } => VerificationStatus::Inconclusive {
                    reason: format!(
                        "Non-assertion failure on base ({}): {}",
                        error_type, message
                    ),
                },
            };

            final_statuses[idx] = Some(status);
        }
    }

    // 6. Explicitly restore host working tree before constructing results
    swap_guard.restore();

    for (idx, target) in active_targets.iter().enumerate() {
        let elapsed = target_timings[idx].elapsed().as_millis() as u64;
        let status = final_statuses[idx]
            .take()
            .unwrap_or(VerificationStatus::Inconclusive {
                reason: "Verification did not complete".to_string(),
            });
        results.push(TestVerificationResult {
            test_id: target.canonical_id(),
            test_file: target.file_path.clone(),
            status,
            runner: runner_name_str(resolved_runners[idx]),
            duration_ms: elapsed,
        });
    }

    for target in overflow_targets {
        results.push(TestVerificationResult {
            test_id: target.canonical_id(),
            test_file: target.file_path.clone(),
            status: VerificationStatus::Inconclusive {
                reason: format!("Skipped: exceeded {}-test CI cap", MAX_VERIFY_TARGETS),
            },
            runner: runner_label(runner, &target.file_path, repo_root),
            duration_ms: 0,
        });
    }

    Ok(results)
}

fn refs_point_to_same_commit(repo_root: &Path, ref_a: &str, ref_b: &str) -> bool {
    let rev = |r: &str| {
        Command::new("git")
            .current_dir(repo_root)
            .args(["rev-parse", "--verify", &format!("{}^{{commit}}", r)])
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
    };
    match (rev(ref_a), rev(ref_b)) {
        (Some(a), Some(b)) => a == b,
        _ => false,
    }
}

fn runner_name_str(r: RunnerType) -> String {
    match r {
        RunnerType::Pytest => "pytest",
        RunnerType::Jest => "jest",
        RunnerType::Vitest => "vitest",
        RunnerType::Auto => "unknown",
    }
    .to_string()
}

fn runner_label(runner: RunnerType, file: &str, repo_root: &Path) -> String {
    match runner {
        RunnerType::Auto => runner_name_str(RunnerType::detect_for_file(file, repo_root)),
        r => runner_name_str(r),
    }
}

fn run_single_test(
    repo_root: &Path,
    runner: RunnerType,
    target: &VerifyTarget,
    timeout: Duration,
) -> SingleRunOutcome {
    match runner {
        RunnerType::Pytest => run_pytest(repo_root, target, timeout),
        RunnerType::Jest => run_jest(repo_root, target, timeout),
        RunnerType::Vitest => run_vitest(repo_root, target, timeout),
        RunnerType::Auto => SingleRunOutcome::ExternalDependency {
            reason: "Unrecognized test file type".to_string(),
        },
    }
}

/// Resolves local pytest binary or `python -m pytest` without network downloads.
enum PytestCmd {
    Direct(PathBuf),
    PythonModule(PathBuf),
}

fn resolve_pytest_cmd(repo_root: &Path) -> Option<PytestCmd> {
    for rel in [".venv/bin/pytest", "venv/bin/pytest"] {
        let p = repo_root.join(rel);
        if p.exists() {
            return Some(PytestCmd::Direct(p));
        }
    }
    for rel in [".venv/bin/python", "venv/bin/python"] {
        let p = repo_root.join(rel);
        if p.exists() {
            return Some(PytestCmd::PythonModule(p));
        }
    }
    if Command::new("pytest")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
    {
        return Some(PytestCmd::Direct(PathBuf::from("pytest")));
    }
    if Command::new("python3")
        .args(["-m", "pytest", "--version"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
    {
        return Some(PytestCmd::PythonModule(PathBuf::from("python3")));
    }
    None
}

fn run_pytest(repo_root: &Path, target: &VerifyTarget, timeout: Duration) -> SingleRunOutcome {
    let Some(pytest_cmd) = resolve_pytest_cmd(repo_root) else {
        return SingleRunOutcome::ExternalDependency {
            reason: "pytest not found in repo virtualenv or PATH".to_string(),
        };
    };

    let tmp_dir = match tempfile::Builder::new().prefix("proctor-junit-").tempdir() {
        Ok(d) => d,
        Err(e) => {
            return SingleRunOutcome::SetupOrCollectionError {
                error_type: "TempDirError".to_string(),
                message: e.to_string(),
            };
        }
    };
    let xml_path = tmp_dir.path().join("junit.xml");
    let nodeid = target.canonical_id();

    let mut cmd = match pytest_cmd {
        PytestCmd::Direct(bin) => {
            let mut c = Command::new(bin);
            c.arg(&nodeid);
            c
        }
        PytestCmd::PythonModule(py) => {
            let mut c = Command::new(py);
            c.args(["-B", "-m", "pytest", &nodeid]);
            c
        }
    };

    cmd.current_dir(repo_root)
        .args(["-p", "no:randomly", "-q", "--tb=short"])
        .arg(format!("--junitxml={}", xml_path.display()))
        .env("PYTHONDONTWRITEBYTECODE", "1");

    let output = match run_command_with_timeout(&mut cmd, timeout) {
        Ok(out) => out,
        Err(e) => {
            return SingleRunOutcome::ExternalDependency {
                reason: format!("pytest spawn failed: {}", e),
            };
        }
    };

    if output.timed_out {
        return SingleRunOutcome::TimedOut;
    }

    if xml_path.exists() {
        if let Ok(xml_text) = std::fs::read_to_string(&xml_path) {
            if let Some(outcome) = parse_junit_xml(&xml_text, output.success) {
                return outcome;
            }
        }
    }

    fallback_classify_console(&output.stdout, &output.stderr, output.success)
}

fn run_jest(repo_root: &Path, target: &VerifyTarget, timeout: Duration) -> SingleRunOutcome {
    let jest_bin = repo_root.join("node_modules/.bin/jest");
    if !jest_bin.exists() {
        return SingleRunOutcome::ExternalDependency {
            reason: "jest not found in node_modules/.bin (npx disabled for zero-egress)"
                .to_string(),
        };
    }

    let tmp_dir = match tempfile::Builder::new().prefix("proctor-jest-").tempdir() {
        Ok(d) => d,
        Err(e) => {
            return SingleRunOutcome::SetupOrCollectionError {
                error_type: "TempDirError".to_string(),
                message: e.to_string(),
            };
        }
    };
    let json_path = tmp_dir.path().join("report.json");
    let pattern = format!("^{}$", escape_regex(&target.js_filter_name()));

    let mut cmd = Command::new(&jest_bin);
    cmd.current_dir(repo_root)
        .arg(&target.file_path)
        .args(["-t", &pattern, "--json", "--no-coverage", "--silent"])
        .arg("--outputFile")
        .arg(&json_path);

    let output = match run_command_with_timeout(&mut cmd, timeout) {
        Ok(out) => out,
        Err(e) => {
            return SingleRunOutcome::ExternalDependency {
                reason: format!("jest spawn failed: {}", e),
            };
        }
    };

    if output.timed_out {
        return SingleRunOutcome::TimedOut;
    }

    if json_path.exists() {
        if let Ok(json_text) = std::fs::read_to_string(&json_path) {
            if let Some(outcome) = parse_jest_vitest_json(&json_text) {
                return outcome;
            }
        }
    }

    fallback_classify_console(&output.stdout, &output.stderr, output.success)
}

fn run_vitest(repo_root: &Path, target: &VerifyTarget, timeout: Duration) -> SingleRunOutcome {
    let vitest_bin = repo_root.join("node_modules/.bin/vitest");
    if !vitest_bin.exists() {
        return SingleRunOutcome::ExternalDependency {
            reason: "vitest not found in node_modules/.bin (npx disabled for zero-egress)"
                .to_string(),
        };
    }

    let tmp_dir = match tempfile::Builder::new().prefix("proctor-vitest-").tempdir() {
        Ok(d) => d,
        Err(e) => {
            return SingleRunOutcome::SetupOrCollectionError {
                error_type: "TempDirError".to_string(),
                message: e.to_string(),
            };
        }
    };
    let json_path = tmp_dir.path().join("report.json");
    let pattern = format!("^{}$", escape_regex(&target.js_filter_name()));

    let mut cmd = Command::new(&vitest_bin);
    cmd.current_dir(repo_root)
        .args(["run", &target.file_path, "-t", &pattern, "--reporter=json"])
        .arg("--outputFile")
        .arg(&json_path);

    let output = match run_command_with_timeout(&mut cmd, timeout) {
        Ok(out) => out,
        Err(e) => {
            return SingleRunOutcome::ExternalDependency {
                reason: format!("vitest spawn failed: {}", e),
            };
        }
    };

    if output.timed_out {
        return SingleRunOutcome::TimedOut;
    }

    if json_path.exists() {
        if let Ok(json_text) = std::fs::read_to_string(&json_path) {
            if let Some(outcome) = parse_jest_vitest_json(&json_text) {
                return outcome;
            }
        }
    }

    fallback_classify_console(&output.stdout, &output.stderr, output.success)
}

fn run_command_with_timeout(cmd: &mut Command, timeout: Duration) -> Result<TimedOutput> {
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = cmd.spawn().context("Failed to spawn test runner")?;

    let mut child_stdout = child.stdout.take();
    let mut child_stderr = child.stderr.take();

    let (status_opt, stdout_buf, stderr_buf) = std::thread::scope(|s| {
        let out_handle = s.spawn(move || {
            let mut buf = Vec::new();
            if let Some(ref mut r) = child_stdout {
                let _ = r.read_to_end(&mut buf);
            }
            buf
        });
        let err_handle = s.spawn(move || {
            let mut buf = Vec::new();
            if let Some(ref mut r) = child_stderr {
                let _ = r.read_to_end(&mut buf);
            }
            buf
        });

        let deadline = Instant::now() + timeout;
        let status_opt = loop {
            match child.try_wait() {
                Ok(Some(status)) => break Some(status),
                Ok(None) => {
                    if Instant::now() >= deadline {
                        let _ = child.kill();
                        let _ = child.wait();
                        break None;
                    }
                    std::thread::sleep(Duration::from_millis(20));
                }
                Err(_) => break None,
            }
        };

        let stdout_buf = out_handle.join().unwrap_or_default();
        let stderr_buf = err_handle.join().unwrap_or_default();
        (status_opt, stdout_buf, stderr_buf)
    });

    match status_opt {
        Some(status) => Ok(TimedOutput {
            stdout: stdout_buf,
            stderr: stderr_buf,
            success: status.success(),
            timed_out: false,
        }),
        None => Ok(TimedOutput {
            stdout: stdout_buf,
            stderr: stderr_buf,
            success: false,
            timed_out: true,
        }),
    }
}

/// Parses pytest JUnit XML output into a structured `SingleRunOutcome`.
fn parse_junit_xml(xml: &str, process_success: bool) -> Option<SingleRunOutcome> {
    if !xml.contains("<testcase") {
        return None;
    }

    // Check <error ...> tags first (setup/teardown/collection failures in pytest)
    if let Some((attrs, body)) = extract_xml_tag(xml, "error") {
        let error_type =
            extract_xml_attr(&attrs, "type").unwrap_or_else(|| "SetupError".to_string());
        let message = extract_xml_attr(&attrs, "message")
            .filter(|m| !m.is_empty())
            .unwrap_or_else(|| first_nonempty_line(&body));

        let combined = format!("{}\n{}\n{}", error_type, message, body);
        if let Some(ext) = detect_external_dep(&combined) {
            return Some(SingleRunOutcome::ExternalDependency { reason: ext });
        }
        if is_import_error_text(&combined) {
            return Some(SingleRunOutcome::MissingSymbol {
                error_type: classify_import_type(&combined),
                message: decode_xml_entities(&message),
            });
        }
        return Some(SingleRunOutcome::SetupOrCollectionError {
            error_type: decode_xml_entities(&error_type),
            message: decode_xml_entities(&message),
        });
    }

    // Check <failure ...> tags (test body assertion/exception failures)
    if let Some((attrs, body)) = extract_xml_tag(xml, "failure") {
        let raw_type = extract_xml_attr(&attrs, "type").unwrap_or_default();
        let raw_message = extract_xml_attr(&attrs, "message").unwrap_or_default();
        let message = if !raw_message.is_empty() {
            decode_xml_entities(&raw_message)
        } else {
            decode_xml_entities(&first_nonempty_line(&body))
        };
        let combined = format!("{}\n{}\n{}", raw_type, message, body);

        if let Some(ext) = detect_external_dep(&combined) {
            return Some(SingleRunOutcome::ExternalDependency { reason: ext });
        }
        if is_import_error_text(&combined) {
            return Some(SingleRunOutcome::MissingSymbol {
                error_type: classify_import_type(&combined),
                message,
            });
        }

        // Strict assertion classification: require AssertionError / Failed / assertion pattern
        // and reject TypeError / SyntaxError / NameError / KeyError crashes.
        let is_non_assertion_crash = matches!(
            raw_type.as_str(),
            "TypeError"
                | "SyntaxError"
                | "IndentationError"
                | "NameError"
                | "KeyError"
                | "AttributeError"
        );
        let is_assertion = !is_non_assertion_crash
            && (raw_type == "AssertionError"
                || raw_type == "Failed"
                || raw_type.ends_with(".AssertionError")
                || message.contains("AssertionError")
                || body.contains("AssertionError")
                || (raw_type.is_empty() && body.contains("\nE       assert ")));

        if is_assertion {
            let err_type = if raw_type.is_empty() {
                "AssertionError".to_string()
            } else {
                raw_type
            };
            return Some(SingleRunOutcome::AssertionFailure {
                error_type: err_type,
                message,
            });
        }

        let err_type = if raw_type.is_empty() {
            "RuntimeError".to_string()
        } else {
            raw_type
        };
        return Some(SingleRunOutcome::OtherFailure {
            error_type: err_type,
            message,
        });
    }

    if extract_xml_tag(xml, "skipped").is_some() || xml.contains("<skipped") {
        return Some(SingleRunOutcome::SetupOrCollectionError {
            error_type: "Skipped".to_string(),
            message: "Test was skipped".to_string(),
        });
    }

    if process_success {
        Some(SingleRunOutcome::Passed)
    } else {
        None
    }
}

/// Parses Jest (`--json`) and Vitest (`--reporter=json`) structured JSON output.
fn parse_jest_vitest_json(json_str: &str) -> Option<SingleRunOutcome> {
    let val: serde_json::Value = serde_json::from_str(json_str).ok()?;
    let test_results = val.get("testResults")?.as_array()?;

    let mut saw_assertion = false;

    for suite in test_results {
        let assertions = suite
            .get("assertionResults")
            .and_then(|a| a.as_array())
            .cloned()
            .unwrap_or_default();

        if assertions.is_empty() && suite.get("status").and_then(|s| s.as_str()) == Some("failed") {
            let msg = suite
                .get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("Suite failed to execute");
            if let Some(ext) = detect_external_dep(msg) {
                return Some(SingleRunOutcome::ExternalDependency { reason: ext });
            }
            if is_import_error_text(msg) {
                return Some(SingleRunOutcome::MissingSymbol {
                    error_type: "CannotFindModule".to_string(),
                    message: first_nonempty_line(msg),
                });
            }
            return Some(SingleRunOutcome::SetupOrCollectionError {
                error_type: "SuiteLoadError".to_string(),
                message: first_nonempty_line(msg),
            });
        }

        for assertion in assertions {
            let status = assertion
                .get("status")
                .and_then(|s| s.as_str())
                .unwrap_or("");
            if status == "pending" || status == "skipped" || status == "todo" {
                continue;
            }
            saw_assertion = true;
            if status == "failed" {
                let messages: Vec<&str> = assertion
                    .get("failureMessages")
                    .and_then(|m| m.as_array())
                    .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect())
                    .unwrap_or_default();
                let combined = messages.join("\n");
                let summary = first_nonempty_line(&combined);

                if let Some(ext) = detect_external_dep(&combined) {
                    return Some(SingleRunOutcome::ExternalDependency { reason: ext });
                }
                if is_import_error_text(&combined) {
                    return Some(SingleRunOutcome::MissingSymbol {
                        error_type: "CannotFindModule".to_string(),
                        message: summary,
                    });
                }

                let has_matcher_details = assertion
                    .get("failureDetails")
                    .and_then(|d| d.as_array())
                    .map(|arr| arr.iter().any(|item| item.get("matcherResult").is_some()))
                    .unwrap_or(false);

                let is_jest_assertion = has_matcher_details
                    || combined.contains("Expected:")
                    || combined.contains("AssertionError")
                    || combined.contains("expect(");

                if is_jest_assertion {
                    return Some(SingleRunOutcome::AssertionFailure {
                        error_type: "AssertionError".to_string(),
                        message: summary,
                    });
                }

                return Some(SingleRunOutcome::OtherFailure {
                    error_type: "RuntimeError".to_string(),
                    message: summary,
                });
            }
        }
    }

    if saw_assertion {
        Some(SingleRunOutcome::Passed)
    } else {
        Some(SingleRunOutcome::SetupOrCollectionError {
            error_type: "NoTestsMatched".to_string(),
            message: "No matching assertions executed in suite".to_string(),
        })
    }
}

/// Fallback classifier when structured XML/JSON was not written (e.g. collection crash).
/// Never promotes an unstructured crash to `AssertionFailure`.
fn fallback_classify_console(stdout: &[u8], stderr: &[u8], is_success: bool) -> SingleRunOutcome {
    if is_success {
        return SingleRunOutcome::Passed;
    }

    let combined = format!(
        "{}\n{}",
        String::from_utf8_lossy(stdout),
        String::from_utf8_lossy(stderr)
    );

    if let Some(ext) = detect_external_dep(&combined) {
        return SingleRunOutcome::ExternalDependency { reason: ext };
    }
    if is_import_error_text(&combined) {
        return SingleRunOutcome::MissingSymbol {
            error_type: classify_import_type(&combined),
            message: first_matching_line(
                &combined,
                &["ImportError", "ModuleNotFoundError", "Cannot find module"],
            ),
        };
    }

    SingleRunOutcome::SetupOrCollectionError {
        error_type: "RunnerCrash".to_string(),
        message: first_nonempty_line(&combined),
    }
}

fn detect_external_dep(text: &str) -> Option<String> {
    let keywords = [
        "No module named pytest",
        "ConnectionRefusedError",
        "DatabaseError",
        "ENOTFOUND",
        "ECONNREFUSED",
        "ETIMEDOUT",
        "NetworkError",
    ];
    for kw in keywords {
        if text.contains(kw) {
            return Some(format!("External environment requirement: {}", kw));
        }
    }
    None
}

fn is_import_error_text(text: &str) -> bool {
    text.contains("ModuleNotFoundError")
        || text.contains("ImportError")
        || text.contains("Cannot find module")
        || text.contains("cannot find package")
}

fn classify_import_type(text: &str) -> String {
    if text.contains("ModuleNotFoundError") {
        "ModuleNotFoundError".to_string()
    } else if text.contains("ImportError") {
        "ImportError".to_string()
    } else {
        "CannotFindModule".to_string()
    }
}

fn extract_xml_tag(xml: &str, tag: &str) -> Option<(String, String)> {
    let open = format!("<{}", tag);
    let start = xml.find(&open)?;
    let after_open = &xml[start + open.len()..];
    let close_bracket = after_open.find('>')?;

    // Check self-closing <tag .../>
    if close_bracket > 0 && after_open.as_bytes()[close_bracket - 1] == b'/' {
        let attrs = after_open[..close_bracket - 1].to_string();
        return Some((attrs, String::new()));
    }

    let attrs = after_open[..close_bracket].to_string();
    let rest = &after_open[close_bracket + 1..];
    let end_tag = format!("</{}>", tag);
    let end = rest.find(&end_tag)?;
    let body = rest[..end].to_string();
    Some((attrs, body))
}

fn extract_xml_attr(attrs: &str, key: &str) -> Option<String> {
    let needle_dq = format!("{}=\"", key);
    if let Some(pos) = attrs.find(&needle_dq) {
        let rest = &attrs[pos + needle_dq.len()..];
        if let Some(end) = rest.find('"') {
            return Some(rest[..end].to_string());
        }
    }
    let needle_sq = format!("{}='", key);
    if let Some(pos) = attrs.find(&needle_sq) {
        let rest = &attrs[pos + needle_sq.len()..];
        if let Some(end) = rest.find('\'') {
            return Some(rest[..end].to_string());
        }
    }
    None
}

fn decode_xml_entities(s: &str) -> String {
    s.replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}

fn first_nonempty_line(s: &str) -> String {
    s.lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or("No output")
        .chars()
        .take(200)
        .collect()
}

fn first_matching_line(s: &str, needles: &[&str]) -> String {
    s.lines()
        .map(str::trim)
        .find(|l| needles.iter().any(|n| l.contains(n)))
        .map(|l| l.chars().take(200).collect())
        .unwrap_or_else(|| first_nonempty_line(s))
}

fn escape_regex(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    for ch in s.chars() {
        if matches!(
            ch,
            '\\' | '.' | '+' | '*' | '?' | '(' | ')' | '|' | '[' | ']' | '{' | '}' | '^' | '$'
        ) {
            out.push('\\');
        }
        out.push(ch);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_junit_assertion_failure() {
        let xml = r#"<?xml version="1.0" encoding="utf-8"?>
<testsuite name="pytest" tests="1" failures="1" errors="0">
  <testcase classname="tests.test_math" name="test_add" time="0.002">
    <failure type="AssertionError" message="assert 4 == 5">AssertionError: assert 4 == 5</failure>
  </testcase>
</testsuite>"#;
        match parse_junit_xml(xml, false) {
            Some(SingleRunOutcome::AssertionFailure {
                error_type,
                message,
            }) => {
                assert_eq!(error_type, "AssertionError");
                assert!(message.contains("assert 4 == 5"));
            }
            other => panic!("Expected AssertionFailure, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_junit_type_error_not_credited_as_assertion() {
        let xml = r#"<testsuite tests="1" failures="1">
  <testcase name="test_calc">
    <failure type="TypeError" message="unsupported operand type(s)">TypeError</failure>
  </testcase>
</testsuite>"#;
        match parse_junit_xml(xml, false) {
            Some(SingleRunOutcome::OtherFailure { error_type, .. }) => {
                assert_eq!(error_type, "TypeError");
            }
            other => panic!("Expected OtherFailure, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_junit_fixture_error_is_setup_crash() {
        let xml = r#"<testsuite tests="1" errors="1">
  <testcase name="test_db">
    <error type="FixtureLookupError" message="fixture 'db' not found">error</error>
  </testcase>
</testsuite>"#;
        match parse_junit_xml(xml, false) {
            Some(SingleRunOutcome::SetupOrCollectionError { error_type, .. }) => {
                assert_eq!(error_type, "FixtureLookupError");
            }
            other => panic!("Expected SetupOrCollectionError, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_junit_collection_import_error_is_missing_symbol() {
        let xml = r#"<testsuite tests="1" errors="1">
  <testcase name="test_new">
    <error message="collection failure">ModuleNotFoundError: No module named 'app.new_mod'</error>
  </testcase>
</testsuite>"#;
        match parse_junit_xml(xml, false) {
            Some(SingleRunOutcome::MissingSymbol { error_type, .. }) => {
                assert_eq!(error_type, "ModuleNotFoundError");
            }
            other => panic!("Expected MissingSymbol, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_jest_json_matcher_failure() {
        let json = r#"{
            "success": false,
            "testResults": [{
                "status": "failed",
                "assertionResults": [{
                    "status": "failed",
                    "title": "adds numbers",
                    "failureMessages": ["Error: expect(received).toBe(expected)\nExpected: 5\nReceived: -1"],
                    "failureDetails": [{"matcherResult": {"pass": false}}]
                }]
            }]
        }"#;
        match parse_jest_vitest_json(json) {
            Some(SingleRunOutcome::AssertionFailure {
                error_type,
                message,
            }) => {
                assert_eq!(error_type, "AssertionError");
                assert!(message.contains("expect(received)"));
            }
            other => panic!("Expected AssertionFailure, got {:?}", other),
        }
    }

    #[test]
    fn test_dependency_manifest_detection() {
        assert_eq!(
            find_changed_dependency_manifest(&["src/app.py".into(), "package-lock.json".into()]),
            Some("package-lock.json".into())
        );
        assert_eq!(
            find_changed_dependency_manifest(&["requirements-dev.txt".into()]),
            Some("requirements-dev.txt".into())
        );
        assert_eq!(
            find_changed_dependency_manifest(&["src/app.py".into(), "tests/test_app.py".into()]),
            None
        );
    }

    #[test]
    fn test_is_test_or_support_file() {
        assert!(is_test_or_support_file("tests/conftest.py"));
        assert!(is_test_or_support_file("src/__snapshots__/comp.snap"));
        assert!(is_test_or_support_file("tests/fixtures/payload.json"));
        assert!(!is_test_or_support_file("src/billing/charge.py"));
    }

    #[test]
    fn test_timeout_kills_hanging_process() {
        let mut cmd = Command::new("python3");
        cmd.args(["-c", "import time; time.sleep(10)"]);
        let out = run_command_with_timeout(&mut cmd, Duration::from_millis(150)).unwrap();
        assert!(out.timed_out);
        assert!(!out.success);
    }
}
