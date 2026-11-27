use proctor_core::*;
use std::collections::HashMap;

pub mod go;
pub mod javascript;
pub mod python;

pub use go::{parse_go_source_file, parse_go_test_file};
pub use javascript::{parse_javascript_source_file, parse_javascript_test_file};
pub use python::{parse_python_source_file, parse_python_test_file};

#[derive(Debug, Clone)]
pub struct ParsedTestFile {
    pub file_path: String,
    pub tests: Vec<ParsedTest>,
    pub harness_mocks: Vec<ParsedMock>,
    pub imports: Vec<String>,
    pub has_syntax_error: bool,
    /// Non-test helper/fixture functions in this file containing skip calls.
    pub fixture_skips: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ParsedTest {
    pub id: TestId,
    pub assertions: Vec<ParsedAssertion>,
    pub mocks: Vec<ParsedMock>,
    pub parametrized_hashes: Vec<u64>,
    pub is_skipped: bool,
    /// Every identifier/attribute referenced anywhere in the test (body + fixture params).
    pub body_symbols: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ParsedAssertion {
    pub line: usize,
    pub text: String,
    pub strength: AssertionStrength,
    pub referenced_symbols: Vec<String>,
    pub expected_literal: Option<String>,
    /// True if nested inside conditional control flow (`if`/`try`) or `pytest.raises`.
    pub is_guarded: bool,
}

#[derive(Debug, Clone)]
pub struct ParsedMock {
    pub line: usize,
    pub target: String,
}

#[derive(Debug, Clone)]
pub struct ParsedSourceFile {
    pub file_path: String,
    pub defined_symbols: Vec<String>,
    /// Whitespace-normalized definition body per symbol (for change and rename detection).
    pub symbol_bodies: HashMap<String, String>,
    pub string_literals: Vec<String>,
    pub has_syntax_error: bool,
}

/// Records a normalized definition body; duplicate names (methods in different classes) are concatenated.
pub(crate) fn record_body(bodies: &mut HashMap<String, String>, name: &str, body: &str) {
    let normalized = body.split_whitespace().collect::<Vec<_>>().join(" ");
    let entry = bodies.entry(name.to_string()).or_default();
    if !entry.is_empty() {
        entry.push_str(" ;; ");
    }
    entry.push_str(&normalized);
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_tree_sitter_compiles() {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_python::language())
            .unwrap();
        assert!(parser.language().is_some());
    }
}
