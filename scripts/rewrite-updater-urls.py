#!/usr/bin/env python3
"""Rewrite tauri-action API asset URLs in latest.json to public download URLs.

tauri-action writes each platform's `url` as
https://api.github.com/repos/<owner>/<repo>/releases/assets/<id>.
Those endpoints are rate-limited (60 requests/hour unauthenticated) and need
an `Accept: application/octet-stream` header to serve bytes. The identical
assets are served without those limits at
https://github.com/<owner>/<repo>/releases/download/<tag>/<asset-name>.

This rewrite is safe: the minisign signature covers the artifact bytes, not
the URL, so changing URLs cannot invalidate signatures. It is idempotent:
already-rewritten URLs are left untouched, so jobs racing each other (the
release-matrix rewrite and the Flatpak mirror step) converge.

Asset id -> name mapping comes from `gh api` (authenticated in CI via
GH_TOKEN, which also lifts the rate limit for the lookup itself).

Usage:
  rewrite-updater-urls.py <latest.json> --repo OWNER/REPO --tag vX.Y.Z
  rewrite-updater-urls.py <latest.json> --repo OWNER/REPO --tag vX.Y.Z --check

--check makes no changes and exits 1 when any URL still needs rewriting.
"""

import argparse
import json
import re
import subprocess
import sys
from urllib.parse import quote

API_URL_RE = re.compile(
    r"^https://api\.github\.com/repos/([^/]+/[^/]+)/releases/assets/(\d+)/?$"
)


def asset_id_to_name(repo: str, tag: str) -> dict[str, str]:
    out = subprocess.run(
        ["gh", "api", f"repos/{repo}/releases/tags/{tag}",
         "--jq", ".assets[] | \"\\(.id)\\t\\(.name)\""],
        capture_output=True,
        text=True,
        check=False,
    )
    if out.returncode != 0:
        print(f"error: failed to list release assets: {out.stderr.strip()}",
              file=sys.stderr)
        sys.exit(1)
    mapping = {}
    for line in out.stdout.splitlines():
        if not line.strip():
            continue
        asset_id, _, name = line.partition("\t")
        mapping[asset_id] = name
    return mapping


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Rewrite api.github.com asset URLs to "
                    "github.com release download URLs.")
    parser.add_argument("path", help="path to latest.json")
    parser.add_argument("--repo", required=True, help="OWNER/REPO")
    parser.add_argument("--tag", required=True, help="release tag, e.g. v0.0.19")
    parser.add_argument("--check", action="store_true",
                        help="exit 1 if any URL needs rewriting, change nothing")
    args = parser.parse_args()

    with open(args.path, encoding="utf-8") as f:
        data = json.load(f)

    platforms = data.get("platforms", {})
    if not platforms:
        print("error: no platforms found in latest.json", file=sys.stderr)
        return 1

    mapping: dict[str, str] | None = None
    changed = 0
    for target, entry in sorted(platforms.items()):
        url = entry.get("url", "")
        m = API_URL_RE.match(url)
        if not m:
            print(f"keep   {target}: {url}")
            continue
        if mapping is None:
            mapping = asset_id_to_name(args.repo, args.tag)
        asset_id = m.group(2)
        name = mapping.get(asset_id)
        if name is None:
            print(f"error: asset id {asset_id} ({target}) not found in "
                  f"release {args.tag}", file=sys.stderr)
            return 1
        new_url = (f"https://github.com/{args.repo}/releases/download/"
                   f"{args.tag}/{quote(name, safe='')}")
        print(f"rewrite {target}:\n  from: {url}\n  to:   {new_url}")
        entry["url"] = new_url
        changed += 1

    if args.check:
        if changed:
            print(f"check: {changed} URL(s) need rewriting", file=sys.stderr)
            return 1
        print("check: all URLs already use public download form")
        return 0

    if changed:
        with open(args.path, "w", encoding="utf-8") as f:
            json.dump(data, f, indent=2)
            f.write("\n")
    print(f"done: rewrote {changed} URL(s)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
