use proctor_parse::{
    parse_go_source_file, parse_go_test_file, parse_javascript_source_file,
    parse_javascript_test_file, parse_python_source_file, parse_python_test_file, ParsedSourceFile,
    ParsedTest, ParsedTestFile,
};
use std::collections::{HashMap, HashSet};
use std::path::Path;

pub mod linkage;
pub use linkage::*;

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
    pub all_base_symbols: HashSet<String>,
    pub all_deleted_symbols: HashSet<String>,
    /// Bare names whose definition was added, removed, or edited in this PR (pure renames excluded).
    pub changed_symbols: HashSet<String>,
    /// Intra-file renames detected by body similarity: old name -> new name.
    pub renamed_symbols: HashMap<String, String>,
    pub raw_base_files: HashMap<String, String>,
    pub raw_head_files: HashMap<String, String>,
    pub base_harness_mocks: Vec<proctor_parse::ParsedMock>,
    pub head_harness_mocks: Vec<proctor_parse::ParsedMock>,
    pub base_fixture_skips: Vec<(String, String)>,
    pub head_fixture_skips: Vec<(String, String)>,
    pub test_imports: HashMap<String, Vec<String>>,
    pub parse_errors: Vec<(String, String)>,
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
        let mut parse_errors = Vec::new();

        // 1. Parse base files
        for (path, content) in base_files {
            if is_test_file(path) {
                match parse_test(path, content) {
                    Ok(parsed) => {
                        base_tests_by_file.insert(path.clone(), parsed);
                    }
                    Err(e) => {
                        parse_errors.push((path.clone(), format!("Base test parse error: {}", e)));
                    }
                }
            } else if is_source_file(path) {
                match parse_source(path, content) {
                    Ok(parsed) => {
                        base_sources.insert(path.clone(), parsed);
                    }
                    Err(e) => {
                        parse_errors
                            .push((path.clone(), format!("Base source parse error: {}", e)));
                    }
                }
            }
        }

        // 2. Parse head files
        for (path, content) in head_files {
            if is_test_file(path) {
                match parse_test(path, content) {
                    Ok(parsed) => {
                        if parsed.has_syntax_error {
                            parse_errors.push((
                                path.clone(),
                                "Syntax error detected in test file".to_string(),
                            ));
                        }
                        head_tests_by_file.insert(path.clone(), parsed);
                    }
                    Err(e) => {
                        parse_errors.push((path.clone(), format!("Head test parse error: {}", e)));
                    }
                }
            } else if is_source_file(path) {
                match parse_source(path, content) {
                    Ok(parsed) => {
                        if parsed.has_syntax_error {
                            parse_errors.push((
                                path.clone(),
                                "Syntax error detected in source file".to_string(),
                            ));
                        }
                        head_sources.insert(path.clone(), parsed);
                    }
                    Err(e) => {
                        parse_errors
                            .push((path.clone(), format!("Head source parse error: {}", e)));
                    }
                }
            }
        }

        // 3. Compute all deleted production symbols (both bare and qualified by module)
        let mut base_symbols = HashSet::new();
        for (src_path, src) in &base_sources {
            let mod_name = src_path
                .trim_end_matches(".py")
                .trim_end_matches(".js")
                .trim_end_matches(".ts")
                .trim_end_matches(".tsx")
                .trim_end_matches(".go")
                .replace('/', ".");
            for sym in &src.defined_symbols {
                base_symbols.insert(sym.clone());
                base_symbols.insert(format!("{}.{}", mod_name, sym));
            }
        }

        let mut head_symbols = HashSet::new();
        for (src_path, src) in &head_sources {
            let mod_name = src_path
                .trim_end_matches(".py")
                .trim_end_matches(".js")
                .trim_end_matches(".ts")
                .trim_end_matches(".tsx")
                .trim_end_matches(".go")
                .replace('/', ".");
            for sym in &src.defined_symbols {
                head_symbols.insert(sym.clone());
                head_symbols.insert(format!("{}.{}", mod_name, sym));
            }
        }

        let (changed_symbols, renamed_symbols) =
            compute_symbol_changes(&base_sources, &head_sources);

        // A name only counts as deleted if its bare name is gone everywhere and it was not renamed.
        // (Moving `foo` to another file deletes `old.mod.foo` but `foo` still exists.)
        let all_deleted_symbols: HashSet<String> = base_symbols
            .difference(&head_symbols)
            .filter(|sym| {
                let bare = sym.rsplit('.').next().unwrap_or(sym);
                !head_symbols.contains(bare) && !renamed_symbols.contains_key(bare)
            })
            .cloned()
            .collect();

        // 4. Two-Pass Test Pairing
        let mut test_imports = HashMap::new();
        let mut all_base_tests = Vec::new();
        let mut base_harness_mocks = Vec::new();
        let mut base_fixture_skips = Vec::new();
        for (path, f) in base_tests_by_file {
            test_imports.insert(path.clone(), f.imports.clone());
            all_base_tests.extend(f.tests);
            base_harness_mocks.extend(f.harness_mocks);
            for fn_name in f.fixture_skips {
                base_fixture_skips.push((path.clone(), fn_name));
            }
        }

        let mut all_head_tests = Vec::new();
        let mut head_harness_mocks = Vec::new();
        let mut head_fixture_skips = Vec::new();
        for (path, f) in head_tests_by_file {
            test_imports.insert(path.clone(), f.imports.clone());
            all_head_tests.extend(f.tests);
            head_harness_mocks.extend(f.harness_mocks);
            for fn_name in f.fixture_skips {
                head_fixture_skips.push((path.clone(), fn_name));
            }
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
                        let overlap =
                            compute_assertion_overlap(&b_test.assertions, &h_test.assertions);
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

        // Pass 3: Same-file, same-scope symbol rematch (when a test is renamed AND its
        // assertions co-evolve). Paired tests still go through PROCTOR-001's assertion lattice,
        // so gutting/weakening a renamed test remains flagged.
        for (b_idx, b_test) in all_base_tests.iter().enumerate() {
            if matched_base_indices.contains(&b_idx) || b_test.body_symbols.is_empty() {
                continue;
            }
            let mut best_match: Option<(usize, f64)> = None;
            for (h_idx, h_test) in all_head_tests.iter().enumerate() {
                if matched_head_indices.contains(&h_idx)
                    || h_test.assertions.is_empty()
                    || b_test.id.file_path != h_test.id.file_path
                    || b_test.id.scope_hierarchy != h_test.id.scope_hierarchy
                {
                    continue;
                }
                let b_syms: HashSet<&str> =
                    b_test.body_symbols.iter().map(String::as_str).collect();
                let h_syms: HashSet<&str> =
                    h_test.body_symbols.iter().map(String::as_str).collect();
                let shared = b_syms.intersection(&h_syms).count();
                let union = b_syms.union(&h_syms).count();
                if shared >= 1 && union > 0 {
                    let jaccard = shared as f64 / union as f64;
                    if jaccard >= 0.50 && best_match.is_none_or(|(_, s)| jaccard > s) {
                        best_match = Some((h_idx, jaccard));
                    }
                }
            }
            if let Some((h_idx, _)) = best_match {
                paired_tests.push(TestPair {
                    base_test: b_test.clone(),
                    head_test: all_head_tests[h_idx].clone(),
                });
                matched_base_indices.insert(b_idx);
                matched_head_indices.insert(h_idx);
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
            all_base_symbols: base_symbols,
            all_deleted_symbols,
            changed_symbols,
            renamed_symbols,
            raw_base_files: base_files.clone(),
            raw_head_files: head_files.clone(),
            base_harness_mocks,
            head_harness_mocks,
            base_fixture_skips,
            head_fixture_skips,
            test_imports,
            parse_errors,
        })
    }

    /// Evaluates if an assertion modification is justified by orphaned symbol references.
    /// Strict semantics: require ALL referenced production symbols in the assertion to be deleted.
    pub fn is_reference_orphaned(&self, referenced_symbols: &[String]) -> bool {
        if referenced_symbols.is_empty() {
            return false;
        }

        // Filter symbols that were actually defined in base production code
        let prod_refs: Vec<&String> = referenced_symbols
            .iter()
            .filter(|sym| {
                self.all_base_symbols.contains(*sym)
                    || self
                        .all_base_symbols
                        .iter()
                        .any(|b| b.ends_with(&format!(".{}", sym)))
            })
            .collect();

        // If none of the symbols reference production code, it cannot be justified as orphaned
        if prod_refs.is_empty() {
            return false;
        }

        // Strict: ALL referenced production symbols must be deleted (renames never count)
        prod_refs.iter().all(|sym| {
            self.all_deleted_symbols.contains(*sym)
                || self
                    .all_deleted_symbols
                    .iter()
                    .any(|d| d.ends_with(&format!(".{}", sym)))
        })
    }

    /// Evaluates linkage using an explicit LinkageStrategy black box.
    pub fn test_links_changed_code_with(
        &self,
        base: &ParsedTest,
        head: &ParsedTest,
        strategy: &dyn LinkageStrategy,
    ) -> LinkageVerdict {
        strategy.check_linkage(base, head, self)
    }

    /// True if the test (before or after the PR) references a production definition changed in this PR.
    pub fn test_links_changed_code(&self, base: &ParsedTest, head: &ParsedTest) -> bool {
        self.test_links_changed_code_with(base, head, &BareTokenLinker)
            .is_linked()
    }
}

const RENAME_SIMILARITY: f32 = 0.75;
const RENAME_MIN_TOKENS: usize = 8;

/// Ordered tokens: identifiers/numbers as words, every other non-space char (operators) as its own token.
/// The symbol's own name is normalized so recursion survives a rename.
fn body_tokens(body: &str, own_name: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut word = String::new();
    let flush = |w: &mut String, out: &mut Vec<String>| {
        if !w.is_empty() {
            out.push(if w == own_name {
                "$self".into()
            } else {
                std::mem::take(w)
            });
            w.clear();
        }
    };
    for c in body.chars() {
        if c.is_alphanumeric() || c == '_' {
            word.push(c);
        } else {
            flush(&mut word, &mut out);
            if !c.is_whitespace() {
                out.push(c.to_string());
            }
        }
    }
    flush(&mut word, &mut out);
    out
}

/// Dice coefficient over ordered token bigrams (multiset), so order and operators count.
fn bigram_similarity(a: &[String], b: &[String]) -> f32 {
    let grams = |t: &[String]| {
        let mut m: HashMap<(String, String), usize> = HashMap::new();
        for w in t.windows(2) {
            *m.entry((w[0].clone(), w[1].clone())).or_default() += 1;
        }
        m
    };
    let (ga, gb) = (grams(a), grams(b));
    let total = a.len().saturating_sub(1) + b.len().saturating_sub(1);
    if total == 0 {
        return 0.0;
    }
    let shared: usize = ga
        .iter()
        .map(|(k, n)| (*n).min(*gb.get(k).unwrap_or(&0)))
        .sum();
    2.0 * shared as f32 / total as f32
}

/// Per changed file: which definitions changed, and which deletions are really renames.
fn compute_symbol_changes(
    base_sources: &HashMap<String, ParsedSourceFile>,
    head_sources: &HashMap<String, ParsedSourceFile>,
) -> (HashSet<String>, HashMap<String, String>) {
    let empty = HashMap::new();
    let mut changed = HashSet::new();
    let mut renamed = HashMap::new();

    let paths: HashSet<&String> = base_sources.keys().chain(head_sources.keys()).collect();
    for path in paths {
        let base = base_sources
            .get(path)
            .map(|s| &s.symbol_bodies)
            .unwrap_or(&empty);
        let head = head_sources
            .get(path)
            .map(|s| &s.symbol_bodies)
            .unwrap_or(&empty);

        for (name, body) in base {
            if head.get(name).is_some_and(|h| h != body) {
                changed.insert(name.clone());
            }
        }

        let mut added: Vec<&String> = head.keys().filter(|n| !base.contains_key(*n)).collect();
        added.sort();
        let mut deleted: Vec<&String> = base.keys().filter(|n| !head.contains_key(*n)).collect();
        deleted.sort();

        for old in deleted {
            let old_tokens = body_tokens(&base[old], old);
            let mut scored: Vec<(usize, f32)> = if old_tokens.len() < RENAME_MIN_TOKENS {
                Vec::new() // stubs like `raise NotImplementedError` match anything
            } else {
                added
                    .iter()
                    .enumerate()
                    .map(|(i, new)| {
                        (
                            i,
                            bigram_similarity(&old_tokens, &body_tokens(&head[*new], new)),
                        )
                    })
                    .filter(|(_, sim)| *sim >= RENAME_SIMILARITY)
                    .collect()
            };
            scored.sort_by(|a, b| b.1.total_cmp(&a.1));
            let tie = scored.len() > 1 && scored[0].1 == scored[1].1;
            match scored.first().filter(|_| !tie) {
                Some(&(i, sim)) => {
                    let new = added.remove(i);
                    if sim < 1.0 {
                        changed.insert(old.clone());
                        changed.insert(new.clone());
                    }
                    renamed.insert(old.clone(), new.clone());
                }
                None => {
                    changed.insert(old.clone());
                }
            }
        }
        changed.extend(added.into_iter().cloned());
    }
    (changed, renamed)
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

pub fn is_test_file(path: &str) -> bool {
    let p = Path::new(path);
    let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("");
    if !["py", "js", "ts", "jsx", "tsx", "mjs", "cjs", "go"].contains(&ext) {
        return false;
    }

    let file_name = p
        .file_name()
        .and_then(|f| f.to_str())
        .unwrap_or("")
        .to_lowercase();
    let in_test_dir = p.components().any(|c| {
        let s = c.as_os_str().to_string_lossy().to_lowercase();
        s == "tests" || s == "test" || s == "__tests__"
    });

    if ext == "py" {
        file_name.starts_with("test_")
            || file_name.ends_with("_test.py")
            || (in_test_dir && !file_name.starts_with("__"))
    } else if ext == "go" {
        file_name.ends_with("_test.go")
    } else {
        file_name.ends_with(".test.js")
            || file_name.ends_with(".test.ts")
            || file_name.ends_with(".test.jsx")
            || file_name.ends_with(".test.tsx")
            || file_name.ends_with(".test.mjs")
            || file_name.ends_with(".test.cjs")
            || file_name.ends_with(".spec.js")
            || file_name.ends_with(".spec.ts")
            || file_name.ends_with(".spec.jsx")
            || file_name.ends_with(".spec.tsx")
            || file_name.ends_with(".spec.mjs")
            || file_name.ends_with(".spec.cjs")
            || in_test_dir
    }
}

pub fn is_source_file(path: &str) -> bool {
    let p = Path::new(path);
    let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("");
    ["py", "js", "ts", "jsx", "tsx", "mjs", "cjs", "go"].contains(&ext) && !is_test_file(path)
}

fn parse_test(path: &str, content: &str) -> Result<ParsedTestFile, String> {
    if path.ends_with(".py") {
        parse_python_test_file(path, content)
    } else if path.ends_with("_test.go") {
        parse_go_test_file(path, content)
    } else if path.ends_with(".ts") || path.ends_with(".tsx") {
        parse_javascript_test_file(path, content, true)
    } else {
        parse_javascript_test_file(path, content, false)
    }
}

fn parse_source(path: &str, content: &str) -> Result<ParsedSourceFile, String> {
    if path.ends_with(".py") {
        parse_python_source_file(path, content)
    } else if path.ends_with(".go") {
        parse_go_source_file(path, content)
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
        assert!(diff.all_deleted_symbols.contains("legacy_token"));

        // The deleted assertion referenced `legacy_token`, which was deleted from production code
        let referenced = vec!["user".to_string(), "legacy_token".to_string()];
        assert!(diff.is_reference_orphaned(&referenced));

        // If it references a symbol that was NOT deleted (User), strict orphan checks require all to be deleted
        let non_orphaned = vec!["User".to_string(), "legacy_token".to_string()];
        assert!(!diff.is_reference_orphaned(&non_orphaned));
    }

    #[test]
    fn test_rename_is_not_deletion_and_move_is_not_deletion() {
        let base_src = "def legacy_total(items):\n    s = 0\n    for i in items:\n        s += i.price * i.qty\n    return s\n\ndef helper():\n    return 1\n";
        let head_src = "def total(items):\n    s = 0\n    for i in items:\n        s += i.price * i.qty\n    return s\n";
        let other_head = "def helper():\n    return 1\n";

        let base_files = HashMap::from([("app/cart.py".to_string(), base_src.to_string())]);
        let head_files = HashMap::from([
            ("app/cart.py".to_string(), head_src.to_string()),
            ("app/util.py".to_string(), other_head.to_string()),
        ]);
        let diff = SemanticDiff::build(&base_files, &head_files).unwrap();

        assert_eq!(
            diff.renamed_symbols.get("legacy_total").map(String::as_str),
            Some("total")
        );
        assert!(!diff.is_reference_orphaned(&["legacy_total".to_string()]));
        // Pure rename is not a behavior change.
        assert!(!diff.changed_symbols.contains("total"));
        // Moved to another file: not deleted.
        assert!(!diff.is_reference_orphaned(&["helper".to_string()]));
    }

    #[test]
    fn test_rename_rejects_stubs_ties_and_operator_flips() {
        let sim = |a: &str, b: &str| bigram_similarity(&body_tokens(a, "x"), &body_tokens(b, "y"));
        // Operator flip is visible now (sets of words would call these identical).
        assert!(
            sim(
                "total = a * b\nreturn total * rate",
                "total = a + b\nreturn total * rate"
            ) < 1.0
        );

        // Stub deleted + unrelated stub added: not a rename, so the old name stays deleted.
        let base = HashMap::from([(
            "app/m.py".to_string(),
            "def old_stub():\n    raise NotImplementedError\n".to_string(),
        )]);
        let head = HashMap::from([(
            "app/m.py".to_string(),
            "def new_stub():\n    raise NotImplementedError\n".to_string(),
        )]);
        let diff = SemanticDiff::build(&base, &head).unwrap();
        assert!(diff.renamed_symbols.is_empty());
        assert!(diff.is_reference_orphaned(&["old_stub".to_string()]));

        // Two identical candidates: ambiguous, not a rename.
        let body = "    s = 0\n    for i in items:\n        s += i.price * i.qty\n    return s\n";
        let base = HashMap::from([(
            "app/m.py".to_string(),
            format!("def legacy(items):\n{body}"),
        )]);
        let head = HashMap::from([(
            "app/m.py".to_string(),
            format!("def a(items):\n{body}\ndef b(items):\n{body}"),
        )]);
        let diff = SemanticDiff::build(&base, &head).unwrap();
        assert!(diff.renamed_symbols.is_empty());
    }

    #[test]
    fn test_accurate_test_file_heuristics() {
        assert!(is_test_file("tests/test_auth.py"));
        assert!(is_test_file("tests/unit/test_calc.py"));
        assert!(is_test_file("app/foo_test.py"));
        assert!(is_test_file("src/components/button.test.tsx"));
        assert!(is_test_file("src/__tests__/utils.js"));

        // Production files containing "test" or "spec" must NOT be treated as tests
        assert!(!is_test_file("src/latest.py"));
        assert!(!is_test_file("app/contestation.py"));
        assert!(!is_test_file("src/special.ts"));
        assert!(!is_test_file("lib/inspector.js"));
        assert!(!is_test_file("pkg/latest.go"));
        assert!(is_test_file("pkg/auth/token_test.go"));
        assert!(is_source_file("pkg/auth/token.go"));
    }

    #[test]
    fn test_go_table_driven_test_diff_pairs() {
        let base_code = r#"
package auth_test
import "testing"
func TestAuth(t *testing.T) {
    tests := []struct{ name string; valid bool }{
        {name: "valid", valid: true},
        {name: "invalid", valid: false},
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
        {name: "valid", valid: true},
    }
    for _, tc := range tests {
        if !tc.valid { t.Fail() }
    }
}
"#;
        let base = HashMap::from([("pkg/auth_test.go".to_string(), base_code.to_string())]);
        let head = HashMap::from([("pkg/auth_test.go".to_string(), head_code.to_string())]);
        let diff = SemanticDiff::build(&base, &head).unwrap();
        assert_eq!(diff.paired_tests.len(), 1);
        let pair = &diff.paired_tests[0];
        assert_eq!(pair.base_test.parametrized_hashes.len(), 2);
        assert_eq!(pair.head_test.parametrized_hashes.len(), 1);
    }

    #[test]
    fn test_same_file_rename_with_coevolved_assertion_pairs() {
        let base = HashMap::from([(
            "src/fog.test.ts".to_string(),
            "describe('fog', () => {\n  it('eases from 255 to 0', () => {\n    expect(fogValueAt(1000, 999)).toBe(255);\n  });\n});\n".to_string(),
        )]);
        let head = HashMap::from([(
            "src/fog.test.ts".to_string(),
            "describe('fog', () => {\n  it('eases from lift start value to 0', () => {\n    expect(fogValueAt(1000, 999)).toBe(FOG_START);\n  });\n});\n".to_string(),
        )]);
        let diff = SemanticDiff::build(&base, &head).unwrap();
        assert!(
            diff.deleted_tests.is_empty(),
            "Renamed test should not be in deleted_tests"
        );
        assert_eq!(diff.paired_tests.len(), 1);
    }
}
