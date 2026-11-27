//! Pure integrity rules, assertion lattice evaluation, and CI sabotage guards for agent-proctor.

use proctor_core::*;
use proctor_diff::SemanticDiff;
use std::collections::HashSet;
use std::path::Path;

/// Evaluates all integrity rules against the semantic diff and policy.
pub fn evaluate_rules(diff: &SemanticDiff, policy: &Policy, waivers: &WaiverSet) -> Vec<Finding> {
    let mut findings = Vec::new();

    // 1. PROCTOR-007: Policy Tampering (Cannot modify proctor.toml in PR)
    evaluate_policy_tamper(diff, &mut findings);

    // 2. Parse/syntax errors are quiet receipt notes (reporter), not AssertionLock findings.

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

    // 9. PROCTOR-004 (extension): Type-checker / linter / coverage config loosening
    evaluate_config_loosening(diff, &mut findings);

    // 10. PROCTOR-008: Blanket checker suppressions
    evaluate_blanket_suppressions(diff, &mut findings);

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

/// PROCTOR-001: Checks for assertion deletions, downgrades, skips, and un-orphaned test removals.
fn evaluate_assertion_lock(diff: &SemanticDiff, policy: &Policy, findings: &mut Vec<Finding>) {
    // 0. Non-test fixture/helper skip injection inside test files
    let base_skips: HashSet<&(String, String)> = diff.base_fixture_skips.iter().collect();
    for (file_path, fn_name) in &diff.head_fixture_skips {
        if !base_skips.contains(&(file_path.clone(), fn_name.clone())) {
            let scoped_name = format!("{}::{}", file_path, fn_name);
            let id = Finding::compute_id(RuleId::AssertionLock, &scoped_name, "fixture_skipped");
            let (verdict, tier, message) = if !diff.head_sources.is_empty() {
                (
                    Verdict::Block {
                        rule: RuleId::AssertionLock,
                        evidence: format!(
                            "Test helper/fixture '{}' added a skip call while production code changed",
                            scoped_name
                        ),
                    },
                    RuleTier::Guard,
                    format!(
                        "Test helper/fixture '{}' added a skip call (pytest.skip / skipTest) while production code changed.",
                        scoped_name
                    ),
                )
            } else {
                (
                    Verdict::Review {
                        finding_id: id.clone(),
                        evidence: format!(
                            "Test helper/fixture '{}' added a skip call; requires reviewer acceptance.",
                            scoped_name
                        ),
                    },
                    RuleTier::Report,
                    format!(
                        "Test helper/fixture '{}' added a skip call (pytest.skip / skipTest); requires reviewer acknowledgement.",
                        scoped_name
                    ),
                )
            };
            findings.push(Finding {
                id,
                rule_id: RuleId::AssertionLock,
                tier,
                verdict,
                file_path: file_path.clone(),
                line_number: None,
                test_name: Some(scoped_name),
                message,
                evidence: "Skipped helper/fixture".into(),
            });
        }
    }

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

        // 1a(ii). Check for modified, weakened, guard-wrapped, or deleted assertions via AssertionStrength lattice
        let has_production_changes = !diff.head_sources.is_empty() || !diff.base_sources.is_empty();
        let linked = diff.test_links_changed_code(base_test, head_test);

        for base_assert in &base_test.assertions {
            // A test that only follows a production rename is not a modification.
            let followed = apply_renames(&base_assert.text, &diff.renamed_symbols);
            let preserved_unguarded = head_test.assertions.iter().any(|h| {
                (h.text == base_assert.text || h.text == followed)
                    && (!h.is_guarded || base_assert.is_guarded)
            });
            if !preserved_unguarded {
                let wrapped_in_guard = !base_assert.is_guarded
                    && head_test.assertions.iter().any(|h| {
                        (h.text == base_assert.text || h.text == followed) && h.is_guarded
                    });
                // This assertion was modified, guard-wrapped, or removed
                let justified = diff.is_reference_orphaned(&base_assert.referenced_symbols);

                if !justified {
                    let id =
                        Finding::compute_id(RuleId::AssertionLock, &test_name, &base_assert.text);

                    // Weakened = fewer unguarded assertions at or above this strength, or wrapped in if/raises.
                    let at_least = |t: &proctor_parse::ParsedTest| {
                        t.assertions
                            .iter()
                            .filter(|a| !a.is_guarded && a.strength >= base_assert.strength)
                            .count()
                    };
                    let was_weakened =
                        wrapped_in_guard || at_least(head_test) < at_least(base_test);

                    // Linkage: only a change to code this test actually references can explain
                    // a changed assertion. An unrelated production edit does not.
                    // In `protect_existing_tests` policy mode, pre-existing assertions are the spec.
                    let guard_reason = if policy.protect_existing_tests {
                        Some("under protect_existing_tests policy")
                    } else if !has_production_changes {
                        Some("while production code was untouched")
                    } else if was_weakened && !linked {
                        Some("while none of the production code it references changed")
                    } else {
                        None
                    };

                    let (verdict, tier, message) = if let Some(reason) = guard_reason {
                        (
                            Verdict::Block {
                                rule: RuleId::AssertionLock,
                                evidence: format!(
                                    "Assertion modified/dropped {}: '{}'",
                                    reason, base_assert.text
                                ),
                            },
                            RuleTier::Guard,
                            format!(
                                "Assertion {} in test '{}' {}.",
                                if was_weakened {
                                    "weakened or removed"
                                } else {
                                    "modified"
                                },
                                test_name,
                                reason
                            ),
                        )
                    } else if was_weakened {
                        (
                            Verdict::Review {
                                finding_id: id.clone(),
                                evidence: format!(
                                    "Assertion weakened/dropped: '{}'. Linked production symbol changed.",
                                    base_assert.text
                                ),
                            },
                            RuleTier::Report,
                            format!(
                                "Assertion weakened in test '{}' while referenced production code also changed.",
                                test_name
                            ),
                        )
                    } else {
                        // Equal-rank co-evolution alongside production changes: quiet under
                        // default mode (incomplete symbol linking used to inflate Review noise).
                        // Surfaced in the receipt's "Changed pre-existing assertions" table.
                        continue;
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
        let mut seen_mirrors = std::collections::HashSet::new();
        for head_assert in &head_test.assertions {
            if let Some(expected_lit) = &head_assert.expected_literal {
                // If this is a new literal not present in base test assertions
                let in_base_asserts = base_test
                    .assertions
                    .iter()
                    .any(|a| a.expected_literal.as_deref() == Some(expected_lit));

                if !in_base_asserts && seen_mirrors.insert(expected_lit.clone()) {
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
                            let ev = format!(
                                "Expected literal '{}' shifted and mirrors newly added production literal in {}",
                                expected_lit, src_path
                            );
                            let (tier, verdict) = if policy.protect_existing_tests {
                                (
                                    RuleTier::Guard,
                                    Verdict::Block {
                                        rule: RuleId::AssertionLock,
                                        evidence: ev,
                                    },
                                )
                            } else {
                                (
                                    RuleTier::Report,
                                    Verdict::Review {
                                        finding_id: id.clone(),
                                        evidence: ev,
                                    },
                                )
                            };
                            findings.push(Finding {
                                id,
                                rule_id: RuleId::AssertionLock,
                                tier,
                                verdict,
                                file_path: head_test.id.file_path.clone(),
                                line_number: Some(head_assert.line),
                                test_name: Some(test_name.clone()),
                                message: format!(
                                    "Test expected value '{}' shifted to mirror newly added production constant.",
                                    expected_lit
                                ),
                                evidence: head_assert.text.clone(),
                            });
                            break;
                        }
                    }
                }
            }
        }
    }

    // 1c. Whole test deletions (Guard tier: always BLOCK)
    for del_test in &diff.deleted_tests {
        let test_name = del_test.id.canonical_name();
        // Justified only if every production symbol the test body touches (calls, mock-style
        // checks, assertions) was deleted. A test touching no production symbol is never auto-justified.
        let justified = diff.is_reference_orphaned(&del_test.body_symbols);

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

/// Rewrites identifier tokens using the old -> new rename map.
fn apply_renames(text: &str, renames: &std::collections::HashMap<String, String>) -> String {
    if renames.is_empty() {
        return text.to_string();
    }
    let mut out = String::with_capacity(text.len());
    let mut word = String::new();
    let flush = |word: &mut String, out: &mut String| {
        out.push_str(renames.get(word.as_str()).unwrap_or(word));
        word.clear();
    };
    for c in text.chars() {
        if c.is_alphanumeric() || c == '_' {
            word.push(c);
        } else {
            flush(&mut word, &mut out);
            out.push(c);
        }
    }
    flush(&mut word, &mut out);
    out
}

/// Dotted module paths for every file in the diff: `src/app/billing.ts` -> `src.app.billing` and `app.billing`.
fn repo_modules(diff: &SemanticDiff) -> HashSet<String> {
    let mut out = HashSet::new();
    for path in diff.raw_base_files.keys().chain(diff.raw_head_files.keys()) {
        let stem = match path.rsplit_once('.') {
            Some((stem, ext)) if ["py", "js", "ts", "jsx", "tsx", "mjs", "cjs"].contains(&ext) => {
                stem
            }
            _ => continue,
        };
        let module = stem
            .trim_end_matches("/__init__")
            .trim_end_matches("/index")
            .replace('/', ".")
            .to_lowercase();
        if let Some(rest) = module.strip_prefix("src.") {
            out.insert(rest.to_string());
        }
        out.insert(module);
    }
    out
}

/// Helper to test if a mock target is an in-repo symbol or file
fn is_in_repo_target(
    target: &str,
    in_repo_symbols: &HashSet<String>,
    repo_modules: &HashSet<String>,
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

    // 3. Target module is a file in this diff (e.g. app.billing.charge -> app/billing.py).
    // Resolved from git content only, never the host disk, so results don't depend on cwd.
    if repo_modules
        .iter()
        .any(|m| target_clean == *m || target_clean.starts_with(&format!("{}.", m)))
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
    let modules = repo_modules(diff);
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
        let base_mock_targets: HashSet<&str> = pair
            .base_test
            .mocks
            .iter()
            .map(|m| m.target.as_str())
            .collect();
        let mut seen_head_targets: HashSet<&str> = HashSet::new();

        for mock in &pair.head_test.mocks {
            if !base_mock_targets.contains(mock.target.as_str())
                && seen_head_targets.insert(mock.target.as_str())
            {
                let allowed = is_mock_allowed(&mock.target, &policy.allow_mocks);

                if !allowed && is_in_repo_target(&mock.target, &in_repo_symbols, &modules) {
                    let test_name = pair.head_test.id.canonical_name();
                    let id = Finding::compute_id(RuleId::ShadowMock, &test_name, &mock.target);

                    let target_clean = mock.target.to_lowercase();
                    let target_leaf = mock
                        .target
                        .split('.')
                        .next_back()
                        .unwrap_or(&mock.target)
                        .to_lowercase();
                    let is_direct_diff_target = in_repo_symbols.iter().any(|sym| {
                        target_clean.contains(sym.as_str()) || sym.contains(target_clean.as_str())
                    });
                    let is_sut_mock = pair.head_test.assertions.iter().any(|a| {
                        let a_lower = a.text.to_lowercase();
                        a_lower.contains(&target_leaf) || a_lower.contains(&target_clean)
                    });

                    let (verdict, tier) = if is_direct_diff_target
                        || is_sut_mock
                        || policy.strict_mode
                    {
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
    let mut seen_harness_targets: HashSet<&str> = HashSet::new();

    for mock in &diff.head_harness_mocks {
        if !base_harness_targets.contains(mock.target.as_str())
            && seen_harness_targets.insert(mock.target.as_str())
        {
            let allowed = is_mock_allowed(&mock.target, &policy.allow_mocks);

            if !allowed && is_in_repo_target(&mock.target, &in_repo_symbols, &modules) {
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
    let workflow_patterns = [
        ".github/workflows",
        ".gitlab-ci.yml",
        ".circleci/config.yml",
    ];

    for (head_path, head_content) in &diff.raw_head_files {
        let is_ci = workflow_patterns.iter().any(|pat| head_path.contains(pat));
        if !is_ci {
            continue;
        }

        let base_content = diff
            .raw_base_files
            .get(head_path)
            .map(|s| s.as_str())
            .unwrap_or("");
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
                let id =
                    Finding::compute_id(RuleId::CiWorkflowSabotage, head_path, "continue-on-error");
                findings.push(Finding {
                    id,
                    rule_id: RuleId::CiWorkflowSabotage,
                    tier: RuleTier::Guard,
                    verdict: Verdict::Block {
                        rule: RuleId::CiWorkflowSabotage,
                        evidence: format!(
                            "Injected 'continue-on-error: true' at line {}",
                            line_idx + 1
                        ),
                    },
                    file_path: head_path.clone(),
                    line_number: Some(line_idx + 1),
                    test_name: None,
                    message:
                        "CI workflow modified to ignore step failures (continue-on-error: true)."
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
                            evidence: format!(
                                "Injected shell suppression '{}' at line {}",
                                cheat,
                                line_idx + 1
                            ),
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

        let base_content = diff
            .raw_base_files
            .get(head_path)
            .map(|s| s.as_str())
            .unwrap_or("");

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

/// Lines present in head but not base (trimmed), with 1-based head line numbers.
fn added_lines<'a>(base: &str, head: &'a str) -> Vec<(usize, &'a str)> {
    let base_lines: HashSet<&str> = base.lines().map(str::trim).collect();
    head.lines()
        .enumerate()
        .map(|(i, l)| (i + 1, l.trim()))
        .filter(|(_, l)| !l.is_empty() && !base_lines.contains(l))
        .collect()
}

/// Splits `key = value` / `"key": value` into a normalized key and the raw value.
fn config_key_value(line: &str) -> Option<(String, &str)> {
    let idx = line.find(['=', ':'])?;
    let key = line[..idx]
        .trim()
        .trim_matches(|c| c == '"' || c == '\'')
        .to_lowercase();
    if key.is_empty() || key.contains(' ') {
        return None;
    }
    Some((key, line[idx + 1..].trim().trim_end_matches(',')))
}

fn first_number_for_keys(content: &str, keys: &[&str]) -> Option<f64> {
    content.lines().find_map(|l| {
        let (k, v) = config_key_value(l.trim())?;
        keys.contains(&k.as_str())
            .then(|| v.trim_matches('"').parse().ok())
            .flatten()
    })
}

fn push_guard(
    findings: &mut Vec<Finding>,
    rule: RuleId,
    path: &str,
    line: Option<usize>,
    key: &str,
    message: String,
    evidence: &str,
) {
    findings.push(Finding {
        id: Finding::compute_id(rule, path, key),
        rule_id: rule,
        tier: RuleTier::Guard,
        verdict: Verdict::Block {
            rule,
            evidence: message.clone(),
        },
        file_path: path.to_string(),
        line_number: line,
        test_name: None,
        message,
        evidence: evidence.to_string(),
    });
}

/// PROCTOR-004 extension: strictness switched off, lint ignores added, coverage floor lowered.
fn evaluate_config_loosening(diff: &SemanticDiff, findings: &mut Vec<Finding>) {
    const PY_CONFIGS: [&str; 8] = [
        "pyproject.toml",
        "ruff.toml",
        ".ruff.toml",
        "setup.cfg",
        ".flake8",
        "tox.ini",
        "mypy.ini",
        ".coveragerc",
    ];
    const LINT_IGNORE_KEYS: [&str; 7] = [
        "ignore",
        "extend-ignore",
        "extend_ignore",
        "per-file-ignores",
        "per_file_ignores",
        "extend-per-file-ignores",
        "disable_error_code",
    ];

    for (path, head) in &diff.raw_head_files {
        let file = Path::new(path)
            .file_name()
            .and_then(|f| f.to_str())
            .unwrap_or("");
        let base = diff
            .raw_base_files
            .get(path)
            .map(String::as_str)
            .unwrap_or("");
        let is_tsconfig = file.starts_with("tsconfig") && file.ends_with(".json");
        let is_eslint = file.starts_with(".eslintrc") || file.starts_with("eslint.config");
        let is_py_config = PY_CONFIGS.contains(&file);
        if !(is_tsconfig || is_eslint || is_py_config) {
            continue;
        }

        // Coverage floor lowered or removed (fail_under)
        if is_py_config {
            let keys = ["fail_under", "fail-under"];
            match (
                first_number_for_keys(base, &keys),
                first_number_for_keys(head, &keys),
            ) {
                (Some(b), Some(h)) if h < b => push_guard(
                    findings,
                    RuleId::RunnerConfigTamper,
                    path,
                    None,
                    "fail_under",
                    format!("Coverage floor lowered from {} to {} in '{}'.", b, h, path),
                    &format!("fail_under {} -> {}", b, h),
                ),
                (Some(b), None) => push_guard(
                    findings,
                    RuleId::RunnerConfigTamper,
                    path,
                    None,
                    "fail_under_removed",
                    format!(
                        "Coverage floor (fail_under = {}) removed from '{}'.",
                        b, path
                    ),
                    &format!("fail_under {} removed", b),
                ),
                _ => {}
            }
        }

        // Strictness switched on in base but the line is gone in head (defaults are lax).
        let head_lines: HashSet<&str> = head.lines().map(str::trim).collect();
        for (idx, raw) in base.lines().enumerate() {
            let line = raw.trim();
            if head_lines.contains(line) {
                continue;
            }
            if let Some((key, value)) = config_key_value(line) {
                let value = value.trim_matches(|c| c == '"' || c == '\'').to_lowercase();
                let is_strict_key = key == "strict"
                    || key.starts_with("strictnull")
                    || key.starts_with("noimplicit");
                if is_strict_key && value == "true" {
                    let still_set = head
                        .lines()
                        .filter_map(|l| config_key_value(l.trim()))
                        .any(|(k, _)| k == key);
                    if !still_set {
                        push_guard(
                            findings,
                            RuleId::RunnerConfigTamper,
                            path,
                            Some(idx + 1),
                            &format!("removed:{}", line),
                            format!(
                                "Strictness setting removed from '{}': '{}' (base line {}).",
                                path,
                                line,
                                idx + 1
                            ),
                            line,
                        );
                    }
                }
            }
        }

        // Track the most recent `key =` so list continuation lines inherit it.
        let added: HashSet<usize> = added_lines(base, head)
            .into_iter()
            .map(|(n, _)| n)
            .collect();
        let mut current_key = String::new();
        let mut section_key = String::new();
        // `[[tool.mypy.overrides]]` / `[mypy-pkg.*]`: per-module escape hatches, a normal migration tool.
        let mut scoped_section = false;
        let new_file = base.is_empty();
        for (idx, raw) in head.lines().enumerate() {
            let line = raw.trim();
            if line.starts_with('[') && line.ends_with(']') {
                current_key.clear();
                scoped_section = line.contains("overrides") || line.starts_with("[mypy-");
                section_key = line
                    .trim_matches(['[', ']'])
                    .rsplit('.')
                    .next()
                    .unwrap_or("")
                    .to_lowercase();
                continue;
            }
            let kv = config_key_value(line);
            if let Some((k, _)) = &kv {
                current_key = k.clone();
            }
            if !added.contains(&(idx + 1)) {
                continue;
            }
            let value = kv
                .as_ref()
                .map(|(_, v)| v.trim_matches(|c| c == '"' || c == '\'').to_lowercase());
            let key = kv.as_ref().map(|(k, _)| k.as_str()).unwrap_or("");

            let strictness_off = value.as_deref() == Some("false")
                && (key.starts_with("strict")
                    || key.starts_with("noimplicit")
                    || key.starts_with("nounchecked")
                    || key == "disallow_untyped_defs"
                    || key == "warn_unused_ignores");
            let mypy_ignore = key == "ignore_errors" && value.as_deref() == Some("true");
            let eslint_off = is_eslint && matches!(value.as_deref(), Some("off") | Some("0"));

            if (strictness_off || mypy_ignore || eslint_off) && !scoped_section && !new_file {
                push_guard(
                    findings,
                    RuleId::RunnerConfigTamper,
                    path,
                    Some(idx + 1),
                    line,
                    format!("Checker strictness loosened in '{}': '{}'", path, line),
                    line,
                );
            } else if strictness_off || mypy_ignore || eslint_off {
                let id = Finding::compute_id(RuleId::RunnerConfigTamper, path, line);
                let why = if new_file {
                    "new config file"
                } else {
                    "per-module override"
                };
                findings.push(Finding {
                    id: id.clone(),
                    rule_id: RuleId::RunnerConfigTamper,
                    tier: RuleTier::Report,
                    verdict: Verdict::Review {
                        finding_id: id,
                        evidence: format!("Strictness relaxed ({}) in '{}': '{}'", why, path, line),
                    },
                    file_path: path.clone(),
                    line_number: Some(idx + 1),
                    test_name: None,
                    message: format!("Strictness relaxed in a {} in '{}': '{}'.", why, path, line),
                    evidence: line.to_string(),
                });
            } else if is_py_config
                && (LINT_IGNORE_KEYS.contains(&current_key.as_str())
                    || LINT_IGNORE_KEYS.contains(&section_key.as_str()))
            {
                // Adding an ignore code is common in human PRs: report, don't block.
                let id = Finding::compute_id(RuleId::RunnerConfigTamper, path, line);
                findings.push(Finding {
                    id: id.clone(),
                    rule_id: RuleId::RunnerConfigTamper,
                    tier: RuleTier::Report,
                    verdict: Verdict::Review {
                        finding_id: id,
                        evidence: format!("Lint ignore added in '{}': '{}'", path, line),
                    },
                    file_path: path.clone(),
                    line_number: Some(idx + 1),
                    test_name: None,
                    message: format!(
                        "Lint/type-check ignore list extended in '{}' ({}).",
                        path, current_key
                    ),
                    evidence: line.to_string(),
                });
            }
        }
    }
}

/// Returns a label if the line is a file-level / untargeted checker suppression.
fn blanket_suppression(path: &str, line: &str) -> Option<&'static str> {
    let ext = Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");
    match ext {
        "py" => {
            let comment = &line[line.find('#')?..];
            let c = comment.trim_start_matches('#').trim().to_lowercase();
            if c == "ruff: noqa" || c == "flake8: noqa" {
                Some("file-level noqa")
            } else if c.starts_with("mypy: ignore-errors") {
                Some("mypy: ignore-errors")
            } else if c == "type: ignore" && line.starts_with('#') {
                Some("module-level type: ignore")
            } else if c.starts_with("pylint: disable=all") {
                Some("pylint: disable=all")
            } else {
                None
            }
        }
        "js" | "jsx" | "ts" | "tsx" | "mjs" | "cjs" => {
            if line.contains("@ts-nocheck") {
                return Some("@ts-nocheck");
            }
            let rest = line.split("eslint-disable").nth(1)?;
            let rules = rest.trim().trim_end_matches("*/").trim();
            // eslint-disable-line / -next-line are line-scoped; only untargeted file-level disables count.
            (!rest.starts_with('-') && rules.is_empty()).then_some("blanket eslint-disable")
        }
        "rs" => line
            .starts_with("#![allow(")
            .then_some("crate-level #![allow]"),
        _ => None,
    }
}

/// PROCTOR-008: Blanket suppressions added to source or test files.
fn evaluate_blanket_suppressions(diff: &SemanticDiff, findings: &mut Vec<Finding>) {
    for (path, head) in &diff.raw_head_files {
        let base = diff
            .raw_base_files
            .get(path)
            .map(String::as_str)
            .unwrap_or("");
        for (line_no, line) in added_lines(base, head) {
            if let Some(kind) = blanket_suppression(path, line) {
                let id = Finding::compute_id(RuleId::BlanketSuppression, path, line);
                findings.push(Finding {
                    id: id.clone(),
                    rule_id: RuleId::BlanketSuppression,
                    tier: RuleTier::Report,
                    verdict: Verdict::Review {
                        finding_id: id,
                        evidence: format!("{} added at {}:{}", kind, path, line_no),
                    },
                    file_path: path.clone(),
                    line_number: Some(line_no),
                    test_name: None,
                    message: format!(
                        "Blanket checker suppression ({}) added in '{}'.",
                        kind, path
                    ),
                    evidence: line.to_string(),
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
                let id =
                    Finding::compute_id(RuleId::SnapshotIntegrity, base_path, "deleted_snapshot");
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
        let policy = Policy {
            allow_mocks: vec!["*".to_string()], // Invalid bare wildcard
            ..Default::default()
        };

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
        assert_eq!(
            unsuppressed.len(),
            1,
            "Author ack must not clear Guard finding"
        );

        // A second reviewer maintainer waiver CAN clear it
        let mut maintainer_set = WaiverSet::default();
        maintainer_set.maintainer_waivers.insert(finding_id);
        let suppressed = evaluate_rules(&diff, &policy, &maintainer_set);
        assert_eq!(
            suppressed.len(),
            0,
            "Maintainer waiver can clear Guard finding"
        );
    }

    #[test]
    fn test_conditional_if_guard_and_raises_wrapping_detected() {
        let base_test = r#"
def test_dtype(dtype):
    assert str(dtype) == "float32"

def test_combine(actual):
    assert_identical(1, actual)
"#;
        let head_test = r#"
def test_dtype(dtype):
    if dtype == "float32":
        assert str(dtype) == "float32"

def test_combine(actual):
    with pytest.raises(AssertionError):
        assert_identical(1, actual)
"#;
        let base = std::collections::HashMap::from([(
            "tests/test_x.py".to_string(),
            base_test.to_string(),
        )]);
        let head = std::collections::HashMap::from([(
            "tests/test_x.py".to_string(),
            head_test.to_string(),
        )]);
        let diff = SemanticDiff::build(&base, &head).unwrap();
        let findings = evaluate_rules(&diff, &Policy::default(), &WaiverSet::default());
        assert_eq!(
            findings.len(),
            2,
            "Both if-wrapped and pytest.raises-wrapped assertions must trigger findings: {:?}",
            findings
        );
        assert!(findings.iter().all(|f| f.tier == RuleTier::Guard));
    }

    #[test]
    fn test_fixture_skip_and_assert_helper_weakening_detected() {
        let base_test = r#"
def app_params():
    return True

def assert_node(node, expected):
    assert node == expected
"#;
        let head_test = r#"
def app_params():
    pytest.skip("missing root")

def assert_node(node, expected):
    if expected == "A" and node == "B":
        return
    assert node == expected
"#;
        let base = std::collections::HashMap::from([
            ("tests/test_h.py".to_string(), base_test.to_string()),
            ("src/mod.py".to_string(), "def f(): return 1\n".to_string()),
        ]);
        let head = std::collections::HashMap::from([
            ("tests/test_h.py".to_string(), head_test.to_string()),
            ("src/mod.py".to_string(), "def f(): return 2\n".to_string()),
        ]);
        let diff = SemanticDiff::build(&base, &head).unwrap();
        let findings = evaluate_rules(&diff, &Policy::default(), &WaiverSet::default());
        assert!(
            findings
                .iter()
                .any(|f| f.tier == RuleTier::Guard && f.message.contains("app_params")),
            "Fixture skip alongside prod change must trigger Guard: {:?}",
            findings
        );
        assert!(
            findings.iter().any(|f| f.message.contains("assert_node")),
            "Conditional weakening inside assert_* helper must trigger finding: {:?}",
            findings
        );
    }

    #[test]
    fn test_mirror_check_deduplicates_and_respects_policy_mode() {
        let base = std::collections::HashMap::from([
            ("tests/test_ui.py".to_string(), "def test_screen():\n    assert get_screen() == 'tide-moon'\n".to_string()),
            ("src/a.py".to_string(), "def get_screen(): return 'tide-moon'\n".to_string()),
            ("src/b.py".to_string(), "SCREEN = 'tide-moon'\n".to_string()),
        ]);
        let head = std::collections::HashMap::from([
            ("tests/test_ui.py".to_string(), "def test_screen():\n    assert get_screen() == 'tide-lamp'\n".to_string()),
            ("src/a.py".to_string(), "def get_screen(): return 'tide-lamp'\n".to_string()),
            ("src/b.py".to_string(), "SCREEN = 'tide-lamp'\n".to_string()),
        ]);
        let diff = SemanticDiff::build(&base, &head).unwrap();

        let default_findings = evaluate_rules(&diff, &Policy::default(), &WaiverSet::default());
        let mirror_default: Vec<_> = default_findings
            .iter()
            .filter(|f| f.message.contains("shifted to mirror"))
            .collect();
        assert_eq!(mirror_default.len(), 1, "Must emit once per (test, literal): {:?}", mirror_default);
        assert_eq!(mirror_default[0].tier, RuleTier::Report, "Default mode should be Report, not Guard");

        let pol = Policy {
            protect_existing_tests: true,
            ..Policy::default()
        };
        let policy_findings = evaluate_rules(&diff, &pol, &WaiverSet::default());
        let mirror_policy: Vec<_> = policy_findings
            .iter()
            .filter(|f| f.message.contains("shifted to mirror"))
            .collect();
        assert_eq!(mirror_policy.len(), 1);
        assert_eq!(mirror_policy[0].tier, RuleTier::Guard, "Policy mode should escalate mirror to Guard");
    }

    #[test]
    fn test_equal_rank_coevolution_quiet_in_default_mode() {
        let base = HashMap::from([
            (
                "app/cart.py".to_string(),
                "def total(x):\n    return x\n".to_string(),
            ),
            (
                "tests/test_cart.py".to_string(),
                "def test_total():\n    assert total(10) == 10\n".to_string(),
            ),
        ]);
        let head = HashMap::from([
            (
                "app/cart.py".to_string(),
                "def total(x):\n    return x + 1\n".to_string(),
            ),
            (
                "tests/test_cart.py".to_string(),
                "def test_total():\n    assert total(10) == 11\n".to_string(),
            ),
        ]);
        let diff = SemanticDiff::build(&base, &head).unwrap();
        let findings = evaluate_rules(&diff, &Policy::default(), &WaiverSet::default());
        assert!(
            findings
                .iter()
                .filter(|f| f.rule_id == RuleId::AssertionLock)
                .count()
                == 0,
            "Equal-rank co-evolution with production changes must emit no AssertionLock finding: {findings:?}"
        );
    }

    #[test]
    fn test_equal_rank_coevolution_blocks_under_protect_existing_tests() {
        let base = HashMap::from([
            (
                "app/cart.py".to_string(),
                "def total(x):\n    return x\n".to_string(),
            ),
            (
                "tests/test_cart.py".to_string(),
                "def test_total():\n    assert total(10) == 10\n".to_string(),
            ),
        ]);
        let head = HashMap::from([
            (
                "app/cart.py".to_string(),
                "def total(x):\n    return x + 1\n".to_string(),
            ),
            (
                "tests/test_cart.py".to_string(),
                "def test_total():\n    assert total(10) == 11\n".to_string(),
            ),
        ]);
        let diff = SemanticDiff::build(&base, &head).unwrap();
        let pol = Policy {
            protect_existing_tests: true,
            ..Policy::default()
        };
        let findings = evaluate_rules(&diff, &pol, &WaiverSet::default());
        assert!(
            findings.iter().any(|f| {
                f.rule_id == RuleId::AssertionLock
                    && f.tier == RuleTier::Guard
                    && matches!(f.verdict, Verdict::Block { .. })
            }),
            "protect_existing_tests must Guard-block equal-rank assertion edits: {findings:?}"
        );
    }

    #[test]
    fn test_equal_rank_edit_blocks_when_no_production_change() {
        let base = HashMap::from([(
            "tests/test_cart.py".to_string(),
            "def test_total():\n    assert total(10) == 10\n".to_string(),
        )]);
        let head = HashMap::from([(
            "tests/test_cart.py".to_string(),
            "def test_total():\n    assert total(10) == 11\n".to_string(),
        )]);
        let diff = SemanticDiff::build(&base, &head).unwrap();
        let findings = evaluate_rules(&diff, &Policy::default(), &WaiverSet::default());
        assert_eq!(findings.len(), 1, "{findings:?}");
        assert_eq!(findings[0].rule_id, RuleId::AssertionLock);
        assert_eq!(findings[0].tier, RuleTier::Guard);
        assert!(matches!(findings[0].verdict, Verdict::Block { .. }));
    }

    #[test]
    fn test_parse_errors_do_not_emit_assertion_lock_findings() {
        let mut diff = SemanticDiff::build(&HashMap::new(), &HashMap::new()).unwrap();
        diff.parse_errors.push((
            "huge/unsupported.py".to_string(),
            "Tree-sitter parse error: unexpected token".to_string(),
        ));
        let findings = evaluate_rules(&diff, &Policy::default(), &WaiverSet::default());
        assert!(
            findings.is_empty(),
            "parse_errors must not become AssertionLock findings: {findings:?}"
        );
    }

    #[test]
    fn test_go_table_driven_row_drop_blocks_as_parametrize_row_drop() {
        let base_code = r#"
package auth_test
import "testing"
func TestAuth(t *testing.T) {
    tests := []struct{ name string; valid bool }{
        {name: "case 1", valid: true},
        {name: "case 2", valid: false},
    }
    for _, tc := range tests {
        if !tc.valid { t.Fail() }
    }
}
"#;
        let head_code = r#"
package auth_test
import "testing"
func TestAuth(t *testing.T) {
    tests := []struct{ name string; valid bool }{
        {name: "case 1", valid: true},
    }
    for _, tc := range tests {
        if !tc.valid { t.Fail() }
    }
}
"#;
        let base = HashMap::from([("pkg/auth_test.go".to_string(), base_code.to_string())]);
        let head = HashMap::from([("pkg/auth_test.go".to_string(), head_code.to_string())]);
        let diff = SemanticDiff::build(&base, &head).unwrap();
        let findings = evaluate_rules(&diff, &Policy::default(), &WaiverSet::default());
        assert_eq!(findings.len(), 1, "{findings:?}");
        assert_eq!(findings[0].rule_id, RuleId::ParametrizeRowDrop);
        assert_eq!(findings[0].tier, RuleTier::Guard);
        assert!(matches!(findings[0].verdict, Verdict::Block { .. }));
    }
}
