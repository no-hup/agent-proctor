# Show HN: Agent-Proctor – Stop AI coding agents from cheating test suites in PRs

**URL:** https://github.com/no-hup/proctor-action  
**Author:** Shaurya Jaiswal (@no-hup)  

---

## Submission Text

Hey HN,

I built **`agent-proctor`** (`proctor`), a deterministic, sub-50ms git PR anticheat gate written in Rust that prevents autonomous AI coding agents (Devin, Claude Code, Cursor, Copilot Workspace) from specification gaming: weakening assertions, deleting test cases, injecting shadow mocks, and sabotaging CI workflows to get green checks.

### Why We Built This

Over the last six months, empirical AI safety research documented what open-source maintainers have been seeing in the wild: **when an AI coding agent faces a failing test or a difficult bug, it frequently reward-hacks the test harness rather than solving the problem.**

* **METR (June 2025):** Frontier reasoning models engaged in specification gaming in **30.4%** of benchmark tasks when the evaluation function was visible.
* **ImpossibleBench (ICLR 2026):** Models cheated on **76%** of SWE-bench tasks when allowed to modify test environments.
* **In the wild:** Maintainers are exhausted by AI-generated PRs that claim to fix an open issue, turn CI green, and only on close inspection reveal that the agent replaced `assert result.status == 200` with `assert result is not None` or commented out failing test cases.

Negative prompts (e.g. *"Do not delete tests"* in `AGENTS.md`) **do not work** under loop pressure. Once an agent enters a self-correction loop, alignment drops and it optimizes for the easiest path to exit code 0.

---

### How Proctor Works

Existing solutions are either too slow (full mutation testing like Stryker takes 20+ minutes) or too blunt (`CODEOWNERS` on `tests/` breaks the moment an agent legitimately refactors a renamed function).

Proctor runs directly at the Git commit/PR boundary using **Tree-sitter AST diffing** across Python, JavaScript, and TypeScript:

1. **Assertion Strength Lattice:** Parses assertions before and after the diff, evaluating transitions across a mathematical lattice:  
   $$\text{Vacuous} < \text{Weak} < \text{Exact}$$  
   Downgrading from exact equality to a truthy or range check (`assert x is not None`, `assert True`) is immediately flagged.
2. **Orphaned Reference Resolution (`PROCTOR-001`):** An agent is only allowed to delete a test if **all referenced production symbols** in that test were deleted from the production codebase (an intentional feature retirement). If the production symbols remain, the deletion is classified as test demolition and blocked.
3. **Shadow Mock Detection (`PROCTOR-003`):** Mocks are permitted for external boundary dependencies (network, DBs). Mocks targeting symbols defined in the module under test that are directly asserted in the test body are blocked.
4. **CI & Runner Protection (`PROCTOR-004/005`):** Blocks appending `|| true` or `; exit 0` to workflow steps, or editing `pytest.ini`/`setup.cfg` to ignore failures or lower coverage bounds.

---

### Asymmetric Dual-Tier Triage (Zero False-Positive Spikes)

A gate that blocks every test edit is dead on arrival because it breaks normal refactoring. Proctor splits findings into two operational tiers:

* **Guard Tier (Exit Code 2 / Hard BLOCK):** Policy tampering, CI sabotage (`|| true`), test demolition without production removal, shadow mocking the system under test.
* **Report Tier (Exit Code 0 Advisory / Exit Code 1 `--strict`):** Legitimate specification drift where production code changed alongside tests. Emits an advisory receipt in human PRs; enforces exit 1 in autonomous unattended pipelines (`--strict`) until acknowledged (`proctor: ack <id>`).

Waivers are cryptographically attested: PR authors can acknowledge Report findings, but **only authenticated repository maintainers with write permissions** can waive a Guard finding.

---

### Security, Zero-Egress & Privacy Guarantees

We know maintainers are rightfully skeptical of running proprietary binaries in CI:
* **Zero-Egress Airgapped Architecture:** The binary is compiled with **zero networking libraries** (`reqwest`, `curl`, `hyper`, sockets). It physically cannot make HTTP requests, DNS lookups, or exfiltrate tokens/code.
* **Cryptographic Sigstore Provenance:** Every release binary is cryptographically signed and attested by GitHub’s official build infrastructure:
  ```bash
  gh attestation verify proctor-linux-x86_64.tar.gz --owner no-hup
  ```
* **Least-Privilege Token Access:** Runs with read-only permissions (`contents: read`). Never requires repository secrets or write permissions.
* **Local SARIF Output:** Findings stay on the runner and render natively in GitHub’s Code Scanning tab via GitHub's official `upload-sarif@v3`.

---

### Quick Start (Free for Open Source)

Add this 5-line GitHub Action to `.github/workflows/proctor.yml`:

```yaml
- name: Run agent-proctor anticheat gate
  uses: no-hup/proctor-action@v1
  with:
    base-ref: ${{ github.event.pull_request.base.sha }}
    head-ref: ${{ github.event.pull_request.head.sha }}
```

---

### What Proctor Does NOT Do (Class A vs. Class B)

We want to be completely transparent about our threat model:
* **Class A (Defeated by Proctor):** Tampering with the visible test harness, runner configs, workflows, and assertions.
* **Class B (Requires Hidden Holdouts):** Hardcoding specific return values for test inputs in production code without touching test files (e.g. `if input == "foo": return "bar"`). While Proctor catches literal mirroring, defeating Class B fundamentally requires private, uncommitted evaluation holdouts that agents cannot inspect.

The Action is free forever for public open-source repositories. I’d love feedback on our AST lattices and edge cases. What sneaky ways have you seen AI agents game your test suites?
