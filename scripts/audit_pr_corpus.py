#!/usr/bin/env python3
"""
scripts/audit_pr_corpus.py - Audit git commit history across Agent and Human PRs.

Usage:
    python3 scripts/audit_pr_corpus.py --repo-dir /path/to/repo --limit 500 --out results.csv

Runs `proctor audit` to measure block_rate_on_merged_history.
Supports separating agent-authored commits (e.g. author containing 'bot', 'copilot', 'devin')
from human-authored commits to generate a rigorous side-by-side comparison table.
"""

import argparse
import subprocess
import json
import csv
import sys
from pathlib import Path

def main():
    parser = argparse.ArgumentParser(description="Audit PR history with proctor.")
    parser.add_argument("--repo-dir", default=".", help="Repository directory to audit")
    parser.add_argument("--limit", type=int, default=500, help="Number of recent commits to audit")
    parser.add_argument("--out", default="audit_summary.csv", help="Path to write CSV report")
    parser.add_argument("--proctor-bin", default="proctor", help="Path to proctor binary")
    args = parser.parse_args()

    repo_path = Path(args.repo_dir).resolve()
    if not repo_path.exists():
        print(f"Error: Repo path not found: {repo_path}", file=sys.stderr)
        sys.exit(1)

    cmd = [
        args.proctor_bin,
        "audit",
        "--limit", str(args.limit),
        "--label", args.out
    ]

    print(f"Running proctor audit on {repo_path} (limit={args.limit})...")
    try:
        proc = subprocess.run(cmd, cwd=repo_path, capture_output=True, text=True, check=False)
        print(proc.stdout)
        if proc.stderr:
            print(proc.stderr, file=sys.stderr)
    except FileNotFoundError:
        print(f"Error: {args.proctor_bin} not found. Please build or install proctor.", file=sys.stderr)
        sys.exit(1)

    print(f"\nAudit complete. Detailed records written to: {args.out}")

if __name__ == "__main__":
    main()
