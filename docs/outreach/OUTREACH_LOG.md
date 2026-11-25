# agent-proctor Maintainer Outreach Log

> **Long-term strategy & experiment notebook:** Read and update [GROWTH_SCRATCHBOOK.md](GROWTH_SCRATCHBOOK.md) before proposing any new outreach, changing GTM copy, or running growth experiments.

Tracks high-converting, reproducible-proof outreach to open-source maintainers.

## Active Outreach Pipeline

| Date | Maintainer | Project | Contact | Gmail ID | Baseline Audited | Status |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| 2026-11-17 | Paul Gauthier | `aider-AI/aider` | `paul@aider.chat` | `1a1e63701b76da5a` | 100 commits (94% clean, 6% spec mod) | SENT |
| 2026-11-17 | Kilian Lieret | `princeton-nlp/SWE-agent` | `kilian.lieret@posteo.de` | `1a1e6371d1f41a52` | 100 commits (98% clean, 0% un-orphaned) | SENT |
| 2026-11-17 | Graham Neubig | `OpenHands/OpenHands` | `neubig@gmail.com` | `1a1e66db099b4a69` | 99 commits (65% clean, 11% test mod) | SENT |
| 2026-11-17 | Krrish Dholakia | `BerriAI/litellm` | `krrish@berri.ai` | `1a1e66dc6434541e` | 99 commits (56% clean, 21% test mod) | SENT |

## Calibrated Follow-Ups & Batch 2 (`v0.2.2`)

Prepared in [BATCH_2_DRAFTS.md](BATCH_2_DRAFTS.md) — **awaiting explicit user approval before sending**:
- Follow-up to Graham Neubig (`OpenHands/OpenHands`, `neubig@gmail.com`) — thread `1a1e66db099b4a69` (`DRAFT`)
- Follow-up to Krrish Dholakia (`BerriAI/litellm`, `krrish@berri.ai`) — thread `1a1e66dc6434541e` (`DRAFT`)
- Batch 2 value-first draft for Gregor Žunič (`browser-use/browser-use`, `@gregpr07`) (`DRAFT`)

## Outreach Guidelines & Invariants
1. **Never accuse maintainers of bad tests**: Always lead with their clean baseline (e.g. "94% clean", "98% clean").
2. **Zero unsolicited issue/PR filings**: Reach out via direct email or Twitter/X DM only.
3. **Always provide reproducible local proof**: Maintainers can run `curl -sSfL ... | tar -xz && ./proctor audit --limit 100` locally in <3s with zero telemetry.
4. **Observe Mode by default**: Always pitch `enforce: false` so it adds value to `$GITHUB_STEP_SUMMARY` without blocking CI builds.
