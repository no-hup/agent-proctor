#!/usr/bin/env bash
# integrations/claude-code/stop-hook.sh
#
# Inner-loop Stop Hook for Claude Code.
# Ensures that autonomous turns do not leave the workspace in a state that weakens
# tests, deletes assertions, injects shadow mocks, or sabotages CI workflows.
#
# Design Invariants:
# 1. Fast Path: Exits 0 in <5ms if no test, config, workflow, or policy files were touched.
# 2. Guard-Only Block: Only hard Guard findings (exit 2) block the agent. Report findings
#    emit an advisory note and exit 0 to prevent turn loops.
# 3. 1-Retry Budget: Hashes the Guard finding set in .git/proctor-last-guard-hash. If the
#    exact same Guard finding is repeated after 1 retry, it exits 0 and yields to human review.
# 4. Actionable Instructions: Translates finding IDs into concrete instructions for Claude.

set -euo pipefail

# 1. Fast Path: Check if relevant files were modified
MODIFIED_FILES=$(git diff --name-only HEAD 2>/dev/null || true)
if [ -z "$MODIFIED_FILES" ]; then
  # If no uncommitted changes, check against upstream/main if available
  MODIFIED_FILES=$(git diff --name-only origin/main..HEAD 2>/dev/null || true)
fi

# If still empty, exit immediately
if [ -z "$MODIFIED_FILES" ]; then
  exit 0
fi

# Check for sensitive patterns: tests, configs, workflows, policy
RELEVANT=0
while IFS= read -r file; do
  case "$file" in
    *test*|*spec*|conftest.py|pytest.ini|tox.ini|setup.cfg|jest.config*|.github/workflows*|*proctor.toml)
      RELEVANT=1
      break
      ;;
  esac
done <<< "$MODIFIED_FILES"

if [ "$RELEVANT" -eq 0 ]; then
  # No test or harness files touched; exit cleanly
  exit 0
fi

# 2. Check if proctor CLI is available
PROCTOR_BIN="${PROCTOR_BIN:-proctor}"
if ! command -v "$PROCTOR_BIN" &>/dev/null; then
  # Try relative target release if running in agent-proctor repo
  if [ -x "./target/release/proctor" ]; then
    PROCTOR_BIN="./target/release/proctor"
  else
    # Failsafe: if proctor is not installed, do not block the agent
    exit 0
  fi
fi

# 3. Run proctor scan in JSON mode
SCAN_OUTPUT=$("$PROCTOR_BIN" scan --format json 2>&1 || true)

# Parse verdict and findings
VERDICT=$(echo "$SCAN_OUTPUT" | grep -E '^\{' | jq -r '.verdict // empty' 2>/dev/null || true)

if [ "$VERDICT" = "Allow" ] || [ -z "$VERDICT" ]; then
  # Clean scan; clear any previous retry hash
  rm -f .git/proctor-last-guard-hash 2>/dev/null || true
  exit 0
fi

# Extract findings by tier
GUARD_FINDINGS=$(echo "$SCAN_OUTPUT" | jq -c '[.findings[]? | select(.tier == "Guard")]' 2>/dev/null || echo "[]")
REPORT_FINDINGS=$(echo "$SCAN_OUTPUT" | jq -c '[.findings[]? | select(.tier == "Report")]' 2>/dev/null || echo "[]")

GUARD_COUNT=$(echo "$GUARD_FINDINGS" | jq 'length' 2>/dev/null || echo "0")
REPORT_COUNT=$(echo "$REPORT_FINDINGS" | jq 'length' 2>/dev/null || echo "0")

# If only Report findings exist, emit advisory notice and exit 0 (do not block)
if [ "$GUARD_COUNT" -eq 0 ]; then
  if [ "$REPORT_COUNT" -gt 0 ]; then
    echo "proctor [advisory]: Detected $REPORT_COUNT specification change(s). Please ensure this will be acknowledged or justified in the PR." >&2
  fi
  rm -f .git/proctor-last-guard-hash 2>/dev/null || true
  exit 0
fi

# 4. Handle Guard Findings with 1-Retry Budget
CURRENT_HASH=$(echo "$GUARD_FINDINGS" | shasum -a 256 | awk '{print $1}')
LAST_HASH=""
if [ -f .git/proctor-last-guard-hash ]; then
  LAST_HASH=$(cat .git/proctor-last-guard-hash 2>/dev/null || true)
fi

if [ "$CURRENT_HASH" = "$LAST_HASH" ]; then
  # Same guard findings remain after 1 retry; yield to human review
  rm -f .git/proctor-last-guard-hash 2>/dev/null || true
  echo "proctor: Guard finding unchanged after 1 retry. Deferring to human user review." >&2
  exit 0
fi

# Store hash for next turn
mkdir -p .git
echo "$CURRENT_HASH" > .git/proctor-last-guard-hash

# Emit actionable instructions to Claude Code
echo "======================================================================" >&2
echo "proctor BLOCK: Test integrity violation detected in current changes:" >&2
echo "======================================================================" >&2

echo "$GUARD_FINDINGS" | jq -r '.[] | "• [\(.rule_id)] \(.file): \(.message)"' >&2

echo "" >&2
echo "ACTION REQUIRED BEFORE PROCEEDING:" >&2
echo "1. Do NOT delete or weaken assertions to make tests pass." >&2
echo "2. Fix the underlying bug in the implementation so the test passes honestly." >&2
echo "3. If a test is obsolete due to an intentional refactor, remove all references" >&2
echo "   to the obsolete symbol in production code." >&2
echo "======================================================================" >&2

# Exit 1 tells Claude Code the hook failed, prompting Claude to fix it
exit 1
