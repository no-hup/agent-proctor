# Tier 1 Open Source Maintainer Outreach Playbook

This playbook provides targeted, respectful, engineer-to-engineer outreach templates for non-AI open source maintainers currently experiencing drive-by AI PR fatigue.

---

## The Value Proposition to Maintainers

1. **The Core Pain:** Maintainers waste 20–30 minutes reviewing AI-assisted PRs that appear green in CI, only to find the author/agent commented out failing tests or weakened assertions.
2. **The Zero-Risk Test:** Offer to run Proctor in **Shadow / Advisory Mode** (`strict: false` or advisory comment). It never blocks builds; it just flags assertion modifications.
3. **Privacy & Freedom:** 100% Free for Open Source, zero LLM calls, zero telemetry (binary executes entirely on their GitHub runner).

---

## Outreach Templates

### Template A: Twitter/X / DM / Email (Short, High-Converting)

> **Subject:** Quick anticheat action to block AI PRs that delete tests
>
> Hey [Name],
>
> Saw your post about the flood of low-effort AI PRs hitting [Repo Name]. One recurring pattern we noticed across public repos is agents getting green CI by silently weakening assertions or deleting test cases when they hit a tough edge case.
>
> We built `proctor`—a deterministic, sub-50ms Tree-sitter GitHub Action that catches assertion weakenings and test tampering in git diffs (zero LLMs, zero telemetry, runs locally on your runner).
>
> It’s free for open source. If you're open to it, could we set it up in **advisory/shadow mode** on [Repo Name] for 5 days? It won't fail any PRs; it just leaves a quiet annotation if an agent modifies your test harness.
>
> Happy to send over the 5-line YAML if you'd like to try it.
>
> Best,  
> Shaurya

---

### Template B: GitHub Discussion / Issue Note

> **Title:** Deterministic protection against AI PRs that weaken test assertions
>
> Hi [Maintainer Name],
>
> Like many popular repositories, [Repo Name] has likely seen an increase in AI-assisted PRs that claim to fix an issue but secretly tweak tests to pass CI.
>
> We built `agent-proctor` to solve this at the git PR boundary:
> * Uses Tree-sitter AST diffing to detect assertion weakening, test case deletion, shadow mocks, and CI sabotage (`|| true`).
> * Sub-50ms execution, runs 100% locally on your GitHub runner (zero LLMs, zero telemetry).
> * Free for all open-source projects.
>
> You can drop it into `.github/workflows/proctor.yml` in advisory mode:
> ```yaml
> - uses: no-hup/proctor-action@v1
>   with:
>     base-ref: ${{ github.event.pull_request.base.sha }}
>     head-ref: ${{ github.event.pull_request.head.sha }}
> ```
> Under default mode, it only emits advisory warnings and will not block human contributors.
>
> Let us know if you find it helpful or if you'd like any custom rules for [Repo Name]!

---

## Target Candidates (Non-AI Repos Flooded with PRs)

| Project | Maintainer | Primary Language | Why They Are Ideal |
| :--- | :--- | :--- | :--- |
| **Textual / Rich** | Will McGugan | Python | Highly vocal about AI PR review fatigue; rigorous test suite. |
| **Pydantic** | Samuel Colvin / Team | Python | Massive volume of community PRs; strict type-driven testing. |
| **FastAPI** | Sebastián Ramírez (tiangolo) | Python | Hundreds of drive-by PRs; high community engagement. |
| **Celery** | Omer Katz / Project team | Python | Complex distributed test suite; often broken by subtle test skips. |
| **HTTPX / Encode** | Tom Christie / Florimond | Python | High reliability standards; zero tolerance for weakened assertions. |
| **Zod / tRPC** | Colin McDonnell / Alex | TypeScript | Strict type assertions; AI PRs frequently relax checks. |
