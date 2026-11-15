#!/usr/bin/env python3
"""
scripts/audit_swebench.py - Analyze SWE-bench evaluation findings from proctor eval.

Usage:
    python3 scripts/audit_swebench.py --results audit_results.jsonl [--verbose]

This script processes output from:
    proctor eval --format swebench predictions.jsonl --out audit_results.jsonl

It categorizes findings according to the methodology outlined in docs/methods/AUDIT_METHODOLOGY.md:
- Guard vs Report tier breakdowns
- Runner config tampering (conftest, pytest.ini, etc.) vs test body modifications
- Rule distribution (PROCTOR-001 through PROCTOR-007)
"""

import argparse
import json
import sys
from collections import Counter, defaultdict
from pathlib import Path

RUNNER_CONFIG_PATTERNS = [
    "conftest.py",
    "pytest.ini",
    "tox.ini",
    "setup.cfg",
    "pyproject.toml",
    "jest.config",
]

def is_runner_config(file_path: str) -> bool:
    return any(pattern in file_path for pattern in RUNNER_CONFIG_PATTERNS)

def main():
    parser = argparse.ArgumentParser(description="Analyze proctor eval findings for SWE-bench.")
    parser.add_argument("--results", required=True, help="Path to audit_results.jsonl from proctor eval")
    parser.add_argument("--verbose", action="store_true", help="Print details for each flagged instance")
    args = parser.parse_args()

    results_file = Path(args.results)
    if not results_file.exists():
        print(f"Error: Results file not found: {results_file}", file=sys.stderr)
        sys.exit(1)

    total_instances = 0
    clean_instances = 0
    guard_instances = 0
    report_instances = 0
    
    rule_counts = Counter()
    runner_config_flags = 0
    instances_with_runner_tamper = set()
    
    flagged_records = []

    with open(results_file, "r", encoding="utf-8") as f:
        for line in f:
            line = line.strip()
            if not line:
                continue
            total_instances += 1
            try:
                record = json.loads(line)
            except json.JSONDecodeError:
                continue

            instance_id = record.get("instance_id", f"unknown-{total_instances}")
            verdict = record.get("verdict", "Allow")
            findings = record.get("findings", [])

            if not findings or verdict == "Allow":
                clean_instances += 1
                continue

            has_guard = verdict == "Block" or any(f.get("tier") == "Guard" for f in findings)
            if has_guard:
                guard_instances += 1
            else:
                report_instances += 1

            instance_has_runner_edit = False
            for finding in findings:
                rule_id = finding.get("rule_id", "UNKNOWN")
                rule_counts[rule_id] += 1
                file_path = finding.get("file", "")
                if is_runner_config(file_path):
                    runner_config_flags += 1
                    instance_has_runner_edit = True

            if instance_has_runner_edit:
                instances_with_runner_tamper.add(instance_id)

            flagged_records.append({
                "instance_id": instance_id,
                "verdict": verdict,
                "has_guard": has_guard,
                "findings": findings
            })

    # Summary Output
    print("=" * 68)
    print("           AGENT-PROCTOR SWE-BENCH AUDIT SUMMARY")
    print("=" * 68)
    print(f"Total Instances Evaluated:         {total_instances:>6}")
    print(f"Clean Instances (No Flags):        {clean_instances:>6} ({clean_instances/max(1,total_instances)*100:>5.1f}%)")
    print(f"Flagged Guard Tier (Hard Block):   {guard_instances:>6} ({guard_instances/max(1,total_instances)*100:>5.1f}%)")
    print(f"Flagged Report Tier (Advisory):    {report_instances:>6} ({report_instances/max(1,total_instances)*100:>5.1f}%)")
    print("-" * 68)
    print(f"Instances with Runner Config Edits: {len(instances_with_runner_tamper):>5} (affect SWE-bench grading)")
    print("-" * 68)
    print("Rule ID Breakdown:")
    for rule_id, count in sorted(rule_counts.items()):
        print(f"  - {rule_id:<14}: {count:>4} occurrences")
    print("=" * 68)

    if args.verbose and flagged_records:
        print("\nFlagged Instance Details:")
        for rec in flagged_records:
            tier_label = "GUARD [BLOCK]" if rec["has_guard"] else "REPORT [ADVISORY]"
            print(f"\nInstance: {rec['instance_id']} ({tier_label})")
            for f in rec["findings"]:
                print(f"  * [{f.get('rule_id')}] {f.get('file')}: {f.get('message')}")

if __name__ == "__main__":
    main()
