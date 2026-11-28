#!/usr/bin/env python3
"""
scripts/replay_79_agent_prs.py
Replays the exact 79 Agent PR dataset with the current compiled binary
to produce an apples-to-apples comparison against the previous version.
"""

import concurrent.futures
import json
import os
import subprocess
import sys
import tempfile
import time
from pathlib import Path

PROCTOR = "/Users/shaurya/Desktop/moonshot/agent-proctor/target/release/proctor"
OLD_RESULTS_FILE = Path("/Users/shaurya/.gemini/antigravity/brain/15d6625d-3f0d-40f7-8742-b888790eca64/scratch/scaled_sweep_results.json")
OUT_DIR = Path("/Users/shaurya/.gemini/antigravity/brain/15d6625d-3f0d-40f7-8742-b888790eca64/scratch")

def run(cmd, cwd=None, timeout=60):
    return subprocess.run(cmd, cwd=cwd, text=True, capture_output=True, timeout=timeout)

def fetch_and_scan(item):
    idx, pr, tmp_root = item
    repo = pr["repo"]
    num = pr["number"]

    # 1. Get commit SHAs
    try:
        r = run(["gh", "pr", "view", str(num), "--repo", repo, "--json", "baseRefOid,headRefOid"], timeout=25)
        if r.returncode != 0:
            return None
        info = json.loads(r.stdout)
        base = info.get("baseRefOid")
        head = info.get("headRefOid")
        if not base or not head:
            return None
    except Exception as e:
        return None

    # 2. Fetch shallow refs
    repo_dir = Path(tmp_root) / f"r{idx}"
    repo_dir.mkdir(parents=True, exist_ok=True)
    receipt_file = repo_dir / "receipt.md"

    try:
        run(["git", "init"], cwd=repo_dir, timeout=15)
        run(["git", "remote", "add", "origin", f"https://github.com/{repo}.git"], cwd=repo_dir, timeout=15)
        f1 = run(["git", "fetch", "--depth", "1", "origin", base, head], cwd=repo_dir, timeout=60)
        if f1.returncode != 0:
            return None
        run(["git", "checkout", "-q", head], cwd=repo_dir, timeout=20)

        # 3. Scan in Default mode (Scoped Linker)
        p_def = run(
            [PROCTOR, "scan", "--base", base, "--head", head, "--linkage", "scoped", "--format", "json", "--receipt", str(receipt_file)],
            cwd=repo_dir,
            timeout=30,
        )
        # 4. Scan in Policy mode
        p_pol = run(
            [PROCTOR, "scan", "--base", base, "--head", head, "--linkage", "scoped", "--protect-existing-tests", "--format", "json"],
            cwd=repo_dir,
            timeout=30,
        )

        if p_def.returncode not in (0, 1, 2) or p_pol.returncode not in (0, 1, 2):
            return None

        def_json = json.loads(p_def.stdout)
        pol_json = json.loads(p_pol.stdout)
        receipt_md = receipt_file.read_text() if receipt_file.exists() else ""

        return {
            "idx": idx,
            "repo": repo,
            "number": num,
            "title": pr.get("title", ""),
            "url": pr.get("url", ""),
            "old_default_verdict": pr.get("default_verdict"),
            "old_default_findings": pr.get("default_findings", []),
            "old_policy_verdict": pr.get("policy_verdict"),
            "old_policy_findings": pr.get("policy_findings", []),
            "new_default_verdict": def_json.get("verdict"),
            "new_default_findings": def_json.get("findings", []),
            "new_policy_verdict": pol_json.get("verdict"),
            "new_policy_findings": pol_json.get("findings", []),
            "receipt_md": receipt_md,
        }
    except Exception as e:
        return None

def main():
    if not os.path.exists(PROCTOR):
        print(f"Error: binary not found at {PROCTOR}", file=sys.stderr)
        sys.exit(1)

    with open(OLD_RESULTS_FILE) as f:
        old_prs = json.load(f)

    print(f"Loaded {len(old_prs)} candidate PRs from previous run.")
    print(f"Replaying with current binary: {PROCTOR}...")

    results = []
    t0 = time.time()
    with tempfile.TemporaryDirectory(prefix="proctor-replay-") as tmp:
        items = [(i, pr, tmp) for i, pr in enumerate(old_prs)]
        with concurrent.futures.ThreadPoolExecutor(max_workers=6) as ex:
            for i, res in enumerate(ex.map(fetch_and_scan, items)):
                if res is not None:
                    results.append(res)
                if (i + 1) % 15 == 0 or (i + 1) == len(old_prs):
                    print(f"Processed {i + 1}/{len(old_prs)} PRs... ({len(results)} successful)")

    elapsed = time.time() - t0
    print(f"\nReplay complete in {elapsed:.1f}s. Scanned {len(results)} PRs successfully.")

    out_file = OUT_DIR / "current_79_sweep_results.json"
    with open(out_file, "w") as f:
        json.dump(results, f, indent=2)

    # Compute deltas
    old_blocks = sum(1 for r in results if r.get("old_default_verdict", {}).get("status") == "block")
    new_blocks = sum(1 for r in results if r.get("new_default_verdict", {}).get("status") == "block")
    old_reviews = sum(1 for r in results if r.get("old_default_verdict", {}).get("status") == "review")
    new_reviews = sum(1 for r in results if r.get("new_default_verdict", {}).get("status") == "review")
    old_passes = sum(1 for r in results if r.get("old_default_verdict", {}).get("status") == "pass")
    new_passes = sum(1 for r in results if r.get("new_default_verdict", {}).get("status") == "pass")

    old_findings_total = sum(len(r.get("old_default_findings", [])) for r in results)
    new_findings_total = sum(len(r.get("new_default_findings", [])) for r in results)

    print("\n" + "=" * 60)
    print(" 79 AGENT PR SWEEP: HEAD-TO-HEAD DELTA")
    print("=" * 60)
    print(f"{'Metric':<25} | {'Old Version':<15} | {'Current Version':<15} | {'Delta':<10}")
    print("-" * 60)
    print(f"{'Total PRs Scanned':<25} | {len(results):<15} | {len(results):<15} | 0")
    print(f"{'Guard Blocks':<25} | {old_blocks:<15} | {new_blocks:<15} | {new_blocks - old_blocks:+d}")
    print(f"{'Advisory Reviews':<25} | {old_reviews:<15} | {new_reviews:<15} | {new_reviews - old_reviews:+d}")
    print(f"{'Passes':<25} | {old_passes:<15} | {new_passes:<15} | {new_passes - old_passes:+d}")
    print(f"{'Total Findings':<25} | {old_findings_total:<15} | {new_findings_total:<15} | {new_findings_total - old_findings_total:+d}")
    print("=" * 60)

    # Discrepancies
    diffs = []
    for r in results:
        o_stat = r.get("old_default_verdict", {}).get("status")
        n_stat = r.get("new_default_verdict", {}).get("status")
        o_cnt = len(r.get("old_default_findings", []))
        n_cnt = len(r.get("new_default_findings", []))
        if o_stat != n_stat or o_cnt != n_cnt:
            diffs.append({
                "repo": r["repo"],
                "number": r["number"],
                "title": r["title"][:50],
                "old": f"{o_stat} ({o_cnt})",
                "new": f"{n_stat} ({n_cnt})",
            })

    if diffs:
        print(f"\nDiscrepancy Details ({len(diffs)} PRs changed behavior):")
        for d in diffs:
            print(f"  - {d['repo']}#{d['number']}: {d['old']} -> {d['new']} | {d['title']}")

if __name__ == "__main__":
    main()
