# AGENTS.md — Agent Contributor Manual

This repository is **agent-ready**. Autonomous coding agents (Claude Code, Devin, GitHub Copilot, Codex, Cursor) and human contributors can safely pick up tasks and open pull requests by following this contract.

---

## ⚡ 1. Fast Command Reference

All commands must be executed from the repository root:

| Action | Command | Expected Output / SLA |
|---|---|---|
| **Check Compilation** | `cargo check --workspace` | Clean check, zero errors |
| **Run Unit & Integration Tests** | `cargo test --workspace` | **80+ passed, 0 failed** (<3 seconds) |
| **Verify Release Build** | `cargo build --release` | Outputs `target/release/proctor` |
| **Zero-Network Invariant** | `cargo test --test zero_network_verification` | Asserts no network/telemetry crates in lockfile |
| **Golden Corpus Anticheat Walk** | `cargo test --test corpus_tests` | Asserts exact verdicts on `corpus/` fixtures |
| **Run Linter** | `cargo clippy --workspace --all-targets -- -D warnings` | Zero clippy warnings |

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

---

## 📋 3. Agent Task Authoring & Execution Protocol

When authoring or picking up an issue, follow the **4-Part Agent Task Protocol**:

### A. Objective
A concise, one-sentence statement of what needs to be added, fixed, or calibrated.

### B. Allowed Boundary (Scope Constraint)
Agents must explicitly restrict edits to named files or directories. Never perform broad or unrequested refactorings outside the specified boundary.
* *Example:* `Edit only crates/proctor-parse/src/javascript.rs and crates/proctor-rules/src/lib.rs`.

### C. Acceptance Criteria (Testable Checkpoints)
Break tasks into discrete, binary verification checkpoints:
* `AC-1:` Adding/modifying the feature passes existing test suites with zero regressions.
* `AC-2:` A new test fixture is added under `corpus/` or integration test suite proving the behavior.
* `AC-3:` In default mode, benign updates produce zero false-positive findings.

### D. Deterministic Verification Command
Every task must end with the exact command to run:
```bash
cargo test --workspace && cargo build --release
```

---

## 🚀 4. How Agents Should Open Pull Requests

1. **Commit Message Convention:** Follow Conventional Commits:
   - `feat(rules): add Go testing.T table-driven test support`
   - `fix(reporter): group repetitive assertion edits per file`
   - `test(corpus): add fixture for mocha describe.skip`
2. **PR Description:** Include:
   - Summary of changes.
   - List of acceptance criteria checked off (`[x] AC-1`, `[x] AC-2`).
   - Output of `cargo test --workspace`.
3. **Self-Review:** Ensure `git diff --stat` touches *only* the files within the assigned boundary.
