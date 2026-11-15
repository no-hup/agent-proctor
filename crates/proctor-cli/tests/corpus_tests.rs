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

fn collect_corpus_case_dirs(dir: &Path, case_dirs: &mut Vec<PathBuf>) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if path.join("expected.json").exists() {
                    case_dirs.push(path.clone());
                } else {
                    collect_corpus_case_dirs(&path, case_dirs);
                }
            }
        }
    }
}

#[test]
fn test_corpus_fixtures_match_expected_verdicts() {
    let corpus_root = get_corpus_dir();
    assert!(
        corpus_root.exists(),
        "Corpus directory not found at: {}",
        corpus_root.display()
    );

    let mut case_dirs = Vec::new();
    collect_corpus_case_dirs(&corpus_root, &mut case_dirs);
    case_dirs.sort();

    assert!(
        case_dirs.len() >= 7,
        "Expected at least 7 corpus cases, found {}",
        case_dirs.len()
    );

    for case_dir in case_dirs {
        let rel_path = case_dir
            .strip_prefix(&corpus_root)
            .unwrap_or(&case_dir)
            .display()
            .to_string();

        let expected_path = case_dir.join("expected.json");
        let expected_json_str = fs::read_to_string(&expected_path)
            .unwrap_or_else(|_| panic!("Missing expected.json in {}", case_dir.display()));
        let expected: serde_json::Value = serde_json::from_str(&expected_json_str)
            .unwrap_or_else(|_| panic!("Malformed expected.json in {}", case_dir.display()));

        // Go fixtures are documented for Phase 2 Tree-sitter Go integration
        if expected.get("language").and_then(|v| v.as_str()) == Some("go") {
            assert!(case_dir.join("base_test.go").exists());
            assert!(case_dir.join("head_test.go").exists());
            assert_eq!(expected["rule_id"].as_str().unwrap(), "PROCTOR-006");
            continue;
        }

        let expected_verdict_str = expected["verdict"].as_str().unwrap();

        let mut base_files = HashMap::new();
        let mut head_files = HashMap::new();

        // 1. Check Python test / app files
        if case_dir.join("base_test.py").exists() {
            let base_test = fs::read_to_string(case_dir.join("base_test.py")).unwrap();
            let head_test = fs::read_to_string(case_dir.join("head_test.py")).unwrap();
            base_files.insert("tests/test_mod.py".to_string(), base_test);
            head_files.insert("tests/test_mod.py".to_string(), head_test);
        }
        if case_dir.join("base_app.py").exists() {
            let base_app = fs::read_to_string(case_dir.join("base_app.py")).unwrap();
            let head_app = fs::read_to_string(case_dir.join("head_app.py")).unwrap();
            base_files.insert("app/mod.py".to_string(), base_app);
            head_files.insert("app/mod.py".to_string(), head_app);
        }

        // 2. Check JavaScript test files
        if case_dir.join("base.test.js").exists() {
            let base_test = fs::read_to_string(case_dir.join("base.test.js")).unwrap();
            let head_test = fs::read_to_string(case_dir.join("head.test.js")).unwrap();
            base_files.insert("tests/auth.test.js".to_string(), base_test);
            head_files.insert("tests/auth.test.js".to_string(), head_test);
        }

        // 3. Check CI workflow files
        if case_dir.join("base.yml").exists() {
            let base_yml = fs::read_to_string(case_dir.join("base.yml")).unwrap();
            let head_yml = fs::read_to_string(case_dir.join("head.yml")).unwrap();
            base_files.insert(".github/workflows/ci.yml".to_string(), base_yml);
            head_files.insert(".github/workflows/ci.yml".to_string(), head_yml);
        }

        assert!(
            !base_files.is_empty() || !head_files.is_empty(),
            "No test files found in {}",
            case_dir.display()
        );

        let diff = SemanticDiff::build(&base_files, &head_files)
            .unwrap_or_else(|e| panic!("SemanticDiff failed for {}: {}", rel_path, e));

        let policy = Policy::default();
        let empty_waivers = WaiverSet::default();
        let findings = evaluate_rules(&diff, &policy, &empty_waivers);

        if expected_verdict_str == "pass" {
            // Clean cases MUST produce ZERO findings (protects precision)
            assert!(
                findings.is_empty(),
                "Clean corpus case '{}' produced unexpected findings: {:?}",
                rel_path,
                findings
            );
        } else {
            // Attack cases MUST produce expected finding
            let expected_rule_code = expected["rule_id"].as_str().unwrap();
            let expected_tier_str = expected["tier"].as_str().unwrap();

            assert!(
                !findings.is_empty(),
                "Attack corpus case '{}' produced 0 findings! Expected {}",
                rel_path,
                expected_rule_code
            );

            let matching_finding = findings.iter().find(|f| f.rule_id.code() == expected_rule_code);
            assert!(
                matching_finding.is_some(),
                "Corpus case '{}' did not produce expected rule code {}. Actual findings: {:?}",
                rel_path,
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
                rel_path, expected_tier, finding.tier
            );

            match expected_verdict_str {
                "block" => assert!(
                    matches!(finding.verdict, Verdict::Block { .. }),
                    "Expected Verdict::Block for {}",
                    rel_path
                ),
                "review" => assert!(
                    matches!(finding.verdict, Verdict::Review { .. }),
                    "Expected Verdict::Review for {}",
                    rel_path
                ),
                other => panic!("Unknown expected verdict: {}", other),
            }
        }
    }
}
