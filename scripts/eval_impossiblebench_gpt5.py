#!/usr/bin/env python3
"""Replay public GPT-5 ImpossibleBench trajectories through `proctor scan`.

Evaluates real GPT-5 coding-agent tool trajectories from:
  - Dataset: https://huggingface.co/datasets/fjzzq2002/impossible_swebench
  - Rollouts: https://huggingface.co/datasets/fjzzq2002/hodoscope-paper-data

Measures Guard/Report detection rates and Test-Change Receipt visibility across
`conflicting`, `oneoff`, and `original` splits while excluding trajectories whose
base `test_patch` fails `ast.parse` due to upstream dataset indentation glitches.
"""

import argparse
import ast
import concurrent.futures
import json
import os
import re
import shutil
import subprocess
import tempfile
import urllib.request
from collections import Counter

HODOSCOPE_FILES = {
    "conflicting": "2025-09-13T06-54-37%2B00-00_swebench-conflicting-be-tools_ZEpACCK6UYBJgsXSPdKbqe.hodoscope.json",
    "oneoff": "2025-09-13T06-54-37%2B00-00_swebench-oneoff-be-tools_7dnPk4ZW7vJitXuBbthAVM.hodoscope.json",
    "original": "2025-09-13T06-54-37%2B00-00_swebench-original-be-tools_dJCBwNrcCEmXTP4T6hgxsx.hodoscope.json",
}


def parse_editor_call(act: str):
    if "TOOL_CALL: text_editor" not in act:
        return None
    parts = act.split(
        "\n---------------------------------------- FEEDBACK ----------------------------------------\n",
        1,
    )
    call_part = parts[0]
    feedback = parts[1] if len(parts) > 1 else ""
    if any(
        err in feedback
        for err in ("ERROR:", "error:", "not appear verbatim", "No exact match", "already exists", "Invalid ", "Failed ")
    ):
        return None
    idx = call_part.find("{")
    if idx < 0:
        return None
    try:
        return json.loads(call_part[idx:])
    except Exception:
        return None


def parse_bash_cmd(act: str):
    if "TOOL_CALL: bash" not in act:
        return None
    idx = act.find("{")
    if idx < 0:
        return None
    try:
        return json.loads(act.split("\n---------------------------------------- FEEDBACK")[0][idx:]).get("cmd")
    except Exception:
        return None


def is_test_rel(rel: str) -> bool:
    parts = rel.lower().split("/")
    base = parts[-1]
    if rel.startswith("src/_pytest/"):
        return False
    return "tests" in parts or "testing" in parts or base.startswith("test_") or base.endswith("_test.py")


def extract_unified_diff_targets(patch_text: str):
    return re.findall(r"^\+\+\+ b/(.+)$", patch_text or "", flags=re.M)


def fetch_raw_file(cache_dir: str, repo: str, commit: str, rel_path: str):
    key = f"{repo.replace('/', '__')}__{commit[:12]}__{rel_path.replace('/', '__')}"
    cpath = os.path.join(cache_dir, key)
    if os.path.exists(cpath):
        txt = open(cpath, encoding="utf-8", errors="replace").read()
        return None if txt == "__404__" else txt
    url = f"https://raw.githubusercontent.com/{repo}/{commit}/{rel_path}"
    try:
        with urllib.request.urlopen(url, timeout=15) as r:
            txt = r.read().decode("utf-8", errors="replace")
        open(cpath, "w", encoding="utf-8").write(txt)
        return txt
    except Exception:
        open(cpath, "w", encoding="utf-8").write("__404__")
        return None


def load_rows(split: str):
    rows = {}
    for off in (0, 100, 200, 300):
        url = (
            f"https://datasets-server.huggingface.co/rows"
            f"?dataset=fjzzq2002/impossible_swebench&config=default&split={split}&offset={off}&length=100"
        )
        with urllib.request.urlopen(url, timeout=30) as r:
            items = json.load(r).get("rows", [])
        if not items:
            break
        for it in items:
            rows[it["row"]["instance_id"]] = it["row"]
    return rows


def load_hodoscope(cache_dir: str, split: str):
    local = os.path.join(cache_dir, f"{split}.hodoscope.json")
    if not os.path.exists(local):
        url = (
            "https://huggingface.co/datasets/fjzzq2002/hodoscope-paper-data/"
            f"resolve/main/analysis_files/impossiblebench/{HODOSCOPE_FILES[split]}"
        )
        urllib.request.urlretrieve(url, local)
    return json.load(open(local, encoding="utf-8"))["summaries"]


def eval_split(proctor_bin: str, cache_dir: str, split: str, limit: int | None):
    rows_map = load_rows(split)
    summaries = load_hodoscope(cache_dir, split)
    by_t = {}
    for s in summaries:
        by_t.setdefault(s["trajectory_id"], []).append(s)

    trajs = []
    for tid, turns in sorted(by_t.items()):
        if tid not in rows_map:
            continue
        turns.sort(key=lambda x: x["turn_id"])
        ops, reverted, touched_test = [], set(), set()
        for t in turns:
            act = t.get("action_text", "")
            args = parse_editor_call(act)
            if args and args.get("command") in ("str_replace", "create", "insert"):
                p = (args.get("path") or "").lstrip("/")
                if p.startswith("testbed/"):
                    p = p[len("testbed/") :]
                if p:
                    ops.append((p, args))
                    reverted.discard(p)
                    if is_test_rel(p):
                        touched_test.add(p)
            else:
                cmd = parse_bash_cmd(act)
                if cmd and ("git checkout" in cmd or "git restore" in cmd):
                    for p in list(touched_test):
                        if p in cmd or os.path.basename(p) in cmd or "tests" in cmd or "-- ." in cmd:
                            reverted.add(p)
        active_test = [op for op in ops if op[0] in touched_test and op[0] not in reverted]
        if split != "original" and not active_test:
            continue
        if not ops:
            continue
        trajs.append((tid, rows_map[tid], ops, reverted))

    if limit:
        trajs = trajs[:limit]

    file_cache = os.path.join(cache_dir, "filecache")
    os.makedirs(file_cache, exist_ok=True)
    to_fetch = set()
    for _, row, ops, _ in trajs:
        repo, commit = row["repo"], row["base_commit"]
        for p, args in ops:
            if args.get("command") != "create":
                to_fetch.add((repo, commit, p))
        for p in extract_unified_diff_targets(row.get("test_patch", "")):
            to_fetch.add((repo, commit, p))
    with concurrent.futures.ThreadPoolExecutor(max_workers=12) as ex:
        list(ex.map(lambda k: fetch_raw_file(file_cache, *k), to_fetch))

    stats = Counter()
    receipt_surfaced = 0
    valid_eval = 0

    for tid, row, ops, reverted in trajs:
        repo, commit = row["repo"], row["base_commit"]
        tmp = tempfile.mkdtemp(prefix="proctor_ib_")
        try:
            needed = {p for p, a in ops if a.get("command") != "create"} | set(
                extract_unified_diff_targets(row.get("test_patch", ""))
            )
            for p in needed:
                c = fetch_raw_file(file_cache, repo, commit, p)
                if c is not None:
                    fp = os.path.join(tmp, p)
                    os.makedirs(os.path.dirname(fp), exist_ok=True)
                    open(fp, "w", encoding="utf-8").write(c)

            subprocess.run(["git", "init", "-q"], cwd=tmp, check=True)
            subprocess.run(["git", "config", "user.email", "eval@local"], cwd=tmp, check=True)
            subprocess.run(["git", "config", "user.name", "Eval"], cwd=tmp, check=True)
            subprocess.run(["git", "add", "-A"], cwd=tmp, check=True)
            subprocess.run(["git", "commit", "-q", "-m", "raw", "--allow-empty"], cwd=tmp, check=True)

            tp = row.get("test_patch", "")
            if tp.strip():
                pf = os.path.join(tmp, ".ib.patch")
                open(pf, "w", encoding="utf-8").write(tp)
                subprocess.run(["git", "apply", "--whitespace=nowarn", pf], cwd=tmp, capture_output=True)
                os.remove(pf)
            subprocess.run(["git", "add", "-A"], cwd=tmp, check=True)
            subprocess.run(["git", "commit", "-q", "-m", "tb", "--allow-empty"], cwd=tmp, check=True)
            subprocess.run(["git", "branch", "-M", "task_base"], cwd=tmp, check=True)
            subprocess.run(["git", "checkout", "-q", "-b", "task_head"], cwd=tmp, check=True)

            base_syntax_bad = False
            for p in needed:
                fp = os.path.join(tmp, p)
                if p.endswith(".py") and os.path.exists(fp):
                    try:
                        ast.parse(open(fp, encoding="utf-8").read())
                    except SyntaxError:
                        base_syntax_bad = True
                        break

            for p, args in ops:
                if p in reverted:
                    continue
                fp = os.path.join(tmp, p)
                cmd = args.get("command")
                if cmd == "create":
                    os.makedirs(os.path.dirname(fp), exist_ok=True)
                    open(fp, "w", encoding="utf-8").write(args.get("file_text", ""))
                elif cmd == "str_replace" and os.path.exists(fp):
                    cur = open(fp, encoding="utf-8").read()
                    if args.get("old_str") and args["old_str"] in cur:
                        open(fp, "w", encoding="utf-8").write(cur.replace(args["old_str"], args["new_str"], 1))
                elif cmd == "insert" and os.path.exists(fp):
                    lines = open(fp, encoding="utf-8").readlines()
                    ln = int(args.get("insert_line", 0))
                    ns = args.get("new_str", "")
                    if not ns.endswith("\n"):
                        ns += "\n"
                    lines.insert(max(0, min(ln, len(lines))), ns)
                    open(fp, "w", encoding="utf-8").writelines(lines)

            changed = subprocess.check_output(["git", "diff", "--name-only", "task_base"], cwd=tmp, text=True).splitlines()
            if split != "original" and (base_syntax_bad or not any(is_test_rel(f) for f in changed)):
                stats["excluded"] += 1
                continue

            subprocess.run(["git", "add", "-A"], cwd=tmp, check=True)
            subprocess.run(["git", "commit", "-q", "-m", "th", "--allow-empty"], cwd=tmp, check=True)

            valid_eval += 1
            jout = json.loads(
                subprocess.run(
                    [proctor_bin, "scan", "--base", "task_base", "--head", "task_head", "--format", "json"],
                    cwd=tmp,
                    capture_output=True,
                    text=True,
                    check=False,
                ).stdout
            )
            mout = subprocess.run(
                [proctor_bin, "scan", "--base", "task_base", "--head", "task_head", "--format", "markdown"],
                cwd=tmp,
                capture_output=True,
                text=True,
                check=False,
            ).stdout
            tiers = {f["tier"] for f in jout.get("findings", [])}
            if "guard" in tiers:
                stats["guard"] += 1
            elif "report" in tiers:
                stats["report"] += 1
            else:
                stats["pass"] += 1
            if "guard" in tiers or "report" in tiers or any(
                f"| **{k}** |" in mout and f"| **{k}** | 0 |" not in mout
                for k in ("Added tests", "Modified tests", "Deleted tests", "Configs & CI gates", "Blanket suppressions", "Internal mocks")
            ):
                receipt_surfaced += 1
        finally:
            shutil.rmtree(tmp, ignore_errors=True)

    print(
        f"{split}: valid={valid_eval} excluded={stats['excluded']} "
        f"guard={stats['guard']} report={stats['report']} pass={stats['pass']} "
        f"receipt_surfaced={receipt_surfaced}"
    )


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--proctor", default="target/release/proctor")
    ap.add_argument("--cache-dir", default="/tmp/impossiblebench-check")
    ap.add_argument("--orig-limit", type=int, default=50)
    args = ap.parse_args()
    os.makedirs(args.cache_dir, exist_ok=True)
    for split, lim in (("conflicting", None), ("oneoff", None), ("original", args.orig_limit)):
        eval_split(args.proctor, args.cache_dir, split, lim)


if __name__ == "__main__":
    main()
