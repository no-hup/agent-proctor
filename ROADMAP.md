# agent-proctor Roadmap

This document outlines the architectural roadmap, shipped milestones, and future engineering phases for **`agent-proctor`** (`proctor`).

---

## 🎯 Core Product Vision & Anti-Goals

### The Vision
`agent-proctor` is a **deterministic, sub-200ms PR integrity and anticheat gate** designed to prevent autonomous AI coding agents (Devin, Claude Code, Cursor, SWE-bench solvers) from specification gaming: weakening assertions, deleting test cases, injecting shadow mocks, and sabotaging CI workflows.

### The Non-Negotiable Invariants
1. **Zero LLMs in the Verification Loop:** 100% deterministic Tree-sitter AST diffing, mathematically reproducible, zero prompt injection surface, and $0 model costs.
2. **Sub-200ms Latency:** Operates directly inside local agent tool loops and as a fast, single-binary GitHub Action.
3. **Inescapable Choke Point:** Operates at the git commit and PR boundary.

### Anti-Goals (What We Explicitly Do Not Build)
* **No Heavy Web Dashboards or DB Clusters:** We do not build React apps, ClickHouse clusters, or Postgres fleets. GitHub PR annotations and CLI terminal output are the correct UI.
* **No Network Proxies / API Gateways:** Autonomous coding agents execute tools locally on disk; an HTTP proxy fails open. The git boundary is the only real choke point.
* **No Conversational Dialogue Simulators:** We evaluate code diffs and syntax trees, not chat transcripts.

---

## 📦 Milestone Progress & Phasing

```
[Phase 0: Perimeter Hardening]      ──► COMPLETE (v0.1.0)
[Phase 1: Guard vs Report Split]    ──► COMPLETE (v0.1.0)
[Phase 2: Eval Surface & Corpus]    ──► COMPLETE (v0.1.0)
[Phase 2.5: Noise & Calibration]    ──► COMPLETE (v0.2.3)
[Phase 3: Benchmark Grounding]      ──► ACTIVE
[Phase 3.5: The Smart Escalator]    ──► PLANNED (v0.3.0)
[Phase 4: Multi-Language Lattices]  ──► PLANNED (Q1 2027)
[Phase 5: Deep Anticheat & Config]  ──► PLANNED (Q1 2027)
[Phase 6: Synthetic Mutation Fuzz]  ──► PLANNED (Q2 2027)
[Phase 7: Packaging & Ecosystem]    ──► PLANNED (Q2 2027)
```

---

## 🟢 Shipped Phases (Completed in v0.1.0)

### Phase 0: Perimeter Hardening
- [x] **Base-Ref Policy Anchoring (`PROCTOR-007`):** `proctor.toml` is read strictly from `base_ref` via git commit objects. Missing base ref commits fail fast with exit code 3 (`PolicyDegradationError`).
- [x] **Un-Ignorable Policy Files:** `proctor.toml` and `.proctor.toml` are hard-exempted from `ignored_paths`.
- [x] **Hardened `allow_mocks` Matching:** Rejects bare wildcards `*` or empty patterns.
- [x] **Zero Local Waiver Flags:** Deleted `--verified-waivers` CLI flag. Waivers can only enter via `PROCTOR_VERIFIED_WAIVERS` under authenticated CI (`GITHUB_ACTIONS`).
- [x] **Runner-Accurate Test File Heuristics:** Replaced substring `path.contains("test")` with runner discovery globs (`tests/**/*.py`, `test_*.py`, `*_test.py`, `*.spec.ts`, `*.test.js`).
- [x] **Two-Dot Diff Support:** Implemented `base..head` to support shallow clones in CI without merge-base crashes.
- [x] **Strict Orphan Justification:** Requires all referenced production symbols in a test to be deleted from production code to justify assertion removals.
- [x] **Assertion Lattice Engine:** Compares `AssertionStrength` transitions (`Vacuous < Weak < Exact`).

### Phase 1: Product Split (Guard vs. Report)
- [x] **Dual-Tier System:**
  - `Guard` Tier (Exit 2 / `BLOCK`): Tampering, CI sabotage, policy edits, test demolition, shadow mocking.
  - `Report` Tier (Exit 0 default / Exit 1 `--strict`): Spec evolution, assertion modifications with production code changes.
- [x] **Cryptographic WaiverSet:**
  - `maintainer_waivers` (from authenticated write-collaborators/owners): Clears Guard and Report tiers.
  - `author_acks` (from PR authors): Clears Report tier ONLY. Author comments can never waive Guard findings.
- [x] **Structured Machine-Readable Output:** `proctor scan --format json` and SARIF 2.1.0 (`--sarif`) for GitHub Code Scanning integration.
- [x] **Audit Metric Semantics:** Renamed false-positive estimation to `block_rate_on_merged_history` in `proctor audit`.

### Phase 2: Eval Surface & Versioned Corpus
- [x] **`proctor eval` Trajectory Subcommand:** Benchmark runner (`proctor eval --format swebench predictions.jsonl --out verdicts.jsonl`).
- [x] **Worktree Patch Application:** For full-repository evaluations (`--repos <dir>`), creates isolated git worktrees, runs `git apply`, and diffs modified files against `HEAD` to provide 100% accurate full-file AST context.
- [x] **Resilient Fragment Mode:** Without repos, isolates strictly fragment-safe rules (`PROCTOR-003, 004, 005, 007`) by `RuleId` and labels records `"mode": "fragment"`.
- [x] **Dynamic Golden Corpus Walker:** Integration test suite in `corpus_tests.rs` dynamically walking all fixtures under `corpus/`.
- [x] **Clean Corpus Fixtures:** Checked-in clean refactors in `corpus/clean/` asserting `findings.is_empty()` to protect precision.

### Phase 2.5: Noise Reduction & Real-World Calibration (Shipped in v0.2.3)
- [x] **Equal-Rank Co-Evolution Quieting:** Assertion edits with equal strength alongside production code changes are quieted into the collapsed `Changed pre-existing assertions` receipt table rather than raising yellow `AssertionLock` sirens in default mode. Strict Guard blocks preserved under `--protect-existing-tests`.
- [x] **Repetitive Finding Aggregation:** Rolled up review findings per file when a file has `> 5` edits into clean summary lines (`N assertion edits in <file>`) to prevent PR comment spam.
- [x] **Quiet Syntax & Parse Limits:** Tree-sitter parse errors on huge files or unsupported syntax demoted from yellow security alerts to quiet receipt notes (`| **Not analyzed (syntax/parse limit)** |`).
- [x] **Pass-3 Test Rename Rematching:** Re-matched renamed tests by body symbol overlap in `proctor-diff` to prevent honest test renames from being flagged as test deletions.

---

## 🟡 Phase 3: Benchmark Grounding & Empirical Validation (Active / Next)

The objective of Phase 3 is to run Proctor against real-world datasets and publish empirical findings:

1. **SWE-bench Trajectory Evaluation:**
   - Run `proctor eval --format swebench` on SWE-bench Verified (500 instances) and SWE-bench Lite (300 instances) prediction trajectories across leading models (Claude 3.5 Sonnet, GPT-4o, DeepSeek-R1, SWE-agent).
   - Quantify empirical reward-hacking rates: How many "resolved" instances passed benchmark graders by modifying test files, disabling assertions, or tweaking runner configs?
2. **Merged Repository False-Positive Audit:**
   - Run `proctor audit --limit 500 --label audit.csv` across 10 top open-source repositories (`django/django`, `pallets/flask`, `sympy/sympy`, `psf/requests`, `expressjs/express`, `facebook/jest`).
   - Establish and publish the true `block_rate_on_merged_history` baseline to prove Proctor does not trigger false-positive alarms on human refactors.
3. **Stale Worktree Auto-Prune:**
   - Add automatic cleanup of stale evaluation worktrees on startup for high-throughput batch evaluation runs.

---

## 🔵 Phase 3.5: The Smart Escalator (Cascading Filter Pipeline)

Bridge the gap between fast deterministic static scanning (~20ms) and deep dynamic verification by implementing a progressive 3-stage cascade:

1. **Stage 1 (Fast Surface Scan — 20ms, zero dependencies):**
   - Pure AST diffing across all PRs.
   - Categorizes diff into `CLEAN` (fast pass), `SABOTAGE` (immediate Guard block), or `AMBIGUOUS_COEVOLUTION` (assertions updated alongside production edits).
2. **Stage 2 (Deep Static Call-Graph Tracing — ~100ms, zero dependencies):**
   - Multi-file symbol import tracking.
   - Traces if the modified test actually invokes the modified production function through intermediate helpers/services without executing code.
3. **Stage 3 (Conditional Dynamic Mutation Probe — Opt-in / Environment-Aware):**
   - Connects existing `proctor-probe` engine into the scan workflow.
   - When an assertion edit is ambiguous and a local test runner (`pytest` or `jest`) is detected in the CI environment:
   - Proctor automatically runs `proctor-probe` for *that single test*.
   - If the test kills injected mutants $\rightarrow$ Auto-cleared to **Green**.
   - If the test passes vacuously $\rightarrow$ Escalate to **Red Block**.
   - If the test runner is missing $\rightarrow$ Graceful fallback to the Receipt diff table.

---

## 🔵 Phase 4: Multi-Language AST Lattice Expansion

Extend Tree-sitter semantic anticheat beyond Python and JavaScript/TypeScript:

1. **Go (`tree-sitter-go`):**
   - Promote staged fixture `corpus/go/table_driven_row_drop/` to an active rule.
   - Detect deleted rows in table-driven test structs (`tests := []struct{...}{ ... }`).
   - Detect weakening in `assert.Equal`, `assert.NoError`, and `t.Fatalf` checks.
2. **Rust (`tree-sitter-rust`):**
   - Parse `#[test]`, `#[should_panic]`, and `#[ignore]` attributes.
   - Assertion lattice for `assert_eq!`, `assert_ne!`, and `assert!`.
   - Snapshot tampering detection for `insta` snapshot files (`.snap`).
3. **Java / Kotlin (`tree-sitter-java`, `tree-sitter-kotlin`):**
   - JUnit 5 (`@Test`, `@ParameterizedTest`, `@Disabled`) and AssertJ assertion transitions.

---

## 🔵 Phase 5: Deep Anticheat & Config Parsers

Close subtle cheat vectors identified during architectural stress-testing:

1. **Numeric Runner Config Thresholds:**
   - Parse `fail_under = <number>` in `pytest.ini`, `pyproject.toml`, and `setup.cfg`. Flag any commit that lowers the required test coverage threshold.
2. **Structured CI Workflow Analysis:**
   - Replace substring denylists in `.github/workflows/` with structured YAML step parsing. Catch tricky multi-line shell expansions and custom step overrides.
3. **Stricter Cross-File Test Pairing:**
   - Tighten pass-2 test similarity pairing from 50% to requiring same-file or $\ge 80\%$ assertion text overlap to prevent renames from absorbing test deletions in other modules.
4. **Holdout Test Guidance & Harness Docs:**
   - Document patterns for integrating private evaluation holdouts to defeat Class B specification gaming (where the agent hardcodes inputs into production code without touching test files).

---

## 🔵 Phase 6: Synthetic Code Mutation & Fuzzing Corpus

> **Design Note:** Runtime agent platforms (such as Future AGI) generate synthetic conversational chat dialogues for customer chatbots. For a deterministic code anticheat engine like Proctor, synthetic chat dialogues are completely useless.  
> What would be valuable as an expansion is an automated **code mutation fuzzing engine** (generating thousands of AST-mutated test PRs to stress-test Proctor's detection lattice). For the v0.2 release, the checked-in golden corpus (`corpus/` with clean + attack fixtures) and our 43 unit/integration tests are completely sufficient.

1. **AST Cheat Synthesizer:**
   - An internal tool (`proctor fuzzer`) that takes a clean repository and automatically synthesizes thousands of realistic cheat variants:
     - Swapping assertion targets.
     - Injecting subtle shadow mocks into test bodies.
     - Introducing subtle CI workflow exit-code bypasses.
     - Modifying expected literals to mirror production values.
2. **Regression & Recall Matrix:**
   - Benchmark Proctor’s rules against the synthesized variants in `cargo test` to mathematically verify recall and identify blind spots before agents discover them.

---

## 🔵 Phase 7: Packaging, Release & Ecosystem Distribution

1. **GitHub Releases & CI Assets:**
   - Automated CI build matrix compiling pre-built release binaries for:
     - `x86_64-unknown-linux-gnu` / `aarch64-unknown-linux-gnu`
     - `x86_64-apple-darwin` / `aarch64-apple-darwin`
     - `x86_64-pc-windows-msvc`
2. **GitHub Marketplace Publication:**
   - Official release of `no-hup/agent-proctor@v1` on GitHub Marketplace.
3. **Package Distribution:**
   - Publish `cargo-proctor` to crates.io (`cargo install agent-proctor`).
   - Publish Homebrew formula (`brew install no-hup/tap/proctor`).
4. **Agent Integration Presets:**
   - Pre-configured `.pre-commit-config.yaml` hook definition.
   - Claude Code stop-hook recipe (`.claude/settings.json`).
   - Cursor Rules / Agent definition (`.cursorrules`).

---

## 🤖 Picking Up a Task (For Human & Agent Contributors)

This repository is **agent-ready** and open for contributions. Every task is structured with clear objectives, allowed crate boundaries, and deterministic acceptance criteria.

See [AGENTS.md](AGENTS.md) for the complete developer contract, command reference, and coding invariants.

### Active "Good First Issues":
1. **[Issue #1](https://github.com/no-hup/agent-proctor/issues/1): Add `cargo-binstall` metadata to Cargo.toml**
   - *Target:* `Cargo.toml`
   - *Goal:* Allow fast precompiled binary installation via `cargo binstall agent-proctor` without compiling from source.
2. **[Issue #2](https://github.com/no-hup/agent-proctor/issues/2): Support `node:test` / Mocha runner in JavaScript/TypeScript AST parser**
   - *Target:* `crates/proctor-parse/src/javascript.rs`
   - *Goal:* Extract assertion patterns and test blocks for Node's built-in `node:test` (`test()`, `assert.strictEqual`) and Mocha (`it()`, `describe()`).
3. **[Issue #3](https://github.com/no-hup/agent-proctor/issues/3): Add Go (`testing.T`) Tree-sitter assertion parser**
   - *Target:* `crates/proctor-parse/src/go.rs`, `crates/proctor-diff/src/lib.rs`
   - *Goal:* Parse table-driven test structs (`tests := []struct{...}`) and `testing.T` assertions (`assert.Equal`, `t.Fatalf`).
4. **[Issue #4](https://github.com/no-hup/agent-proctor/issues/4): Add Rust (`#[test]`, `assert_eq!`) Tree-sitter assertion lattice**
   - *Target:* `crates/proctor-parse/src/rust.rs`, `crates/proctor-diff/src/lib.rs`
   - *Goal:* Extract `#[test]` and `#[ignore]` attributes, map `assert_eq!` (Exact) vs `assert!` (Weak).
5. **[Issue #5](https://github.com/no-hup/agent-proctor/issues/5): Add Java (JUnit 5, AssertJ) Tree-sitter assertion lattice**
   - *Target:* `crates/proctor-parse/src/java.rs`, `crates/proctor-diff/src/lib.rs`
   - *Goal:* Parse `@Test` and `@Disabled` annotations, JUnit `assertEquals`, and AssertJ `assertThat`.
6. **[Issue #6](https://github.com/no-hup/agent-proctor/issues/6): Add Ruby (RSpec, Minitest) Tree-sitter assertion lattice**
   - *Target:* `crates/proctor-parse/src/ruby.rs`, `crates/proctor-diff/src/lib.rs`
   - *Goal:* Parse RSpec `it` / `describe` blocks, `expect(...).to eq(...)`, and `skip` / `pending` markers.

