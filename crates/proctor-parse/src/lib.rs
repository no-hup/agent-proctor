use proctor_core::*;

pub mod python;
pub mod javascript;

pub use python::{parse_python_source_file, parse_python_test_file};
pub use javascript::{parse_javascript_source_file, parse_javascript_test_file};

#[derive(Debug, Clone)]
pub struct ParsedTestFile {
    pub file_path: String,
    pub tests: Vec<ParsedTest>,
    pub harness_mocks: Vec<ParsedMock>,
    pub imports: Vec<String>,
    pub has_syntax_error: bool,
}

#[derive(Debug, Clone)]
pub struct ParsedTest {
    pub id: TestId,
    pub assertions: Vec<ParsedAssertion>,
    pub mocks: Vec<ParsedMock>,
    pub parametrized_hashes: Vec<u64>,
    pub is_skipped: bool,
}

#[derive(Debug, Clone)]
pub struct ParsedAssertion {
    pub line: usize,
    pub text: String,
    pub strength: AssertionStrength,
    pub referenced_symbols: Vec<String>,
    pub expected_literal: Option<String>,
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
    pub string_literals: Vec<String>,
    pub has_syntax_error: bool,
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_tree_sitter_compiles() {
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&tree_sitter_python::language()).unwrap();
        assert!(parser.language().is_some());
    }
}
