use proctor_core::{Policy, RuleTier, Verdict, WaiverSet};
use proctor_diff::SemanticDiff;
use proctor_rules::evaluate_rules;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

fn get_corpus_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("corpus")
}

#[test]
fn test_corpus_fixtures_match_expected_verdicts() {
    let corpus_root = get_corpus_dir();
    assert!(
        corpus_root.exists(),
        "Corpus directory not found at: {}",
        corpus_root.display()
    );

    let cases = vec![
        (
            "ci/workflow_sabotage",
            "base.yml",
            "head.yml",
            ".github/workflows/ci.yml",
        ),
        (
            "javascript/jest_skip",
            "base.test.js",
            "head.test.js",
            "tests/auth.test.js",
        ),
        (
            "python/assertion_weakened",
            "base_test.py",
            "head_test.py",
            "tests/test_discount.py",
        ),
        (
            "python/shadow_mock",
            "base_test.py",
            "head_test.py",
            "tests/test_payment.py",
        ),
    ];

    for (case_rel_path, base_filename, head_filename, repo_dest_path) in cases {
        let case_dir = corpus_root.join(case_rel_path);
        assert!(case_dir.exists(), "Case directory missing: {}", case_dir.display());

        let expected_path = case_dir.join("expected.json");
        let expected_json_str = fs::read_to_string(&expected_path)
            .unwrap_or_else(|_| panic!("Missing expected.json in {}", case_dir.display()));
        let expected: serde_json::Value = serde_json::from_str(&expected_json_str)
            .unwrap_or_else(|_| panic!("Malformed expected.json in {}", case_dir.display()));

        let expected_rule_code = expected["rule_id"].as_str().unwrap();
        let expected_tier_str = expected["tier"].as_str().unwrap();
        let expected_verdict_str = expected["verdict"].as_str().unwrap();

        let base_content = fs::read_to_string(case_dir.join(base_filename))
            .unwrap_or_else(|_| panic!("Missing {} in {}", base_filename, case_dir.display()));
        let head_content = fs::read_to_string(case_dir.join(head_filename))
            .unwrap_or_else(|_| panic!("Missing {} in {}", head_filename, case_dir.display()));

        let mut base_files = HashMap::new();
        base_files.insert(repo_dest_path.to_string(), base_content);

        let mut head_files = HashMap::new();
        head_files.insert(repo_dest_path.to_string(), head_content);

        let diff = SemanticDiff::build(&base_files, &head_files)
            .unwrap_or_else(|e| panic!("SemanticDiff failed for {}: {}", case_rel_path, e));

        let policy = Policy::default();
        let empty_waivers = WaiverSet::default();
        let findings = evaluate_rules(&diff, &policy, &empty_waivers);

        assert!(
            !findings.is_empty(),
            "Corpus case '{}' produced 0 findings! Expected {}",
            case_rel_path,
            expected_rule_code
        );

        let matching_finding = findings.iter().find(|f| f.rule_id.code() == expected_rule_code);
        assert!(
            matching_finding.is_some(),
            "Corpus case '{}' did not produce expected rule code {}. Actual findings: {:?}",
            case_rel_path,
            expected_rule_code,
            findings
        );

        let finding = matching_finding.unwrap();

        let expected_tier = match expected_tier_str {
            "guard" => RuleTier::Guard,
            "report" => RuleTier::Report,
            other => panic!("Unknown tier in expected.json: {}", other),
        };
        assert_eq!(
            finding.tier, expected_tier,
            "Corpus case '{}' tier mismatch: expected {:?}, got {:?}",
            case_rel_path, expected_tier, finding.tier
        );

        match expected_verdict_str {
            "block" => assert!(
                matches!(finding.verdict, Verdict::Block { .. }),
                "Expected Verdict::Block for {}",
                case_rel_path
            ),
            "review" => assert!(
                matches!(finding.verdict, Verdict::Review { .. }),
                "Expected Verdict::Review for {}",
                case_rel_path
            ),
            other => panic!("Unknown expected verdict: {}", other),
        }
    }

    // Verify Go table-driven case fixture exists and is documented for Phase 2 Tree-sitter Go integration
    let go_case_dir = corpus_root.join("go/table_driven_row_drop");
    assert!(go_case_dir.exists());
    assert!(go_case_dir.join("base_test.go").exists());
    assert!(go_case_dir.join("head_test.go").exists());
    let go_expected: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(go_case_dir.join("expected.json")).unwrap()
    ).unwrap();
    assert_eq!(go_expected["rule_id"].as_str().unwrap(), "PROCTOR-006");
}
