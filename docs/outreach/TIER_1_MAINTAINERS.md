# Tier 1 Open Source Maintainer Outreach Playbook

This playbook provides respectful, engineer-to-engineer outreach templates built around **Observe Mode Test-Change Receipts** and opt-in **`proctor verify`**.

---

## Core Outreach Invariants

1. **Lead with a concrete, flattering PR receipt:** Run `v0.2.2` against recent merged PRs from their repo and attach a real green `Test-Change Receipt` (never lead by accusing a maintainer of weak tests).
2. **Pitch Observe Mode (`enforce: false`) first:** Static AST rules still catch legitimate test maintenance (~43–46% Guard reviewer attention on historical human commits), so CI blocking should always be opt-in.
3. **Zero unsolicited PRs or issues:** Reach out via direct email or X/Twitter DM first.
4. **Honest & verifiable:** Every release tarball under `no-hup/agent-proctor/releases` ships with a `.sha256` checksum, zero network libraries (`test_zero_network_dependencies_in_lockfile`), and read-only permissions.

---

## Outreach Templates

### Template A: Follow-Up Calibration Note (Previous Early Outreach)

> **Subject:** fixed the noisy TSX / context-manager flags in agent-proctor (v0.2.2)
>
> Hey [Name],
>
> Quick follow-up—earlier I shared `v0.1.0` audit numbers for `[Repo]` that turned out to have a lot of false positives (our TypeScript `.tsx` parser choked on `typeof import(...)` and Python context managers like `with wire_server():` triggered assertion alerts).
>
> We fixed those parsers and added definition-linkage calibration in **`v0.2.2`**, plus shifted the core pitch from "auto-blocking CI" to **Test-Change Receipts** in GitHub Step Summaries + opt-in `proctor verify` (running added tests against `base` to confirm reproduction):
>
> [Paste real clean v0.2.2 receipt from a recent merged PR in their repo]
>
> Re-running the same 100-commit replay on `[Repo]` moved clean passes from `[old]` -> **`[new]`** (full methodology + hand-labeled precision numbers—including where static heuristics still flag ~43–46% legitimate maintenance so `enforce: false` stays default—are in `docs/audit/V0_1_VS_V0_2_COMPARISON.md`).
>
> Happy to share the 5-line Observe Mode YAML if you'd like automatic PR receipts on `$GITHUB_STEP_SUMMARY`, otherwise won't bug you again!
>
> Best,  
> Shaurya  
> https://github.com/no-hup/agent-proctor

---

### Template B: Cold Value-First Outreach (Direct Email / X DM)

> **Subject:** Test-Change Receipts for `[Repo]` PRs (zero-egress, non-blocking)
>
> Hey [Name],
>
> With AI coding PRs ramping up across open-source repos, one annoying time-sink in review is figuring out *what* a PR changed inside `tests/`—whether new tests assert exact values, existing assertions got weakened, or internal code got mocked out.
>
> I built **`agent-proctor`** (Apache-2.0, Rust, zero network libraries): it posts a compact **Test-Change Receipt** to `$GITHUB_STEP_SUMMARY` in **Observe Mode (`enforce: false`)** so builds never block, plus an optional `proctor verify` mode that executes new/edited tests on `base` to prove they actually fail before the fix.
>
> Ran `v0.2.2` against recent merged commits on `[Repo]` (e.g., `[commit/PR]`):
>
> [Paste clean v0.2.2 Test-Change Receipt]
>
> If adding an automatic summary table to PRs sounds useful, it's 5 lines via `no-hup/proctor-action@v1` (read-only permissions, runs completely locally on your runner). Happy to share details or stay out of your hair—either way, congrats on `[recent milestone]`.
>
> Best,  
> Shaurya  
> https://github.com/no-hup/agent-proctor

---

## Target Candidates (Python / TypeScript Repos)

| Project | Maintainer | Primary Language | Why They Are Relevant |
| :--- | :--- | :--- | :--- |
| **browser-use** | Gregor Žunič (`@gregpr07`) | Python | High PR velocity (`9afbc07e` clean receipt verified) |
| **Textual / Rich** | Will McGugan | Python | Rigorous test suite; vocal about PR review load |
| **Pydantic** | Samuel Colvin / Team | Python | Large community PR volume; strict type & behavior tests |
| **FastAPI** | Sebastián Ramírez (`tiangolo`) | Python | High volume of community & agent-assisted PRs |
| **HTTPX / Encode** | Tom Christie / Team | Python | Strict contract testing |
| **Zod / tRPC** | Colin McDonnell / Alex | TypeScript | `.tsx`/TS contract assertions |
