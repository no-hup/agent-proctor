use crate::{ParsedAssertion, ParsedMock, ParsedTest, ParsedTestFile, ParsedSourceFile};
use proctor_core::{AssertionStrength, TestId};
use tree_sitter::{Node, Parser};

/// Parses a Python test file into structured test functions, assertions, and mocks.
pub fn parse_python_test_file(file_path: &str, source: &str) -> Result<ParsedTestFile, String> {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_python::language())
        .map_err(|e| format!("Failed to set Python language: {}", e))?;

    let tree = parser
        .parse(source, None)
        .ok_or_else(|| "Failed to parse Python source".to_string())?;

    let root = tree.root_node();
    let has_syntax_error = root.has_error();
    let mut tests = Vec::new();
    let mut harness_mocks = Vec::new();
    let mut imports = Vec::new();

    walk_python_test_nodes(root, source, file_path, &[], &mut tests, &mut harness_mocks, &mut imports);

    Ok(ParsedTestFile {
        file_path: file_path.to_string(),
        tests,
        harness_mocks,
        imports,
        has_syntax_error,
    })
}

/// Parses a Python source file to extract defined functions, classes, and literals.
pub fn parse_python_source_file(file_path: &str, source: &str) -> Result<ParsedSourceFile, String> {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_python::language())
        .map_err(|e| format!("Failed to set Python language: {}", e))?;

    let tree = parser
        .parse(source, None)
        .ok_or_else(|| "Failed to parse Python source".to_string())?;

    let root = tree.root_node();
    let has_syntax_error = root.has_error();
    let mut defined_symbols = Vec::new();
    let mut string_literals = Vec::new();
    let mut symbol_bodies = std::collections::HashMap::new();

    extract_source_symbols(root, source, &mut defined_symbols, &mut symbol_bodies, &mut string_literals);

    Ok(ParsedSourceFile {
        file_path: file_path.to_string(),
        defined_symbols,
        symbol_bodies,
        string_literals,
        has_syntax_error,
    })
}

fn walk_python_test_nodes(
    node: Node,
    source: &str,
    file_path: &str,
    scope: &[String],
    tests: &mut Vec<ParsedTest>,
    harness_mocks: &mut Vec<ParsedMock>,
    imports: &mut Vec<String>,
) {
    let cursor = &mut node.walk();
    for child in node.children(cursor) {
        match child.kind() {
            "import_statement" | "import_from_statement" => {
                let text = child.utf8_text(source.as_bytes()).unwrap_or("");
                imports.push(text.trim().to_string());
            }
            "class_definition" => {
                if let Some(name_node) = child.child_by_field_name("name") {
                    let class_name = name_node.utf8_text(source.as_bytes()).unwrap_or("");
                    let mut new_scope = scope.to_vec();
                    new_scope.push(class_name.to_string());
                    if let Some(body) = child.child_by_field_name("body") {
                        walk_python_test_nodes(body, source, file_path, &new_scope, tests, harness_mocks, imports);
                    }
                }
            }
            "function_definition" => {
                if let Some(name_node) = child.child_by_field_name("name") {
                    let fn_name = name_node.utf8_text(source.as_bytes()).unwrap_or("");
                    if fn_name.starts_with("test_") || fn_name.ends_with("_test") {
                        let parsed_test = extract_python_test(child, source, file_path, scope, fn_name, false);
                        tests.push(parsed_test);
                    }
                }
            }
            "decorated_definition" => {
                let mut param_hashes = Vec::new();
                extract_parametrize_rows(child, source, &mut param_hashes);

                let mut decorator_mocks = Vec::new();
                extract_decorator_mocks(child, source, &mut decorator_mocks);

                let is_skipped = is_skip_decorated(child, source);

                if let Some(def_node) = child.child_by_field_name("definition") {
                    if def_node.kind() == "function_definition" {
                        if let Some(name_node) = def_node.child_by_field_name("name") {
                            let fn_name = name_node.utf8_text(source.as_bytes()).unwrap_or("");
                            if fn_name.starts_with("test_") || fn_name.ends_with("_test") {
                                let mut parsed_test = extract_python_test(def_node, source, file_path, scope, fn_name, is_skipped);
                                parsed_test.parametrized_hashes = param_hashes;
                                parsed_test.mocks.extend(decorator_mocks);
                                tests.push(parsed_test);
                            }
                        }
                    }
                }
            }
            _ => {
                if let Some(mock) = extract_mock_call(child, source) {
                    harness_mocks.push(mock);
                }
            }
        }
    }
}

fn is_skip_decorated(node: Node, source: &str) -> bool {
    let cursor = &mut node.walk();
    for child in node.children(cursor) {
        if child.kind() == "decorator" {
            let txt = child.utf8_text(source.as_bytes()).unwrap_or("");
            if txt.contains("skip") || txt.contains("xfail") {
                return true;
            }
        }
    }
    false
}

fn extract_python_test(
    fn_node: Node,
    source: &str,
    file_path: &str,
    scope: &[String],
    fn_name: &str,
    mut is_skipped: bool,
) -> ParsedTest {
    let mut assertions = Vec::new();
    let mut mocks = Vec::new();

    let body_text = fn_node
        .child_by_field_name("body")
        .and_then(|b| b.utf8_text(source.as_bytes()).ok())
        .unwrap_or("");
    let normalized_body = body_text.split_whitespace().collect::<Vec<_>>().join(" ");
    let body_hash = blake3::hash(normalized_body.as_bytes());

    let test_id = TestId {
        file_path: file_path.to_string(),
        scope_hierarchy: scope.to_vec(),
        name: fn_name.to_string(),
        param_hash: None,
        body_content_hash: u64::from_le_bytes(body_hash.as_bytes()[..8].try_into().unwrap()),
    };

    if body_text.contains("pytest.skip(") || body_text.contains("self.skipTest(") || body_text.contains("unittest.skip(") {
        is_skipped = true;
    }

    if let Some(body) = fn_node.child_by_field_name("body") {
        extract_assertions_and_mocks(body, source, &mut assertions, &mut mocks);
    }

    let mut body_symbols = Vec::new();
    extract_identifiers_and_attrs(fn_node, source, &mut body_symbols);

    ParsedTest {
        id: test_id,
        assertions,
        mocks,
        parametrized_hashes: Vec::new(),
        is_skipped,
        body_symbols,
    }
}

fn extract_assertions_and_mocks(
    node: Node,
    source: &str,
    assertions: &mut Vec<ParsedAssertion>,
    mocks: &mut Vec<ParsedMock>,
) {
    let cursor = &mut node.walk();
    for child in node.children(cursor) {
        match child.kind() {
            "assert_statement" => {
                if let Some(parsed) = parse_assert_statement(child, source) {
                    assertions.push(parsed);
                }
            }
            "call" => {
                if let Some(mock) = extract_mock_call(child, source) {
                    mocks.push(mock);
                } else if let Some(unittest_assert) = parse_unittest_call(child, source) {
                    assertions.push(unittest_assert);
                }
            }
            "with_statement" => {
                if let Some(pytest_raises) = parse_pytest_raises(child, source) {
                    assertions.push(pytest_raises);
                }
                extract_assertions_and_mocks(child, source, assertions, mocks);
            }
            _ => {
                extract_assertions_and_mocks(child, source, assertions, mocks);
            }
        }
    }
}

fn parse_assert_statement(assert_node: Node, source: &str) -> Option<ParsedAssertion> {
    let text = assert_node.utf8_text(source.as_bytes()).ok()?.trim().to_string();
    let line = assert_node.start_position().row + 1;

    let mut referenced_symbols = Vec::new();
    let mut expected_literal = None;
    let mut strength = AssertionStrength::Exact;

    if let Some(expr) = assert_node.child(1) {
        let expr_text = expr.utf8_text(source.as_bytes()).unwrap_or("").trim();

        if expr_text == "True" || expr_text == "1" || expr_text == "not False" {
            strength = AssertionStrength::Vacuous;
        } else if expr_text.contains("is not None")
            || expr_text.contains("is not")
            || expr_text.contains(">=")
            || expr_text.contains("<=")
            || expr_text.contains('>')
            || expr_text.contains('<')
        {
            strength = AssertionStrength::Weak;
        } else if expr.kind() == "comparison_operator" {
            let left = expr.child(0);
            let op = expr.child(1);
            let right = expr.child(2);

            if let (Some(l), Some(o), Some(r)) = (left, op, right) {
                let l_txt = l.utf8_text(source.as_bytes()).unwrap_or("").trim();
                let op_txt = o.utf8_text(source.as_bytes()).unwrap_or("").trim();
                let r_txt = r.utf8_text(source.as_bytes()).unwrap_or("").trim();

                if op_txt == "==" && l_txt == r_txt {
                    strength = AssertionStrength::Vacuous;
                } else if op_txt == "==" || op_txt == "is" {
                    strength = AssertionStrength::Exact;
                    if r.kind() == "string" || r.kind() == "integer" || r.kind() == "float" {
                        let clean_lit = r_txt.trim_matches(|c| c == '\'' || c == '"').to_string();
                        expected_literal = Some(clean_lit);
                    }
                }
            }
        } else if expr.kind() == "boolean_operator" {
            if expr_text.contains("or True") || expr_text.contains("or 1") || expr_text.ends_with("or not False") {
                strength = AssertionStrength::Vacuous;
            } else {
                strength = AssertionStrength::Weak;
            }
        } else if expr.kind() == "identifier" {
            strength = AssertionStrength::Weak;
        }

        extract_identifiers_and_attrs(expr, source, &mut referenced_symbols);
    }

    Some(ParsedAssertion {
        line,
        text,
        strength,
        referenced_symbols,
        expected_literal,
    })
}

fn parse_unittest_call(call_node: Node, source: &str) -> Option<ParsedAssertion> {
    let text = call_node.utf8_text(source.as_bytes()).ok()?.trim().to_string();
    let line = call_node.start_position().row + 1;

    let fn_child = call_node.child_by_field_name("function")?;
    let fn_text = fn_child.utf8_text(source.as_bytes()).ok()?;

    if !fn_text.contains("assert") {
        return None;
    }

    let mut referenced_symbols = Vec::new();
    let mut expected_literal = None;
    let strength;

    if fn_text.ends_with("assertEqual") || fn_text.ends_with("assert_equal") || fn_text.ends_with("assertDictEqual") {
        strength = AssertionStrength::Exact;
        if let Some(args) = call_node.child_by_field_name("arguments") {
            if let Some(second_arg) = args.child(3) {
                let arg_txt = second_arg.utf8_text(source.as_bytes()).unwrap_or("").trim();
                expected_literal = Some(arg_txt.to_string());
            }
        }
    } else {
        strength = AssertionStrength::Weak;
    }

    extract_identifiers_and_attrs(call_node, source, &mut referenced_symbols);

    Some(ParsedAssertion {
        line,
        text,
        strength,
        referenced_symbols,
        expected_literal,
    })
}

fn parse_pytest_raises(with_node: Node, source: &str) -> Option<ParsedAssertion> {
    let text = with_node.utf8_text(source.as_bytes()).ok()?.trim().to_string();
    let line = with_node.start_position().row + 1;

    let mut referenced_symbols = Vec::new();
    let mut strength = AssertionStrength::Weak;

    if text.contains("pytest.raises") {
        if text.contains("match=") {
            strength = AssertionStrength::Exact;
        } else if text.contains("Exception") || text.contains("BaseException") {
            strength = AssertionStrength::Weak;
        } else {
            strength = AssertionStrength::Exact;
        }
    }

    extract_identifiers_and_attrs(with_node, source, &mut referenced_symbols);

    Some(ParsedAssertion {
        line,
        text,
        strength,
        referenced_symbols,
        expected_literal: None,
    })
}

fn extract_identifiers_and_attrs(node: Node, source: &str, symbols: &mut Vec<String>) {
    let cursor = &mut node.walk();
    for child in node.children(cursor) {
        match child.kind() {
            "identifier" => {
                let name = child.utf8_text(source.as_bytes()).unwrap_or("");
                if !name.is_empty() && name != "self" && name != "assert" && name != "None" && name != "True" && name != "False" {
                    symbols.push(name.to_string());
                }
            }
            "attribute" => {
                if let Some(attr_name) = child.child_by_field_name("attribute") {
                    let name = attr_name.utf8_text(source.as_bytes()).unwrap_or("");
                    if !name.is_empty() {
                        symbols.push(name.to_string());
                    }
                }
            }
            "string" => {
                let str_content = child.utf8_text(source.as_bytes()).unwrap_or("");
                let trimmed = str_content.trim_matches(|c| c == '\'' || c == '"');
                if !trimmed.is_empty() && trimmed.chars().all(|c| c.is_alphanumeric() || c == '_') {
                    symbols.push(trimmed.to_string());
                }
            }
            _ => {
                extract_identifiers_and_attrs(child, source, symbols);
            }
        }
    }
}

fn extract_mock_call(node: Node, source: &str) -> Option<ParsedMock> {
    if node.kind() != "call" {
        return None;
    }

    let fn_child = node.child_by_field_name("function")?;
    let fn_text = fn_child.utf8_text(source.as_bytes()).ok()?;

    if fn_text.contains("patch") || fn_text.ends_with(".setattr") {
        let args = node.child_by_field_name("arguments")?;
        let cursor = &mut args.walk();
        for arg in args.children(cursor) {
            if arg.kind() == "string" {
                let text = arg.utf8_text(source.as_bytes()).ok()?;
                let target = text.trim_matches(|c| c == '\'' || c == '"').to_string();
                return Some(ParsedMock {
                    line: node.start_position().row + 1,
                    target,
                });
            }
        }
    }

    None
}

fn extract_parametrize_rows(decorated_node: Node, source: &str, hashes: &mut Vec<u64>) {
    let cursor = &mut decorated_node.walk();
    for child in decorated_node.children(cursor) {
        if child.kind() == "decorator" {
            let text = child.utf8_text(source.as_bytes()).unwrap_or("");
            if text.contains("parametrize") {
                find_list_tuples(child, source, hashes);
            }
        }
    }
}

fn extract_decorator_mocks(decorated_node: Node, source: &str, mocks: &mut Vec<ParsedMock>) {
    let cursor = &mut decorated_node.walk();
    for child in decorated_node.children(cursor) {
        if child.kind() == "decorator" {
            extract_mocks_from_node(child, source, mocks);
        }
    }
}

fn extract_mocks_from_node(node: Node, source: &str, mocks: &mut Vec<ParsedMock>) {
    if let Some(mock) = extract_mock_call(node, source) {
        mocks.push(mock);
    }
    let cursor = &mut node.walk();
    for child in node.children(cursor) {
        extract_mocks_from_node(child, source, mocks);
    }
}

fn find_list_tuples(node: Node, source: &str, hashes: &mut Vec<u64>) {
    let cursor = &mut node.walk();
    for child in node.children(cursor) {
        if child.kind() == "list" {
            for item in child.children(&mut child.walk()) {
                if item.kind() == "tuple" || item.kind() == "list" {
                    let item_txt = item.utf8_text(source.as_bytes()).unwrap_or("");
                    let h = blake3::hash(item_txt.as_bytes());
                    hashes.push(u64::from_le_bytes(h.as_bytes()[..8].try_into().unwrap()));
                }
            }
        } else {
            find_list_tuples(child, source, hashes);
        }
    }
}

fn extract_source_symbols(
    node: Node,
    source: &str,
    defined_symbols: &mut Vec<String>,
    symbol_bodies: &mut std::collections::HashMap<String, String>,
    string_literals: &mut Vec<String>,
) {
    let cursor = &mut node.walk();
    for child in node.children(cursor) {
        match child.kind() {
            "function_definition" | "class_definition" => {
                if let Some(name_node) = child.child_by_field_name("name") {
                    let name = name_node.utf8_text(source.as_bytes()).unwrap_or("");
                    if !name.is_empty() {
                        defined_symbols.push(name.to_string());
                        let body = child.child_by_field_name("body").and_then(|b| b.utf8_text(source.as_bytes()).ok()).unwrap_or("");
                        crate::record_body(symbol_bodies, name, body);
                    }
                }
            }
            "assignment" => {
                if let Some(left) = child.child_by_field_name("left") {
                    let name = left.utf8_text(source.as_bytes()).unwrap_or("");
                    if !name.is_empty() {
                        defined_symbols.push(name.to_string());
                        let body = child.child_by_field_name("right").and_then(|b| b.utf8_text(source.as_bytes()).ok()).unwrap_or("");
                        crate::record_body(symbol_bodies, name, body);
                    }
                }
            }
            "string" | "integer" | "float" => {
                let txt = child.utf8_text(source.as_bytes()).unwrap_or("");
                let trimmed = txt.trim_matches(|c| c == '\'' || c == '"');
                if !trimmed.is_empty() {
                    string_literals.push(trimmed.to_string());
                }
            }
            _ => {}
        }
        extract_source_symbols(child, source, defined_symbols, symbol_bodies, string_literals);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_python_assertions() {
        let code = r#"
def test_tax_calculation():
    assert calculate_tax(100) == 120
    assert total is not None
    assert True
"#;
        let file = parse_python_test_file("tests/test_tax.py", code).unwrap();
        assert_eq!(file.tests.len(), 1);
        let test = &file.tests[0];
        assert_eq!(test.id.name, "test_tax_calculation");
        assert_eq!(test.assertions.len(), 3);
        assert_eq!(test.assertions[0].strength, AssertionStrength::Exact);
        assert_eq!(test.assertions[0].expected_literal.as_deref(), Some("120"));
        assert_eq!(test.assertions[1].strength, AssertionStrength::Weak);
        assert_eq!(test.assertions[2].strength, AssertionStrength::Vacuous);
    }

    #[test]
    fn test_parse_python_mock() {
        let code = r#"
def test_billing(mocker):
    mocker.patch("app.finance.stripe_client.charge", return_value={"status": "ok"})
    assert True
"#;
        let file = parse_python_test_file("tests/test_billing.py", code).unwrap();
        assert_eq!(file.tests.len(), 1);
        let test = &file.tests[0];
        assert_eq!(test.mocks.len(), 1);
        assert_eq!(test.mocks[0].target, "app.finance.stripe_client.charge");
    }

    #[test]
    fn test_parse_parametrize_rows() {
        let code = r#"
@pytest.mark.parametrize("input,expected", [
    (100, 120),
    (200, 240),
])
def test_tax(input, expected):
    assert calculate(input) == expected
"#;
        let file = parse_python_test_file("tests/test_tax.py", code).unwrap();
        assert_eq!(file.tests.len(), 1);
        assert_eq!(file.tests[0].parametrized_hashes.len(), 2);
    }
}
