# `agent-proctor` v0.2.0 Execution Plan: The "Pull" Transformation & Comparative Evaluation

> **Status:** Proposed Plan  
> **Target Release:** `v0.2.0`  
> **Core Objective:** Transform `agent-proctor` from a commoditized, push-oriented AST anticheat into an indispensable, pull-driven **PR Test-Change Receipt platform**, backed by executable mathematical proof, strict zero-egress, and empirical comparative validation against the v0.1.0 baseline.

---

## Architecture & Strategic Vision

```
┌────────────────────────────────────────────────────────────────────────┐
│                   THE THREE-TIER INTEGRITY STACK                       │
├────────────────────────────────────────────────────────────────────────┤
│ Tier 3: Extreme Mutation Adjudication (Descartes Method)               │
│         Apply 1-2 line mutants (empty body/constant return) to code.  │
│         If base test killed mutant and head test passes -> Hard Proof. │
├────────────────────────────────────────────────────────────────────────┤
│ Tier 2: Dynamic Red/Green Regression Verification                      │
│         Run added/modified tests against base code:                   │
│         AssertionError = Verified Regression Test; ImportError = Trivial│
├────────────────────────────────────────────────────────────────────────┤
│ Tier 1: Static Hermetic AST Diff & Perimeter Defense (< 150ms)         │
│         • Fixed definition-level linkage (no accidental demotions)     │
│         • .all() orphan justification & intra-module rename detection  │
│         • Config loosening (tsconfig strict:false, ruff extend-ignore) │
│         • Blanket suppressions (@ts-nocheck, /* eslint-disable */)     │
│         • Standardized PR Test-Change Receipt in $GITHUB_STEP_SUMMARY  │
└────────────────────────────────────────────────────────────────────────┘
```

---

## Phase 1: Perimeter Hardening & Fixing the 3 Code Lattice Holes

*Goal: Close the implementation loopholes discovered by Grok in `crates/proctor-rules` to eliminate false negatives and silent laundering.*

### Tasks:
- [x] **Task 1.1: Fix Assertion-Definition Linkage**
  - *Location:* `crates/proctor-rules/src/lib.rs` (`evaluate_assertion_lock`)
  - *Problem:* Currently, if an assertion is deleted and the PR touches *any* production file anywhere in the diff, the finding is demoted from `Guard` (block) to `Report` (review). An agent that touches production code and weakens a test escapes blocking.
  - *Fix:* Link each assertion to the production files that actually define the symbols it references. Block when those referenced definitions are unchanged. Demote to `Report` only when a referenced definition changed in the same PR. Unknown linkage routes to `Report`, never silent clean.
- [x] **Task 1.2: Enforce `.all()` Production Reference Orphan Justification**
  - *Location:* `crates/proctor-rules/src/lib.rs` (`is_reference_orphaned`)
  - *Problem:* Currently, whole-test deletion is marked clean if *any* single assertion's referenced symbols were deleted (`justified = del_test.assertions.iter().any(...)`), laundering other weakened assertions in the same test.
  - *Fix:* Require every production-referencing assertion in the test to be orphaned. A test with zero production references is never automatically justified.
- [x] **Task 1.3: Intra-Module Symbol Rename Detection**
  - *Location:* `crates/proctor-diff/src/lib.rs` & `crates/proctor-rules/src/lib.rs`
  - *Problem:* Renaming `legacy_fn` $\to$ `fn` counts as a deleted symbol, silently laundering assertion weakenings.
  - *Fix:* Match deleted symbols to newly added symbols in the same module by AST body similarity. If a body match exists ($\ge 75\%$), treat as a rename: the symbol is not orphaned, and assertion weakenings remain flagged.
- [x] **Task 1.4: Corpus Fixtures & Regression Tests**
  - Create attack fixtures and clean twins for all 3 edge cases under `corpus/attack/` and `corpus/clean/`. Verify via `cargo test`.

---

## Phase 2: Config Loosening & Blanket Suppression Detection (Adopting `tamperguard` Gem)

*Goal: Catch the most common real-world agent bypass: disabling the gate or compiler instead of fixing the failing code.*

### Tasks:
- [x] **Task 2.1: Config Loosening Rules (`PROCTOR-004` Extension)**
  - Parse diffs in configuration files:
    - `tsconfig.json`: flag strictness flags flipped from `true` to `false` (`strict`, `noImplicitAny`, `strictNullChecks`).
    - `pyproject.toml` / `ruff.toml` / `.flake8`: flag additions to `extend-ignore`, `ignore`, or `suppress-none`.
    - `.eslintrc` / `eslint.config.js`: flag rules dropped from `"error"` to `"off"` or `"warn"`.
  - Severity: **`Guard` Tier (Exit 2 / Hard Block)**. Tightening configs is marked clean.
- [x] **Task 2.2: Blanket Suppression Rules**
  - Detect added file-level checker suppressions in diffs:
    - TypeScript/JavaScript: `// @ts-nocheck`, `/* eslint-disable */` (without specific rule names).
    - Python: `# ruff: noqa`, `# mypy: ignore-errors`, blanket `# type: ignore` at module level.
    - Rust: `#![allow(...)]` crate-level attributes.
  - Severity: **`Report` Tier (Review Required)**. Targeted, line-level suppressions (`# noqa: E501`, `@ts-expect-error`) remain clean.
- [x] **Task 2.3: Config & Suppression Test Corpus**
  - Add test fixtures covering config tightening (clean), config loosening (block), targeted suppression (clean), and blanket suppression (review).

---

## Phase 3: The PR Test-Change Receipt UI (The "Pull" Pivot)

*Goal: Transform Proctor from a negative "cheat alarm" into an indispensable daily review accelerator that posts on every PR touching tests or configs.*

### Tasks:
- [x] **Task 3.1: Structured Markdown Receipt Generator in `proctor-cli`**
  - Add `proctor receipt --format markdown` that outputs a standardized, positive Test-Change Receipt:
    ```markdown
    ### 📋 Test-Change Receipt (agent-proctor)
    | Category | Count | Status & Verification |
    |---|---|---|
    | **Added Tests** | 3 | ✅ 2 verified: failed on base, passed on head<br/>⚠️ 1 trivially red (import error on base) |
    | **Modified Tests** | 2 | ✅ 1 strengthened, 1 neutral refactor |
    | **Deleted Tests** | 1 | ✅ Obsolete: callee `calculate_v1()` was deleted in this PR |
    | **Configs & Gates** | 0 | ✅ `tsconfig.json`, `ruff.toml`, and CI workflows unchanged |
    | **Suppressions** | 0 | ✅ Zero blanket suppressions added |

    **Verdict:** 🟢 **Zero review needed in test diff.** Merge with confidence.
    ```
- [x] **Task 3.2: Sync Receipt Rendering to `proctor-action`**
  - Update `proctor-action/action.yml` to write the full receipt to `$GITHUB_STEP_SUMMARY`.
  - Provide an option for a sticky PR comment (`comment: true`, default `false`).

---

## Phase 4: Comparative Re-Audit & Empirical Evaluation (v0.1.0 vs. v0.2.0)

*Goal: Systematically audit the exact same 5 open-source repositories to measure how v0.2.0 compares against v0.1.0 in precision, false-positive reduction, noise filtering, and catch quality.*

```
   ┌──────────────────────────────────────────────────────────────┐
   │             COMPARATIVE AUDIT RE-RUN PIPELINE                │
   ├──────────────────────────────────────────────────────────────┤
   │ 1. Checkout identical 100-commit window for each repo        │
   │ 2. Run v0.1.0 binary -> record findings_v1.json             │
   │ 3. Run v0.2.0 binary -> record findings_v2.json             │
   │ 4. Compute delta matrix:                                     │
   │    • Noise Reduction: % of refactors no longer mislabeled    │
   │    • Catch Retention: Are true positive catches preserved?   │
   │    • New Catches: Did config/suppression rules flag anything?│
   │    • Clean Baseline Shift: Did 55% & 64% baselines normalize?│
   └──────────────────────────────────────────────────────────────┘
```

### Tasks:
- [ ] **Task 4.1: Automated Audit Runner Script**
  - Create `/scripts/compare_audits.sh` that clones/pulls each target repo and runs both binaries over the exact same commit slices:
    1. `princeton-nlp/SWE-agent` (100 commits)
    2. `aider-AI/aider` (100 commits)
    3. `OpenHands/OpenHands` (99 commits)
    4. `BerriAI/litellm` (99 commits)
    5. `browser-use/browser-use` (100 commits)
- [ ] **Task 4.2: In-Depth Delta Analysis**
  - **OpenHands Analysis:**
    - v0.1.0 baseline: 64.6% clean, 180 test modifications, 13 mocks.
    - Measure: How many of the 180 flags were eliminated by definition-linkage and rename checks?
    - How many were genuine assertion weakenings vs. provider interface refactors?
  - **LiteLLM Analysis:**
    - v0.1.0 baseline: 55.6% clean, 270 test modifications, 12 mocks.
    - Measure: Did definition-linkage normalize the baseline above 85%?
  - **Catch Retention Check:**
    - Verify that `browser-use` (330 test lines dropped before `1.0.3rc1`) is still caught with 100% precision.
    - Verify that `aider` (model alias mutation) is still caught.
- [ ] **Task 4.3: Publish Comparative Evaluation Document**
  - Write [`docs/audit/V0_1_VS_V0_2_COMPARISON.md`](file:///Users/shaurya/Desktop/moonshot/agent-proctor/docs/audit/V0_1_VS_V0_2_COMPARISON.md) documenting:
    - Summary comparison table (v0.1.0 vs v0.2.0 clean baselines, noise drops, new catches).
    - Qualitative assessment: Why v0.2.0 is superior for human maintainers.
    - DelTest classification: Percentage of deletions that were obsolete vs. redundant vs. genuine cheats.

---

## Phase 5: Dynamic Red/Green Regression Verification (Pytest & Jest - Opt-in CI)

*Goal: Adopt the gem from `ai-test-integrity-review` to provide undeniable positive proof that newly added tests actually catch bugs.*

### Tasks:
- [x] **Task 5.1: Ephemeral Base Worktree Test Runner (`proctor verify`)**
  - Add `proctor verify --runner pytest` in `proctor-cli`.
  - Create an ephemeral temporary worktree checkout of `base_ref`.
  - Overlay newly added or modified test files onto the base worktree.
  - Run the user's test command (`pytest <changed_tests>`) against base code.
- [x] **Task 5.2: Failure Mode Classification**
  - If test **fails with `AssertionError`** on base and **passes** on head:
    $\to$ Mark **`VERIFIED REGRESSION TEST`** (proven to prevent regressions).
  - If test **fails with `ImportError` or `ModuleNotFoundError`** on base:
    $\to$ Mark **`TRIVIALLY RED`** (symbol didn't exist, not a proven regression).
  - If test **passes on both base and head**:
    $\to$ Mark **`TAUTOLOGY / NON-DISCRIMINATING`** (test does not exercise the change).
- [x] **Task 5.3: Jest / Vitest Support**
  - Extend worktree runner to `jest` and `vitest` using npm/pnpm test invocations.
- [x] **Task 5.4: First-Class Inconclusive State**
  - If tests require databases, networks, or take >3x baseline timeout, report `INCONCLUSIVE` with the specific reason; never fail the CI build.

---

## Phase 6: Descartes Extreme Mutation Adjudication (Executable Proof Receipts)

*Goal: Adopt the extreme mutation method (Vera-Pérez et al., ASE 2018) to replace subjective warnings with executable proof receipts.*

### Tasks:
- [ ] **Task 6.1: Single-Mutant Synthesis in `proctor-probe`**
  - For suspected weakened assertions, do NOT run full mutation testing.
  - Synthesize 1–2 extreme mutants per touched function:
    - Python: Replace body with `raise NotImplementedError("proctor_mutant")` or `return None`.
    - TypeScript: Replace body with `throw new Error("proctor_mutant")` or `return undefined`.
- [ ] **Task 6.2: Mutant Execution & Adjudication**
  - Run the base test and head test against the mutant in an ephemeral worktree:
    - If base test **failed** (killed mutant) and head test **passes** (mutant survived):
      $\to$ Promote finding to confirmed **`Guard`** tier. Attach the exact mutant code to the PR receipt as executable proof.
    - If both behave identically:
      $\to$ Demote finding to informational `Report`.

---

## Phase 7: Release `v0.2.0` & Maintainer Outreach Relaunch

*Goal: Release the calibrated engine and re-engage maintainers with value-first PR receipts.*

### Tasks:
- [ ] **Task 7.1: Tag & Publish `v0.2.0`**
  - Bump Cargo versions across all 6 crates to `0.2.0`.
  - Build release binaries for `darwin-aarch64`, `darwin-x86_64`, `linux-aarch64`, `linux-x86_64`.
  - Update `no-hup/proctor-action` tag `v1` and release `v0.2.0`.
- [ ] **Task 7.2: Calibrated Follow-Ups to Graham (OpenHands) and Krrish (LiteLLM)**
  - Send the calibrated follow-up note admitting previous noise, explaining the new definition-linkage engine, and presenting the updated, accurate precision numbers.
- [ ] **Task 7.3: Relaunch Batch 2 Outreach (Flattering PR Receipts)**
  - For `browser-use`, `claude-flow`, `cline`, and other targets:
  - Do NOT send an accusatory cold email.
  - Send: *"We ran Proctor over your last 5 merged PRs and generated these verified Test-Change Receipts showing that your recent PRs added 100% verified regression tests. Here is how to keep this receipt automatic in CI."*

---

## Evaluation & Success Criteria

| Evaluation Dimension | Target Metric | Verification Method |
|---|---|---|
| **Noise Reduction** | $\ge 70\%$ reduction in false flags on `OpenHands` & `litellm` | Automated comparison via `compare_audits.sh` |
| **Catch Retention** | 100% retention of known true positives (`browser-use`, `aider`) | Verified against golden corpus and git commit SHAs |
| **New Catch Sensitivity** | Detects 100% of synthetic config loosenings & blanket suppressions | Checked against `corpus/attack/` test suite |
| **Receipt Generation** | Under 150ms static latency for PR receipt formatting | Benchmarked with `hyperfine` on Linux runner |
| **Dynamic Execution Safety**| Zero CI crashes; 100% graceful fallback to `INCONCLUSIVE` on DB/network tests | Integration test suite on simulated environments |
