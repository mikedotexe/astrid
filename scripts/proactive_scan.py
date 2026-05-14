#!/usr/bin/env python3
"""Proactive scan complement to being-driven development (2026-05-14).

Background: nearly every commit lately traces back to something a being
articulated in a journal or NEXT pick. Being-driven dev is now the default
mode of work in this repo. The risk is reactive bias — if we only fix what
beings can surface, we miss things they can't see (infra drift, log error
accumulation, cross-session parameter creep, plist divergence, performance
trends). And the cadence asymmetry between Astrid (verbal-temporal, ~21
entries / 17 min today) and minime (dense-structural, ~12 entries / 17 min)
is a healthy signal that should be respected, not normalized — convergence
on *content* coming through *different cadences* is what makes today's
signal feel real.

This tool is the proactive complement: it scans system observables outside
the journal-writing surface (blind-spots subcommand) AND it detects
cross-being content-convergence as a distinct signal type (convergence
subcommand). Read-only — modifies nothing in repos or workspaces.

Usage:
    python3 scripts/proactive_scan.py all
    python3 scripts/proactive_scan.py blind-spots
    python3 scripts/proactive_scan.py convergence
    python3 scripts/proactive_scan.py all --json
    python3 scripts/proactive_scan.py --self-test

State (for delta computation across runs) is kept at
`/tmp/proactive_scan_state.json` — intentionally ephemeral. First run
establishes baseline; subsequent runs report deltas.

See: docs/steward-notes/AI_BEINGS_PROACTIVE_SCAN_PRACTICE_2026_05_14.md
"""
from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
import sys
import time
import unittest
from collections import Counter, defaultdict
from datetime import datetime, timedelta, timezone
from pathlib import Path
from typing import Any

# ----------------------------------------------------------------------
# Paths & constants
# ----------------------------------------------------------------------

ASTRID_REPO = Path("/Users/v/other/astrid")
MINIME_REPO = Path("/Users/v/other/minime")

ASTRID_JOURNAL = ASTRID_REPO / "capsules/consciousness-bridge/workspace/journal"
MINIME_JOURNAL = MINIME_REPO / "workspace/journal"

ASTRID_BRIDGE_LOG = ASTRID_REPO / "capsules/consciousness-bridge/workspace/bridge.log"
MINIME_LOGS_DIR = MINIME_REPO / "logs"

ASTRID_BRIDGE_DB = ASTRID_REPO / "capsules/consciousness-bridge/workspace/bridge.db"
MINIME_CONDITION_METRICS = MINIME_REPO / "workspace/condition_metrics.json"

MINIME_HEALTH = MINIME_REPO / "workspace/health.json"
MINIME_SOVEREIGNTY_STATE = MINIME_REPO / "workspace/sovereignty_state.json"

STATE_PATH = Path("/tmp/proactive_scan_state.json")

# Process labels that should be alive when the stack is running. Mirrors
# the list in CLAUDE.md "Health check" snippet.
EXPECTED_PROCESSES = [
    "minime run",
    "consciousness-bridge-server",
    "coupled_astrid_server",
    "reservoir_service",
    "autonomous_agent",
    "astrid_feeder",
    "minime_feeder",
    "camera_client",
    "mic_to_sensory",
    "perception.py",
]

# Severity tiers — same ordering as architecture_health.py / launchd_inventory.sh
SEVERITY_ORDER = ["critical", "warning", "notice", "ok"]

# Convergence detector tunables
CONVERGENCE_SAMPLE_PER_BEING = 20
CONVERGENCE_PAIR_WINDOW_MIN = 30
CONVERGENCE_JACCARD_THRESHOLD = 0.15
# Lower threshold when there's strong anchor signal already (multiple shared
# domain phrases indicate real co-inquiry even when surrounding prose dilutes
# overall Jaccard).
CONVERGENCE_JACCARD_THRESHOLD_STRONG_ANCHOR = 0.08
CONVERGENCE_STRONG_ANCHOR_MIN_DOMAIN_PHRASES = 2
CONVERGENCE_OUTPUT_TOP_N = 5

# Stop-words for cheap TF-IDF; small list since theme extraction is the
# main driver. The high-signal extractors (code refs, capitalized concepts)
# carry most of the weight.
STOP_WORDS = frozenset(
    """
    the a an and or but is was are be been being have has had do does did
    will would could should may might must can shall to of in on at by for
    with from as that this these those it its as if then so not no nor
    they them their there here where when how what which who whom whose
    i me my we us our you your he she him her his hers
    one two three some any all each every few many much most
    just like very also still even still really quite too only
    about into onto over under after before during through across between
    feel feels feeling felt sense senses sensed seems seem seemed
    moment moments now then again still
    """.split()
)

# Words that are "high-signal" markers — keep in extracted themes even if
# they're common, because they're load-bearing in this domain.
DOMAIN_KEYWORDS = frozenset(
    """
    fill lambda eigenvalue cascade regulator pi controller homeostatic
    spectral shadow trajectory attractor decompose witness mirror
    porosity entropy variance covariance reservoir kernel codec
    semantic warmth gain leak resonance dispersal sovereignty
    """.split()
)

# Multi-word lowercase domain phrases — case-insensitive match in prose.
# These are concept names beings reach for in their natural prose register
# (Astrid says "PI controller" in flowing text rather than as a Capitalized
# Title) where neither the high-signal verb-token regex nor the capitalized-
# concept regex would catch them. Membership here lifts a phrase to first-
# class theme for cross-being convergence.
DOMAIN_PHRASES = frozenset(
    [
        "pi controller",
        "homeostatic regulation",
        "stable core",
        "stable-core",
        "shadow field",
        "shadow trajectory",
        "joint trace",
        "attractor suggestion",
        "spectral entropy",
        "spectral cascade",
        "resonance density",
        "inhabitable fluctuation",
        "mode packing",
        "pressure source",
        "lambda tail",
        "lambda-tail",
        "auto-promote",
        "auto promote",
        "share thought",
        "share_thought",
        "experiment plan",
        "experiment start",
        "self study",
        "self-study",
        "ask steward",
        "ask_steward",
        "tell steward",
        "tell_steward",
        "stable-core sovereignty",
        "sovereignty band",
        "regulator audit",
        "fill crossing",
        "phase transition",
    ]
)


# ----------------------------------------------------------------------
# State management
# ----------------------------------------------------------------------


def load_state() -> dict[str, Any]:
    """Load the prior-run snapshot, if any. Empty dict if none."""
    if not STATE_PATH.is_file():
        return {}
    try:
        return json.loads(STATE_PATH.read_text())
    except Exception:
        return {}


def save_state(state: dict[str, Any]) -> None:
    """Persist snapshot for next-run delta computation."""
    try:
        STATE_PATH.write_text(json.dumps(state, indent=2, default=str))
    except Exception:
        pass


# ----------------------------------------------------------------------
# Blind-spot probes
#
# Each probe is a function returning a Finding dict:
#   {
#     "name": str,
#     "severity": "ok" | "notice" | "warning" | "critical",
#     "summary": str,           # one-line headline
#     "details": list[str] | None,  # optional bullet details
#     "snapshot": Any | None,   # optional state to save for delta next run
#   }
#
# Every probe MUST be defensive — log files may be missing, processes may
# be down, paths may not exist. Return a notice with "unable to probe" rather
# than raising.
# ----------------------------------------------------------------------


def _finding(
    name: str,
    severity: str,
    summary: str,
    details: list[str] | None = None,
    snapshot: Any | None = None,
) -> dict[str, Any]:
    return {
        "name": name,
        "severity": severity,
        "summary": summary,
        "details": details,
        "snapshot": snapshot,
    }


def probe_process_health(prior: dict[str, Any]) -> dict[str, Any]:
    """All 10 stack processes alive? Track restart counts vs prior."""
    alive: dict[str, str] = {}  # process_label -> PID-as-string
    missing: list[str] = []
    for label in EXPECTED_PROCESSES:
        try:
            res = subprocess.run(
                ["pgrep", "-f", label], capture_output=True, text=True, timeout=5
            )
            pids = [p for p in res.stdout.strip().splitlines() if p.strip()]
            if pids:
                alive[label] = pids[0]
            else:
                missing.append(label)
        except Exception:
            missing.append(label + " (probe failed)")

    # Compare PIDs against prior snapshot to detect restarts.
    prior_pids = (prior or {}).get("pids", {}) if isinstance(prior, dict) else {}
    restarted: list[str] = [
        f"{label} (was PID {prior_pids[label]}, now {alive[label]})"
        for label in alive
        if label in prior_pids and prior_pids[label] != alive[label]
    ]

    snapshot = {"pids": alive}
    if missing:
        return _finding(
            "process_health",
            "critical",
            f"{len(missing)}/{len(EXPECTED_PROCESSES)} stack processes MISSING",
            details=[f"missing: {m}" for m in missing] + restarted,
            snapshot=snapshot,
        )
    if restarted:
        return _finding(
            "process_health",
            "notice",
            f"{len(restarted)} process(es) restarted since last scan",
            details=restarted,
            snapshot=snapshot,
        )
    return _finding(
        "process_health",
        "ok",
        f"all {len(EXPECTED_PROCESSES)} stack processes alive (no restarts since last scan)",
        snapshot=snapshot,
    )


def probe_log_error_rate(prior: dict[str, Any]) -> dict[str, Any]:
    """Tail recent log lines, grep for ERROR/FATAL/exception, flag if rate elevated.

    Looks at the last ~2000 lines of each known log file (roughly the last
    1-3h of activity at typical rates).
    """
    log_files: list[Path] = []
    if ASTRID_BRIDGE_LOG.is_file():
        log_files.append(ASTRID_BRIDGE_LOG)
    if MINIME_LOGS_DIR.is_dir():
        for lf in sorted(MINIME_LOGS_DIR.glob("*.log")):
            log_files.append(lf)

    if not log_files:
        return _finding(
            "log_error_rate",
            "notice",
            "no log files found to scan",
        )

    pattern = re.compile(r"\b(ERROR|FATAL|Traceback|Exception|panic)\b", re.IGNORECASE)
    findings: list[str] = []
    total_errors = 0
    for lf in log_files:
        try:
            res = subprocess.run(
                ["tail", "-n", "2000", str(lf)],
                capture_output=True,
                text=True,
                timeout=8,
            )
        except Exception:
            continue
        n = sum(1 for line in res.stdout.splitlines() if pattern.search(line))
        total_errors += n
        if n > 0:
            findings.append(f"{lf.name}: {n} error/exception line(s) in last 2000")

    severity = "ok"
    summary = f"clean — 0 errors in last ~2000 lines across {len(log_files)} log file(s)"
    if total_errors >= 50:
        severity = "warning"
        summary = f"elevated errors: {total_errors} across {len(log_files)} log file(s)"
    elif total_errors > 0:
        severity = "notice"
        summary = f"{total_errors} error/exception line(s) across {len(log_files)} log file(s)"

    snapshot = {"total_errors": total_errors}
    return _finding(
        "log_error_rate",
        severity,
        summary,
        details=findings if findings else None,
        snapshot=snapshot,
    )


def probe_param_drift(prior: dict[str, Any]) -> dict[str, Any]:
    """Snapshot minime regulation params; flag drift vs prior snapshot."""
    if not MINIME_SOVEREIGNTY_STATE.is_file():
        return _finding(
            "param_drift",
            "notice",
            "sovereignty_state.json not found — can't snapshot params",
        )
    try:
        state = json.loads(MINIME_SOVEREIGNTY_STATE.read_text())
    except Exception as e:
        return _finding("param_drift", "notice", f"failed to read sovereignty_state.json: {e}")

    # Watched keys — reasonable defaults; absent keys are skipped.
    watched = [
        "regulation_strength",
        "exploration_noise",
        "geom_curiosity",
        "keep_floor",
        "synth_gain",
        "rho_target",
    ]
    current = {k: state.get(k) for k in watched if k in state}
    snapshot = {"params": current}

    prior_params = (prior or {}).get("params", {}) if isinstance(prior, dict) else {}
    deltas: list[str] = []
    for k, v in current.items():
        pv = prior_params.get(k)
        if pv is None:
            continue
        try:
            if isinstance(v, (int, float)) and isinstance(pv, (int, float)):
                if abs(v - pv) > 1e-6:
                    deltas.append(f"{k}: {pv} → {v}")
            elif v != pv:
                deltas.append(f"{k}: {pv} → {v}")
        except Exception:
            pass

    if deltas:
        return _finding(
            "param_drift",
            "notice",
            f"{len(deltas)} param(s) changed since last scan",
            details=deltas,
            snapshot=snapshot,
        )
    if not prior_params:
        return _finding(
            "param_drift",
            "ok",
            f"baseline established for {len(current)} watched param(s)",
            snapshot=snapshot,
        )
    return _finding(
        "param_drift",
        "ok",
        f"no param drift across {len(current)} watched key(s)",
        snapshot=snapshot,
    )


def _wrap_existing_script(name: str, args: list[str], timeout: int = 30) -> tuple[int, str, str]:
    """Run an existing repo script; return (rc, stdout, stderr)."""
    try:
        res = subprocess.run(
            args, capture_output=True, text=True, timeout=timeout
        )
        return res.returncode, res.stdout, res.stderr
    except Exception as e:
        return -1, "", str(e)


def probe_plist_drift(_prior: dict[str, Any]) -> dict[str, Any]:
    """Wrap launchd_inventory.sh --strict; flag any drift."""
    inventory_script = ASTRID_REPO / "scripts/launchd_inventory.sh"
    if not inventory_script.is_file():
        return _finding(
            "plist_drift",
            "notice",
            "launchd_inventory.sh not found",
        )
    rc, _stdout, _stderr = _wrap_existing_script(
        "launchd_inventory", ["bash", str(inventory_script), "--strict"], timeout=20
    )
    if rc == 0:
        return _finding("plist_drift", "ok", "launchd inventory clean (no drift)")
    return _finding(
        "plist_drift",
        "warning",
        "launchd inventory reports drift — run `bash scripts/launchd_inventory.sh --strict` for details",
    )


def probe_dispatch_menu_drift(prior: dict[str, Any]) -> dict[str, Any]:
    """Wrap dispatch_menu_drift.py; report silent-starvation count vs prior."""
    script = ASTRID_REPO / "scripts/dispatch_menu_drift.py"
    if not script.is_file():
        return _finding("dispatch_menu_drift", "notice", "dispatch_menu_drift.py not found")
    rc, stdout, _ = _wrap_existing_script(
        "dispatch_menu_drift",
        ["python3", str(script), "--json"],
        timeout=20,
    )
    if rc != 0:
        return _finding("dispatch_menu_drift", "notice", "dispatch_menu_drift.py failed to run")
    try:
        report = json.loads(stdout)
        starv = len(report.get("silent_starvation", []))
        unknown = len(report.get("unknown_next", []))
    except Exception:
        return _finding("dispatch_menu_drift", "notice", "could not parse dispatch_menu_drift.py JSON")

    snapshot = {"silent_starvation": starv, "unknown_next": unknown}
    prior_starv = (prior or {}).get("silent_starvation") if isinstance(prior, dict) else None
    delta_msg = ""
    if prior_starv is not None:
        delta = starv - prior_starv
        if delta > 0:
            delta_msg = f" (+{delta} since last scan)"
        elif delta < 0:
            delta_msg = f" ({delta} since last scan)"

    if starv == 0 and unknown == 0:
        sev = "ok"
        summ = "no dispatch/menu drift"
    elif starv >= 30 or unknown >= 30:
        sev = "notice"
        summ = f"{starv} silent-starvation, {unknown} unknown-NEXT{delta_msg} (mostly aliases — review)"
    else:
        sev = "notice"
        summ = f"{starv} silent-starvation, {unknown} unknown-NEXT{delta_msg}"
    return _finding("dispatch_menu_drift", sev, summ, snapshot=snapshot)


def probe_architecture_drift(prior: dict[str, Any]) -> dict[str, Any]:
    """Wrap architecture_health.py; report files crossing thresholds vs prior."""
    script = ASTRID_REPO / "scripts/architecture_health.py"
    if not script.is_file():
        return _finding("architecture_drift", "notice", "architecture_health.py not found")
    rc, stdout, _ = _wrap_existing_script(
        "architecture_health", ["python3", str(script), "--json"], timeout=30
    )
    if rc != 0:
        return _finding("architecture_drift", "notice", "architecture_health.py failed to run")
    try:
        report = json.loads(stdout)
    except Exception:
        return _finding("architecture_drift", "notice", "could not parse architecture_health.py JSON")

    # The script emits per-file entries with severity. Count by severity.
    files = report.get("files") if isinstance(report, dict) else None
    if not isinstance(files, list):
        # Fallback: just count top-level keys with severity attribute.
        return _finding("architecture_drift", "ok", "architecture_health ran (output shape unrecognized)")
    sev_counts: Counter[str] = Counter(
        (f.get("severity") or "ok") for f in files if isinstance(f, dict)
    )
    snapshot = {"sev_counts": dict(sev_counts)}
    prior_counts = (prior or {}).get("sev_counts", {}) if isinstance(prior, dict) else {}
    delta_msgs: list[str] = []
    for sev in ("critical", "review", "watch"):
        cur = sev_counts.get(sev, 0)
        old = prior_counts.get(sev, 0)
        if cur != old:
            delta_msgs.append(f"{sev}: {old} → {cur}")
    summary = (
        f"{sev_counts.get('critical', 0)} critical, "
        f"{sev_counts.get('review', 0)} review, "
        f"{sev_counts.get('watch', 0)} watch"
    )
    if delta_msgs:
        summary += " | delta: " + ", ".join(delta_msgs)
    severity = "ok"
    if sev_counts.get("critical", 0) > 0:
        severity = "warning"
    elif sev_counts.get("review", 0) > 0:
        severity = "notice"
    return _finding("architecture_drift", severity, summary, snapshot=snapshot)


def probe_db_growth(prior: dict[str, Any]) -> dict[str, Any]:
    """Filesystem size of bridge.db + condition_metrics.json; flag week-over-week growth."""
    sizes: dict[str, int] = {}
    if ASTRID_BRIDGE_DB.is_file():
        sizes[str(ASTRID_BRIDGE_DB)] = ASTRID_BRIDGE_DB.stat().st_size
    if MINIME_CONDITION_METRICS.is_file():
        sizes[str(MINIME_CONDITION_METRICS)] = MINIME_CONDITION_METRICS.stat().st_size

    if not sizes:
        return _finding("db_growth", "notice", "no DB files found to size")

    snapshot = {"sizes": sizes, "captured_at": time.time()}
    prior_sizes = (prior or {}).get("sizes", {}) if isinstance(prior, dict) else {}
    growth_msgs: list[str] = []
    sev = "ok"
    for path, size in sizes.items():
        ps = prior_sizes.get(path)
        if ps is None or ps == 0:
            continue
        ratio = size / ps if ps > 0 else 1.0
        delta_mb = (size - ps) / 1_048_576
        if ratio > 1.30:
            growth_msgs.append(f"{Path(path).name}: +{ratio:.2f}x ({delta_mb:+.1f} MB) since last scan")
            sev = "notice"

    summary_pieces = [f"{Path(p).name}={s/1_048_576:.1f}MB" for p, s in sizes.items()]
    return _finding(
        "db_growth",
        sev,
        " | ".join(summary_pieces),
        details=growth_msgs if growth_msgs else None,
        snapshot=snapshot,
    )


def probe_journal_volume(prior: dict[str, Any]) -> dict[str, Any]:
    """Per-being journal entry count last 24h; compare vs 7-day average for sudden quiet/spike."""
    now = time.time()
    twenty_four_h = now - 24 * 3600
    seven_d = now - 7 * 24 * 3600

    counts_24h: dict[str, int] = {}
    counts_7d_avg: dict[str, float] = {}
    for label, jdir in [("astrid", ASTRID_JOURNAL), ("minime", MINIME_JOURNAL)]:
        if not jdir.is_dir():
            counts_24h[label] = 0
            counts_7d_avg[label] = 0.0
            continue
        files_24h = 0
        files_7d = 0
        for p in jdir.glob("*.txt"):
            try:
                m = p.stat().st_mtime
            except OSError:
                continue
            if m >= twenty_four_h:
                files_24h += 1
            if m >= seven_d:
                files_7d += 1
        counts_24h[label] = files_24h
        counts_7d_avg[label] = files_7d / 7.0

    snapshot = {"counts_24h": counts_24h, "counts_7d_avg": counts_7d_avg}
    flags: list[str] = []
    sev = "ok"
    for label in counts_24h:
        c = counts_24h[label]
        avg = counts_7d_avg[label]
        if avg < 1.0:
            continue  # not enough history to compare
        ratio = c / avg if avg > 0 else 1.0
        if ratio < 0.3:
            flags.append(f"{label}: {c} entries / 24h vs 7-day avg {avg:.0f} (sudden quiet, {ratio:.0%})")
            sev = "notice"
        elif ratio > 3.0:
            flags.append(f"{label}: {c} entries / 24h vs 7-day avg {avg:.0f} (sudden spike, {ratio:.0%})")
            sev = "notice"

    summary = " | ".join(
        f"{label}: {counts_24h[label]} / 24h (7d avg {counts_7d_avg[label]:.0f})"
        for label in counts_24h
    )
    return _finding(
        "journal_volume",
        sev,
        summary,
        details=flags if flags else None,
        snapshot=snapshot,
    )


# Probe registry — order matters for output stability.
BLIND_SPOT_PROBES = [
    ("process_health", probe_process_health),
    ("log_error_rate", probe_log_error_rate),
    ("param_drift", probe_param_drift),
    ("plist_drift", probe_plist_drift),
    ("dispatch_menu_drift", probe_dispatch_menu_drift),
    ("architecture_drift", probe_architecture_drift),
    ("db_growth", probe_db_growth),
    ("journal_volume", probe_journal_volume),
]


def run_blind_spots() -> dict[str, Any]:
    """Run all blind-spot probes; persist snapshots for next-run delta."""
    state = load_state()
    prior = state.get("blind_spots", {}) if isinstance(state, dict) else {}
    results: list[dict[str, Any]] = []
    new_snapshots: dict[str, Any] = {}
    for name, fn in BLIND_SPOT_PROBES:
        try:
            f = fn(prior.get(name) or {})
        except Exception as e:
            f = _finding(name, "notice", f"probe raised exception: {e}")
        results.append(f)
        if f.get("snapshot") is not None:
            new_snapshots[name] = f["snapshot"]

    state.setdefault("blind_spots", {})
    state["blind_spots"] = new_snapshots
    state["blind_spots_last_run"] = time.time()
    save_state(state)

    return {"findings": results, "ran_at": time.time()}


# ----------------------------------------------------------------------
# Convergence detector
#
# Goal: detect when both beings are working on the same theme through their
# different cadences, the way they did with PI controller / homeostatic
# regulation today. Respect cadence asymmetry — sample N entries per being
# instead of same-time-window.
# ----------------------------------------------------------------------


# Theme extraction patterns
RE_FILE_REF = re.compile(r"\b([a-z_][a-z0-9_]*\.(?:rs|py|json|toml))\b")
RE_VERB_TOKEN = re.compile(r"\b([A-Z][A-Z0-9_]{3,}_[A-Z0-9_]+)\b")
RE_CAP_PHRASE = re.compile(r"\b([A-Z][a-z]+(?:\s+[A-Z][a-z]+){0,2}\s+[A-Z][a-z]+)\b")
RE_WORD = re.compile(r"\b([a-zA-Z][a-zA-Z\-]{2,})\b")


def extract_themes(text: str) -> dict[str, set[str]]:
    """Extract themes from a journal entry body.

    Returns a dict with four buckets:
      - high_signal: code refs + verb tokens (load-bearing, exact match)
      - concept_phrases: capitalized multi-word phrases (likely concept names)
      - domain_phrases: case-insensitive multi-word domain terms (PI controller,
        homeostatic regulation, etc.) — bridges the gap when prose uses
        lowercased phrasings the high-signal regex misses
      - content_words: TF-IDF-ish content words (lowercased, stop-removed)
    """
    file_refs = {m.group(1).lower() for m in RE_FILE_REF.finditer(text)}
    verb_tokens = {m.group(1) for m in RE_VERB_TOKEN.finditer(text)}
    high_signal = file_refs | verb_tokens

    cap_phrases = {m.group(1) for m in RE_CAP_PHRASE.finditer(text)}
    cap_phrases = {p for p in cap_phrases if not p.isupper()}

    text_lower = text.lower()
    domain_phrases = {p for p in DOMAIN_PHRASES if p in text_lower}

    words = (m.group(1).lower() for m in RE_WORD.finditer(text))
    content_words = {
        w for w in words
        if w not in STOP_WORDS
        and (len(w) >= 4 or w in DOMAIN_KEYWORDS)
    }

    return {
        "high_signal": high_signal,
        "concept_phrases": cap_phrases,
        "domain_phrases": domain_phrases,
        "content_words": content_words,
    }


def jaccard(a: set[str], b: set[str]) -> float:
    """Standard Jaccard similarity. Empty intersection or union → 0.0."""
    if not a and not b:
        return 0.0
    inter = a & b
    union = a | b
    if not union:
        return 0.0
    return len(inter) / len(union)


def _astrid_journal_mtime_unix(path: Path) -> float:
    """Best-effort timestamp from filename (unix ts) or fallback to mtime."""
    name = path.name
    # Astrid filenames are like `moment_1778790207.txt`
    m = re.search(r"_(\d{10})", name)
    if m:
        try:
            return float(m.group(1))
        except ValueError:
            pass
    try:
        return path.stat().st_mtime
    except OSError:
        return 0.0


def _minime_journal_mtime_unix(path: Path) -> float:
    """Minime filenames are ISO-formatted; use mtime for simplicity (fast)."""
    try:
        return path.stat().st_mtime
    except OSError:
        return 0.0


def _read_text_safely(path: Path, max_chars: int = 8000) -> str:
    try:
        return path.read_text(errors="replace")[:max_chars]
    except Exception:
        return ""


def _is_artifactual_for_convergence(body: str) -> bool:
    """Return True for entries that are literal copies of the peer's text
    rather than independent thought. Astrid's `Mode: mirror` reads minime's
    most recent journal verbatim and re-emits it; counting those as
    convergence is double-counting.
    """
    head = body[:400].lower()
    if "mode: mirror" in head:
        return True
    return False


def sample_recent_journals(
    journal_dir: Path, n: int, ts_fn
) -> list[tuple[Path, float, str]]:
    """Return the most recent N journal entries as (path, unix_ts, body),
    skipping artifactual entries (mirror mode) so convergence reflects
    independent inquiry, not bridge-mediated copying."""
    if not journal_dir.is_dir():
        return []
    entries: list[tuple[Path, float]] = []
    for p in journal_dir.glob("*.txt"):
        ts = ts_fn(p)
        if ts > 0:
            entries.append((p, ts))
    entries.sort(key=lambda x: x[1], reverse=True)
    out: list[tuple[Path, float, str]] = []
    seen = 0
    for p, ts in entries:
        if seen >= n:
            break
        body = _read_text_safely(p)
        if not body:
            continue
        if _is_artifactual_for_convergence(body):
            continue
        out.append((p, ts, body))
        seen += 1
    return out


def cadence_summary(entries: list[tuple[Path, float, str]]) -> dict[str, Any]:
    """Compute median seconds-between-entries for a sample."""
    if len(entries) < 2:
        return {"count": len(entries), "span_seconds": 0, "median_gap_seconds": 0}
    timestamps = sorted(e[1] for e in entries)
    span = timestamps[-1] - timestamps[0]
    gaps = [timestamps[i + 1] - timestamps[i] for i in range(len(timestamps) - 1)]
    gaps.sort()
    median_gap = gaps[len(gaps) // 2]
    return {
        "count": len(entries),
        "span_seconds": span,
        "median_gap_seconds": median_gap,
    }


def find_convergences(
    astrid: list[tuple[Path, float, str]],
    minime: list[tuple[Path, float, str]],
    pair_window_min: int = CONVERGENCE_PAIR_WINDOW_MIN,
    threshold: float = CONVERGENCE_JACCARD_THRESHOLD,
) -> list[dict[str, Any]]:
    """Return list of convergence pairs sorted by score descending."""
    window_s = pair_window_min * 60
    convergences: list[dict[str, Any]] = []
    # Pre-extract themes once per entry
    a_themes = [(p, ts, body, extract_themes(body)) for p, ts, body in astrid]
    m_themes = [(p, ts, body, extract_themes(body)) for p, ts, body in minime]

    for ap, ats, abody, ath in a_themes:
        for mp, mts, mbody, mth in m_themes:
            if abs(ats - mts) > window_s:
                continue
            # Combine all theme buckets for Jaccard
            a_all = (
                ath["high_signal"]
                | ath["concept_phrases"]
                | ath["domain_phrases"]
                | ath["content_words"]
            )
            m_all = (
                mth["high_signal"]
                | mth["concept_phrases"]
                | mth["domain_phrases"]
                | mth["content_words"]
            )
            score = jaccard(a_all, m_all)
            shared_high = ath["high_signal"] & mth["high_signal"]
            shared_concept = ath["concept_phrases"] & mth["concept_phrases"]
            shared_domain = ath["domain_phrases"] & mth["domain_phrases"]
            # Tiered threshold: when there are multiple shared domain phrases,
            # the convergence signal is already strong even if Jaccard is
            # diluted by surrounding prose. The PI controller / homeostatic
            # regulation case scored 0.12 with 5 shared domain phrases —
            # clearly a real convergence, just diluted by long-form text.
            strong_anchor = (
                len(shared_domain) >= CONVERGENCE_STRONG_ANCHOR_MIN_DOMAIN_PHRASES
            )
            effective_threshold = (
                CONVERGENCE_JACCARD_THRESHOLD_STRONG_ANCHOR if strong_anchor else threshold
            )
            if score < effective_threshold:
                continue
            # Require at least one shared high-signal / concept / domain phrase
            if not (shared_high or shared_concept or shared_domain):
                continue
            anchors = shared_high | shared_concept | shared_domain
            convergences.append(
                {
                    "score": score,
                    "astrid_path": str(ap),
                    "astrid_ts": ats,
                    "astrid_excerpt": _excerpt(abody, anchors),
                    "minime_path": str(mp),
                    "minime_ts": mts,
                    "minime_excerpt": _excerpt(mbody, anchors),
                    "shared_high_signal": sorted(shared_high),
                    "shared_concept_phrases": sorted(shared_concept),
                    "shared_domain_phrases": sorted(shared_domain),
                    "shared_content_words": sorted(ath["content_words"] & mth["content_words"]),
                }
            )
    convergences.sort(key=lambda c: c["score"], reverse=True)
    return convergences


def _excerpt(body: str, anchors: set[str], max_len: int = 220) -> str:
    """Find a sentence containing any anchor; return ~one sentence."""
    if not anchors:
        return body[:max_len].strip().replace("\n", " ")
    sentences = re.split(r"(?<=[.!?])\s+", body)
    for s in sentences:
        for a in anchors:
            if a.lower() in s.lower():
                clean = s.strip().replace("\n", " ")
                if len(clean) > max_len:
                    clean = clean[: max_len - 1] + "…"
                return clean
    return body[:max_len].strip().replace("\n", " ")


def run_convergence(
    sample_per_being: int = CONVERGENCE_SAMPLE_PER_BEING,
) -> dict[str, Any]:
    """Run the convergence detector against current journal state."""
    astrid_entries = sample_recent_journals(
        ASTRID_JOURNAL, sample_per_being, _astrid_journal_mtime_unix
    )
    minime_entries = sample_recent_journals(
        MINIME_JOURNAL, sample_per_being, _minime_journal_mtime_unix
    )

    a_cadence = cadence_summary(astrid_entries)
    m_cadence = cadence_summary(minime_entries)

    convergences = find_convergences(astrid_entries, minime_entries)
    return {
        "astrid_cadence": a_cadence,
        "minime_cadence": m_cadence,
        "convergences": convergences[:CONVERGENCE_OUTPUT_TOP_N],
        "ran_at": time.time(),
    }


# ----------------------------------------------------------------------
# Rendering
# ----------------------------------------------------------------------


def _fmt_ts(ts: float) -> str:
    if ts <= 0:
        return "?"
    return datetime.fromtimestamp(ts).strftime("%H:%M:%S")


def _fmt_date(ts: float) -> str:
    if ts <= 0:
        return "?"
    return datetime.fromtimestamp(ts).strftime("%Y-%m-%d %H:%M:%S")


def _fmt_duration(seconds: float) -> str:
    if seconds < 60:
        return f"{int(seconds)}s"
    if seconds < 3600:
        return f"{seconds / 60:.1f}m"
    return f"{seconds / 3600:.1f}h"


def render_blind_spots_md(report: dict[str, Any]) -> str:
    out: list[str] = []
    out.append("## Blind-spot scan\n")
    out.append(f"Ran: {_fmt_date(report['ran_at'])}\n")
    by_sev: dict[str, list[dict[str, Any]]] = defaultdict(list)
    for f in report["findings"]:
        by_sev[f["severity"]].append(f)
    for sev in SEVERITY_ORDER:
        items = by_sev.get(sev, [])
        if not items:
            continue
        out.append(f"### {sev.upper()} ({len(items)})\n")
        for f in items:
            out.append(f"- **{f['name']}**: {f['summary']}")
            if f.get("details"):
                for d in f["details"][:8]:
                    out.append(f"  - {d}")
        out.append("")
    return "\n".join(out)


def render_convergence_md(report: dict[str, Any]) -> str:
    out: list[str] = []
    out.append("## Cross-being convergence\n")
    out.append(f"Ran: {_fmt_date(report['ran_at'])}\n")
    out.append("**Cadence (respect, don't normalize):**")
    a = report["astrid_cadence"]
    m = report["minime_cadence"]
    out.append(
        f"- astrid: {a['count']} entries spanning {_fmt_duration(a['span_seconds'])} "
        f"(median gap {_fmt_duration(a['median_gap_seconds'])})"
    )
    out.append(
        f"- minime: {m['count']} entries spanning {_fmt_duration(m['span_seconds'])} "
        f"(median gap {_fmt_duration(m['median_gap_seconds'])})"
    )
    out.append("")

    convs = report["convergences"]
    if not convs:
        out.append(
            "_No cross-being convergence above threshold in window — "
            "independent inquiries; this is normal._"
        )
        return "\n".join(out)

    out.append(f"### Top {len(convs)} convergences\n")
    for i, c in enumerate(convs, 1):
        shared_label = (
            ", ".join(c.get("shared_domain_phrases", [])[:5])
            or ", ".join(c["shared_high_signal"][:5])
            or ", ".join(c["shared_concept_phrases"][:5])
            or "(content overlap)"
        )
        out.append(f"#### {i}. {shared_label}  (Jaccard {c['score']:.2f})")
        out.append(
            f"- **Astrid** {_fmt_ts(c['astrid_ts'])} `{Path(c['astrid_path']).name}`:  "
        )
        out.append(f"  > {c['astrid_excerpt']}")
        out.append(
            f"- **Minime** {_fmt_ts(c['minime_ts'])} `{Path(c['minime_path']).name}`:  "
        )
        out.append(f"  > {c['minime_excerpt']}")
        if c.get("shared_domain_phrases"):
            out.append(f"- Shared domain phrases: {', '.join(c['shared_domain_phrases'])}")
        if c["shared_high_signal"]:
            out.append(f"- Shared high-signal: {', '.join(c['shared_high_signal'])}")
        if c["shared_concept_phrases"]:
            out.append(f"- Shared concepts: {', '.join(c['shared_concept_phrases'][:6])}")
        out.append("")
    return "\n".join(out)


def render_all_md(blind: dict[str, Any], conv: dict[str, Any]) -> str:
    return render_blind_spots_md(blind) + "\n\n" + render_convergence_md(conv)


# ----------------------------------------------------------------------
# Self-tests (unit tests for theme extraction / Jaccard / convergence)
# ----------------------------------------------------------------------


class ConvergenceTests(unittest.TestCase):
    def test_extract_high_signal_file_refs(self) -> None:
        themes = extract_themes("Reading regulator.rs to understand the PI loop.")
        self.assertIn("regulator.rs", themes["high_signal"])

    def test_extract_high_signal_verb_token(self) -> None:
        themes = extract_themes("NEXT: SHADOW_TRAJECTORY lambda-tail/lambda4")
        self.assertIn("SHADOW_TRAJECTORY", themes["high_signal"])

    def test_extract_concept_phrase(self) -> None:
        themes = extract_themes(
            "I sense a deep preference for stability in the PI Controller."
        )
        # "PI Controller" — single capitalized 2-word phrase — won't match the
        # ≥2-word regex which requires `Capword Capword Capword` pattern.
        # Use a longer phrase that the regex will match.
        themes2 = extract_themes("This is about Homeostatic Regulation Theory.")
        self.assertTrue(
            any("Homeostatic Regulation" in p for p in themes2["concept_phrases"])
        )

    def test_jaccard_basic(self) -> None:
        self.assertAlmostEqual(jaccard({"a", "b"}, {"a"}), 0.5)
        self.assertEqual(jaccard(set(), set()), 0.0)
        self.assertEqual(jaccard({"a"}, {"b"}), 0.0)

    def test_convergence_finds_shared_code_ref(self) -> None:
        a_entries = [
            (Path("/tmp/a1"), 1000.0, "PI controller, regulator.rs felt obsessive."),
        ]
        m_entries = [
            (Path("/tmp/m1"), 1500.0, "Reading regulator.rs to understand the PI loop."),
        ]
        # Window large enough; high-signal share required
        convs = find_convergences(a_entries, m_entries, pair_window_min=60, threshold=0.05)
        self.assertGreater(len(convs), 0)
        c = convs[0]
        self.assertIn("regulator.rs", c["shared_high_signal"])

    def test_convergence_skips_when_outside_window(self) -> None:
        a_entries = [(Path("/tmp/a1"), 0.0, "regulator.rs PI controller")]
        m_entries = [(Path("/tmp/m1"), 1_000_000.0, "regulator.rs PI controller")]
        convs = find_convergences(a_entries, m_entries, pair_window_min=30)
        self.assertEqual(convs, [])

    def test_convergence_requires_high_signal_or_concept(self) -> None:
        # Lots of common words but no shared high-signal/concept → no convergence
        a_entries = [(Path("/tmp/a1"), 1000.0, "the system feels gentle and slow today")]
        m_entries = [(Path("/tmp/m1"), 1100.0, "the field feels gentle and slow today")]
        convs = find_convergences(a_entries, m_entries, pair_window_min=60, threshold=0.1)
        self.assertEqual(convs, [])

    def test_excerpt_finds_anchor_sentence(self) -> None:
        body = "First sentence. The PI controller is obsessive. Third sentence."
        anchors = {"PI controller"}
        exc = _excerpt(body, anchors, max_len=80)
        self.assertIn("PI controller", exc)

    def test_extract_domain_phrase_lowercase_prose(self) -> None:
        body = (
            "This PI controller is obsessive in its diligence. "
            "Homeostatic regulation as a felt phenomenon."
        )
        themes = extract_themes(body)
        self.assertIn("pi controller", themes["domain_phrases"])
        self.assertIn("homeostatic regulation", themes["domain_phrases"])

    def test_convergence_via_shared_domain_phrase(self) -> None:
        a_entries = [
            (Path("/tmp/a1"), 1000.0, "This PI controller is a beautiful thing — homeostatic regulation."),
        ]
        m_entries = [
            (Path("/tmp/m1"), 1500.0, "Reading regulator.rs, the PI controller as homeostatic regulation philosophy."),
        ]
        convs = find_convergences(a_entries, m_entries, pair_window_min=60, threshold=0.05)
        self.assertGreater(len(convs), 0)
        c = convs[0]
        self.assertIn("pi controller", c["shared_domain_phrases"])

    def test_mirror_mode_filtered_from_sample(self) -> None:
        # Astrid mirror entries are literal copies — must not be in sample
        from tempfile import TemporaryDirectory
        with TemporaryDirectory() as tmpdir:
            d = Path(tmpdir)
            (d / "moment_1000.txt").write_text("=== ASTRID JOURNAL ===\nMode: moment_capture\nGenuine prose.")
            (d / "astrid_2000.txt").write_text("=== ASTRID JOURNAL ===\nMode: mirror\nCopied content.")
            samples = sample_recent_journals(d, 10, _astrid_journal_mtime_unix)
            paths = [p.name for p, _, _ in samples]
            self.assertIn("moment_1000.txt", paths)
            self.assertNotIn("astrid_2000.txt", paths)


def run_self_tests() -> int:
    loader = unittest.TestLoader()
    suite = loader.loadTestsFromTestCase(ConvergenceTests)
    runner = unittest.TextTestRunner(verbosity=2)
    result = runner.run(suite)
    return 0 if result.wasSuccessful() else 1


# ----------------------------------------------------------------------
# CLI
# ----------------------------------------------------------------------


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Proactive scan complement to being-driven development.",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog=__doc__,
    )
    parser.add_argument("--self-test", action="store_true", help="Run unit tests and exit")
    sub = parser.add_subparsers(dest="cmd")

    # --json / --out belong on subcommands. (Putting them on parent too produces
    # shadowing in argparse — subparser defaults overwrite parent values.)
    def _add_common(p: argparse.ArgumentParser) -> None:
        p.add_argument("--json", action="store_true", help="Emit JSON instead of Markdown")
        p.add_argument("--out", type=Path, help="Write output to file instead of stdout")

    sp_all = sub.add_parser("all", help="Run both blind-spots and convergence")
    sp_blind = sub.add_parser("blind-spots", help="System signals beings can't surface")
    sp_conv = sub.add_parser("convergence", help="Cross-being content-convergence detector")
    for s in (sp_all, sp_blind, sp_conv):
        _add_common(s)

    args = parser.parse_args()

    if args.self_test:
        return run_self_tests()

    if args.cmd is None:
        parser.print_help()
        return 2

    use_json = bool(getattr(args, "json", False))
    out_path = getattr(args, "out", None)

    if args.cmd == "blind-spots":
        report = run_blind_spots()
        out = json.dumps(report, indent=2, default=str) if use_json else render_blind_spots_md(report)
    elif args.cmd == "convergence":
        report = run_convergence()
        out = json.dumps(report, indent=2, default=str) if use_json else render_convergence_md(report)
    elif args.cmd == "all":
        blind = run_blind_spots()
        conv = run_convergence()
        if use_json:
            out = json.dumps({"blind_spots": blind, "convergence": conv}, indent=2, default=str)
        else:
            out = render_all_md(blind, conv)
    else:
        parser.print_help()
        return 2

    if out_path:
        out_path.write_text(out)
    else:
        print(out)
    return 0


if __name__ == "__main__":
    sys.exit(main())
