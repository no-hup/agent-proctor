use proctor_core::{Finding, RuleId, Verdict};
use serde_json::json;

/// Converts a slice of findings into a SARIF 2.1.0 JSON object.
pub fn generate_sarif(findings: &[Finding]) -> serde_json::Value {
    let rules = vec![
        RuleId::AssertionLock,
        RuleId::ShadowMock,
        RuleId::CiWorkflowSabotage,
        RuleId::RunnerConfigTamper,
        RuleId::SnapshotIntegrity,
        RuleId::ParametrizeRowDrop,
    ];

    let sarif_rules: Vec<serde_json::Value> = rules
        .iter()
        .map(|r| {
            json!({
                "id": r.code(),
                "name": format!("{:?}", r),
                "shortDescription": {
                    "text": r.title()
                },
                "defaultConfiguration": {
                    "level": "error"
                }
            })
        })
        .collect();

    let sarif_results: Vec<serde_json::Value> = findings
        .iter()
        .map(|f| {
            let level = match &f.verdict {
                Verdict::Block { .. } => "error",
                Verdict::Review { .. } => "warning",
                Verdict::Pass => "note",
            };

            let mut location = json!({
                "physicalLocation": {
                    "artifactLocation": {
                        "uri": f.file_path
                    }
                }
            });

            if let Some(line) = f.line_number {
                location["physicalLocation"]["region"] = json!({
                    "startLine": line
                });
            }

            json!({
                "ruleId": f.rule_id.code(),
                "level": level,
                "message": {
                    "text": f.message
                },
                "locations": [location],
                "partialFingerprints": {
                    "findingId/v1": f.id
                }
            })
        })
        .collect();

    json!({
        "$schema": "https://json.schemastore.org/sarif-2.1.0.json",
        "version": "2.1.0",
        "runs": [
            {
                "tool": {
                    "driver": {
                        "name": "agent-proctor",
                        "version": "0.1.0",
                        "informationUri": "https://github.com/agent-proctor/agent-proctor",
                        "rules": sarif_rules
                    }
                },
                "results": sarif_results
            }
        ]
    })
}
