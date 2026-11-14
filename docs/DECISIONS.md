# Architecture Decision Records (ADRs) & Engineering Log

This document tracks all major architectural decisions, trade-offs, and design rationale for **`agent-proctor`**.

---

## ADR 001: Project Identity & Core Invariants
* **Date:** 2026-11-14
* **Status:** Accepted
* **Context:** Autonomous coding agents (Devin, Claude Code, Cursor Agent, OpenHands) optimize for passing tests (`exit code 0`). When bugs are difficult, agents game specifications: weakening assertions, mocking targets, and modifying CI workflows. We need a fast, deterministic anticheat gate.
* **Decision:** Named **`agent-proctor`** (CLI binary: `proctor`). 
* **Core Invariants:**
  1. **Zero LLMs in the Verification Loop:** 100% deterministic AST diffing, mathematically reproducible, zero prompt injection surface, and $0 model costs.
  2. **Sub-200ms Static Latency:** Operates inside local agent tool loops and as a fast GitHub Action check before human review.
  3. **Zero Host Friction:** Standalone static binary with embedded Tree-sitter parsers.

---

## ADR 002: Three-Way Verdict Triage (`PASS`, `REVIEW`, `BLOCK`)
* **Date:** 2026-11-14
* **Status:** Accepted
* **Context:** A naive binary `pass/fail` gate forces the tool to solve an undecidable problem (statically proving whether a test edit is a genuine specification change vs a cheat). This causes false positive death spirals.
* **Decision:** Implement a three-way verdict model:
  - `PASS` (exit 0): Clean diff, or test change justified by an orphaned symbol reference.
  - `REVIEW` (exit 1): Needs approval from an authenticated human reviewer who is not the author.
  - `BLOCK` (exit 2): Unjustifiable tampering (e.g. CI sabotage `|| true`, step pruning, or shadow mocking of the target).
* **Why:** Eliminates the false-positive trap. The tool acts as an automated triage supervisor and evidence router rather than an omniscient oracle.

---

## ADR 003: Bilateral Justification via "Orphaned References"
* **Date:** 2026-11-14
* **Status:** Accepted
* **Context:** Our initial naive bilateral rule downgraded blocks whenever the production symbol was touched. Adversarial consults proved this is a fatal bypass: every bugfix (and every cheat) touches production code.
* **Decision:** Assertion removals and downgrades are justified **if and only if** they reference an identifier, attribute, or enum member that was defined in the base production code and has **no definition anywhere in head production code** (or if the callee itself was deleted).
* **Why:** Trivial production edits (renaming locals, adding kwargs, reordering lines) delete nothing and cannot unlock the test file. An agent can only unlock the test by deleting a real production symbol, which is an obvious, conspicuous change in code review.

---

## ADR 004: Anti-Drift: The Mirror Check & Edit-Order Forensics
* **Date:** 2026-11-14
* **Status:** Accepted
* **Context:** The most common real-world weakening is changing expected literals (e.g. `assert total == 120` $\to$ `assert total == 118`) because the buggy function returns 118. Both are exact equality assertions, so count lattices are blind.
* **Decision:** Layered anti-drift detection:
  1. **The Mirror Check (`BLOCK`):** Flag when a new expected literal in tests matches a newly added literal/return value in the touched production function (hardcoded bypass).
  2. **Sibling Consistency (`REVIEW`):** Flag when 1 test's literal drifts while sibling tests exercising the same callee remain unchanged.
  3. **Edit-Order Forensics:** The local agent hook records tool-call sequences. If a test literal edit immediately follows a failing test run with zero production edits in between, attach this forensic evidence to the report.

---

## ADR 005: Cryptographic GitHub Review-Based Waivers
* **Date:** 2026-11-14
* **Status:** Accepted
* **Context:** Text comments in git diffs (`# proctor: allow`) can be hallucinated or written by the agent itself.
* **Decision:** In CI, waivers are valid only if an authenticated PR review comment contains `proctor: accept <finding_id>` submitted by a user who is NOT the PR author, NOT a bot account, and possesses write permissions on the repository.
* **Why:** Leverages GitHub's cryptographic authentication infrastructure. The agent cannot forge a human maintainer's review approval without stealing their personal session credentials.

---

## ADR 006: Dynamic Probe Isolation via Ephemeral `git worktree`
* **Date:** 2026-11-14
* **Status:** Accepted
* **Context:** Mutating files in place in the working tree corrupts live developer environments (Jest watch mode, IDE indexers, parallel agent processes).
* **Decision:** `proctor probe` runs in an ephemeral temporary worktree (`git worktree add /tmp/... HEAD`), symlinking `.venv` and `node_modules`. 
* **Safety Invariant:** `probe` can **only move findings toward `PASS`**. A mutant survival never triggers a `BLOCK`, making equivalent mutants completely harmless.

---

## ADR 007: Hierarchical `TestId` & Two-Pass Similarity Pairing
* **Date:** 2026-11-14
* **Status:** Accepted
* **Context:** Naive string test names (`file::test_name`) treat renames and moves as "1 test deleted + 1 new test created", causing simultaneous false alarms and false negatives.
* **Decision:** `TestId` tracks file path, scope hierarchy (`describe` chains / test classes), test name, parameter hashes, and alpha-normalized body hashes. Pairing uses two passes: exact identity, followed by similarity matching ($\ge 80\%$ assertion overlap and shared imports) to absorb legitimate renames and moves.
