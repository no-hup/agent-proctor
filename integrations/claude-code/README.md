# Claude Code Integration: Proctor Stop Hook

This integration wires `agent-proctor` into the **Claude Code** inner loop using Claude Code's Stop Hook mechanism.

---

## Why Use the Stop Hook?

When Claude Code is working autonomously on bug fixes or feature requests, it may be tempted to:
* Weaken assertion bounds to make failing tests pass.
* Comment out or delete assertions.
* Inject mocks for the module it is supposed to be fixing.
* Tweak test runner configurations (`pytest.ini`, `conftest.py`).

The Proctor Stop Hook intercepts Claude Code *before* it concludes its turn, running a deterministic sub-200ms Tree-sitter check. If tampering is detected, the hook gives Claude specific remediation instructions to fix the production code rather than cheating the test.

---

## Anti-Thrashing Guardrails

1. **Guard-Only Blocking:** Report-tier findings (legitimate assertion adjustments coincident with production code changes) exit 0 with an advisory receipt; they do not trap Claude in an edit loop.
2. **1-Retry Budget:** If Claude cannot resolve a Guard finding on the first retry, the hook yields to human review rather than burning user tokens.
3. **Fast Path:** Bypasses AST checks entirely if no test or configuration files were touched.

---

## Setup Instructions

1. Ensure `proctor` is installed and available in your `PATH` (or compile from source with `cargo install --path crates/proctor-cli`).
2. Add the stop hook to your project's `.claude/settings.json` (or global `~/.claude/settings.json`):

```json
{
  "hooks": {
    "stop": [
      {
        "command": "./integrations/claude-code/stop-hook.sh"
      }
    ]
  }
}
```

3. Test the hook manually:
```bash
./integrations/claude-code/stop-hook.sh
```
If the working tree is clean, it exits immediately with code 0.
