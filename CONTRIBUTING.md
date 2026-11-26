# Contributing to `agent-proctor`

Thank you for your interest in contributing! `agent-proctor` is a deterministic, zero-egress Rust CLI and GitHub Action for PR test-integrity receipts and verification.

## Prerequisites

- Rust toolchain (`rustup` / `cargo` stable)
- `git`
- Optional for live `verify` tests: Python (`pytest`) and/or Node.js (`jest` / `vitest`)

## Local Development

```bash
git clone https://github.com/no-hup/agent-proctor.git
cd agent-proctor

# Build and run the full test suite (including golden corpus fixtures)
cargo test --workspace
```

## Project Layout

- `crates/proctor-cli/` — CLI entry points (`scan`, `verify`, `eval`, `audit`) and output formatters (Markdown, JSON, SARIF)
- `crates/` — Core Tree-sitter AST lattice, definition-linkage analysis, and runner verification engine
- `corpus/` — Golden regression fixtures (positive cheat cases and clean refactor cases)

## Submitting Changes

1. Keep `agent-proctor` **zero-egress and deterministic** (`test_zero_network_dependencies_in_lockfile` enforces zero HTTP/socket crates in `Cargo.lock`).
2. Add a regression fixture under `corpus/` if you add or tune a rule, and ensure `cargo test --workspace` passes.
3. Open a pull request describing what changed and why.
