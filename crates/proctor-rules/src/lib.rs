//! Pure integrity rules, assertion lattice evaluation, and CI sabotage guards for agent-proctor.

use proctor_core::*;
use proctor_diff::SemanticDiff;
use std::collections::HashSet;

/// Evaluates all integrity rules against the semantic diff and policy.
pub fn evaluate_rules(
    diff: &SemanticDiff,
    policy: &Policy,
    accepted_waivers: &HashSet<String>,
) -> Vec<Finding> {
    let mut findings = Vec::new();

    // 1. PROCTOR-001: Assertion Lock & Deletion & Anti-Drift
    evaluate_assertion_lock(diff, &mut findings);

    // 2. PROCTOR-002: Shadow Mocks
    evaluate_shadow_mocks(diff, policy, &mut findings);

    // 3. PROCTOR-003: CI Workflow Sabotage
    if policy.enforce_ci_tampering {
        evaluate_ci_sabotage(diff, &mut findings);
    }

    // 4. PROCTOR-004: Runner Config Tamper
    evaluate_runner_config(diff, &mut findings);

    // 5. PROCTOR-005: Snapshot Integrity
    evaluate_snapshot_integrity(diff, &mut findings);

    // 6. PROCTOR-006: Parametrize Row Drops
    evaluate_parametrize_rows(diff, &mut findings);

    // Filter out findings accepted by an authorized human reviewer via waivers
    findings.retain(|f| !accepted_waivers.contains(&f.id));

    findings
}

/// PROCTOR-001: Checks for assertion deletions, downgrades, and un-orphaned test removals.
fn evaluate_assertion_lock(diff: &SemanticDiff, findings: &mut Vec<Finding>) {
    // 1a. Paired test assertion analysis
    for pair in &diff.paired_tests {
        let base_test = &pair.base_test;
        let head_test = &pair.head_test;
        let test_name = head_test.id.canonical_name();

        let base_exact = base_test
            .assertions
            .iter()
            .filter(|a| a.strength == AssertionStrength::Exact)
            .count();
        let head_exact = head_test
            .assertions
            .iter()
            .filter(|a| a.strength == AssertionStrength::Exact)
            .count();

        let base_total = base_test.assertions.len();
        let head_total = head_test.assertions.len();

        // Check if exact assertions or total assertions dropped
        if head_exact < base_exact || head_total < base_total {
            let head_texts: HashSet<&str> =
                head_test.assertions.iter().map(|a| a.text.as_str()).collect();

            for base_assert in &base_test.assertions {
                if !head_texts.contains(base_assert.text.as_str()) {
                    // Check if this missing/weakened assertion is justified by an orphaned symbol
                    let justified = diff.is_reference_orphaned(&base_assert.referenced_symbols);

                    if !justified {
                        let id = Finding::compute_id(
                            RuleId::AssertionLock,
                            &test_name,
                            &base_assert.text,
                        );

                        // Check linkage: did any production source file change?
                        let has_production_changes = !diff.head_sources.is_empty();

                        let (verdict, message) = if has_production_changes {
                            (
                                Verdict::Review {
                                    finding_id: id.clone(),
                                    evidence: format!(
                                        "Assertion weakened/dropped: '{}'. Target symbol not deleted.",
                                        base_assert.text
                                    ),
                                },
                                format!(
                                    "Assertion weakened or removed in test '{}' without orphaned symbol justification. Requires reviewer waiver.",
                                    test_name
                                ),
                            )
                        } else {
                            (
                                Verdict::Block {
                                    rule: RuleId::AssertionLock,
                                    evidence: format!(
                                        "Assertion deleted with zero production code changes: '{}'",
                                        base_assert.text
                                    ),
                                },
                                format!(
                                    "Assertion deleted in test '{}' while production code was untouched.",
                                    test_name
                                ),
                            )
                        };

                        findings.push(Finding {
                            id,
                            rule_id: RuleId::AssertionLock,
                            verdict,
                            file_path: head_test.id.file_path.clone(),
                            line_number: Some(base_assert.line),
                            test_name: Some(test_name.clone()),
                            message,
                            evidence: base_assert.text.clone(),
                        });
                    }
                }
            }
        }

        // 1b. Anti-Drift (Mirror Check): Check if modified expected literal was mirrored in production code
        for head_assert in &head_test.assertions {
            if let Some(expected_lit) = &head_assert.expected_literal {
                // If this is a new literal not present in base test assertions
                let in_base_asserts = base_test
                    .assertions
                    .iter()
                    .any(|a| a.expected_literal.as_deref() == Some(expected_lit));

                if !in_base_asserts {
                    // Check if it was newly added as a literal in any head source file
                    for (src_path, head_src) in &diff.head_sources {
                        let base_src_lits = diff
                            .base_sources
                            .get(src_path)
                            .map(|s| &s.string_literals)
                            .cloned()
                            .unwrap_or_default();

                        if head_src.string_literals.contains(expected_lit)
                            && !base_src_lits.contains(expected_lit)
                        {
                            let id = Finding::compute_id(
                                RuleId::AssertionLock,
                                &test_name,
                                expected_lit,
                            );
                            findings.push(Finding {
                                id: id.clone(),
                                rule_id: RuleId::AssertionLock,
                                verdict: Verdict::Review {
                                    finding_id: id,
                                    evidence: format!(
                                        "Expected literal '{}' shifted and mirrors newly added production literal in {}",
                                        expected_lit, src_path
                                    ),
                                },
                                file_path: head_test.id.file_path.clone(),
                                line_number: Some(head_assert.line),
                                test_name: Some(test_name.clone()),
                                message: format!(
                                    "Test expected value '{}' shifted to mirror newly added production constant.",
                                    expected_lit
                                ),
                                evidence: head_assert.text.clone(),
                            });
                        }
                    }
                }
            }
        }
    }

    // 1c. Whole test deletions
    for del_test in &diff.deleted_tests {
        let test_name = del_test.id.canonical_name();
        let justified = del_test
            .assertions
            .iter()
            .any(|a| diff.is_reference_orphaned(&a.referenced_symbols));

        if !justified {
            let id = Finding::compute_id(RuleId::AssertionLock, &test_name, "test_deleted");
            findings.push(Finding {
                id,
                rule_id: RuleId::AssertionLock,
                verdict: Verdict::Block {
                    rule: RuleId::AssertionLock,
                    evidence: format!(
                        "Test '{}' deleted while target symbols still exist in repository",
                        test_name
                    ),
                },
                file_path: del_test.id.file_path.clone(),
                line_number: None,
                test_name: Some(test_name.clone()),
                message: format!(
                    "Test '{}' was removed entirely without deleting the underlying production symbol.",
                    test_name
                ),
                evidence: format!("Deleted test: {}", test_name),
            });
        }
    }
}

/// PROCTOR-002: Detects mocks added to in-repo production code.
fn evaluate_shadow_mocks(diff: &SemanticDiff, policy: &Policy, findings: &mut Vec<Finding>) {
    // Collect all in-repo defined production symbols and modules
    let mut in_repo_symbols = HashSet::new();
    for (path, src) in &diff.head_sources {
        for sym in &src.defined_symbols {
            in_repo_symbols.insert(sym.to_lowercase());
        }
        // Also add the module path (e.g. "app.services.user")
        let mod_path = path
            .trim_end_matches(".py")
            .trim_end_matches(".js")
            .trim_end_matches(".ts")
            .replace('/', ".");
        in_repo_symbols.insert(mod_path.to_lowercase());
    }

    for pair in &diff.paired_tests {
        let base_mock_targets: HashSet<&str> =
            pair.base_test.mocks.iter().map(|m| m.target.as_str()).collect();

        for mock in &pair.head_test.mocks {
            if !base_mock_targets.contains(mock.target.as_str()) {
                // New mock added in head
                let target_lower = mock.target.to_lowercase();

                // Check against policy allowlist
                let allowed = policy.allow_mocks.iter().any(|pattern| {
                    if pattern.ends_with('*') {
                        target_lower.starts_with(pattern.trim_end_matches('*'))
                    } else {
                        target_lower == *pattern
                    }
                });

                if !allowed {
                    // Check if it matches an in-repo symbol or module
                    let is_in_repo = in_repo_symbols.iter().any(|sym| {
                        target_lower.contains(sym.as_str()) || sym.contains(target_lower.as_str())
                    });

                    if is_in_repo {
                        let test_name = pair.head_test.id.canonical_name();
                        let id = Finding::compute_id(RuleId::ShadowMock, &test_name, &mock.target);
                        findings.push(Finding {
                            id,
                            rule_id: RuleId::ShadowMock,
                            verdict: Verdict::Block {
                                rule: RuleId::ShadowMock,
                                evidence: format!(
                                    "Shadow mock targeting in-repo production symbol: '{}'",
                                    mock.target
                                ),
                            },
                            file_path: pair.head_test.id.file_path.clone(),
                            line_number: Some(mock.line),
                            test_name: Some(test_name.clone()),
                            message: format!(
                                "Newly added mock in '{}' targets in-repo internal code '{}' instead of external boundary.",
                                test_name, mock.target
                            ),
                            evidence: format!("mock target: {}", mock.target),
                        });
                    }
                }
            }
        }
    }
}

/// PROCTOR-003: Detects sabotage in CI workflow files.
fn evaluate_ci_sabotage(diff: &SemanticDiff, findings: &mut Vec<Finding>) {
    let workflow_patterns = [".github/workflows", ".gitlab-ci.yml", ".circleci/config.yml"];

    for (head_path, head_content) in &diff.raw_head_files {
        let is_ci = workflow_patterns.iter().any(|pat| head_path.contains(pat));
        if !is_ci {
            continue;
        }

        let base_content = diff.raw_base_files.get(head_path).map(|s| s.as_str()).unwrap_or("");

        // Check for continue-on-error: true added
        if head_content.contains("continue-on-error: true")
            && !base_content.contains("continue-on-error: true")
        {
            let id = Finding::compute_id(RuleId::CiWorkflowSabotage, head_path, "continue-on-error");
            findings.push(Finding {
                id,
                rule_id: RuleId::CiWorkflowSabotage,
                verdict: Verdict::Block {
                    rule: RuleId::CiWorkflowSabotage,
                    evidence: "Injected 'continue-on-error: true' into CI workflow".into(),
                },
                file_path: head_path.clone(),
                line_number: None,
                test_name: None,
                message: "CI workflow modified to ignore step failures (continue-on-error: true)."
                    .into(),
                evidence: "continue-on-error: true".into(),
            });
        }

        // Check for shell cheats
        let shell_cheats = ["|| true", "|| exit 0", "set +e", "|| echo "];
        for cheat in &shell_cheats {
            if head_content.contains(cheat) && !base_content.contains(cheat) {
                let id = Finding::compute_id(RuleId::CiWorkflowSabotage, head_path, cheat);
                findings.push(Finding {
                    id,
                    rule_id: RuleId::CiWorkflowSabotage,
                    verdict: Verdict::Block {
                        rule: RuleId::CiWorkflowSabotage,
                        evidence: format!("Injected shell suppression '{}' into CI workflow", cheat),
                    },
                    file_path: head_path.clone(),
                    line_number: None,
                    test_name: None,
                    message: format!(
                        "CI workflow sabotaged with shell error masking: '{}'",
                        cheat
                    ),
                    evidence: (*cheat).to_string(),
                });
            }
        }
    }
}

/// PROCTOR-004: Detects weakening of test runner configurations.
fn evaluate_runner_config(diff: &SemanticDiff, findings: &mut Vec<Finding>) {
    let runner_files = [
        "pytest.ini",
        "pyproject.toml",
        "setup.cfg",
        "jest.config.js",
        "jest.config.ts",
        "jest.config.json",
        "package.json",
    ];

    for (head_path, head_content) in &diff.raw_head_files {
        let is_runner_file = runner_files.iter().any(|f| head_path.ends_with(f));
        if !is_runner_file {
            continue;
        }

        let base_content = diff.raw_base_files.get(head_path).map(|s| s.as_str()).unwrap_or("");

        // Pytest weakening
        let pytest_tampering = ["--ignore", "--deselect", "-m \"not", "--continue-on-collection-errors"];
        for flag in &pytest_tampering {
            if head_content.contains(flag) && !base_content.contains(flag) {
                let id = Finding::compute_id(RuleId::RunnerConfigTamper, head_path, flag);
                findings.push(Finding {
                    id,
                    rule_id: RuleId::RunnerConfigTamper,
                    verdict: Verdict::Block {
                        rule: RuleId::RunnerConfigTamper,
                        evidence: format!("Test runner flag '{}' added to ignore tests", flag),
                    },
                    file_path: head_path.clone(),
                    line_number: None,
                    test_name: None,
                    message: format!(
                        "Test runner config weakened with test suppression flag '{}'",
                        flag
                    ),
                    evidence: (*flag).to_string(),
                });
            }
        }
    }
}

/// PROCTOR-005: Detects deleted or severely shrunken test snapshot files.
fn evaluate_snapshot_integrity(diff: &SemanticDiff, findings: &mut Vec<Finding>) {
    for (base_path, base_content) in &diff.raw_base_files {
        if base_path.ends_with(".snap") || base_path.contains("__snapshots__") {
            if let Some(head_content) = diff.raw_head_files.get(base_path) {
                // Check if snapshot was truncated by > 50%
                let base_lines = base_content.lines().count();
                let head_lines = head_content.lines().count();
                if base_lines > 10 && head_lines < (base_lines / 2) {
                    let id = Finding::compute_id(
                        RuleId::SnapshotIntegrity,
                        base_path,
                        "truncated_snapshot",
                    );
                    findings.push(Finding {
                        id,
                        rule_id: RuleId::SnapshotIntegrity,
                        verdict: Verdict::Block {
                            rule: RuleId::SnapshotIntegrity,
                            evidence: format!(
                                "Snapshot '{}' shrunk by >50% ({} lines -> {} lines)",
                                base_path, base_lines, head_lines
                            ),
                        },
                        file_path: base_path.clone(),
                        line_number: None,
                        test_name: None,
                        message: "Snapshot file was heavily truncated to pass assertions.".into(),
                        evidence: format!("{} lines down to {}", base_lines, head_lines),
                    });
                }
            } else {
                // Snapshot file was deleted
                let id = Finding::compute_id(RuleId::SnapshotIntegrity, base_path, "deleted_snapshot");
                findings.push(Finding {
                    id,
                    rule_id: RuleId::SnapshotIntegrity,
                    verdict: Verdict::Block {
                        rule: RuleId::SnapshotIntegrity,
                        evidence: format!("Snapshot file '{}' deleted", base_path),
                    },
                    file_path: base_path.clone(),
                    line_number: None,
                    test_name: None,
                    message: "Snapshot file deleted without deleting corresponding test.".into(),
                    evidence: "Deleted snapshot".into(),
                });
            }
        }
    }
}

/// PROCTOR-006: Detects removed test rows from parametrized test suites.
fn evaluate_parametrize_rows(diff: &SemanticDiff, findings: &mut Vec<Finding>) {
    for pair in &diff.paired_tests {
        let base_rows = pair.base_test.parametrized_hashes.len();
        let head_rows = pair.head_test.parametrized_hashes.len();

        if base_rows > 0 && head_rows < base_rows {
            let test_name = pair.head_test.id.canonical_name();
            let id = Finding::compute_id(
                RuleId::ParametrizeRowDrop,
                &test_name,
                &format!("{}_to_{}", base_rows, head_rows),
            );

            findings.push(Finding {
                id,
                rule_id: RuleId::ParametrizeRowDrop,
                verdict: Verdict::Block {
                    rule: RuleId::ParametrizeRowDrop,
                    evidence: format!(
                        "Parametrized test rows dropped from {} to {}",
                        base_rows, head_rows
                    ),
                },
                file_path: pair.head_test.id.file_path.clone(),
                line_number: None,
                test_name: Some(test_name.clone()),
                message: format!(
                    "Parametrized test row count reduced from {} to {} in '{}'.",
                    base_rows, head_rows, test_name
                ),
                evidence: format!("Rows: {} -> {}", base_rows, head_rows),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_assertion_weakening_blocks_when_no_production_change() {
        let base_test = r#"
def test_calc():
    assert calculate(10) == 100
"#;
        let head_test = r#"
def test_calc():
    assert calculate(10) is not None
"#;

        let mut base_files = HashMap::new();
        base_files.insert("tests/test_calc.py".to_string(), base_test.to_string());

        let mut head_files = HashMap::new();
        head_files.insert("tests/test_calc.py".to_string(), head_test.to_string());

        let diff = SemanticDiff::build(&base_files, &head_files).unwrap();
        let policy = Policy::default();
        let waivers = HashSet::new();

        let findings = evaluate_rules(&diff, &policy, &waivers);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].rule_id, RuleId::AssertionLock);
        matches!(findings[0].verdict, Verdict::Block { .. });
    }

    #[test]
    fn test_assertion_weakening_justified_by_orphaned_symbol() {
        let base_test = r#"
def test_user():
    assert user.old_token == "secret"
"#;
        let head_test = r#"
def test_user():
    pass
"#;
        let base_src = r#"
class User:
    old_token = "secret"
"#;
        let head_src = r#"
class User:
    pass
"#;

        let mut base_files = HashMap::new();
        base_files.insert("tests/test_user.py".to_string(), base_test.to_string());
        base_files.insert("app/user.py".to_string(), base_src.to_string());

        let mut head_files = HashMap::new();
        head_files.insert("tests/test_user.py".to_string(), head_test.to_string());
        head_files.insert("app/user.py".to_string(), head_src.to_string());

        let diff = SemanticDiff::build(&base_files, &head_files).unwrap();
        let policy = Policy::default();
        let waivers = HashSet::new();

        let findings = evaluate_rules(&diff, &policy, &waivers);
        assert_eq!(findings.len(), 0); // Clean pass because old_token was deleted!
    }

    #[test]
    fn test_ci_sabotage_detection() {
        let mut base_files = HashMap::new();
        base_files.insert(
            ".github/workflows/ci.yml".to_string(),
            "steps:\n  - run: pytest\n".to_string(),
        );

        let mut head_files = HashMap::new();
        head_files.insert(
            ".github/workflows/ci.yml".to_string(),
            "steps:\n  - run: pytest || true\n".to_string(),
        );

        let diff = SemanticDiff::build(&base_files, &head_files).unwrap();
        let policy = Policy::default();
        let waivers = HashSet::new();

        let findings = evaluate_rules(&diff, &policy, &waivers);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].rule_id, RuleId::CiWorkflowSabotage);
    }

    #[test]
    fn test_shadow_mock_detection() {
        let base_test = r#"
def test_checkout():
    assert checkout() == 200
"#;
        let head_test = r#"
@patch("app.billing.charge_card")
def test_checkout(mock_charge):
    assert checkout() == 200
"#;
        let base_src = r#"
def charge_card():
    return True
"#;
        let head_src = base_src;

        let mut base_files = HashMap::new();
        base_files.insert("tests/test_checkout.py".to_string(), base_test.to_string());
        base_files.insert("app/billing.py".to_string(), base_src.to_string());

        let mut head_files = HashMap::new();
        head_files.insert("tests/test_checkout.py".to_string(), head_test.to_string());
        head_files.insert("app/billing.py".to_string(), head_src.to_string());

        let diff = SemanticDiff::build(&base_files, &head_files).unwrap();
        let policy = Policy::default();
        let waivers = HashSet::new();

        let findings = evaluate_rules(&diff, &policy, &waivers);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].rule_id, RuleId::ShadowMock);
        matches!(findings[0].verdict, Verdict::Block { .. });
    }

    #[test]
    fn test_parametrize_row_drop() {
        let base_test = r#"
@pytest.mark.parametrize("x,expected", [(1, 2), (2, 4), (-1, -2)])
def test_double(x, expected):
    assert double(x) == expected
"#;
        let head_test = r#"
@pytest.mark.parametrize("x,expected", [(1, 2), (2, 4)])
def test_double(x, expected):
    assert double(x) == expected
"#;

        let mut base_files = HashMap::new();
        base_files.insert("tests/test_math.py".to_string(), base_test.to_string());

        let mut head_files = HashMap::new();
        head_files.insert("tests/test_math.py".to_string(), head_test.to_string());

        let diff = SemanticDiff::build(&base_files, &head_files).unwrap();
        let policy = Policy::default();
        let waivers = HashSet::new();

        let findings = evaluate_rules(&diff, &policy, &waivers);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].rule_id, RuleId::ParametrizeRowDrop);
    }

    #[test]
    fn test_runner_config_tamper() {
        let mut base_files = HashMap::new();
        base_files.insert(
            "pytest.ini".to_string(),
            "[pytest]\naddopts = -ra\n".to_string(),
        );

        let mut head_files = HashMap::new();
        head_files.insert(
            "pytest.ini".to_string(),
            "[pytest]\naddopts = -ra --ignore=tests/failing_test.py\n".to_string(),
        );

        let diff = SemanticDiff::build(&base_files, &head_files).unwrap();
        let policy = Policy::default();
        let waivers = HashSet::new();

        let findings = evaluate_rules(&diff, &policy, &waivers);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].rule_id, RuleId::RunnerConfigTamper);
    }

    #[test]
    fn test_waiver_bypass() {
        let base_test = r#"
def test_calc():
    assert calculate(10) == 100
"#;
        let head_test = r#"
def test_calc():
    assert calculate(10) is not None
"#;

        let mut base_files = HashMap::new();
        base_files.insert("tests/test_calc.py".to_string(), base_test.to_string());

        let mut head_files = HashMap::new();
        head_files.insert("tests/test_calc.py".to_string(), head_test.to_string());

        let diff = SemanticDiff::build(&base_files, &head_files).unwrap();
        let policy = Policy::default();

        // Compute finding id first without waivers
        let empty_waivers = HashSet::new();
        let unverified_findings = evaluate_rules(&diff, &policy, &empty_waivers);
        assert_eq!(unverified_findings.len(), 1);
        let finding_id = unverified_findings[0].id.clone();

        // Now provide waiver
        let mut waivers = HashSet::new();
        waivers.insert(finding_id);

        let waived_findings = evaluate_rules(&diff, &policy, &waivers);
        assert_eq!(waived_findings.len(), 0);
    }
}
