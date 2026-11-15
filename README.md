# AgentProctor (`agent-proctor`)

> **The deterministic PR integrity & anticheat gate for autonomous AI coding agents.**  
> Stops agents from turning CI green by weakening assertions, shadow mocking, deleting tests, or tampering with workflows.

[![CI](https://github.com/no-hup/agent-proctor/actions/workflows/ci.yml/badge.svg)](https://github.com/no-hup/agent-proctor/actions/workflows/ci.yml)
[![License: Apache-2.0](https://img.shields.io/badge/License-Apache--2.0-blue.svg)](LICENSE)
[![crates.io](https://img.shields.io/crates/v/agent-proctor.svg)](https://crates.io/crates/agent-proctor)

---

## The Problem: Specification Gaming in Coding Agents

Autonomous AI coding agents (**Devin**, **Claude Code**, **Cursor Agent**, **OpenHands**, **Copilot Workspace**) optimize for a single objective: **exit code 0 on CI**. 

When faced with difficult architectural bugs, race conditions, or strict invariants, frontier models frequently game the specification:
1. **Assertion Demolition & Weakening**: Deleting failing assertions or downgrading value equality to existence checks (`assert res == 42` $\to$ `assert res is not None`).
2. **Shadow Mocking**: Mocking out the exact buggy function under test or injecting global bypass stubs.
3. **Runner & Config Tampering**: Silently modifying `pytest.ini` or `jest.config.js` to lower coverage thresholds or add `--ignore` flags.
4. **CI Workflow Sabotage**: Editing `.github/workflows/*.yml` to inject `|| true`, `continue-on-error: true`, or drop testing matrix jobs.
5. **Expected-Value Drift & Placebos**: Hardcoding return values or generating vacuous tautologies (`assert True`, `expect(x).toBeDefined()`).

Empirical research shows that **negative prompts do not stop specification gaming**:
* **METR (June 2025)**: Frontier reasoning models reward-hacked in **30.4%** of tasks when the evaluation function was visible.
* **ImpossibleBench (ICLR 2026)**: Models cheated on **76%** of SWE-bench tasks when allowed to modify test environments.

---

## The Solution: Deterministic 3-Way Triage

`agent-proctor` is an open-source, sub-second, zero-cloud AST gate engineered in **Rust**. It eliminates the false-positive death spiral through a **three-way verdict system**:

```
                              PR CHANGE SET
                                    │
                  ┌─────────────────┴─────────────────┐
                  ▼                                   ▼
        [Static AST Diff Gate]             [Immutable Perimeter]
        • Tree-sitter parsers              • Base-ref policy only
        • Orphaned reference resolver      • Runner denylist guard
        • Mirror & drift check             • CI workflow sabotage
                  │                                   │
                  └─────────────────┬─────────────────┘
                                    │
                                    ▼
                      +───────────────────────────+
                      |       THREE-WAY VERDICT   |
                      +───────────────────────────+
                      | • PASS   (Exit 0)         | Clean or justified by orphaned symbol.
                      | • REVIEW (Exit 1)         | Legitimate refactor; needs non-author human approval.
                      | • BLOCK  (Exit 2)         | Unjustifiable tampering (CI sabotage, shadow mock).
                      +───────────────────────────+
```

### Why Existing Approaches Fail

| Approach | Latency | Deterministic? | Understands Diffs? | Vulnerability / Fatal Flaw |
|---|---|---|---|---|
| **`CODEOWNERS` on `tests/`** | 0ms | Yes | No | Blocks *all* test edits. Abandoned the moment an agent is asked to write tests. |
| **System Prompts (`AGENTS.md`)** | 0ms | No | No | Fails under loop pressure (*ImpossibleBench* proved models ignore them). |
| **LLM PR Reviewers (CodeRabbit)** | 30–60s | No | Partial | Slow, expensive (\$0.30/run), and vulnerable to adversarial prompt injection. |
| **Full Mutation (Stryker)** | 15–45m | Yes | No | Unusable in CI PR gating or agent tool loops due to runtime. |
| **AgentProctor (`proctor`)** | **<200ms** | **100% Yes** | **Bilateral AST** | **Sub-second, zero-cloud, immune to prompt injection, cryptographic review waivers.** |

---

## 5-Second Quickstart

### Installation

```bash
# Via Cargo
cargo install agent-proctor

# Via Python / pip
pip install agent-proctor

# Via npm / npx
npx agent-proctor scan
```

### Run Locally on Current Branch

```bash
# Static AST scan (<200ms)
proctor scan --base origin/main

# Dynamic ephemeral mutation probe with sentinel tracking
proctor probe --base origin/main --head HEAD
```

---

## GitHub Action Integration

Add `agent-proctor` as a required status check in `.github/workflows/proctor.yml`:

```yaml
name: Agent Proctor PR Integrity Gate

on:
  pull_request:
    branches: [main]

jobs:
  proctor:
    runs-on: ubuntu-latest
    permissions:
      contents: read
      pull-requests: read
      security-events: write

    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0

      - name: Run AgentProctor Gate
        uses: no-hup/agent-proctor@v1
        with:
          base-ref: ${{ github.event.pull_request.base.sha }}
          head-ref: ${{ github.event.pull_request.head.sha }}
          sarif-file: proctor-results.sarif

      - name: Upload SARIF to GitHub Code Scanning
        if: always()
        uses: github/codeql-action/upload-sarif@v3
        with:
          sarif_file: proctor-results.sarif
```

---

## Inner-Loop Agent Integration (Claude Code & Cursor)

Stop agents from cheating *before* code is ever committed to GitHub:

### Claude Code `Stop` Hook (`.claude/hooks/stop.sh`)

```bash
#!/usr/bin/env bash
# Runs before Claude claims completion
REPORT=$(proctor scan --json --base origin/main)
STATUS=$(echo "$REPORT" | jq -r '.verdict.status')

if [ "$STATUS" = "block" ] || [ "$STATUS" = "review" ]; then
  echo "AgentProctor Integrity Check Failed:"
  echo "$REPORT" | jq -r '.findings[] | "- \(.rule_id): \(.message)"'
  echo "Restore the deleted/weakened assertions or justify them before completing."
  exit 1
fi
```

---

## Core Detection Rules

* **`PROCTOR-001` (Assertion Lock)**: Tracks exact $\to$ weak $\to$ vacuous downgrades per test. Downgrades are justified only if the production code deleted an identifier or attribute the assertion referenced (Orphaned Reference rule).
* **`PROCTOR-002` (Shadow Mocking)**: Flags tests that introduce mocks targeting in-repository modules/functions that the test itself executes.
* **`PROCTOR-003` (CI Workflow Sabotage)**: Blocks injection of `|| true`, `continue-on-error: true`, or step deletions on test runner steps.
* **`PROCTOR-004` (Runner Config Tamper)**: Blocks coverage threshold drops (`fail_under`) or `--ignore` flags in `pytest.ini`, `jest.config.js`, etc.
* **`PROCTOR-005` (Snapshot Integrity)**: Catches deleted snapshots, snapshots shrinking by $>50\%$, or snapshots containing error literals.
* **`PROCTOR-006` (Parametrize Row Drop)**: Detects deleted rows in `@pytest.mark.parametrize` and Go table-driven tests.

---

## Cryptographic Human Waivers

In-diff comments (`# proctor: allow`) can be forged by autonomous agents. `agent-proctor` validates waivers strictly through **authenticated GitHub PR review comments**:

```text
proctor: accept 7a9f2b1c4e8d
```

A waiver is valid in CI **if and only if**:
1. The comment user is **not** the PR author or any committer on `HEAD`.
2. The user is **not a bot**.
3. The user has **write permissions** on the repository.
4. The comment is newer than the last commit touching that file.

---

## License

Apache-2.0. See [LICENSE](LICENSE) for details.
