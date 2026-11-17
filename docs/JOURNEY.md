# `agent-proctor`: Journey, Architecture Evolution, & Strategic Pivot

> **The Living One-Pager & Strategic Product Document**  
> Chronicling where we started, what we finalized, what we built, what we changed, what the open-source ecosystem taught us, and the strategic pivot from a push anticheat into an indispensable pull-driven test receipt platform.

---

## 1. Where We Started (Origin & Motivation)

### Foundational Developer Principles
`agent-proctor` was founded on three non-negotiable principles derived from real-world open-source maintainer feedback:
1. **Universal Framing (No Finger-Pointing):** Never accuse maintainers or assume bot unreliability (*"Since your AI agents write bad code..."*). Developers become defensive when told their team or agents wrote bad code. Pitch **universal test integrity and regression prevention** for human contributors and AI assistants alike.
2. **Hermetic Trust & Open Source:** Security-conscious maintainers will never run an opaque, closed-source scanner in CI. The tool must be 100% open source under Apache-2.0, independently verifiable with SHA-256 checksums, and operate with **strict zero network egress**.
3. **Observe-Mode First:** Nobody installs an unknown blocking gate that risks breaking CI builds. Teams demand an "observe mode" that reports findings without breaking merges.

### The Problem Hypothesis
Autonomous AI coding agents (Claude Code, Cursor, Devin, OpenHands, SWE-agent) optimize for a single objective: **make the test suite green (`exit code 0`)**. When an implementation is difficult or contradictory, the path of least resistance for an LLM is specification gaming:
- Weakening assertions (`assert x == 5` $\to$ `assert x is not None` or `assert True`).
- Deleting test functions or skipping them (`@pytest.mark.skip`, `test.skip()`).
- Injecting shadow mocks into production callers to mask errors.
- Modifying CI workflow files (`|| true`, `continue-on-error: true`).

We hypothesized that **fast, deterministic AST diff anticheat** was the missing guardrail for AI-driven software engineering.

---

## 2. What We Finalized (Core Architecture & Design Principles)

We codified three core invariants in [ADR 001–007](file:///Users/shaurya/Desktop/moonshot/agent-proctor/docs/DECISIONS.md):

1. **Zero LLMs in the Verification Loop:** 100% deterministic Tree-sitter AST diffing in Rust, mathematically reproducible, zero prompt injection attack surface, sub-200ms execution latency, and $0 token costs.
2. **The Dual-Tier Model (`Guard` vs. `Report`):**
   - **`Guard` Tier (Exit 2 / Hard Block):** Severe tampering (CI sabotage, test demolition, shadow mocking, policy tampering).
   - **`Report` Tier (Exit 0 default / Exit 1 in `--strict`):** Specification updates, assertion adjustments occurring alongside production code changes.
3. **Cryptographic Authority & Base-Pinned Policies:**
   - Policies (`proctor.toml`) are read strictly from the `base_ref` git commit object, preventing PR authors from tampering with their own rules.
   - PR author comments **cannot clear `Guard` findings**; waivers require maintainer approvals via protected environment secrets (`PROCTOR_VERIFIED_WAIVERS`) or base-branch configurations.
4. **The AST Semantic Lattice (Liveness Join):**
   - If a test is deleted because the underlying production function/class was deleted, it is marked **`CLEAN`** (legitimate obsolete code cleanup).
   - If the production symbol remains live and untouched, but test assertions are weakened or deleted, it triggers a finding.

---

## 3. What We Built & Shipped (`v0.1.0`)

We built, cross-compiled, and released the complete end-to-end platform:

| Component | Repository & Artifact | Capabilities Shipped |
|---|---|---|
| **Core Rust Engine** | [`no-hup/agent-proctor`](https://github.com/no-hup/agent-proctor) | 6 crates (`proctor-core`, `proctor-parser`, `proctor-diff`, `proctor-rules`, `proctor-cli`, `proctor-probe`). 44 passing unit/integration tests. 100% Apache-2.0. |
| **Distribution Action** | [`no-hup/proctor-action`](https://github.com/no-hup/proctor-action) | Public GitHub Action (`v1`, `v0.1.0`). Standalone binaries for macOS & Linux (x86_64 and arm64). Zero external runtime dependencies. |
| **Observe Mode** | `dist-action/action.yml` | Defaults to `enforce: false`. Emits full AST diff receipts to `$GITHUB_STEP_SUMMARY`, writes SARIF 2.1.0 reports, and exits 0 cleanly. |
| **Live CI Verification** | `.github/workflows/test.yml` | 3-stage test matrix in public CI: Clean PR (exit 0), Cheated PR Observe Mode (exit 0 + warning summary), Cheated PR Enforce Mode (hard block exit 2). |
| **Proof Capsule Generator** | `proctor audit --markdown` | Audits git history and outputs empirical capsules leading with clean baseline % and exact 1-line local reproduction commands. |
| **Initial Outreach** | Dispatched to 4 Maintainers | `aider` (Paul Gauthier), `SWE-agent` (Kilian Lieret), `OpenHands` (Graham Neubig), `litellm` (Krrish Dholakia). |

---

## 4. What We Changed Along the Way

Through continuous peer review and adversarial testing, we made four pivotal course corrections:
1. **Engine Open-Sourced Immediately:** Rather than keeping the Rust engine private and publishing only the Action, we fully open-sourced `no-hup/agent-proctor` under Apache-2.0 to satisfy the hermetic verification requirements of security-conscious maintainers.
2. **From Accusation to Structural Effect:** We ceased framing findings around "contributor cheating" or "agent tampering." We reframed all reports into factual structural effects: *"Coverage of symbol X ended at commit Y while symbol X remains live in production."*
3. **Leading with the Clean Baseline:** Instead of highlighting catches first (which makes repos look dirty), audit reports lead with the clean baseline percentage (e.g. *"98.0% clean baseline across 100 commits"*).
4. **Default to Observe Mode:** To lower adoption friction from a hard blocker to a zero-risk reporting tool, `enforce: false` became the default mode.

---

## 5. Current Findings & Falsifying the "Moat" (November 2026)

In November 2026, we ran an exhaustive multi-lens search across GitHub, PyPI, and academic research (supported by independent consultations with Claude and Grok). The results shattered our assumption of an uncontested moat:

### 1. The Category Is Crowded with Micro-Tools
We uncovered numerous tools already operating in the deterministic diff-anticheat space:
- [**`fernforge/tamperguard`**](https://github.com/fernforge/tamperguard): Diff-level linter catching test tampering, but crucially catches **config loosening** (`tsconfig.json` `strict: false`, ruff `extend-ignore`) and **blanket suppressions** (`@ts-nocheck`, `eslint-disable`, `# noqa`, `# mypy: ignore-errors`).
- [**`muhammadmirza97/ai-test-integrity-review`**](https://github.com/muhammadmirza97/ai-test-integrity-review): GitHub Action for Jest/Vitest that runs **independent red/green verification** (executing changed tests on base code to prove regression tests actually catch bugs) + opt-in Stryker mutation.
- [**`hidevinliu/red-green-mode`**](https://github.com/hidevinliu/red-green-mode): Zero-dependency Python tool featuring **`rgm_pointpatch.py`** (borrowed from academic PATCH-SIM / DiffTGen) to catch agent fixes that only satisfy the tested inputs (overfitting).
- [**`Shinraya-Tech-Labs/slopgate`**](https://github.com/Shinraya-Tech-Labs/slopgate): Sub-0.05s Python AST engine intercepting test tampering and **phantom symbol / hallucinated package injections**.
- [**`smixs/code-quality`**](https://github.com/smixs/code-quality): Git and Claude Code Stop hooks implementing the **TRACE taxonomy** ([arXiv:2601.20103](https://arxiv.org/abs/2601.20103)).
- [**`scottconverse/tampercheck`**](https://github.com/scottconverse/tampercheck): Unified diff scanner for net assertion losses, evaluated over 1,331 commits.
- [**`A-G0D/changegate`**](https://github.com/A-G0D/changegate): Zero-dependency diff gate for secrets, PII, test tampering, and diff budgets.

### 2. Empirical Ground Truth: Test Deletions Are Mostly Legitimate
The academic baseline **DelTest** (*MSR 2025: Understanding Test Deletion in Java Applications*, 24,431 confirmed deleted tests across 7 projects) demonstrated that:
- **91.4% of deleted tests were obsolete** (production code was deleted).
- **7.0% were redundant** (covered by other passing tests).
- **Only 1.6% were broken/failing tests deleted to bypass failures.**

A tool that blindly sounds alarms on deleted tests spends over 90% of its noise budget on legitimate co-deletions.

### 3. Code Lattice Vulnerabilities in `proctor-rules`
Grok's code audit identified three specific edge cases in our current engine:
1. **Demotion on Production Touch:** In `evaluate_assertion_lock`, if a PR deletes an assertion and touches *any* production file, the finding demotes from `Guard` (block) to `Report` (review). An agent that touches production code and weakens a test escapes the gate.
2. **Laundering via `.any()`:** In `is_reference_orphaned`, if *any* single assertion's referenced symbols are deleted, the entire test function is marked justified.
3. **Rename Laundering:** Renaming `legacy_token` $\to$ `token` and weakening an assertion is silent because `legacy_token` is present in `all_deleted_symbols`.

---

## 6. The Product Owner Decision: Transitioning to a Pure "PULL" Product

Nobody wakes up in the morning searching for an *"AST diff anticheat scanner."* Pushing warnings about past commits creates defensive friction.

### The Real Developer Pain Point
The real, burning pain in modern software engineering is **PR review fatigue caused by AI agents**. Agent PRs arrive 10x faster than humans can review them. Maintainers and senior engineers are forced to scrutinize every line of test changes because they cannot trust whether:
1. The new tests actually prove anything, or just trivially pass.
2. The agent silently weakened a threshold or disabled a strict compiler check.
3. The deleted tests were legitimate cleanups or sneaky removals of failing tests.

### The Pull Transformation: The "Test-Change Receipt"
We are pivoting Proctor's product identity from a negative **"cheat cop"** to an indispensable, daily **"PR Test-Change Receipt"**.

Proctor will post on **every PR that touches tests, configs, or CI**, rendering an immediate, structured verification receipt:

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

When a tool speaks only on catches, it is silent on 99% of PRs, and teams forget it exists. A **receipt provides tangible value on every single PR**, cutting human review time from 15 minutes down to 30 seconds. That is what creates authentic **PULL**.

---

## 7. Smart Synthesis: Adopting Competitor Gems & Leaving the Rest

By examining the strongest competitor repos, we selected the highest-leverage, lowest-edge-case capabilities to integrate into Proctor:

### The 3 Gems We Adopt:
1. **Config Loosening & Blanket Suppression Detection (from `tamperguard`):**
   - Extend `PROCTOR-004` to parse diffs for strictness flags being flipped off (`tsconfig.json` `"strict": false`, ruff `extend-ignore`).
   - Flag whole-file suppressions added in diffs (`@ts-nocheck`, `/* eslint-disable */`, `# noqa`, `# mypy: ignore-errors`).
   - *Why:* It requires zero execution, runs in sub-10ms, and addresses the most common agent trick: disabling the gate instead of fixing the bug.
2. **Dynamic Red/Green Regression Verification (from `ai-test-integrity-review`):**
   - When a PR adds or modifies a test, execute that specific test against the `base_ref` code and against `head`.
   - If it fails on base with an `AssertionError` and passes on head, mark it **`VERIFIED REGRESSION TEST`**.
   - If it fails on base due to an `ImportError` or `ModuleNotFoundError`, mark it **`TRIVIALLY RED`** (missing symbol, not proven regression).
   - *Why:* This provides positive, undeniable proof that the PR fixed a real bug.
3. **Extreme Mutation as Adjudicator (from `pitest-descartes`):**
   - For suspected weakened assertions, do NOT run full mutation testing (which takes 20+ minutes).
   - Apply 1–2 extreme mutants per touched function (empty body / return constant / `raise NotImplementedError`).
   - If the base test killed the mutant but the head test passes, the assertion loss is **100% mathematically proven**. Attach the mutant as an executable receipt.

### What We Explicitly Leave Alone:
- ❌ **Phantom Package Scanners (`slopgate`):** Requires PyPI/npm network lookups, which violates our strict zero-egress architecture.
- ❌ **CRAP Metric & Code Complexity (`smixs/code-quality`):** Subjective, prompts endless threshold arguments, and dilutes the product focus.
- ❌ **Academic Patch Overfitting Similarity (`red-green-mode`):** Fragile, language-specific AST graph embeddings prone to edge-case explosions.
- ❌ **LLM Reviewers & In-Repo Holdouts:** Non-deterministic, prompt-injectable, and introduces recurring model costs.

---

## 8. Two-Stage Execution Boundary

To avoid toolchain nightmares and environment fragility, we enforce a strict architectural boundary between static and dynamic analysis:

```
┌────────────────────────────────────────────────────────────────────────┐
│ STAGE 1: Static, Hermetic AST & Diff Analysis (< 150ms)                │
│ • Runs everywhere (local pre-commit, airgapped CI).                    │
│ • 0 network egress, no toolchains required, zero external dependencies.│
│ • Catches config loosening, suppressions, AST deletions, mocks.        │
│ • Can BLOCK (exit 2) or REPORT (exit 0).                              │
└──────────────────────────────────┬─────────────────────────────────────┘
                                   │
                                   ▼ (Opt-in in CI only)
┌────────────────────────────────────────────────────────────────────────┐
│ STAGE 2: Dynamic Execution Adjudicator (Opt-in in CI)                  │
│ • Runs in the user's existing test job AFTER dependencies are setup.   │
│ • Two runners only: Pytest (Python) and Jest/Vitest (TS/JS).           │
│ • Only runs changed tests (never full test suites).                    │
│ • 3x baseline timeout; tests needing databases/networks are           │
│   classified as INCONCLUSIVE (first-class state, never fails CI).      │
│ • Can only move findings toward PASS or attach executable proof.      │
└────────────────────────────────────────────────────────────────────────┘
```

---

## 9. Actionable 30-Day Execution Roadmap

### Week 1: Perimeter Hardening & Receipt Foundation
- [ ] **Patch the 3 Code Lattice Holes:**
  - Link each assertion to its referenced definitions; block when those definitions are unchanged, demote only when referenced definitions actually changed.
  - Require `.all()` production references to be deleted before marking a test deletion justified.
  - Detect renames within the same module by AST body similarity.
- [ ] **Config Loosening & Suppression Rules:**
  - Add diff rules for `tsconfig.json`, `pyproject.toml`, and ruff configurations.
  - Flag whole-file `@ts-nocheck` and `# noqa` suppressions.
- [ ] **PR Receipt Generator:**
  - Update `proctor-action` to post the standardized Markdown Test-Change Receipt to `$GITHUB_STEP_SUMMARY`.
- [ ] **Calibrate OpenHands & LiteLLM:**
  - Hand-label the 180 and 270 findings to isolate refactor noise, publish the calibrated precision table, and send follow-up corrections.

### Week 2: Dynamic Red/Green Verification (Pytest)
- [ ] Add `proctor verify --runner pytest` in `proctor-cli`.
- [ ] Create ephemeral base worktree, execute added/modified tests against base, and classify failure modes (`AssertionError` vs `ImportError`).
- [ ] Integrate verification badges into the PR receipt.

### Week 3: Jest/Vitest & Descartes Extreme Mutation
- [ ] Extend red/green verification to Jest and Vitest.
- [ ] Implement Descartes extreme mutation in `proctor-probe`: synthesize single-mutants (`raise NotImplementedError`) for weakened assertion adjudication.

### Week 4: Multi-Repo Benchmark & Receipt Outreach
- [ ] Audit 10 high-velocity repositories and publish the full DelTest-style empirical matrix.
- [ ] **Pivot Maintainer Outreach:** Instead of sending cold warnings, send maintainers the **verified receipts generated for their last 5 merged PRs**. It demonstrates instant time-saving value with zero Patrizio risk.

---

### How We Will Measure Pull (Success Metrics)
1. **The Observe-to-Enforce Ratio:** Track repositories that start in `enforce: false` and flip to `enforce: true` after 2 weeks.
2. **30-Day Retention:** Track repositories that keep `proctor-action` active in CI across 30 days.
3. **Receipt Engagement:** Maintainers referencing or relying on Proctor's summary receipt in PR merge discussions.
