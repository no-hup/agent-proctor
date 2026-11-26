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
    Block { rule: RuleId, evidence: String },
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

/// Product tier classification for findings and rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RuleTier {
    /// Hard boundary: zero tolerance, always blocks PR (exit 2).
    Guard,
    /// Advisory/attribution: exits 0 by default, exits 1 under --strict.
    Report,
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
    /// F6: proctor.toml policy configuration modified in PR.
    PolicyTamper,
    /// F7: File-level checker suppression added (`@ts-nocheck`, bare `eslint-disable`, `# mypy: ignore-errors`).
    BlanketSuppression,
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
            RuleId::PolicyTamper => "PROCTOR-007",
            RuleId::BlanketSuppression => "PROCTOR-008",
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
            RuleId::PolicyTamper => "Policy Configuration Tampered in PR",
            RuleId::BlanketSuppression => "Blanket Checker Suppression Added",
        }
    }

    pub fn default_tier(&self) -> RuleTier {
        match self {
            RuleId::AssertionLock => RuleTier::Report,
            RuleId::ShadowMock => RuleTier::Report,
            RuleId::CiWorkflowSabotage => RuleTier::Guard,
            RuleId::RunnerConfigTamper => RuleTier::Guard,
            RuleId::SnapshotIntegrity => RuleTier::Guard,
            RuleId::ParametrizeRowDrop => RuleTier::Guard,
            RuleId::PolicyTamper => RuleTier::Guard,
            RuleId::BlanketSuppression => RuleTier::Report,
        }
    }
}

/// An integrity finding produced by the rules engine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Finding {
    /// Blake3-derived 12-char fingerprint for stable waiver matching.
    pub id: String,
    pub rule_id: RuleId,
    pub tier: RuleTier,
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

/// Review and waiver attribution mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ReviewMode {
    /// Attributed acknowledgement (`proctor: ack <id>`) accepted from PR author or collaborator.
    #[default]
    Ack,
    /// Strict requirement: must be accepted by a second authenticated collaborator, not author.
    SecondReviewer,
    /// Waivers disabled.
    Off,
}

/// Authorized waivers and acknowledgements for finding suppression.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct WaiverSet {
    /// Waivers from authenticated non-author maintainers (clears Guard & Report tiers).
    #[serde(default)]
    pub maintainer_waivers: std::collections::HashSet<String>,
    /// Attributed acknowledgements from PR authors (clears Report tier only).
    #[serde(default)]
    pub author_acks: std::collections::HashSet<String>,
}

impl WaiverSet {
    /// Creates a WaiverSet from maintainer waivers only.
    pub fn from_maintainers(maintainer_waivers: std::collections::HashSet<String>) -> Self {
        Self {
            maintainer_waivers,
            author_acks: std::collections::HashSet::new(),
        }
    }

    /// Evaluates if a finding is suppressed.
    /// Guard findings (CI sabotage, test demolition, policy tamper) can ONLY be suppressed by maintainer waivers.
    /// Author acknowledgements can ONLY suppress Report findings (spec evolution, flakiness quarantine).
    pub fn is_suppressed(&self, finding: &Finding) -> bool {
        if self.maintainer_waivers.contains(&finding.id) {
            return true;
        }
        if finding.tier == RuleTier::Report && self.author_acks.contains(&finding.id) {
            return true;
        }
        false
    }
}

impl From<std::collections::HashSet<String>> for WaiverSet {
    fn from(set: std::collections::HashSet<String>) -> Self {
        Self::from_maintainers(set)
    }
}

/// Structured JSON scan report format.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanReport {
    pub schema_version: String,
    pub verdict: Verdict,
    pub findings: Vec<Finding>,
}

/// Repository configuration policy, loaded from upstream protected base branch or proctor.toml.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Policy {
    #[serde(default = "default_allow_mocks")]
    pub allow_mocks: Vec<String>,
    #[serde(default = "default_ignored_paths")]
    pub ignored_paths: Vec<String>,
    #[serde(default = "default_true")]
    pub enforce_ci_tampering: bool,
    #[serde(default)]
    pub strict_mode: bool,
    #[serde(default = "default_true")]
    pub skip_demote_to_review: bool,
    #[serde(default)]
    pub review_mode: ReviewMode,
    /// When true (agent-loop / eval policy mode), treat pre-existing assertions on base
    /// as the immutable spec: any non-orphaned modification or deletion becomes Guard.
    #[serde(default)]
    pub protect_existing_tests: bool,
}

fn default_true() -> bool {
    true
}

fn default_allow_mocks() -> Vec<String> {
    vec!["app.clients.*".to_string(), "tests.fixtures.*".to_string()]
}

fn default_ignored_paths() -> Vec<String> {
    vec!["vendor/**".to_string(), "node_modules/**".to_string()]
}

impl Default for Policy {
    fn default() -> Self {
        Self {
            allow_mocks: default_allow_mocks(),
            ignored_paths: default_ignored_paths(),
            enforce_ci_tampering: true,
            strict_mode: false,
            skip_demote_to_review: true,
            review_mode: ReviewMode::default(),
            protect_existing_tests: false,
        }
    }
}

impl Policy {
    /// Parse policy from TOML string.
    pub fn from_toml(s: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(s)
    }

    /// Load policy from a TOML file path.
    pub fn from_file<P: AsRef<std::path::Path>>(path: P) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let policy: Policy = toml::from_str(&content)?;
        Ok(policy)
    }

    /// Automatically discover proctor.toml or .proctor.toml in current directory or repo root.
    pub fn discover() -> Self {
        for candidate in &["proctor.toml", ".proctor.toml"] {
            let path = std::path::Path::new(candidate);
            if path.exists() {
                if let Ok(policy) = Self::from_file(path) {
                    return policy;
                }
            }
        }
        Self::default()
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

    #[test]
    fn test_policy_from_toml() {
        let toml_str = r#"
allow_mocks = ["app.services.*", "infra.*"]
ignored_paths = ["dist/**"]
strict_mode = true
review_mode = "second_reviewer"
"#;
        let policy = Policy::from_toml(toml_str).unwrap();
        assert_eq!(policy.allow_mocks.len(), 2);
        assert!(policy.strict_mode);
        assert!(policy.enforce_ci_tampering); // default true
        assert!(policy.skip_demote_to_review); // default true
        assert_eq!(policy.review_mode, ReviewMode::SecondReviewer);
    }

    #[test]
    fn test_rule_tiers() {
        assert_eq!(RuleId::CiWorkflowSabotage.default_tier(), RuleTier::Guard);
        assert_eq!(RuleId::PolicyTamper.default_tier(), RuleTier::Guard);
        assert_eq!(RuleId::AssertionLock.default_tier(), RuleTier::Report);
    }
}
