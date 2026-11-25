#!/usr/bin/env bash
# Claude Code Stop hook for agent-proctor.
# Blocks ONLY when an added/modified test is Broken on HEAD; emits a soft note when a
# test passes on both head and base (`DoesNotDiscriminate`). Safe against loops and hangs.
#
# Recommended .claude/settings.json entry (timeout > verify budget):
#   {
#     "hooks": {
#       "Stop": [{
#         "hooks": [{
#           "type": "command",
#           "command": "bash .claude/hooks/proctor-hook.sh",
#           "timeout": 180
#         }]
#       }]
#     }
#   }

set -euo pipefail

HOOK_INPUT="$(cat || true)"
STOP_ACTIVE="$(python3 -c '
import json, sys
try:
    data = json.loads(sys.stdin.read() or "{}")
    print("true" if data.get("stop_hook_active") else "false")
except Exception:
    print("false")
' <<< "$HOOK_INPUT")"

# Prevent infinite Stop -> Block -> Stop loops
if [ "$STOP_ACTIVE" = "true" ]; then
  exit 0
fi

PROCTOR_BIN="${PROCTOR_BIN:-proctor}"
if ! command -v "$PROCTOR_BIN" >/dev/null 2>&1; then
  exit 0
fi

# Resolve merge-base on branches so upstream main edits do not pollute base swaps
BASE_REF="${PROCTOR_BASE_REF:-}"
if [ -z "$BASE_REF" ]; then
  if git rev-parse --verify origin/main >/dev/null 2>&1; then
    MB="$(git merge-base HEAD origin/main 2>/dev/null || true)"
    [ -n "$MB" ] && BASE_REF="$MB" || BASE_REF="HEAD"
  elif git rev-parse --verify main >/dev/null 2>&1; then
    MB="$(git merge-base HEAD main 2>/dev/null || true)"
    [ -n "$MB" ] && BASE_REF="$MB" || BASE_REF="HEAD"
  else
    BASE_REF="HEAD"
  fi
fi

ERR_FILE="$(mktemp)"
trap 'rm -f "$ERR_FILE"' EXIT

if ! VERIFY_JSON="$("$PROCTOR_BIN" verify --base "$BASE_REF" --max-tests 5 --format json 2>"$ERR_FILE")"; then
  FIRST_ERR="$(head -n 2 "$ERR_FILE" | tr '\n' ' ')"
  echo "proctor [advisory]: verify did not run ($FIRST_ERR)" >&2
  exit 0
fi

PARSED="$(python3 -c '
import json, sys
data = json.loads(sys.stdin.read() or "[]")
broken = []
advisory = []
for r in data:
    st = r.get("status")
    tid = r.get("test_id", r.get("test_file"))
    if isinstance(st, dict) and "Broken" in st:
        err = st["Broken"].get("error", "")
        broken.append(f"BROKEN {tid}: {err}")
    elif st == "DoesNotDiscriminate":
        advisory.append(f"PASSES ON BOTH {tid}: passed on base without your changes (only review if testing a behavior change)")
print("BROKEN_COUNT=" + str(len(broken)))
for b in broken:
    print("B:" + b)
for a in advisory:
    print("A:" + a)
' <<< "$VERIFY_JSON")"

while IFS= read -r line; do
  case "$line" in
    A:*) echo "proctor [advisory]: ${line#A:}" >&2 ;;
  esac
done <<< "$PARSED"

BROKEN_LINES="$(echo "$PARSED" | grep '^B:' | sed 's/^B://' || true)"
if [ -n "$BROKEN_LINES" ]; then
  echo "proctor: test(s) failed on current branch:" >&2
  echo "$BROKEN_LINES" >&2
  exit 2
fi

exit 0
