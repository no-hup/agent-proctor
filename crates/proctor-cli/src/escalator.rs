use proctor_core::{Finding, RuleId, RuleTier, Verdict};
use proctor_probe::TestVerificationResult;

/// Black-box trait governing how dynamic mutation/regression verification results
/// alter static findings in the final report.
pub trait EscalatorStrategy {
    fn apply_verification(&self, findings: &mut Vec<Finding>, res: &TestVerificationResult);
}

/// Recommended default: Evidence annotator.
/// Preserves the finding in the receipt, annotating it with proof that it reproduced the bug,
/// while promoting vacuous assertion weakenings to a hard Guard Block.
#[derive(Debug, Default, Clone, Copy)]
pub struct AnnotatorEscalator;

impl EscalatorStrategy for AnnotatorEscalator {
    fn apply_verification(&self, findings: &mut Vec<Finding>, res: &TestVerificationResult) {
        match &res.status {
            proctor_probe::VerificationStatus::Discriminating { reason } => {
                eprintln!(
                    "proctor: Escalator verified test '{}' is discriminating against base: {}",
                    res.test_id, reason
                );
                for f in findings.iter_mut() {
                    if f.file_path == res.test_file && f.rule_id == RuleId::AssertionLock {
                        f.message.push_str(&format!(
                            " [escalator: discriminating against base: {}]",
                            reason
                        ));
                    }
                }
            }
            proctor_probe::VerificationStatus::DoesNotDiscriminate => {
                eprintln!(
                    "proctor: Escalator verified test '{}' does NOT discriminate against base logic!",
                    res.test_id
                );
                for f in findings.iter_mut() {
                    if f.file_path == res.test_file && f.rule_id == RuleId::AssertionLock {
                        f.tier = RuleTier::Guard;
                        f.verdict = Verdict::Block {
                            rule: RuleId::AssertionLock,
                            evidence: format!(
                                "Escalator: Test '{}' passes on base without exercising changed logic; assertion weakening is vacuous.",
                                res.test_id
                            ),
                        };
                        f.message = format!(
                            "Dynamic probe confirmed assertion change in '{}' is vacuous and does not discriminate against base.",
                            res.test_id
                        );
                    }
                }
            }
            proctor_probe::VerificationStatus::Broken { error } => {
                eprintln!(
                    "proctor: Escalator test '{}' is broken on head: {}",
                    res.test_id, error
                );
                for f in findings.iter_mut() {
                    if f.file_path == res.test_file && f.rule_id == RuleId::AssertionLock {
                        f.tier = RuleTier::Guard;
                        f.verdict = Verdict::Block {
                            rule: RuleId::AssertionLock,
                            evidence: format!(
                                "Escalator: Test is broken on head commit: {}",
                                error
                            ),
                        };
                    }
                }
            }
            proctor_probe::VerificationStatus::MissingOnBase { error } => {
                eprintln!(
                    "proctor: Escalator test '{}' references symbols missing on base: {}",
                    res.test_id, error
                );
            }
            proctor_probe::VerificationStatus::Inconclusive { reason } => {
                eprintln!(
                    "proctor: Escalator probe inconclusive for '{}' ({}); preserving static review.",
                    res.test_id, reason
                );
            }
        }
    }
}

/// Legacy implementation: erases discriminating findings from report.
#[derive(Debug, Default, Clone, Copy)]
pub struct EraserEscalator;

impl EscalatorStrategy for EraserEscalator {
    fn apply_verification(&self, findings: &mut Vec<Finding>, res: &TestVerificationResult) {
        match &res.status {
            proctor_probe::VerificationStatus::Discriminating { reason } => {
                eprintln!(
                    "proctor: Escalator verified test '{}' is discriminating against base: {}",
                    res.test_id, reason
                );
                findings.retain(|f| {
                    !(f.file_path == res.test_file
                        && f.rule_id == RuleId::AssertionLock
                        && matches!(f.verdict, Verdict::Review { .. }))
                });
            }
            _ => {
                AnnotatorEscalator.apply_verification(findings, res);
            }
        }
    }
}
