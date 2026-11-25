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
- [x] **Task 4.1: Automated Audit Runner Script**
  - Created `scripts/compare_audits.sh` and `scripts/compare_audits_per_commit.py`, running per-commit JSON `proctor scan` comparisons (`v0.1.0` vs `v0.2.1` vs `v0.2.2`) over identical 100-commit slices for all 5 repos (500 primary commits) plus a disjoint 100-commit `browser-use` holdout (`skip=100`):
    1. `princeton-nlp/SWE-agent` (100 commits)
    2. `aider-AI/aider` (100 commits)
    3. `OpenHands/OpenHands` (100 commits)
    4. `BerriAI/litellm` (100 commits)
    5. `browser-use/browser-use` (100 primary + 100 holdout commits)
- [x] **Task 4.2: In-Depth Delta Analysis & Hand-Labeling**
  - Diffed findings by Blake3 `finding.id` across `v0.1.0`, `v0.2.1`, and `v0.2.2`.
  - Hand-labeled 140 stratified `v0.2.1` findings in `docs/audit/labels_v0_2_1.csv` and 50 disjoint `browser-use` holdout findings in `docs/audit/labels_v0_2_2_holdout.csv`.
  - Fixed `.tsx` grammar (`language_tsx()` + `typeof import(...)` error filter), excluded `monkeypatch.setenv`/`delenv`/`patch.dict(os.environ)` from `ShadowMock`, and made linked `!was_weakened` co-evolution informational while keeping `!was_weakened && !linked` at `Report` (`Review`).
  - Reduced yellow rates on `OpenHands` (37% -> 7.0%) and `litellm` (13% -> 5.0%), lifted surviving labeled `ShadowMock::guard` accuracy to 13/13 (primary) and 7/7 (holdout), and kept zero Guard regressions on `aider` (6/6 block commits, 11/11 findings).
- [x] **Task 4.3: Publish Comparative Evaluation Document**
  - Updated `docs/audit/V0_1_VS_V0_2_COMPARISON.md` with verified 3-way commit tables, holdout validation, finding-ID delta root causes, and two-column per-stratum precision tables.

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

## Phase 7: Release `v0.2.2` & Maintainer Outreach Relaunch

*Goal: Release the calibrated engine and re-engage maintainers with value-first PR receipts.*

### Tasks:
- [x] **Task 7.1: Tag & Publish `v0.2.2`**
  - Bump Cargo versions across all 6 crates to `0.2.2`.
  - Build release binaries for `darwin-aarch64`, `darwin-x86_64`, `linux-aarch64`, `linux-x86_64`.
  - Release `v0.2.2` and sync `no-hup/proctor-action` tag `v1`.
- [ ] **Task 7.2: Calibrated Follow-Ups to Graham (OpenHands) and Krrish (LiteLLM)**
  - Drafted in `docs/outreach/BATCH_2_DRAFTS.md` (awaiting user approval): admits previous `.tsx`/`with` noise, links `V0_1_VS_V0_2_COMPARISON.md`, includes verified clean `v0.2.2` PR receipts.
- [ ] **Task 7.3: Relaunch Batch 2 Outreach (Flattering PR Receipts)**
  - Drafted in `docs/outreach/BATCH_2_DRAFTS.md` (awaiting user approval) using non-blocking Observe Mode (`enforce: false`) + opt-in `proctor verify` framing.

---

## Evaluation & Success Criteria

| Evaluation Dimension | Target Metric | Verification Method |
|---|---|---|
| **Noise Reduction** | $\ge 70\%$ reduction in false flags on `OpenHands` & `litellm` | Automated comparison via `compare_audits.sh` |
| **Catch Retention** | 100% retention of known true positives (`browser-use`, `aider`) | Verified against golden corpus and git commit SHAs |
| **New Catch Sensitivity** | Detects 100% of synthetic config loosenings & blanket suppressions | Checked against `corpus/attack/` test suite |
| **Receipt Generation** | Under 150ms static latency for PR receipt formatting | Benchmarked with `hyperfine` on Linux runner |
| **Dynamic Execution Safety**| Zero CI crashes; 100% graceful fallback to `INCONCLUSIVE` on DB/network tests | Integration test suite on simulated environments |
