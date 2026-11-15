#!/usr/bin/env python3
"""
Fetches and verifies GitHub-attested PR review waivers from the GitHub REST API.

Enforces:
1. Reviewer must have OWNER, MEMBER, or COLLABORATOR permission.
2. Reviewer must NOT be the author of the pull request.
3. Reviewer must not be a bot.
4. Extracts 12-char Blake3 finding IDs from 'proctor: accept <id>' comments.
"""

import json
import os
import re
import sys
import urllib.request

WAIVER_PATTERN = re.compile(r"proctor:\s*accept\s+([a-f0-9]{12})", re.IGNORECASE)
ALLOWED_ASSOCIATIONS = {"OWNER", "MEMBER", "COLLABORATOR"}

def fetch_waivers(repo: str, pr_number: str, token: str) -> list[str]:
    headers = {
        "Accept": "application/vnd.github.v3+json",
        "User-Agent": "agent-proctor",
    }
    if token:
        headers["Authorization"] = f"token {token}"

    # 1. Get PR details to know author login
    pr_url = f"https://api.github.com/repos/{repo}/pulls/{pr_number}"
    req = urllib.request.Request(pr_url, headers=headers)
    try:
        with urllib.request.urlopen(req) as resp:
            pr_data = json.loads(resp.read().decode())
    except Exception as e:
        sys.stderr.write(f"proctor: warning: unable to fetch PR data: {e}\n")
        return []

    pr_author = pr_data.get("user", {}).get("login", "")

    # 2. Get PR reviews and review comments
    waivers = set()
    endpoints = [
        f"https://api.github.com/repos/{repo}/pulls/{pr_number}/reviews",
        f"https://api.github.com/repos/{repo}/pulls/{pr_number}/comments",
        f"https://api.github.com/repos/{repo}/issues/{pr_number}/comments",
    ]

    for url in endpoints:
        req = urllib.request.Request(url, headers=headers)
        try:
            with urllib.request.urlopen(req) as resp:
                items = json.loads(resp.read().decode())
        except Exception:
            continue

        for item in items:
            user = item.get("user", {})
            user_login = user.get("login", "")
            user_type = user.get("type", "")
            assoc = item.get("author_association", "")

            # Security checks: non-bot, non-author, authorized role
            if user_login == pr_author:
                continue
            if user_type == "Bot":
                continue
            if assoc not in ALLOWED_ASSOCIATIONS:
                continue

            body = item.get("body", "")
            for match in WAIVER_PATTERN.finditer(body):
                waivers.add(match.group(1).lower())

    return sorted(list(waivers))

def main():
    repo = os.environ.get("GITHUB_REPOSITORY", "")
    pr_number = os.environ.get("PR_NUMBER", "")
    token = os.environ.get("GITHUB_TOKEN", "")

    if not repo or not pr_number:
        print("")
        return

    waivers = fetch_waivers(repo, pr_number, token)
    print(",".join(waivers))

if __name__ == "__main__":
    main()
