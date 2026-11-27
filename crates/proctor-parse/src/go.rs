use crate::{
    record_body, ParsedAssertion, ParsedMock, ParsedSourceFile, ParsedTest, ParsedTestFile,
};
use proctor_core::{AssertionStrength, TestId};
use tree_sitter::{Node, Parser};

/// Parses a Go test file (`*_test.go`) into structured test functions, assertions, table rows, and mocks.
pub fn parse_go_test_file(file_path: &str, source: &str) -> Result<ParsedTestFile, String> {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_go::language())
        .map_err(|e| format!("Failed to set Go language: {}", e))?;

    let tree = parser
        .parse(source, None)
        .ok_or_else(|| "Failed to parse Go source".to_string())?;

    let root = tree.root_node();
    let has_syntax_error = root.has_error();
    let mut tests = Vec::new();
    let mut harness_mocks = Vec::new();
    let mut imports = Vec::new();
    let mut fixture_skips = Vec::new();

    walk_go_test_nodes(
        root,
        source,
        file_path,
        &mut tests,
        &mut harness_mocks,
        &mut imports,
        &mut fixture_skips,
    );

    Ok(ParsedTestFile {
        file_path: file_path.to_string(),
        tests,
        harness_mocks,
        imports,
        has_syntax_error,
        fixture_skips,
    })
}

/// Parses a Go source file (`*.go`) to extract defined package, functions, types, and literals.
pub fn parse_go_source_file(file_path: &str, source: &str) -> Result<ParsedSourceFile, String> {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_go::language())
        .map_err(|e| format!("Failed to set Go language: {}", e))?;

    let tree = parser
        .parse(source, None)
        .ok_or_else(|| "Failed to parse Go source".to_string())?;

    let root = tree.root_node();
    let has_syntax_error = root.has_error();
    let mut defined_symbols = Vec::new();
    let mut string_literals = Vec::new();
    let mut symbol_bodies = std::collections::HashMap::new();

    walk_go_source_nodes(
        root,
        source,
        &mut defined_symbols,
        &mut string_literals,
        &mut symbol_bodies,
    );

    Ok(ParsedSourceFile {
        file_path: file_path.to_string(),
        defined_symbols,
        symbol_bodies,
        string_literals,
        has_syntax_error,
    })
}

fn walk_go_source_nodes(
    node: Node,
    source: &str,
    defined_symbols: &mut Vec<String>,
    string_literals: &mut Vec<String>,
    symbol_bodies: &mut std::collections::HashMap<String, String>,
) {
    match node.kind() {
        "function_declaration" => {
            if let Some(name_node) = node.child_by_field_name("name") {
                let name = node_text(name_node, source);
                defined_symbols.push(name.clone());
                if let Some(body_node) = node.child_by_field_name("body") {
                    let body = node_text(body_node, source);
                    record_body(symbol_bodies, &name, &body);
                }
            }
        }
        "method_declaration" => {
            if let Some(name_node) = node.child_by_field_name("name") {
                let name = node_text(name_node, source);
                defined_symbols.push(name.clone());
                if let Some(body_node) = node.child_by_field_name("body") {
                    let body = node_text(body_node, source);
                    record_body(symbol_bodies, &name, &body);
                }
            }
        }
        "type_spec" => {
            if let Some(name_node) = node.child_by_field_name("name") {
                let name = node_text(name_node, source);
                defined_symbols.push(name.clone());
                if let Some(type_node) = node.child_by_field_name("type") {
                    let body = node_text(type_node, source);
                    record_body(symbol_bodies, &name, &body);
                }
            }
        }
        "interpreted_string_literal" | "raw_string_literal" => {
            let text = node_text(node, source);
            let cleaned = text.trim_matches('"').trim_matches('`').to_string();
            string_literals.push(cleaned);
        }
        _ => {}
    }

    let cursor = &mut node.walk();
    for child in node.children(cursor) {
        walk_go_source_nodes(
            child,
            source,
            defined_symbols,
            string_literals,
            symbol_bodies,
        );
    }
}

fn walk_go_test_nodes(
    node: Node,
    source: &str,
    file_path: &str,
    tests: &mut Vec<ParsedTest>,
    _harness_mocks: &mut Vec<ParsedMock>,
    imports: &mut Vec<String>,
    fixture_skips: &mut Vec<String>,
) {
    match node.kind() {
        "import_spec" => {
            if let Some(path_node) = node.child_by_field_name("path") {
                let path_text = node_text(path_node, source);
                imports.push(path_text.trim_matches('"').to_string());
            }
        }
        "function_declaration" => {
            if let Some(name_node) = node.child_by_field_name("name") {
                let name = node_text(name_node, source);
                if is_go_test_name(&name) {
                    let parsed = parse_single_go_test(node, source, file_path, &name);
                    tests.push(parsed);
                    return; // Test body parsed inside
                } else if is_go_helper_name(&name) {
                    // Check if helper/fixture contains skip calls
                    if contains_skip_call(node, source) {
                        fixture_skips.push(name);
                    }
                }
            }
        }
        "method_declaration" => {
            if let Some(name_node) = node.child_by_field_name("name") {
                let name = node_text(name_node, source);
                if is_go_test_name(&name) {
                    let parsed = parse_single_go_test(node, source, file_path, &name);
                    tests.push(parsed);
                    return;
                }
            }
        }
        _ => {}
    }

    let cursor = &mut node.walk();
    for child in node.children(cursor) {
        walk_go_test_nodes(
            child,
            source,
            file_path,
            tests,
            _harness_mocks,
            imports,
            fixture_skips,
        );
    }
}

fn is_go_test_name(name: &str) -> bool {
    (name.starts_with("Test") && name.len() > 4 && !name.chars().nth(4).unwrap().is_lowercase())
        || (name.starts_with("Fuzz") && name.len() > 4 && !name.chars().nth(4).unwrap().is_lowercase())
        || (name.starts_with("Benchmark") && name.len() > 9 && !name.chars().nth(9).unwrap().is_lowercase())
        || (name.starts_with("Example") && name.len() > 7 && !name.chars().nth(7).unwrap().is_lowercase())
}

fn is_go_helper_name(name: &str) -> bool {
    name.starts_with("setup")
        || name.starts_with("Setup")
        || name.starts_with("helper")
        || name.starts_with("Helper")
        || name.starts_with("fixture")
        || name.starts_with("Fixture")
}

fn parse_single_go_test(
    func_node: Node,
    source: &str,
    file_path: &str,
    test_name: &str,
) -> ParsedTest {
    let mut assertions = Vec::new();
    let mut mocks = Vec::new();
    let mut parametrized_hashes = Vec::new();
    let mut is_skipped = false;
    let mut body_symbols = Vec::new();

    // Check if test contains skip call
    if contains_skip_call(func_node, source) {
        is_skipped = true;
    }

    // Walk body to extract assertions, table rows, and mocks
    walk_go_test_body(
        func_node,
        source,
        &mut assertions,
        &mut mocks,
        &mut parametrized_hashes,
        &mut body_symbols,
        false,
    );

    let body_text = func_node
        .child_by_field_name("body")
        .map(|b| node_text(b, source))
        .unwrap_or_default();
    let normalized_body = body_text.split_whitespace().collect::<Vec<_>>().join(" ");
    let body_hash = blake3::hash(normalized_body.as_bytes());

    let id = TestId {
        file_path: file_path.to_string(),
        scope_hierarchy: Vec::new(),
        name: test_name.to_string(),
        param_hash: None,
        body_content_hash: u64::from_le_bytes(body_hash.as_bytes()[..8].try_into().unwrap()),
    };

    ParsedTest {
        id,
        assertions,
        mocks,
        parametrized_hashes,
        is_skipped,
        body_symbols,
    }
}

fn contains_skip_call(node: Node, source: &str) -> bool {
    let text = node_text(node, source);
    text.contains(".Skip(")
        || text.contains(".Skipf(")
        || text.contains(".SkipNow()")
        || text.contains("testing.Short()")
}

fn walk_go_test_body(
    node: Node,
    source: &str,
    assertions: &mut Vec<ParsedAssertion>,
    mocks: &mut Vec<ParsedMock>,
    parametrized_hashes: &mut Vec<u64>,
    body_symbols: &mut Vec<String>,
    inside_guard: bool,
) {
    let kind = node.kind();
    let is_guard_node = inside_guard || kind == "if_statement" || kind == "type_switch_statement";

    match kind {
        "call_expression" => {
            if let Some(fn_node) = node.child_by_field_name("function") {
                let fn_text = node_text(fn_node, source);

                // 1. Check testify assertions: assert.Equal(t, a, b), require.NoError(t, err)
                if is_testify_assertion(&fn_text) {
                    let line = node.start_position().row + 1;
                    let text = node_text(node, source);
                    let strength = determine_testify_strength(&fn_text);
                    let (symbols, expected_lit) = extract_call_symbols_and_literal(node, source);

                    assertions.push(ParsedAssertion {
                        line,
                        text,
                        strength,
                        referenced_symbols: symbols,
                        expected_literal: expected_lit,
                        is_guarded: inside_guard,
                    });
                }
                // 2. Check gomock / mock calls
                else if is_go_mock_call(&fn_text) {
                    mocks.push(ParsedMock {
                        line: node.start_position().row + 1,
                        target: fn_text.clone(),
                    });
                }
            }
        }
        "if_statement" => {
            // Standard Go pattern: if got != want { t.Errorf(...) } or if err != nil { t.Fatalf(...) }
            if let Some(assertion) = extract_go_if_assertion(node, source) {
                assertions.push(assertion);
            }
        }
        "composite_literal" => {
            // Check for table-driven test composite literals: []struct{...}{ ... }
            extract_table_driven_rows(node, source, parametrized_hashes);
        }
        "identifier" | "field_identifier" => {
            let ident = node_text(node, source);
            if ident.len() > 1 && !is_go_keyword(&ident) {
                body_symbols.push(ident);
            }
        }
        _ => {}
    }

    let cursor = &mut node.walk();
    for child in node.children(cursor) {
        walk_go_test_body(
            child,
            source,
            assertions,
            mocks,
            parametrized_hashes,
            body_symbols,
            is_guard_node,
        );
    }
}

fn is_testify_assertion(fn_text: &str) -> bool {
    let parts: Vec<&str> = fn_text.split('.').collect();
    if parts.len() < 2 {
        return false;
    }
    let receiver = parts[0];
    let method = parts[1];

    if receiver == "assert" || receiver == "require" || receiver.ends_with("Assert") || receiver.ends_with("Require") {
        matches!(
            method,
            "Equal"
                | "EqualValues"
                | "Exactly"
                | "NotEqual"
                | "NoError"
                | "Error"
                | "True"
                | "False"
                | "Nil"
                | "NotNil"
                | "Empty"
                | "NotEmpty"
                | "Len"
                | "Contains"
                | "NotContains"
                | "JSONEq"
                | "ElementsMatch"
                | "Subset"
                | "Regexp"
        )
    } else {
        false
    }
}

fn determine_testify_strength(fn_text: &str) -> AssertionStrength {
    let method = fn_text.split('.').next_back().unwrap_or(fn_text);
    match method {
        "Equal" | "EqualValues" | "Exactly" | "JSONEq" | "ElementsMatch" | "NoError" | "Nil" | "NotNil" | "NotEqual" => AssertionStrength::Exact,
        "True" | "False" | "Empty" | "NotEmpty" | "Len" | "Subset" | "Contains" | "NotContains" => AssertionStrength::Weak,
        _ => AssertionStrength::Weak,
    }
}

fn is_go_mock_call(fn_text: &str) -> bool {
    fn_text.ends_with(".EXPECT")
        || fn_text.contains(".EXPECT().")
        || fn_text.ends_with(".On")
        || fn_text.contains(".On(")
        || fn_text.ends_with(".Return")
}

fn extract_call_symbols_and_literal(
    call_node: Node,
    source: &str,
) -> (Vec<String>, Option<String>) {
    let mut symbols = Vec::new();
    let mut expected_literal = None;

    if let Some(arg_list) = call_node.child_by_field_name("arguments") {
        let cursor = &mut arg_list.walk();
        for child in arg_list.children(cursor) {
            match child.kind() {
                "identifier" | "field_identifier" => {
                    let sym = node_text(child, source);
                    if sym != "t" && !is_go_keyword(&sym) {
                        symbols.push(sym);
                    }
                }
                "interpreted_string_literal" | "raw_string_literal" | "int_literal" | "float_literal"
                    if expected_literal.is_none() => {
                        expected_literal = Some(node_text(child, source));
                    }
                _ => {}
            }
        }
    }

    (symbols, expected_literal)
}

/// Extracts an assertion from Go `if condition { t.Errorf(...) / t.Fatalf(...) / t.Fail() }`.
fn extract_go_if_assertion(if_node: Node, source: &str) -> Option<ParsedAssertion> {
    let consequence = if_node.child_by_field_name("consequence")?;
    let condition = if_node.child_by_field_name("condition")?;

    let cons_text = node_text(consequence, source);
    let is_test_failure = cons_text.contains("t.Errorf(")
        || cons_text.contains("t.Fatalf(")
        || cons_text.contains("t.Error(")
        || cons_text.contains("t.Fatal(")
        || cons_text.contains("t.Fail()")
        || cons_text.contains("t.FailNow()");

    if !is_test_failure {
        return None;
    }

    let line = if_node.start_position().row + 1;
    let cond_text = node_text(condition, source);
    let full_text = format!("if {} {{ ... }}", cond_text);

    let strength = if cond_text.contains("==") || cond_text.contains("!=") {
        AssertionStrength::Exact
    } else {
        AssertionStrength::Weak
    };

    let mut referenced_symbols = Vec::new();
    let mut expected_literal = None;

    // Collect symbols and literals from condition
    let cursor = &mut condition.walk();
    for child in condition.children(cursor) {
        match child.kind() {
            "identifier" | "field_identifier" => {
                let sym = node_text(child, source);
                if !is_go_keyword(&sym) {
                    referenced_symbols.push(sym);
                }
            }
            "interpreted_string_literal" | "raw_string_literal" | "int_literal" | "float_literal"
                if expected_literal.is_none() => {
                    expected_literal = Some(node_text(child, source));
                }
            _ => {}
        }
    }

    Some(ParsedAssertion {
        line,
        text: full_text,
        strength,
        referenced_symbols,
        expected_literal,
        is_guarded: false, // In Go, the `if` IS the assertion, so not a weakening wrapper
    })
}

/// Detects table-driven test struct slices: `tests := []struct{...}{ {row1}, {row2} }` or `[]testCase{ ... }`.
fn extract_table_driven_rows(
    node: Node,
    source: &str,
    parametrized_hashes: &mut Vec<u64>,
) {
    let type_node = match node.child_by_field_name("type") {
        Some(t) => t,
        None => return,
    };

    let type_kind = type_node.kind();
    let is_slice_or_array = type_kind == "slice_type" || type_kind == "array_type" || type_kind == "map_type";
    let type_text = node_text(type_node, source);

    if !is_slice_or_array && !type_text.contains("struct") && !type_text.ends_with("Test") && !type_text.ends_with("TestCase") {
        return;
    }

    if let Some(body) = node.child_by_field_name("body") {
        let cursor = &mut body.walk();
        for child in body.children(cursor) {
            // Elements in literal_value: literal_element or keyed_element
            if child.kind() == "literal_element" || child.kind() == "keyed_element" {
                let row_text = node_text(child, source);
                let normalized: String = row_text.split_whitespace().collect::<Vec<_>>().join(" ");
                if normalized.len() > 2 {
                    let hash_bytes = blake3::hash(normalized.as_bytes());
                    let mut hash_buf = [0u8; 8];
                    hash_buf.copy_from_slice(&hash_bytes.as_bytes()[..8]);
                    let hash_u64 = u64::from_le_bytes(hash_buf);
                    parametrized_hashes.push(hash_u64);
                }
            }
        }
    }
}

fn node_text(node: Node, source: &str) -> String {
    let bytes = source.as_bytes();
    let start = node.start_byte();
    let end = node.end_byte().min(bytes.len());
    String::from_utf8_lossy(&bytes[start..end]).to_string()
}

fn is_go_keyword(s: &str) -> bool {
    matches!(
        s,
        "break"
            | "case"
            | "chan"
            | "const"
            | "continue"
            | "default"
            | "defer"
            | "else"
            | "fallthrough"
            | "for"
            | "func"
            | "go"
            | "goto"
            | "if"
            | "import"
            | "interface"
            | "map"
            | "package"
            | "range"
            | "return"
            | "select"
            | "struct"
            | "switch"
            | "type"
            | "var"
            | "nil"
            | "true"
            | "false"
            | "iota"
            | "err"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_go_standard_test_assertions() {
        let code = r#"
package auth_test

import (
    "testing"
)

func TestTokenValidation(t *testing.T) {
    got := ValidateToken("valid")
    want := true
    if got != want {
        t.Errorf("got %v, want %v", got, want)
    }

    err := CheckExpiry()
    if err != nil {
        t.Fatalf("unexpected error: %v", err)
    }
}
"#;
        let parsed = parse_go_test_file("auth_test.go", code).unwrap();
        assert_eq!(parsed.tests.len(), 1);
        let test = &parsed.tests[0];
        assert_eq!(test.id.name, "TestTokenValidation");
        assert_eq!(test.assertions.len(), 2);
        assert_eq!(test.assertions[0].strength, AssertionStrength::Exact);
        assert_eq!(test.assertions[1].strength, AssertionStrength::Exact);
    }

    #[test]
    fn test_parse_go_testify_and_table_driven_rows() {
        let code = r#"
package calc_test

import (
    "testing"
    "github.com/stretchr/testify/assert"
)

func TestAddTable(t *testing.T) {
    tests := []struct {
        name string
        a, b int
        want int
    }{
        {name: "positive", a: 1, b: 2, want: 3},
        {name: "negative", a: -1, b: -2, want: -3},
        {name: "zero", a: 0, b: 0, want: 0},
    }

    for _, tc := range tests {
        t.Run(tc.name, func(t *testing.T) {
            got := Add(tc.a, tc.b)
            assert.Equal(t, tc.want, got)
        })
    }
}
"#;
        let parsed = parse_go_test_file("calc_test.go", code).unwrap();
        assert_eq!(parsed.tests.len(), 1);
        let test = &parsed.tests[0];
        assert_eq!(test.id.name, "TestAddTable");
        // 3 table rows extracted as parameterized hashes
        assert_eq!(test.parametrized_hashes.len(), 3);
        // assert.Equal call inside
        assert_eq!(test.assertions.len(), 1);
        assert_eq!(test.assertions[0].strength, AssertionStrength::Exact);
    }

    #[test]
    fn test_parse_go_source_file_symbols() {
        let code = r#"
package calc

type Calculator struct {
    multiplier int
}

func Add(a, b int) int {
    return a + b
}

func (c *Calculator) Multiply(x int) int {
    return x * c.multiplier
}
"#;
        let parsed = parse_go_source_file("calc.go", code).unwrap();
        assert!(parsed.defined_symbols.contains(&"Calculator".to_string()));
        assert!(parsed.defined_symbols.contains(&"Add".to_string()));
        assert!(parsed.defined_symbols.contains(&"Multiply".to_string()));
        assert!(parsed.symbol_bodies.contains_key("Add"));
    }
}
