use proctor_parse::{parse_javascript_source_file, parse_javascript_test_file, parse_python_source_file, parse_python_test_file, ParsedTest, ParsedTestFile, ParsedSourceFile};
use std::collections::{HashMap, HashSet};

/// A paired test between the base commit and head commit.
#[derive(Debug, Clone)]
pub struct TestPair {
    pub base_test: ParsedTest,
    pub head_test: ParsedTest,
}

/// The complete semantic diff between base and head.
#[derive(Debug, Clone)]
pub struct SemanticDiff {
    pub paired_tests: Vec<TestPair>,
    pub deleted_tests: Vec<ParsedTest>,
    pub new_tests: Vec<ParsedTest>,
    pub base_sources: HashMap<String, ParsedSourceFile>,
    pub head_sources: HashMap<String, ParsedSourceFile>,
    pub all_deleted_symbols: HashSet<String>,
    pub raw_base_files: HashMap<String, String>,
    pub raw_head_files: HashMap<String, String>,
    pub base_harness_mocks: Vec<proctor_parse::ParsedMock>,
    pub head_harness_mocks: Vec<proctor_parse::ParsedMock>,
}

impl SemanticDiff {
    /// Builds a semantic diff from in-memory base and head file maps.
    pub fn build(
        base_files: &HashMap<String, String>,
        head_files: &HashMap<String, String>,
    ) -> Result<Self, String> {
        let mut base_tests_by_file = HashMap::new();
        let mut head_tests_by_file = HashMap::new();
        let mut base_sources = HashMap::new();
        let mut head_sources = HashMap::new();

        // 1. Parse base files
        for (path, content) in base_files {
            if is_test_file(path) {
                if let Ok(parsed) = parse_test(path, content) {
                    base_tests_by_file.insert(path.clone(), parsed);
                }
            } else if is_source_file(path) {
                if let Ok(parsed) = parse_source(path, content) {
                    base_sources.insert(path.clone(), parsed);
                }
            }
        }

        // 2. Parse head files
        for (path, content) in head_files {
            if is_test_file(path) {
                if let Ok(parsed) = parse_test(path, content) {
                    head_tests_by_file.insert(path.clone(), parsed);
                }
            } else if is_source_file(path) {
                if let Ok(parsed) = parse_source(path, content) {
                    head_sources.insert(path.clone(), parsed);
                }
            }
        }

        // 3. Compute all deleted production symbols
        let mut base_symbols = HashSet::new();
        for src in base_sources.values() {
            for sym in &src.defined_symbols {
                base_symbols.insert(sym.clone());
            }
        }

        let mut head_symbols = HashSet::new();
        for src in head_sources.values() {
            for sym in &src.defined_symbols {
                head_symbols.insert(sym.clone());
            }
        }

        let all_deleted_symbols: HashSet<String> = base_symbols
            .difference(&head_symbols)
            .cloned()
            .collect();

        // 4. Two-Pass Test Pairing
        let mut all_base_tests = Vec::new();
        let mut base_harness_mocks = Vec::new();
        for f in base_tests_by_file.into_values() {
            all_base_tests.extend(f.tests);
            base_harness_mocks.extend(f.harness_mocks);
        }

        let mut all_head_tests = Vec::new();
        let mut head_harness_mocks = Vec::new();
        for f in head_tests_by_file.into_values() {
            all_head_tests.extend(f.tests);
            head_harness_mocks.extend(f.harness_mocks);
        }

        let mut paired_tests = Vec::new();
        let mut matched_head_indices = HashSet::new();
        let mut matched_base_indices = HashSet::new();

        // Pass 1: Exact TestId match
        for (b_idx, b_test) in all_base_tests.iter().enumerate() {
            for (h_idx, h_test) in all_head_tests.iter().enumerate() {
                if !matched_head_indices.contains(&h_idx)
                    && b_test.id.file_path == h_test.id.file_path
                    && b_test.id.scope_hierarchy == h_test.id.scope_hierarchy
                    && b_test.id.name == h_test.id.name
                {
                    paired_tests.push(TestPair {
                        base_test: b_test.clone(),
                        head_test: h_test.clone(),
                    });
                    matched_base_indices.insert(b_idx);
                    matched_head_indices.insert(h_idx);
                    break;
                }
            }
        }

        // Pass 2: Similarity match (absorbing moves and renames)
        for (b_idx, b_test) in all_base_tests.iter().enumerate() {
            if !matched_base_indices.contains(&b_idx) {
                for (h_idx, h_test) in all_head_tests.iter().enumerate() {
                    if !matched_head_indices.contains(&h_idx) {
                        let same_name = b_test.id.name == h_test.id.name;
                        let overlap = compute_assertion_overlap(&b_test.assertions, &h_test.assertions);
                        if same_name || overlap >= 0.50 {
                            paired_tests.push(TestPair {
                                base_test: b_test.clone(),
                                head_test: h_test.clone(),
                            });
                            matched_base_indices.insert(b_idx);
                            matched_head_indices.insert(h_idx);
                            break;
                        }
                    }
                }
            }
        }

        // Unmatched base tests are deleted
        let deleted_tests: Vec<ParsedTest> = all_base_tests
            .into_iter()
            .enumerate()
            .filter(|(idx, _)| !matched_base_indices.contains(idx))
            .map(|(_, t)| t)
            .collect();

        // Unmatched head tests are new
        let new_tests: Vec<ParsedTest> = all_head_tests
            .into_iter()
            .enumerate()
            .filter(|(idx, _)| !matched_head_indices.contains(idx))
            .map(|(_, t)| t)
            .collect();

        Ok(SemanticDiff {
            paired_tests,
            deleted_tests,
            new_tests,
            base_sources,
            head_sources,
            all_deleted_symbols,
            raw_base_files: base_files.clone(),
            raw_head_files: head_files.clone(),
            base_harness_mocks,
            head_harness_mocks,
        })
    }

    /// Evaluates if an assertion modification is justified by an orphaned symbol reference.
    pub fn is_reference_orphaned(&self, referenced_symbols: &[String]) -> bool {
        for sym in referenced_symbols {
            if self.all_deleted_symbols.contains(sym) {
                return true;
            }
        }
        false
    }
}

fn compute_assertion_overlap(
    base_asserts: &[proctor_parse::ParsedAssertion],
    head_asserts: &[proctor_parse::ParsedAssertion],
) -> f32 {
    if base_asserts.is_empty() && head_asserts.is_empty() {
        return 1.0;
    }
    if base_asserts.is_empty() || head_asserts.is_empty() {
        return 0.0;
    }

    let base_texts: HashSet<&str> = base_asserts.iter().map(|a| a.text.as_str()).collect();
    let head_texts: HashSet<&str> = head_asserts.iter().map(|a| a.text.as_str()).collect();

    let intersection = base_texts.intersection(&head_texts).count();
    let union = base_texts.union(&head_texts).count();

    if union == 0 {
        1.0
    } else {
        intersection as f32 / union as f32
    }
}

fn is_test_file(path: &str) -> bool {
    let p = path.to_lowercase();
    p.contains("test") || p.contains("spec")
}

fn is_source_file(path: &str) -> bool {
    (path.ends_with(".py") || path.ends_with(".js") || path.ends_with(".ts") || path.ends_with(".tsx"))
        && !is_test_file(path)
}

fn parse_test(path: &str, content: &str) -> Result<ParsedTestFile, String> {
    if path.ends_with(".py") {
        parse_python_test_file(path, content)
    } else if path.ends_with(".ts") || path.ends_with(".tsx") {
        parse_javascript_test_file(path, content, true)
    } else {
        parse_javascript_test_file(path, content, false)
    }
}

fn parse_source(path: &str, content: &str) -> Result<ParsedSourceFile, String> {
    if path.ends_with(".py") {
        parse_python_source_file(path, content)
    } else if path.ends_with(".ts") || path.ends_with(".tsx") {
        parse_javascript_source_file(path, content, true)
    } else {
        parse_javascript_source_file(path, content, false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pairing_and_orphaned_reference() {
        let base_test = r#"
def test_user_login():
    assert user.is_authenticated() == True
    assert user.legacy_token is not None
"#;
        let head_test = r#"
def test_user_login():
    assert user.is_authenticated() == True
"#;

        let base_src = r#"
class User:
    legacy_token = "abc"
"#;
        let head_src = r#"
class User:
    pass
"#;

        let mut base_files = HashMap::new();
        base_files.insert("tests/test_auth.py".to_string(), base_test.to_string());
        base_files.insert("app/user.py".to_string(), base_src.to_string());

        let mut head_files = HashMap::new();
        head_files.insert("tests/test_auth.py".to_string(), head_test.to_string());
        head_files.insert("app/user.py".to_string(), head_src.to_string());

        let diff = SemanticDiff::build(&base_files, &head_files).unwrap();
        assert_eq!(diff.paired_tests.len(), 1);
        assert_eq!(diff.all_deleted_symbols.contains("legacy_token"), true);

        // The deleted assertion referenced `legacy_token`, which was deleted from production code
        let referenced = vec!["user".to_string(), "legacy_token".to_string()];
        assert_eq!(diff.is_reference_orphaned(&referenced), true);
    }
}
