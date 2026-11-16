# agent-proctor Maintainer Outreach Log

Tracks high-converting, reproducible-proof outreach to open-source maintainers.

## Active Outreach Pipeline

| Date | Maintainer | Project | Contact | Gmail ID | Baseline Audited | Status |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| 2026-11-17 | Paul Gauthier | `aider-AI/aider` | `paul@aider.chat` | `1a1e63701b76da5a` | 100 commits (94% clean, 6% spec mod) | SENT |
| 2026-11-17 | Kilian Lieret | `princeton-nlp/SWE-agent` | `kilian.lieret@posteo.de` | `1a1e6371d1f41a52` | 100 commits (98% clean, 0% un-orphaned) | SENT |

## Outreach Guidelines & Invariants
1. **Never accuse maintainers of bad tests**: Always lead with their clean baseline (e.g. "94% clean", "98% clean").
2. **Zero unsolicited issue/PR filings**: Reach out via direct email or Twitter/X DM only.
3. **Always provide reproducible local proof**: Maintainers can run `curl -sSfL ... | tar -xz && ./proctor audit --limit 100` locally in <3s with zero telemetry.
4. **Observe Mode by default**: Always pitch `enforce: false` so it adds value to `$GITHUB_STEP_SUMMARY` without blocking CI builds.
