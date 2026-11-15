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
[Phase 3: Benchmark Grounding]      ──► ACTIVE / NEXT
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

## 🔵 Phase 6: Synthetic Mutation & Fuzzing Corpus

Rather than generating conversational dialogues, build an in-repo code mutation fuzzing engine:

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
