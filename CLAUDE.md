# CLAUDE.md — Claude Code Contributor Guidelines

Welcome to `agent-proctor`! We actively welcome autonomous coding agents and human contributors using Anthropic's Claude Code.

Because `agent-proctor` is an anticheat and test-integrity engine, our standards for code rigor are exceptionally high. An agent's greatest superpower is tireless verification—we expect every contributor agent to act as its own adversarial reviewer and execute comprehensive pre-flight verification before opening a pull request.

---

## ⚡ Essential Commands

Execute all commands from the repository root:

- **Check Compilation:** `cargo check --workspace`
- **Run Workspace Tests:** `cargo test --workspace` (must pass 80+ tests with 0 failures)
- **Compile Release Binary:** `cargo build --release`
- **Zero-Network Invariant Test:** `cargo test --test zero_network_verification`
- **Golden Corpus Anticheat Walk:** `cargo test --test corpus_tests`
- **Zero-Warning Linter Check:** `cargo clippy --workspace --all-targets -- -D warnings`
- **Self-Proctoring Diff Check:** `cargo run --bin proctor -- scan --base origin/main`

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

## 🛡️ Non-Negotiable Invariants

1. **Zero LLMs in the Verification Loop:** Never add LLM API calls, prompt strings, or non-deterministic heuristics to verification rules. All analysis must be 100% deterministic and reproducible via Tree-sitter AST diffing.
2. **Zero Network Dependencies:** Never add crates that introduce network, HTTP, or telemetry dependencies. The lockfile must remain zero-network (`cargo test --test zero_network_verification` must always pass).
3. **Sub-200ms Static Latency:** `proctor scan` runs in git hooks and CI pipelines; keep static passes fast (<50ms).
4. **Strict Diff Hygiene:** Do not perform unrequested refactorings or alter unrelated files. Only edit the crates specified in the task issue.

---

## 🔬 The 5-Point Mandatory Pre-Flight Battery

Before opening any PR, an agent must execute and satisfy every item in this battery:

1. **Clean Linting:** `cargo clippy --workspace --all-targets -- -D warnings` exits 0 with zero warnings.
2. **Full Regression Suite:** `cargo test --workspace` passes with 0 failures.
3. **Paired Test Proof:** If modifying or adding syntax parsing or rules, you **must** supply both:
   - A positive test case proving the new syntax/rule is caught or parsed.
   - A negative test case proving clean code produces **zero false-positive findings**.
4. **Self-Proctoring Gate:** Run `cargo run --bin proctor -- scan --base origin/main`. Your own diff must evaluate to `🟢 PASS` with zero unexpected findings.
5. **Diff Audit:** Run `git diff --stat` and verify that **only** files strictly necessary for the objective were touched.

---

## 📋 Pull Request Protocol

In your pull request description, you must:
1. State the objective and reference the issue number (e.g. `Fixes #2`).
2. Include the output of `git diff --stat`.
3. Include the terminal output or explicit confirmation of the 5-point Pre-Flight Battery.
