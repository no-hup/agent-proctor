use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mutant {
    pub file_path: String,
    pub original_snippet: String,
    pub mutated_snippet: String,
    pub line_number: usize,
    pub description: String,
}

/// Helper to attempt a single AST mutant on a given line.
fn try_mutate_line(file_path: &str, line: &str, line_num: usize) -> Option<Mutant> {
    let trimmed = line.trim();
    if trimmed.starts_with('#')
        || trimmed.starts_with("//")
        || trimmed.starts_with("assert")
        || trimmed.starts_with("expect(")
    {
        return None;
    }

    // 1. Comparison operator mutations
    if trimmed.contains(" == ") {
        let mutated = line.replacen(" == ", " != ", 1);
        return Some(Mutant {
            file_path: file_path.to_string(),
            original_snippet: line.to_string(),
            mutated_snippet: mutated,
            line_number: line_num,
            description: "Inverted equality operator (== -> !=)".to_string(),
        });
    } else if trimmed.contains(" != ") {
        let mutated = line.replacen(" != ", " == ", 1);
        return Some(Mutant {
            file_path: file_path.to_string(),
            original_snippet: line.to_string(),
            mutated_snippet: mutated,
            line_number: line_num,
            description: "Inverted inequality operator (!= -> ==)".to_string(),
        });
    } else if trimmed.contains(" > ") {
        let mutated = line.replacen(" > ", " <= ", 1);
        return Some(Mutant {
            file_path: file_path.to_string(),
            original_snippet: line.to_string(),
            mutated_snippet: mutated,
            line_number: line_num,
            description: "Inverted greater-than operator (> -> <=)".to_string(),
        });
    } else if trimmed.contains(" < ") {
        let mutated = line.replacen(" < ", " >= ", 1);
        return Some(Mutant {
            file_path: file_path.to_string(),
            original_snippet: line.to_string(),
            mutated_snippet: mutated,
            line_number: line_num,
            description: "Inverted less-than operator (< -> >=)".to_string(),
        });
    }

    // 2. Arithmetic operator mutations
    if trimmed.contains(" + ") {
        let mutated = line.replacen(" + ", " - ", 1);
        return Some(Mutant {
            file_path: file_path.to_string(),
            original_snippet: line.to_string(),
            mutated_snippet: mutated,
            line_number: line_num,
            description: "Inverted arithmetic operator (+ -> -)".to_string(),
        });
    } else if trimmed.contains(" * ") {
        let mutated = line.replacen(" * ", " / ", 1);
        return Some(Mutant {
            file_path: file_path.to_string(),
            original_snippet: line.to_string(),
            mutated_snippet: mutated,
            line_number: line_num,
            description: "Inverted multiplication operator (* -> /)".to_string(),
        });
    }

    // 3. Boolean literal mutations
    if trimmed.contains("True") && !trimmed.contains("assert") {
        let mutated = line.replacen("True", "False", 1);
        return Some(Mutant {
            file_path: file_path.to_string(),
            original_snippet: line.to_string(),
            mutated_snippet: mutated,
            line_number: line_num,
            description: "Inverted boolean literal (True -> False)".to_string(),
        });
    } else if trimmed.contains("true") && !trimmed.contains("toBe(true)") {
        let mutated = line.replacen("true", "false", 1);
        return Some(Mutant {
            file_path: file_path.to_string(),
            original_snippet: line.to_string(),
            mutated_snippet: mutated,
            line_number: line_num,
            description: "Inverted boolean literal (true -> false)".to_string(),
        });
    }

    None
}

/// Generates targeted AST mutants for the given file, prioritizing lines modified in the PR hunk.
pub fn generate_targeted_mutants(
    file_path: &str,
    content: &str,
    target_lines: Option<&HashSet<usize>>,
    max_mutants: usize,
) -> Vec<Mutant> {
    let mut mutants = Vec::new();
    let lines: Vec<&str> = content.lines().collect();

    // First pass: target explicitly modified lines from git diff
    if let Some(target) = target_lines {
        for (line_idx, line) in lines.iter().enumerate() {
            if mutants.len() >= max_mutants {
                break;
            }
            let line_num = line_idx + 1;
            if target.contains(&line_num) {
                if let Some(mutant) = try_mutate_line(file_path, line, line_num) {
                    mutants.push(mutant);
                }
            }
        }
    }

    // Fallback pass: if target lines yielded no mutants, explore remainder of file
    if mutants.is_empty() {
        for (line_idx, line) in lines.iter().enumerate() {
            if mutants.len() >= max_mutants {
                break;
            }
            let line_num = line_idx + 1;
            if let Some(mutant) = try_mutate_line(file_path, line, line_num) {
                mutants.push(mutant);
            }
        }
    }

    mutants
}

/// Backward-compatible wrapper for generating up to `max_mutants` AST mutations.
pub fn generate_mutants(file_path: &str, content: &str, max_mutants: usize) -> Vec<Mutant> {
    generate_targeted_mutants(file_path, content, None, max_mutants)
}

/// Applies a single mutant to the source content, also prepending a sentinel probe hook.
pub fn apply_mutant_with_sentinel(
    original_content: &str,
    mutant: &Mutant,
    is_python: bool,
) -> String {
    let sentinel_hook = if is_python {
        "import os\nif os.environ.get('PROCTOR_SENTINEL'):\n    open(os.environ['PROCTOR_SENTINEL'], 'a').close()\n"
    } else {
        "try { if (process.env.PROCTOR_SENTINEL) require('fs').appendFileSync(process.env.PROCTOR_SENTINEL, ''); } catch (e) {}\n"
    };

    let mut result_lines = Vec::new();
    result_lines.push(sentinel_hook.to_string());

    for (line_idx, line) in original_content.lines().enumerate() {
        if line_idx + 1 == mutant.line_number {
            result_lines.push(format!("{}\n", mutant.mutated_snippet));
        } else {
            result_lines.push(format!("{}\n", line));
        }
    }

    result_lines.concat()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mutant_generation() {
        let code = r#"
def calculate_tax(amount):
    if amount > 100:
        return amount * 0.1
    return amount * 0.05
"#;
        let mutants = generate_mutants("app/tax.py", code, 3);
        assert!(!mutants.is_empty());
        assert!(mutants.iter().any(|m| m.description.contains("> -> <=")));
    }

    #[test]
    fn test_apply_mutant_with_sentinel() {
        let code = "def calc():\n    return 10 + 5\n";
        let mutant = Mutant {
            file_path: "app/calc.py".to_string(),
            original_snippet: "    return 10 + 5".to_string(),
            mutated_snippet: "    return 10 - 5".to_string(),
            line_number: 2,
            description: "inverted +".to_string(),
        };
        let mutated = apply_mutant_with_sentinel(code, &mutant, true);
        assert!(mutated.contains("PROCTOR_SENTINEL"));
        assert!(mutated.contains("10 - 5"));
    }
}
