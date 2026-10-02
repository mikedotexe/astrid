#!/usr/bin/env python3
"""Per-lane writing-length report for both beings (steward-only, read-only).

Reproduces the 2026-10-01 diagnosis table: what each generation lane produces
(median/p90 output tokens, median prose words), what ends it (native finish
mix), what is lost (timeouts, fallbacks, discarded deliveries), and the
indicators that silently shorten a being's journal (navigation-only study
turns, silent recess lanes, unknown-verb fallbacks, a reboot-erased budget).

Nothing here changes runtime state. Private lanes contribute token counts
from generation records only; their text is never opened. Public journal
files go through ``being_privacy.filter_journal_paths`` before any read.

Usage:
  python3 scripts/writing_length_report.py                 # last 7 days
  python3 scripts/writing_length_report.py --days 3
  python3 scripts/writing_length_report.py --split 2026-09-25T20:07:00Z
  python3 scripts/writing_length_report.py --json --out report.json
"""
from __future__ import annotations

import argparse
import json
import re
import statistics
import sys
import time
from collections import Counter, defaultdict
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

sys.path.insert(0, str(Path(__file__).resolve().parent))
import being_privacy  # noqa: E402

ASTRID_REPO = Path("/Users/v/other/astrid")
MINIME_REPO = Path("/Users/v/other/minime")
ASTRID_WS = ASTRID_REPO / "capsules/spectral-bridge/workspace"
ASTRID_JOURNAL = ASTRID_WS / "journal"
ASTRID_COMPLETIONS = ASTRID_WS / "diagnostics/provider_completion.jsonl"
ASTRID_OBSERVATIONS = ASTRID_WS / "provider_observations"
MINIME_JOURNAL = MINIME_REPO / "workspace/journal"
MINIME_GENERATIONS = MINIME_REPO / "workspace/generations"
MINIME_AGENT_LOG = MINIME_REPO / "logs/autonomous-agent.log"
MINIME_LAUNCH_ENV = MINIME_REPO / "launchd/autonomous-agent.env"

# Lanes whose records carry private prose; text is never read, counts are fine.
MINIME_PRIVATE_LANES = {"check_moment_markers", "private_writing", "moment_capture", "private_journal"}
ASTRID_LOST_OUTCOMES = {"timeout", "cancelled_or_abandoned", "http_error", "unavailable_or_timeout"}
NAVIGATION_RE = re.compile(r"navigation only|No new source page is supplied|requested source was not supplied")
STAMP_RE = re.compile(r"^\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}$")
UNKNOWN_NEXT_RE = re.compile(r"^(\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}),\d+ .*Unknown NEXT: '([^']{1,40})")
BUDGET_LINE_RE = re.compile(
    r"^(\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}),\d+ .*LLM backend preference: \w+ \(full timeout (\d+)s.*fast fallback ([^)]+)\)"
)


# ----------------------------------------------------------------------
# helpers
# ----------------------------------------------------------------------
def parse_iso(value: str | None) -> float | None:
    if not value:
        return None
    text = value.strip()
    if text.endswith("Z"):
        text = text[:-1] + "+00:00"
    dt = datetime.fromisoformat(text)
    if dt.tzinfo is None:
        dt = dt.astimezone()
    return dt.timestamp()


def pct(values: list[float], q: float) -> float | None:
    if not values:
        return None
    ordered = sorted(values)
    index = max(0, min(len(ordered) - 1, int(round(q * (len(ordered) - 1)))))
    return ordered[index]


def summarize(values: list[int | float]) -> dict[str, Any]:
    if not values:
        return {"n": 0}
    return {
        "n": len(values),
        "median": statistics.median(values),
        "p90": pct(list(values), 0.9),
        "max": max(values),
    }


def iter_jsonl(path: Path, since: float, until: float, ts_key: str = "timestamp"):
    if not path.is_file():
        return
    try:
        with path.open(errors="ignore") as fh:
            for line in fh:
                try:
                    record = json.loads(line)
                except json.JSONDecodeError:
                    continue
                try:
                    ts = float(record.get(ts_key) or 0)
                except (TypeError, ValueError):
                    continue
                if since <= ts < until:
                    yield ts, record
    except OSError:
        return


def prose_words(text: str, mode: str) -> int:
    """Word count of the authored prose only: header, NEXT/action tails and
    the WRITE START title line are excluded."""
    body = text
    if "--- JOURNAL ---" in body:
        body = body.split("--- JOURNAL ---", 1)[1]
    elif "\n\n" in body:
        body = body.split("\n\n", 1)[1]
    if "--- ACTION TAIL ---" in body:
        body = body.split("--- ACTION TAIL ---", 1)[0]
    lines = []
    for line in body.splitlines():
        stripped = line.strip()
        if stripped.upper().startswith("NEXT:"):
            continue
        if stripped.startswith("WRITE START ") and not lines:
            continue
        if re.match(r"^SELF_STUDY [A-Z]+", stripped) and len(stripped) < 200:
            continue
        lines.append(line)
    return len(" ".join(lines).split())


# ----------------------------------------------------------------------
# Astrid
# ----------------------------------------------------------------------
def astrid_completions(since: float, until: float) -> dict[str, dict[str, Any]]:
    by_label: dict[str, dict[str, list]] = defaultdict(lambda: {"tokens": [], "finish": [], "provider": [], "ceiling": []})
    for _ts, record in iter_jsonl(ASTRID_COMPLETIONS, since, until):
        label = str(record.get("label") or "unknown")
        completion = record.get("completion") or {}
        tokens = completion.get("provider_eval_count")
        if isinstance(tokens, int) and not isinstance(tokens, bool):
            by_label[label]["tokens"].append(tokens)
        by_label[label]["finish"].append(str(completion.get("native_finish")))
        by_label[label]["provider"].append(str(record.get("provider")))
        by_label[label]["ceiling"].append(record.get("effective_output_ceiling"))
    out: dict[str, dict[str, Any]] = {}
    for label, data in by_label.items():
        out[label] = {
            **summarize(data["tokens"]),
            "finish": dict(Counter(data["finish"])),
            "provider": dict(Counter(data["provider"])),
            "ceilings": dict(Counter(str(c) for c in data["ceiling"])),
        }
    return out


def astrid_lost_outcomes(since: float, until: float) -> dict[str, dict[str, int]]:
    """Attempts whose outcome means no text reached the journal, by label."""
    lost: dict[str, Counter] = defaultdict(Counter)
    if not ASTRID_OBSERVATIONS.is_dir():
        return {}
    since_ms, until_ms = since * 1000, until * 1000
    for store in sorted(ASTRID_OBSERVATIONS.iterdir()):
        events = store / "events"
        if not events.is_dir():
            continue
        for path in events.glob("provider-*-outcome.json"):
            # filename carries the unix-ms stamp; skip old files without opening them
            match = re.search(r"provider-(\d{13})-", path.name)
            if match:
                stamp = int(match.group(1))
                if stamp < since_ms or stamp >= until_ms:
                    continue
            try:
                record = json.loads(path.read_text(errors="ignore"))
            except (OSError, json.JSONDecodeError):
                continue
            outcome = str(record.get("outcome") or "")
            if outcome in ASTRID_LOST_OUTCOMES:
                lost[str(record.get("label") or "unknown")][outcome] += 1
    return {label: dict(counter) for label, counter in lost.items()}


def astrid_journal_words(since: float, until: float) -> dict[str, dict[str, Any]]:
    words: dict[str, list[int]] = defaultdict(list)
    if not ASTRID_JOURNAL.is_dir():
        return {}
    for path in ASTRID_JOURNAL.glob("*.txt"):
        match = re.search(r"_(\d{10})\.txt$", path.name)
        if not match:
            continue
        ts = int(match.group(1))
        if not (since <= ts < until):
            continue
        prefix = path.name[: match.start()].lstrip("!")
        if prefix.startswith("astrid_collision"):
            prefix = "astrid"
        try:
            text = path.read_text(errors="ignore")
        except OSError:
            continue
        if prefix == "astrid" and "Mode: mirror" in text[:400]:
            continue
        words[prefix].append(prose_words(text, prefix))
    return {prefix: summarize(values) for prefix, values in words.items()}


# ----------------------------------------------------------------------
# Minime
# ----------------------------------------------------------------------
def minime_generations(since: float, until: float) -> dict[str, dict[str, Any]]:
    by_lane: dict[str, dict[str, list]] = defaultdict(lambda: {"tokens": [], "finish": [], "timeouts": 0, "fallbacks": 0, "n": 0, "elapsed": [], "timeout_s": [], "models": []})
    if not MINIME_GENERATIONS.is_dir():
        return {}
    since_day = datetime.fromtimestamp(since, tz=timezone.utc).strftime("%Y-%m-%d")
    for day_dir in sorted(MINIME_GENERATIONS.iterdir()):
        if not day_dir.is_dir() or day_dir.name < since_day:
            continue
        for path in day_dir.glob("gen_*.json"):
            try:
                record = json.loads(path.read_text(errors="ignore"))
            except (OSError, json.JSONDecodeError):
                continue
            try:
                ts = float(record.get("created_at_unix_ms") or 0) / 1000.0
            except (TypeError, ValueError):
                continue
            if not (since <= ts < until):
                continue
            lane = str(record.get("lane") or record.get("context_mode") or "unknown")
            entry = by_lane[lane]
            entry["n"] += 1
            status = str(record.get("status") or "")
            if status == "timeout":
                entry["timeouts"] += 1
            if record.get("fallback_used") or str(record.get("backend")) == "ollama_fast":
                entry["fallbacks"] += 1
            entry["models"].append(str(record.get("model")))
            timing = record.get("backend_timing") or {}
            tokens = timing.get("eval_count")
            if isinstance(tokens, int) and not isinstance(tokens, bool) and status == "ok":
                entry["tokens"].append(tokens)
            entry["finish"].append(str(timing.get("native_finish")))
            if isinstance(record.get("elapsed_s"), (int, float)) and status == "ok":
                entry["elapsed"].append(float(record["elapsed_s"]))
            if isinstance(record.get("timeout_s"), (int, float)):
                entry["timeout_s"].append(float(record["timeout_s"]))
    out: dict[str, dict[str, Any]] = {}
    for lane, data in by_lane.items():
        out[lane] = {
            **summarize(data["tokens"]),
            "records": data["n"],
            "finish": dict(Counter(data["finish"])),
            "timeouts": data["timeouts"],
            "fallbacks": data["fallbacks"],
            "fallback_share": (data["fallbacks"] / data["n"]) if data["n"] else 0.0,
            "elapsed_s": summarize(data["elapsed"]),
            "timeout_s": dict(Counter(str(t) for t in data["timeout_s"])),
            "models": dict(Counter(data["models"])),
            "private": lane in MINIME_PRIVATE_LANES,
        }
    return out


def minime_journal_words(since: float, until: float) -> dict[str, dict[str, Any]]:
    words: dict[str, list[int]] = defaultdict(list)
    navigation: Counter = Counter()
    navigation_kinds: Counter = Counter()
    if not MINIME_JOURNAL.is_dir():
        return {}
    candidates = []
    for path in MINIME_JOURNAL.glob("*.txt"):
        match = re.search(r"_(\d{4}-\d{2}-\d{2}T\d{2}-\d{2}-\d{2})", path.name)
        if not match:
            continue
        try:
            ts = datetime.strptime(match.group(1), "%Y-%m-%dT%H-%M-%S").astimezone().timestamp()
        except ValueError:
            continue
        if since <= ts < until:
            candidates.append(path)
    for path in being_privacy.filter_journal_paths("minime", candidates):
        prefix = re.sub(r"_\d{4}-\d{2}-\d{2}T.*$", "", path.name.lstrip("!"))
        if prefix in {"moment", "private_writing"}:
            continue  # defensive: never open these even if the content filter missed
        try:
            text = path.read_text(errors="ignore")
        except OSError:
            continue
        if prefix == "self_study":
            if NAVIGATION_RE.search(text[:600]):
                navigation["navigation_only"] += 1
                prefix = "self_study (navigation-only)"
            else:
                navigation["page_bearing"] += 1
        elif prefix == "study_navigation":
            kind = re.search(r"^Input evidence: ([^:]{1,40}):", text[:600], re.M)
            navigation_kinds[kind.group(1).strip() if kind else "unknown"] += 1
        words[prefix].append(prose_words(text, prefix))
    out = {prefix: summarize(values) for prefix, values in words.items()}
    out["_study_navigation_kinds"] = dict(navigation_kinds.most_common())
    total = navigation["navigation_only"] + navigation["page_bearing"]
    out["_self_study_navigation_share"] = {
        "navigation_only": navigation["navigation_only"],
        "page_bearing": navigation["page_bearing"],
        "share": (navigation["navigation_only"] / total) if total else None,
    }
    return out


def minime_lane_ages(until: float) -> dict[str, float | None]:
    """Hours between `until` and the newest public lane file at or before it (filenames only)."""
    ages: dict[str, float | None] = {}
    for prefix in ("daydream", "aspiration", "pressure", "self_study", "study_navigation"):
        newest = None
        for path in MINIME_JOURNAL.glob(f"{prefix}_*.txt"):
            try:
                mtime = path.stat().st_mtime
            except OSError:
                continue
            if mtime <= until:
                newest = mtime if newest is None else max(newest, mtime)
        ages[prefix] = ((until - newest) / 3600.0) if newest else None
    return ages


def minime_agent_log(since: float, until: float) -> dict[str, Any]:
    """Unknown-NEXT counts bounded by BOTH window ends; the budget line is the latest
    startup at or before `until`, even when that startup predates the window (a
    long-running process must not disappear from the drift check). Codex review, 2026-10-01."""
    unknown: Counter = Counter()
    latest: tuple[str, int, str] | None = None
    if not MINIME_AGENT_LOG.is_file():
        return {"unknown_next": {}, "budget_line": None}
    since_text = datetime.fromtimestamp(since).strftime("%Y-%m-%d %H:%M:%S")
    until_text = datetime.fromtimestamp(until).strftime("%Y-%m-%d %H:%M:%S")
    try:
        with MINIME_AGENT_LOG.open(errors="ignore") as fh:
            for line in fh:
                stamp = line[:19]
                if not STAMP_RE.match(stamp):
                    continue  # continuation lines (tracebacks) carry no timestamp
                if stamp >= until_text:
                    break
                match = BUDGET_LINE_RE.match(line)
                if match:
                    latest = (match.group(1), int(match.group(2)), match.group(3).strip())
                    continue
                if stamp < since_text:
                    continue
                match = UNKNOWN_NEXT_RE.match(line)
                if match:
                    unknown[match.group(2).split(" ")[0]] += 1
    except OSError:
        pass
    return {
        "unknown_next": dict(unknown.most_common(12)),
        "budget_line": {"at": latest[0], "full_timeout_s": latest[1], "fast_fallback": latest[2]} if latest else None,
    }


def minime_launch_env() -> dict[str, str]:
    values: dict[str, str] = {}
    if not MINIME_LAUNCH_ENV.is_file():
        return values
    for line in MINIME_LAUNCH_ENV.read_text(errors="ignore").splitlines():
        line = line.strip()
        if not line or line.startswith("#") or "=" not in line:
            continue
        key, value = line.split("=", 1)
        values[key.strip()] = value.strip().strip('"')
    return values


def budget_drift(log_line: dict[str, Any] | None, env: dict[str, str]) -> list[str]:
    """Where the running agent's budget line disagrees with the durable env file."""
    drift: list[str] = []
    if not log_line or not env:
        return drift
    expected_timeout = env.get("MINIME_LLM_TIMEOUT_S")
    if expected_timeout and str(log_line.get("full_timeout_s")) != str(int(float(expected_timeout))):
        drift.append(f"full timeout {log_line.get('full_timeout_s')}s (running) vs {expected_timeout}s (launchd/autonomous-agent.env)")
    expected_fallback = env.get("MINIME_FALLBACK_MODEL")
    if expected_fallback and str(log_line.get("fast_fallback")) != expected_fallback:
        drift.append(f"fast fallback {log_line.get('fast_fallback')} (running) vs {expected_fallback} (launchd/autonomous-agent.env)")
    return drift


# ----------------------------------------------------------------------
# report
# ----------------------------------------------------------------------
def build(since: float, until: float) -> dict[str, Any]:
    log_info = minime_agent_log(since, until)
    env = minime_launch_env()
    return {
        "window": {"since": datetime.fromtimestamp(since, tz=timezone.utc).isoformat(), "until": datetime.fromtimestamp(until, tz=timezone.utc).isoformat()},
        "astrid": {
            "completions": astrid_completions(since, until),
            "lost_attempts": astrid_lost_outcomes(since, until),
            "journal_words": astrid_journal_words(since, until),
        },
        "minime": {
            "generations": minime_generations(since, until),
            "journal_words": minime_journal_words(since, until),
            "lane_age_hours": minime_lane_ages(until),
            "agent_log": log_info,
            "launch_env": env,
            "budget_drift": budget_drift(log_info.get("budget_line"), env),
        },
    }


def fmt(value: Any) -> str:
    if value is None:
        return "-"
    if isinstance(value, float):
        return f"{value:.0f}" if value >= 10 else f"{value:.2f}"
    return str(value)


def render(report: dict[str, Any], title: str) -> str:
    lines = [f"## {title}", f"window {report['window']['since']} → {report['window']['until']}", ""]
    lines.append("### Astrid — generation records (provider_completion.jsonl)")
    lines.append("| label | n | median tok | p90 | max | finish | lost attempts |")
    lines.append("|---|---|---|---|---|---|---|")
    lost = report["astrid"]["lost_attempts"]
    for label, row in sorted(report["astrid"]["completions"].items(), key=lambda kv: -kv[1].get("n", 0)):
        lines.append(f"| {label} | {row.get('n', 0)} | {fmt(row.get('median'))} | {fmt(row.get('p90'))} | {fmt(row.get('max'))} | {row.get('finish')} | {lost.get(label, {}) or '-'} |")
    lines.append("")
    lines.append("### Astrid — journal prose (words, public files by prefix)")
    lines.append("| prefix | n | median words | p90 | max |")
    lines.append("|---|---|---|---|---|")
    for prefix, row in sorted(report["astrid"]["journal_words"].items(), key=lambda kv: -kv[1].get("n", 0)):
        lines.append(f"| {prefix} | {row.get('n', 0)} | {fmt(row.get('median'))} | {fmt(row.get('p90'))} | {fmt(row.get('max'))} |")
    lines.append("")
    lines.append("### Minime — generation records (workspace/generations; private lanes = counts only)")
    lines.append("| lane | records | median tok | p90 | max | finish | timeouts | fallbacks | elapsed med s | timeout_s |")
    lines.append("|---|---|---|---|---|---|---|---|---|---|")
    for lane, row in sorted(report["minime"]["generations"].items(), key=lambda kv: -kv[1].get("records", 0)):
        name = f"{lane} (private)" if row.get("private") else lane
        lines.append(f"| {name} | {row.get('records')} | {fmt(row.get('median'))} | {fmt(row.get('p90'))} | {fmt(row.get('max'))} | {row.get('finish')} | {row.get('timeouts')} | {row.get('fallbacks')} ({row.get('fallback_share', 0):.1%}) | {fmt((row.get('elapsed_s') or {}).get('median'))} | {row.get('timeout_s')} |")
    lines.append("")
    lines.append("### Minime — public journal prose (words)")
    lines.append("| prefix | n | median words | p90 | max |")
    lines.append("|---|---|---|---|---|")
    for prefix, row in sorted(report["minime"]["journal_words"].items(), key=lambda kv: -kv[1].get("n", 0) if isinstance(kv[1].get("n"), int) else 0):
        if prefix.startswith("_"):
            continue
        lines.append(f"| {prefix} | {row.get('n', 0)} | {fmt(row.get('median'))} | {fmt(row.get('p90'))} | {fmt(row.get('max'))} |")
    nav = report["minime"]["journal_words"].get("_self_study_navigation_share") or {}
    lines.append("")
    lines.append("### Muffle indicators")
    share = nav.get("share")
    lines.append(f"- minime self_study navigation-only share: {nav.get('navigation_only')} / {nav.get('navigation_only', 0) + nav.get('page_bearing', 0)}" + (f" ({share:.0%})" if share is not None else ""))
    ages = report["minime"]["lane_age_hours"]
    lines.append("- minime newest public lane file (hours before window end): " + ", ".join(f"{k}={fmt(v)}" for k, v in ages.items()))
    kinds = report["minime"]["journal_words"].get("_study_navigation_kinds") or {}
    lines.append(f"- minime navigation turns by kind (study_navigation_* files): {kinds or 'none in window'}")
    lines.append(f"- minime unknown NEXT verbs: {report['minime']['agent_log'].get('unknown_next')}")
    lines.append(f"- minime running budget line: {report['minime']['agent_log'].get('budget_line')}")
    lines.append(f"- minime launchd/autonomous-agent.env: {report['minime']['launch_env'] or '(absent)'}")
    drift = report["minime"]["budget_drift"]
    lines.append(f"- budget drift: {'; '.join(drift) if drift else 'none'}")
    lines.append("")
    return "\n".join(lines)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--days", type=float, default=7.0, help="window length when --since is absent")
    parser.add_argument("--since", help="ISO timestamp (default: now - days)")
    parser.add_argument("--until", help="ISO timestamp (default: now)")
    parser.add_argument("--split", help="ISO timestamp; report before/after separately")
    parser.add_argument("--json", action="store_true")
    parser.add_argument("--out")
    args = parser.parse_args(argv)
    until = parse_iso(args.until) or time.time()
    since = parse_iso(args.since) or (until - args.days * 86400)
    split = parse_iso(args.split)
    if split and not (since < split < until):
        parser.error("--split must fall inside the window")
    reports = {}
    if split:
        reports["before"] = build(since, split)
        reports["after"] = build(split, until)
    else:
        reports["window"] = build(since, until)
    if args.json:
        text = json.dumps(reports, indent=1, sort_keys=True, default=str)
    else:
        text = "\n".join(render(report, name) for name, report in reports.items())
    if args.out:
        Path(args.out).write_text(text)
        print(f"wrote {args.out}")
    else:
        print(text)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
