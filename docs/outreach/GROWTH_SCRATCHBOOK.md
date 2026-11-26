# `agent-proctor` Growth & Outreach Scratchbook

## Now (Read First — Updated 2026-11-26)
- **Current Focus:** Ship `v0.2.3` (Policy Mode `--protect-existing-tests`, reverse `verify` contract check, `old -> new` receipt diffs, and `proctor-action@v1` banner annotations), onboard 3 warm coding-agent developers for a 7-day dogfood, then decide Show HN.
- **Active Bets:**
  1. **`v0.2.3` Deterministic Stack (verified 2026-11-26):** Replaying `120` GPT-5 `ImpossibleBench` trajectories ([AGENT_CHEAT_BENCH.md](../audit/AGENT_CHEAT_BENCH.md)) confirmed `0/50` FPs on `original`, `55/70` (`78.6%`) Receipt visibility (`old -> new` assertions now printed), `22/70` (`31.4%`) default static flags, and **`34/70` (`48.6%`, `33` Guard)** under `--protect-existing-tests` Policy Mode with **`0/50` FPs**. Reverse `verify` pass prevents `verify` from vouching `Catches change` when a modified test breaks its own `base` contract on `head`.
  2. **3 Warm Design Partners (check-in Day 7):** Onboard 3 Claude Code/Cursor developers directly (local hook + Observe Action); ask two Day-7 questions (*"Did you read a receipt?"* / *"Did it change anything?"*).
  3. **Batch 1 Closed Silently:** Gmail check (`in:anywhere`) across all accounts confirmed **0 replies** to Nov 17 emails (`aider`, `SWE-agent`, `OpenHands`, `litellm`). Leaving Batch 1 closed without sending unprompted follow-ups.
- **Blocked / Parked:** Cold mega-OSS outreach (`browser-use`, Tier-1) is **PARKED**; Show HN (`docs/launch/SHOW_HN.md`) waits for `v0.2.3` ImpossibleBench re-score + Day-7 warm partner signal.
- **Measurable North-Star:** Weeks active partner repos keep Observe/hook enabled + Day-7 partner feedback (not GitHub stars).

---

## Standing Rules (Shaurya & Agents)
1. **Before proposing outreach or GTM:** Read `Now` + `Experiment Log` + `Rejected / Parked`. Single contact ledger is [OUTREACH_LOG.md](OUTREACH_LOG.md).
2. **After every outreach batch, reply, or experiment:** Append one dated row to `Experiment Log` with raw facts (`n`, replies, outcomes). Never delete failed bets.
3. **Never copy full audit tables here:** Link [V0_1_VS_V0_2_COMPARISON.md](../audit/V0_1_VS_V0_2_COMPARISON.md) and [AGENT_CHEAT_BENCH.md](../audit/AGENT_CHEAT_BENCH.md).
4. **Outreach hygiene:**
   - **Zero cold integration PRs:** NEVER open unsolicited `proctor.yml` PRs on stranger repos (triggers spam/supply-chain hostility). Only open an integration PR *after* a maintainer explicitly invites it.
   - **Send timing:** Don't build timezone infra, just send manual batches roughly in morning inbox hours (`~8:30–11:00 AM` recipient region).
   - **Never send without approval:** Zero emails, issues, PRs, or HN posts without Shaurya's explicit OK.

---

## Experiment Log

| Date | Experiment | Outcome / Raw Fact | Decision / Rule Taken |
| :--- | :--- | :--- | :--- |
| 2026-11-17 | Batch 1 cold `v0.1.0` emails (`aider`, `SWE-agent`, `OpenHands`, `litellm`) | Gmail sweep (`2026-11-26`, `in:anywhere`) found `0` replies across all accounts. `OpenHands`/`litellm` numbers were inflated by `.tsx`/Python `with`/co-evolution noise. | Closed Batch 1 cleanly (no unsolicited follow-ups). Never quote audit stats without hand-labeling >=20 surviving flags. |
| 2026-11-25 | 5-round persisted Grok GTM/ICP consult | Classified Observe static diff as vitamin; flagged mega external-PR OSS firehoses as high-bounce vs. mid-size coding-agent teams. | Park Batch 2 cold mega-OSS outreach (`browser-use`, Tier-1 frameworks). |
| 2026-11-26 | Persisted Claude audit (`f17ec863`) + owner execution + `v0.2.3` bypass fixes | Added `::error`/`::warning` file annotations + check summary title to `proctor-action@v1` under `contents: read`. Replayed GPT-5 `ImpossibleBench` (`AGENT_CHEAT_BENCH.md`): `0/50` FPs on `original`, `55/70` Receipt visibility; fixing `if`/early-return/`pytest.raises` wrapping + fixture skips + `assert_*` helpers lifted `conflicting` flags `14/39 -> 16/39` (`22/70` total) with `0/50` FPs. | Remaining unflagged cases are co-edited linked constants (`sympy`/`django` `== X -> == Y`), confirming Test-Change Receipts + `--verify` + 3 warm partner installs are the next focus before Show HN. |

---

## Kill & Pivot Criteria
- **Bet 1 (`v0.2.3` ImpossibleBench Re-Score — revisit 2026-11-28):** Fixing fixture skips, `if`-wrapping, and `pytest.raises` wrapping must substantially lift `conflicting`/`oneoff` flag rate without triggering false positives on the `50` `original` controls or inflating `V0_1_VS_V0_2_COMPARISON.md` yellow rates.
- **Bet 2 (3 Warm Partners — revisit 2026-12-03):** If <2 of 3 partners keep the hook/Action past Day 7 or say receipts didn't matter, interview directly on workflow friction before Show HN.

---

## Rejected / Parked Ideas (Do Not Re-Suggest Unchanged)
- Cold integration PRs (`proctor.yml`) opened on stranger repos without permission.
- Cold "we scanned your clean PR" pitches to mega-OSS maintainers (`browser-use`, Tier-1 frameworks).
- Multi-week OSS git-history scraping as primary proof (`Revert` / `skip->unskip` noise).
- Check-Run API edits requiring `checks: write` (replaced cleanly by `emit_annotations.py`).
- Product Hunt, awesome-lists, SaaS dashboards before sticky organic usage, pitching `--enforce` blocking as headline.
