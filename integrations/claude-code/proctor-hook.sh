#!/usr/bin/env bash
# Drop-in Claude Code Stop / pre-completion hook for agent-proctor.
# Checks modified tests with `proctor verify` and guards against weakened tests before finishing.
# Install in .claude/settings.json under hooks.Stop:
#   { "type": "command", "command": "bash .claude/hooks/proctor-hook.sh" }

set -euo pipefail

if ! command -v proctor >/dev/null 2>&1; then
  exit 0
fi

BASE_REF="${PROCTOR_BASE_REF:-HEAD}"
# Prefer origin/main or main if we are on a feature branch with commits ahead
if git rev-parse --verify origin/main >/dev/null 2>&1 && [ "$(git rev-parse HEAD)" != "$(git rev-parse origin/main)" ]; then
  BASE_REF="${PROCTOR_BASE_REF:-origin/main}"
elif git rev-parse --verify main >/dev/null 2>&1 && [ "$(git rev-parse HEAD)" != "$(git rev-parse main)" ]; then
  BASE_REF="${PROCTOR_BASE_REF:-main}"
fi

# Skip cleanly if no tests or test-related files changed
if git diff --name-only "$BASE_REF" -- 2>/dev/null | grep -qE '(^|/)(tests?|__tests__|spec)/|_test\.|\.test\.|\.spec\.'; then
  :
elif git ls-files --others --exclude-standard 2>/dev/null | grep -qE '(^|/)(tests?|__tests__|spec)/|_test\.|\.test\.|\.spec\.'; then
  :
else
  exit 0
fi

VERIFY_JSON="$(proctor verify --base "$BASE_REF" --format json 2>/dev/null || echo '[]')"
BROKEN_OR_WEAK="$(python3 -c '
import json, sys
data = json.loads(sys.stdin.read() or "[]")
problems = []
for r in data:
    st = r.get("status")
    tid = r.get("test_id", r.get("test_file"))
    if isinstance(st, dict) and "Broken" in st:
        err = st["Broken"].get("error", "")
        problems.append(f"BROKEN {tid}: {err}")
    elif st == "DoesNotDiscriminate":
        problems.append(f"DOES NOT DISCRIMINATE {tid}: passes on base without your fix")
print("\n".join(problems))
' <<< "$VERIFY_JSON")"

if [ -n "$BROKEN_OR_WEAK" ]; then
  echo "proctor verify found tests that fail on head or pass on base without the fix:" >&2
  echo "$BROKEN_OR_WEAK" >&2
  exit 2
fi

exit 0
