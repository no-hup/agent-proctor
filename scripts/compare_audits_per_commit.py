#!/usr/bin/env python3
"""Reproducible per-commit `proctor scan` comparison and finding-ID diff across benchmark repos."""

import argparse
import json
import subprocess
import sys
from collections import Counter
from pathlib import Path

DEFAULT_REPOS = ["aider", "SWE-agent", "browser-use", "OpenHands", "litellm"]


def get_commits(repo_dir: Path, limit: int, offset: int) -> list[str]:
    res = subprocess.run(
        ["git", "log", f"--skip={offset}", f"-n{limit}", "--format=%H"],
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


def run_scan_cached(binary: str, repo_dir: Path, sha: str, cache_file: Path) -> dict:
    if cache_file.exists():
        return json.loads(cache_file.read_text())

    res = subprocess.run(
        [binary, "scan", "--base", f"{sha}^", "--head", sha, "--format", "json"],
        cwd=repo_dir,
        capture_output=True,
        text=True,
    )
    if res.returncode not in (0, 1, 2):
        raise RuntimeError(
            f"Unexpected exit code {res.returncode} for {binary} on {repo_dir.name}@{sha}: {res.stderr}"
        )
    data = json.loads(res.stdout)
    cache_file.parent.mkdir(parents=True, exist_ok=True)
    cache_file.write_text(json.dumps(data))
    return data


def summarize_repo(
    repo_root: Path,
    results_dir: Path,
    repo_name: str,
    commits: list[str],
    v1_bin: str,
    v2_bin: str,
    v1_tag: str,
    v2_tag: str,
) -> dict:
    repo_dir = repo_root / repo_name
    v1_verdicts = Counter()
    v2_verdicts = Counter()
    v1_rules = Counter()
    v2_rules = Counter()
    v1_tiers = Counter()
    v2_tiers = Counter()

    added = []
    removed = []
    tier_changed = []
    verdict_changed = []
    all_v2_findings = []

    for sha in commits:
        c1 = results_dir / v1_tag / repo_name / f"{sha}.json"
        c2 = results_dir / v2_tag / repo_name / f"{sha}.json"
        r1 = run_scan_cached(v1_bin, repo_dir, sha, c1)
        r2 = run_scan_cached(v2_bin, repo_dir, sha, c2)

        v1_status = r1["verdict"]["status"]
        v2_status = r2["verdict"]["status"]
        v1_verdicts[v1_status] += 1
        v2_verdicts[v2_status] += 1
        if v1_status != v2_status:
            verdict_changed.append({"sha": sha, "v1": v1_status, "v2": v2_status})

        f1_by_id = {}
        for f in r1.get("findings", []):
            v1_rules[f["rule_id"]] += 1
            v1_tiers[f["tier"]] += 1
            f1_by_id.setdefault(f["id"], []).append(f)

        f2_by_id = {}
        for f in r2.get("findings", []):
            v2_rules[f["rule_id"]] += 1
            v2_tiers[f["tier"]] += 1
            f2_by_id.setdefault(f["id"], []).append(f)
            all_v2_findings.append({"repo": repo_name, "sha": sha, **f})

        for fid, flist in f2_by_id.items():
            if fid not in f1_by_id:
                for f in flist:
                    added.append({"repo": repo_name, "sha": sha, **f})
            else:
                t1 = [x["tier"] for x in f1_by_id[fid]]
                t2 = [x["tier"] for x in flist]
                if t1 != t2:
                    tier_changed.append(
                        {"repo": repo_name, "sha": sha, "id": fid, "v1_tiers": t1, "v2_tiers": t2, "sample": flist[0]}
                    )
                elif len(flist) != len(f1_by_id[fid]):
                    added.append({"repo": repo_name, "sha": sha, "note": f"count_delta {len(f1_by_id[fid])}->{len(flist)}", **flist[0]})

        for fid, flist in f1_by_id.items():
            if fid not in f2_by_id:
                for f in flist:
                    removed.append({"repo": repo_name, "sha": sha, **f})

    return {
        "repo": repo_name,
        "commit_count": len(commits),
        "commits": commits,
        "v1_verdicts": dict(v1_verdicts),
        "v2_verdicts": dict(v2_verdicts),
        "v1_rules": dict(v1_rules),
        "v2_rules": dict(v2_rules),
        "v1_tiers": dict(v1_tiers),
        "v2_tiers": dict(v2_tiers),
        "added_count": len(added),
        "removed_count": len(removed),
        "tier_changed_count": len(tier_changed),
        "verdict_changed_count": len(verdict_changed),
        "added": added,
        "removed": removed,
        "tier_changed": tier_changed,
        "verdict_changed": verdict_changed,
        "all_v2_findings": all_v2_findings,
    }


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--audit-root", type=Path, default=Path("/tmp/proctor-audits"))
    ap.add_argument("--repos", nargs="*", default=DEFAULT_REPOS)
    ap.add_argument("--limit", type=int, default=100)
    ap.add_argument("--offset", type=int, default=0)
    ap.add_argument("--v1-bin", required=True)
    ap.add_argument("--v2-bin", required=True)
    ap.add_argument("--v1-tag", default="v0.1.0")
    ap.add_argument("--v2-tag", default="v0.2.2")
    args = ap.parse_args()

    v1_bin = str(Path(args.v1_bin).resolve()) if "/" in args.v1_bin else args.v1_bin
    v2_bin = str(Path(args.v2_bin).resolve()) if "/" in args.v2_bin else args.v2_bin

    results_dir = args.audit_root / "results"
    summary = {}
    for repo in args.repos:
        repo_dir = args.audit_root / repo
        if not (repo_dir / ".git").exists():
            print(f"[SKIP] {repo} not found at {repo_dir}", file=sys.stderr)
            continue
        commits = get_commits(repo_dir, args.limit, args.offset)
        print(f"Scanning {repo} ({len(commits)} commits, skip={args.offset})...", flush=True)
        summary[repo] = summarize_repo(
            args.audit_root, results_dir, repo, commits, v1_bin, v2_bin, args.v1_tag, args.v2_tag
        )
        s = summary[repo]
        print(
            f"  {args.v1_tag} verdicts: {s['v1_verdicts']} | {args.v2_tag} verdicts: {s['v2_verdicts']}\n"
            f"  {args.v1_tag} rules: {s['v1_rules']} | {args.v2_tag} rules: {s['v2_rules']}\n"
            f"  ID diff: +{s['added_count']} added, -{s['removed_count']} removed, ~{s['tier_changed_count']} tier-changed",
            flush=True,
        )

    results_dir.mkdir(parents=True, exist_ok=True)
    suffix = f"_skip{args.offset}" if args.offset else ""
    out_path = results_dir / f"diff_summary_{args.v2_tag}{suffix}.json"
    out_path.write_text(json.dumps(summary, indent=2))
    print(f"Wrote diff summary to {out_path}")


if __name__ == "__main__":
    main()
