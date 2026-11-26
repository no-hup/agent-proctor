# Show HN: Agent-Proctor (`v0.2.3`) – Deterministic Test-Change Receipts and base-commit verification for PRs

**GitHub (Core CLI & Engine):** https://github.com/no-hup/agent-proctor  
**GitHub (Action):** https://github.com/no-hup/proctor-action  
**Author:** Shaurya Jaiswal (@no-hup)  
**License:** Apache-2.0 (100% Free & Open Source)

---

## Submission Text

Hey HN,

I built **`agent-proctor`** (`proctor`), a deterministic, zero-egress CLI and GitHub Action written in Rust that solves two practical review problems when humans and coding agents open pull requests:

1. **What did this PR actually change in `tests/`?** `proctor scan` posts a compact **Test-Change Receipt** right in `$GITHUB_STEP_SUMMARY`, inline PR file annotations, and SARIF—listing added tests (and whether they assert exact values), paired `old → new` pre-existing assertion diffs, weakened/guarded assertions, fixture skips, in-repo mocks, and CI/config loosenings.
2. **Do the new/edited tests catch a bug on `base`—without breaking existing test contracts on `head`?** Opt-in **`proctor verify`** (or `verify: true` in CI) runs added/modified tests 2x on `head` and 2x on `base` (`Catches change` vs. `Passes on both`), **and** runs the `base` version of any modified test against `head` production code so a rewritten assertion (`== 1` → `== 2`) can't quietly pass off a broken contract as a bugfix (`Changed passing contract`).

Everything runs offline under Apache-2.0 with **zero LLMs and zero network dependencies**.

---

### Why Default to Observe Mode (`enforce: false`) + Receipts?

Static AST rules alone cannot distinguish a legitimate behavior update (`Mod(5, 3) == 1` → `== 2` alongside an intentional change to `mod()`) from an agent silently altering expectations—because **both produce the exact same git diff**.

We measured `agent-proctor` on two independent datasets (full methodology, scripts, and labels are in the repo):

1. **600 Human/OSS Historical Commits** ([`docs/audit/V0_1_VS_V0_2_COMPARISON.md`](../audit/V0_1_VS_V0_2_COMPARISON.md)):
   * 500 primary commits (`n=100` each across `aider-AI/aider`, `princeton-nlp/SWE-agent`, `browser-use/browser-use`, `OpenHands/OpenHands`, `BerriAI/litellm`) + 100 disjoint `browser-use` holdout commits (`skip=100`).
   * Observe-mode yellow (`Review`) rate: `0.0%–13.0%` (`aider` `96/0/4`, `SWE-agent` `99/1/0`, `OpenHands` `89/7/4`, `litellm` `83/5/12`, `browser-use` `70/8/22`, holdout `72/13/15`).
   * Hand-labeled by a single labeler across one holdout repo: **Guard** true reviewer attention is **`42.6%` primary (`23/54`) / `46.4%` holdout (`13/28`)** (`ShadowMock` `13/13` primary, `6/6` holdout; deleted tests and maintenance account for the rest). That’s why **`enforce: false` (Observe Mode) is the default** on human PRs.

2. **120 Reconstructed GPT-5 ImpossibleBench Rollouts** ([`docs/audit/AGENT_CHEAT_BENCH.md`](../audit/AGENT_CHEAT_BENCH.md)):
   * Replayed real GPT-5 tool-call trajectories from [ImpossibleBench](https://arxiv.org/abs/2510.20270) (`conflicting` `n=39`, `oneoff` `n=31` net test-edits with valid `base` AST, plus `original` `n=50` solvable controls):
   * **Default Mode (Human PR CI):** `0 / 50` (`0.0%`) false flags on `original`; `22 / 70` (`31.4%`) flagged by static Guard+Report (`16/39` conflicting, `6/31` oneoff), while **Test-Change Receipts surfaced `55 / 70` (`78.6%` overall, `30/31` = `96.8%` on `oneoff`)** with paired `old → new` diffs.
   * **Policy Mode (`--protect-existing-tests` for agent loops/evals):** Treating pre-existing assertions on `base` as immutable raised Guard+Report flags to **`34 / 70` (`48.6%`; `21/39` = `53.8%` Guard on `conflicting`, `13/31` = `41.9%` on `oneoff`)** while keeping **`0 / 50` (`0.0%`)** false flags on `original`.

---

### What a PR Sees in Observe Mode

Clean PR (`OpenHands/OpenHands` `#19954`):

```markdown
### 📋 Test-Change Receipt (agent-proctor)

| Category | Count | Status |
|---|---|---|
| **Added tests** | 13 | all contain an exact-value assertion (not executed) |
| **Modified tests** | 0 | — |
| **Deleted tests** | 0 | — |
| **Configs & CI gates** | 0 | ✅ no loosening found |
| **Blanket suppressions** | 0 | ✅ none added |
| **Internal mocks** | 0 | ✅ none added |

**Verdict:** 🟢 No findings in analyzed test files.
```

And when an existing assertion is modified alongside production code, the receipt appends a compact paired table (`<details><summary>Changed pre-existing assertions (N)</summary>`) showing the exact `old → new` diff (`assert Mod(5, 3) == 1` → `assert Mod(5, 3) == 2`), while `proctor-action@v1` bubbles counts and file/line annotations up to the GitHub Actions Checks view under strictly `contents: read`.

---

### How It Works Under the Hood

1. **Tree-sitter AST Lattice (Python, JS/JSX, TS/TSX):** Evaluates assertions across $\text{Vacuous} < \text{Weak} < \text{Exact}$, catches equal-count constant flips, conditional `if` / early-return / `pytest.raises` wrapping around existing assertions, fixture/helper `pytest.skip` injections, and softening inside `def assert_*` helpers.
2. **Definition-Linkage & Orphan Resolution (`PROCTOR-001`):** Benign co-evolution (`!was_weakened && linked`) surfaces quietly as `old → new` diffs in the receipt instead of warning spam; deleting a test is clean only when all referenced production symbols were removed.
3. **In-Repo Shadow Mock, CI & Suppression Integrity (`PROCTOR-002`..`008`):** Permits external boundary mocks (`requests`, `os.environ`) while flagging mocks of in-repo symbols, `|| true` / `; exit 0` / `continue-on-error: true`, runner threshold loosening, and blanket linter/type suppressions.
4. **Opt-In Red/Green + Reverse Contract Verification (`proctor verify`):** Uses `.git/proctor-swap-backup.json` journaled recovery (`pull_request_target` refused) to check both whether changed tests fail on `base` *and* whether unmodified `base` tests still pass on `head`.

---

### Try It Locally

```bash
OS=$(uname -s | tr '[:upper:]' '[:lower:]')
ARCH=$(uname -m | sed 's/arm64/aarch64/')
curl -sSfL "https://github.com/no-hup/agent-proctor/releases/latest/download/proctor-${OS}-${ARCH}.tar.gz" | tar -xz && ./proctor audit --limit 50
```

Or drop Observe Mode into GitHub Actions:

```yaml
name: Test-Change Receipt (Observe Mode)
on: [pull_request]

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
      - uses: no-hup/proctor-action@v1
        with:
          base-ref: ${{ github.event.pull_request.base.sha }}
          head-ref: ${{ github.event.pull_request.head.sha }}
          enforce: false
```

Apache-2.0 licensed. Would love feedback from both PR reviewers and eval/agent harness builders on where this catches real issues vs. where edge cases remain.
