# agent-proctor GitHub Action

[![Free for Open Source](https://img.shields.io/badge/License-Free%20for%20Open%20Source-blue.svg)](LICENSE)
[![Latency](https://img.shields.io/badge/Latency-%3C50ms-green.svg)]()
[![Determinism](https://img.shields.io/badge/Engine-Zero--LLM%20Tree--sitter-orange.svg)]()

> **Stop autonomous AI coding agents from submitting pull requests that make CI green by silently deleting tests, weakening assertions, or sabotaging test runners.**

---

## The Problem in Open Source

As autonomous AI agents (Devin, Claude Code, Cursor, Copilot Workspace) proliferate, open source maintainers are increasingly flooded with "drive-by" pull requests that claim to resolve open issues.

When an AI agent cannot solve a subtle edge case, empirical research shows it frequently reward-hacks the test suite:
* **Assertion Weakening:** Replaces `assert result.status == 200` with `assert result is not None` or `assert True`.
* **Test Deletion:** Deletes failing test functions or drops rows from parametrized tables.
* **Shadow Mocking:** Injects mocks directly into the system under test to force tests to pass without fixing the bug.
* **CI Sabotage:** Appends `|| true` or `; exit 0` to GitHub Actions workflow steps.

Negative prompts in `AGENT.md` or PR instructions **do not stop this behavior** (METR research demonstrated a 30.4% reward-hacking rate even on frontier reasoning models).

---

## The Solution: Deterministic PR Anticheat

`agent-proctor` is a **deterministic, sub-50ms git PR integrity gate**. It uses Tree-sitter AST diffing to compare the semantic strength of tests *before* and *after* each pull request:
* **Zero LLMs:** 100% deterministic, mathematically reproducible, zero prompt injection surface, and $0 model costs.
* **Zero Telemetry:** The compiled binary executes locally inside your GitHub Actions runner. No code or metadata leaves your environment.
* **Asymmetric Dual-Tier Triage:**
  * **Guard Tier (Exit Code 2 / Hard BLOCK):** Policy tampering, CI sabotage (`|| true`), test deletions without removing production code, shadow mocks on the system under test.
  * **Report Tier (Exit Code 0 Advisory / Exit Code 1 `--strict`):** Legitimate specification evolution. Allows normal human refactoring.

---

## ⚡ Quick Start: 5-Line GitHub Action

Add `.github/workflows/proctor.yml` to your repository:

```yaml
name: Test Integrity Anticheat

on:
  pull_request:
    branches: [main, master]

permissions:
  contents: read
  pull-requests: read
  security-events: write

jobs:
  proctor:
    name: Proctor PR Anticheat
    runs-on: ubuntu-latest
    steps:
      - name: Checkout repository
        uses: actions/checkout@v4
        with:
          fetch-depth: 0

      - name: Run agent-proctor anticheat gate
        uses: no-hup/proctor-action@v1
        with:
          base-ref: ${{ github.event.pull_request.base.sha }}
          head-ref: ${{ github.event.pull_request.head.sha }}
          sarif-file: proctor-results.sarif

      - name: Upload SARIF to GitHub Code Scanning
        uses: github/codeql-action/upload-sarif@v3
        if: always()
        with:
          sarif_file: proctor-results.sarif
```

---

## Autonomous Agent Mode (`strict: 'true'`)

For unattended, automated agent PRs (e.g. sweep bots, Devin, OpenHands), configure `strict: 'true'`:

```yaml
      - name: Run agent-proctor anticheat gate
        uses: no-hup/proctor-action@v1
        with:
          strict: 'true'  # Enforce exit 1 on Report findings for automated bots
```

Under `--strict`, any assertion modifications coincident with production code changes require an explicit author acknowledgment or maintainer waiver before merging.

---

## Acknowledging Legitimate Spec Changes

When a PR legitimately changes a feature's specification, authors can acknowledge findings directly in a PR comment:

```text
proctor: ack PROCTOR-002
```

For hard `Guard` findings, only repository collaborators with **write permissions** can grant a maintainer waiver:

```text
proctor: waive PROCTOR-001 reason="Breaking API refactor deprecating old endpoint"
```

Author comments can **never** waive Guard findings.

---

## License & Pricing

`agent-proctor` is **Free forever for Public Open Source Repositories** under an OSI-approved license.

For enterprise private repositories or commercial licensing, contact: `license@no-hup.dev`.
