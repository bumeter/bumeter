#!/usr/bin/env python3
"""Download counts per release, straight from the GitHub Releases API.

Install counting uses only `download_count` on release assets: no telemetry, and
nothing here needs a token (a token only raises the 60 req/h unauthenticated
rate limit).

    ./scripts/downloads.py                 # one line per release, newest first
    ./scripts/downloads.py --assets        # per-asset breakdown
    ./scripts/downloads.py --installers    # ignore updater files, count installs only
    ./scripts/downloads.py --csv           # pastable into a spreadsheet
    ./scripts/downloads.py --json | jq .   # raw payload
    ./scripts/downloads.py --repo user/repo --version v1.0.4

Assets are grouped so the numbers stay readable:
  installers  .exe .msi .deb .dmg .AppImage .rpm — a person downloaded this
  updater     *.sig latest.json *.app.tar.gz — the auto-updater, not a person
  other       anything else (release notes, checksums)
"""

from __future__ import annotations

import argparse
import csv
import json
import os
import sys
import urllib.error
import urllib.request

API = "https://api.github.com"
DEFAULT_REPO = os.environ.get("BUMETER_REPO", "chieund/bumeter")

INSTALLER_SUFFIXES = (".exe", ".msi", ".deb", ".dmg", ".appimage", ".rpm", ".pkg")
UPDATER_SUFFIXES = (".sig", ".tar.gz", ".tar.gz.sig")


def kind(name: str) -> str:
    lowered = name.lower()
    if lowered in ("latest.json", "latest-mac.json"):
        return "updater"
    if lowered.endswith(UPDATER_SUFFIXES):
        return "updater"
    if lowered.endswith(INSTALLER_SUFFIXES):
        return "installers"
    return "other"


def fetch(path: str, token: str | None) -> object:
    request = urllib.request.Request(
        API + path,
        headers={
            "Accept": "application/vnd.github+json",
            "User-Agent": "bumeter-downloads",
            **({"Authorization": f"Bearer {token}"} if token else {}),
        },
    )
    try:
        with urllib.request.urlopen(request, timeout=30) as response:
            return json.load(response)
    except urllib.error.HTTPError as error:
        hint = ""
        if error.code == 403 and not token:
            hint = "\n  Rate limit hit. Wait, or pass GITHUB_TOKEN=... to raise it."
        if error.code == 404:
            hint = f"\n  No such repository: {path.split('/')[2]}"
        sys.exit(f"GitHub API error {error.code}: {error.reason}{hint}")
    except urllib.error.URLError as error:
        sys.exit(f"Cannot reach GitHub: {error.reason}")


def releases(repo: str, token: str | None, wanted: list[str]) -> list[dict]:
    data = fetch(f"/repos/{repo}/releases?per_page=100", token)
    result = [release for release in data if not release["draft"]]
    if wanted:
        lower = {tag.lower().lstrip("v") for tag in wanted}
        result = [r for r in result if r["tag_name"].lower().lstrip("v") in lower]
    return result


def tally(release: dict) -> tuple[dict[str, int], list[dict]]:
    groups = {"installers": 0, "updater": 0, "other": 0}
    assets = []
    for asset in release["assets"]:
        bucket = kind(asset["name"])
        groups[bucket] += asset["download_count"]
        assets.append({"bucket": bucket, **asset})
    return groups, assets


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Download counts per release (GitHub Releases).",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="Assets are grouped" + __doc__.split("Assets are grouped", 1)[1],
    )
    parser.add_argument("--repo", default=DEFAULT_REPO, help="OWNER/NAME (default: %(default)s)")
    parser.add_argument("--token", default=os.environ.get("GITHUB_TOKEN") or os.environ.get("GH_TOKEN"),
                        help="GitHub token, optional (or set GITHUB_TOKEN)")
    parser.add_argument("--version", action="append", default=[], metavar="TAG",
                        help="only these tags, repeatable (v1.0.5 or 1.0.5)")
    parser.add_argument("--assets", action="store_true", help="list every asset")
    parser.add_argument("--installers", action="store_true", help="count installs only, drop updater files")
    parser.add_argument("--csv", action="store_true", help="comma separated output")
    parser.add_argument("--json", action="store_true", help="raw API payload")
    args = parser.parse_args()

    if args.json:
        json.dump(fetch(f"/repos/{args.repo}/releases?per_page=100", args.token), sys.stdout, indent=2)
        print()
        return 0

    rows = []
    grand = {"installers": 0, "updater": 0, "other": 0}
    for release in releases(args.repo, args.token, args.version):
        groups, assets = tally(release)
        for bucket, count in groups.items():
            grand[bucket] += count
        rows.append({"tag": release["tag_name"], "groups": groups, "assets": assets,
                     "prerelease": release["prerelease"], "date": release["published_at"][:10]})

    if not rows:
        print(f"No releases found in {args.repo}.")
        return 1

    if args.csv:
        writer = csv.writer(sys.stdout)
        writer.writerow(["version", "published", "prerelease", "asset", "kind", "downloads"])
        for row in rows:
            for asset in row["assets"]:
                writer.writerow([row["tag"], row["date"], row["prerelease"],
                                 asset["name"], asset["bucket"], asset["download_count"]])
            writer.writerow([row["tag"], row["date"], row["prerelease"], "(total)", "", sum(row["groups"].values())])
        return 0

    columns = ["version", "date", "installers", "updater", "other", "total"]
    table = [columns]
    for row in rows:
        total = row["groups"]["installers"] if args.installers else sum(row["groups"].values())
        table.append([row["tag"], row["date"], str(row["groups"]["installers"]),
                      str(row["groups"]["updater"]), str(row["groups"]["other"]), str(total)])
    width = [max(len(line[i]) for line in table) for i in range(len(columns))]
    for index, line in enumerate(table):
        print("  ".join(cell.ljust(width[i]) for i, cell in enumerate(line)).rstrip())
        if index == 0:
            print("  ".join("-" * w for w in width))

    if args.assets:
        for row in rows:
            print(f"\n{row['tag']}  ({row['date']}{', prerelease' if row['prerelease'] else ''})")
            for asset in sorted(row["assets"], key=lambda a: (-a["download_count"], a["name"])):
                if asset["bucket"] == "other":
                    tag = "  other"
                else:
                    tag = asset["bucket"]
                print(f"  {asset['download_count']:>6}  {tag:<10}  {asset['name']}")

    total_all = sum(grand.values())
    total_install = grand["installers"]
    print(f"\nTotal {total_all} downloads ({total_install} installers, {grand['updater']} updater, "
          f"{grand['other']} other) across {len(rows)} releases.")
    print("Updater files are fetched by the app itself, so they are not installs.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
