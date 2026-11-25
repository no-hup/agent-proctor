# Track D Outreach Drafts (`v0.2.2`)

> **POLICY PER [GROWTH_SCRATCHBOOK.md](GROWTH_SCRATCHBOOK.md):**
> - **Do NOT send any email/DM automatically.**
> - **Section 1 (Corrections to `OpenHands` / `litellm`):** Send **ONLY** if Graham or Krrish replied or might cite our noisy `v0.1.0` numbers. If they stayed silent, do not send an unprompted second email. No receipt tables, no YAML pitch, pure 4-line correction.
> - **Section 2 (`browser-use`):** **PARKED / DO NOT SEND** (cold flattering-receipt mega-OSS outreach was retired after Grok + Claude reviews).

---

## 1. Conditional 4-Line Factual Corrections Only

### Graham Neubig (`OpenHands/OpenHands`, `neubig@gmail.com` — thread `1a1e66db099b4a69`)
*Trigger: only if Graham replied or cited the Nov 17 baseline.*

```text
Hey Graham — quick factual correction to the v0.1.0 OpenHands numbers I sent Nov 17: a `.tsx` parser bug (`typeof import(...)`) and noisy co-evolution rules inflated our flags (`56/33/11` Pass/Review/Block). After fixing both in v0.2.2, the 100-commit replay is `89/7/4` (`docs/audit/V0_1_VS_V0_2_COMPARISON.md`). Wanted to make sure you didn't rely on the inflated baseline — no action needed.
```

### Krrish Dholakia (`BerriAI/litellm`, `krrish@berri.ai` — thread `1a1e66dc6434541e`)
*Trigger: only if Krrish replied or cited the Nov 17 baseline.*

```text
Hey Krrish — quick factual correction to the v0.1.0 LiteLLM numbers I sent Nov 17: those flags were inflated mostly by normal co-evolution (tests intentionally updated with code changes) plus a Python `with`-block parser bug. After calibrating both in v0.2.2, the 100-commit replay is `83/5/12` (`docs/audit/V0_1_VS_V0_2_COMPARISON.md`). Wanted to leave accurate numbers on the record — no reply needed.
```

---

## 2. Retired / Parked Cold Mega-OSS Drafts
- `browser-use/browser-use` (`@gregpr07`), Tier-1 cold outreach (`Pydantic`, `FastAPI`, `Textual`, `HTTPX`, `Zod`): **PARKED** per [GROWTH_SCRATCHBOOK.md](GROWTH_SCRATCHBOOK.md). Replaced by warm coding-agent teams + the `20+20` headless agent benchmark (`docs/audit/AGENT_CHEAT_BENCH.md`).
