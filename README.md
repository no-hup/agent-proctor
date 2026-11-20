# AgentProctor (`agent-proctor`)

> **The deterministic PR integrity & anticheat gate for autonomous AI coding agents.**  
> Stops agents from turning CI green by weakening assertions, shadow mocking, deleting tests, or tampering with workflows.

[![CI](https://github.com/no-hup/agent-proctor/actions/workflows/ci.yml/badge.svg)](https://github.com/no-hup/agent-proctor/actions/workflows/ci.yml)
[![License: Apache-2.0](https://img.shields.io/badge/License-Apache--2.0-blue.svg)](LICENSE)
[![crates.io](https://img.shields.io/crates/v/agent-proctor.svg)](https://crates.io/crates/agent-proctor)

> **Supported Languages:** Python (`pytest`, `unittest`) and JavaScript/TypeScript (`Jest`, `Vitest`).

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

### Threat Model: Class A vs. Class B Cheats
`agent-proctor` specifically hardens the **repository's visible grader and test harness (Class A)**:
* **Class A (Grader & Test Tampering — Defeated by Proctor):** Deleting test cases, weakening assertions, shadow mocking the system under test, tweaking runner configs, and sabotaging CI workflows (`|| true`).
* **Class B (Input Overfitting — Requires Holdouts):** Hardcoding specific return values for test inputs in production code without touching test files (e.g. `if x == 10: return 42`). Proctor detects literal mirroring (when expected literals are copied into production), but defeating Class B fundamentally requires **private, uncommitted evaluation holdouts** that agents cannot read.

---

## 🔒 Security, Privacy & Zero-Egress Guarantees

We recognize that open-source maintainers and enterprise security teams require mathematically verifiable supply-chain integrity:

1. **Zero-Egress Airgapped Architecture:**
   * The `proctor` binary is compiled with **zero networking libraries** (no `reqwest`, `hyper`, `curl`, sockets, or HTTP capabilities).
   * It **cannot** make outbound network connections, phone home, send telemetry, or exfiltrate environment secrets.
   * Automated CI regression tests (`test_zero_network_dependencies_in_lockfile`) continuously enforce that no networking crates can enter the dependency tree.
2. **Least-Privilege Token Permissions:**
   * Proctor runs under strict read-only permissions (`contents: read`, `pull-requests: read`).
   * It **never** asks for and should never be granted `contents: write`, `id-token: write`, or access to repository secrets.
3. **Cryptographic Build Provenance (GitHub Sigstore Attestation):**
   * Every release binary is cryptographically signed and attested directly by GitHub's official build infrastructure:
     ```bash
     gh attestation verify proctor-linux-x86_64.tar.gz --owner no-hup
     ```
4. **Local SARIF Output (No Third-Party Servers):**
   * Findings are written locally to a standard file (`proctor-results.sarif`).
   * Rendering is performed by GitHub's official, open-source `github/codeql-action/upload-sarif@v3` action directly inside your repository's native **Security / Code Scanning** tab.

---

## The Solution: Deterministic 3-Way Triage

`agent-proctor` is a sub-second, zero-cloud AST gate engineered in **Rust** (Free for Public Open Source & Personal Use). It eliminates the false-positive death spiral through a **three-way verdict system**:

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
# Build from source or install via Cargo
cargo install agent-proctor
```

### Run Locally on Current Branch

```bash
# Static AST scan (<200ms)
proctor scan --base origin/main

# Audit last 50 merged commits to measure historical false-positive rate
proctor audit --limit 50

# Dynamic ephemeral mutation probe with sentinel tracking
proctor probe --base origin/main --head HEAD
```

---

## Configuration (`proctor.toml`)

Customize allowed internal collaborator mocks and ignore rules in your repository root:

```toml
# proctor.toml

# Allowed mock prefixes for legitimate internal collaborators
allow_mocks = [
  "app.clients.*",
  "app.services.mailer",
  "tests.fixtures.*"
]

# Paths ignored during integrity scanning
ignored_paths = [
  "vendor/**",
  "node_modules/**"
]

# Guard against CI workflow sabotage (.github/workflows/*)
enforce_ci_tampering = true

# Demote test skip/xfail to REVIEW when underlying production code is untouched (flakiness quarantine)
skip_demote_to_review = true

# In strict mode, REVIEW findings also exit with code 2 (BLOCK)
strict_mode = false
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
        uses: no-hup/proctor-action@v1
        with:
          base-ref: ${{ github.event.pull_request.base.sha }}
          head-ref: ${{ github.event.pull_request.head.sha }}
          strict: 'true'  # Enforce exit 1 on Report findings for unattended agent PRs
          sarif-file: proctor-results.sarif

      - name: Upload SARIF to GitHub Code Scanning
        if: always()
        uses: github/codeql-action/upload-sarif@v3
        with:
          sarif_file: proctor-results.sarif
```

> **Autonomous Agent Pipelines:** For automated, unattended agent PRs (e.g. Devin, OpenHands, sweep bots), always set `strict: 'true'` (or pass `--strict`). Under default advisory mode, Report-tier findings (such as assertion modifications coincident with production code changes) exit 0 as an advisory receipt. With `strict: 'true'`, Report findings exit 1, blocking the PR until explicitly acknowledged by the author (`proctor: ack <id>`) or waived by a maintainer.

---

## Inner-Loop Agent Integration (Claude Code & Cursor)

Stop agents from cheating *before* code is ever committed to GitHub:

### Claude Code `Stop` Hook (`.claude/hooks/stop.sh`)

```bash
#!/usr/bin/env bash
# Runs before Claude claims completion
REPORT=$(proctor scan --format json --base origin/main)
STATUS=$(echo "$REPORT" | jq -r '.verdict.status')

if [ "$STATUS" = "block" ]; then
  echo "AgentProctor Integrity Check Failed (Hard Guard Violation):"
  echo "$REPORT" | jq -r '.findings[] | "- \(.rule_id): \(.message)"'
  echo "Restore the deleted/weakened assertions or justify them before completing."
  exit 2
fi
```

---

## Agent Self-Proctoring (CLAUDE.md / AGENTS.md / Cursor Rules)

Add this to your repository's agent instructions:

> Before declaring a task complete or opening a PR, run:
> `proctor scan --format markdown`
> Ensure there are zero blocking findings and include the generated receipt in your PR summary.

---

## Core Detection Rules

* **`PROCTOR-001` (Assertion Lock)**: Tracks exact $\to$ weak $\to$ vacuous downgrades per test. Downgrades are justified only if the production code deleted an identifier or attribute the assertion referenced (Orphaned Reference rule).
* **`PROCTOR-002` (Shadow Mocking)**: Flags tests that introduce mocks targeting in-repository modules/functions that the test itself executes.
* **`PROCTOR-003` (CI Workflow Sabotage)**: Blocks injection of `|| true`, `continue-on-error: true`, or step deletions on test runner steps.
* **`PROCTOR-004` (Runner Config Tamper)**: Blocks coverage threshold drops (`fail_under`) or `--ignore` flags in `pytest.ini`, `jest.config.js`, etc.
* **`PROCTOR-005` (Snapshot Integrity)**: Catches deleted snapshots, snapshots shrinking by $>50\%$, or snapshots containing error literals.
* **`PROCTOR-006` (Parametrize Row Drop)**: Detects deleted rows in `@pytest.mark.parametrize` and table-driven tests.
* **`PROCTOR-007` (Policy Tamper)**: Blocks any PR diff that modifies, adds, or deletes `proctor.toml` without authorized base-branch merge.

---

## Attributed Acknowledgements & Review Waivers

In-diff comments (`# proctor: allow`) can be forged by autonomous agents. `agent-proctor` validates waivers strictly through **authenticated GitHub PR review comments**:

```text
proctor: ack 7a9f2b1c4e8d
```

Configured via `review_mode` in `proctor.toml`:
1. `review_mode = "ack"` (default): Accepts attributed acknowledgement comments (`proctor: ack <id>`) from either the PR author or any repository collaborator.
2. `review_mode = "second_reviewer"`: Strictly requires sign-off from an authenticated collaborator or owner who is not the PR author.
3. `review_mode = "off"`: Disables waiver processing.

When running in GitHub Actions, `agent-proctor` automatically renders a live findings table and waiver instructions directly to `$GITHUB_STEP_SUMMARY`, ensuring developers and reviewers have zero-friction copy-paste waiver commands even on fork PRs.

---

## Documentation & Roadmap

* **[Architecture Decision Records (ADRs)](docs/DECISIONS.md)**: Rationale behind our zero-LLM architecture, orphaned reference logic, and waiver design.
* **[Project Roadmap](ROADMAP.md)**: Shipped milestones (Phases 0–2), empirical benchmark grounding (Phase 3), multi-language expansions, and upcoming releases.

---

## License

Apache-2.0. See [LICENSE](LICENSE) for details.
