# Track D Outreach Drafts (`v0.2.2` Calibrated)

> **STATUS: DRAFT — DO NOT SEND UNTIL EXPLICITLY APPROVED.**
> Zero emails have been dispatched from this batch. Every benchmark number and PR receipt below was generated directly on `v0.2.2` ([V0_1_VS_V0_2_COMPARISON.md](../audit/V0_1_VS_V0_2_COMPARISON.md)).

---

## 1. Follow-Up to Graham Neubig (`OpenHands/OpenHands`)

- **Channel / Reply Target:** Email to `neubig@gmail.com` (replying to Gmail thread/ID `1a1e66db099b4a69` from Nov 17)
- **Subject:** Re: quick audit of OpenHands PR test integrity — fixed our noisy v0.1.0 flags (v0.2.2)

```text
Hey Graham,

Quick follow-up on my note from last week—I owed you a correction.

When I first ran agent-proctor (v0.1.0) over OpenHands, it flagged ~33% of commits for review and ~11% as blocks. Digging in by hand, almost all of that noise was on us: our TypeScript parser flagged valid `.tsx` / `typeof import(...)` syntax as errors, local helpers triggered substring shadow-mock hits, and legitimate co-evolution with touched production code showed up as review warnings.

We fixed those parser bugs and added definition-linkage calibration in v0.2.2, and moved the default workflow to Observe Mode (`enforce: false`) + Test-Change Receipts so builds never block while reviewers get a clear summary of what changed in `tests/` (plus an opt-in `proctor verify` flag to run added/modified tests against `base` to prove reproduction).

Re-running OpenHands over a 100-commit window (`e1b8b60f..c11b5fc6`) dropped false noise sharply:
- v0.1.0 -> v0.2.2: 56/33/11 (Pass/Review/Block) -> 89/7/4 (7.0% review, 4.0% block)
- Guard precision for true reviewer attention across our primary/holdout benchmark is ~43-46% (legitimate test deletions and constant updates still account for the rest), which is why Observe Mode (`enforce: false`) is the default.

For example, running `proctor scan` on OpenHands #19954 (`d56c1ab`) and #19272 (`adf71d6`) produces clean green receipts:

### 📋 Test-Change Receipt (agent-proctor) — #19954 (`d56c1ab`)
| Category | Count | Status |
|---|---|---|
| Added tests | 13 | all contain an exact-value assertion (not executed) |
| Modified tests | 0 | — |
| Deleted tests | 0 | — |
| Configs & CI gates | 0 | ✅ no loosening found |
| Blanket suppressions | 0 | ✅ none added |
| Internal mocks | 0 | ✅ none added |
Verdict: 🟢 No findings in analyzed test files.

Full benchmark comparison and labeled CSVs are public in the repo (`docs/audit/V0_1_VS_V0_2_COMPARISON.md`). If automatic Test-Change Receipts on `$GITHUB_STEP_SUMMARY` sound useful on OpenHands PRs, happy to share the 5-line `no-hup/proctor-action@v1` setup—otherwise appreciate the awesome work on OpenHands and won't follow up again!

Best,
Shaurya
https://github.com/no-hup/agent-proctor
```

---

## 2. Follow-Up to Krrish Dholakia (`BerriAI/litellm`)

- **Channel / Reply Target:** Email to `krrish@berri.ai` (replying to Gmail thread/ID `1a1e66dc6434541e` from Nov 17)
- **Subject:** Re: LiteLLM test-integrity baseline — fixed our noisy Python `with` / TSX rules (v0.2.2)

```text
Hey Krrish,

Quick follow-up with a mea culpa on the v0.1.0 baseline I sent over last week.

A closer hand-audit showed a chunk of the flags we reported on LiteLLM came from bugs on our side—mainly treating arbitrary Python `with ...:` context managers as `pytest.raises` blocks, choking on `.tsx` `typeof import(...)`, and counting benign co-evolution where the underlying function actually changed.

In v0.2.2 we fixed those parsers, gated `with`-block assertions properly, and centered the tool on non-blocking Test-Change Receipts (`enforce: false`) in `$GITHUB_STEP_SUMMARY` plus opt-in `proctor verify` (running new/edited tests on `base` to prove they reproduce the bug before the fix).

On a 100-commit LiteLLM window (`13101c8a..34dea88f`):
- v0.1.0 -> v0.2.2: 75/13/12 -> 83/5/12 (5.0% yellow review rate; PROCTOR-001 alerts cut 145 -> 83)
- Since static AST checks still surface ~43-46% reviewer-attention precision across repositories (human test cleanup accounts for the rest), Observe Mode is the default and blocking is opt-in.

Running v0.2.2 on LiteLLM #54379 (`2c4d2344`) outputs:

### 📋 Test-Change Receipt (agent-proctor) — #54379 (`2c4d2344`)
| Category | Count | Status |
|---|---|---|
| Added tests | 5 | all contain an exact-value assertion (not executed) |
| Modified tests | 0 | — |
| Deleted tests | 0 | — |
| Configs & CI gates | 0 | ✅ no loosening found |
| Blanket suppressions | 0 | ✅ none added |
| Internal mocks | 0 | ✅ none added |
Verdict: 🟢 No findings in analyzed test files.

All comparison scripts and CSV labels are at `github.com/no-hup/agent-proctor/tree/main/docs/audit`. If having automatic Test-Change Receipts in LiteLLM's workflow summaries would save reviewers time on external/agent PRs, happy to share the 5-line read-only Action config—otherwise no need to reply!

Best,
Shaurya
https://github.com/no-hup/agent-proctor
```

---

## 3. Batch 2 Draft — Gregor Žunič (`browser-use/browser-use`)

- **Channel Target:** X/Twitter DM `@gregpr07` (note: public GitHub API profile has `email: null`; confirm email or send via X DM only after approval)

```text
Hey Gregor,

With the volume of PRs browser-use merges, we ran our open-source Test-Change Receipt CLI (`agent-proctor` v0.2.2, zero-egress Rust, Apache-2.0) across 200 recent `browser-use` commits (100 primary + 100 holdout) to stress-test our precision before launching.

Recent commits like `9afbc07e` ("Gateway calls ask for 16384 output tokens, not 32768") come out clean:

### 📋 Test-Change Receipt (agent-proctor)
| Category | Count | Status |
|---|---|---|
| Added tests | 1 | all contain an exact-value assertion (not executed) |
| Modified tests | 0 | — |
| Deleted tests | 0 | — |
| Configs & CI gates | 0 | ✅ no loosening found |
| Blanket suppressions | 0 | ✅ none added |
| Internal mocks | 0 | ✅ none added |
Verdict: 🟢 No findings in analyzed test files.

Across the 200-commit benchmark, 70-72% pass cleanly with an 8-13% quiet review rate—and when `verify: true` is enabled, it executes added/edited tests against `base` to prove bug reproduction (`Catches change`) rather than trusting static rules alone. Because static heuristics alone have ~43-46% true reviewer-attention rate on real commits, `no-hup/proctor-action@v1` defaults to non-blocking Observe Mode (`enforce: false`) in `$GITHUB_STEP_SUMMARY`.

Would love to hear if automatic PR Test-Change Receipts on `browser-use` would be useful (happy to send the 5-line read-only YAML, zero external calls)—either way, congrats on v1.0.8!

Best,
Shaurya
https://github.com/no-hup/agent-proctor
```
