use crate::{ParsedAssertion, ParsedMock, ParsedSourceFile, ParsedTest, ParsedTestFile};
use proctor_core::{AssertionStrength, TestId};
use tree_sitter::{Node, Parser};

/// Selects the appropriate Tree-sitter grammar for JS, TS, or TSX based on path/flag.
fn js_language(file_path: &str, is_typescript: bool) -> tree_sitter::Language {
    if is_typescript {
        if file_path.to_ascii_lowercase().ends_with(".tsx") {
            tree_sitter_typescript::language_tsx()
        } else {
            tree_sitter_typescript::language_typescript()
        }
    } else {
        tree_sitter_javascript::language()
    }
}

/// Checks whether any Tree-sitter `ERROR` or `MISSING` node represents a real syntax error
/// rather than the tree-sitter-typescript 0.21 TSX quirk on `typeof import("...")` type args.
fn has_real_syntax_error(root: Node, source: &str, is_typescript: bool) -> bool {
    if !root.has_error() {
        return false;
    }
    if !is_typescript {
        return true;
    }
    fn walk_errors(node: Node, source: &str) -> bool {
        if node.is_error() || node.is_missing() {
            // Check node text or its line window for `typeof import(`
            let bytes = source.as_bytes();
            let start = node.start_byte();
            let end = node.end_byte().min(bytes.len());
            let line_start = source[..start].rfind('\n').map(|i| i + 1).unwrap_or(0);
            let line_end = source[end..]
                .find('\n')
                .map(|i| end + i)
                .unwrap_or(source.len());
            let snippet = &source[line_start..line_end];
            let prev_line_start = source[..line_start.saturating_sub(1)]
                .rfind('\n')
                .map(|i| i + 1)
                .unwrap_or(0);
            let next_line_end = if line_end < source.len() {
                source[line_end + 1..]
                    .find('\n')
                    .map(|i| line_end + 1 + i)
                    .unwrap_or(source.len())
            } else {
                line_end
            };
            let window = &source[prev_line_start..next_line_end];
            if !snippet.contains("typeof import(") && !window.contains("typeof import(") {
                return true;
            }
            return false;
        }
        let cursor = &mut node.walk();
        for child in node.children(cursor) {
            if child.has_error() && walk_errors(child, source) {
                return true;
            }
        }
        false
    }
    walk_errors(root, source)
}

/// Parses a JavaScript or TypeScript test file into structured tests, assertions, and mocks.
pub fn parse_javascript_test_file(
    file_path: &str,
    source: &str,
    is_typescript: bool,
) -> Result<ParsedTestFile, String> {
    let mut parser = Parser::new();
    let lang = js_language(file_path, is_typescript);

    parser
        .set_language(&lang)
        .map_err(|e| format!("Failed to set JS/TS language: {}", e))?;

    let tree = parser
        .parse(source, None)
        .ok_or_else(|| "Failed to parse JS/TS source".to_string())?;

    let root = tree.root_node();
    let has_syntax_error = has_real_syntax_error(root, source, is_typescript);
    let mut tests = Vec::new();
    let mut harness_mocks = Vec::new();
    let mut imports = Vec::new();

    walk_js_nodes(
        root,
        source,
        file_path,
        &[],
        &mut tests,
        &mut harness_mocks,
        &mut imports,
    );

    Ok(ParsedTestFile {
        file_path: file_path.to_string(),
        tests,
        harness_mocks,
        imports,
        has_syntax_error,
    })
}

/// Parses a JavaScript/TypeScript source file to extract defined functions, classes, and literals.
pub fn parse_javascript_source_file(
    file_path: &str,
    source: &str,
    is_typescript: bool,
) -> Result<ParsedSourceFile, String> {
    let mut parser = Parser::new();
    let lang = js_language(file_path, is_typescript);

    parser
        .set_language(&lang)
        .map_err(|e| format!("Failed to set JS/TS language: {}", e))?;

    let tree = parser
        .parse(source, None)
        .ok_or_else(|| "Failed to parse JS/TS source".to_string())?;

    let root = tree.root_node();
    let has_syntax_error = has_real_syntax_error(root, source, is_typescript);
    let mut defined_symbols = Vec::new();
    let mut string_literals = Vec::new();
    let mut symbol_bodies = std::collections::HashMap::new();

    extract_js_source_symbols(
        root,
        source,
        false,
        &mut defined_symbols,
        &mut symbol_bodies,
        &mut string_literals,
    );

    Ok(ParsedSourceFile {
        file_path: file_path.to_string(),
        defined_symbols,
        symbol_bodies,
        string_literals,
        has_syntax_error,
    })
}

fn walk_js_nodes(
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
            "import_statement" => {
                let text = child.utf8_text(source.as_bytes()).unwrap_or("");
                imports.push(text.trim().to_string());
            }
            "call_expression" => {
                if let Some(fn_node) = child.child_by_field_name("function") {
                    let fn_name = fn_node.utf8_text(source.as_bytes()).unwrap_or("");
                    if fn_name == "describe"
                        || fn_name == "context"
                        || fn_name == "describe.skip"
                        || fn_name == "xdescribe"
                    {
                        let suite_name = extract_first_string_arg(child, source)
                            .unwrap_or_else(|| "describe".to_string());
                        let mut new_scope = scope.to_vec();
                        new_scope.push(suite_name);
                        if let Some(args) = child.child_by_field_name("arguments") {
                            walk_js_nodes(
                                args,
                                source,
                                file_path,
                                &new_scope,
                                tests,
                                harness_mocks,
                                imports,
                            );
                        }
                    } else if fn_name == "test"
                        || fn_name == "it"
                        || fn_name == "test.skip"
                        || fn_name == "it.skip"
                        || fn_name == "xit"
                        || fn_name == "xtest"
                        || fn_name == "test.todo"
                        || fn_name == "it.todo"
                    {
                        let is_skipped = fn_name.contains("skip")
                            || fn_name.starts_with('x')
                            || fn_name.contains("todo");
                        let test_name = extract_first_string_arg(child, source)
                            .unwrap_or_else(|| "test".to_string());
                        let mut parsed_test =
                            extract_js_test(child, source, file_path, scope, &test_name);
                        parsed_test.is_skipped = is_skipped;
                        tests.push(parsed_test);
                    } else if let Some(mock) = extract_js_mock_call(child, source) {
                        harness_mocks.push(mock);
                    }
                }
            }
            _ => {
                walk_js_nodes(
                    child,
                    source,
                    file_path,
                    scope,
                    tests,
                    harness_mocks,
                    imports,
                );
            }
        }
    }
}

fn extract_js_test(
    call_node: Node,
    source: &str,
    file_path: &str,
    scope: &[String],
    test_name: &str,
) -> ParsedTest {
    let mut assertions = Vec::new();
    let mut mocks = Vec::new();

    let text = call_node.utf8_text(source.as_bytes()).unwrap_or("");
    let normalized = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let body_hash = blake3::hash(normalized.as_bytes());

    let test_id = TestId {
        file_path: file_path.to_string(),
        scope_hierarchy: scope.to_vec(),
        name: test_name.to_string(),
        param_hash: None,
        body_content_hash: u64::from_le_bytes(body_hash.as_bytes()[..8].try_into().unwrap()),
    };

    extract_js_assertions_and_mocks(call_node, source, &mut assertions, &mut mocks);

    let mut body_symbols = Vec::new();
    extract_js_identifiers_and_props(call_node, source, &mut body_symbols);
    let mut locals = std::collections::HashSet::new();
    collect_js_locals(call_node, source, &mut locals);
    body_symbols.retain(|s| !locals.contains(s));

    ParsedTest {
        id: test_id,
        assertions,
        mocks,
        parametrized_hashes: Vec::new(),
        is_skipped: false,
        body_symbols,
    }
}

fn extract_js_assertions_and_mocks(
    node: Node,
    source: &str,
    assertions: &mut Vec<ParsedAssertion>,
    mocks: &mut Vec<ParsedMock>,
) {
    let cursor = &mut node.walk();
    for child in node.children(cursor) {
        if child.kind() == "call_expression" {
            let text = child.utf8_text(source.as_bytes()).unwrap_or("");
            if text.starts_with("expect(") && (text.contains(".to") || text.contains(".not.")) {
                if let Some(parsed) = parse_expect_call(child, source) {
                    assertions.push(parsed);
                    // Avoid recursing into inner expect(...) call
                    continue;
                }
            } else if let Some(mock) = extract_js_mock_call(child, source) {
                mocks.push(mock);
                continue;
            }
        }
        extract_js_assertions_and_mocks(child, source, assertions, mocks);
    }
}

fn parse_expect_call(call_node: Node, source: &str) -> Option<ParsedAssertion> {
    let text = call_node
        .utf8_text(source.as_bytes())
        .ok()?
        .trim()
        .to_string();
    let line = call_node.start_position().row + 1;

    let mut referenced_symbols = Vec::new();
    let mut expected_literal = None;
    let mut strength = AssertionStrength::Exact;

    if text.contains(".toBe(true)") && text.contains("expect(true)") {
        strength = AssertionStrength::Vacuous;
    } else if text.contains("expect.anything()")
        || text.contains("expect.any(")
        || text.contains("toMatchObject({})")
        || text.contains(".toBeDefined()")
        || text.contains(".toBeTruthy()")
        || text.contains(".toBeFalsy()")
        || text.contains(".toBeNull()")
        || text.contains(".toBeNaN()")
    {
        strength = AssertionStrength::Weak;
    } else if text.contains(".toBe(")
        || text.contains(".toEqual(")
        || text.contains(".toStrictEqual(")
    {
        strength = AssertionStrength::Exact;
        if let Some(args) = call_node.child_by_field_name("arguments") {
            let arg_txt = args.utf8_text(source.as_bytes()).unwrap_or("").trim();
            expected_literal = Some(
                arg_txt
                    .trim_matches(|c| c == '(' || c == ')' || c == '\'' || c == '"' || c == '`')
                    .to_string(),
            );
        }
    }

    extract_js_identifiers_and_props(call_node, source, &mut referenced_symbols);

    Some(ParsedAssertion {
        line,
        text,
        strength,
        referenced_symbols,
        expected_literal,
    })
}

fn extract_js_identifiers_and_props(node: Node, source: &str, symbols: &mut Vec<String>) {
    let cursor = &mut node.walk();
    for child in node.children(cursor) {
        match child.kind() {
            "identifier" | "property_identifier" => {
                let name = child.utf8_text(source.as_bytes()).unwrap_or("");
                if !name.is_empty()
                    && name != "expect"
                    && name != "toBe"
                    && name != "toEqual"
                    && name != "toBeDefined"
                {
                    symbols.push(name.to_string());
                }
            }
            "string" => {
                let txt = child.utf8_text(source.as_bytes()).unwrap_or("");
                let trimmed = txt.trim_matches(|c| c == '\'' || c == '"' || c == '`');
                if !trimmed.is_empty() && trimmed.chars().all(|c| c.is_alphanumeric() || c == '_') {
                    symbols.push(trimmed.to_string());
                }
            }
            _ => {
                extract_js_identifiers_and_props(child, source, symbols);
            }
        }
    }
}

/// Names declared inside a test (`const x = ...`): the test's own variables.
fn collect_js_locals(node: Node, source: &str, locals: &mut std::collections::HashSet<String>) {
    let cursor = &mut node.walk();
    for child in node.children(cursor) {
        if child.kind() == "variable_declarator" {
            if let Some(n) = child
                .child_by_field_name("name")
                .filter(|n| n.kind() == "identifier")
            {
                locals.insert(n.utf8_text(source.as_bytes()).unwrap_or("").to_string());
            }
        }
        collect_js_locals(child, source, locals);
    }
}

fn extract_js_mock_call(node: Node, source: &str) -> Option<ParsedMock> {
    if node.kind() != "call_expression" {
        return None;
    }

    let fn_child = node.child_by_field_name("function")?;
    let fn_text = fn_child.utf8_text(source.as_bytes()).ok()?;

    if fn_text == "jest.mock"
        || fn_text == "vi.mock"
        || fn_text.ends_with(".mock")
        || fn_text.contains("spyOn")
        || fn_text.contains("doMock")
    {
        if let Some(target) = extract_first_string_arg(node, source) {
            return Some(ParsedMock {
                line: node.start_position().row + 1,
                target,
            });
        }
    }

    None
}

fn extract_first_string_arg(call_node: Node, source: &str) -> Option<String> {
    let args = call_node.child_by_field_name("arguments")?;
    let cursor = &mut args.walk();
    for arg in args.children(cursor) {
        if arg.kind() == "string" {
            let txt = arg.utf8_text(source.as_bytes()).ok()?;
            return Some(
                txt.trim_matches(|c| c == '\'' || c == '"' || c == '`')
                    .to_string(),
            );
        }
    }
    None
}

/// Records top-level and class-member definitions only; locals inside function bodies are skipped.
fn extract_js_source_symbols(
    node: Node,
    source: &str,
    in_function: bool,
    defined_symbols: &mut Vec<String>,
    symbol_bodies: &mut std::collections::HashMap<String, String>,
    string_literals: &mut Vec<String>,
) {
    let cursor = &mut node.walk();
    for child in node.children(cursor) {
        match child.kind() {
            "function_declaration" | "class_declaration" | "method_definition" if !in_function => {
                if let Some(name_node) = child.child_by_field_name("name") {
                    let name = name_node.utf8_text(source.as_bytes()).unwrap_or("");
                    if !name.is_empty() {
                        defined_symbols.push(name.to_string());
                        let body = child
                            .child_by_field_name("body")
                            .and_then(|b| b.utf8_text(source.as_bytes()).ok())
                            .unwrap_or("");
                        crate::record_body(symbol_bodies, name, body);
                    }
                }
            }
            "variable_declarator" if !in_function => {
                if let Some(name_node) = child.child_by_field_name("name") {
                    let name = name_node.utf8_text(source.as_bytes()).unwrap_or("");
                    if !name.is_empty() {
                        defined_symbols.push(name.to_string());
                        let body = child
                            .child_by_field_name("value")
                            .and_then(|b| b.utf8_text(source.as_bytes()).ok())
                            .unwrap_or("");
                        crate::record_body(symbol_bodies, name, body);
                    }
                }
            }
            "string" | "number" => {
                let txt = child.utf8_text(source.as_bytes()).unwrap_or("");
                let trimmed = txt.trim_matches(|c| c == '\'' || c == '"' || c == '`');
                if !trimmed.is_empty() {
                    string_literals.push(trimmed.to_string());
                }
            }
            _ => {}
        }
        let nested = in_function
            || matches!(
                child.kind(),
                "statement_block" | "arrow_function" | "function_expression" | "function"
            );
        extract_js_source_symbols(
            child,
            source,
            nested,
            defined_symbols,
            symbol_bodies,
            string_literals,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_jest_assertions() {
        let code = r#"
describe("BillingService", () => {
    it("charges card with exact amount", () => {
        expect(charge(100)).toBe(100);
        expect(res).toBeDefined();
        expect(true).toBe(true);
    });
});
"#;
        let file = parse_javascript_test_file("src/__tests__/billing.test.ts", code, true).unwrap();
        assert_eq!(file.tests.len(), 1);
        let test = &file.tests[0];
        assert_eq!(test.id.name, "charges card with exact amount");
        assert_eq!(test.id.scope_hierarchy, vec!["BillingService"]);
        assert_eq!(test.assertions.len(), 3);
        assert_eq!(test.assertions[0].strength, AssertionStrength::Exact);
        assert_eq!(test.assertions[1].strength, AssertionStrength::Weak);
        assert_eq!(test.assertions[2].strength, AssertionStrength::Vacuous);
    }

    #[test]
    fn test_parse_jest_mock() {
        let code = r#"
jest.mock("./stripe_client");

test("charge user", () => {
    expect(true).toBe(true);
});
"#;
        let file = parse_javascript_test_file("src/test.js", code, false).unwrap();
        assert_eq!(file.harness_mocks.len(), 1);
        assert_eq!(file.harness_mocks[0].target, "./stripe_client");
    }

    #[test]
    fn test_parse_tsx_and_typeof_import_without_false_syntax_error() {
        let valid_tsx = r#"
import { describe, expect, it, vi } from "vitest";
vi.mock("react-i18next", async (importOriginal) => ({
    ...(await importOriginal<typeof import("react-i18next")>()),
}));
describe("Panel", () => {
    it("renders", () => {
        const node = <div data-testid="ok">hi</div>;
        expect(node.props).toEqual({ "data-testid": "ok" });
    });
});
"#;
        let parsed =
            parse_javascript_test_file("__tests__/panel.test.tsx", valid_tsx, true).unwrap();
        assert!(!parsed.has_syntax_error);
        assert_eq!(parsed.tests.len(), 1);

        let broken_tsx = "function broken( { return <div />; ";
        let broken = parse_javascript_source_file("src/broken.tsx", broken_tsx, true).unwrap();
        assert!(broken.has_syntax_error);
    }
}
