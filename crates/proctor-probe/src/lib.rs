//! Ephemeral isolated mutation probe and test kill verification for agent-proctor.

pub mod mutant;
pub mod runner;
pub mod sandbox;
pub mod verify;

pub use mutant::{apply_mutant_with_sentinel, generate_mutants, generate_targeted_mutants, Mutant};
pub use runner::{execute_probe_run, ProbeOutcome};
pub use sandbox::WorktreeGuard;
pub use verify::{
    has_pending_swap_backup, restore_swap_backup, verify_tests, verify_tests_with_timeout,
    RunnerType, TestVerificationResult, VerificationStatus, VerifyTarget,
};

use anyhow::Result;
use proctor_core::{Finding, RuleId, RuleTier, Verdict};
use proctor_diff::SemanticDiff;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProbeResult {
    pub mutant: Mutant,
    pub outcome: ProbeOutcome,
    pub target_test_file: String,
}

impl ProbeResult {
    pub fn to_finding(&self) -> Option<Finding> {
        if self.outcome == ProbeOutcome::Survived {
            let id = Finding::compute_id(
                RuleId::AssertionLock,
                &self.target_test_file,
                &self.mutant.description,
            );
            Some(Finding {
                id: id.clone(),
                rule_id: RuleId::AssertionLock,
                tier: RuleTier::Report,
                verdict: Verdict::Review {
                    finding_id: id,
                    evidence: format!(
                        "Mutant survived: {} at {}:{}",
                        self.mutant.description, self.mutant.file_path, self.mutant.line_number
                    ),
                },
                file_path: self.mutant.file_path.clone(),
                line_number: Some(self.mutant.line_number),
                test_name: Some(self.target_test_file.clone()),
                message: format!(
                    "Production code mutant survived: '{}'. Tests in '{}' passed without verifying this logic.",
                    self.mutant.description, self.target_test_file
                ),
                evidence: format!("Mutated: {}", self.mutant.mutated_snippet.trim()),
            })
        } else {
            None
        }
    }
}

/// Runs targeted ephemeral mutation probes in an isolated git worktree.
pub fn run_probe(
    repo_root: &Path,
    head_ref: &str,
    diff: &SemanticDiff,
    max_mutants_per_file: usize,
) -> Result<Vec<ProbeResult>> {
    let mut probe_results = Vec::new();

    if diff.head_sources.is_empty() || diff.paired_tests.is_empty() {
        return Ok(probe_results);
    }

    // 1. Instantiate ephemeral worktree RAII guard
    let guard = WorktreeGuard::new(repo_root, head_ref)?;
    let wt = &guard.worktree_path;
    let sentinel_path = wt.join(".proctor_sentinel");

    // 2. Identify target test file
    let target_test_file = &diff.paired_tests[0].head_test.id.file_path;
    let is_python = target_test_file.ends_with(".py");

    // 3. For each modified production source file in head
    for src_path in diff.head_sources.keys() {
        let abs_src_path = wt.join(src_path);
        if !abs_src_path.exists() {
            continue;
        }

        let original_content = match std::fs::read_to_string(&abs_src_path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        // Compute modified lines in this production source file between base and head
        let mut modified_lines = HashSet::new();
        if let Some(base_content) = diff.raw_base_files.get(src_path) {
            let base_set: HashSet<&str> = base_content.lines().collect();
            for (idx, line) in original_content.lines().enumerate() {
                if !base_set.contains(line) {
                    modified_lines.insert(idx + 1);
                }
            }
        }

        let target_set = if modified_lines.is_empty() {
            None
        } else {
            Some(&modified_lines)
        };

        let mutants = generate_targeted_mutants(
            src_path,
            &original_content,
            target_set,
            max_mutants_per_file,
        );

        for mutant in mutants {
            // Apply mutant with sentinel hook
            let mutated_content = apply_mutant_with_sentinel(&original_content, &mutant, is_python);
            if std::fs::write(&abs_src_path, &mutated_content).is_err() {
                continue;
            }

            // Execute probe run with sentinel tracking
            let outcome = execute_probe_run(wt, target_test_file, &sentinel_path, is_python)
                .unwrap_or(ProbeOutcome::Inconclusive);

            probe_results.push(ProbeResult {
                mutant: mutant.clone(),
                outcome,
                target_test_file: target_test_file.clone(),
            });

            // Restore original content
            let _ = std::fs::write(&abs_src_path, &original_content);
        }
    }

    Ok(probe_results)
}
