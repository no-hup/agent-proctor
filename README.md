# `agent-proctor` (`proctor`)

> **Prove your new tests fail without the fix and pass with it — and ensure existing test contracts stay intact.**  
> Zero-egress, deterministic CLI & GitHub Action for Python (`pytest`) and JavaScript/TypeScript (`Jest`, `Vitest`).

[![CI](https://github.com/no-hup/agent-proctor/actions/workflows/ci.yml/badge.svg)](https://github.com/no-hup/agent-proctor/actions/workflows/ci.yml)
[![License: Apache-2.0](https://img.shields.io/badge/License-Apache--2.0-blue.svg)](LICENSE)

---

## Why `proctor`?

When a bugfix PR (whether written by a teammate or a coding agent like Claude Code, Cursor, or Devin) adds a test and turns CI green, reviewers still have to guess two things:

1. **Does the new test actually catch the bug?** Or would it pass even on `main` without the fix?
2. **Did existing tests stay just as strong?** Or did assertions get loosened, mocked out, skipped, or runner configs relaxed?

`proctor` answers both deterministically with zero LLMs and zero network calls.

---

## 1. Prove Tests Catch the Bug (`proctor verify` — Experimental)

`proctor verify` temporarily swaps your modified production files back to the base commit (preserving your existing `.venv`, editable installs, and `node_modules`), runs your new and edited tests twice on `head` and twice on `base` using structured `--junitxml` / `--json` outputs, and restores the working tree cleanly:

```bash
proctor verify --base main
```

Every changed test gets a concrete verdict:

- **`Discriminating`**: Passed 2x on `head` and failed 2x on `base` with an assertion failure — proves the fix.
- **`DoesNotDiscriminate`**: Passed on both `head` and `base` — your test doesn't exercise the change.
- **`MissingOnBase`**: Passed on `head` and failed on `base` because the symbol/module didn't exist yet.
- **`Broken`**: Fails on your own `head` branch.
- **`Inconclusive`**: Timed out, flaky, dependency manifest changed, setup/type error on base, or missing runner (never blocks CI unexpectedly).

If a run is killed mid-swap (`SIGINT`/`SIGKILL`), `proctor verify --restore` recovers every file from `.git/proctor-swap-backup.json`.

---

## 2. Run Inside Your Coding Agent Loop

Stop discovering weak or non-discriminating tests at PR review time. Drop `proctor` into your local agent workflow so the agent proves its own tests before saying "done":

- **Claude Code Hook:** Copy [`integrations/claude-code/proctor-hook.sh`](integrations/claude-code/proctor-hook.sh) to `.claude/hooks/proctor-hook.sh` and wire it into `.claude/settings.json` under `hooks.Stop`.
- **Cursor Rule:** Copy [`integrations/cursor/proctor.mdc`](integrations/cursor/proctor.mdc) to `.cursor/rules/proctor.mdc`.
- **`AGENTS.md` / `CLAUDE.md`:** Append the snippet in [`integrations/AGENTS.md`](integrations/AGENTS.md).

---

## 3. Quiet Static Diff Gate (`proctor scan`)

When you run `proctor scan --base main` (under 200ms, pure Tree-sitter AST diffing), `proctor` checks existing tests and CI gates without blocking legitimate refactors:

- Keeps deleted tests clean when every referenced production symbol was removed or renamed.
- Flags assertion weakening, unexpected shadow mocks of changed code, dropped `@pytest.mark.parametrize` cases, snapshot truncation, config strictness loosening (`tsconfig.json`, `ruff.toml`, `eslint`), blanket file-level suppressions (`@ts-nocheck`, bare `eslint-disable`), and CI workflow disabling (`|| true`).
- Writes clean Markdown PR summaries via `--receipt $GITHUB_STEP_SUMMARY` and standard SARIF 2.1.0 via `--sarif proctor-results.sarif`.

---

## 5-Second Quickstart

```bash
cargo install --path crates/proctor-cli

# Verify newly added or edited tests against main
proctor verify --base main

# Fast static diff check + Markdown summary
proctor scan --base origin/main --format markdown

# Static scan + dynamic verification together
proctor scan --base origin/main --verify
```

---

## GitHub Action (`no-hup/proctor-action`)

```yaml
name: Test Integrity & Proof

on:
  pull_request:
    branches: [main, master]

permissions:
  contents: read
  pull-requests: read

jobs:
  proctor:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0

      # Install project dependencies first when enabling verify: true
      - uses: no-hup/proctor-action@v1
        with:
          base-ref: ${{ github.event.pull_request.base.sha }}
          head-ref: ${{ github.event.pull_request.head.sha }}
          verify: 'true'    # Experimental: requires dependencies installed first
          enforce: 'false'  # Observe Mode: posts PR step summary without failing CI
```

See [proctor-action](https://github.com/no-hup/proctor-action) for pinned SHA-256 airgapped installation, waiver comments (`proctor: ack <id>`), and `proctor.toml` options.

---

## Security & Zero-Egress Guarantees

1. **Zero Networking Dependencies:** Compiled without `reqwest`, `hyper`, `curl`, sockets, or outbound HTTP (`test_zero_network_dependencies_in_lockfile` enforces this in CI). No telemetry leaves your machine or runner.
2. **Never Invokes `npx`:** `verify` executes your repo's installed binaries directly (`node_modules/.bin/{jest,vitest}`, `.venv/bin/python -m pytest`).
3. **`pull_request_target` Protection:** Dynamic verification automatically refuses to run on `pull_request_target` events to keep repository secrets isolated from fork PRs.
4. **Sigstore Provenance:** Release tarballs are attested via GitHub Artifact Attestations (`gh attestation verify`).

---

## Documentation & Reference

- **[Rules & Configuration Reference](RULES_CATALOG.md)** *(in `proctor-action`)*: Full specification of rule codes, `proctor.toml`, and waivers.
- **[Architecture Decision Records](docs/DECISIONS.md)**: Rationale behind zero-LLM verification, linkage analysis, and waiver isolation.
- **[Track C Live Verification Matrix](docs/audit/TRACK_C_MATRIX.md)**: Verified outcomes across live `uv`/`pytest`, editable `src/` installs, `Jest`, `Vitest`, and crash recovery.

## License

Apache-2.0. See [LICENSE](LICENSE).
