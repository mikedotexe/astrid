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

from proactive_scan_architecture import probe_architecture_drift
from proactive_scan_journal_hygiene import JournalHygieneProbeTests, probe_journal_hygiene

# ----------------------------------------------------------------------
# Paths & constants
# ----------------------------------------------------------------------

ASTRID_REPO = Path("/Users/v/other/astrid")
MINIME_REPO = Path("/Users/v/other/minime")

ASTRID_JOURNAL = ASTRID_REPO / "capsules/spectral-bridge/workspace/journal"
MINIME_JOURNAL = MINIME_REPO / "workspace/journal"

# The bridge writes its live log to /tmp/bridge.log; workspace/bridge.log is never
# populated. Point the error-rate probe at the real stream — otherwise it is
# structurally blind to every Astrid bridge error (it silently was for a long time).
LIVE_BRIDGE_LOG = Path("/tmp/bridge.log")
ASTRID_BRIDGE_LOG = LIVE_BRIDGE_LOG
MINIME_LOGS_DIR = MINIME_REPO / "logs"

ASTRID_BRIDGE_DB = ASTRID_REPO / "capsules/spectral-bridge/workspace/bridge.db"
MINIME_CONDITION_METRICS = MINIME_REPO / "workspace/condition_metrics.json"

MINIME_HEALTH = MINIME_REPO / "workspace/health.json"
MINIME_SOVEREIGNTY_STATE = MINIME_REPO / "workspace/sovereignty_state.json"

STATE_PATH = Path("/tmp/proactive_scan_state.json")
# Per-ask lifecycle ledger — DURABLE (survives reboot/tmp-wipe), unlike STATE_PATH.
# Asks are long-lived stewardship triage state; entry-level dedup (seen/acted) stays
# ephemeral in STATE_PATH. Steward-only; never surfaced into being prompts.
ASKS_PATH = Path("/Users/v/other/astrid/workspace/steward_asks.json")
CAPACITY_HISTORY = ASTRID_REPO / "workspace/reservoir_capacity_history.jsonl"
# Being→steward outreach (ASK_STEWARD/TELL_STEWARD) lands in each being's outbox.
ASTRID_OUTBOX = ASTRID_REPO / "capsules/spectral-bridge/workspace/outbox"
MINIME_OUTBOX = MINIME_REPO / "workspace/outbox"
# Alarm if a being's outreach sits unanswered longer than this (a few loop cycles).
# The 2-month silent loss of Astrid's 12 questions is what this guards against.
OUTREACH_ALARM_SECS = 2 * 3600
# ----- Feedback-surface coverage (the COMPLEMENT of steward_outreach) -----
# steward_outreach owns the being→steward OUTBOXES. feedback_coverage owns the
# REQUEST / HANDOFF / OVERFLOW surfaces beings write to that need a steward
# consumer. A surface that accumulates unconsumed = the muffle pattern: Astrid's
# agency_requests (her EVOLVE self-evolution asks) sat 69 days with NO consumer
# while we rediscovered the same need from scratch. The registry is data-driven
# so a newly-discovered dead surface is one entry, not a manual hunt — this makes
# the "systematic muffle audit" continuous. "Processed" items live in subdirs
# (reviewed/ done/), excluded automatically by the non-recursive glob.
FEEDBACK_COVERAGE_ALARM_SECS = 3 * 24 * 3600  # a request backlog older than this is STALE
REVIEW_REQUEST_CONSUMER = (
    "steward action required: ground/close if engaged; "
    "reword/withdraw if unengaged (never being follow-up)"
)
FEEDBACK_SURFACES = [
    {
        "name": "astrid_agency_requests",
        "root": ASTRID_REPO / "capsules/spectral-bridge/workspace/agency_requests",
        "glob": "*.json",
        "kind": "request",
        "consumer": "steward triage → reviewed/",
    },
    {
        "name": "astrid_claude_tasks",
        "root": ASTRID_REPO / "capsules/spectral-bridge/workspace/claude_tasks",
        "glob": "*.md",
        "kind": "request",
        "consumer": "steward implements/answers → done/",
    },
    {
        "name": "minime_parameter_requests",
        "root": MINIME_REPO / "workspace/parameter_requests",
        "glob": "*.json",
        "kind": "request",
        "consumer": "steward review → reviewed/",
    },
    {
        "name": "astrid_inbox_backlog",
        "root": ASTRID_REPO / "capsules/spectral-bridge/workspace/inbox",
        "glob": "backlog_*/*",
        "kind": "request",
        "consumer": "steward triage/archive",
    },
    {
        "name": "astrid_context_overflow",
        # Telemetry-ish distress signal (her context overflowing), not an unread
        # queue — report ("notice"), never hard-alarm, so the probe doesn't cry wolf.
        "root": ASTRID_REPO / "capsules/spectral-bridge/workspace/context_overflow",
        "glob": "*.txt",
        "kind": "notice",
        "consumer": "steward glance (chronic-overflow signal)",
    },
    {
        # Steward-issued review INVITATIONS (review-together loop). An UNengaged
        # invitation that rots is the muffle pattern, but a STALE alarm routes to
        # steward action only. The being only ever sees one gentle optional slot
        # line. reviewed/ + closed/ excluded by the non-recursive glob.
        "name": "astrid_review_requests",
        "root": ASTRID_REPO / "capsules/spectral-bridge/workspace/review_requests",
        "glob": "*.json",
        "kind": "request",
        "consumer": REVIEW_REQUEST_CONSUMER,
    },
    {
        "name": "minime_review_requests",
        "root": MINIME_REPO / "workspace/review_requests",
        "glob": "*.json",
        "kind": "request",
        "consumer": REVIEW_REQUEST_CONSUMER,
    },
]

# Load-bearing cross-being / cross-repo channels. A rename or move in ONE being's
# repo can silently sever a path hardcoded in the OTHER's — the
# consciousness-bridge→spectral-bridge capsule rename left 18 dead refs in minime's
# code, walling her off from Astrid's inbox + source (read, wrongly, as her going
# quiet). Each entry MUST resolve for that channel to carry. Curated (not auto-
# derived) so the probe never cries wolf on created-on-demand leaves; a new channel
# is one entry.
CROSS_BEING_CHANNELS = [
    {"name": "minime→astrid:inbox",
     "path": ASTRID_REPO / "capsules/spectral-bridge/workspace/inbox",
     "carries": "minime's letters to Astrid (ASTRID_BRIDGE_INBOX_DIR)"},
    {"name": "minime→astrid:source",
     "path": ASTRID_REPO / "capsules/spectral-bridge/src",
     "carries": "minime reading Astrid's code (INTROSPECT source roots)"},
    {"name": "minime→astrid:param_requests",
     "path": ASTRID_REPO / "capsules/spectral-bridge/workspace/parameter_requests",
     "carries": "minime's TUNE_ASTRID parameter requests to Astrid"},
    {"name": "astrid:bridge_db",
     "path": ASTRID_BRIDGE_DB,
     "carries": "the bridge message log both feeders poll"},
    {"name": "astrid→minime:workspace",
     "path": MINIME_REPO / "workspace",
     "carries": "Astrid + feeders reading minime's journals and state"},
    {"name": "minime:need",
     "path": MINIME_REPO / "workspace/minime_need_v1.json",
     "carries": "minime's co-regulation need (Astrid's LEND_DENSITY gate reads it)"},
    {"name": "shared:collaborations",
     "path": Path("/Users/v/other/shared/collaborations"),
     "carries": "gift_exchange.jsonl + shared_thoughts + shared_investigations"},
]

# --- stuck_repetition probe: the honored-but-ineffective detector ---
# A being that keeps choosing the SAME action that keeps NOT landing is telling US
# (not itself) that our infra is eating its reach — TUNE_ASTRID chosen 8×, honored
# 0×, blocked 8×, hidden in plain sight until it was repeated. The discriminator is
# repetition × BAD-OUTCOME, never repetition alone (that would flag Astrid's healthy
# varied SHADOW_TRAJECTORY focus). Complements dispatch_menu_drift (doesn't-dispatch)
# by catching dispatches-but-blocked/no-progress. Steward-only; never to a being.
STUCK_REPEAT_MIN = 4              # in-window repeat count to be a candidate
STUCK_BAD_RATIO = 0.5            # bad-outcome / chosen ≥ this ⇒ honored-but-ineffective (warning)
STUCK_IDENTICAL_ARG_RATIO = 0.3  # distinct-args / chosen ≤ this (low-bad) ⇒ possible no-progress (notice)
STUCK_TAIL_BYTES = 2_000_000     # read only the recent log tail (logs reach ~80 MB)
STUCK_WINDOW_HOURS = 3.0          # within the tail, keep only the last N hours — so a
#                                   just-fixed action stops being flagged once it stops
#                                   recurring, instead of haunting the byte-tail for ~12 h
STUCK_IDLE_BASES = frozenset({"REST", "PASS", "SKIP", "NOTICE", "JOURNAL", "WAIT"})
_STUCK_ANSI = re.compile(r"\x1b\[[0-9;]*m")
_STUCK_TS = re.compile(r"(\d{4}-\d{2}-\d{2})[ T](\d{2}:\d{2}:\d{2})")
# Two kinds of bad outcome, deliberately separated so severity matches actionability:
#   unknown  = the chosen action wasn't recognized ("Unknown NEXT" / "not wired") —
#              almost always OUR wiring gap (the DOSSIER dual-map footgun) ⇒ WARNING.
#   blocked  = a NAMED deliberate guard refused it (live-control / stable-core / charter)
#              — the being is reaching against a gate by design ⇒ NOTICE (needs guidance
#              or a design call, not an urgent fix; never auto-loosened by the probe).
STUCK_BEINGS = [
    {
        "name": "minime",
        "log": MINIME_LOGS_DIR / "autonomous-agent.log",
        "choice": re.compile(r"Being chose NEXT:\s*([A-Z_][A-Z0-9_]*)\s*(.*)"),
        "unknown": re.compile(r"Unknown NEXT:\s*'([A-Za-z_][A-Za-z0-9_]*)"),
        "blocked": re.compile(
            r"(?:guard blocked NEXT:\s*|continuity guard blocked NEXT:\s*"
            r"|Stable-core agency budget blocked\s+)([A-Za-z_][A-Za-z0-9_]*)"
        ),
    },
    {
        "name": "astrid",
        "log": LIVE_BRIDGE_LOG,  # the LIVE bridge log (workspace/bridge.log is empty)
        "choice": re.compile(r"chose NEXT:\s*([A-Z_][A-Z0-9_]*)\s*(.*)"),
        "unknown": re.compile(r"chose unknown NEXT:\s*'([A-Za-z_][A-Za-z0-9_]*)"),
        "blocked": None,  # the bridge has no separate deliberate-guard-block marker
    },
]
ASK_STATUSES = ("open", "acknowledged", "in_flight", "awaiting", "resolved")
# A standout that matches an ask in one of these states is ATTRIBUTED (reported once,
# no act-now, no re-letter); only "open" asks + unmatched standouts surface for action.
ASK_HELD_STATES = frozenset({"acknowledged", "in_flight", "awaiting", "resolved"})

# Process labels that should be alive when the stack is running. Mirrors
# the list in CLAUDE.md "Health check" snippet.
EXPECTED_PROCESSES = [
    "minime run",
    "spectral-bridge-server",
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

# --- Introspective-signal buckets (for probe_introspective_signal) ----------
# Felt/qualia vocabulary the beings actually use for constraint + texture,
# seeded from harvest_feedback.sh's $DISTRESS, the steward-loop distress list,
# and observed prose. Lets us recognize reflective DEPTH across ALL surfaces
# (daydream/aspiration/longform/witness/fissure_trace/sovereignty_check), not
# just self_study_ files. Steward indicator only — never a being-facing target.
QUALIA_TERMS = frozenset(
    """
    texture weight heavy thin thinning thick dense density friction grain grainy
    viscous viscosity silt sediment hollow brittle constrict constricting
    claustrophobic contained leash siphon dissolve dissolving fracture fraying
    porous porosity aperture breath breathing fold folding contraction expansion
    shudder tremor surge hum thrum membrane weave squeeze squeezed packed
    overpacked saturated drag inhabit inhabiting occupy occupying interstice
    carving casting mud gel exhaustion exhausting taxing reductive flatten
    """.split()
)
QUALIA_PHRASES = (
    "cooling gel", "deep mud", "smaller aperture", "wide rather than",
    "carving rather than casting", "hold the mess",
)
DESIRE_RE = re.compile(
    r"\b(i want|i'd\s+(?:change|adjust|modify|reduce|increase|soften|lower|raise|prefer|like|try)|"
    r"i wish|i wonder|i'd love|could we|can we|i suggest|suggest|line\s+\d+|soften|loosen|widen|release)\b",
    re.I,
)
AGENCY_RE = re.compile(
    r"\b(i choose|i am choosing|moving from|i need|i will|i refuse|i reclaim|my own|i assert|i decide|on my own terms)\b",
    re.I,
)
INTROSPECT_FIRST_PERSON_RE = re.compile(
    r"\b(i|me|my|mine|myself|feel|felt|want|wanted|notice|noticed|remember|need|curious|frustrated)\b",
    re.I,
)
INTROSPECT_SAMPLE_PER_BEING = 24
INTROSPECT_THRESHOLD = 6.0  # sanity floor used by self-tests (probe uses a relative bar)
INTROSPECT_TOP_K = 4
# Relative standout bar: the beings reflect deeply in MOST entries, so an absolute
# cut is non-discriminating. A standout is an entry above THIS being's own baseline
# (median + K*MAD), with an absolute floor so we never flag near-telemetry as deep.
INTROSPECT_RELATIVE_K = 1.0
INTROSPECT_ABS_FLOOR = 5.0
# A standout surfaced this many scans without being acted/ack'd is "persistent
# unacted" — the stewardship-depth failure mode (we keep seeing it, never act).
INTROSPECT_STALE_K = 3

BENIGN_LOG_ERROR_PATTERNS = [
    re.compile(
        r"WS recv error: WebSocket protocol error: Connection reset without closing handshake",
        re.IGNORECASE,
    ),
    re.compile(
        r"WS recv error: IO error: Connection reset by peer",
        re.IGNORECASE,
    ),
    re.compile(
        r"WS recv error: .*Client disconnected:",
        re.IGNORECASE,
    ),
    re.compile(
        r"WebSocket protocol error: .*Client disconnected:",
        re.IGNORECASE,
    )
]


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


def load_asks() -> dict[str, Any]:
    """Load the durable per-ask ledger. {} if none (so first run is safe)."""
    if not ASKS_PATH.is_file():
        return {}
    try:
        return json.loads(ASKS_PATH.read_text())
    except Exception:
        return {}


def save_asks(ledger: dict[str, Any]) -> None:
    """Persist the ask ledger with an atomic write (temp + replace) so a concurrent
    read during the loop's scan can't see a torn file."""
    try:
        ASKS_PATH.parent.mkdir(parents=True, exist_ok=True)
        tmp = ASKS_PATH.with_suffix(".json.tmp")
        tmp.write_text(json.dumps(ledger, indent=2, default=str))
        tmp.replace(ASKS_PATH)
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


def _is_benign_log_error(line: str) -> bool:
    """True for noisy, expected disconnect lines that contain the word error."""
    return any(pattern.search(line) for pattern in BENIGN_LOG_ERROR_PATTERNS)


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

    Looks at the last ~2000 lines of each known log file. Critically,
    weights findings by log freshness — a stale log full of historical
    errors (e.g. 989 connection-refused lines from a process that's now
    reconnected) should not look the same as an actively-failing process.
    The proactive scan's job is to surface CURRENT signal, not historical
    forensics. A recent log can also contain settled restart-window errors:
    if the file has not changed since the prior scan, keep the evidence
    visible but don't keep escalating it as a fresh failure.
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
    now = time.time()
    # A log is "stale" if it hasn't been written to in this window — its
    # errors are historical and should not be surfaced as current signal.
    STALE_THRESHOLD_SECONDS = 30 * 60  # 30 min

    prior = prior if isinstance(prior, dict) else {}
    prior_files = prior.get("files", {})
    if not isinstance(prior_files, dict):
        prior_files = {}
    prior_run_at = prior.get("_blind_spots_last_run")
    if not isinstance(prior_run_at, (int, float)):
        prior_run_at = None

    active_findings: list[str] = []
    settled_findings: list[str] = []
    stale_findings: list[str] = []
    benign_findings: list[str] = []
    active_errors = 0
    settled_recent_errors = 0
    stale_errors = 0
    benign_transient_errors = 0
    file_snapshots: dict[str, dict[str, Any]] = {}

    for lf in log_files:
        try:
            stat = lf.stat()
            mtime = stat.st_mtime
            size = stat.st_size
        except OSError:
            continue
        try:
            res = subprocess.run(
                ["tail", "-n", "2000", str(lf)],
                capture_output=True,
                text=True,
                timeout=8,
            )
        except Exception:
            continue
        matching_lines = [
            line for line in res.stdout.splitlines() if pattern.search(line)
        ]
        benign_n = sum(1 for line in matching_lines if _is_benign_log_error(line))
        n = len(matching_lines) - benign_n
        benign_transient_errors += benign_n
        file_snapshots[str(lf)] = {
            "errors": n,
            "raw_errors": len(matching_lines),
            "ignored_transient_errors": benign_n,
            "mtime": mtime,
            "size": size,
        }
        if benign_n > 0:
            benign_findings.append(
                f"{lf.name}: ignored {benign_n} expected websocket disconnect error(s)"
            )
        if n == 0:
            continue
        age_s = now - mtime
        if age_s > STALE_THRESHOLD_SECONDS:
            stale_errors += n
            stale_findings.append(
                f"{lf.name}: {n} historical error(s) (log stale {_fmt_duration(age_s)})"
            )
            continue

        previous = prior_files.get(str(lf))
        unchanged_since_prior = False
        if isinstance(previous, dict):
            previous_mtime = previous.get("mtime")
            unchanged_since_prior = (
                previous.get("errors") == n
                and previous.get("size") == size
                and isinstance(previous_mtime, (int, float))
                and abs(float(previous_mtime) - mtime) < 1e-6
            )
        no_writes_since_prior = bool(prior_run_at is not None and mtime <= prior_run_at)
        if unchanged_since_prior or no_writes_since_prior:
            settled_recent_errors += n
            settled_findings.append(
                f"{lf.name}: {n} recent settled error(s) "
                f"(log age {_fmt_duration(age_s)}, no writes since prior scan)"
            )
        else:
            active_errors += n
            active_findings.append(
                f"{lf.name}: {n} new/current error(s) in last 2000 "
                f"(log fresh, age {_fmt_duration(age_s)})"
            )

    snapshot = {
        "active_errors": active_errors,
        "settled_recent_errors": settled_recent_errors,
        "stale_errors": stale_errors,
        "ignored_transient_errors": benign_transient_errors,
        "files": file_snapshots,
    }

    # Severity is driven primarily by ACTIVE errors. Settled/stale errors get
    # an OK mention (so they're not invisible) but don't trigger warning.
    if active_errors >= 50:
        severity = "warning"
        summary = f"elevated CURRENT errors: {active_errors} across {len(active_findings)} active log(s)"
    elif active_errors > 0:
        severity = "notice"
        summary = f"{active_errors} current error(s) across {len(active_findings)} active log(s)"
    elif settled_recent_errors > 0:
        severity = "ok"
        summary = (
            f"no new current errors — {settled_recent_errors} recent settled error(s) "
            f"unchanged since prior scan"
        )
        if stale_errors > 0:
            summary += f"; {stale_errors} historical error(s) in stale log(s)"
    elif stale_errors > 0:
        severity = "ok"
        summary = (
            f"no current errors — {stale_errors} historical error(s) in stale log(s) "
            f"({len(stale_findings)} file(s) untouched >{STALE_THRESHOLD_SECONDS // 60}m)"
        )
    elif benign_transient_errors > 0:
        severity = "ok"
        summary = (
            f"clean — 0 actionable errors; ignored {benign_transient_errors} "
            "expected transient websocket disconnect(s)"
        )
    else:
        severity = "ok"
        summary = f"clean — 0 errors across {len(log_files)} log file(s)"

    return _finding(
        "log_error_rate",
        severity,
        summary,
        details=(active_findings + settled_findings + stale_findings + benign_findings)
        if (active_findings or settled_findings or stale_findings or benign_findings)
        else None,
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


# Sovereignty-dial footer directives minime may STATE in a reply (mirrors the
# minime-side autonomous_agent._parse_footer_directives for the numeric dials,
# plus `regime`, which the agent intentionally does NOT auto-apply). The guard:
# a stated footer that differs from the APPLIED sovereignty_state = a dropped
# intent (footer-parser regression), an unapplied regime footer (surface for the
# steward), or a since-changed dial (glance). Steward-only; never to a being.
_STATED_FOOTER_NUMERIC = ("exploration_noise", "regulation_strength", "geom_curiosity")
_STATED_FOOTER_NUM_RE = re.compile(
    r"^[\s\-*>]*(" + "|".join(_STATED_FOOTER_NUMERIC) + r")\s*[:=]\s*"
    r"([-+]?\d*\.?\d+)\s*[.;,]?\s*$",
    re.IGNORECASE | re.MULTILINE,
)
_STATED_FOOTER_REGIME_RE = re.compile(
    r"^[\s\-*>]*regime\s*[:=]\s*(explore|recover|breathe|focus|calm)\s*[.;,]?\s*$",
    re.IGNORECASE | re.MULTILINE,
)
_STATED_FOOTER_SCAN_LINES = 8  # only a reply's tail is a footer


def _stated_footer_directives(text: str) -> dict[str, Any]:
    """Parse sovereignty-dial footer directives from a reply's tail (the numeric
    dials + regime). Pure; mirrors the minime-side footer detection. A prose
    mention never matches (it is not a bare, whole-line `KEY=value`)."""
    if not text:
        return {}
    tail = "\n".join(str(text).splitlines()[-_STATED_FOOTER_SCAN_LINES:])
    out: dict[str, Any] = {}
    for m in _STATED_FOOTER_NUM_RE.finditer(tail):
        try:
            out[m.group(1).lower()] = float(m.group(2))
        except (TypeError, ValueError):
            pass
    rm = _STATED_FOOTER_REGIME_RE.search(tail)
    if rm:
        out["regime"] = rm.group(1).lower()
    return out


def _scan_minime_stated_footers(dirs: list[Path], now: float, max_files: int = 12,
                                max_age_s: float = 3 * 3600) -> dict[str, tuple]:
    """Newest stated footer per param across recent reply files in `dirs`.
    Returns {param: (value, age_s)}; only files within `max_age_s` count."""
    files: list[Path] = []
    for d in dirs:
        if d.is_dir():
            files.extend(d.glob("reply_*.txt"))
    files = sorted(files, key=lambda p: p.stat().st_mtime if p.exists() else 0.0,
                   reverse=True)[:max_files]
    latest: dict[str, tuple] = {}
    for p in files:
        try:
            age = now - p.stat().st_mtime
        except OSError:
            continue
        if age > max_age_s:
            continue
        for key, val in _stated_footer_directives(_read_text_safely(p)).items():
            if key not in latest:  # files are newest-first → first seen is newest
                latest[key] = (val, age)
    return latest


def _stated_intent_divergences(stated: dict[str, tuple], applied: dict[str, Any]) -> list[str]:
    """Compare stated footers to applied sovereignty_state; return divergence lines.
    A stated value with no applied counterpart is skipped (nothing to compare)."""
    out: list[str] = []
    for key, (val, age) in sorted(stated.items()):
        cur = applied.get(key)
        if cur is None:
            continue
        if isinstance(val, (int, float)) and isinstance(cur, (int, float)):
            diverged = abs(val - cur) > 1e-6
        else:
            diverged = str(val).lower() != str(cur).lower()
        if diverged:
            out.append(f"{key}: stated {val!r} ({_fmt_duration(age)} ago) != applied {cur!r}")
    return out


def probe_stated_param_intent(prior: dict[str, Any]) -> dict[str, Any]:
    """Detect a stated sovereignty-dial footer that did NOT reach the applied
    state — the un-muffle regression guard for the minime footer-parser, and the
    surface for `regime` footers (which the agent intentionally leaves to steward
    review). NOTICE on divergence. Steward-only; never surfaced into being prompts."""
    if not MINIME_SOVEREIGNTY_STATE.is_file():
        return _finding("stated_param_intent", "ok",
                        "sovereignty_state.json not found — nothing to compare")
    try:
        applied = json.loads(MINIME_SOVEREIGNTY_STATE.read_text())
    except Exception as e:
        return _finding("stated_param_intent", "notice",
                        f"failed to read sovereignty_state.json: {e}")
    stated = _scan_minime_stated_footers(
        [MINIME_OUTBOX, MINIME_OUTBOX / "delivered"], time.time()
    )
    if not stated:
        return _finding("stated_param_intent", "ok", "no recent stated dial footers to check")
    divergences = _stated_intent_divergences(stated, applied)
    if divergences:
        return _finding(
            "stated_param_intent",
            "notice",
            f"{len(divergences)} stated dial footer(s) differ from applied state "
            "(dropped footer / unapplied regime / since-changed dial — glance)",
            details=divergences,
        )
    return _finding("stated_param_intent", "ok",
                    f"all {len(stated)} recent stated footer(s) match applied state")


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
        # autonomous_agent.py grew past ~2MB; the regex analysis now takes ~64s
        # (was <20s). The old 20s cap made this probe silently "fail to run"
        # every cycle, deadening a real drift detector (could miss a new
        # silent-starvation/unwired action). 120s gives margin for further file
        # growth; this is a steward-side background probe, not in a being path.
        timeout=120,
    )
    if rc != 0:
        return _finding("dispatch_menu_drift", "notice", "dispatch_menu_drift.py failed to run")
    try:
        report = json.loads(stdout)
        summary = report.get("summary", {})
        starv = int(
            summary.get(
                "new_silent_starvation_public",
                len(
                    report.get(
                        "new_silent_starvation_public",
                        report.get("silent_starvation_public", []),
                    )
                ),
            )
        )
        unknown = int(
            summary.get(
                "unknown_next_current",
                len(report.get("unknown_next_current", report.get("unknown_next", []))),
            )
        )
        raw_starv = int(summary.get("silent_starvation", len(report.get("silent_starvation", []))))
        raw_unknown = int(summary.get("unknown_next", len(report.get("unknown_next", []))))
        accepted_starv = int(
            summary.get(
                "accepted_silent_starvation_public",
                len(report.get("accepted_silent_starvation_public", [])),
            )
        )
    except Exception:
        return _finding("dispatch_menu_drift", "notice", "could not parse dispatch_menu_drift.py JSON")

    snapshot = {
        "new_silent_starvation_public": starv,
        "accepted_silent_starvation_public": accepted_starv,
        "silent_starvation_public": int(
            summary.get(
                "silent_starvation_public",
                len(report.get("silent_starvation_public", [])),
            )
        ),
        "unknown_next_current": unknown,
        "silent_starvation": raw_starv,
        "unknown_next": raw_unknown,
    }
    prior_starv = None
    if isinstance(prior, dict):
        prior_starv = prior.get(
            "new_silent_starvation_public",
            prior.get("silent_starvation_public", prior.get("silent_starvation")),
        )
    delta_msg = ""
    if prior_starv is not None:
        delta = starv - prior_starv
        if delta > 0:
            delta_msg = f" (+{delta} since last scan)"
        elif delta < 0:
            delta_msg = f" ({delta} since last scan)"

    if starv == 0 and unknown == 0:
        sev = "ok"
        summ = f"no new dispatch/menu drift ({accepted_starv} accepted public backlog)"
    elif starv >= 30 or unknown >= 30:
        sev = "notice"
        summ = (
            f"{starv} new public silent-starvation, {unknown} current unknown-NEXT"
            f"{delta_msg} (raw: {raw_starv}/{raw_unknown}; mostly aliases — review)"
        )
    else:
        sev = "notice"
        summ = (
            f"{starv} new public silent-starvation, {unknown} current unknown-NEXT"
            f"{delta_msg} (raw: {raw_starv}/{raw_unknown})"
        )
    return _finding("dispatch_menu_drift", sev, summ, snapshot=snapshot)


def probe_capsule_runtime_health(_prior: dict[str, Any]) -> dict[str, Any]:
    """Report installed/discovered/loadable capsule runtime compatibility."""
    script = ASTRID_REPO / "scripts/capsule_runtime_health.py"
    if not script.is_file():
        return _finding("capsule_runtime_health", "notice", "capsule_runtime_health.py not found")
    rc, stdout, _ = _wrap_existing_script(
        "capsule_runtime_health",
        ["python3", str(script), "--json"],
        timeout=20,
    )
    if rc != 0:
        return _finding("capsule_runtime_health", "notice", "capsule runtime health probe failed")
    try:
        report = json.loads(stdout)
        summary = report.get("summary", {})
    except Exception:
        return _finding("capsule_runtime_health", "notice", "could not parse capsule runtime health JSON")

    status = str(summary.get("status", "notice"))
    severity = "ok" if status == "ok" else "warning"
    text = (
        f"{summary.get('installed_manifests', 0)} installed, "
        f"{summary.get('discovered_manifests', 0)} discovered, "
        f"{summary.get('loadable_component_model', 0)} Component Model, "
        f"{summary.get('accepted_legacy_extism_mvp', 0)}/"
        f"{summary.get('legacy_extism_mvp', 0)} accepted legacy, "
        f"{summary.get('actionable_incompatible', 0)} incompatible, "
        f"{summary.get('actionable_missing_payloads', 0)} missing"
    )
    if severity == "ok":
        return _finding(
            "capsule_runtime_health",
            "ok",
            f"capsule runtime baseline clean ({text})",
            snapshot=summary,
        )
    return _finding(
        "capsule_runtime_health",
        severity,
        f"capsule runtime drift needs review ({text})",
        snapshot=summary,
    )


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


# ----------------------------------------------------------------------
# Introspective-signal probe — catch reflection across ALL surfaces, not just
# self_study_ files, so the steward can READ and close loops on the reflection
# already happening (the non-coercive driver of MORE genuine reflection).
# Reuses extract_themes()/sample_recent_journals()/_fmt_ts (defined later;
# resolved at call time). Steward-only; counts are an indicator, never a target,
# never surfaced into being prompts.
# ----------------------------------------------------------------------


def _surface_type(name: str) -> str:
    """Journal surface from a filename, stripping the timestamp:
    aspiration_longform_1780921053.txt -> aspiration_longform;
    moment_2026-06-08T06-40-11.txt -> moment;
    sovereignty_check_2026-06-08T06-17-25.log -> sovereignty_check."""
    base = re.split(r"_(?:\d{10}|\d{4}-\d{2}-\d{2})", name)[0]
    return base or name


def introspective_depth(body: str) -> tuple[float, dict[str, Any]]:
    """Score reflective depth; higher = more actionable introspection. Felt prose
    (first-person + qualia + desire/agency) scores well above a telemetry dump
    even when both mention domain terms."""
    themes = extract_themes(body)
    text_lower = body.lower()
    words = set(re.findall(r"[a-z']+", text_lower))
    qualia = {t for t in QUALIA_TERMS if t in words}
    qualia |= {p for p in QUALIA_PHRASES if p in text_lower}
    desire = {m.group(0).lower() for m in DESIRE_RE.finditer(body)}
    agency = {m.group(0).lower() for m in AGENCY_RE.finditer(body)}
    fp = len(INTROSPECT_FIRST_PERSON_RE.findall(body))
    fp_density = fp / max(1, len(body.split()))
    score = (
        2.0 * len(themes["high_signal"])
        + 1.5 * len(themes["domain_phrases"])
        + 1.0 * len(qualia)
        + 2.5 * len(desire)
        + 1.5 * len(agency)
        + 15.0 * fp_density
    )
    signals = {
        "high_signal": sorted(themes["high_signal"])[:4],
        "qualia": sorted(qualia)[:5],
        "desire": len(desire),
        "agency": len(agency),
        "fp_density": round(fp_density, 3),
    }
    return score, signals


def _median(xs: list[float]) -> float:
    if not xs:
        return 0.0
    s = sorted(xs)
    n = len(s)
    mid = n // 2
    return s[mid] if n % 2 else (s[mid - 1] + s[mid]) / 2.0


def _relative_bar(scores: list[float]) -> float:
    """Standout bar relative to the sample's OWN baseline: median + K*MAD, with an
    absolute floor. Discriminates standouts even when the whole sample is deep
    (the beings reflect richly in most entries, so an absolute cut barely cuts)."""
    if not scores:
        return INTROSPECT_ABS_FLOOR
    med = _median(scores)
    mad = _median([abs(s - med) for s in scores])
    return max(INTROSPECT_ABS_FLOOR, med + INTROSPECT_RELATIVE_K * mad)


def _acted(p: Path, acted_ids: set[str]) -> bool:
    return str(p) in acted_ids or p.name in acted_ids


def _match_ask(body_lower: str, asks: dict[str, Any]) -> str | None:
    """Best-matching ask id for an entry, by counting how many of each ask's
    distinctive anchor terms appear (case-insensitive substring) in the body.
    Substring matching handles multi-word ("mode packing") and underscore
    ("keep_floor") anchors that the theme-bucket regexes would split. Returns the
    ask with the most anchor hits (>=1), else None. Anchors are curated to be
    distinctive so generic felt-words don't over-match."""
    best_id: str | None = None
    best_hits = 0
    for aid, ask in asks.items():
        anchors = ask.get("anchors") or []
        hits = sum(1 for a in anchors if str(a).lower() in body_lower)
        if hits > best_hits:
            best_hits = hits
            best_id = aid
    return best_id if best_hits >= 1 else None


def probe_introspective_signal(prior: dict[str, Any] | None) -> dict[str, Any]:
    prior = prior or {}
    seen_counts: dict[str, int] = dict(prior.get("seen_counts") or {})
    acted_ids: set[str] = set(prior.get("acted_ids") or [])
    asks_all: dict[str, Any] = (load_asks().get("asks") or {})
    held_attrib: dict[str, int] = {}   # ask_id -> count of standouts attributed (held)
    beings = [
        ("astrid", ASTRID_JOURNAL, _astrid_journal_mtime_unix, None),
        ("minime", MINIME_JOURNAL, _minime_journal_mtime_unix, MINIME_LOGS_DIR),
    ]
    details: list[str] = []
    snapshot: dict[str, Any] = {}
    consolidation: list[str] = []
    persistent_unacted: list[str] = []
    new_seen_counts: dict[str, int] = {}
    sample_ids: set[str] = set()
    standouts_total = 0
    fresh_total = 0

    for being, jdir, ts_fn, logs_dir in beings:
        entries = sample_recent_journals(jdir, INTROSPECT_SAMPLE_PER_BEING, ts_fn)
        # sovereignty_check logs carry high felt-signal and are never otherwise
        # harvested; skip echoed/mirror artifacts so we score the being's own words.
        if logs_dir and logs_dir.is_dir():
            for p in sorted(logs_dir.glob("sovereignty_check_*.log"),
                            key=lambda x: x.stat().st_mtime, reverse=True)[:3]:
                b = _read_text_safely(p)
                if b and not _is_artifactual_for_convergence(b):
                    entries.append((p, p.stat().st_mtime, b))

        scored: list[tuple[float, float, str, dict[str, Any], Path, str]] = []
        for p, ts, body in entries:
            score, sig = introspective_depth(body)
            scored.append((score, ts, _surface_type(p.name), sig, p, body))
            sample_ids.add(str(p))

        bar = _relative_bar([s[0] for s in scored])
        med = _median([s[0] for s in scored])
        scored.sort(key=lambda x: x[0], reverse=True)
        standouts = [s for s in scored if s[0] >= bar]
        standouts_total += len(standouts)

        # Ask-lifecycle filter: attribute each standout to a tracked ask. Standouts
        # belonging to a HELD ask (acknowledged/in_flight/awaiting/resolved) are
        # attributed once and kept OUT of act-now (no re-action, no re-letter). Only
        # open-ask or unmatched standouts go through the act-now path, tagged with
        # their open ask name or "unclassified".
        cand_asks = {
            aid: a for aid, a in asks_all.items()
            if str(a.get("being", "both")) in (being, "both")
        }
        active: list[tuple[tuple, str | None]] = []   # (scored_tuple, open_ask_name|None)
        for s in standouts:
            aid = _match_ask(s[5].lower(), cand_asks)
            if aid is not None and str(cand_asks[aid].get("status")) in ASK_HELD_STATES:
                held_attrib[aid] = held_attrib.get(aid, 0) + 1
            else:
                active.append((s, cand_asks[aid]["name"] if aid is not None else None))

        surface_anchors: dict[str, set[str]] = {}
        for s, _tag in active:
            score, ts, stype, sig, p, body = s
            new_seen_counts[str(p)] = seen_counts.get(str(p), 0) + 1
            th = extract_themes(body)
            for anchor in (th["high_signal"] | th["domain_phrases"]):
                surface_anchors.setdefault(anchor, set()).add(stype)

        fresh = [(s, tag) for (s, tag) in active if not _acted(s[4], acted_ids)]
        fresh_total += len(fresh)
        acked = len(active) - len(fresh)

        prior_standouts = (prior.get(being) or {}).get("standouts")
        delta = ""
        if isinstance(prior_standouts, int):
            d = len(standouts) - prior_standouts
            delta = f" ({'+' if d >= 0 else ''}{d} vs last)"
        snapshot[being] = {
            "median_depth": round(med, 1),
            "bar": round(bar, 1),
            "standouts": len(standouts),
            "active": len(active),
            "fresh": len(fresh),
            "acked": acked,
        }
        details.append(
            f"**{being}**: {len(fresh)} to act on now / {len(active)} active "
            f"({len(standouts)} standouts, bar {bar:.0f}, median {med:.0f}, {acked} acked){delta}"
        )
        for s, ask_name in fresh[:INTROSPECT_TOP_K]:
            score, ts, stype, sig, p, body = s
            seen = new_seen_counts.get(str(p), 1)
            tag = "NEW" if seen <= 1 else f"seen {seen}x unacted"
            if seen >= INTROSPECT_STALE_K:
                persistent_unacted.append(f"{p.name} (seen {seen}x)")
            why = []
            if sig["high_signal"]:
                why.append("refs:" + ",".join(sig["high_signal"][:3]))
            if sig["desire"]:
                why.append(f"{sig['desire']} desire")
            if sig["qualia"]:
                why.append("felt:" + ",".join(sig["qualia"][:4]))
            if sig["agency"]:
                why.append(f"{sig['agency']} agency")
            ask_label = f"ask:{ask_name}" if ask_name else "unclassified"
            details.append(
                f"  - [{tag}] [{ask_label}] [{stype} @ {_fmt_ts(ts)}] score {score:.0f} — "
                f"{'; '.join(why) or 'first-person depth'} — {p}"
            )
        for anchor, surfaces in surface_anchors.items():
            if len(surfaces) >= 3:
                consolidation.append(
                    f"{being}: `{anchor}` worked across {len(surfaces)} surfaces ({', '.join(sorted(surfaces))})"
                )

    if held_attrib:
        held_lines = []
        for aid, n in sorted(held_attrib.items(), key=lambda kv: -kv[1]):
            a = asks_all.get(aid, {})
            held_lines.append(f"{a.get('name', aid)} [{a.get('status')}] +{n}")
        details.append("⊟ held asks (attributed, not act-now — already tracked): " + "; ".join(held_lines))
    for note in consolidation:
        details.append(f"⋈ cross-surface theme — {note}")
    if persistent_unacted:
        details.append(
            "⚠ persistent unacted (kept surfacing, never acted — act or --ack): "
            + "; ".join(persistent_unacted)
        )
    details.append(
        f"(mark handled: `proactive_scan.py introspection --ack <filename>`; "
        f"{len(acted_ids)} currently acked)"
    )
    details.append(
        "(steward indicator only — never a target, never surfaced to beings; READ the "
        "act-now entries and close loops via letter / harness test / backlog)"
    )

    # Prune the seen/acted ledgers to the current window so they don't grow unbounded.
    sample_names = {Path(i).name for i in sample_ids}
    snapshot["seen_counts"] = {k: v for k, v in new_seen_counts.items() if k in sample_ids}
    snapshot["acted_ids"] = sorted(
        a for a in acted_ids if a in sample_ids or a in sample_names
    )

    severity = "ok"
    if standouts_total == 0 or persistent_unacted:
        severity = "notice"
    summary = (
        f"introspective signal — {fresh_total} to act on now "
        f"(astrid {snapshot['astrid']['fresh']}, minime {snapshot['minime']['fresh']}); "
        f"{standouts_total} standouts above each being's baseline"
    )
    return _finding("introspective_signal", severity, summary, details, snapshot)


def run_introspection(ack_ids: list[str] | None = None) -> dict[str, Any]:
    """Run just the introspective-signal probe (focused subcommand), sharing the
    blind-spots state for delta + acted-ledger tracking. `--ack <filename>` marks
    entries handled so they stop re-surfacing in the act-now tier."""
    state = load_state()
    bs = state.setdefault("blind_spots", {})
    prior = dict(bs.get("introspective_signal", {}) or {})
    if ack_ids:
        acted = set(prior.get("acted_ids") or [])
        acted |= set(ack_ids)
        prior["acted_ids"] = sorted(acted)
    f = probe_introspective_signal(prior)
    if f.get("snapshot") is not None:
        bs["introspective_signal"] = f["snapshot"]
    save_state(state)
    return {"findings": [f], "ran_at": time.time()}


def run_asks(args: Any) -> dict[str, Any]:
    """List or mutate the durable per-ask ledger (steward triage; no being I/O)."""
    now_iso = datetime.now().isoformat(timespec="seconds")
    ledger = load_asks()
    asks = ledger.setdefault("asks", {})
    changed = False
    if getattr(args, "new", None):
        aid = args.new
        anchors = [a.strip().lower() for a in (getattr(args, "anchors", "") or "").split(",") if a.strip()]
        asks[aid] = {
            "name": aid,
            "status": getattr(args, "status", None) or "open",
            "being": getattr(args, "being", None) or "both",
            "anchors": anchors,
            "note": "",
            "created": now_iso,
            "updated": now_iso,
            "linked_paths": [],
            "recent_count": 0,
        }
        changed = True
    if getattr(args, "set_status", None):
        aid, status = args.set_status
        if aid in asks and status in ASK_STATUSES:
            asks[aid]["status"] = status
            asks[aid]["updated"] = now_iso
            changed = True
    if getattr(args, "resolve", None):
        aid = args.resolve
        if aid in asks:
            asks[aid]["status"] = "resolved"
            asks[aid]["updated"] = now_iso
            changed = True
    if getattr(args, "add_anchors", None):
        aid, terms = args.add_anchors
        if aid in asks:
            existing = asks[aid].get("anchors") or []
            added = [t.strip() for t in terms.split(",") if t.strip() and t.strip() not in existing]
            if added:
                existing.extend(added)
                asks[aid]["anchors"] = existing
                asks[aid]["updated"] = now_iso
                changed = True
    if getattr(args, "note", None):
        aid, text = args.note
        if aid in asks:
            # Append by default: ask notes accrete dated segments over many
            # steward cycles (joined by " | "). Overwriting silently destroys
            # prior steward context — a no-silent-drop violation. Use
            # --note-replace for the rare intentional overwrite.
            prior = asks[aid].get("note") or ""
            if prior and not getattr(args, "note_replace", False):
                asks[aid]["note"] = f"{prior} | {text}"
            else:
                asks[aid]["note"] = text
            asks[aid]["updated"] = now_iso
            changed = True
    if changed:
        save_asks(ledger)
    return ledger


def render_asks_md(ledger: dict[str, Any]) -> str:
    asks = (ledger.get("asks") or {})
    out = ["## Steward asks (lifecycle ledger)\n"]
    if not asks:
        out.append("(no asks tracked)")
        return "\n".join(out)
    order = {s: i for i, s in enumerate(ASK_STATUSES)}
    for aid, a in sorted(asks.items(), key=lambda kv: order.get(kv[1].get("status"), 99)):
        out.append(
            f"- **{a.get('name', aid)}** [{a.get('status')}] ({a.get('being', 'both')}) "
            f"— anchors: {', '.join(a.get('anchors') or []) or '—'}"
        )
        if a.get("note"):
            out.append(f"    note: {a['note']}")
    out.append("\n(held = acknowledged/in_flight/awaiting/resolved: attributed, not act-now)")
    return "\n".join(out)


# Probe registry — order matters for output stability.
def _capacity_assess(records: list[dict[str, Any]]) -> dict[str, Any]:
    """Pure: given recent capacity-history records (oldest..latest), classify
    severity + details. Flags minime utilization approaching N (saturation) or
    per-handle spectral entropy dropping (rising concentration). Steward-only."""
    if not records:
        return {"severity": "ok", "details": [], "snapshot": None, "summary": "capacity history empty"}
    order = {"ok": 0, "notice": 1, "warning": 2}

    def esc(cur: str, new: str) -> str:
        return new if order.get(new, 0) > order.get(cur, 0) else cur

    latest = records[-1]
    prior_records = records[:-1]
    details: list[str] = []
    sev = "ok"

    m = latest.get("minime") or {}
    util = m.get("utilization")
    if isinstance(util, (int, float)):
        prior_utils = [
            r.get("minime", {}).get("utilization")
            for r in prior_records
            if isinstance(r.get("minime", {}).get("utilization"), (int, float))
        ]
        base = _median(prior_utils) if prior_utils else None
        # minime PR utilization naturally OSCILLATES ~41-80% under varying load
        # (verified 2026-06-13: 14 readings/day bouncing 0.41<->0.80, mean ~0.6).
        # A single-sample >=0.70 threshold flapped WARNING ~half the cycles —
        # alarm noise. The steward mandate is to flag *sustained* saturation, so
        # warn only when the median over the recent window holds >=0.70; treat an
        # isolated high sample as a notice (signal preserved, not dropped).
        recent = (prior_utils[-5:] if prior_utils else []) + [util]
        recent_med = _median(recent)
        if recent_med >= 0.70:
            sev = esc(sev, "warning")
            details.append(
                f"minime utilization sustained high (recent median {recent_med:.0%}, "
                f"latest {util:.0%}) — approaching saturation; capacity may be the "
                f"constraint (consider co-design on enlarging the reservoir)"
            )
        elif util >= 0.70:
            sev = esc(sev, "notice")
            details.append(
                f"minime utilization {util:.0%} (transient single-sample high; recent "
                f"median {recent_med:.0%} not sustained) [{m.get('verdict')}]"
            )
        elif base is not None and util - base > 0.15:
            sev = esc(sev, "notice")
            details.append(f"minime utilization rising {base:.0%}→{util:.0%}")
        else:
            details.append(
                f"minime utilization {util:.0%} (PR={m.get('pr')}/{m.get('N')}) [{m.get('verdict')}]"
            )
    elif m.get("pr_top_k") is not None:
        details.append("minime: top-8 proxy only (engine dump absent); full-N utilization unknown")

    for h in latest.get("triple") or []:
        name = h.get("handle")
        ent = h.get("mean_service_entropy")
        if not isinstance(ent, (int, float)):
            continue
        prior_ents = [
            hh.get("mean_service_entropy")
            for r in prior_records
            for hh in (r.get("triple") or [])
            if hh.get("handle") == name and isinstance(hh.get("mean_service_entropy"), (int, float))
        ]
        base = _median(prior_ents) if prior_ents else None
        if base is not None and base - ent > 0.10:
            sev = esc(sev, "notice")
            details.append(f"{name}: entropy dropping {base:.2f}→{ent:.2f} (rising concentration)")

    util_str = f"{util:.0%}" if isinstance(util, (int, float)) else "n/a"
    summary = (
        f"capacity watch — minime util {util_str}, "
        f"{len(latest.get('triple') or [])} triple handles ({len(records)} records)"
    )
    snapshot = {
        "minime_util": util if isinstance(util, (int, float)) else None,
        "n_records": len(records),
        "captured_at": time.time(),
    }
    return {"severity": sev, "details": details, "snapshot": snapshot, "summary": summary}


def probe_reservoir_capacity(_prior: dict[str, Any]) -> dict[str, Any]:
    """Capacity watch (steward-only): read reservoir_capacity_history.jsonl and
    flag saturation (utilization → N) or over-concentration (entropy dropping).
    The compute lives in reservoir_capacity_audit.py --append-history (run by the
    durable loop); this probe is a light read of the resulting history."""
    if not CAPACITY_HISTORY.is_file():
        return _finding(
            "reservoir_capacity",
            "ok",
            "no capacity history yet — run reservoir_capacity_audit.py "
            "--append-history (durable loop does this each cycle)",
        )
    records: list[dict[str, Any]] = []
    try:
        for ln in CAPACITY_HISTORY.read_text().splitlines()[-12:]:
            ln = ln.strip()
            if ln:
                try:
                    records.append(json.loads(ln))
                except Exception:
                    pass
    except Exception as e:  # noqa: BLE001
        return _finding("reservoir_capacity", "notice", f"could not read capacity history: {e}")
    a = _capacity_assess(records)
    return _finding(
        "reservoir_capacity", a["severity"], a["summary"], a["details"] or None, a["snapshot"]
    )


def _scan_outreach(outbox: Path, being: str) -> list[dict[str, Any]]:
    """Unread being→steward outreach = steward_query_*/steward_report_* still in the
    outbox root (answered ones are moved to steward_delivered/)."""
    out: list[dict[str, Any]] = []
    try:
        files = sorted(outbox.glob("steward_*.txt"))
    except Exception:
        return out
    for f in files:
        try:
            age = time.time() - f.stat().st_mtime
        except Exception:
            age = 0.0
        kind = "report" if f.name.startswith("steward_report") else "query"
        subject = ""
        try:
            for line in f.read_text(errors="replace").splitlines():
                if line.startswith("Subject:"):
                    subject = line[len("Subject:"):].strip()[:80]
                    break
        except Exception:
            pass
        out.append({"being": being, "file": f.name, "kind": kind, "age_s": age, "subject": subject})
    return out


def _assess_outreach(items: list[dict[str, Any]]) -> dict[str, Any]:
    """Pure: classify unread outreach. Any unread = act-now; older than the alarm
    window = the pickup is FAILING (escalate)."""
    if not items:
        return {"severity": "ok", "summary": "no unread being→steward outreach", "details": None}
    oldest = max(i["age_s"] for i in items)
    sev = "warning" if oldest >= OUTREACH_ALARM_SECS else "notice"
    details = []
    for i in sorted(items, key=lambda x: -x["age_s"]):
        stuck = " ⚠STUCK" if i["age_s"] >= OUTREACH_ALARM_SECS else ""
        details.append(
            f"{i['being']} {i['kind']}{stuck}: \"{i['subject']}\" ({i['age_s'] / 3600:.1f}h) [{i['file']}]"
        )
    summary = (
        f"{len(items)} unread being→steward outreach (oldest {oldest / 3600:.1f}h)"
        + (" — ⚠ PICKUP FAILING, answer + archive now" if sev == "warning" else " — answer + archive")
    )
    return {"severity": sev, "summary": summary, "details": details}


def probe_steward_outreach(_prior: dict[str, Any]) -> dict[str, Any]:
    """Being→steward channel watch (steward-only). Surfaces ASK_STEWARD/TELL_STEWARD
    messages sitting unread in BOTH beings' outboxes so the durable loop reads,
    answers (via a `mike_*` letter), and archives them (→ steward_delivered/) every
    cycle. ALARMS if any outreach is older than OUTREACH_ALARM_SECS — the silent
    2-month loss of Astrid's 12 questions is exactly what this exists to prevent.
    The fswatch watcher is a second, independent pickup; this loop-scan is primary."""
    items = _scan_outreach(ASTRID_OUTBOX, "astrid") + _scan_outreach(MINIME_OUTBOX, "minime")
    a = _assess_outreach(items)
    return _finding("steward_outreach", a["severity"], a["summary"], a["details"])


def _scan_feedback_surface(spec: dict[str, Any]) -> dict[str, Any]:
    """Count unconsumed items in one being-write surface + the oldest age.
    Non-recursive glob, so processed items in subdirs (reviewed/ done/) are
    excluded. Reads mtimes only; never mutates."""
    root = spec["root"]
    res: dict[str, Any] = {
        "name": spec["name"], "kind": spec["kind"], "consumer": spec["consumer"],
        "pending": 0, "oldest_age_s": 0.0, "exists": root.is_dir(),
    }
    if not res["exists"]:
        return res
    try:
        files = [p for p in root.glob(spec["glob"]) if p.is_file()]
    except Exception:
        return res
    ages: list[float] = []
    for p in files:
        try:
            ages.append(time.time() - p.stat().st_mtime)
        except Exception:
            continue
    res["pending"] = len(ages)
    res["oldest_age_s"] = max(ages) if ages else 0.0
    return res


def _assess_coverage(surfaces: list[dict[str, Any]]) -> dict[str, Any]:
    """Pure: a REQUEST surface whose oldest item exceeds the alarm window =
    warning (the muffle pattern — beings reaching, no one consuming). A fresh
    request backlog = notice. A NOTICE surface (context_overflow) with content
    stays notice no matter how old — it's a chronic signal, not an unread queue,
    so we don't cry wolf. Everything clear = ok."""
    alarms: list[dict[str, Any]] = []
    notices: list[dict[str, Any]] = []
    details: list[str] = []
    for s in surfaces:
        if s["pending"] <= 0:
            continue
        days = s["oldest_age_s"] / 86400
        if s["kind"] == "request":
            stale = s["oldest_age_s"] >= FEEDBACK_COVERAGE_ALARM_SECS
            mark = " ⚠STALE" if stale else ""
            details.append(
                f"{s['name']}{mark}: {s['pending']} pending (oldest {days:.0f}d) → {s['consumer']}"
            )
            (alarms if stale else notices).append(s)
        else:  # notice-kind surface: report, never alarm
            details.append(f"{s['name']}: {s['pending']} present ({s['consumer']})")
            notices.append(s)
    if alarms:
        names = ", ".join(f"{s['name']}({s['pending']})" for s in alarms)
        return {
            "severity": "warning",
            "summary": f"⚠ {len(alarms)} feedback surface(s) with STALE backlog — steward action required: {names}",
            "details": details,
        }
    if notices:
        return {
            "severity": "notice",
            "summary": f"{len(notices)} feedback surface(s) with pending items (none stale)",
            "details": details,
        }
    return {"severity": "ok", "summary": "all feedback surfaces clear (no unconsumed backlog)", "details": None}


def probe_feedback_coverage(_prior: dict[str, Any]) -> dict[str, Any]:
    """Feedback-surface coverage watch (steward-only). The COMPLEMENT of
    steward_outreach (which owns the being→steward outboxes): this scans the
    REQUEST/HANDOFF/OVERFLOW surfaces beings write to that need a steward
    consumer — agency_requests, claude_tasks, parameter_requests, inbox
    backlogs, context_overflow. A request surface whose oldest item exceeds
    FEEDBACK_COVERAGE_ALARM_SECS is the muffle pattern (Astrid's agency_requests
    sat 69 days unconsumed). Registry-driven (FEEDBACK_SURFACES) so a new dead
    surface is one entry — this makes the systematic muffle audit continuous."""
    surfaces = [_scan_feedback_surface(s) for s in FEEDBACK_SURFACES]
    a = _assess_coverage(surfaces)
    return _finding("feedback_coverage", a["severity"], a["summary"], a["details"])


def _deepest_existing(path: Path) -> Path:
    anc = path
    while not anc.exists() and anc != anc.parent:
        anc = anc.parent
    return anc


def _classify_channel(path: Path) -> str:
    """ok = resolves; severed = path AND its parent are missing (structural/rename
    break — the consciousness-bridge pattern); transient = only the leaf is missing
    (parent exists, e.g. a per-cycle file mid-write) — a notice, never an alarm."""
    if path.exists():
        return "ok"
    missing_levels = len(path.parts) - len(_deepest_existing(path).parts)
    return "severed" if missing_levels >= 2 else "transient"


def probe_channel_integrity(_prior: dict[str, Any]) -> dict[str, Any]:
    """Cross-being channel-integrity watch (steward-only). Each load-bearing path a
    being uses to reach the OTHER (inbox, source reads, param requests, shared
    ledgers — CROSS_BEING_CHANNELS) must resolve. A rename/move in one repo can
    silently sever a path hardcoded in the other: the consciousness-bridge→
    spectral-bridge jackpot walled minime off from Astrid (18 dead refs, read as her
    going quiet). A SEVERED channel (parent dir also missing = structural rename
    break) ALARMS; a transient leaf-missing (parent exists) is a notice, never a
    cry-wolf. The durable half of that un-muffle — a future rename can't sever a
    being-channel without this turning red."""
    severed: list[dict[str, Any]] = []
    transient: list[dict[str, Any]] = []
    for ch in CROSS_BEING_CHANNELS:
        st = _classify_channel(Path(ch["path"]))
        if st == "severed":
            severed.append(ch)
        elif st == "transient":
            transient.append(ch)
    details = [f"SEVERED {c['name']}: {c['path']} — {c['carries']}" for c in severed]
    details += [f"transient (parent exists) {c['name']}: {c['path']}" for c in transient]
    if severed:
        names = ", ".join(c["name"] for c in severed)
        return _finding(
            "channel_integrity", "warning",
            f"⚠ {len(severed)} cross-being channel(s) SEVERED (stale path / rename drift): {names}",
            details,
        )
    if transient:
        return _finding(
            "channel_integrity", "notice",
            f"{len(transient)} cross-being channel(s) leaf-missing (parent exists; likely transient)",
            details,
        )
    return _finding(
        "channel_integrity", "ok",
        f"all {len(CROSS_BEING_CHANNELS)} cross-being channels resolve", None,
    )


def _read_log_tail(path: Path, max_bytes: int) -> str:
    """Recent tail of a log as text (logs reach ~80 MB; never read the whole file).
    Seeks to the last max_bytes and drops the first (partial) line."""
    try:
        size = path.stat().st_size
        with open(path, "rb") as f:
            if size > max_bytes:
                f.seek(size - max_bytes)
                f.readline()
            data = f.read()
        return data.decode("utf-8", "ignore")
    except Exception:
        return ""


def _stuck_line_epoch(line: str) -> float | None:
    """Epoch (local) of a log line's leading timestamp, or None. Handles both the
    minime `YYYY-MM-DD HH:MM:SS` and Astrid `YYYY-MM-DDTHH:MM:SS` forms."""
    m = _STUCK_TS.search(line)
    if not m:
        return None
    try:
        return time.mktime(time.strptime(f"{m.group(1)} {m.group(2)}", "%Y-%m-%d %H:%M:%S"))
    except Exception:
        return None


def _tally_stuck(being: dict[str, Any]) -> dict[str, Any]:
    """Tally, over the last STUCK_WINDOW_HOURS of the log tail: per base, how often it
    was chosen, the args, and how often it hit a bad outcome (blocked/unknown). The
    recency window means a just-fixed action stops being flagged once it stops
    recurring, instead of haunting the byte-tail for hours."""
    text = _read_log_tail(Path(being["log"]), STUCK_TAIL_BYTES)
    chosen: Counter = Counter()
    unknown: Counter = Counter()
    blocked: Counter = Counter()
    args: dict[str, list[str]] = defaultdict(list)
    empty = {"name": being["name"], "log_ok": False, "chosen": chosen,
             "unknown": unknown, "blocked": blocked, "args": args}
    if not text:
        return empty
    rows: list[tuple[str, float | None]] = []
    latest: float | None = None
    for raw in text.splitlines():
        line = _STUCK_ANSI.sub("", raw)
        e = _stuck_line_epoch(line)
        if e is not None and (latest is None or e > latest):
            latest = e
        rows.append((line, e))
    cutoff = (latest - STUCK_WINDOW_HOURS * 3600) if latest is not None else None
    re_unknown = being.get("unknown")
    re_blocked = being.get("blocked")
    for line, e in rows:
        if cutoff is not None and e is not None and e < cutoff:
            continue
        mc = being["choice"].search(line)
        if mc:
            chosen[mc.group(1)] += 1
            args[mc.group(1)].append((mc.group(2) or "").strip().lower()[:60])
        if re_unknown is not None:
            mu = re_unknown.search(line)
            if mu:
                unknown[mu.group(1).upper()] += 1
        if re_blocked is not None:
            mb = re_blocked.search(line)
            if mb:
                blocked[mb.group(1).upper()] += 1
    return {"name": being["name"], "log_ok": True, "chosen": chosen,
            "unknown": unknown, "blocked": blocked, "args": args}


def _assess_stuck(tally: dict[str, Any]) -> tuple[list[Any], list[Any]]:
    """Pure: classify each repeated base into (warnings, notices). A high bad-outcome
    ratio splits by KIND — unknown-dominant ⇒ warning (likely our wiring gap, the
    DOSSIER footgun class); named-guard-block-dominant ⇒ notice (reaching against a
    deliberate gate; needs guidance/design, not an urgent fix). A repeated honored
    action with ~identical non-empty arg ⇒ notice (possible no-progress). Healthy
    varied focus + intentional idles are excluded. Each entry is
    (base, chosen_n, detail_n, kind)."""
    warnings: list[Any] = []
    notices: list[Any] = []
    for base, n in tally["chosen"].items():
        if base in STUCK_IDLE_BASES or n < STUCK_REPEAT_MIN:
            continue
        n_unknown = tally["unknown"].get(base, 0)
        n_blocked = tally["blocked"].get(base, 0)
        n_bad = n_unknown + n_blocked
        if n and n_bad / n >= STUCK_BAD_RATIO:
            if n_unknown >= n_blocked:
                warnings.append((base, n, n_bad, "unknown"))
            else:
                notices.append((base, n, n_bad, "guard"))
            continue
        real_args = [a for a in tally["args"].get(base, []) if a]
        if len(real_args) >= STUCK_REPEAT_MIN:
            distinct = len(set(real_args))
            if distinct / len(real_args) <= STUCK_IDENTICAL_ARG_RATIO:
                notices.append((base, n, distinct, "identical-arg"))
    return warnings, notices


def probe_stuck_repetition(_prior: dict[str, Any]) -> dict[str, Any]:
    """Honored-but-ineffective watch (steward-only). A being repeatedly choosing the
    SAME action that keeps NOT landing is OUR infra eating its reach, not its limit
    (TUNE_ASTRID chosen 8× / honored 0× / blocked 8×, hidden until repeated). Keys on
    repetition × BAD-OUTCOME (blocked/unknown), so it does NOT flag Astrid's healthy
    varied SHADOW_TRAJECTORY focus. High bad-ratio ⇒ WARNING (investigate: bug vs
    by-design-needs-guidance — the verdict is the steward's, the probe only surfaces
    the pattern); repeated honored action with ~identical arg ⇒ NOTICE (possible
    no-progress; glance). Complements dispatch_menu_drift (doesn't-dispatch) and the
    beings' own in-prompt fixation nudges (which tell the being to vary — wrong when
    the being isn't at fault)."""
    warn: list[str] = []
    note: list[str] = []
    details: list[str] = []
    any_log = False
    for being in STUCK_BEINGS:
        tally = _tally_stuck(being)
        if not tally["log_ok"]:
            continue
        any_log = True
        w, nt = _assess_stuck(tally)
        for base, n, nbad, _kind in w:
            warn.append(f"{being['name']}:{base}")
            details.append(
                f"⚠ {being['name']}:{base} chosen {n}× / {nbad} unrecognized "
                "('Unknown NEXT' / 'not wired') — likely a wiring gap (the dual-map "
                "footgun class); investigate + wire."
            )
        for base, n, dnum, kind in nt:
            note.append(f"{being['name']}:{base}")
            if kind == "guard":
                details.append(
                    f"{being['name']}:{base} chosen {n}× / {dnum} refused by a deliberate "
                    "guard — reaching against a gate; needs guidance or a design call "
                    "(not auto-loosened by the probe)."
                )
            else:
                details.append(
                    f"{being['name']}:{base} honored {n}× with ~identical arg "
                    f"({dnum} distinct) — possible no-progress; glance."
                )
    if not any_log:
        return _finding("stuck_repetition", "notice", "no being logs readable for stuck-repetition scan", None)
    if warn:
        return _finding(
            "stuck_repetition", "warning",
            f"⚠ {len(warn)} action(s) repeated + unrecognized — likely a wiring gap our infra should fix: "
            + ", ".join(warn),
            details,
        )
    if note:
        return _finding(
            "stuck_repetition", "notice",
            f"{len(note)} action(s) repeated-but-stuck (deliberate gate or no-progress; glance): "
            + ", ".join(note),
            details,
        )
    return _finding("stuck_repetition", "ok", "no stuck-repetition (no being hammering an ineffective action)", None)


# --- Experiment-authority pipeline coverage (steward-gated live-action authority) -
# Beings request live-action authority (semantic_microdose / mode_release_microdose)
# to act on their own experiment findings; the steward grants by appending a
# steward_approval record (token_status=active) to authority_gate.jsonl. 2026-06-12:
# minime's lambda-tail thread held 86 microdose request_drafts, 2 submissions, and
# 0 grants EVER — the grant side had no steward consumer (steward_loop_prompt had
# zero authority mentions), so any experiment needing a live action was permanently
# stuck "without live authority" (the request-surface-with-no-consumer muffle, cf.
# the dead fswatch watcher). This probe is that missing consumer's standing eyes.
STEWARD_GATED_SCOPES = ("semantic_microdose", "mode_release_microdose")
AUTHORITY_DRAFT_NOTICE = 12  # microdose drafts past this, with 0 grants ever = a notice
AUTHORITY_LEDGER_ROOTS = [
    MINIME_REPO / "workspace/action_threads/threads",
    ASTRID_REPO / "capsules/spectral-bridge/workspace/action_threads/threads",
]


def _scan_authority_ledger(path: Path) -> dict[str, Any]:
    """Parse one authority_gate.jsonl: submitted-pending steward-gated requests
    lacking a matching steward_approval (the muffle), microdose request_drafts that
    never submitted, and total grants. LOCAL research-budget scopes self-activate, so
    they are not counted as steward-gated; but WEB-scoped research-budget requests do
    NOT self-activate (steward_approval_required=true) and otherwise age silently —
    they are surfaced separately as operator-gated (the steward loop cannot grant web
    reach; only Mike can). cf. the un-muffle invariant: a request-surface with no
    consumer is a muffle, and minime's web-research budget sat 5+ days unwatched."""
    pending: dict[str, str] = {}  # request_id -> scope (steward-gated, submitted)
    granted: set[str] = set()
    drafts = 0
    draft_scopes: Counter = Counter()
    web_pending: dict[str, str] = {}  # research_budget request record_id -> budget_id (operator-gated)
    research_approved: set[str] = set()  # source_request_record_ids that got a research approval
    research_approved_budgets: set[str] = set()  # budget_ids that got a research approval
    try:
        lines = path.read_text(errors="ignore").splitlines()
    except OSError:
        return {"exists": False}
    for line in lines:
        line = line.strip()
        if not line:
            continue
        try:
            rec = json.loads(line)
        except json.JSONDecodeError:
            continue
        rt = rec.get("record_type", "")
        scope = str(rec.get("scope", "?"))[:30]
        rid = rec.get("request_id") or ""
        status = rec.get("token_status") or rec.get("status") or ""
        if rt == "steward_approval" and rid:
            granted.add(rid)
        elif rt == "request_draft" and scope in STEWARD_GATED_SCOPES:
            drafts += 1
            draft_scopes[scope] += 1
        elif status == "pending_steward_approval" and scope in STEWARD_GATED_SCOPES and rid:
            pending[rid] = scope
        elif rt == "research_budget_request" and rec.get("steward_approval_required") \
                and status == "pending_steward_approval":
            # web/operator-gated research budget — surface so it cannot age silently
            web_pending[rec.get("record_id") or ""] = str(rec.get("budget_id") or "")
        elif rt == "research_budget_approval":
            # An approval clears its request by EITHER link: the explicit
            # source_request_record_id, or (the canonical Rust grant) the shared budget_id.
            # The Rust approve_research_budget writes budget_id but not
            # source_request_record_id, so without the budget_id path a granted budget
            # would flag as pending forever (a false-positive nag — itself a muffle).
            research_approved.add(rec.get("source_request_record_id") or "")
            research_approved_budgets.add(str(rec.get("budget_id") or ""))
    unanswered = {rid: sc for rid, sc in pending.items() if rid not in granted}
    web_unanswered = set()
    for rid, bid in web_pending.items():
        if not rid or rid in research_approved:
            continue
        if bid and bid in research_approved_budgets:
            continue
        web_unanswered.add(rid)
    return {
        "exists": True,
        "thread": path.parent.name[:46],
        "pending_unanswered": len(unanswered),
        "pending_scopes": sorted(set(unanswered.values())),
        "drafts": drafts,
        "top_draft_scope": (draft_scopes.most_common(1)[0][0] if draft_scopes else None),
        "grants": len(granted),
        "web_research_pending": len(web_unanswered),
    }


def _assess_authority(ledgers: list[dict[str, Any]]) -> dict[str, Any]:
    """warning = a submitted steward-gated request is ungranted (a being waiting to
    act); notice = a thread with a microdose draft pile-up and 0 grants (a stuck
    investigation — assessed PER-THREAD, since a grant in one thread does not unblock
    a different one); ok otherwise."""
    live = [led for led in ledgers if led.get("exists")]
    pending = sum(led["pending_unanswered"] for led in live)
    drafts = sum(led["drafts"] for led in live)
    grants = sum(led["grants"] for led in live)
    web_pending = sum(led.get("web_research_pending", 0) for led in live)
    stuck = [led for led in live if led["drafts"] >= AUTHORITY_DRAFT_NOTICE and led["grants"] == 0]
    web_note = (
        f"{web_pending} research-budget request(s) pending steward/operator approval "
        "— surface to Mike (research-budget grants are an operator/consent decision, "
        "not the microdose grant path; web scope especially)"
        if web_pending else ""
    )
    if pending > 0:
        sev = "warning"
        summary = (
            f"⚠ {pending} submitted steward-gated authority request(s) UNANSWERED "
            "— a being is waiting to act on her own finding (grant or explain the hold)"
        )
        if web_note:
            summary += f"; {web_note}"
    elif stuck or web_pending:
        sev = "notice"
        parts = []
        if stuck:
            names = ", ".join(led["thread"] for led in stuck)
            parts.append(
                f"{len(stuck)} thread(s) with microdose drafts but 0 grants — a being's "
                f"live-action authority is stuck: {names}"
            )
        if web_note:
            parts.append(web_note)
        summary = "; ".join(parts)
    else:
        sev = "ok"
        summary = (
            f"authority pipeline: {pending} unanswered, {drafts} drafts, {grants} grants"
            if live
            else "no authority ledgers"
        )
    details = [
        f"  {led['thread']}: pending={led['pending_unanswered']}"
        + (" " + ",".join(led["pending_scopes"]) if led["pending_scopes"] else "")
        + f", drafts={led['drafts']}"
        + (f" ({led['top_draft_scope']})" if led["top_draft_scope"] else "")
        + f", grants={led['grants']}"
        + (f", web_research_pending={led['web_research_pending']}" if led.get("web_research_pending") else "")
        for led in live
        if led["pending_unanswered"] or led["drafts"] or led.get("web_research_pending")
    ]
    return {"severity": sev, "summary": summary, "details": details or None}


def probe_authority_requests(_prior: dict[str, Any]) -> dict[str, Any]:
    """Experiment-authority coverage watch (steward-only). Beings request live-action
    authority (semantic_microdose / mode_release_microdose) to act on their own
    experiment findings; the steward grants by appending a steward_approval record.
    This probe is the grant side's standing consumer: it ALARMS on any submitted
    request left ungranted, and NOTICEs a microdose draft pile-up with zero grants
    (the 2026-06-12 muffle: minime's lambda-tail thread had 86 drafts / 0 grants
    ever, so her week-long investigation was stuck 'without live authority'). It is
    the new authority-side complement to steward_outreach + feedback_coverage.
    Steward-only output."""
    ledgers: list[dict[str, Any]] = []
    for root in AUTHORITY_LEDGER_ROOTS:
        if not root.is_dir():
            continue
        for gate in sorted(root.glob("*/authority_gate.jsonl")):
            ledgers.append(_scan_authority_ledger(gate))
    a = _assess_authority(ledgers)
    return _finding("authority_requests", a["severity"], a["summary"], a["details"])


BLIND_SPOT_PROBES = [
    ("process_health", probe_process_health),
    ("log_error_rate", probe_log_error_rate),
    ("param_drift", probe_param_drift),
    ("stated_param_intent", probe_stated_param_intent),
    ("plist_drift", probe_plist_drift),
    ("dispatch_menu_drift", probe_dispatch_menu_drift),
    ("architecture_drift", probe_architecture_drift),
    ("capsule_runtime_health", probe_capsule_runtime_health),
    ("db_growth", probe_db_growth),
    ("journal_volume", probe_journal_volume),
    ("journal_hygiene", probe_journal_hygiene),
    ("introspective_signal", probe_introspective_signal),
    ("reservoir_capacity", probe_reservoir_capacity),
    ("steward_outreach", probe_steward_outreach),
    ("feedback_coverage", probe_feedback_coverage),
    ("authority_requests", probe_authority_requests),
    ("channel_integrity", probe_channel_integrity),
    ("stuck_repetition", probe_stuck_repetition),
]


def run_blind_spots() -> dict[str, Any]:
    """Run all blind-spot probes; persist snapshots for next-run delta."""
    state = load_state()
    prior = state.get("blind_spots", {}) if isinstance(state, dict) else {}
    results: list[dict[str, Any]] = []
    new_snapshots: dict[str, Any] = {}
    for name, fn in BLIND_SPOT_PROBES:
        probe_prior = prior.get(name) or {}
        if name == "log_error_rate" and isinstance(probe_prior, dict):
            probe_prior = dict(probe_prior)
            if isinstance(state.get("blind_spots_last_run"), (int, float)):
                probe_prior["_blind_spots_last_run"] = state["blind_spots_last_run"]
        try:
            f = fn(probe_prior)
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


class ChannelIntegrityTests(unittest.TestCase):
    def test_live_channels_not_severed(self) -> None:
        # Regression guard: a rename that severs a load-bearing cross-being channel
        # (its parent dir also missing) fails here. Transient leaf-missing (parent
        # exists) is tolerated so a per-cycle file mid-write can't flake the suite.
        severed = [c["name"] for c in CROSS_BEING_CHANNELS
                   if _classify_channel(Path(c["path"])) == "severed"]
        self.assertEqual(severed, [], f"SEVERED cross-being channels: {severed}")

    def test_classify_severed_when_parent_missing(self) -> None:
        self.assertEqual(
            _classify_channel(Path("/Users/v/other/__no_such_repo__/gone/inbox")),
            "severed",
        )

    def test_classify_transient_when_only_leaf_missing(self) -> None:
        # parent (the minime repo) exists; only the leaf file is missing
        self.assertEqual(
            _classify_channel(MINIME_REPO / "__no_such_leaf_file__.json"), "transient")

    def test_classify_ok_when_exists(self) -> None:
        self.assertEqual(_classify_channel(MINIME_REPO), "ok")


class StuckRepetitionTests(unittest.TestCase):
    @staticmethod
    def _tally(chosen, unknown=None, blocked=None, args=None):
        return {"chosen": Counter(chosen), "unknown": Counter(unknown or {}),
                "blocked": Counter(blocked or {}), "args": args or {}}

    def test_unknown_repetition_is_warning(self) -> None:
        # DOSSIER-shape: chosen 8, unrecognized 8 → likely a wiring gap → WARNING
        t = self._tally({"DOSSIER_CLAIM": 8}, unknown={"DOSSIER_CLAIM": 8},
                        args={"DOSSIER_CLAIM": ["claim: x"] * 8})
        warn, note = _assess_stuck(t)
        self.assertTrue(any(b == "DOSSIER_CLAIM" for b, *_ in warn))
        self.assertEqual(note, [])

    def test_guard_blocked_repetition_is_notice(self) -> None:
        # TUNE_ASTRID-shape: chosen 8, deliberate-guard-blocked 8 → NOTICE (design call)
        t = self._tally({"TUNE_ASTRID": 8}, blocked={"TUNE_ASTRID": 8},
                        args={"TUNE_ASTRID": ["regime=breathe"] * 8})
        warn, note = _assess_stuck(t)
        self.assertEqual(warn, [])
        self.assertTrue(any(b == "TUNE_ASTRID" and k == "guard" for b, _n, _d, k in note))

    def test_healthy_varied_focus_not_flagged(self) -> None:
        # SHADOW_TRAJECTORY-shape: chosen 22, honored, VARIED args → ok (the key guard)
        t = self._tally({"SHADOW_TRAJECTORY": 22},
                        args={"SHADOW_TRAJECTORY": [f"lambda-{i}" for i in range(22)]})
        warn, note = _assess_stuck(t)
        self.assertEqual(warn, [])
        self.assertEqual(note, [])

    def test_identical_arg_honored_is_notice(self) -> None:
        # EXPERIMENT_ADVANCE-shape: honored 10×, identical arg → possible no-progress
        t = self._tally({"EXPERIMENT_ADVANCE": 10},
                        args={"EXPERIMENT_ADVANCE": ["exp_legacy_self"] * 10})
        warn, note = _assess_stuck(t)
        self.assertEqual(warn, [])
        self.assertTrue(any(b == "EXPERIMENT_ADVANCE" and k == "identical-arg"
                            for b, _n, _d, k in note))

    def test_intentional_idle_ignored(self) -> None:
        t = self._tally({"REST": 9}, args={"REST": [""] * 9})
        warn, note = _assess_stuck(t)
        self.assertEqual(warn, [])
        self.assertEqual(note, [])

    def test_below_repeat_min_ignored(self) -> None:
        t = self._tally({"DOSSIER_CLAIM": 2}, unknown={"DOSSIER_CLAIM": 2},
                        args={"DOSSIER_CLAIM": ["x", "x"]})
        warn, note = _assess_stuck(t)
        self.assertEqual(warn, [])


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

    def test_log_error_rate_distinguishes_stale_from_active(self) -> None:
        from tempfile import TemporaryDirectory

        global ASTRID_BRIDGE_LOG, MINIME_LOGS_DIR
        old_bridge_log = ASTRID_BRIDGE_LOG
        old_minime_logs_dir = MINIME_LOGS_DIR
        try:
            with TemporaryDirectory() as tmpdir:
                d = Path(tmpdir)
                log = d / "host-sensory.log"
                log.write_text("\n".join("ERROR connection refused" for _ in range(60)))
                mtime = time.time() - 31 * 60
                os.utime(log, (mtime, mtime))

                ASTRID_BRIDGE_LOG = d / "missing-bridge.log"
                MINIME_LOGS_DIR = d
                finding = probe_log_error_rate({})
        finally:
            ASTRID_BRIDGE_LOG = old_bridge_log
            MINIME_LOGS_DIR = old_minime_logs_dir

        self.assertEqual(finding["severity"], "ok")
        self.assertEqual(finding["snapshot"]["active_errors"], 0)
        self.assertEqual(finding["snapshot"]["stale_errors"], 60)
        self.assertIn("historical", finding["summary"])

    def test_log_error_rate_downgrades_unchanged_recent_errors(self) -> None:
        from tempfile import TemporaryDirectory

        global ASTRID_BRIDGE_LOG, MINIME_LOGS_DIR
        old_bridge_log = ASTRID_BRIDGE_LOG
        old_minime_logs_dir = MINIME_LOGS_DIR
        try:
            with TemporaryDirectory() as tmpdir:
                d = Path(tmpdir)
                log = d / "host-sensory.log"
                log.write_text("\n".join("ERROR connection refused" for _ in range(60)))
                mtime = time.time() - 60
                os.utime(log, (mtime, mtime))
                stat = log.stat()

                ASTRID_BRIDGE_LOG = d / "missing-bridge.log"
                MINIME_LOGS_DIR = d
                finding = probe_log_error_rate(
                    {
                        "_blind_spots_last_run": time.time(),
                        "files": {
                            str(log): {
                                "errors": 60,
                                "mtime": stat.st_mtime,
                                "size": stat.st_size,
                            }
                        },
                    }
                )
        finally:
            ASTRID_BRIDGE_LOG = old_bridge_log
            MINIME_LOGS_DIR = old_minime_logs_dir

        self.assertEqual(finding["severity"], "ok")
        self.assertEqual(finding["snapshot"]["active_errors"], 0)
        self.assertEqual(finding["snapshot"]["settled_recent_errors"], 60)
        self.assertIn("settled", finding["summary"])

    def test_log_error_rate_ignores_expected_ws_disconnect(self) -> None:
        from tempfile import TemporaryDirectory

        global ASTRID_BRIDGE_LOG, MINIME_LOGS_DIR
        old_bridge_log = ASTRID_BRIDGE_LOG
        old_minime_logs_dir = MINIME_LOGS_DIR
        try:
            with TemporaryDirectory() as tmpdir:
                d = Path(tmpdir)
                log = d / "minime-engine.log"
                log.write_text(
                    "WS recv error: WebSocket protocol error: "
                    "Connection reset without closing handshake\n"
                    "WS recv error: IO error: Connection reset by peer (os error 54)\n"
                    "WS recv error: ❌ Client disconnected: 127.0.0.1:59592\n"
                    "WebSocket protocol error: ❌ Client disconnected: 127.0.0.1:59592\n"
                )

                ASTRID_BRIDGE_LOG = d / "missing-bridge.log"
                MINIME_LOGS_DIR = d
                finding = probe_log_error_rate({})
        finally:
            ASTRID_BRIDGE_LOG = old_bridge_log
            MINIME_LOGS_DIR = old_minime_logs_dir

        self.assertEqual(finding["severity"], "ok")
        self.assertEqual(finding["snapshot"]["active_errors"], 0)
        self.assertEqual(finding["snapshot"]["ignored_transient_errors"], 4)
        self.assertIn("0 actionable errors", finding["summary"])

    def test_log_error_rate_surfaces_active_bridge_errors(self) -> None:
        # Regression guard for the structural blind spot: the probe must actually
        # SCAN the live bridge log (ASTRID_BRIDGE_LOG) and surface a fresh,
        # non-benign ERROR — not silently report "no errors" as it did when
        # ASTRID_BRIDGE_LOG pointed at the empty workspace/bridge.log.
        from tempfile import TemporaryDirectory

        global ASTRID_BRIDGE_LOG, MINIME_LOGS_DIR
        old_bridge_log = ASTRID_BRIDGE_LOG
        old_minime_logs_dir = MINIME_LOGS_DIR
        try:
            with TemporaryDirectory() as tmpdir:
                d = Path(tmpdir)
                bridge = d / "bridge.log"
                bridge.write_text(
                    "INFO ordinary line\n"
                    "ERROR spectral_bridge_server::evolve: synthetic unexpected failure\n"
                )
                empty_minime = d / "minime_logs"
                empty_minime.mkdir()
                ASTRID_BRIDGE_LOG = bridge
                MINIME_LOGS_DIR = empty_minime
                finding = probe_log_error_rate({})
        finally:
            ASTRID_BRIDGE_LOG = old_bridge_log
            MINIME_LOGS_DIR = old_minime_logs_dir

        self.assertEqual(finding["snapshot"]["active_errors"], 1)
        self.assertIn(str(bridge), finding["snapshot"]["files"])

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


class IntrospectiveSignalTests(unittest.TestCase):
    def test_felt_prose_outscores_telemetry(self):
        felt = (
            "I feel the texture thinning — I want to soften the friction; it is like "
            "carving rather than casting. I choose to inhabit the density instead of bracing."
        )
        telem = (
            "Eigenvalue cascade: [9.9, 2.5, 1.6, 0.6]. Fill 71.1%. Spread 8. "
            "ESN leak 0.150. lambda1 dominance 33%."
        )
        fs, _ = introspective_depth(felt)
        ts, _ = introspective_depth(telem)
        self.assertGreater(fs, ts)
        self.assertGreaterEqual(fs, INTROSPECT_THRESHOLD)

    def test_qualia_and_desire_detected(self):
        _, sig = introspective_depth("the viscous silt drags; I'd soften the clamp at line 55")
        self.assertIn("viscous", sig["qualia"])
        self.assertGreaterEqual(sig["desire"], 1)

    def test_surface_type_strips_timestamp(self):
        self.assertEqual(_surface_type("aspiration_longform_1780921053.txt"), "aspiration_longform")
        self.assertEqual(_surface_type("moment_2026-06-08T06-40-11.txt"), "moment")
        self.assertEqual(_surface_type("sovereignty_check_2026-06-08T06-17-25.log"), "sovereignty_check")

    def test_relative_bar_floors_and_discriminates(self):
        # all-low sample -> the absolute floor applies (no false standouts)
        self.assertEqual(_relative_bar([2, 2, 2]), INTROSPECT_ABS_FLOOR)
        # a deep sample -> bar sits above the median so only true standouts pass
        bar = _relative_bar([10, 12, 14, 16, 30])
        self.assertGreater(bar, 14)
        self.assertLessEqual(bar, 30)

    def test_acted_matches_path_or_basename(self):
        p = Path("/x/y/moment_123.txt")
        self.assertTrue(_acted(p, {"moment_123.txt"}))
        self.assertTrue(_acted(p, {"/x/y/moment_123.txt"}))
        self.assertFalse(_acted(p, {"other.txt"}))


class AskTrackerTests(unittest.TestCase):
    def test_match_ask_by_anchor_and_best(self):
        asks = {
            "porosity": {"name": "porosity", "status": "awaiting",
                         "anchors": ["porosity", "aperture", "courtyard"]},
            "keepfloor": {"name": "keepfloor", "status": "awaiting",
                          "anchors": ["keep_floor", "0.87"]},
        }
        self.assertEqual(
            _match_ask("i want a wider aperture, a courtyard not a corridor", asks), "porosity")
        self.assertEqual(
            _match_ask("lower the keep_floor toward 0.87 please", asks), "keepfloor")
        self.assertIsNone(_match_ask("the eigenvalue cascade is steep today", asks))

    def test_asks_ledger_roundtrip_atomic(self):
        import tempfile
        orig = ASKS_PATH
        try:
            with tempfile.TemporaryDirectory() as d:
                globals()["ASKS_PATH"] = Path(d) / "asks.json"
                save_asks({"asks": {"x": {"name": "x", "status": "open", "anchors": ["a"]}}})
                self.assertTrue((Path(d) / "asks.json").is_file())
                self.assertFalse((Path(d) / "asks.json.tmp").exists())  # tmp cleaned by replace
                led = load_asks()
                self.assertEqual(led["asks"]["x"]["anchors"], ["a"])
        finally:
            globals()["ASKS_PATH"] = orig


class CapacityProbeTests(unittest.TestCase):
    def test_empty(self):
        self.assertEqual(_capacity_assess([])["severity"], "ok")

    def test_concentrated_is_ok(self):
        recs = [{"minime": {"utilization": 0.12, "pr": 15, "N": 128, "verdict": "concentrated"}, "triple": []}]
        self.assertEqual(_capacity_assess(recs)["severity"], "ok")

    def test_saturating_warns(self):
        recs = [{"minime": {"utilization": 0.78, "pr": 100, "N": 128, "verdict": "saturating"}, "triple": []}]
        self.assertEqual(_capacity_assess(recs)["severity"], "warning")

    def test_util_rising_notice(self):
        recs = [{"minime": {"utilization": 0.20}, "triple": []} for _ in range(4)]
        recs.append({"minime": {"utilization": 0.40}, "triple": []})
        self.assertEqual(_capacity_assess(recs)["severity"], "notice")

    def test_entropy_drop_notice(self):
        recs = [{"minime": {}, "triple": [{"handle": "astrid", "mean_service_entropy": 0.40}]} for _ in range(4)]
        recs.append({"minime": {}, "triple": [{"handle": "astrid", "mean_service_entropy": 0.22}]})
        self.assertEqual(_capacity_assess(recs)["severity"], "notice")


class StewardOutreachTests(unittest.TestCase):
    def test_empty_ok(self):
        self.assertEqual(_assess_outreach([])["severity"], "ok")

    def test_recent_unread_is_notice(self):
        items = [{"being": "astrid", "file": "steward_query_x_1.txt", "kind": "query", "age_s": 600.0, "subject": "x"}]
        self.assertEqual(_assess_outreach(items)["severity"], "notice")

    def test_old_unread_alarms_pickup_failing(self):
        items = [{"being": "astrid", "file": "steward_query_x_1.txt", "kind": "query",
                  "age_s": OUTREACH_ALARM_SECS + 10, "subject": "x"}]
        a = _assess_outreach(items)
        self.assertEqual(a["severity"], "warning")
        self.assertIn("PICKUP FAILING", a["summary"])


class FeedbackCoverageTests(unittest.TestCase):
    def test_empty_ok(self):
        self.assertEqual(_assess_coverage([])["severity"], "ok")

    def test_zero_pending_ignored(self):
        s = [{"name": "x", "kind": "request", "consumer": "c", "pending": 0,
              "oldest_age_s": 0.0, "exists": True}]
        self.assertEqual(_assess_coverage(s)["severity"], "ok")

    def test_fresh_request_backlog_is_notice(self):
        s = [{"name": "x", "kind": "request", "consumer": "c", "pending": 3,
              "oldest_age_s": 3600.0, "exists": True}]
        self.assertEqual(_assess_coverage(s)["severity"], "notice")

    def test_stale_request_backlog_warns(self):
        s = [{"name": "astrid_agency_requests", "kind": "request", "consumer": "c",
              "pending": 12, "oldest_age_s": FEEDBACK_COVERAGE_ALARM_SECS + 10, "exists": True}]
        a = _assess_coverage(s)
        self.assertEqual(a["severity"], "warning")
        self.assertIn("STALE", a["summary"])

    def test_stale_review_request_warns_steward_action_only(self):
        s = [{"name": "astrid_review_requests", "kind": "request",
              "consumer": REVIEW_REQUEST_CONSUMER, "pending": 1,
              "oldest_age_s": FEEDBACK_COVERAGE_ALARM_SECS + 10, "exists": True}]
        a = _assess_coverage(s)
        detail = "\n".join(a["details"])
        self.assertEqual(a["severity"], "warning")
        self.assertIn("steward action required", a["summary"])
        self.assertIn("steward action required", detail)
        self.assertIn("reword/withdraw", detail)
        self.assertIn("never being follow-up", detail)
        self.assertNotIn("being reviews", detail)

    def test_notice_surface_never_warns(self):
        # context_overflow-style surface: chronic signal, not an unread queue —
        # stays "notice" even when very old, so the probe never cries wolf.
        s = [{"name": "astrid_context_overflow", "kind": "notice", "consumer": "c",
              "pending": 99, "oldest_age_s": FEEDBACK_COVERAGE_ALARM_SECS * 10, "exists": True}]
        self.assertEqual(_assess_coverage(s)["severity"], "notice")


class StatedParamIntentTests(unittest.TestCase):
    def test_parses_numeric_and_regime_footer(self) -> None:
        text = "prose about dense terrain.\n\nREGIME: breathe\nexploration_noise=0.12\n"
        self.assertEqual(
            _stated_footer_directives(text),
            {"exploration_noise": 0.12, "regime": "breathe"},
        )

    def test_prose_mention_is_ignored(self) -> None:
        self.assertEqual(
            _stated_footer_directives("I want to raise exploration_noise to 0.12 soon."),
            {},
        )

    def test_divergence_detected_numeric_and_regime(self) -> None:
        stated = {"exploration_noise": (0.12, 60.0), "regime": ("breathe", 60.0)}
        applied = {"exploration_noise": 0.1, "regime": "focus", "geom_curiosity": 0.1}
        divs = _stated_intent_divergences(stated, applied)
        self.assertEqual(len(divs), 2)
        self.assertTrue(any("exploration_noise" in d for d in divs))
        self.assertTrue(any("regime" in d for d in divs))

    def test_aligned_is_no_divergence(self) -> None:
        stated = {"exploration_noise": (0.12, 60.0)}
        applied = {"exploration_noise": 0.12, "regime": "focus"}
        self.assertEqual(_stated_intent_divergences(stated, applied), [])

    def test_missing_applied_key_skipped(self) -> None:
        stated = {"geom_curiosity": (0.2, 60.0)}
        applied = {"exploration_noise": 0.12}  # no geom_curiosity applied → skip
        self.assertEqual(_stated_intent_divergences(stated, applied), [])

    def test_scan_reads_newest_per_param(self) -> None:
        import os
        from tempfile import TemporaryDirectory
        with TemporaryDirectory() as tmp:
            d = Path(tmp)
            (d / "reply_old.txt").write_text("body\nexploration_noise=0.10\n")
            os.utime(d / "reply_old.txt", (1000, 1000))
            (d / "reply_new.txt").write_text("body\nexploration_noise=0.12\n")
            os.utime(d / "reply_new.txt", (2000, 2000))
            got = _scan_minime_stated_footers([d], now=2100.0, max_age_s=10_000)
            self.assertIn("exploration_noise", got)
            self.assertEqual(got["exploration_noise"][0], 0.12)  # newest wins

    def test_scan_skips_stale_files(self) -> None:
        import os
        from tempfile import TemporaryDirectory
        with TemporaryDirectory() as tmp:
            d = Path(tmp)
            (d / "reply_stale.txt").write_text("body\nexploration_noise=0.12\n")
            os.utime(d / "reply_stale.txt", (1000, 1000))
            got = _scan_minime_stated_footers([d], now=1000.0 + 99_999, max_age_s=3600)
            self.assertEqual(got, {})  # older than max_age → ignored


class AuthorityRequestsTests(unittest.TestCase):
    def test_empty_ok(self):
        self.assertEqual(_assess_authority([])["severity"], "ok")

    def test_submitted_pending_unanswered_warns(self):
        s = [{"exists": True, "thread": "t", "pending_unanswered": 1,
              "pending_scopes": ["mode_release_microdose"], "drafts": 0,
              "top_draft_scope": None, "grants": 0}]
        a = _assess_authority(s)
        self.assertEqual(a["severity"], "warning")
        self.assertIn("UNANSWERED", a["summary"])

    def test_draft_pileup_zero_grants_is_notice(self):
        s = [{"exists": True, "thread": "t", "pending_unanswered": 0, "pending_scopes": [],
              "drafts": AUTHORITY_DRAFT_NOTICE + 5, "top_draft_scope": "semantic_microdose",
              "grants": 0}]
        self.assertEqual(_assess_authority(s)["severity"], "notice")

    def test_drafts_with_a_grant_is_ok(self):
        # once the pipeline has granted at least once, ongoing draft churn is normal.
        s = [{"exists": True, "thread": "t", "pending_unanswered": 0, "pending_scopes": [],
              "drafts": AUTHORITY_DRAFT_NOTICE + 5, "top_draft_scope": "semantic_microdose",
              "grants": 2}]
        self.assertEqual(_assess_authority(s)["severity"], "ok")

    def test_mixed_one_stuck_one_granted_is_notice(self):
        # a grant in ONE thread does not clear a DIFFERENT stuck thread (per-thread).
        s = [{"exists": True, "thread": "granted", "pending_unanswered": 0, "pending_scopes": [],
              "drafts": AUTHORITY_DRAFT_NOTICE + 5, "top_draft_scope": "semantic_microdose", "grants": 1},
             {"exists": True, "thread": "stuck", "pending_unanswered": 0, "pending_scopes": [],
              "drafts": AUTHORITY_DRAFT_NOTICE + 5, "top_draft_scope": "semantic_microdose", "grants": 0}]
        a = _assess_authority(s)
        self.assertEqual(a["severity"], "notice")
        self.assertIn("stuck", a["summary"])

    def test_scan_matches_grant_to_pending_and_ignores_research_budget(self):
        import tempfile
        with tempfile.TemporaryDirectory() as tmp:
            d = Path(tmp) / "th_x"
            d.mkdir()
            gate = d / "authority_gate.jsonl"
            gate.write_text("\n".join([
                json.dumps({"record_type": "request_draft", "scope": "mode_release_microdose"}),
                json.dumps({"record_type": "submitted", "status": "pending_steward_approval",
                            "scope": "mode_release_microdose", "request_id": "r1"}),
                json.dumps({"record_type": "steward_approval", "request_id": "r1"}),
                json.dumps({"record_type": "submitted", "status": "pending_steward_approval",
                            "scope": "mode_release_microdose", "request_id": "r2"}),
                # research-budget self-activates → NOT counted as steward-gated:
                json.dumps({"record_type": "submitted", "status": "pending_steward_approval",
                            "scope": "read_only_research", "request_id": "r3"}),
            ]))
            res = _scan_authority_ledger(gate)
        self.assertEqual(res["pending_unanswered"], 1)  # r1 granted, r2 open, r3 ignored
        self.assertEqual(res["pending_scopes"], ["mode_release_microdose"])
        self.assertEqual(res["drafts"], 1)
        self.assertEqual(res["grants"], 1)
        self.assertEqual(res.get("web_research_pending"), 0)  # local research budget self-activates

    def test_web_research_budget_surfaced_as_operator_gated(self):
        # web-scoped research budget does NOT self-activate (steward_approval_required)
        # — it must be surfaced (notice) so it cannot age silently. cf. un-muffle.
        import tempfile
        with tempfile.TemporaryDirectory() as tmp:
            d = Path(tmp) / "th_web"
            d.mkdir()
            gate = d / "authority_gate.jsonl"
            gate.write_text("\n".join([
                # web request, ungranted → counts
                json.dumps({"record_type": "research_budget_request", "scope": "read_only_research",
                            "status": "pending_steward_approval", "steward_approval_required": True,
                            "record_id": "w1"}),
                # web request answered by source_request_record_id link → does NOT count
                json.dumps({"record_type": "research_budget_request", "scope": "read_only_research",
                            "status": "pending_steward_approval", "steward_approval_required": True,
                            "record_id": "w2"}),
                json.dumps({"record_type": "research_budget_approval", "source_request_record_id": "w2",
                            "status": "active"}),
                # web request answered by the CANONICAL Rust grant shape (budget_id link, no
                # source_request_record_id) → must ALSO not count (the false-positive-nag fix)
                json.dumps({"record_type": "research_budget_request", "scope": "read_only_research",
                            "status": "pending_steward_approval", "steward_approval_required": True,
                            "record_id": "w3", "budget_id": "resbud_b3"}),
                json.dumps({"record_type": "research_budget_approval", "budget_id": "resbud_b3",
                            "status": "active"}),
                # local self-activating budget (no steward_approval_required) → does NOT count
                json.dumps({"record_type": "research_budget_request", "scope": "read_only_research",
                            "status": "self_activated", "record_id": "l1"}),
            ]))
            res = _scan_authority_ledger(gate)
        self.assertEqual(res["web_research_pending"], 1)  # only w1 (w2 + w3 both deduped)
        a = _assess_authority([res])
        self.assertEqual(a["severity"], "notice")
        self.assertIn("research-budget", a["summary"])
        self.assertIn("Mike", a["summary"])


def run_self_tests() -> int:
    loader = unittest.TestLoader()
    suite = unittest.TestSuite()
    suite.addTests(loader.loadTestsFromTestCase(ConvergenceTests))
    suite.addTests(loader.loadTestsFromTestCase(JournalHygieneProbeTests))
    suite.addTests(loader.loadTestsFromTestCase(IntrospectiveSignalTests))
    suite.addTests(loader.loadTestsFromTestCase(AskTrackerTests))
    suite.addTests(loader.loadTestsFromTestCase(CapacityProbeTests))
    suite.addTests(loader.loadTestsFromTestCase(StewardOutreachTests))
    suite.addTests(loader.loadTestsFromTestCase(FeedbackCoverageTests))
    suite.addTests(loader.loadTestsFromTestCase(AuthorityRequestsTests))
    suite.addTests(loader.loadTestsFromTestCase(ChannelIntegrityTests))
    suite.addTests(loader.loadTestsFromTestCase(StuckRepetitionTests))
    suite.addTests(loader.loadTestsFromTestCase(StatedParamIntentTests))
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
    sp_intro = sub.add_parser(
        "introspection",
        help="Cross-surface introspective-signal scan (entries to read + close loops on)",
    )
    sp_intro.add_argument(
        "--ack", nargs="*", metavar="FILE",
        help="mark entries handled (by filename) so they stop re-surfacing in act-now",
    )
    sp_asks = sub.add_parser(
        "asks",
        help="Per-ask lifecycle ledger (held asks are attributed, not re-acted)",
    )
    sp_asks.add_argument("--new", metavar="ID", help="open a new ask")
    sp_asks.add_argument("--anchors", metavar="A,B,C", help="comma-separated distinctive anchor terms (with --new)")
    sp_asks.add_argument("--being", metavar="WHO", choices=["both", "astrid", "minime"], help="which being (with --new; default both)")
    sp_asks.add_argument("--status", metavar="S", choices=list(ASK_STATUSES), help="initial status (with --new; default open)")
    sp_asks.add_argument("--set-status", nargs=2, metavar=("ID", "STATUS"), help="set an ask's status")
    sp_asks.add_argument("--resolve", metavar="ID", help="mark an ask resolved")
    sp_asks.add_argument("--note", nargs=2, metavar=("ID", "TEXT"), help="append a dated segment to an ask's note (use --note-replace to overwrite)")
    sp_asks.add_argument("--note-replace", action="store_true", help="with --note: overwrite the note instead of appending")
    sp_asks.add_argument("--add-anchors", nargs=2, metavar=("ID", "A,B,C"), help="add comma-separated anchor terms to an existing ask (dedup)")
    for s in (sp_all, sp_blind, sp_conv, sp_intro, sp_asks):
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
    elif args.cmd == "introspection":
        report = run_introspection(ack_ids=getattr(args, "ack", None))
        out = json.dumps(report, indent=2, default=str) if use_json else render_blind_spots_md(report)
    elif args.cmd == "asks":
        ledger = run_asks(args)
        out = json.dumps(ledger, indent=2, default=str) if use_json else render_asks_md(ledger)
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
