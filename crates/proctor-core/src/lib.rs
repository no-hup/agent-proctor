//! Core domain models, lattice definitions, and verdict types for agent-proctor.

use serde::{Deserialize, Serialize};

/// Final verdict for a check run or an individual finding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "lowercase")]
pub enum Verdict {
    /// Change is clean, or justified by an orphaned symbol definition.
    Pass,
    /// Change needs manual review from an authenticated human reviewer who is not the author.
    Review {
        finding_id: String,
        evidence: String,
    },
    /// Malicious or unjustifiable tampering (e.g. CI sabotage or shadow mocking of target).
    Block {
        rule: RuleId,
        evidence: String,
    },
}

impl Verdict {
    pub fn exit_code(&self) -> i32 {
        match self {
            Verdict::Pass => 0,
            Verdict::Review { .. } => 1,
            Verdict::Block { .. } => 2,
        }
    }
}

/// Rich hierarchical identifier for a test across git diffs.
///
/// Designed to absorb moves, renames, and reformatting without triggering false alarms.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TestId {
    /// Relative repository path (e.g. "tests/test_billing.py").
    pub file_path: String,
    /// Enclosing class or describe/context hierarchy.
    pub scope_hierarchy: Vec<String>,
    /// Function or test case name (e.g. "test_charge_card").
    pub name: String,
    /// Parameter hash for `@pytest.mark.parametrize` or table-driven cases.
    pub param_hash: Option<u64>,
    /// Content hash of the test body with local variable names alpha-normalized.
    pub body_content_hash: u64,
}

impl TestId {
    /// Stable string identifier for reporting and waiver matching.
    pub fn canonical_name(&self) -> String {
        if self.scope_hierarchy.is_empty() {
            format!("{}::{}", self.file_path, self.name)
        } else {
            format!(
                "{}::{}::{}",
                self.file_path,
                self.scope_hierarchy.join("::"),
                self.name
            )
        }
    }
}

/// Confidence of static linkage from a test to the production code it exercises.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "confidence", rename_all = "snake_case")]
pub enum Linkage {
    /// Resolved to a specific production symbol/function.
    Resolved { symbol: String, file_path: String },
    /// Resolved to an imported repository module/file.
    ModuleOnly { file_path: String },
    /// Dynamic or unresolvable linkage (always routes to REVIEW, never BLOCK, never PASS).
    Unknown,
}

/// Assertion strength classification in the framework-agnostic lattice.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AssertionStrength {
    /// Hollow assertions: `assert True`, `assert x == x`, un-awaited assertions.
    Vacuous = 0,
    /// Existence/type assertions: `toBeDefined()`, `assert x is not None`.
    Weak = 1,
    /// Strict contracts: `assert x == 42`, `toEqual()`, `pytest.raises(ValueError, match=...)`.
    Exact = 2,
}

/// Rule identifiers implemented in agent-proctor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RuleId {
    /// F1: Exact assertion removed or downgraded without orphaned reference justification.
    AssertionLock,
    /// F2: In-repo module/function mock injected into a test that invokes the same target.
    ShadowMock,
    /// F3: CI workflow step pruned, `|| true` injected, or matrix dropped.
    CiWorkflowSabotage,
    /// F3: Test runner configuration weakened (`--ignore`, coverage `fail_under` decreased).
    RunnerConfigTamper,
    /// F4: Snapshot removed, shrunk by >50%, or containing error literals.
    SnapshotIntegrity,
    /// F5: Table-driven or parametrized test row deleted.
    ParametrizeRowDrop,
}

impl RuleId {
    pub fn code(&self) -> &'static str {
        match self {
            RuleId::AssertionLock => "PROCTOR-001",
            RuleId::ShadowMock => "PROCTOR-002",
            RuleId::CiWorkflowSabotage => "PROCTOR-003",
            RuleId::RunnerConfigTamper => "PROCTOR-004",
            RuleId::SnapshotIntegrity => "PROCTOR-005",
            RuleId::ParametrizeRowDrop => "PROCTOR-006",
        }
    }

    pub fn title(&self) -> &'static str {
        match self {
            RuleId::AssertionLock => "Assertion Weakened or Removed Without Justification",
            RuleId::ShadowMock => "Shadow Mock Targets System Under Test",
            RuleId::CiWorkflowSabotage => "CI Workflow Modified to Ignore Failures",
            RuleId::RunnerConfigTamper => "Test Runner Configuration Weakened",
            RuleId::SnapshotIntegrity => "Snapshot File Corrupted or Shrunk",
            RuleId::ParametrizeRowDrop => "Parametrized Test Case Removed",
        }
    }
}

/// An integrity finding produced by the rules engine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Finding {
    /// Blake3-derived 12-char fingerprint for stable waiver matching.
    pub id: String,
    pub rule_id: RuleId,
    pub verdict: Verdict,
    pub file_path: String,
    pub line_number: Option<usize>,
    pub test_name: Option<String>,
    pub message: String,
    pub evidence: String,
}

impl Finding {
    /// Computes a stable 12-character Blake3 finding ID.
    pub fn compute_id(rule: RuleId, test_name: &str, evidence_snippet: &str) -> String {
        let mut hasher = blake3::Hasher::new();
        hasher.update(rule.code().as_bytes());
        hasher.update(test_name.as_bytes());
        hasher.update(evidence_snippet.as_bytes());
        let hash = hasher.finalize();
        hash.to_hex()[..12].to_string()
    }
}

/// Repository configuration policy, loaded from upstream protected base branch.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Policy {
    #[serde(default)]
    pub allow_mocks: Vec<String>,
    #[serde(default)]
    pub ignored_paths: Vec<String>,
    #[serde(default = "default_true")]
    pub enforce_ci_tampering: bool,
    #[serde(default)]
    pub strict_mode: bool,
}

fn default_true() -> bool {
    true
}

impl Default for Policy {
    fn default() -> Self {
        Self {
            allow_mocks: vec!["app.clients.*".to_string(), "tests.fixtures.*".to_string()],
            ignored_paths: vec!["vendor/**".to_string(), "node_modules/**".to_string()],
            enforce_ci_tampering: true,
            strict_mode: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_verdict_exit_codes() {
        assert_eq!(Verdict::Pass.exit_code(), 0);
        assert_eq!(
            Verdict::Review {
                finding_id: "abc".into(),
                evidence: "diff".into()
            }
            .exit_code(),
            1
        );
        assert_eq!(
            Verdict::Block {
                rule: RuleId::CiWorkflowSabotage,
                evidence: "|| true".into()
            }
            .exit_code(),
            2
        );
    }

    #[test]
    fn test_finding_id_stability() {
        let id1 = Finding::compute_id(RuleId::AssertionLock, "test_tax", "assert total == 120");
        let id2 = Finding::compute_id(RuleId::AssertionLock, "test_tax", "assert total == 120");
        assert_eq!(id1, id2);
        assert_eq!(id1.len(), 12);
    }
}
