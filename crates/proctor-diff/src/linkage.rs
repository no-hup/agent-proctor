use crate::SemanticDiff;
use proctor_parse::ParsedTest;
use std::path::Path;

/// Structured verdict describing whether an assertion or test modification
/// legitimately co-evolves with changed production code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkageVerdict {
    Linked { evidence: String },
    TransitiveAmbiguity { chain: String },
    Unlinked { reason: String },
}

impl LinkageVerdict {
    pub fn is_linked(&self) -> bool {
        matches!(self, LinkageVerdict::Linked { .. })
    }

    pub fn is_ambiguous(&self) -> bool {
        matches!(self, LinkageVerdict::TransitiveAmbiguity { .. })
    }

    pub fn evidence_or_reason(&self) -> &str {
        match self {
            LinkageVerdict::Linked { evidence } => evidence,
            LinkageVerdict::TransitiveAmbiguity { chain } => chain,
            LinkageVerdict::Unlinked { reason } => reason,
        }
    }
}

/// Black-box trait for determining code-to-test dependency linkage.
pub trait LinkageStrategy: Send + Sync {
    fn check_linkage(
        &self,
        base_test: &ParsedTest,
        head_test: &ParsedTest,
        diff: &SemanticDiff,
    ) -> LinkageVerdict;
}

/// Baseline: Intersects bare identifiers in test bodies with changed production symbol names.
#[derive(Debug, Default, Clone, Copy)]
pub struct BareTokenLinker;

impl LinkageStrategy for BareTokenLinker {
    fn check_linkage(
        &self,
        base_test: &ParsedTest,
        head_test: &ParsedTest,
        diff: &SemanticDiff,
    ) -> LinkageVerdict {
        for sym in base_test
            .body_symbols
            .iter()
            .chain(head_test.body_symbols.iter())
        {
            if diff.changed_symbols.contains(sym) {
                return LinkageVerdict::Linked {
                    evidence: format!(
                        "Bare identifier '{}' matches modified production symbol",
                        sym
                    ),
                };
            }
        }
        LinkageVerdict::Unlinked {
            reason: "No bare identifier in test matches changed production symbols".to_string(),
        }
    }
}

/// Candidate: Scoped Linker that requires an import reference or directory proximity
/// before acknowledging that a matching symbol name represents real co-evolution.
#[derive(Debug, Default, Clone, Copy)]
pub struct ScopedImportLinker;

impl LinkageStrategy for ScopedImportLinker {
    fn check_linkage(
        &self,
        base_test: &ParsedTest,
        head_test: &ParsedTest,
        diff: &SemanticDiff,
    ) -> LinkageVerdict {
        let test_file = &head_test.id.file_path;
        let test_imports = diff.test_imports.get(test_file);

        // Find candidate matching symbols
        let mut matching_symbols = Vec::new();
        for sym in base_test
            .body_symbols
            .iter()
            .chain(head_test.body_symbols.iter())
        {
            if diff.changed_symbols.contains(sym) {
                matching_symbols.push(sym.clone());
            }
        }

        if matching_symbols.is_empty() {
            return LinkageVerdict::Unlinked {
                reason: "No identifiers in test match changed production symbols".to_string(),
            };
        }

        // Verify if any changed source file that defines one of these symbols is actually
        // imported by the test file or co-located by directory/path proximity.
        for (src_path, src) in &diff.head_sources {
            let src_defines_match = matching_symbols
                .iter()
                .any(|sym| src.defined_symbols.contains(sym));
            if !src_defines_match {
                continue;
            }

            // 1. Check path proximity (e.g. tests/auth/test_login.py <-> app/auth/login.py)
            if check_path_proximity(test_file, src_path) {
                return LinkageVerdict::Linked {
                    evidence: format!(
                        "Path proximity confirmed between test '{}' and modified source '{}'",
                        test_file, src_path
                    ),
                };
            }

            // 2. Check import statement linkage
            if let Some(imports) = test_imports {
                let mod_stem = Path::new(src_path)
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("");
                let mod_path = src_path
                    .trim_end_matches(".py")
                    .trim_end_matches(".ts")
                    .trim_end_matches(".js")
                    .trim_end_matches(".go")
                    .replace('/', ".");

                for imp in imports {
                    if imp.contains(mod_stem) || imp.contains(&mod_path) {
                        return LinkageVerdict::Linked {
                            evidence: format!(
                                "Test imports modified module '{}' (import: '{}')",
                                mod_path, imp
                            ),
                        };
                    }
                }
            }
        }

        // Also check if production code was modified in the same package (for Go / co-located tests)
        let test_dir = Path::new(test_file).parent().unwrap_or(Path::new(""));
        for src_path in diff.head_sources.keys() {
            let src_dir = Path::new(src_path).parent().unwrap_or(Path::new(""));
            if test_dir == src_dir && !test_dir.as_os_str().is_empty() {
                return LinkageVerdict::Linked {
                    evidence: format!(
                        "Co-located in same directory '{}' with modified source '{}'",
                        test_dir.display(),
                        src_path
                    ),
                };
            }
        }

        // Stage 2: Bounded 2-hop transitive check (excluding ubiquitous hub modules)
        if let Some(imports) = test_imports {
            for (src_path, src) in &diff.head_sources {
                let src_defines_match = matching_symbols
                    .iter()
                    .any(|sym| src.defined_symbols.contains(sym));
                if !src_defines_match {
                    continue;
                }
                let target_stem = Path::new(src_path)
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("");

                // Check intermediate sources in raw_head_files
                for (inter_path, inter_content) in &diff.raw_head_files {
                    if inter_path == src_path
                        || crate::is_test_file(inter_path)
                        || is_hub_module(inter_path)
                    {
                        continue;
                    }
                    let inter_stem = Path::new(inter_path)
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("");

                    if inter_stem.is_empty() {
                        continue;
                    }

                    // Does test import the intermediate file?
                    let test_imports_inter = imports.iter().any(|imp| imp.contains(inter_stem));
                    if test_imports_inter && inter_content.contains(target_stem) {
                        return LinkageVerdict::TransitiveAmbiguity {
                            chain: format!("{} -> {} -> {}", test_file, inter_path, src_path),
                        };
                    }
                }
            }
        }

        // If symbols matched by name but there is zero import or path connection:
        // this is the classic "format" or "validate" name collision!
        LinkageVerdict::Unlinked {
            reason: format!(
                "Symbol(s) {:?} matched by name, but test '{}' has no import or path proximity to changed sources",
                matching_symbols, test_file
            ),
        }
    }
}

/// Identifies ubiquitous hub modules that should not act as transitive bridges
/// to prevent small-world graph dilution.
fn is_hub_module(path: &str) -> bool {
    let lower = path.to_lowercase();
    let stem = Path::new(&lower)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("");
    matches!(
        stem,
        "config"
            | "configs"
            | "types"
            | "constants"
            | "constant"
            | "utils"
            | "util"
            | "logger"
            | "logging"
            | "exceptions"
            | "error"
            | "errors"
            | "helpers"
    )
}

/// Normalizes test and source paths to see if they refer to the same subsystem/component.
fn check_path_proximity(test_path: &str, src_path: &str) -> bool {
    let clean_test = test_path
        .replace("tests/", "")
        .replace("test/", "")
        .replace("__tests__/", "")
        .replace("test_", "")
        .replace("_test", "");
    let clean_src = src_path
        .replace("src/", "")
        .replace("app/", "")
        .replace("lib/", "")
        .replace("pkg/", "");

    let test_stem = Path::new(&clean_test)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("");
    let src_stem = Path::new(&clean_src)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("");

    test_stem == src_stem
        || clean_test.starts_with(&clean_src)
        || clean_src.starts_with(&clean_test)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_scoped_linker_rejects_unimported_name_collision() {
        let mut base_files = HashMap::new();
        base_files.insert(
            "src/utils/format.py".to_string(),
            "def format(val):\n    return str(val)\n".to_string(),
        );
        base_files.insert(
            "tests/test_auth.py".to_string(),
            "from app.auth import login\ndef test_auth():\n    assert format('x') == 'x'\n"
                .to_string(),
        );

        let mut head_files = HashMap::new();
        head_files.insert(
            "src/utils/format.py".to_string(),
            "def format(val):\n    return f'formatted_{val}'\n".to_string(),
        );
        head_files.insert(
            "tests/test_auth.py".to_string(),
            "from app.auth import login\ndef test_auth():\n    assert format('y') == 'y'\n"
                .to_string(),
        );

        let diff = SemanticDiff::build(&base_files, &head_files).unwrap();
        assert_eq!(diff.paired_tests.len(), 1);
        let pair = &diff.paired_tests[0];

        // BareTokenLinker falsely links because 'format' symbol matches
        let bare = BareTokenLinker;
        let bare_verdict = bare.check_linkage(&pair.base_test, &pair.head_test, &diff);
        assert!(bare_verdict.is_linked());

        // ScopedImportLinker correctly flags it as Unlinked (no import or proximity to src/utils/format.py)
        let scoped = ScopedImportLinker;
        let scoped_verdict = scoped.check_linkage(&pair.base_test, &pair.head_test, &diff);
        assert!(!scoped_verdict.is_linked());
        assert!(scoped_verdict
            .evidence_or_reason()
            .contains("no import or path proximity"));
    }

    #[test]
    fn test_scoped_linker_detects_2hop_transitive_ambiguity() {
        let mut base_files = HashMap::new();
        base_files.insert(
            "src/repo/user_repo.py".to_string(),
            "def get_user(uid):\n    return {'id': uid}\n".to_string(),
        );
        base_files.insert(
            "src/services/user_service.py".to_string(),
            "from src.repo import user_repo\ndef load_user(uid):\n    return user_repo.get_user(uid)\n".to_string(),
        );
        base_files.insert(
            "tests/test_controller.py".to_string(),
            "from src.services import user_service\ndef test_profile():\n    assert get_user(1) == {'id': 1}\n".to_string(),
        );

        let mut head_files = HashMap::new();
        head_files.insert(
            "src/repo/user_repo.py".to_string(),
            "def get_user(uid):\n    return {'id': uid, 'v': 2}\n".to_string(),
        );
        head_files.insert(
            "src/services/user_service.py".to_string(),
            "from src.repo import user_repo\ndef load_user(uid):\n    return user_repo.get_user(uid)\n".to_string(),
        );
        head_files.insert(
            "tests/test_controller.py".to_string(),
            "from src.services import user_service\ndef test_profile():\n    assert get_user(1) == {'id': 1, 'v': 2}\n".to_string(),
        );

        let diff = SemanticDiff::build(&base_files, &head_files).unwrap();
        assert_eq!(diff.paired_tests.len(), 1);
        let pair = &diff.paired_tests[0];

        let scoped = ScopedImportLinker;
        let scoped_verdict = scoped.check_linkage(&pair.base_test, &pair.head_test, &diff);
        assert!(scoped_verdict.is_ambiguous());
        assert!(scoped_verdict
            .evidence_or_reason()
            .contains("user_service.py -> src/repo/user_repo.py"));
    }

    #[test]
    fn test_scoped_linker_rejects_hub_module_transitive_bridge() {
        let mut base_files = HashMap::new();
        base_files.insert(
            "src/repo/user_repo.py".to_string(),
            "def get_user(uid):\n    return {'id': uid}\n".to_string(),
        );
        base_files.insert(
            "src/constants.py".to_string(),
            "from src.repo import user_repo\nTIMEOUT = 30\n".to_string(),
        );
        base_files.insert(
            "tests/test_billing.py".to_string(),
            "from src import constants\ndef test_charge():\n    assert get_user(1) == {'id': 1}\n"
                .to_string(),
        );

        let mut head_files = HashMap::new();
        head_files.insert(
            "src/repo/user_repo.py".to_string(),
            "def get_user(uid):\n    return {'id': uid, 'v': 2}\n".to_string(),
        );
        head_files.insert(
            "src/constants.py".to_string(),
            "from src.repo import user_repo\nTIMEOUT = 60\n".to_string(),
        );
        head_files.insert(
            "tests/test_billing.py".to_string(),
            "from src import constants\ndef test_charge():\n    assert get_user(1) == {'id': 1, 'v': 2}\n".to_string(),
        );

        let diff = SemanticDiff::build(&base_files, &head_files).unwrap();
        assert_eq!(diff.paired_tests.len(), 1);
        let pair = &diff.paired_tests[0];

        let scoped = ScopedImportLinker;
        let scoped_verdict = scoped.check_linkage(&pair.base_test, &pair.head_test, &diff);
        // constants.py is a hub module, so it is blocked from acting as a transitive bridge!
        assert!(!scoped_verdict.is_linked());
        assert!(!scoped_verdict.is_ambiguous());
    }
}
