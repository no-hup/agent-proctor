# CLAUDE.md — Claude Code Project Guidelines

Welcome to `agent-proctor`! This repository is designed to be fully compatible with autonomous coding agents and Anthropic's Claude Code.

## ⚡ Essential Commands

Run all commands from the repository root:

- **Check Compilation:** `cargo check --workspace`
- **Run All Tests:** `cargo test --workspace` (must pass 80+ tests with 0 failures)
- **Compile Release Binary:** `cargo build --release`
- **Zero-Network Invariant Test:** `cargo test --test zero_network_verification`
- **Golden Corpus Anticheat Walk:** `cargo test --test corpus_tests`
- **Linter Check:** `cargo clippy --workspace --all-targets -- -D warnings`

---

## 🏗️ Architecture & Crates

```
crates/
├── proctor-core/     # Data models & verdicts: RuleId, RuleTier, Verdict, Policy, Waivers
├── proctor-parse/    # Tree-sitter extractors for Python & TypeScript/JavaScript ASTs
├── proctor-diff/     # Semantic AST diffing, test pairing, rename rematching, symbol linking
├── proctor-rules/    # Pure deterministic integrity rules (evaluate_rules, PROCTOR-001..008)
├── proctor-probe/    # Dynamic ephemeral worktrees, mutation sentinel injection, test runner
└── proctor-cli/      # CLI frontend, JSON/SARIF output, and Test-Change Receipt markdown
```

---

## 🛡️ Non-Negotiable Rules & Invariants

1. **Zero LLMs in the Loop:** Never add LLM API calls, prompt strings, or non-deterministic heuristics to verification rules. All analysis must be 100% deterministic and reproducible via Tree-sitter AST diffing.
2. **Zero Network Dependencies:** Never add crates that introduce network, HTTP, or telemetry dependencies. The lockfile must remain zero-network (`cargo test --test zero_network_verification` must always pass).
3. **Sub-200ms Static Latency:** `proctor scan` runs in git hooks and CI pipelines; keep static passes fast (<50ms).
4. **Tight Boundaries:** Do not perform unrequested refactorings or alter unrelated files. Only edit the crates specified in the task issue.

---

## 📋 Pull Request Protocol

1. Write tests for any new syntax parsing, diffing logic, or rule calibrations.
2. Verify that `cargo test --workspace` passes cleanly.
3. Check `git diff --stat` to ensure only intended files were modified.
4. Reference the issue number in the PR description (e.g. `Fixes #2`).
