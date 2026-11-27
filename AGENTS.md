# AGENTS.md — Autonomous Agent Contributor Contract

This repository is **agent-ready** and actively welcomes contributions from autonomous coding agents (Claude Code, Devin, GitHub Copilot, Codex, Cursor) and human developers.

Because `agent-proctor` is an anticheat and test-integrity engine, our code quality standards are uncompromising. An agent's core advantage is infinite patience and tireless testing: **we place the onus of validation heavily on contributor agents before requesting human maintainer review.**

---

## ⚡ 1. Fast Command Reference

All commands must be executed from the repository root:

| Action | Command | Required SLA |
|---|---|---|
| **Check Compilation** | `cargo check --workspace` | Clean check, 0 errors |
| **Run Full Test Suite** | `cargo test --workspace` | **80+ passed, 0 failed** (<3s) |
| **Verify Release Build** | `cargo build --release` | Outputs `target/release/proctor` |
| **Zero-Network Invariant** | `cargo test --test zero_network_verification` | Asserts no network crates in lockfile |
| **Golden Corpus Anticheat Walk** | `cargo test --test corpus_tests` | Asserts exact verdicts on `corpus/` fixtures |
| **Zero-Warning Linter** | `cargo clippy --workspace --all-targets -- -D warnings` | Zero clippy warnings |
| **Self-Proctoring Diff Scan** | `cargo run --bin proctor -- scan --base origin/main` | Evaluates own diff; must pass `🟢 PASS` |

---

## 🏗️ 2. Architectural Blueprint & Crate Map

Proctor is a multi-crate Rust workspace designed for zero-egress, deterministic AST analysis:

```
crates/
├── proctor-core/     # Pure data models, Enums: RuleId, RuleTier, Verdict, Policy, Waivers.
├── proctor-parse/    # Tree-sitter AST extractors for Python & TypeScript/JavaScript tests & source.
├── proctor-diff/     # Semantic AST diffing, test pairing, symbol overlap, rename detection.
├── proctor-rules/    # Pure deterministic integrity rules (evaluate_rules, PROCTOR-001..008).
├── proctor-probe/    # Dynamic ephemeral worktrees, mutation sentinel injection, pytest/jest runner.
└── proctor-cli/      # CLI frontend (proctor scan, proctor verify), SARIF, JSON, and Receipt reporter.
```

### Key Invariants (Non-Negotiable)
1. **Zero LLMs in the Loop:** Never add LLM calls, prompt strings, or API clients to verification rules. Analysis must be 100% deterministic and reproducible via Tree-sitter.
2. **Zero Network Dependencies:** Never add crates that pull in HTTP, telemetry, or network networking libraries to core analysis crates. The `test_zero_network_dependencies_in_lockfile` test must always pass.
3. **Sub-200ms Execution for Static Scan:** `proctor scan` must remain fast enough to run in pre-commit hooks and local agent tool loops.
4. **Strict Scope Confinement:** Never wander into unrequested refactorings. Modifying files outside the task's stated boundary will result in immediate rejection.

---

## 🔬 3. The Mandatory Pre-Flight Verification Battery

Before generating a commit or opening a pull request, the agent **must execute and satisfy** all five checks:

1. **`cargo check --workspace`**: Clean compilation across all crates.
2. **`cargo clippy --workspace --all-targets -- -D warnings`**: Strict zero-warning linter enforcement.
3. **`cargo test --workspace`**: All 80+ unit and integration tests must pass with 0 failures.
4. **Paired Test Proof (Precision & Recall):**
   - Any new rule, parser change, or calibration must be accompanied by tests verifying the new behavior.
   - You must also verify that clean, legitimate code produces **zero false-positive findings** in default mode.
5. **Self-Proctoring Gate:**
   - Execute: `cargo run --bin proctor -- scan --base origin/main`
   - Proctor must inspect your own branch diff and award a `🟢 PASS` verdict. If Proctor flags your own changes, resolve the findings locally before raising a PR.

---

## 📋 4. Agent Task Authoring & PR Submission Contract

When submitting a Pull Request, the agent's PR description must follow this structure:

1. **Objective:** 1-sentence statement referencing the issue (e.g. `Fixes #2`).
2. **Diff Hygiene (`git diff --stat`):** Paste the exact `git diff --stat` output confirming that only in-scope files were modified.
3. **Verification Evidence:** Paste the terminal output confirming that the 5-Point Pre-Flight Battery passed locally.

PRs that fail to include pre-flight verification evidence or fail the self-proctoring check will be automatically closed as incomplete.
