#!/usr/bin/env python3
"""
scripts/scale_agent_pr_sweep.py
Empirical benchmark sweep evaluating LinkageStrategy implementations:
BareTokenLinker vs ScopedImportLinker across real-world PR datasets.

Measures:
- Total execution time and throughput (<50ms budget verification).
- Discrepancy analysis: finding count deltas, false negative suppression by bare names,
  and stability of Guard vs Review tiers.
"""

import argparse
import json
import subprocess
import sys
import time
from collections import Counter
from pathlib import Path

DEFAULT_REPOS = ["aider", "SWE-agent", "browser-use", "OpenHands", "litellm"]

def get_commits(repo_dir: Path, limit: int = 100) -> list[str]:
    res = subprocess.run(
        ["git", "log", f"-n{limit}", "--format=%H"],
        cwd=repo_dir,
        capture_output=True,
        text=True,
        check=True,
    )
    commits = [line.strip() for line in res.stdout.splitlines() if line.strip()]
    valid = []
    for c in commits:
        p = subprocess.run(
            ["git", "rev-parse", "--verify", f"{c}^"],
            cwd=repo_dir,
            capture_output=True,
            text=True,
        )
        if p.returncode == 0:
            valid.append(c)
    return valid

def scan_commit(bin_path: Path, repo_dir: Path, sha: str, linkage: str) -> dict:
    t0 = time.perf_counter()
    res = subprocess.run(
        [
            str(bin_path),
            "scan",
            "--base", f"{sha}^",
            "--head", sha,
            "--format", "json",
            "--linkage", linkage,
        ],
        cwd=repo_dir,
        capture_output=True,
        text=True,
    )
    elapsed_ms = (time.perf_counter() - t0) * 1000.0
    if res.returncode not in (0, 1, 2):
        return {
            "error": res.stderr.strip(),
            "elapsed_ms": elapsed_ms,
            "status": "error",
            "findings": [],
        }
    try:
        data = json.loads(res.stdout)
        data["elapsed_ms"] = elapsed_ms
        return data
    except Exception as e:
        return {
            "error": f"JSON decode error: {e}",
            "elapsed_ms": elapsed_ms,
            "status": "error",
            "findings": [],
        }

def run_sweep(bin_path: Path, audit_root: Path, limit: int = 100):
    print("=" * 70)
    print(" PROCTOR EMPIRICAL BENCHMARK SWEEP: BARE vs SCOPED LINKAGE")
    print(f" Binary:     {bin_path}")
    print(f" Dataset:    {audit_root}")
    print(f" PR Limit:   {limit} per repo")
    print("=" * 70)

    total_commits = 0
    bare_times = []
    scoped_times = []
    
    bare_verdicts = Counter()
    scoped_verdicts = Counter()
    bare_findings_count = 0
    scoped_findings_count = 0

    discrepancies = []

    for repo_name in DEFAULT_REPOS:
        repo_dir = audit_root / repo_name
        if not repo_dir.exists():
            print(f"Skipping {repo_name} (not found in {audit_root})")
            continue

        commits = get_commits(repo_dir, limit=limit)
        total_commits += len(commits)
        print(f"\nEvaluating '{repo_name}' ({len(commits)} PRs/commits)...")

        repo_bare_count = 0
        repo_scoped_count = 0

        for i, sha in enumerate(commits, 1):
            bare_res = scan_commit(bin_path, repo_dir, sha, "bare")
            scoped_res = scan_commit(bin_path, repo_dir, sha, "scoped")

            if "elapsed_ms" in bare_res:
                bare_times.append(bare_res["elapsed_ms"])
            if "elapsed_ms" in scoped_res:
                scoped_times.append(scoped_res["elapsed_ms"])

            b_stat = bare_res.get("verdict", {}).get("status", "Unknown")
            s_stat = scoped_res.get("verdict", {}).get("status", "Unknown")
            bare_verdicts[b_stat] += 1
            scoped_verdicts[s_stat] += 1

            b_finds = bare_res.get("findings", [])
            s_finds = scoped_res.get("findings", [])
            bare_findings_count += len(b_finds)
            scoped_findings_count += len(s_finds)
            repo_bare_count += len(b_finds)
            repo_scoped_count += len(s_finds)

            if len(b_finds) != len(s_finds) or b_stat != s_stat:
                diff_item = {
                    "repo": repo_name,
                    "sha": sha,
                    "bare_status": b_stat,
                    "scoped_status": s_stat,
                    "bare_findings": len(b_finds),
                    "scoped_findings": len(s_finds),
                }
                discrepancies.append(diff_item)

            if i % 25 == 0 or i == len(commits):
                print(f"  [{i}/{len(commits)}] PRs scanned... bare={repo_bare_count} scoped={repo_scoped_count}")

    avg_bare_ms = sum(bare_times) / len(bare_times) if bare_times else 0
    avg_scoped_ms = sum(scoped_times) / len(scoped_times) if scoped_times else 0

    print("\n" + "=" * 70)
    print(" SWEEP RESULTS SUMMARY")
    print("=" * 70)
    print(f" Total Commits/PRs Analyzed: {total_commits}")
    print(f" Avg Scan Time (Bare):        {avg_bare_ms:.2f} ms")
    print(f" Avg Scan Time (Scoped):      {avg_scoped_ms:.2f} ms")
    print(f" Target Budget (<50ms):       {'PASSED' if avg_scoped_ms < 50 else 'EXCEEDED'}")
    print("\nVerdict Distribution:")
    print(f"  Bare:   {dict(bare_verdicts)}")
    print(f"  Scoped: {dict(scoped_verdicts)}")
    print("\nTotal Findings:")
    print(f"  Bare:   {bare_findings_count}")
    print(f"  Scoped: {scoped_findings_count}")
    print(f"  Discrepancies: {len(discrepancies)} PRs with differing results")

    if discrepancies:
        print("\nDiscrepancy Samples:")
        for d in discrepancies[:5]:
            print(f"  - {d['repo']}@{d['sha'][:8]}: bare={d['bare_status']} ({d['bare_findings']}) vs scoped={d['scoped_status']} ({d['scoped_findings']})")

    out_dir = Path("benchmarks/results")
    out_dir.mkdir(parents=True, exist_ok=True)
    out_file = out_dir / "linkage_sweep_results.json"
    with open(out_file, "w") as f:
        json.dump({
            "total_commits": total_commits,
            "avg_bare_ms": avg_bare_ms,
            "avg_scoped_ms": avg_scoped_ms,
            "bare_verdicts": dict(bare_verdicts),
            "scoped_verdicts": dict(scoped_verdicts),
            "bare_findings_count": bare_findings_count,
            "scoped_findings_count": scoped_findings_count,
            "discrepancies": discrepancies,
        }, f, indent=2)
    print(f"\nDetailed JSON report saved to: {out_file}")

def main():
    parser = argparse.ArgumentParser(description="Run linkage benchmark sweep.")
    parser.add_argument(
        "--bin",
        type=Path,
        default=Path("target/release/proctor"),
        help="Path to proctor binary",
    )
    parser.add_argument(
        "--audit-root",
        type=Path,
        default=Path("/tmp/proctor-audits"),
        help="Root directory containing audited clone repositories",
    )
    parser.add_argument(
        "--limit",
        type=int,
        default=100,
        help="Max commits per repo",
    )
    args = parser.parse_args()

    bin_path = args.bin.resolve()
    if not bin_path.exists():
        print(f"Error: binary not found at {bin_path}. Run `cargo build --release` first.")
        sys.exit(1)

    run_sweep(bin_path, args.audit_root, limit=args.limit)

if __name__ == "__main__":
    main()
