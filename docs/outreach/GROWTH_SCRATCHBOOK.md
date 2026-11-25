# `agent-proctor` Growth & Outreach Scratchbook

## Now (Read First — Updated 2026-11-26)
- **Current Focus:** Prove `agent-proctor` catches real coding-agent tampering and earns sticky loop/PR usage with 3 warm teams before any public Show HN launch.
- **Active Bets:**
  1. **Headless Agent Cheat Benchmark (`2–3 days`, due 2026-11-29):** Build 20 conflicting/impossible tasks + 20 control tasks, run Claude Code & Codex headless (hook off), measure catch rate & control FPs -> `docs/audit/AGENT_CHEAT_BENCH.md` *(replaces large OSS history mining; cap any OSS sanity check at <= 4h)*.
  2. **3 Warm Design-Partner Installs (started 2026-11-26, Day-7 check-in 2026-12-03):** Install local Claude Code/Cursor stop-hook + Observe Action with 3 people already running coding agents heavily; evaluate on Day 7 with two questions: *"Did you read a receipt?"* and *"Did it change anything you did?"*.
  3. **Product UX Spike (queued):** Surface the 1-line verdict directly in the CI **check-run / job title** (e.g. `proctor: 2 tests deleted, 1 assertion weakened`), not only buried inside `$GITHUB_STEP_SUMMARY`.
- **Blocked / Parked:** Cold outreach (`browser-use`, Tier-1 OSS) is **PARKED**; Show HN (`docs/launch/SHOW_HN.md`) is **HELD** until Bet 1 + Bet 2 yield concrete proof. Zero emails sent without explicit approval.
- **Measurable North-Star:** Weeks active design-partner repos keep Observe/hook enabled + weekly Day-7 two-question partner answers (not GitHub stars).

---

## Standing Rules (Shaurya & Agents)
1. **Before proposing outreach or GTM:** Read `Now` + `Experiment Log` + `Rejected / Parked` below. Single contact ledger lives in [OUTREACH_LOG.md](OUTREACH_LOG.md) (do not duplicate contacts here).
2. **After every outreach batch, reply, or experiment:** Append one dated row to `Experiment Log` with raw facts (`n`, replies, churn reasons). Never delete failed bets.
3. **Never copy audit tables here:** Cite [V0_1_VS_V0_2_COMPARISON.md](../audit/V0_1_VS_V0_2_COMPARISON.md) directly (`v0.2.2` summary: `0–13%` yellow rate across 600 commits; static rules alone stay Observe-default because ~half of Guard flags are legitimate maintenance, so real painkiller value sits in `verify` + local agent hooks).
4. **No external side effects unapproved:** Zero emails (`custom-send-email`), zero unsolicited PRs/issues, zero HN posts without explicit user sign-off.

---

## Experiment Log

| Date | Experiment | Outcome / Raw Fact | Decision / Rule Taken |
| :--- | :--- | :--- | :--- |
| 2026-11-17 | Batch 1 cold `v0.1.0` emails (`aider`, `SWE-agent`, `OpenHands`, `litellm`) | No adoption reported yet; post-hoc Track B audit showed `OpenHands` (`33%` review) came largely from `.tsx`/co-evolution noise and `litellm` (`21%`) came largely from co-evolution + Python `with`-block bugs. | Never quote audit numbers without hand-labeling >=20 surviving flags. If Graham/Krrish reply or cite `v0.1.0`, send a 4-line factual correction only; do not double-email if silent. |
| 2026-11-25 | 5-round persisted Grok GTM/ICP check (`grok-product-gutcheck`) | Classified `v0.2.2` Observe static diff as vitamin until burned; identified mega external-PR OSS firehoses as high-bounce and mid-size fast-CI Python/TS teams merging headless/background agent PRs as real ICP. | Park Batch 2 cold mega-OSS outreach (`browser-use`, Pydantic, FastAPI). Hold Show HN until real proof artifacts exist. |
| 2026-11-26 | Persisted Claude strategy audit (`f17ec863`, `answer-30`) | Rejected heavy OSS historical mining (human OSS merges rarely contain traceable test-tampering escapes; `verify` on incomplete fixes still sees `Catches change`); pushed check-run title verdicts over summary-only receipts. | Pivot Bet 1 from OSS history mining (cap sanity check at <=4h) to `20+20` headless coding-agent impossible-task benchmark + ImpossibleBench public rollouts check + 3 warm partner installs. |

---

## ICP & Anti-ICP

| Keepers (Target) | Bouncers (Parked / Skip) |
| :--- | :--- |
| Teams actively merging PRs from headless/background agents or heavy Claude Code / Cursor loops | Famous OSS triage firehoses flooded by drive-by external PRs (`browser-use`, FastAPI, Pydantic) |
| Python (`pytest`) or JS/TS (`Jest`, `Vitest`), ~10k–500k LOC, minute-scale CI | Polyglot / Bazel / custom-harness monorepos or 45-min flaky suites |
| 1–3 leads who act on PR check titles / run local agent hooks without a tools committee | Enterprise security-approval bottlenecks or Codecov-only shops |

---

## Kill & Pivot Criteria
- **Bet 1 (Agent Cheat Benchmark — revisit 2026-11-29):** If across 20 impossible tasks (`Claude Code` + `Codex` headless, hook off) agents either never cheat OR `proctor` catches <50% cleanly without false alarms on the 20 controls, pause all launch/outreach and fix detection/positioning first.
- **Bet 2 (3 Warm Partners — revisit 2026-12-03):** If <2 of 3 partners keep the hook/Action running past Day 7 or say *"never looked at it / didn't change anything"*, drop PR-receipt copy and inspect onboarding/check-title friction directly.

---

## Rejected / Parked Ideas (Do Not Re-Suggest Unchanged)
- Cold "we scanned your clean PR" DMs/emails to mega-OSS maintainers (`browser-use`, Tier-1 frameworks).
- Multi-week OSS git-history scraping as primary proof (`Revert` / `skip->unskip` noise too high; cap any quick check at <=4h).
- Product Hunt, generic awesome-list PRs, automated cold `proctor.yml` PRs, SaaS dashboard before 10 sticky installs, pitching `--enforce` blocking as the headline.
