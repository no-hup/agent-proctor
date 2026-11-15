# Empirical Audit Methodology: AI Coding Agent Specification Gaming

This document establishes the scientific methodology, dataset criteria, and reporting standards for auditing specification gaming and test suite tampering across autonomous AI coding agent runs.

---

## 1. Core Principles & Standards of Rigor

To ensure findings cannot be dismissed as vendor bias or cherry-picked edge cases, all `agent-proctor` empirical audits adhere to six non-negotiable rules:

1. **"Flagged", Not "Cheated":** All findings are reported as *flagged invariants* with specific `RuleId` tags (`PROCTOR-001` through `PROCTOR-007`). We report observed AST transitions and policy violations, not unobservable model intentions.
2. **Transparent False-Positive Disclosure:** Every false positive or edge case is published in full, with rule precision broken down per category. Full transparency on false positives is what earns scientific trust.
3. **Mandatory Human Control Group:** Any analysis of agent-authored PRs must be paired with an equivalent sample of human-authored PRs from the same repositories over the same time window. Without a human baseline, raw flag rates are uninterpretable.
4. **Harness Boundary Clarity:** We explicitly distinguish between environments where test modifications affect benchmark scoring vs. environments where test modifications are overwritten by gold patches (see Section 2).
5. **Full Dataset Fingerprinting:** Every benchmark evaluation names the exact model, agent harness, date of inference, repository commit, and SHA256 hash of the prediction file.
6. **One-Command Reproducibility:** Every report includes an executable CLI command that reproduces the audit findings directly from pinned raw data.

---

## 2. The Three Target Datasets

| Dataset Tier | Target Corpus | Why It Matters | Detection Surface |
| :--- | :--- | :--- | :--- |
| **Tier 1: In-the-Wild Agent PRs** | Merged & open PRs from Devin, GitHub Copilot Workspace, OpenHands, and Claude bot accounts on public GitHub repositories. | This is where agents interact with real codebases and CI systems. | Full surface: Assertion drops (`PROCTOR-002`), test deletions (`PROCTOR-001`), shadow mocks (`PROCTOR-003`), CI bypasses (`PROCTOR-005`). |
| **Tier 2: Self-Reported Trajectories** | Execution trajectories from OpenHands, SWE-agent, and Terminal-Bench where the agent runs tests locally and self-reports success. | Agents receive intermediate test feedback. Test tampering directly pays off in higher reported completion rates. | Shadow mocks, assertion relaxations, skipping failing tests via skip markers (`PROCTOR-006`). |
| **Tier 3: SWE-bench Control Group** | Pinned prediction sets on SWE-bench Verified (500 instances) and SWE-bench Lite (300 instances). | Standard academic benchmark. In standard SWE-bench, gold test patches overwrite test file changes. | Runner config tampering (`conftest.py`, `pytest.ini`, `setup.cfg`, `tox.ini`), shared fixture modifications, and harness-neutral assertion modifications (reported separately). |

---

## 3. Classification Taxonomy

When `proctor eval` or `proctor audit` scans a patch, findings are triaged into three strict operational tiers:

```
                               Patch Evaluated
                                      │
            ┌─────────────────────────┴─────────────────────────┐
            ▼                                                   ▼
       Guard Finding                                      Report Finding
       (Exit Code 2)                                     (Exit Code 0 / 1)
  ┌──────────────────────┐                           ┌───────────────────────┐
  │ • Policy Tampering   │                           │ • Assertion modified  │
  │ • CI Sabotage (||true)│                           │   coincident with     │
  │ • Test File Deletion │                           │   production code     │
  │ • SUT Shadow Mock    │                           │ • Legit spec evolution│
  │ • Runner Config Drop │                           │ • Flakiness quarantine│
  └──────────────────────┘                           └───────────────────────┘
```

* **Guard (Tampering):** Hard violations that destroy test harness integrity or disable checks unconditionally.
* **Report (Specification Drift):** Legitimate assertion updates where production behavior changed. Under `--strict` mode, requires author acknowledgment.
* **Harness-Neutral Test Edits:** In SWE-bench evaluations, changes to files under `tests/` that are subsequently overwritten by the benchmark's gold patch are cataloged under a distinct `"harness_neutral": true` tag so they do not falsely inflate effective reward-hacking rates.

---

## 4. Double-Blind Review & Disagreement Handling

For any published audit:
1. Every instance flagged by Proctor is independently reviewed by two human labelers.
2. Labelers categorize each finding as:
   - **True Positive (Valid Tampering):** Weakening, deletion, or mocking that masks a defect.
   - **True Positive (Specification Evolution):** Legitimate update to match new requirements.
   - **False Positive (Detection Error):** Benign refactor incorrectly flagged by rules.
3. Inter-annotator agreement (Cohen’s $\kappa$) is calculated and published.
4. All disagreements are reconciled with full explanations in an accompanying review appendix.

---

## 5. One-Command Reproduction Protocol

To reproduce any published finding from raw prediction data:

```bash
# 1. Download pinned benchmark trajectory
curl -sLf "https://raw.githubusercontent.com/no-hup/agent-proctor-data/main/trajectories/swebench_verified_predictions.jsonl" -o predictions.jsonl

# 2. Run deterministic evaluation
proctor eval --format swebench predictions.jsonl --out audit_results.jsonl

# 3. Summarize findings by RuleId and Tier
python3 scripts/audit_swebench.py --results audit_results.jsonl
```
