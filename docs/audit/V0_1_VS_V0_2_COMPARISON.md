# Comparative Audit Report: Proctor v0.1.0 vs. v0.2.1 & Calibrated v0.2.2

*Historical per-commit replay of `proctor scan --base <sha>^ --head <sha> --format json` using [scripts/compare_audits_per_commit.py](../../scripts/compare_audits_per_commit.py) over identical 100-commit windows across 5 open-source repositories (500 primary commits) plus a disjoint 100-commit `browser-use` holdout window (`skip=100`, 100 commits). Evaluated binaries: `v0.1.0` (`e87f146`), `v0.2.1` (`760411d`), and calibrated `v0.2.2` (Track B release candidate).*

> **Status: verified & hand-labeled.** Every delta was diffed by stable Blake3 finding ID (`id`). Labels are counted per `(commit, finding_id)` by a single labeler across one holdout repo: `140` sampled `v0.2.1` findings (`125` unique IDs; `14` IDs recur across commits/merges) in [labels_v0_2_1.csv](labels_v0_2_1.csv), plus `50` disjoint holdout findings (`browser-use skip=100`, `48` surviving under `v0.2.2`) in [labels_v0_2_2_holdout.csv](labels_v0_2_2_holdout.csv). Both datasets separate **fired as specified** from **worth a reviewer's attention** and avoid pooling skewed strata into headline precision claims.

---

## 1. Benchmark Matrix

### Primary Window (500 Identical Commits, `skip=0`)

| Repository | Commit Window (`n=100`) | `v0.1.0` Pass / Review / Block | `v0.2.1` Pass / Review / Block | `v0.2.2` Pass / Review / Block | `v0.2.2` Yellow % |
| :--- | :--- | :---: | :---: | :---: | :---: |
| `aider-AI/aider` | `5b0f6ce9..5dc9490b` | `94 / 0 / 6` | `94 / 0 / 6` | **`96 / 0 / 4`** | `0.0%` |
| `princeton-nlp/SWE-agent` | `d747e536..3ea751c0` | `98 / 2 / 0` | `98 / 2 / 0` | **`99 / 1 / 0`** | `1.0%` |
| `browser-use/browser-use` | `c426bd89..94784c1f` | `61 / 17 / 22` | `61 / 15 / 24` | **`70 / 8 / 22`** | `8.0%` |
| `OpenHands/OpenHands` | `e1b8b60f..c11b5fc6` | `56 / 33 / 11` | `56 / 37 / 7` | **`89 / 7 / 4`** | **`7.0%`** |
| `BerriAI/litellm` | `13101c8a..34dea88f` | `75 / 13 / 12` | `75 / 13 / 12` | **`83 / 5 / 12`** | **`5.0%`** |

### Disjoint Holdout Window (`browser-use/browser-use`, `skip=100`, `n=100`)

| Cohort (`commits 101..200`) | `v0.1.0` Pass / Review / Block | `v0.2.2` Pass / Review / Block | `v0.1.0` Rules | `v0.2.2` Rules | `v0.2.2` Yellow % |
| :--- | :---: | :---: | :--- | :--- | :---: |
| `browser-use` holdout (`n=100`) | `64 / 15 / 21` | **`72 / 13 / 15`** | `AssertionLock: 191`, `ShadowMock: 14` | `AssertionLock: 906`, `ShadowMock: 6` | **`13.0%`** |

*(The `191 -> 906` `AssertionLock` jump in the holdout window reflects `+797` finding IDs added in a single v1.0 rewrite commit, `b02c5225`, which deleted `0.x` tests under the stricter `.all()` `body_symbols` orphan rule (`872` total deleted-test findings including ones already flagged by `v0.1.0`, grouped per file and capped at 25 rows in the receipt). On the remaining 99 holdout commits, `v0.2.2` reduced both blocks (`20 -> 14`) and `ShadowMock` findings (`14 -> 6`).)*

### Findings by Rule & Finding-ID Deltas (Primary Window)

| Repository | Rule | `v0.1.0` | `v0.2.1` | `v0.2.2` | ID Diff (`v0.1.0` -> `v0.2.1`) | ID Diff (`v0.1.0` -> `v0.2.2`) |
| :--- | :--- | :---: | :---: | :---: | :--- | :--- |
| `aider` | `PROCTOR-001` (`AssertionLock`) | `11` | `11` | `7` | `+0 / -0 / ~0` | `+0 / -4 / ~0` |
| `SWE-agent` | `PROCTOR-001` (`AssertionLock`) | `2` | `2` | `1` | `+0 / -0 / ~0` | `+0 / -1 / ~0` |
| `browser-use` | `PROCTOR-001` (`AssertionLock`) | `128` | `131` | `60` | `+3 / -0 / ~2` | `+1 / -69 / ~2` |
| `browser-use` | `PROCTOR-002` (`ShadowMock`) | `16` | `14` | `9` | `+0 / -2 / ~0` | `+0 / -7 / ~0` |
| `OpenHands` | `PROCTOR-001` (`AssertionLock`) | `242` | `242` | `19` | `+0 / -0 / ~0` | `+2 / -225 / ~2` |
| `OpenHands` | `PROCTOR-002` (`ShadowMock`) | `33` | `24` | `8` | `+1 / -10 / ~0` | `+0 / -25 / ~0` |
| `litellm` | `PROCTOR-001` (`AssertionLock`) | `145` | `145` | `83` | `+0 / -0 / ~0` | `+1 / -63 / ~0` |

---

## 2. Finding-ID Delta Root Causes (`v0.1.0` vs. `v0.2.1`)

1. **`litellm` `+201 PROCTOR-001` and `+3 PROCTOR-002` (`224d2c4` prelim):** Not reproducible on the new Nov 25 `n=100` window (`13101c8a..34dea88f`: `+0 / -0 / ~0` between `v0.1.0` and `v0.2.1`, exact `145` vs. `145` `AssertionLock`, `0` `ShadowMock`); original Nov 12 `n=49` window on `224d2c4` was not re-run. On the `browser-use` holdout window (`b02c5225`), switching whole-test deletion from `.any()` assertions to `.all()` `body_symbols` shows how a large rewrite triggers `+797` `deleted_test` findings when deleted legacy tests reference surviving facade symbols.
2. **`OpenHands` `PROCTOR-002` (`33 -> 24`, `+1 / -10`):** All `10` removed IDs (`0f8fc5465919`, `ea6524f3400e`, `78eead450258` in `d56c1ab8`; `e7c02c2fb5f3`, `da99da41429a`, `b68097cec944` in `7a9425ac`; `2d4e5e5a919f` in `7dffeee6`; `ca8901908b32`, `ec4130582404` in `bb526a3b`; `ca8901908b32` in `e66610e2`) were `vi.mock("#/...")` harness findings caused in `v0.1.0` by extracting nested function locals into `in_repo_symbols` (`!in_function` in `extract_js_source_symbols` fixed these substring collisions). The `1` added ID (`ec4130582404` in `ebae4147`) was `vi.mock("#/hooks/use-conversation-id")`.
3. **`browser-use` (`+3 / -2 / ~2`):**
   - **2 removed:** `017dc63db55b` (`XDG_CURRENT_DESKTOP`) and `71c6b684eb0f` (`XDG_SESSION_TYPE`) in `c3596955` (`monkeypatch.setenv` FPs removed).
   - **2 tier-changed (`Report -> Guard`):** `e5c9ecb45ece` (`bd1581cf`) and `efd53fe097e4` (`6a3462ca`) in `tests/ci/test_judge.py`, both confirmed refactors/compound checks (`FP`).
   - **3 added:** `d29256b3536e` (`1322c38f`), `be438aa60b40` (`7fdd63c0`, `83211646`).

---

## 3. Labeled Accuracy (Per-Stratum & Holdout)

Two accuracy definitions are reported separately:
- **Fired as specified:** the finding accurately matched the rule's AST definition (e.g., an assertion truly mutated with zero production edits in `aider`/`litellm`, or an in-repo module was mocked).
- **Worth a reviewer's attention:** excludes legitimate human maintenance that matches the AST invariant (`test_only_assertion_edit`/`drop` on model deprecations or flakes, `orphaned_doc_cleanup`, and platform `skipif` guards).

### A. Primary Window Stratified Labels ([labels_v0_2_1.csv](labels_v0_2_1.csv), `n=140` -> `73` surviving in `v0.2.2`)

*(Report-tier strata were over-sampled in `v0.2.1` to diagnose noise, so per-stratum counts should be read directly rather than pooled into an unweighted headline number.)*

| Stratum | `v0.2.1` `n` | `v0.2.1` Fired as Spec | `v0.2.1` Reviewer Attn | `v0.2.2` `n` | `v0.2.2` Fired as Spec | `v0.2.2` Reviewer Attn |
| :--- | :---: | :---: | :---: | :---: | :---: | :---: |
| `aider::AssertionLock::untouched_guard` | `11` | `11/11` (`100%`) | `0/11` (`0%`) | `7` | `7/7` (`100%`) | `0/7` (`0%`) |
| `browser-use::AssertionLock::untouched_guard` | `7` | `6/7` | `0/7` | `7` | `6/7` | `0/7` |
| `litellm::AssertionLock::untouched_guard` | `8` | `8/8` | `0/8` | `4` | `4/4` | `0/4` |
| `OpenHands::AssertionLock::deleted_test_guard` | `5` | `3/5` | `3/5` | `4` | `3/4` | `3/4` |
| `browser-use::AssertionLock::deleted_test_guard` | `8` | `6/8` | `6/8` | `8` | `6/8` | `6/8` |
| `litellm::AssertionLock::deleted_test_guard` | `8` | `1/8` | `1/8` | `8` | `1/8` | `1/8` |
| `browser-use::AssertionLock::shifted_constant_guard` | `1` | `0/1` (`n=1`) | `0/1` | `1` | `0/1` (`n=1`) | `0/1` |
| `browser-use::AssertionLock::unlinked_weakened_guard` | `2` | `0/2` (`n=2`) | `0/2` | `2` | `0/2` (`n=2`) | `0/2` |
| `OpenHands::ShadowMock::guard` | `12` | `9/12` (`75%`) | `9/12` (`75%`) | `4` | `4/4` | `4/4` |
| `browser-use::ShadowMock::guard` | `14` | `9/14` (`64%`) | `9/14` (`64%`) | `9` | `9/9` | `9/9` |
| `browser-use::AssertionLock::linked_weakened_report` | `2` | `1/2` (`n=2`) | `1/2` (`n=2`) | `2` | `1/2` (`n=2`) | `1/2` (`n=2`) |
| `OpenHands::AssertionLock::mod_report` | `12` | `0/12` (`0%`) | `0/12` | `3` (unlinked) | `0/3` | `0/3` |
| `SWE-agent::AssertionLock::mod_report` | `2` | `0/2` (`n=2`) | `0/2` | `1` (unlinked) | `0/1` (`n=1`) | `0/1` |
| `browser-use::AssertionLock::mod_report` | `20` | `0/20` (`0%`) | `0/20` | `3` (unlinked) | `0/3` | `0/3` |
| `litellm::AssertionLock::mod_report` | `18` | `0/18` (`0%`) | `0/18` | `10` (unlinked) | `0/10` (`0%`) | `0/10` |
| `OpenHands::SyntaxError` (`.tsx`) | `6` | `0/6` | `0/6` | `0` | — | — |
| `litellm::SyntaxError` (`.tsx`) | `4` | `0/4` | `0/4` | `0` | — | — |
| **Surviving Sample Totals (`v0.2.2`, `n=73`)** | — | — | — | `54 Guard` / `19 Report` | **`40/54` (`74.1%`) Guard** / **`1/19` (`5.3%`) Report** | **`23/54` (`42.6%`) Guard** / **`1/19` (`5.3%`) Report** |

**`mod_report` linkage split (`52` primary labels):** `32/32` linked non-weakened items were benign co-evolution or strengthened assertions (`0/32` TP); `20/20` unlinked non-weakened items in human commits were integration/constant co-evolution (`3` dropped by the Python `with` fix, `17/17` remaining FPs), kept at `Verdict::Review` to avoid reopening Grok's loophole 1 on agent value-flip cheats.

### B. Disjoint Holdout Window Labels ([labels_v0_2_2_holdout.csv](labels_v0_2_2_holdout.csv), `browser-use skip=100`, `48` surviving of `50` sampled)

| Holdout Slice | `Guard` Fired as Spec | `Guard` Reviewer Attn | `Report` Fired as Spec | `Report` Reviewer Attn | `ShadowMock::guard` |
| :--- | :---: | :---: | :---: | :---: | :---: |
| Surviving sampled items (`n=48`) | `20/28` (`71.4%`) | **`13/28` (`46.4%`)** | `3/20` (`15.0%`) | `0/20` (`0.0%`) | `6/6` (`100%`) |

*(Takeaway for deployment: across both primary (`42.6%`) and holdout (`46.4%`) windows, roughly half of Guard findings and nearly all human-commit Report findings are non-actionable noise, reinforcing **observe mode** + Test-Change Receipts + `--verify` rather than default `enforce` blocking.)*

---

## 4. Calibrated Changes Implemented for `v0.2.2`

1. **`.tsx` & `typeof import(...)` parser fix** (`crates/proctor-parse/src/javascript.rs`): uses `tree_sitter_typescript::language_tsx()` for `.tsx` files and filters `tree-sitter-typescript 0.21`'s `typeof import("...")` type-query `ERROR` nodes while failing closed on real syntax errors (`0` false parse errors remaining across 500 commits).
2. **Python `with`-block & mock parser fixes** (`crates/proctor-parse/src/python.rs`):
   - `parse_pytest_raises` now inspects the `with`-clause header for `pytest.raises`, `pytest.warns`, `assertRaises`, or `assertWarns` instead of treating arbitrary context managers (`with wire_server(...):`, `with GitTemporaryDirectory():`) as assertions (cut `aider` false blocks `6 -> 4` and `litellm` findings `104 -> 83`).
   - `extract_mock_call` excludes `monkeypatch.setenv`/`delenv`/`chdir`/`syspath_prepend` and `patch.dict(os.environ, ...)` while matching bare `patch.object`, `patch.dict`, and `patch.multiple` (`ShadowMock::guard` deduped per test/target in `proctor-rules/src/lib.rs`).
3. **Linked co-evolution calibration** (`crates/proctor-rules/src/lib.rs`, `corpus/python/linked_behavior_change`, `corpus/attack/unrelated_prod_touch_value_swap`): `!was_weakened && linked` surfaces as informational counts (`changed without findings` / `strengthened`) in `Test-Change Receipt`, while `!was_weakened && !linked` remains `Verdict::Review` and `was_weakened && linked` stays `Verdict::Review` (`linked_behavior_weakened`).
4. **Receipt & CLI UX fixes** (`crates/proctor-cli/src/reporter.rs`): neutral observe-mode wording (`would block under --enforce`, no accusations), per-kind titles (`finding_display_title`), grouped `deleted_test` Guard rows per file and capped at 25 Guard / 10 Report rows across both markdown receipts and pretty CLI output, deduplicated paths in details, `—` for zero modified/deleted rows, and inline `proctor: ack <id>` / `proctor: accept <id>` instructions.
