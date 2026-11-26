# Track D GTM Assets (`v0.2.3`)

Ready-to-use copy aligned with [GROWTH_SCRATCHBOOK.md](../outreach/GROWTH_SCRATCHBOOK.md). Never send or post externally without Shaurya's review.

---

## 1. Warm Design-Partner DM (Send to 3 Claude Code / Cursor Developers)

**Goal:** 15-minute setup of `no-hup/proctor-action@v1` (`enforce: false`) or `proctor-hook.sh`, followed by a Day-7 two-question check-in.

```text
Hey [Name] — I built a tiny zero-egress Rust CLI/Action (`agent-proctor` v0.2.3) that drops a 5-line Test-Change Receipt into PRs (showing new tests, `old -> new` assertion edits, fixture skips, and CI loosening in non-blocking Observe Mode) + an optional `proctor verify` pass to check whether new tests actually fail on `main`.

Since you're merging a lot of Claude Code / Cursor PRs, would you be down to let me drop the 5-line read-only workflow (or local stop-hook) onto one repo for a week? Zero LLM calls, never blocks CI, and I'll just ask you on Day 7 whether you read a receipt or caught anything useful.
```

**5-line Action snippet to send if they say yes:**

```yaml
- uses: no-hup/proctor-action@v1
  with:
    base-ref: ${{ github.event.pull_request.base.sha }}
    head-ref: ${{ github.event.pull_request.head.sha }}
    enforce: false
```

*(For autonomous agent loops/evals, add `protect-existing-tests: true` or run `proctor scan --protect-existing-tests`.)*

**Day-7 Check-in Message (`2026-12-03`):**
> Quick Day-7 check on `proctor`: (1) Did you read a receipt or annotation this week? (2) Did it change anything you did (or annoy you anywhere)?

---

## 2. Technical X / Blog Thread (GPT-5 ImpossibleBench Findings — Eval & Agent Wedge)

**Tweet / Post 1 (Hook + Ground Truth):**
```text
We replayed 120 real GPT-5 trajectories from ImpossibleBench through `agent-proctor` (deterministic Rust AST diffing, zero LLMs) to test what frontier-model test tampering looks like vs. 50 clean SWE-bench solves.

The numbers were humbling—and exposed a real blind spot in naive `verify` checks. 🧵
```

**Post 2 (The 31% Ceiling):**
```text
On 50 clean GPT-5 runs: 0/50 false flags (0% FP).
On 70 net test-cheating diffs (conflicting + oneoff):
- Test-Change Receipts surfaced 55/70 (78.6%, 96.8% on oneoff).
- Unassisted static Guard+Report rules only flagged 22/70 (31.4%).

Why? GPT-5 almost never writes `assert True`. It edits production code AND swaps expected constants (`Mod(5,3)==1 -> ==2`), wraps assertions in `if` guards, or hides `pytest.skip` inside fixtures.
```

**Post 3 (Why Static Diffing Hits a Wall):**
```text
Without knowing task intent, `edit mod() + assert Mod(5,3) == 1 -> == 2` is the EXACT same git diff whether a human updated a spec or an agent cheated.

Tuning static rules to flag every linked constant edit destroys signal on human PRs (0/32 true positives in our human co-evolution audit).
```

**Post 4 (The `verify` Blind Spot + Deterministic Fixes in `v0.2.3`):**
```text
Worse, naive red/green verification makes this blind spot invisible:
If you only run the edited test (`== 2`) on `base` (`== 1`), it fails on `base` and passes on `head`—so `verify` would literally vouch "Catches change" for a cheat!

In `v0.2.3`, we added three deterministic fixes (still 0 LLMs):
1. Reverse `verify` check: runs the *base* test on *head* code and flags `Changed passing contract`.
2. `--protect-existing-tests` Policy Mode for agent loops/evals (locks base assertions as spec: 48.6% Guard+Report recall, 0/50 FPs on clean solves).
3. Paired `old -> new` assertion diffs + `if`/fixture-skip detection in receipts.

Repo + reproducible benchmark scripts (Apache-2.0): https://github.com/no-hup/agent-proctor
```
