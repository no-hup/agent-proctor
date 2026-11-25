# Show HN: Agent-Proctor – Deterministic Test-Change Receipts and base-commit verification for PRs

**GitHub (Core CLI & Engine):** https://github.com/no-hup/agent-proctor  
**GitHub (Action):** https://github.com/no-hup/proctor-action  
**Author:** Shaurya Jaiswal (@no-hup)  
**License:** Apache-2.0 (100% Free & Open Source)

---

## Submission Text

Hey HN,

I built **`agent-proctor`** (`proctor`), a deterministic, zero-egress CLI and GitHub Action written in Rust that solves a practical PR review problem—especially as AI coding agents open more pull requests:

1. **What did this PR actually change in the tests?** `proctor scan` generates a compact **Test-Change Receipt** right in `$GITHUB_STEP_SUMMARY` and SARIF—showing added tests (and whether they assert exact values), modified assertions (strengthened vs. weakened vs. co-evolved), deleted tests, CI/runner config edits, blanket suppressions, and internal mocks.
2. **Do those new/edited tests actually catch a bug on `base`?** Opt-in **`proctor verify`** (or `verify: true` in CI) temporarily runs the PR's added or modified tests against the `base` commit and `head` commit so you know whether a test reproduces a failure (`Catches change`) rather than passing on untouched code.

Everything runs offline under Apache-2.0 with **zero LLMs and zero network dependencies**.

---

### Why Default to Observe Mode (`enforce: false`) + Receipts?

Most developer tools pitch Day-1 CI blocking, and then get uninstalled within a week because static AST rules flag legitimate maintenance (updating an assertion when an upstream model/endpoint changes, cleaning up stale fixtures, adding OS skip guards).

We measured our own engine carefully before calling it "ready":

* Evaluated across **500 commits (`n=100` each)** of `aider-AI/aider`, `princeton-nlp/SWE-agent`, `browser-use/browser-use`, `OpenHands/OpenHands`, and `BerriAI/litellm`, plus a disjoint **100-commit holdout window** on `browser-use` (`skip=100`).
* Full per-commit methodology and CSV labels are checked into the repo: [`docs/audit/V0_1_VS_V0_2_COMPARISON.md`](../audit/V0_1_VS_V0_2_COMPARISON.md).

#### `v0.2.2` Historical Replay Matrix (`proctor scan --base <sha>^ --head <sha>`)

| Repository | Window (`n=100`) | Pass / Review / Block | Yellow (`Review`) Rate | Notes |
| :--- | :--- | :---: | :---: | :--- |
| `aider-AI/aider` | `5b0f6ce9..5dc9490b` | `96 / 0 / 4` | `0.0%` | Clean human & agent baseline |
| `princeton-nlp/SWE-agent` | `d747e536..3ea751c0` | `99 / 1 / 0` | `1.0%` | Clean baseline |
| `OpenHands/OpenHands` | `e1b8b60f..c11b5fc6` | `89 / 7 / 4` | `7.0%` | Down from `56 / 33 / 11` in `v0.1.0` |
| `BerriAI/litellm` | `13101c8a..34dea88f` | `83 / 5 / 12` | `5.0%` | Down from `75 / 13 / 12` in `v0.1.0` |
| `browser-use/browser-use` | `c426bd89..94784c1f` | `70 / 8 / 22` | `8.0%` | Down from `61 / 17 / 22` in `v0.1.0` |
| `browser-use` holdout (`skip=100`) | `commits 101..200` | `72 / 13 / 15` | `13.0%` | Disjoint window |

We also hand-labeled sampled findings (single labeler, one holdout repo), explicitly separating **"fired per AST rule"** from **"actually worth a human reviewer's attention"**:

* **Guard tier (surviving samples, `n=54` primary / `n=28` holdout):** `74.1%` primary (`71.4%` holdout) fired strictly as specified, and **`42.6%` primary (`23/54`) / `46.4%` holdout (`13/28`) warranted reviewer attention** (`ShadowMock` was `13/13` primary and `6/6` holdout; test deletions and untouched-production edits carry the remaining legitimate maintenance noise).
* **Report tier (surviving samples, `n=19` primary / `n=20` holdout):** kept intentionally sensitive on unlinked constant/assertion changes (`5.3%` primary / `0.0%` holdout reviewer attention) so quiet single-value flips in agent PRs don't slip through silently while keeping the yellow rate at `0–13%`.

**Design takeaway:** Roughly half of static Guard flags on real human commits are legitimate maintenance. So **`enforce: false` (Observe Mode) is the default**. Every PR still gets a clear Test-Change Receipt and optional `proctor: ack <id>` / `proctor: accept <id>` reviewer sign-off, without breaking builds—and when you want proof rather than heuristics, **`proctor verify`** executes the changed tests on `base` vs. `head`.

---

### What a Clean Test-Change Receipt Looks Like

Running `proctor scan` on `OpenHands/OpenHands` `#19954` (`d56c1ab`):

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

When something *is* modified, deleted, suppressed (`skip` / `# noqa` / `@ts-ignore`), or loosened in CI (`|| true`, `continue-on-error: true`, pytest/jest config edits), the receipt surfaces a bounded summary table (grouped per file for test deletions, capped at 25 Guard / 10 Report rows) plus clickable SARIF annotations.

---

### How It Works Under the Hood

1. **Tree-sitter AST Lattice (Python, JS/JSX, TS/TSX):** Compares old vs. new test functions across an assertion strength lattice ($\text{Vacuous} < \text{Weak} < \text{Exact}$), detects constant/value flips with equal assertion counts, and catches `pytest.raises` / Jest `toThrow` removals.
2. **Definition-Linkage & Orphan Resolution (`PROCTOR-001`):** Co-evolution where an existing test's linked production symbol changed *without* weakening assertions counts silently as normal co-evolution (`changed without findings` / `strengthened`) instead of spamming reviewers. Deleting an entire test is only clean when every referenced production symbol was removed.
3. **In-Repo Shadow Mock Detection (`PROCTOR-002`):** Allows external boundary mocks (`requests`, DB clients, `os.environ` / `monkeypatch.setenv`), while flagging mocks targeting in-repo symbols defined inside the repository.
4. **Config, CI & Suppression Integrity (`PROCTOR-003`..`008`):** Flags `|| true`, `; exit 0`, `continue-on-error: true`, runner threshold relaxation, skip markers added alongside production edits, and blanket linter/type suppressions.
5. **Opt-In Red/Green Base Verification (`proctor verify`):** Safely swaps changed test targets onto `base` inside `.git/proctor-swap-backup.json` journaled recovery (refuses `verify: true` under `pull_request_target` for safety) and reports whether new tests actually fail on the unfixed base commit.

---

### Security & Zero-Egress Design

* **Zero Network Capabilities:** `proctor` compiles with zero HTTP/network crates (verified by `test_zero_network_dependencies_in_lockfile` in `cargo test`). It cannot phone home or exfiltrate secrets.
* **Least-Privilege CI Permissions:** Runs cleanly with `contents: read` and `pull-requests: read` (plus `security-events: write` only if uploading SARIF).
* **SHA-256 Checksummed Releases:** Official release tarballs ship with `.sha256` digests under `no-hup/agent-proctor/releases`.

---

### Try It Locally

```bash
curl -sSfL https://github.com/no-hup/agent-proctor/releases/latest/download/proctor-$(uname -s | tr '[:upper:]' '[:lower:]')-$(uname -m).tar.gz | tar -xz && ./proctor audit --limit 50
```

Or drop Observe Mode into GitHub Actions:

```yaml
name: Test-Change Receipt (Observe Mode)
on: [pull_request]

jobs:
  proctor:
    runs-on: ubuntu-latest
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

Apache-2.0 licensed. Would love feedback on where the AST lattice misses sneaky agent workarounds—or where test-change receipts save you review cycles.
