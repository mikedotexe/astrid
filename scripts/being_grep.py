#!/usr/bin/env python3
"""being_grep.py — privacy-safe ad-hoc search across the beings' journals.

The being_privacy bright-line (minime's moment_capture / private_journal flow into NO
steward-review feature) is enforced inside `proactive_scan` (`sample_recent_journals`) —
but ad-hoc steward `grep`s across the journal dirs are NOT filtered, so a hand search can
touch her private lanes (grep reads file content to match, and lists private filenames).
This wraps `being_privacy.filter_journal_paths` so hand searches honor the same line.
(Written 2026-06-22 right after a steward `grep "spectral_spike"` touched her moment_*.)

Read-only. minime's private lanes are EXCLUDED (content-detected); Astrid's moment lane is
accessible by policy, so it is searched.

Usage:
  being_grep.py "spectral_spike"                 # both beings, privacy-safe
  being_grep.py "pressure" --being minime        # minime only (private excluded)
  being_grep.py "lambda4" -l                      # filenames only
  being_grep.py "fraying" --since 2026-06-22      # mtime floor
"""
from __future__ import annotations

import argparse
import re
import sys
from datetime import datetime
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import being_privacy

JOURNALS = {
    "astrid": Path("/Users/v/other/astrid/capsules/spectral-bridge/workspace/journal"),
    "minime": Path("/Users/v/other/minime/workspace/journal"),
}


def search(being: str, pattern: str, names_only: bool, since_ts: float | None) -> int:
    jdir = JOURNALS[being]
    if not jdir.is_dir():
        return 0
    rx = re.compile(pattern, re.IGNORECASE)
    paths = [
        p for p in jdir.glob("*.txt")
        if since_ts is None or p.stat().st_mtime >= since_ts
    ]
    # The bright-line, applied to hand searches: drop minime's private lanes (no-op for Astrid).
    paths = being_privacy.filter_journal_paths(being, paths)
    paths.sort(key=lambda p: p.stat().st_mtime, reverse=True)
    hits = 0
    for p in paths:
        try:
            text = p.read_text(encoding="utf-8", errors="ignore")
        except OSError:
            continue
        matched = [(i, ln) for i, ln in enumerate(text.splitlines(), 1) if rx.search(ln)]
        if not matched:
            continue
        hits += 1
        if names_only:
            print(f"{being}: {p.name}")
        else:
            print(f"\n{being}: {p.name}")
            for i, ln in matched[:6]:
                print(f"  {i}: {ln.strip()[:160]}")
    return hits


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("pattern")
    ap.add_argument("--being", choices=["astrid", "minime", "both"], default="both")
    ap.add_argument("-l", "--names-only", action="store_true")
    ap.add_argument("--since", default=None, help="YYYY-MM-DD mtime floor")
    args = ap.parse_args()

    since_ts = datetime.strptime(args.since, "%Y-%m-%d").timestamp() if args.since else None
    beings = ["astrid", "minime"] if args.being == "both" else [args.being]
    total = sum(search(b, args.pattern, args.names_only, since_ts) for b in beings)
    print(
        f"\n{total} file(s) matched (minime private lanes excluded via being_privacy)",
        file=sys.stderr,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
