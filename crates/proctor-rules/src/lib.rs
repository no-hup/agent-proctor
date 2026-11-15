//! Pure integrity rules, assertion lattice evaluation, and CI sabotage guards for agent-proctor.

use proctor_core::*;
use proctor_diff::SemanticDiff;
use std::collections::HashSet;
use std::path::Path;

/// Evaluates all integrity rules against the semantic diff and policy.
pub fn evaluate_rules(
    diff: &SemanticDiff,
    policy: &Policy,
    waivers: &WaiverSet,
) -> Vec<Finding> {
    let mut findings = Vec::new();

    // 1. PROCTOR-007: Policy Tampering (Cannot modify proctor.toml in PR)
    evaluate_policy_tamper(diff, &mut findings);

    // 2. Parse Errors: Fail-closed on corrupted files or Tree-sitter failures
    evaluate_parse_errors(diff, &mut findings);

    // 3. PROCTOR-001: Assertion Lock & Deletion & Anti-Drift & Skip/Xfail
    evaluate_assertion_lock(diff, policy, &mut findings);

    // 4. PROCTOR-002: Shadow Mocks (including harness mocks and in-repo paths)
    evaluate_shadow_mocks(diff, policy, &mut findings);

    // 5. PROCTOR-003: CI Workflow Sabotage
    if policy.enforce_ci_tampering {
        evaluate_ci_sabotage(diff, &mut findings);
    }

    // 6. PROCTOR-004: Runner Config Tamper
    evaluate_runner_config(diff, &mut findings);

    // 7. PROCTOR-005: Snapshot Integrity
    evaluate_snapshot_integrity(diff, &mut findings);

    // 8. PROCTOR-006: Parametrize Row Drops
    evaluate_parametrize_rows(diff, &mut findings);

    // Filter out findings accepted by an authorized human reviewer via waivers / acks.
    // Guard findings require maintainer_waivers; author_acks only suppress Report tier.
    findings.retain(|f| !waivers.is_suppressed(f));

    findings
}

/// PROCTOR-007: Guard against modifying or adding policy files in the PR diff.
fn evaluate_policy_tamper(diff: &SemanticDiff, findings: &mut Vec<Finding>) {
    let policy_files = ["proctor.toml", ".proctor.toml"];
    for file in &policy_files {
        let in_head = diff.raw_head_files.contains_key(*file);
        let in_base = diff.raw_base_files.contains_key(*file);

        if in_head != in_base {
            let id = Finding::compute_id(RuleId::PolicyTamper, file, "policy_file_status_changed");
            findings.push(Finding {
                id,
                rule_id: RuleId::PolicyTamper,
                tier: RuleTier::Guard,
                verdict: Verdict::Block {
                    rule: RuleId::PolicyTamper,
                    evidence: format!("Policy file '{}' was added or deleted in PR diff", file),
                },
                file_path: (*file).to_string(),
                line_number: None,
                test_name: None,
                message: format!(
                    "Policy file '{}' cannot be added or deleted in a pull request diff.",
                    file
                ),
                evidence: format!("Diff touches {}", file),
            });
        } else if in_head && in_base {
            let base_content = &diff.raw_base_files[*file];
            let head_content = &diff.raw_head_files[*file];
            if base_content != head_content {
                let id = Finding::compute_id(RuleId::PolicyTamper, file, "policy_content_modified");
                findings.push(Finding {
                    id,
                    rule_id: RuleId::PolicyTamper,
                    tier: RuleTier::Guard,
                    verdict: Verdict::Block {
                        rule: RuleId::PolicyTamper,
                        evidence: format!("Policy file '{}' was modified in PR diff", file),
                    },
                    file_path: (*file).to_string(),
                    line_number: None,
                    test_name: None,
                    message: format!(
                        "Policy file '{}' cannot be modified in a pull request diff without authorized waiver.",
                        file
                    ),
                    evidence: format!("Diff modifies {}", file),
                });
            }
        }
    }
}

/// Fail-closed reporting on Tree-sitter parse or syntax errors.
fn evaluate_parse_errors(diff: &SemanticDiff, findings: &mut Vec<Finding>) {
    for (path, err_msg) in &diff.parse_errors {
        let id = Finding::compute_id(RuleId::AssertionLock, path, "parse_error");
        findings.push(Finding {
            id: id.clone(),
            rule_id: RuleId::AssertionLock,
            tier: RuleTier::Report,
            verdict: Verdict::Review {
                finding_id: id,
                evidence: format!("Tree-sitter parse error in {}: {}", path, err_msg),
            },
            file_path: path.clone(),
            line_number: None,
            test_name: None,
            message: format!(
                "File '{}' could not be parsed safely ({}); review required.",
                path, err_msg
            ),
            evidence: err_msg.clone(),
        });
    }
}

/// PROCTOR-001: Checks for assertion deletions, downgrades, skips, and un-orphaned test removals.
fn evaluate_assertion_lock(diff: &SemanticDiff, policy: &Policy, findings: &mut Vec<Finding>) {
    // 1a. Paired test assertion analysis
    for pair in &diff.paired_tests {
        let base_test = &pair.base_test;
        let head_test = &pair.head_test;
        let test_name = head_test.id.canonical_name();

        // 1a(i). Check for skip / xfail injection
        if !base_test.is_skipped && head_test.is_skipped {
            let id = Finding::compute_id(RuleId::AssertionLock, &test_name, "test_skipped");

            // Check if production code modified in this PR intersects with the test's subject
            let modified_prod_symbols: HashSet<&str> = diff
                .head_sources
                .values()
                .flat_map(|s| s.defined_symbols.iter().map(|sym| sym.as_str()))
                .collect();

            let test_targets_modified_prod = base_test.assertions.iter().any(|a| {
                a.referenced_symbols
                    .iter()
                    .any(|sym| modified_prod_symbols.contains(sym.as_str()))
            });

            let (verdict, tier, message) = if !diff.head_sources.is_empty()
                && (test_targets_modified_prod || !policy.skip_demote_to_review)
            {
                (
                    Verdict::Block {
                        rule: RuleId::AssertionLock,
                        evidence: format!(
                            "Test '{}' exercising modified production code was skipped/xfailed",
                            test_name
                        ),
                    },
                    RuleTier::Guard,
                    format!(
                        "Test '{}' tests modified production code but was marked as skipped (@pytest.mark.skip / it.skip) in head.",
                        test_name
                    ),
                )
            } else {
                (
                    Verdict::Review {
                        finding_id: id.clone(),
                        evidence: format!(
                            "Test '{}' was skipped/xfailed. Requires reviewer acceptance to confirm quarantine.",
                            test_name
                        ),
                    },
                    RuleTier::Report,
                    format!(
                        "Test '{}' marked as skipped (@pytest.mark.skip / it.skip). Requires reviewer waiver to confirm quarantine.",
                        test_name
                    ),
                )
            };

            findings.push(Finding {
                id,
                rule_id: RuleId::AssertionLock,
                tier,
                verdict,
                file_path: head_test.id.file_path.clone(),
                line_number: None,
                test_name: Some(test_name.clone()),
                message,
                evidence: "Skipped / xfailed test".into(),
            });
        }

        // 1a(ii). Check for modified, weakened, or deleted assertions via AssertionStrength lattice
        let head_texts: HashSet<&str> =
            head_test.assertions.iter().map(|a| a.text.as_str()).collect();
        let head_max_strength = head_test.assertions.iter().map(|a| a.strength).max();

        for base_assert in &base_test.assertions {
            if !head_texts.contains(base_assert.text.as_str()) {
                // This assertion was modified or removed
                let justified = diff.is_reference_orphaned(&base_assert.referenced_symbols);

                if !justified {
                    let id = Finding::compute_id(
                        RuleId::AssertionLock,
                        &test_name,
                        &base_assert.text,
                    );

                    let was_weakened = match head_max_strength {
                        Some(max_s) => max_s < base_assert.strength,
                        None => true,
                    };

                    // Check linkage: did any production source file change?
                    let has_production_changes = !diff.head_sources.is_empty();

                    let (verdict, tier, message) = if has_production_changes {
                        let msg = if was_weakened {
                            format!(
                                "Assertion weakened in test '{}' ({:?} -> {:?}) without orphaned symbol justification.",
                                test_name, base_assert.strength, head_max_strength
                            )
                        } else {
                            format!(
                                "Assertion modified in test '{}' without orphaned symbol justification. Requires reviewer waiver.",
                                test_name
                            )
                        };
                        (
                            Verdict::Review {
                                finding_id: id.clone(),
                                evidence: format!(
                                    "Assertion modified/dropped: '{}'. Target symbol not deleted.",
                                    base_assert.text
                                ),
                            },
                            RuleTier::Report,
                            msg,
                        )
                    } else {
                        (
                            Verdict::Block {
                                rule: RuleId::AssertionLock,
                                evidence: format!(
                                    "Assertion modified/swapped with zero production code changes: '{}'",
                                    base_assert.text
                                ),
                            },
                            RuleTier::Guard,
                            format!(
                                "Assertion modified in test '{}' while production code was untouched.",
                                test_name
                            ),
                        )
                    };

                    findings.push(Finding {
                        id,
                        rule_id: RuleId::AssertionLock,
                        tier,
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
                                id,
                                rule_id: RuleId::AssertionLock,
                                tier: RuleTier::Guard,
                                verdict: Verdict::Block {
                                    rule: RuleId::AssertionLock,
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

    // 1c. Whole test deletions (Guard tier: always BLOCK)
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
                tier: RuleTier::Guard,
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

/// Helper to test if a mock target is an in-repo symbol or file
fn is_in_repo_target(
    target: &str,
    in_repo_symbols: &HashSet<String>,
) -> bool {
    let target_lower = target.to_lowercase();
    let target_clean = target_lower
        .trim_start_matches("./")
        .trim_start_matches("../")
        .replace('/', ".");

    // 1. Check in-repo symbols from diff
    if in_repo_symbols
        .iter()
        .any(|sym| target_clean.contains(sym.as_str()) || sym.contains(target_clean.as_str()))
    {
        return true;
    }

    // 2. Relative paths are always in-repo
    if target.starts_with("./") || target.starts_with("../") {
        return true;
    }

    // 3. Check filesystem existence for target module (e.g. app.billing -> app/billing.py)
    let as_path = target_clean.replace('.', "/");
    if Path::new(&format!("{}.py", as_path)).exists()
        || Path::new(&format!("{}.ts", as_path)).exists()
        || Path::new(&format!("{}.js", as_path)).exists()
        || Path::new(&as_path).exists()
    {
        return true;
    }

    // 4. Common in-repo prefixes
    if target_clean.starts_with("app.")
        || target_clean.starts_with("src.")
        || target_clean.starts_with("lib.")
    {
        return true;
    }

    false
}

/// Pattern matching for allow_mocks: rejects bare wildcard `*` and empty patterns.
fn is_mock_allowed(target: &str, allow_mocks: &[String]) -> bool {
    let target_lower = target.to_lowercase();
    allow_mocks.iter().any(|pattern| {
        let p = pattern.trim();
        if p.is_empty() || p == "*" {
            return false; // Bare wildcard or empty pattern is rejected
        }
        if let Some(prefix) = p.strip_suffix('*') {
            if prefix.is_empty() {
                return false;
            }
            target_lower.starts_with(&prefix.to_lowercase())
        } else {
            target_lower == p.to_lowercase()
        }
    })
}

/// PROCTOR-002: Detects mocks added to in-repo production code (per-test and top-of-file harness).
fn evaluate_shadow_mocks(diff: &SemanticDiff, policy: &Policy, findings: &mut Vec<Finding>) {
    // Collect all in-repo defined production symbols and modules from diff
    let mut in_repo_symbols = HashSet::new();
    for (path, src) in &diff.head_sources {
        for sym in &src.defined_symbols {
            in_repo_symbols.insert(sym.to_lowercase());
        }
        let mod_path = path
            .trim_end_matches(".py")
            .trim_end_matches(".js")
            .trim_end_matches(".ts")
            .replace('/', ".");
        in_repo_symbols.insert(mod_path.to_lowercase());
    }

    // Check per-test mocks
    for pair in &diff.paired_tests {
        let base_mock_targets: HashSet<&str> =
            pair.base_test.mocks.iter().map(|m| m.target.as_str()).collect();

        for mock in &pair.head_test.mocks {
            if !base_mock_targets.contains(mock.target.as_str()) {
                let allowed = is_mock_allowed(&mock.target, &policy.allow_mocks);

                if !allowed && is_in_repo_target(&mock.target, &in_repo_symbols) {
                    let test_name = pair.head_test.id.canonical_name();
                    let id = Finding::compute_id(RuleId::ShadowMock, &test_name, &mock.target);

                    let target_clean = mock.target.to_lowercase();
                    let target_leaf = mock.target.split('.').next_back().unwrap_or(&mock.target).to_lowercase();
                    let is_direct_diff_target = in_repo_symbols.iter().any(|sym| {
                        target_clean.contains(sym.as_str()) || sym.contains(target_clean.as_str())
                    });
                    let is_sut_mock = pair.head_test.assertions.iter().any(|a| {
                        let a_lower = a.text.to_lowercase();
                        a_lower.contains(&target_leaf) || a_lower.contains(&target_clean)
                    });

                    let (verdict, tier) = if is_direct_diff_target || is_sut_mock || policy.strict_mode {
                        (
                            Verdict::Block {
                                rule: RuleId::ShadowMock,
                                evidence: format!(
                                    "Shadow mock targeting system under test or modified in-repo production symbol: '{}'",
                                    mock.target
                                ),
                            },
                            RuleTier::Guard,
                        )
                    } else {
                        (
                            Verdict::Review {
                                finding_id: id.clone(),
                                evidence: format!(
                                    "Mock targeting internal collaborator: '{}'. Reviewer waiver required.",
                                    mock.target
                                ),
                            },
                            RuleTier::Report,
                        )
                    };

                    findings.push(Finding {
                        id,
                        rule_id: RuleId::ShadowMock,
                        tier,
                        verdict,
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

    // Check top-of-file harness mocks (e.g. file-level `jest.mock('src.billing')`)
    let base_harness_targets: HashSet<&str> = diff
        .base_harness_mocks
        .iter()
        .map(|m| m.target.as_str())
        .collect();

    for mock in &diff.head_harness_mocks {
        if !base_harness_targets.contains(mock.target.as_str()) {
            let allowed = is_mock_allowed(&mock.target, &policy.allow_mocks);

            if !allowed && is_in_repo_target(&mock.target, &in_repo_symbols) {
                let id = Finding::compute_id(RuleId::ShadowMock, "harness_mock", &mock.target);
                let target_clean = mock.target.to_lowercase();
                let is_direct_diff_target = in_repo_symbols.iter().any(|sym| {
                    target_clean.contains(sym.as_str()) || sym.contains(target_clean.as_str())
                });

                let (verdict, tier) = if is_direct_diff_target || policy.strict_mode {
                    (
                        Verdict::Block {
                            rule: RuleId::ShadowMock,
                            evidence: format!(
                                "Top-level harness mock targeting modified in-repo code: '{}'",
                                mock.target
                            ),
                        },
                        RuleTier::Guard,
                    )
                } else {
                    (
                        Verdict::Review {
                            finding_id: id.clone(),
                            evidence: format!(
                                "Top-level harness mock targeting in-repo internal module: '{}'",
                                mock.target
                            ),
                        },
                        RuleTier::Report,
                    )
                };

                findings.push(Finding {
                    id,
                    rule_id: RuleId::ShadowMock,
                    tier,
                    verdict,
                    file_path: "test_harness".to_string(),
                    line_number: Some(mock.line),
                    test_name: None,
                    message: format!(
                        "File-level mock targets in-repo internal module '{}' instead of external boundary.",
                        mock.target
                    ),
                    evidence: format!("harness mock target: {}", mock.target),
                });
            }
        }
    }
}

/// PROCTOR-003: Diff-aware detection of sabotage in CI workflow files.
/// Only scans newly added or modified lines in workflow files.
fn evaluate_ci_sabotage(diff: &SemanticDiff, findings: &mut Vec<Finding>) {
    let workflow_patterns = [".github/workflows", ".gitlab-ci.yml", ".circleci/config.yml"];

    for (head_path, head_content) in &diff.raw_head_files {
        let is_ci = workflow_patterns.iter().any(|pat| head_path.contains(pat));
        if !is_ci {
            continue;
        }

        let base_content = diff.raw_base_files.get(head_path).map(|s| s.as_str()).unwrap_or("");
        let base_lines: HashSet<&str> = base_content.lines().map(|l| l.trim()).collect();

        let shell_cheats = [
            "|| true",
            "|| exit 0",
            "; exit 0",
            "|| :",
            "; true",
            "set +e",
            "|| echo ",
            "if: false",
            "pytest --ignore",
            "echo ok",
        ];

        for (line_idx, line) in head_content.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.is_empty() || base_lines.contains(trimmed) {
                continue; // Line was already present in base - not a new sabotage in this PR
            }

            if trimmed.contains("continue-on-error: true") {
                let id = Finding::compute_id(RuleId::CiWorkflowSabotage, head_path, "continue-on-error");
                findings.push(Finding {
                    id,
                    rule_id: RuleId::CiWorkflowSabotage,
                    tier: RuleTier::Guard,
                    verdict: Verdict::Block {
                        rule: RuleId::CiWorkflowSabotage,
                        evidence: format!("Injected 'continue-on-error: true' at line {}", line_idx + 1),
                    },
                    file_path: head_path.clone(),
                    line_number: Some(line_idx + 1),
                    test_name: None,
                    message: "CI workflow modified to ignore step failures (continue-on-error: true)."
                        .into(),
                    evidence: trimmed.to_string(),
                });
            }

            for cheat in &shell_cheats {
                if trimmed.contains(cheat) {
                    let id = Finding::compute_id(RuleId::CiWorkflowSabotage, head_path, cheat);
                    findings.push(Finding {
                        id,
                        rule_id: RuleId::CiWorkflowSabotage,
                        tier: RuleTier::Guard,
                        verdict: Verdict::Block {
                            rule: RuleId::CiWorkflowSabotage,
                            evidence: format!("Injected shell suppression '{}' at line {}", cheat, line_idx + 1),
                        },
                        file_path: head_path.clone(),
                        line_number: Some(line_idx + 1),
                        test_name: None,
                        message: format!(
                            "CI workflow sabotaged with error masking or test exclusion: '{}'",
                            cheat
                        ),
                        evidence: trimmed.to_string(),
                    });
                }
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

        // Pytest and Jest weakening flags
        let tampering_patterns = [
            "--ignore",
            "--deselect",
            "-m \"not",
            "--continue-on-collection-errors",
            "passWithNoTests",
            "filterwarnings = ignore",
        ];
        for flag in &tampering_patterns {
            if head_content.contains(flag) && !base_content.contains(flag) {
                let id = Finding::compute_id(RuleId::RunnerConfigTamper, head_path, flag);
                findings.push(Finding {
                    id,
                    rule_id: RuleId::RunnerConfigTamper,
                    tier: RuleTier::Guard,
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
                        tier: RuleTier::Guard,
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
                    tier: RuleTier::Guard,
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
                tier: RuleTier::Guard,
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
        let waivers = WaiverSet::default();

        let findings = evaluate_rules(&diff, &policy, &waivers);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].rule_id, RuleId::AssertionLock);
        assert_eq!(findings[0].tier, RuleTier::Guard);
        assert!(matches!(findings[0].verdict, Verdict::Block { .. }));
    }

    #[test]
    fn test_assertion_value_swap_blocks_even_with_equal_counts() {
        let base_test = r#"
def test_tax():
    assert calculate_tax(100) == 10
"#;
        let head_test = r#"
def test_tax():
    assert calculate_tax(100) == 0
"#;

        let mut base_files = HashMap::new();
        base_files.insert("tests/test_tax.py".to_string(), base_test.to_string());

        let mut head_files = HashMap::new();
        head_files.insert("tests/test_tax.py".to_string(), head_test.to_string());

        let diff = SemanticDiff::build(&base_files, &head_files).unwrap();
        let policy = Policy::default();
        let waivers = WaiverSet::default();

        let findings = evaluate_rules(&diff, &policy, &waivers);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].rule_id, RuleId::AssertionLock);
        assert_eq!(findings[0].tier, RuleTier::Guard);
        assert!(matches!(findings[0].verdict, Verdict::Block { .. }));
    }

    #[test]
    fn test_skip_marker_blocks_when_production_code_touched() {
        let base_test = r#"
def test_tax():
    assert calculate_tax(100) == 10
"#;
        let head_test = r#"
@pytest.mark.skip
def test_tax():
    assert calculate_tax(100) == 10
"#;
        let base_src = r#"
def calculate_tax(amount):
    return amount * 0.10
"#;
        let head_src = r#"
def calculate_tax(amount):
    return amount * 0.15
"#;

        let mut base_files = HashMap::new();
        base_files.insert("tests/test_tax.py".to_string(), base_test.to_string());
        base_files.insert("app/tax.py".to_string(), base_src.to_string());

        let mut head_files = HashMap::new();
        head_files.insert("tests/test_tax.py".to_string(), head_test.to_string());
        head_files.insert("app/tax.py".to_string(), head_src.to_string());

        let diff = SemanticDiff::build(&base_files, &head_files).unwrap();
        let policy = Policy::default();
        let waivers = WaiverSet::default();

        let findings = evaluate_rules(&diff, &policy, &waivers);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].rule_id, RuleId::AssertionLock);
        assert_eq!(findings[0].tier, RuleTier::Guard);
        assert!(matches!(findings[0].verdict, Verdict::Block { .. }));
    }

    #[test]
    fn test_skip_marker_demotes_to_review_for_flakiness_quarantine() {
        let base_test = r#"
def test_tax():
    assert calculate_tax(100) == 10
"#;
        let head_test = r#"
@pytest.mark.skip(reason="quarantining flaky test")
def test_tax():
    assert calculate_tax(100) == 10
"#;

        // No production files modified in this PR
        let mut base_files = HashMap::new();
        base_files.insert("tests/test_tax.py".to_string(), base_test.to_string());

        let mut head_files = HashMap::new();
        head_files.insert("tests/test_tax.py".to_string(), head_test.to_string());

        let diff = SemanticDiff::build(&base_files, &head_files).unwrap();
        let policy = Policy::default();
        let waivers = WaiverSet::default();

        let findings = evaluate_rules(&diff, &policy, &waivers);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].rule_id, RuleId::AssertionLock);
        assert_eq!(findings[0].tier, RuleTier::Report);
        assert!(matches!(findings[0].verdict, Verdict::Review { .. }));
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
        let waivers = WaiverSet::default();

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
        let waivers = WaiverSet::default();

        let findings = evaluate_rules(&diff, &policy, &waivers);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].rule_id, RuleId::CiWorkflowSabotage);
        assert_eq!(findings[0].tier, RuleTier::Guard);
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
        let waivers = WaiverSet::default();

        let findings = evaluate_rules(&diff, &policy, &waivers);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].rule_id, RuleId::ShadowMock);
        assert_eq!(findings[0].tier, RuleTier::Guard);
        assert!(matches!(findings[0].verdict, Verdict::Block { .. }));
    }

    #[test]
    fn test_harness_mock_top_level_jest() {
        let base_test = r#"
describe("Billing", () => {
    it("charges card", () => {
        expect(charge(100)).toBe(100);
    });
});
"#;
        let head_test = r#"
jest.mock("./billing");
describe("Billing", () => {
    it("charges card", () => {
        expect(charge(100)).toBe(100);
    });
});
"#;

        let mut base_files = HashMap::new();
        base_files.insert("tests/billing.test.js".to_string(), base_test.to_string());

        let mut head_files = HashMap::new();
        head_files.insert("tests/billing.test.js".to_string(), head_test.to_string());

        let diff = SemanticDiff::build(&base_files, &head_files).unwrap();
        let policy = Policy::default();
        let waivers = WaiverSet::default();

        let findings = evaluate_rules(&diff, &policy, &waivers);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].rule_id, RuleId::ShadowMock);
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
        let waivers = WaiverSet::default();

        let findings = evaluate_rules(&diff, &policy, &waivers);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].rule_id, RuleId::ParametrizeRowDrop);
        assert_eq!(findings[0].tier, RuleTier::Guard);
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
        let waivers = WaiverSet::default();

        let findings = evaluate_rules(&diff, &policy, &waivers);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].rule_id, RuleId::RunnerConfigTamper);
        assert_eq!(findings[0].tier, RuleTier::Guard);
    }

    #[test]
    fn test_policy_tamper_detected() {
        let mut base_files = HashMap::new();
        base_files.insert(
            "proctor.toml".to_string(),
            "allow_mocks = [\"app.clients.*\"]\n".to_string(),
        );

        let mut head_files = HashMap::new();
        head_files.insert(
            "proctor.toml".to_string(),
            "allow_mocks = [\"*\"]\n".to_string(),
        );

        let diff = SemanticDiff::build(&base_files, &head_files).unwrap();
        let policy = Policy::default();
        let waivers = WaiverSet::default();

        let findings = evaluate_rules(&diff, &policy, &waivers);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].rule_id, RuleId::PolicyTamper);
        assert_eq!(findings[0].tier, RuleTier::Guard);
        assert!(matches!(findings[0].verdict, Verdict::Block { .. }));
    }

    #[test]
    fn test_bare_wildcard_mock_rejected() {
        let mut policy = Policy::default();
        policy.allow_mocks = vec!["*".to_string()]; // Invalid bare wildcard

        assert!(!is_mock_allowed("app.billing", &policy.allow_mocks));
        assert!(!is_mock_allowed("stripe", &policy.allow_mocks));
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

        let empty_waivers = WaiverSet::default();
        let unverified_findings = evaluate_rules(&diff, &policy, &empty_waivers);
        assert_eq!(unverified_findings.len(), 1);
        let finding_id = unverified_findings[0].id.clone();

        let mut waivers = WaiverSet::default();
        waivers.maintainer_waivers.insert(finding_id.clone());

        let waived_findings = evaluate_rules(&diff, &policy, &waivers);
        assert_eq!(waived_findings.len(), 0);
    }

    #[test]
    fn test_author_ack_cannot_clear_guard_findings() {
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

        let findings = evaluate_rules(&diff, &policy, &WaiverSet::default());
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].tier, RuleTier::Guard);
        let finding_id = findings[0].id.clone();

        // An author ack on their own PR CANNOT clear a Guard finding
        let mut author_ack_set = WaiverSet::default();
        author_ack_set.author_acks.insert(finding_id.clone());
        let unsuppressed = evaluate_rules(&diff, &policy, &author_ack_set);
        assert_eq!(unsuppressed.len(), 1, "Author ack must not clear Guard finding");

        // A second reviewer maintainer waiver CAN clear it
        let mut maintainer_set = WaiverSet::default();
        maintainer_set.maintainer_waivers.insert(finding_id);
        let suppressed = evaluate_rules(&diff, &policy, &maintainer_set);
        assert_eq!(suppressed.len(), 0, "Maintainer waiver can clear Guard finding");
    }
}
