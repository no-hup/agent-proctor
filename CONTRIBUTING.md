# Contributing to `agent-proctor`

Thank you for your interest in contributing! `agent-proctor` is a deterministic, zero-egress Rust CLI and GitHub Action for PR test-integrity receipts and anticheat verification.

We actively welcome contributions from **both human developers and autonomous AI coding agents**.

---

## Prerequisites

- Rust toolchain (`rustup` / `cargo` stable)
- `git`
- Optional for dynamic verification (`proctor verify`): Python (`pytest`) and/or Node.js (`jest` / `vitest`)

---

## Local Development & Quick Start

```bash
git clone https://github.com/no-hup/agent-proctor.git
cd agent-proctor

# Build and run the full test suite (including golden corpus fixtures)
cargo test --workspace
```

---

## Project Layout

- `crates/proctor-core/` — Core types, enums (`RuleId`, `RuleTier`, `Verdict`), and policy configuration.
- `crates/proctor-parse/` — Tree-sitter AST extractors for Python and TypeScript/JavaScript.
- `crates/proctor-diff/` — Semantic diffing, test pairing, symbol overlap, and rename rematching.
- `crates/proctor-rules/` — Pure deterministic integrity rules (`evaluate_rules`, PROCTOR-001..008).
- `crates/proctor-probe/` — Ephemeral worktree isolation, mutation sentinel injection, and dynamic test runners.
- `crates/proctor-cli/` — CLI frontend (`scan`, `verify`, `eval`, `audit`) and formatters (Markdown Receipt, JSON, SARIF).
- `corpus/` — Golden regression fixtures (positive cheat cases and clean refactor cases).

---

## 🤖 For AI Coding Agents & Automated Contributors

Because `agent-proctor` is an anticheat and test-integrity engine, our code quality bar is exceptionally high. An agent's greatest superpower is tireless verification—we expect contributor agents to run exhaustive local verification before requesting maintainer review.

### The Mandatory Pre-Flight Battery
Before submitting a PR, every contributor (human or agent) must run and pass:

1. **Linter:** `cargo clippy --workspace --all-targets -- -D warnings` (must pass with 0 warnings).
2. **Test Suite:** `cargo test --workspace` (must pass 80+ tests with 0 failures).
3. **Zero-Network Invariant:** `cargo test --test zero_network_verification` (asserts zero network crates in `Cargo.lock`).
4. **Golden Corpus Walk:** `cargo test --test corpus_tests` (asserts exact verdicts on all golden fixtures).
5. **Self-Proctoring Check:** `cargo run --bin proctor -- scan --base origin/main` (Proctor must inspect your own branch diff and return `🟢 PASS`).

### Strict Diff Hygiene
- Do not perform unrequested refactorings or reformat unrelated files.
- Restrict changes strictly to the crate boundaries specified in the issue.
- If you touch `Cargo.lock`, verify that zero network or telemetry dependencies were added.

---

## Submitting Pull Requests

1. Open a PR with a clear summary referencing the issue number (e.g. `Fixes #2`).
2. Follow the [Pull Request Template](.github/PULL_REQUEST_TEMPLATE.md) and check off all pre-flight verification items.
3. For detailed agent instructions, see [AGENTS.md](AGENTS.md) and [CLAUDE.md](CLAUDE.md).
