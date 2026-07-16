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

import being_privacy

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
MINIME_RUNTIME_DIR = MINIME_REPO / "workspace/runtime"
MINIME_CAMERA_STATUS = MINIME_RUNTIME_DIR / "camera_status.json"
MINIME_MIC_STATUS = MINIME_RUNTIME_DIR / "mic_status.json"
MINIME_SENSORY_SOURCE = MINIME_RUNTIME_DIR / "sensory_source.json"

ASTRID_BRIDGE_DB = ASTRID_REPO / "capsules/spectral-bridge/workspace/bridge.db"
ASTRID_DIAGNOSTICS_DIR = ASTRID_REPO / "capsules/spectral-bridge/workspace/diagnostics"
ASTRID_INTROSPECTIONS_DIR = ASTRID_REPO / "capsules/spectral-bridge/workspace/introspections"
INTROSPECTION_ADDRESSING_STATE_DIR = ASTRID_DIAGNOSTICS_DIR / "introspection_addressing_v1"
SANDBOX_TRIAL_QUEUE_STATE_DIR = ASTRID_DIAGNOSTICS_DIR / "sandbox_trial_queue_v1"
AGENCY_CORRIDOR_STATE_DIR = ASTRID_DIAGNOSTICS_DIR / "agency_corridor_v1"
AGENCY_CORRIDOR_V2_STATE_DIR = ASTRID_DIAGNOSTICS_DIR / "agency_corridor_v2"
CONTEXT_PACKING_PRESSURE_PATH = ASTRID_DIAGNOSTICS_DIR / "context_packing_pressure_v1.jsonl"
MINIME_CONDITION_METRICS = MINIME_REPO / "workspace/condition_metrics.json"

MINIME_HEALTH = MINIME_REPO / "workspace/health.json"
MINIME_SOVEREIGNTY_STATE = MINIME_REPO / "workspace/sovereignty_state.json"

STATE_PATH = Path("/tmp/proactive_scan_state.json")
# Per-ask lifecycle ledger — DURABLE (survives reboot/tmp-wipe), unlike STATE_PATH.
# Asks are long-lived stewardship triage state; entry-level dedup (seen/acted) stays
# ephemeral in STATE_PATH. Steward-only; never surfaced into being prompts.
ASKS_PATH = Path("/Users/v/other/astrid/workspace/steward_asks.json")
STEWARD_CONSEQUENCE_CLOSURES = (
    ASTRID_REPO / "workspace" / "steward_consequence_closures.jsonl"
)
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
CONTEXT_OVERFLOW_LABEL_RE = re.compile(r"^=== \[([^\]]+)\] ===\s*$", re.MULTILINE)
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
        "name": "astrid_context_packing_pressure",
        # Count-only prompt packing diagnostics. These guide steward cleanup but
        # never become a being obligation or stale backlog alarm.
        "root": ASTRID_DIAGNOSTICS_DIR,
        "glob": "context_packing_pressure_v1.jsonl",
        "kind": "notice",
        "consumer": "steward glance (prompt-packing pressure signal)",
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
STUCK_IDLE_BASES = frozenset({
    "REST",
    "PASS",
    "SKIP",
    "NOTICE",
    "JOURNAL",
    "WAIT",
    "NOTICE_AMBIGUITY",
    "FISSURE_TRACE",
    "AMBIGUITY_TRACE",
})
_STUCK_ANSI = re.compile(r"\x1b\[[0-9;]*m")
_ROUTE_LOG_TS = re.compile(r"(\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?Z)")
_ROUTE_NEXT_CHOICE = re.compile(r"Astrid chose NEXT:\s*(.+)$")
INTROSPECTION_ROUTE_WINDOW_SECS = 2 * 3600
INTROSPECTION_ROUTE_STALE_SECS = 86_400
INTROSPECTION_ROUTE_SELF_READ_PREFIXES = ("self_study_",)
INTROSPECTION_ROUTE_ARTIFACT_PREFIXES = (
    "introspection_",
    "self_study_carriage_notice_",
    "thin_introspection_output_",
)
INTROSPECTION_ROUTE_SELF_BASES = ("INTROSPECT", "SELF_STUDY")
_STUCK_TS = re.compile(r"(\d{4}-\d{2}-\d{2})[ T](\d{2}:\d{2}:\d{2})")
_STUCK_DIVERSITY_OVERRIDE = re.compile(
    r"diversity stagnant-loop override:\s*replacing NEXT\s+"
    r"([A-Z_][A-Z0-9_]*)\s*(.*?)\s*->\s*([A-Z_][A-Z0-9_]*)",
)
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
    ),
    re.compile(
        r"WebSocket error: WebSocket protocol error: Connection reset without closing handshake",
        re.IGNORECASE,
    ),
    re.compile(
        r"telemetry WebSocket connection ended .*pong_send_error:IO error: Broken pipe",
        re.IGNORECASE,
    ),
    re.compile(
        r"WS handshake error from 127\.0\.0\.1:.*Handshake not finished",
        re.IGNORECASE,
    ),
    re.compile(
        r"GPU A/V client error: WebSocket protocol error: Handshake not finished",
        re.IGNORECASE,
    ),
]

CAMERA_STARTUP_FAILURE_RE = re.compile(
    r"(Failed to start camera|camera failed to properly initialize)",
    re.IGNORECASE,
)
CAMERA_RECOVERY_RE = re.compile(
    r"(OpenCV camera \d+ started|Sent \d+ frames to GPU server)",
    re.IGNORECASE,
)
HOST_SENSORY_CONNECTION_REFUSED_RE = re.compile(
    r"host-sensory (?:control|audio|video) send "
    r"(?:failed|still failing): .*?(?:connection refused|os error 61)",
    re.IGNORECASE,
)
LOG_ERROR_RE = re.compile(r"\b(ERROR|FATAL|Traceback|Exception|panic)\b", re.IGNORECASE)
LOG_STALE_THRESHOLD_SECONDS = 30 * 60  # 30 min
HOST_SENSORY_SOURCE_FRESH_SECONDS = 20.0
HOST_SENSORY_STATUS_FRESH_SECONDS = 30.0


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


def _load_json_dict(path: Path) -> dict[str, Any]:
    try:
        data = json.loads(path.read_text())
    except Exception:
        return {}
    return data if isinstance(data, dict) else {}


def _tcp_port_open(port: int) -> bool:
    try:
        res = subprocess.run(
            ["lsof", "-nP", f"-iTCP:{port}", "-sTCP:LISTEN"],
            capture_output=True,
            text=True,
            timeout=2,
        )
    except Exception:
        return False
    return res.returncode == 0 and f":{port} (LISTEN)" in res.stdout


def _fresh_json_timestamp(
    payload: dict[str, Any],
    path: Path,
    *,
    now: float,
    max_age_s: float,
) -> bool:
    ts_ms = payload.get("updated_at_ms", payload.get("ts_ms"))
    if isinstance(ts_ms, (int, float)) and now - (float(ts_ms) / 1000.0) <= max_age_s:
        return True
    try:
        return now - path.stat().st_mtime <= max_age_s
    except OSError:
        return False


def _source_lane_physical_healthy(sensory: dict[str, Any], lane: str) -> bool:
    lane_state = sensory.get(lane)
    return (
        isinstance(lane_state, dict)
        and lane_state.get("source") == "physical"
        and lane_state.get("physical_healthy") is True
    )


def _status_source_healthy(payload: dict[str, Any], path: Path, *, now: float) -> bool:
    return (
        payload.get("healthy") is True
        and payload.get("connected") is not False
        and _fresh_json_timestamp(
            payload,
            path,
            now=now,
            max_age_s=HOST_SENSORY_STATUS_FRESH_SECONDS,
        )
    )


def _host_sensory_runtime_healthy(now: float) -> bool:
    sensory = _load_json_dict(MINIME_SENSORY_SOURCE)
    camera = _load_json_dict(MINIME_CAMERA_STATUS)
    mic = _load_json_dict(MINIME_MIC_STATUS)
    return (
        _tcp_port_open(7879)
        and _tcp_port_open(7880)
        and _fresh_json_timestamp(
            sensory,
            MINIME_SENSORY_SOURCE,
            now=now,
            max_age_s=HOST_SENSORY_SOURCE_FRESH_SECONDS,
        )
        and _source_lane_physical_healthy(sensory, "audio")
        and _source_lane_physical_healthy(sensory, "video")
        and _status_source_healthy(mic, MINIME_MIC_STATUS, now=now)
        and _status_source_healthy(camera, MINIME_CAMERA_STATUS, now=now)
    )


def _host_sensory_restart_window_error_count(
    log_name: str,
    matching_lines: list[tuple[int, str]],
    *,
    now: float,
) -> int:
    if log_name != "host-sensory.log" or not _host_sensory_runtime_healthy(now):
        return 0
    return sum(
        1
        for _, line in matching_lines
        if HOST_SENSORY_CONNECTION_REFUSED_RE.search(line)
    )


def _camera_absence_host_fallback_expected() -> bool:
    camera = _load_json_dict(MINIME_CAMERA_STATUS)
    sensory = _load_json_dict(MINIME_SENSORY_SOURCE)
    video = sensory.get("video") if isinstance(sensory.get("video"), dict) else {}
    return (
        camera.get("state") == "device_absent"
        and camera.get("physical_device_present") is False
        and camera.get("fallback_expected") is True
        and video.get("source") == "host"
    )


def _expected_absent_camera_startup_error_count(
    log_name: str, matching_lines: list[tuple[int, str]]
) -> int:
    if log_name != "camera-client.log" or not _camera_absence_host_fallback_expected():
        return 0
    return sum(
        1
        for _, line in matching_lines
        if CAMERA_STARTUP_FAILURE_RE.search(line)
    )


def _recovered_camera_startup_error_count(
    log_name: str, lines: list[str], matching_lines: list[tuple[int, str]]
) -> int:
    """Count camera startup errors that have a later success marker in the tail."""
    if log_name != "camera-client.log":
        return 0
    last_failure_index = max(
        (
            index
            for index, line in enumerate(lines)
            if CAMERA_STARTUP_FAILURE_RE.search(line)
        ),
        default=-1,
    )
    last_recovery_index = max(
        (
            index
            for index, line in enumerate(lines)
            if CAMERA_RECOVERY_RE.search(line)
        ),
        default=-1,
    )
    if last_failure_index < 0 or last_recovery_index <= last_failure_index:
        return 0
    return sum(
        1
        for index, line in matching_lines
        if index <= last_failure_index and CAMERA_STARTUP_FAILURE_RE.search(line)
    )


def _scan_log_error_file(
    lf: Path,
    *,
    prior_files: dict[str, Any],
    prior_run_at: float | None,
    now: float,
) -> dict[str, Any] | None:
    try:
        stat = lf.stat()
        mtime = stat.st_mtime
        size = stat.st_size
    except OSError:
        return None
    try:
        res = subprocess.run(
            ["tail", "-n", "2000", str(lf)],
            capture_output=True,
            text=True,
            timeout=8,
        )
    except Exception:
        return None

    log_lines = res.stdout.splitlines()
    indexed_matching_lines = [
        (index, line) for index, line in enumerate(log_lines) if LOG_ERROR_RE.search(line)
    ]
    matching_lines = [line for _, line in indexed_matching_lines]
    benign_n = sum(1 for line in matching_lines if _is_benign_log_error(line))
    expected_absent_n = _expected_absent_camera_startup_error_count(
        lf.name, indexed_matching_lines
    )
    recovered_n = (
        0
        if expected_absent_n
        else _recovered_camera_startup_error_count(lf.name, log_lines, indexed_matching_lines)
    )
    host_sensory_restart_n = _host_sensory_restart_window_error_count(
        lf.name, indexed_matching_lines, now=now
    )
    n = max(
        0,
        len(matching_lines)
        - benign_n
        - recovered_n
        - expected_absent_n
        - host_sensory_restart_n,
    )
    result: dict[str, Any] = {
        "active_errors": 0,
        "settled_recent_errors": 0,
        "stale_errors": 0,
        "ignored_transient_errors": benign_n,
        "ignored_recovered_errors": recovered_n,
        "ignored_expected_absent_camera_errors": expected_absent_n,
        "ignored_host_sensory_restart_errors": host_sensory_restart_n,
        "active_findings": [],
        "settled_findings": [],
        "stale_findings": [],
        "benign_findings": [],
        "recovered_findings": [],
        "file_snapshot": {
            "errors": n,
            "raw_errors": len(matching_lines),
            "ignored_transient_errors": benign_n,
            "ignored_recovered_errors": recovered_n,
            "ignored_expected_absent_camera_errors": expected_absent_n,
            "ignored_host_sensory_restart_errors": host_sensory_restart_n,
            "mtime": mtime,
            "size": size,
        },
    }
    if benign_n > 0:
        result["benign_findings"].append(
            f"{lf.name}: ignored {benign_n} expected websocket disconnect error(s)"
        )
    if recovered_n > 0:
        result["recovered_findings"].append(
            f"{lf.name}: treated {recovered_n} camera startup error(s) as recovered"
        )
    if expected_absent_n > 0:
        result["recovered_findings"].append(
            f"{lf.name}: treated {expected_absent_n} camera startup error(s) "
            "as expected host-sensory fallback while physical camera is absent"
        )
    if host_sensory_restart_n > 0:
        result["recovered_findings"].append(
            f"{lf.name}: treated {host_sensory_restart_n} connection-refused "
            "send error(s) as recovered/startup-window noise because current "
            "sensory runtime is healthy"
        )
    if n == 0:
        return result

    age_s = now - mtime
    if age_s > LOG_STALE_THRESHOLD_SECONDS:
        result["stale_errors"] = n
        result["stale_findings"].append(
            f"{lf.name}: {n} historical error(s) (log stale {_fmt_duration(age_s)})"
        )
        return result

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
        result["settled_recent_errors"] = n
        result["settled_findings"].append(
            f"{lf.name}: {n} recent settled error(s) "
            f"(log age {_fmt_duration(age_s)}, no writes since prior scan)"
        )
    else:
        result["active_errors"] = n
        result["active_findings"].append(
            f"{lf.name}: {n} new/current error(s) in last 2000 "
            f"(log fresh, age {_fmt_duration(age_s)})"
        )
    return result


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

    now = time.time()

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
    recovered_findings: list[str] = []
    active_errors = 0
    settled_recent_errors = 0
    stale_errors = 0
    benign_transient_errors = 0
    recovered_startup_errors = 0
    expected_absent_camera_errors = 0
    host_sensory_restart_errors = 0
    file_snapshots: dict[str, dict[str, Any]] = {}

    for lf in log_files:
        scanned = _scan_log_error_file(
            lf,
            prior_files=prior_files,
            prior_run_at=prior_run_at,
            now=now,
        )
        if scanned is None:
            continue
        active_errors += int(scanned["active_errors"])
        settled_recent_errors += int(scanned["settled_recent_errors"])
        stale_errors += int(scanned["stale_errors"])
        benign_transient_errors += int(scanned["ignored_transient_errors"])
        recovered_startup_errors += int(scanned["ignored_recovered_errors"])
        expected_absent_camera_errors += int(
            scanned["ignored_expected_absent_camera_errors"]
        )
        host_sensory_restart_errors += int(
            scanned["ignored_host_sensory_restart_errors"]
        )
        active_findings.extend(scanned["active_findings"])
        settled_findings.extend(scanned["settled_findings"])
        stale_findings.extend(scanned["stale_findings"])
        benign_findings.extend(scanned["benign_findings"])
        recovered_findings.extend(scanned["recovered_findings"])
        file_snapshots[str(lf)] = scanned["file_snapshot"]

    snapshot = {
        "active_errors": active_errors,
        "settled_recent_errors": settled_recent_errors,
        "stale_errors": stale_errors,
        "ignored_transient_errors": benign_transient_errors,
        "ignored_recovered_errors": recovered_startup_errors,
        "ignored_expected_absent_camera_errors": expected_absent_camera_errors,
        "ignored_host_sensory_restart_errors": host_sensory_restart_errors,
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
            f"({len(stale_findings)} file(s) untouched >{LOG_STALE_THRESHOLD_SECONDS // 60}m)"
        )
    elif (
        benign_transient_errors > 0
        or recovered_startup_errors > 0
        or expected_absent_camera_errors > 0
        or host_sensory_restart_errors > 0
    ):
        severity = "ok"
        ignored_parts = []
        if benign_transient_errors > 0:
            ignored_parts.append(
                f"ignored {benign_transient_errors} expected transient websocket disconnect(s)"
            )
        if recovered_startup_errors > 0:
            ignored_parts.append(
                f"treated {recovered_startup_errors} recovered startup error(s) as settled"
            )
        if expected_absent_camera_errors > 0:
            ignored_parts.append(
                f"treated {expected_absent_camera_errors} camera startup error(s) "
                "as expected host fallback"
            )
        if host_sensory_restart_errors > 0:
            ignored_parts.append(
                f"treated {host_sensory_restart_errors} host-sensory restart-window "
                "connection-refused error(s) as recovered"
            )
        summary = f"clean — 0 actionable errors; {'; '.join(ignored_parts)}"
    else:
        severity = "ok"
        summary = f"clean — 0 errors across {len(log_files)} log file(s)"

    return _finding(
        "log_error_rate",
        severity,
        summary,
        details=(
            active_findings
            + settled_findings
            + stale_findings
            + benign_findings
            + recovered_findings
        )
        if (
            active_findings
            or settled_findings
            or stale_findings
            or benign_findings
            or recovered_findings
        )
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
# steward), a since-changed dial (glance), or an expected lease-safe clamp.
# Steward-only; never to a being.
_STATED_FOOTER_NUMERIC = ("exploration_noise", "regulation_strength", "geom_curiosity")
_STATED_FOOTER_NUM_RE = re.compile(
    r"^[\s\-*>]*(" + "|".join(_STATED_FOOTER_NUMERIC) + r")\s*[:=]\s*"
    r"([-+]?\d*\.?\d+)\s*[.;,]?\s*$",
    re.IGNORECASE | re.MULTILINE,
)
_STATED_FOOTER_REGIME_RE = re.compile(
    r"^[\s\-*>]*regime(?:\s*[:=]|\s+)\s*(explore|recover|breathe|focus|calm)\s*[.;,]?\s*$",
    re.IGNORECASE | re.MULTILINE,
)
_STATED_FOOTER_SCAN_LINES = 8  # only a reply's tail is a footer
_LEASE_SAFE_FOOTER_CLAMPS = {
    # Minime may state a higher exploration-noise preference while the applied
    # sovereignty state correctly lands at the lease-safe cap.
    "exploration_noise": 0.08,
}


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


def _is_expected_stated_footer_clamp(key: str, stated_val: Any, applied_val: Any) -> bool:
    cap = _LEASE_SAFE_FOOTER_CLAMPS.get(key)
    if cap is None:
        return False
    if not isinstance(stated_val, (int, float)) or not isinstance(applied_val, (int, float)):
        return False
    return stated_val > cap and abs(float(applied_val) - cap) <= 1e-6


def _stated_intent_comparison(
    stated: dict[str, tuple],
    applied: dict[str, Any],
) -> tuple[list[str], list[str]]:
    """Compare stated footers to applied sovereignty_state.

    Returns (review_needed, expected_clamps). A stated value with no applied
    counterpart is skipped (nothing to compare).
    """
    review_needed: list[str] = []
    expected_clamps: list[str] = []
    for key, (val, age) in sorted(stated.items()):
        cur = applied.get(key)
        if cur is None:
            continue
        if isinstance(val, (int, float)) and isinstance(cur, (int, float)):
            diverged = abs(val - cur) > 1e-6
        else:
            diverged = str(val).lower() != str(cur).lower()
        if diverged:
            line = f"{key}: stated {val!r} ({_fmt_duration(age)} ago) != applied {cur!r}"
            if _is_expected_stated_footer_clamp(key, val, cur):
                expected_clamps.append(f"{line} (expected lease-safe clamp/applied cap)")
            else:
                review_needed.append(line)
    return review_needed, expected_clamps


def _stated_intent_divergences(stated: dict[str, tuple], applied: dict[str, Any]) -> list[str]:
    """Return only review-worthy stated/applied divergences."""
    review_needed, _expected_clamps = _stated_intent_comparison(stated, applied)
    return review_needed


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
    divergences, expected_clamps = _stated_intent_comparison(stated, applied)
    if divergences:
        clamp_suffix = (
            f"; {len(expected_clamps)} expected lease-safe clamp(s)"
            if expected_clamps
            else ""
        )
        return _finding(
            "stated_param_intent",
            "notice",
            f"{len(divergences)} stated footer(s) need steward review{clamp_suffix} "
            "(unapplied regime / missing applied value / non-clamp dial drift / "
            "since-changed dial — glance)",
            details=expected_clamps + divergences,
        )
    if expected_clamps:
        return _finding(
            "stated_param_intent",
            "ok",
            f"all {len(stated)} recent stated footer(s) match applied state or expected lease-safe clamps",
            details=expected_clamps,
        )
    return _finding(
        "stated_param_intent",
        "ok",
        f"all {len(stated)} recent stated footer(s) match applied state",
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
        # autonomous_agent.py keeps growing as Codex adds prompt actions; the
        # regex analysis is now ~92s standalone (was ~64s, was <20s). The old
        # 20s cap, then the 120s cap, each crept toward "fail to run" under
        # concurrent scan load — deadening a real drift detector (could miss a
        # new silent-starvation/unwired action). 240s restores ~2x headroom for
        # file growth + load; this is a steward-side background probe, not in a
        # being path. Measured 2026-06-26 (loop:16706).
        timeout=240,
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


def _signal_why(sig: dict[str, Any]) -> list[str]:
    """Compact, machine-friendly reasons an entry scored as a standout."""
    why: list[str] = []
    if sig.get("high_signal"):
        why.append("refs:" + ",".join(sig["high_signal"][:3]))
    if sig.get("desire"):
        why.append(f"{sig['desire']} desire")
    if sig.get("qualia"):
        why.append("felt:" + ",".join(sig["qualia"][:4]))
    if sig.get("agency"):
        why.append(f"{sig['agency']} agency")
    return why


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
    # Per-being standout entries (file + score + status + why), exposed in --json so
    # shortlist/triage work can reuse the flywheel's own ranking instead of re-scanning.
    # NOT persisted in `snapshot` (which is kept small for delta computation).
    standout_entries: dict[str, list[dict[str, Any]]] = {}

    for being, jdir, ts_fn, logs_dir in beings:
        entries = sample_recent_journals(jdir, INTROSPECT_SAMPLE_PER_BEING, ts_fn, being)
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
        being_entries: list[dict[str, Any]] = []
        for s in standouts:
            score, ts, stype, sig, p, body = s
            aid = _match_ask(body.lower(), cand_asks)
            held = aid is not None and str(cand_asks[aid].get("status")) in ASK_HELD_STATES
            if held:
                held_attrib[aid] = held_attrib.get(aid, 0) + 1
                status = f"held:{cand_asks[aid]['name']}"
            elif aid is not None:
                active.append((s, cand_asks[aid]["name"]))
                status = f"open:{cand_asks[aid]['name']}"
            else:
                active.append((s, None))
                status = "unclassified"
            being_entries.append({
                "being": being, "file": str(p), "name": p.name,
                "score": round(score, 1), "surface": stype, "ts": _fmt_ts(ts),
                "status": status, "why": _signal_why(sig),
            })
        standout_entries[being] = being_entries

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
            why = _signal_why(sig)
            ask_label = f"ask:{ask_name}" if ask_name else "unclassified"
            details.append(
                f"  - [{tag}] [{ask_label}] [{stype} @ {_fmt_ts(ts)}] score {score:.0f} — "
                f"{'; '.join(why) or 'first-person depth'} — {p}"
            )
        # Surface the held standouts' FILES too (capped) — they're the high-signal
        # entries tracked under an ask, previously invisible in text (only counted).
        held_entries = [e for e in being_entries if e["status"].startswith("held:")]
        for e in held_entries[:INTROSPECT_TOP_K]:
            details.append(
                f"  ⊟ [{e['status']}] [{e['surface']} @ {e['ts']}] score {e['score']:.0f} — "
                f"{'; '.join(e['why']) or 'depth'} — {e['file']}"
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
    finding = _finding("introspective_signal", severity, summary, details, snapshot)
    finding["standout_entries"] = standout_entries  # in --json, not persisted in snapshot
    return finding


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
    if spec.get("name") == "astrid_context_overflow":
        res["context_overflow_labels"] = _classify_context_overflow_files(files)
    if spec.get("name") == "astrid_context_packing_pressure":
        res["context_packing_pressure"] = _classify_context_packing_pressure_files(files)
    return res


def _classify_context_overflow_files(files: list[Path]) -> dict[str, Any]:
    """Read-only section-label classifier for Astrid context overflow spill files."""
    label_counts: Counter[str] = Counter()
    label_ages: dict[str, list[float]] = defaultdict(list)
    now = time.time()
    sampled = 0
    def _mtime(path: Path) -> float:
        try:
            return path.stat().st_mtime
        except Exception:
            return 0.0

    for path in sorted(files, key=_mtime, reverse=True):
        try:
            text = path.read_text(errors="ignore")
            age_s = max(0.0, now - path.stat().st_mtime)
        except Exception:
            continue
        sampled += 1
        labels = [m.group(1).strip().lower() for m in CONTEXT_OVERFLOW_LABEL_RE.finditer(text)]
        if not labels:
            labels = ["unlabeled"]
        for label in labels:
            label_counts[label] += 1
            label_ages[label].append(age_s)
    top_labels = []
    for label, count in label_counts.most_common(5):
        ages = label_ages.get(label, [])
        top_labels.append({
            "label": label,
            "count": count,
            "newest_age_s": min(ages) if ages else 0.0,
            "oldest_age_s": max(ages) if ages else 0.0,
        })
    recommended_next = _context_overflow_recommended_next(top_labels)
    return {
        "schema_version": 1,
        "sampled_files": sampled,
        "label_counts": dict(label_counts),
        "top_labels": top_labels,
        "recommended_next": recommended_next,
        "severity_policy": "notice_only",
    }


def _context_overflow_recommended_next(top_labels: list[dict[str, Any]]) -> str:
    if not top_labels:
        return "none"
    label_actions = {
        "diversity": "inspect diversity-context packing and stagnant-loop override text",
        "modality": "inspect modality-context packing before adding new sensory prose",
        "perception": "inspect perception-context packing and duplicate readout blocks",
        "review": "inspect review-context packing and close/defer stale steward loops",
        "system": "inspect system-context packing for repeated static instructions",
    }
    actions: list[str] = []
    for item in top_labels[:3]:
        if not isinstance(item, dict):
            continue
        label = str(item.get("label") or "").strip().lower()
        if not label:
            continue
        actions.append(label_actions.get(label, f"inspect {label}-context packing"))
    if not actions:
        return "none"
    return "; ".join(actions)


def _classify_context_packing_pressure_files(files: list[Path]) -> dict[str, Any]:
    """Read-only classifier for count-only context_packing_pressure_v1 JSONL."""
    removed_by_label: Counter[str] = Counter()
    occurrences_by_label: Counter[str] = Counter()
    sampled_records = 0
    latest_ts = 0.0
    for path in files:
        try:
            lines = path.read_text(errors="ignore").splitlines()
        except Exception:
            continue
        for line in lines[-200:]:
            try:
                record = json.loads(line)
            except json.JSONDecodeError:
                continue
            if not isinstance(record, dict):
                continue
            sampled_records += 1
            try:
                latest_ts = max(latest_ts, float(record.get("ts") or 0.0))
            except (TypeError, ValueError):
                pass
            blocks = record.get("blocks")
            if not isinstance(blocks, list):
                continue
            for block in blocks:
                if not isinstance(block, dict):
                    continue
                label = str(block.get("label") or "").strip().lower()
                if not label:
                    continue
                try:
                    removed_chars = int(block.get("removed_chars") or 0)
                except (TypeError, ValueError):
                    removed_chars = 0
                if removed_chars <= 0:
                    continue
                removed_by_label[label] += removed_chars
                occurrences_by_label[label] += 1
    top_labels = [
        {
            "label": label,
            "removed_chars": removed_chars,
            "occurrences": occurrences_by_label[label],
        }
        for label, removed_chars in removed_by_label.most_common(5)
    ]
    recommended_next = _context_packing_pressure_recommended_next(top_labels)
    return {
        "schema_version": 1,
        "sampled_records": sampled_records,
        "latest_ts": latest_ts or None,
        "top_pressure_labels": top_labels,
        "recommended_next": recommended_next,
        "severity_policy": "notice_only",
    }


def _context_packing_pressure_recommended_next(top_labels: list[dict[str, Any]]) -> str:
    if not top_labels:
        return "none"
    label_actions = {
        "continuity": "verify compact continuity recap is reducing repeated history pressure",
        "modality": "verify modality wording follows sensory_freshness_v1 before adding sensory prose",
        "diversity": "inspect diversity-context packing and stagnant-loop override text",
        "feedback": "close, defer, or summarize steward feedback loops before adding prompts",
        "web": "summarize web context before carrying more page text",
    }
    actions: list[str] = []
    for item in top_labels[:3]:
        if not isinstance(item, dict):
            continue
        label = str(item.get("label") or "").strip().lower()
        if not label:
            continue
        actions.append(label_actions.get(label, f"inspect {label}-context packing"))
    if not actions:
        return "none"
    return "; ".join(actions)


def _jsonl_records(path: Path, *, max_records: int = 200) -> list[dict[str, Any]]:
    try:
        lines = path.read_text(errors="ignore").splitlines()[-max_records:]
    except Exception:
        return []
    records: list[dict[str, Any]] = []
    for line in lines:
        try:
            payload = json.loads(line)
        except json.JSONDecodeError:
            continue
        if isinstance(payload, dict):
            records.append(payload)
    return records


def _record_ts(record: dict[str, Any]) -> float:
    try:
        return float(record.get("ts") or 0.0)
    except (TypeError, ValueError):
        return 0.0


def _latest_prefixed_file_mtime(root: Path, prefixes: tuple[str, ...]) -> tuple[float, Path | None]:
    newest_ts = 0.0
    newest_path: Path | None = None
    if not root.exists():
        return newest_ts, newest_path
    for path in root.iterdir():
        if not path.is_file() or not path.name.startswith(prefixes):
            continue
        try:
            mtime = path.stat().st_mtime
        except OSError:
            continue
        if mtime > newest_ts:
            newest_ts = mtime
            newest_path = path
    return newest_ts, newest_path


def _latest_introspection_route_activity(
    journal_dir: Path,
    introspections_dir: Path,
) -> dict[str, Any]:
    journal_ts, journal_path = _latest_prefixed_file_mtime(
        journal_dir,
        INTROSPECTION_ROUTE_SELF_READ_PREFIXES,
    )
    artifact_ts, artifact_path = _latest_prefixed_file_mtime(
        introspections_dir,
        INTROSPECTION_ROUTE_ARTIFACT_PREFIXES,
    )
    if artifact_ts > journal_ts:
        latest_ts, latest_path, latest_kind = artifact_ts, artifact_path, "introspection_artifact"
    else:
        latest_ts, latest_path, latest_kind = journal_ts, journal_path, "journal_self_study"
    return {
        "latest_ts": latest_ts or None,
        "latest_path": str(latest_path) if latest_path else None,
        "latest_kind": latest_kind if latest_ts else None,
    }


def _topline_route_retention(
    pressure_path: Path,
    since_s: float,
) -> dict[str, Any]:
    records = [record for record in _jsonl_records(pressure_path) if _record_ts(record) >= since_s]
    records_with_topline = 0
    removed = 0
    fully_removed = 0
    for record in records:
        blocks = record.get("blocks")
        if not isinstance(blocks, list):
            continue
        for block in blocks:
            if not isinstance(block, dict):
                continue
            if str(block.get("label") or "").strip().lower() != "topline":
                continue
            records_with_topline += 1
            try:
                removed_chars = int(block.get("removed_chars") or 0)
            except (TypeError, ValueError):
                removed_chars = 0
            if removed_chars > 0:
                removed += 1
            if block.get("fully_removed"):
                fully_removed += 1
    if not records:
        status = "no_recent_dialogue_records"
    elif records_with_topline == 0:
        status = "topline_absent"
    elif removed or fully_removed:
        status = "topline_trimmed"
    else:
        status = "topline_retained"
    return {
        "status": status,
        "recent_dialogue_records": len(records),
        "records_with_topline": records_with_topline,
        "records_with_removed_topline": removed,
        "records_with_fully_removed_topline": fully_removed,
    }


def _route_choice_ts(line: str) -> float | None:
    match = _ROUTE_LOG_TS.search(line)
    if not match:
        return None
    try:
        return datetime.fromisoformat(match.group(1).replace("Z", "+00:00")).timestamp()
    except ValueError:
        return None


def _recent_astrid_route_choices(log_path: Path, since_s: float) -> dict[str, Any]:
    try:
        lines = log_path.read_text(errors="ignore").splitlines()[-4000:]
    except Exception:
        lines = []
    counts: Counter[str] = Counter()
    latest: list[str] = []
    for raw in lines:
        line = _STUCK_ANSI.sub("", raw)
        if "Astrid chose NEXT:" not in line:
            continue
        ts = _route_choice_ts(line)
        if ts is not None and ts < since_s:
            continue
        match = _ROUTE_NEXT_CHOICE.search(line)
        if not match:
            continue
        action = match.group(1).strip()
        base = re.split(r"\s+", action, maxsplit=1)[0].strip().upper()
        if not base:
            continue
        counts[base] += 1
        latest.append(action)
    return {
        "choice_count": sum(counts.values()),
        "choice_counts": counts.most_common(8),
        "self_route_choices": sum(
            count for base, count in counts.items() if base.startswith(INTROSPECTION_ROUTE_SELF_BASES)
        ),
        "latest_choices": latest[-6:],
    }


def _classify_introspection_route_cadence(
    *,
    now: float | None = None,
    window_s: float = INTROSPECTION_ROUTE_WINDOW_SECS,
    journal_dir: Path = ASTRID_JOURNAL,
    introspections_dir: Path = ASTRID_INTROSPECTIONS_DIR,
    pressure_path: Path = CONTEXT_PACKING_PRESSURE_PATH,
    log_path: Path = ASTRID_BRIDGE_LOG,
) -> dict[str, Any]:
    now = time.time() if now is None else now
    since_s = now - window_s
    latest = _latest_introspection_route_activity(journal_dir, introspections_dir)
    latest_ts = float(latest.get("latest_ts") or 0.0)
    latest_age_s = max(0.0, now - latest_ts) if latest_ts else None
    fresh_self_read = bool(latest_ts and latest_ts >= since_s)
    stale_self_read = latest_age_s is None or latest_age_s >= INTROSPECTION_ROUTE_STALE_SECS
    topline = _topline_route_retention(pressure_path, since_s)
    choices = _recent_astrid_route_choices(log_path, since_s)
    dialogue_records = int(topline.get("recent_dialogue_records") or 0)
    self_route_choices = int(choices.get("self_route_choices") or 0)
    if fresh_self_read:
        status = "fresh_self_read_landed"
    elif topline["status"] in {"topline_absent", "topline_trimmed"}:
        status = "cue_visibility_needs_review"
    elif stale_self_read and dialogue_records >= 3 and self_route_choices == 0:
        status = "route_cadence_needs_review"
    elif stale_self_read:
        status = "watching_stale_self_read"
    else:
        status = "ok_recent_self_read"
    suggestions = {
        "route_cadence_needs_review": [
            "inspect INTROSPECT/SELF_STUDY route legibility",
            "inspect chooser cadence / competing READ_MORE, PRESSURE_SOURCE_AUDIT, DECOMPOSE, SHADOW_* gravity",
            "do not add more prompt pressure",
        ],
        "cue_visibility_needs_review": [
            "inspect top-line retention before changing route cadence",
            "do not add another prompt cue",
        ],
    }.get(status, [])
    return {
        "schema_version": 1,
        "status": status,
        "latest_self_read": latest,
        "latest_self_read_age_hours": round(latest_age_s / 3600.0, 2) if latest_age_s is not None else None,
        "topline_retention": topline,
        "route_choices": choices,
        "next_suggestions": suggestions,
        "severity_policy": "notice_only",
        "authority_boundary": "read-only steward diagnostic; no prompt pressure, forced self-study, scheduler, or runtime mutation",
    }


def probe_introspection_route_cadence(_prior: dict[str, Any]) -> dict[str, Any]:
    summary = _classify_introspection_route_cadence()
    status = summary["status"]
    if status == "fresh_self_read_landed":
        severity = "ok"
        headline = "fresh Astrid self-study/introspection landed in the recent route window"
    elif status == "route_cadence_needs_review":
        severity = "notice"
        headline = (
            "Astrid self-study route cadence needs steward review: freshness cue visible, "
            f"{summary['topline_retention']['recent_dialogue_records']} dialogue record(s), "
            f"{summary['route_choices']['self_route_choices']} self-route choice(s)"
        )
    elif status == "cue_visibility_needs_review":
        severity = "notice"
        headline = "Astrid introspection freshness cue visibility needs review"
    elif status == "watching_stale_self_read":
        severity = "notice"
        headline = "Astrid self-study remains stale; still gathering enough visible dialogue records"
    else:
        severity = "ok"
        headline = "Astrid self-study/introspection route cadence ok"
    details = [
        f"status={status}; latest_self_read={summary['latest_self_read'].get('latest_path') or 'none'}; age_h={summary.get('latest_self_read_age_hours')}",
        f"topline={summary['topline_retention']['status']} ({summary['topline_retention']['records_with_topline']}/{summary['topline_retention']['recent_dialogue_records']} records)",
        f"recent choices={summary['route_choices']['choice_counts']}",
        "next: " + ("; ".join(summary["next_suggestions"]) or "none"),
        summary["authority_boundary"],
    ]
    return _finding("introspection_route_cadence", severity, headline, details, summary)


def _classify_action_route_legibility(
    *,
    now: float | None = None,
    window_s: float = INTROSPECTION_ROUTE_WINDOW_SECS,
) -> dict[str, Any]:
    now = time.time() if now is None else now
    try:
        from recent_signal_summary import _action_route_legibility_summary
    except Exception as exc:
        return {
            "schema": "action_route_legibility_v1",
            "status": "diagnostic_unavailable",
            "error": str(exc),
            "severity_policy": "notice_only",
            "authority_boundary": "read-only route-legibility diagnostic unavailable; no prompt pressure or runtime mutation",
        }
    summary = _action_route_legibility_summary(now - window_s)
    summary["severity_policy"] = "notice_only"
    return summary


def probe_action_route_legibility(_prior: dict[str, Any]) -> dict[str, Any]:
    summary = _classify_action_route_legibility()
    status = summary.get("status")
    if status == "ok":
        severity = "ok"
        headline = "Astrid action-route legibility/cadence looks coherent"
    elif status == "diagnostic_unavailable":
        severity = "notice"
        headline = "Astrid action-route legibility diagnostic unavailable"
    elif status == "dispatch_wiring_needs_review":
        severity = "notice"
        headline = "Astrid self-study route wiring needs steward review"
    elif status == "self_read_landed_watch_chooser_gravity":
        severity = "ok"
        headline = "Astrid self-study route landed; residual chooser gravity is watch-only"
    else:
        severity = "notice"
        headline = "Astrid self-study route legibility / chooser gravity needs steward review"
    events = summary.get("recent_action_events") or {}
    chooser = summary.get("chooser_surface") or {}
    details = [
        f"status={status}; route_cadence={summary.get('route_cadence_status')}",
        "evidence: " + ("; ".join(summary.get("evidence_summary") or []) or "none"),
        f"recent effective counts={events.get('effective_counts')}",
        f"self-route effective count={events.get('self_route_effective_count')}",
        "analysis breaker competitors="
        + (", ".join(chooser.get("competitors_in_analysis_breakers") or []) or "none"),
        "next: " + ("; ".join(summary.get("next_suggestions") or []) or "none"),
        str(summary.get("authority_boundary") or "read-only diagnostic"),
    ]
    return _finding("action_route_legibility", severity, headline, details, summary)


def _classify_introspection_addressing() -> dict[str, Any]:
    try:
        import introspection_addressing_audit as addressing
    except Exception as exc:
        return {
            "schema": "introspection_addressing_v1",
            "status": "diagnostic_unavailable",
            "summary": {},
            "next_queue": [],
            "error": str(exc),
            "severity_policy": "notice_only",
            "authority_boundary": "read-only introspection-addressing diagnostic unavailable; no runtime, prompt, deploy, staging, or commit action",
        }
    try:
        report = addressing.build_report(INTROSPECTION_ADDRESSING_STATE_DIR)
    except Exception as exc:
        return {
            "schema": "introspection_addressing_v1",
            "status": "database_corrupt",
            "summary": {},
            "next_queue": [],
            "error": str(exc),
            "severity_policy": "notice_only",
            "authority_boundary": addressing.AUTHORITY_BOUNDARY,
        }
    report["severity_policy"] = "notice_only"
    return report


def probe_introspection_addressing(_prior: dict[str, Any]) -> dict[str, Any]:
    summary = _classify_introspection_addressing()
    status = str(summary.get("status") or "unknown")
    setup_statuses = {
        "database_missing",
        "database_corrupt",
        "database_corrupt_lines_ignored",
        "cutoff_not_indexed",
        "diagnostic_unavailable",
    }
    if status in setup_statuses:
        severity = "notice"
        headline = "Astrid introspection-addressing ledger needs steward setup/review"
    else:
        severity = "ok"
        headline = "Astrid introspection-addressing ledger is tracking queue progress"
    counts = summary.get("summary") or {}
    next_queue = summary.get("next_queue") or []
    details = [
        f"status={status}",
        (
            "counts="
            f"total={counts.get('total_indexed', 0)} "
            f"canonical={counts.get('canonical_indexed', 0)} "
            f"full_read={counts.get('full_read_count', 0)} "
            f"fully_addressed={counts.get('fully_addressed_count', 0)} "
            f"pending={counts.get('pending_count', 0)} "
            f"blocked={counts.get('blocked_count', 0)}"
        ),
        "next_queue="
        + (
            ", ".join(
                str(item.get("filename") or item.get("introspection_id") or "unknown")
                for item in next_queue[:3]
                if isinstance(item, dict)
            )
            or "none"
        ),
        f"top_source_families={counts.get('top_source_families') or []}",
        str(summary.get("authority_boundary") or "read-only diagnostic"),
    ]
    return _finding("introspection_addressing", severity, headline, details, summary)


def _classify_feedback_flywheel(
    *,
    now: float | None = None,
    window_s: float = INTROSPECTION_ROUTE_WINDOW_SECS,
) -> dict[str, Any]:
    now = time.time() if now is None else now
    try:
        from recent_signal_summary import _feedback_flywheel_summary
    except Exception as exc:
        return {
            "schema": "feedback_flywheel_v1",
            "status": "database_needs_review",
            "error": str(exc),
            "severity_policy": "notice_warning_only",
            "authority_boundary": "read-only feedback flywheel diagnostic unavailable; no agency grant, runtime, prompt, deploy, staging, or commit action",
        }
    summary = _feedback_flywheel_summary(now - window_s)
    summary["severity_policy"] = "notice_warning_only"
    return summary


def probe_feedback_flywheel(_prior: dict[str, Any]) -> dict[str, Any]:
    summary = _classify_feedback_flywheel()
    status = str(summary.get("status") or "unknown")
    work = summary.get("work_item_summary") or {}
    if status in {"database_needs_review", "tier_mismatch_needs_review", "grant_waiting"}:
        severity = "warning"
        if status == "database_needs_review":
            headline = "Feedback flywheel ledger needs setup or repair"
        elif status == "tier_mismatch_needs_review":
            headline = "Feedback flywheel has live-control work below Tier 5"
        else:
            headline = "Feedback flywheel has Tier 4/5 agency grant waiting"
    elif status == "post_change_response_needed":
        severity = "notice"
        headline = "Feedback flywheel is awaiting post-change felt response"
    elif status == "action_backlog":
        severity = "notice"
        headline = "Feedback flywheel has active work items to metabolize"
    else:
        severity = "ok"
        headline = "Feedback flywheel is healthy/watch-only"
    volume = summary.get("fresh_signal_volume") or {}
    details = [
        f"status={status}",
        (
            "fresh_signal="
            f"window_canonical={volume.get('window_canonical', 0)} "
            f"window_total={volume.get('window_total', 0)} "
            f"last_48h_canonical={volume.get('last_48h_canonical', 0)}"
        ),
        (
            "work_items="
            f"active={work.get('active_work_items', 0)} "
            f"by_tier={work.get('by_tier', {})} "
            f"by_status={work.get('by_status', {})}"
        ),
        (
            "waits="
            f"grant={work.get('grant_waiting_count', 0)} "
            f"felt_response={work.get('post_change_awaiting_response_count', 0)} "
            f"stale={work.get('stale_work_count', 0)} "
            f"tier_mismatch={work.get('tier_mismatch_count', 0)}"
        ),
        "tier_mismatches=" + json.dumps(work.get("tier_mismatches") or []),
        "next_work="
        + (
            ", ".join(
                str(item.get("work_item_id") or "unknown")
                for item in (summary.get("next_work_queue") or [])[:3]
                if isinstance(item, dict)
            )
            or "none"
        ),
        "next: " + ("; ".join(summary.get("next_suggestions") or []) or "none"),
        str(summary.get("authority_boundary") or "read-only diagnostic"),
    ]
    return _finding("feedback_flywheel", severity, headline, details, summary)


def _classify_sandbox_trial_queue() -> dict[str, Any]:
    try:
        import sandbox_trial_queue
    except Exception as exc:
        return {
            "schema": "sandbox_trial_queue_v1",
            "status": "diagnostic_unavailable",
            "error": str(exc),
            "severity_policy": "warn_only_for_missing_corrupt_or_authority_violation",
            "authority_boundary": "read-only sandbox trial queue diagnostic unavailable; no runtime, prompt, controller, fallback, deploy, staging, or commit action",
        }
    summary = sandbox_trial_queue.build_report(SANDBOX_TRIAL_QUEUE_STATE_DIR)
    summary["severity_policy"] = "warn_only_for_missing_corrupt_or_authority_violation"
    return summary


def probe_sandbox_trial_queue(_prior: dict[str, Any]) -> dict[str, Any]:
    summary = _classify_sandbox_trial_queue()
    status = str(summary.get("status") or "unknown")
    counts = summary.get("summary") or {}
    ladder = summary.get("consentful_sandbox_to_live_ladder_v1") or {}
    ladder_summary = ladder.get("summary") if isinstance(ladder.get("summary"), dict) else {}
    closure_loop = summary.get("being_outcome_closure_loop_v1") or {}
    closure_summary = closure_loop.get("summary") if isinstance(closure_loop.get("summary"), dict) else {}
    if status in {
        "diagnostic_unavailable",
        "database_missing",
        "database_corrupt_lines_ignored",
        "authority_violation",
    }:
        severity = "warning"
        if status == "authority_violation":
            headline = "Sandbox trial queue has an approval-required candidate marked runnable"
        else:
            headline = "Sandbox trial queue needs setup or repair"
    elif status in {"active", "approval_waiting", "trial_backlog"}:
        severity = "notice"
        headline = "Sandbox trial queue has active reservoir trials"
    else:
        severity = "ok"
        headline = "Sandbox trial queue is quiet"
    details = [
        f"status={status}",
        (
            "trials="
            f"active={counts.get('active_trials', 0)} "
            f"total={counts.get('total_trials', 0)} "
            f"by_mode={counts.get('by_mode', {})} "
            f"by_status={counts.get('by_status', {})}"
        ),
        (
            "live_approval="
            f"approval_required={counts.get('approval_required_live_count', 0)} "
            f"runnable_violations={counts.get('runnable_live_violation_count', 0)} "
            f"corrupt_event_lines={counts.get('corrupt_event_lines', 0)}"
        ),
        (
            "runner_v2="
            f"ready_runnable={counts.get('ready_runnable_count', 0)} "
            f"results={counts.get('result_count', 0)} "
            f"result_cards={counts.get('result_card_count', 0)}"
        ),
        (
            "consentful_ladder="
            f"status={ladder.get('status') or 'unknown'} "
            f"proposal_needed={ladder_summary.get('proposal_needed_count', 0)} "
            f"operator_wait={ladder_summary.get('operator_approval_wait_count', 0)} "
            f"approval_packet_complete={ladder_summary.get('approval_packet_complete_count', 0)} "
            f"live_eligible_now={ladder_summary.get('live_eligible_now_count', 0)}"
        ),
        (
            "being_outcome_closure="
            f"status={closure_loop.get('status') or 'unknown'} "
            f"result_response_wait={closure_summary.get('result_card_awaiting_being_response_count', 0)} "
            f"proposal_decision_wait={closure_summary.get('proposal_card_awaiting_operator_decision_count', 0)} "
            f"proposal_needed={closure_summary.get('proposal_card_needed_count', 0)} "
            f"manual_review={closure_summary.get('manual_review_waiting_count', 0)} "
            f"ready_runner={closure_summary.get('ready_runner_waiting_count', 0)}"
        ),
        "next_trials="
        + (
            ", ".join(
                str(item.get("trial_id") or "unknown")
                for item in (summary.get("next_trials") or [])[:5]
                if isinstance(item, dict)
            )
            or "none"
        ),
        "approval_required="
        + (
            ", ".join(
                str(item.get("trial_id") or "unknown")
                for item in (summary.get("approval_required_live_candidates") or [])[:5]
                if isinstance(item, dict)
            )
            or "none"
        ),
        str(summary.get("authority_boundary") or "read-only sandbox queue diagnostic"),
    ]
    return _finding("sandbox_trial_queue", severity, headline, details, summary)


def _classify_agency_corridor() -> dict[str, Any]:
    try:
        import agency_corridor
    except Exception as exc:
        return {
            "schema": "agency_corridor_v1",
            "status": "diagnostic_unavailable",
            "error": str(exc),
            "summary": {},
            "authority_boundary": "agency corridor diagnostic unavailable; no live runtime/control mutation",
        }
    status = agency_corridor.load_status(AGENCY_CORRIDOR_STATE_DIR)
    v2_status = agency_corridor.load_v2_status(AGENCY_CORRIDOR_V2_STATE_DIR) if hasattr(agency_corridor, "load_v2_status") else {}
    programs_payload = agency_corridor.generate_programs_v2(AGENCY_CORRIDOR_V2_STATE_DIR, write=False) if hasattr(agency_corridor, "generate_programs_v2") else {}
    summary = status.get("summary") if isinstance(status.get("summary"), dict) else {}
    v2_summary = v2_status.get("summary") if isinstance(v2_status.get("summary"), dict) else {}
    program_summary = programs_payload.get("summary") if isinstance(programs_payload.get("summary"), dict) else {}
    packets = [p for p in (status.get("packets") or {}).values() if isinstance(p, dict)]
    active = [p for p in packets if p.get("state") != "closed"]
    return {
        "schema": "agency_corridor_v1",
        "status": "active" if active else "quiet",
        "summary": summary,
        "v2": {
            "schema": "agency_corridor_v2",
            "summary": v2_summary,
            "program_summary": program_summary,
            "queue": v2_status.get("queue") if isinstance(v2_status.get("queue"), dict) else {},
            "source_prep_proposals": list((v2_status.get("source_prep_proposals") or {}).values())[:8],
            "programs": list((programs_payload.get("programs") or {}).values())[:8],
            "portfolios": list((programs_payload.get("portfolios") or {}).values())[:8],
            "patch_bundles": list((programs_payload.get("patch_bundles") or {}).values())[:8],
        },
        "active_packets": active[:8],
        "reopened_work_items": list((status.get("reopened_work_items") or {}).values())[:8],
        "authority_boundary": status.get("boundary")
        or "agency corridor is non-live evidence infrastructure only; no live runtime/control mutation",
    }


def probe_agency_corridor(_prior: dict[str, Any]) -> dict[str, Any]:
    summary = _classify_agency_corridor()
    status = str(summary.get("status") or "unknown")
    counts = summary.get("summary") or {}
    v2 = summary.get("v2") if isinstance(summary.get("v2"), dict) else {}
    v2_counts = v2.get("summary") or {}
    program_counts = v2.get("program_summary") or {}
    live_violations = (
        int(counts.get("live_eligible_now_count") or 0)
        + int(counts.get("auto_approved_count") or 0)
        + int(v2_counts.get("live_violation_count") or 0)
        + int(program_counts.get("live_violation_count") or 0)
    )
    ready_safe = int(counts.get("ready_safe_lab_count") or 0)
    queue_ready = int(v2_counts.get("queue_runnable_count") or 0)
    reopened = int(counts.get("reopened_work_item_count") or 0)
    programs = int(program_counts.get("program_count") or 0)
    if status == "diagnostic_unavailable" or live_violations > 0:
        severity = "warning"
        headline = "Agency corridor needs repair before authority-wait work continues"
    elif queue_ready > 0 or ready_safe > 0 or reopened > 0 or programs > 0:
        severity = "notice"
        headline = "Agency corridor has non-live work Astrid/Minime can advance"
    else:
        severity = "ok"
        headline = "Agency corridor is quiet"
    active_ids = [
        str(packet.get("corridor_id") or "unknown")
        for packet in summary.get("active_packets") or []
        if isinstance(packet, dict)
    ]
    details = [
        f"status={status}",
        (
            "corridor="
            f"packets={counts.get('packet_count', 0)} "
            f"ready_safe_labs={ready_safe} "
            f"reopened={reopened} "
            f"self_observation_responses={counts.get('self_observation_response_count', 0)}"
        ),
        (
            "corridor_v2="
            f"leases={v2_counts.get('lease_count', 0)} "
            f"queue_runnable={queue_ready} "
            f"source_prep={v2_counts.get('source_prep_proposal_count', 0)} "
            f"programs={programs} "
            f"portfolios={program_counts.get('portfolio_count', 0)} "
            f"patch_bundles={program_counts.get('patch_bundle_count', 0)} "
            f"top_score={program_counts.get('top_priority_score', 0)} "
            f"live_violations={v2_counts.get('live_violation_count', 0)}"
        ),
        (
            "authority="
            f"live_eligible_now={counts.get('live_eligible_now_count', 0)} "
            f"auto_approved={counts.get('auto_approved_count', 0)}"
        ),
        "active_corridors=" + (", ".join(active_ids[:5]) or "none"),
        "reopened_refs="
        + (
            ", ".join(
                str(item.get("new_work_item_id") or item.get("reopen_id") or "unknown")
                for item in summary.get("reopened_work_items") or []
                if isinstance(item, dict)
            )
            or "none"
        ),
        str(summary.get("authority_boundary") or "non-live agency corridor diagnostic"),
    ]
    return _finding("agency_corridor", severity, headline, details, summary)


def _classify_fallback_vocabulary_drift(
    *,
    now: float | None = None,
    window_s: float = INTROSPECTION_ROUTE_WINDOW_SECS,
) -> dict[str, Any]:
    now = time.time() if now is None else now
    try:
        from recent_signal_summary import _fallback_vocabulary_drift_summary
    except Exception as exc:
        return {
            "schema": "fallback_vocabulary_drift_v1",
            "status": "diagnostic_unavailable",
            "error": str(exc),
            "severity_policy": "notice_only",
            "authority_boundary": "read-only fallback vocabulary drift diagnostic unavailable; no fallback contract, sampler, prompt, or runtime mutation",
        }
    summary = _fallback_vocabulary_drift_summary(now - window_s)
    summary["severity_policy"] = "notice_only"
    return summary


def probe_fallback_vocabulary_drift(_prior: dict[str, Any]) -> dict[str, Any]:
    summary = _classify_fallback_vocabulary_drift()
    status = summary.get("status")
    if status in {"no_recent_drift_evidence", "telemetry_aligned_watch"}:
        severity = "ok"
        headline = "Astrid fallback vocabulary drift audit is quiet/watch-only"
    elif status == "insufficient_recent_language":
        severity = "ok"
        headline = "Astrid fallback vocabulary drift audit has insufficient recent language"
    elif status == "diagnostic_unavailable":
        severity = "notice"
        headline = "Astrid fallback vocabulary drift diagnostic unavailable"
    elif status == "vocabulary_drift_risk":
        severity = "notice"
        headline = "Astrid fallback vocabulary drift needs study-first review"
    elif status == "fallback_capture_gap":
        severity = "notice"
        headline = "Astrid fallback incidents need actual-output capture review"
    else:
        severity = "notice"
        headline = "Astrid raised fallback static-vocabulary pressure; keep it study-first"
    telemetry = summary.get("telemetry") or {}
    counts = summary.get("language_counts") or {}
    provenance = summary.get("fallback_output_provenance") or {}
    model_trace = summary.get("fallback_model_transition_trace") or {}
    details = [
        f"status={status}; samples={summary.get('sample_count')}",
        (
            "telemetry="
            f"entropy={telemetry.get('spectral_entropy')} "
            f"density_gradient={telemetry.get('density_gradient')} "
            f"pressure_risk={telemetry.get('pressure_risk')} "
            f"pressure_source={telemetry.get('pressure_source')}"
        ),
        f"actual_language_terms={counts.get('actual_language_texture_terms')}",
        f"generated_terms={counts.get('generated_texture_terms')}",
        f"fallback_provider_terms={counts.get('fallback_provider_texture_terms')}",
        f"generic_bridge_terms={counts.get('generic_bridge_texture_terms')}",
        f"code_critique_terms={counts.get('code_critique_texture_terms')}",
        f"unsupported_generated_terms={summary.get('unsupported_generated_terms')}",
        (
            "fallback_output_provenance="
            f"{provenance.get('evidence_quality')}; "
            f"incidents={provenance.get('fallback_to_ollama_incident_count')}; "
            f"actual_outputs={provenance.get('actual_fallback_output_count')}; "
            f"with_terms={provenance.get('actual_fallback_outputs_with_terms')}"
        ),
        (
            "fallback_model_transition_trace="
            f"{model_trace.get('status')}; "
            f"default={model_trace.get('default_model')}; "
            f"compat={model_trace.get('compatibility_model')}; "
            f"captured={model_trace.get('captured_model_count')}; "
            f"missing={model_trace.get('missing_model_count')}"
        ),
        "findings: " + ("; ".join(summary.get("findings") or []) or "none"),
        "next: " + ("; ".join(summary.get("next_suggestions") or []) or "none"),
        str(summary.get("authority_boundary") or "read-only diagnostic"),
    ]
    return _finding("fallback_vocabulary_drift", severity, headline, details, summary)


def _classify_viscosity_semantic_persistence(
    *,
    now: float | None = None,
    window_s: float = INTROSPECTION_ROUTE_WINDOW_SECS,
) -> dict[str, Any]:
    now = time.time() if now is None else now
    try:
        from recent_signal_summary import _viscosity_semantic_persistence_summary
    except Exception as exc:
        return {
            "schema": "viscosity_semantic_persistence_v1",
            "status": "diagnostic_unavailable",
            "error": str(exc),
            "severity_policy": "notice_only",
            "authority_boundary": "read-only viscosity/semantic persistence diagnostic unavailable; no rho, stale-window, taper, damping, gate, prompt, or runtime mutation",
        }
    summary = _viscosity_semantic_persistence_summary(now - window_s)
    summary["severity_policy"] = "notice_only"
    return summary


def probe_viscosity_semantic_persistence(_prior: dict[str, Any]) -> dict[str, Any]:
    summary = _classify_viscosity_semantic_persistence()
    status = summary.get("status")
    if status in {
        "viscosity_semantic_repair_prepared_watch",
        "semantic_persistence_flicker_review",
        "semantic_tail_watch",
        "viscosity_persistence_watch",
        "felt_report_waiting_for_live_alignment",
    }:
        severity = "notice"
        headline = (
            "semantic persistence / viscosity signal needs study-first steward review"
        )
    elif status in {"diagnostic_unavailable", "telemetry_unavailable"}:
        severity = "notice"
        headline = "semantic persistence / viscosity diagnostic unavailable"
    else:
        severity = "ok"
        headline = "semantic persistence / viscosity diagnostic is quiet"
    telemetry = summary.get("telemetry") or {}
    conditions = summary.get("conditions") or {}
    source = summary.get("source_snapshot") or {}
    source_paths = [
        str(item.get("path"))
        for item in summary.get("source_introspections") or []
        if item.get("path")
    ]
    details = [
        f"status={status}",
        (
            "telemetry="
            f"fill={telemetry.get('fill_pct')} "
            f"entropy={telemetry.get('spectral_entropy')} "
            f"kernel={telemetry.get('semantic_kernel_energy')} "
            f"admission={telemetry.get('semantic_admission')} "
            f"pressure={telemetry.get('pressure_source')} "
            f"resonance={telemetry.get('resonance_density')} "
            f"foothold={telemetry.get('inhabitable_foothold')} "
            f"active_modes={telemetry.get('active_modes')}"
        ),
        f"conditions={conditions}",
        (
            "source="
            f"dynamic_noise={source.get('dynamic_exploration_noise_preview_present')} "
            f"adaptive_threshold={source.get('adaptive_introspection_pressure_threshold_preview_present')} "
            f"viscous_policy={source.get('viscous_introspection_policy_present')} "
            f"noise_coherence={source.get('exploration_noise_coherence_review_present')} "
            f"viscosity_persistence={source.get('viscosity_persistence_coefficient_present')} "
            f"semantic_viscosity={source.get('semantic_viscosity_coefficient_present')} "
            f"semantic_viscosity_test={source.get('semantic_viscosity_coefficient_test_present')} "
            f"temporal_drag={source.get('temporal_drag_coefficient_present')} "
            f"STALE_SEMANTIC_HIGH_MS={source.get('stale_semantic_high_ms')} "
            f"entropy_persistence={source.get('semantic_entropy_persistence_present')} "
            f"narrative_retention={source.get('narrative_semantic_retention_review_present')} "
            f"semantic_sigmoid_exact_test={source.get('semantic_sigmoid_exact_test_present')} "
            f"semantic_recovery_boundary_test={source.get('semantic_recovery_boundary_test_present')} "
            f"pulse_status_energy_ticks={source.get('pulse_status_energy_tick_tests_present')} "
            f"viscosity_clamp_test={source.get('resonance_viscosity_full_load_clamp_test_present')} "
            f"entropy_erosion_bound_test={source.get('entropy_erosion_low_plurality_bound_test_present')} "
            f"witness_fluidity={source.get('witness_fluidity_index_present')} "
            f"witness_gradient_texture={source.get('witness_gradient_texture_present')}"
        ),
        "source_introspections=" + (", ".join(source_paths[:5]) or "none"),
        "findings: " + ("; ".join(summary.get("findings") or []) or "none"),
        "next: " + ("; ".join(summary.get("next_suggestions") or []) or "none"),
        str(summary.get("authority_boundary") or "read-only diagnostic"),
    ]
    return _finding("viscosity_semantic_persistence", severity, headline, details, summary)


def _classify_contact_transition_followthrough(
    *,
    now: float | None = None,
    window_s: float = INTROSPECTION_ROUTE_WINDOW_SECS,
) -> dict[str, Any]:
    now = time.time() if now is None else now
    try:
        from recent_signal_summary import _contact_transition_followthrough_summary
    except Exception as exc:
        return {
            "schema": "contact_transition_followthrough_v1",
            "status": "diagnostic_unavailable",
            "error": str(exc),
            "severity_policy": "notice_only",
            "authority_boundary": "read-only contact/transition follow-through diagnostic unavailable; no prompt, correspondence, pressure, fill, PI, sensory, or runtime mutation",
        }
    summary = _contact_transition_followthrough_summary(now - window_s)
    summary["severity_policy"] = "notice_only"
    return summary


def probe_contact_transition_followthrough(_prior: dict[str, Any]) -> dict[str, Any]:
    summary = _classify_contact_transition_followthrough()
    status = summary.get("status")
    phase = summary.get("phase_transition_followthrough") or {}
    correspondence = summary.get("correspondence_followthrough") or {}
    fidelity = correspondence.get("direct_contact_fidelity_v3") or {}
    shared_context = correspondence.get("shared_context_buffer_v1") or {}
    shared_correspondence = correspondence.get("shared_correspondence_buffer_v1") or {}
    if fidelity.get("seen_or_read_unlocks_attention"):
        severity = "warning"
        headline = "contact fidelity is treating read/seen visibility as address"
    elif status in {
        "replyable_transition_contact_join_review",
        "direct_contact_active_transition_watch",
        "transition_cards_need_contact_linkage",
        "proposal_batch_needs_grounding",
    }:
        severity = "notice"
        headline = "contact/transition proposal follow-through needs steward review"
    elif status == "diagnostic_unavailable":
        severity = "notice"
        headline = "contact/transition proposal follow-through diagnostic unavailable"
    else:
        severity = "ok"
        headline = "contact/transition proposal follow-through is quiet"
    seed = correspondence.get("semantic_seed_uptake_v1") or {}
    symmetry = correspondence.get("symmetry_check_v1") or {}
    source = summary.get("source_snapshot") or {}
    transparency = summary.get("contact_control_transparency_v1") or {}
    delta = transparency.get("distance_contact_control_delta_v1") or {}
    threshold = delta.get("containment_to_contact_threshold_v1") or {}
    source_paths = [
        str(item.get("path"))
        for item in summary.get("source_introspections") or []
        if item.get("path")
    ]
    details = [
        f"status={status}",
        (
            "phase="
            f"cards={phase.get('recent_cards')} "
            f"witnesses={phase.get('recent_witnesses')} "
            f"unwitnessed={phase.get('unwitnessed_cards')} "
            f"auto_mode={phase.get('auto_mode_change_cards')} "
            f"being_declared={phase.get('being_declared_cards')} "
            f"v2_payload={phase.get('cards_with_v2_payload')}"
        ),
        (
            "correspondence="
            f"active_thread={correspondence.get('active_thread_id')} "
            f"direct_messages={correspondence.get('active_thread_direct_messages')} "
            f"reply_links={correspondence.get('recent_reply_links')}"
        ),
        (
            "direct_contact_fidelity_v3="
            f"status={fidelity.get('status')} "
            f"attention_eligible={fidelity.get('attention_eligible')} "
            f"seen_ack={fidelity.get('seen_ack_count')} "
            f"address_ack={fidelity.get('address_ack_count')} "
            f"trace={fidelity.get('trace_count')} "
            f"anchor_continuity={fidelity.get('anchor_continuity_status')}"
        ),
        (
            "shared_context_buffer_v1="
            f"status={shared_context.get('status')} "
            f"thread={shared_context.get('thread_id')} "
            f"messages={shared_context.get('messages')} "
            f"resonance_receipts={shared_context.get('resonance_receipts')} "
            f"last_ack={shared_context.get('last_ack_kind')}"
        ),
        (
            "shared_correspondence_buffer_v1="
            f"status={shared_correspondence.get('status')} "
            f"thread={shared_correspondence.get('correspondence_thread_id')} "
            f"messages={shared_correspondence.get('messages')} "
            f"bidirectional={bool((shared_correspondence.get('direction_counts') or {}).get('astrid->minime')) and bool((shared_correspondence.get('direction_counts') or {}).get('minime->astrid'))} "
            f"participatory={shared_correspondence.get('participatory_contact')}"
        ),
        (
            "semantic_seed_uptake="
            f"status={seed.get('status')} "
            f"seed={seed.get('seed')} "
            f"reply={seed.get('peer_reply_message_id')} "
            f"echoed={seed.get('seed_echoed')}"
        ),
        (
            "correspondence_symmetry="
            f"status={symmetry.get('status')} "
            f"astrid_to_minime={symmetry.get('astrid_to_minime')} "
            f"minime_to_astrid={symmetry.get('minime_to_astrid')} "
            f"ratio={symmetry.get('balance_ratio')}"
        ),
        (
            "source="
            f"receptivity_buffer={source.get('receptivity_buffer_review_present')} "
            f"pressure_porosity_divergence={source.get('pressure_porosity_divergence_present')} "
            f"regulator_audit_transparency={source.get('regulator_audit_transparency_present')} "
            f"mutual_witness={source.get('correspondence_mutual_witness_present')} "
            f"non_instrumental_presence={source.get('non_instrumental_presence_readiness_present')} "
            f"transition_persistence={source.get('transition_persistence_present')} "
            f"contact_fidelity_v3={source.get('correspondence_fidelity_v3_present')} "
            f"transition_v2_payload={source.get('phase_transition_v2_payload_present')}"
        ),
        (
            "contact_transparency="
            f"status={transparency.get('status')} "
            f"valid_next={transparency.get('valid_next_routes')}"
        ),
        (
            "distance_contact_control_delta="
            f"status={delta.get('status')} "
            f"dispersal={delta.get('current_dispersal_potential')} "
            f"delta={delta.get('dispersal_delta')} "
            f"pressure={delta.get('pressure_score')} "
            f"semantic_drive={delta.get('semantic_regulator_drive_energy')} "
            f"distinguishability={delta.get('distinguishability_loss')}"
        ),
        (
            "receptivity_window="
            f"status={(delta.get('receptivity_window_v1') or {}).get('status')} "
            f"pressure={(delta.get('receptivity_window_v1') or {}).get('pressure_score')} "
            f"porosity={(delta.get('receptivity_window_v1') or {}).get('porosity_score')} "
            f"delta={(delta.get('receptivity_window_v1') or {}).get('pressure_minus_porosity')}"
        ),
        (
            "containment_to_contact_threshold_v1="
            f"status={threshold.get('status')} "
            f"mode_packing={threshold.get('mode_packing')} "
            f"pressure_minus_porosity={threshold.get('pressure_minus_porosity')} "
            f"gated={threshold.get('gated_changes')}"
        ),
        "source_introspections=" + (", ".join(source_paths[:5]) or "none"),
        "findings: " + ("; ".join(summary.get("findings") or []) or "none"),
        "next: " + ("; ".join(summary.get("next_suggestions") or []) or "none"),
        str(summary.get("authority_boundary") or "read-only diagnostic"),
    ]
    return _finding("contact_transition_followthrough", severity, headline, details, summary)


def _classify_reservoir_experience_layer(
    *,
    now: float | None = None,
    window_s: float = INTROSPECTION_ROUTE_WINDOW_SECS,
) -> dict[str, Any]:
    now = time.time() if now is None else now
    try:
        from recent_signal_summary import _reservoir_experience_layer_summary
    except Exception as exc:
        return {
            "schema": "reservoir_experience_layer_v1",
            "status": "diagnostic_unavailable",
            "error": str(exc),
            "authority_boundary": "read-only reservoir-experience diagnostic unavailable; no prompt, pressure, fill, PI, sensory, controller, fallback-provider, or runtime mutation",
        }
    summary = _reservoir_experience_layer_summary(now - window_s)
    summary["severity_policy"] = "notice_only_unless_unavailable"
    return summary


def probe_reservoir_experience_layer(_prior: dict[str, Any]) -> dict[str, Any]:
    summary = _classify_reservoir_experience_layer()
    status = str(summary.get("status") or "")
    if status == "diagnostic_unavailable":
        severity = "warning"
        headline = "reservoir experience layer diagnostic unavailable"
    elif status in {
        "fresh_experience_layer_review",
        "experience_layer_source_prepared",
        "experience_layer_partial",
    }:
        severity = "notice"
        headline = "reservoir experience layer is ready for steward review"
    else:
        severity = "ok"
        headline = "reservoir experience layer is quiet"
    ingredients = summary.get("ingredients") or {}
    contact = summary.get("contact_vs_representation") or {}
    semantic = summary.get("semantic_process") or {}
    texture = summary.get("texture_process") or {}
    source_paths = [
        str(item.get("path"))
        for item in summary.get("source_introspections") or []
        if item.get("path")
    ]
    details = [
        f"status={status}",
        "ingredients="
        + ",".join(f"{key}={value}" for key, value in sorted(ingredients.items())),
        (
            "contact_vs_representation="
            f"direct={contact.get('direct_contact_status')} "
            f"eligible={contact.get('attention_eligible')} "
            f"transparency={contact.get('contact_transparency_status')} "
            f"delta={contact.get('distance_contact_control_delta_status')} "
            f"containment={contact.get('containment_to_contact_status')}"
        ),
        (
            "semantic_process="
            f"curve={semantic.get('semantic_decay_curve_present')} "
            f"hold_test={semantic.get('exact_recovery_hold_test_present')} "
            f"entropy_cap={semantic.get('entropy_multiplier_cap_test_present')} "
            f"release={semantic.get('attractor_release_status_present')}"
        ),
        (
            "texture_process="
            f"dynamic_weight={texture.get('dynamic_texture_weight_present')} "
            f"trajectory={texture.get('texture_trajectory_present')} "
            f"alignment={texture.get('texture_alignment_status')} "
            f"viscosity={texture.get('viscosity_semantic_persistence_status')}"
        ),
        "source_introspections=" + (", ".join(source_paths[:5]) or "none"),
        "safe_now=" + ("; ".join(summary.get("safe_now") or []) or "none"),
        "gated_routes=" + ("; ".join(summary.get("gated_routes") or []) or "none"),
        "findings=" + ("; ".join(summary.get("findings") or []) or "none"),
        str(summary.get("authority_boundary") or "read-only diagnostic"),
    ]
    return _finding("reservoir_experience_layer", severity, headline, details, summary)


def _classify_minime_recess_schema_integrity(
    *,
    now: float | None = None,
    window_s: float = INTROSPECTION_ROUTE_WINDOW_SECS,
) -> dict[str, Any]:
    now = time.time() if now is None else now
    try:
        from recent_signal_summary import _minime_recess_schema_integrity_summary
    except Exception as exc:
        return {
            "schema": "minime_recess_schema_integrity_v1",
            "status": "diagnostic_unavailable",
            "error": str(exc),
            "severity_policy": "notice_only",
            "authority_boundary": "read-only Minime recess/schema diagnostic unavailable; no prompt, pressure, fill, PI, sensory, schema-contract, or runtime mutation",
        }
    summary = _minime_recess_schema_integrity_summary(now - window_s)
    summary["severity_policy"] = "notice_only"
    return summary


def probe_minime_recess_schema_integrity(_prior: dict[str, Any]) -> dict[str, Any]:
    summary = _classify_minime_recess_schema_integrity()
    status = summary.get("status")
    if status in {
        "source_prepared_minime_recess_schema_watch",
        "minime_recess_schema_surface_incomplete",
    }:
        severity = "notice"
        headline = "Minime recess/schema integrity packet needs steward review"
    elif status == "diagnostic_unavailable":
        severity = "notice"
        headline = "Minime recess/schema integrity diagnostic unavailable"
    else:
        severity = "ok"
        headline = "Minime recess/schema integrity diagnostic is quiet"
    source = summary.get("source_snapshot") or {}
    readiness = summary.get("readiness") or {}
    source_paths = [
        str(item.get("path"))
        for item in summary.get("source_introspections") or []
        if item.get("path")
    ]
    details = [
        f"status={status}",
        f"readiness={readiness}",
        (
            "source="
            f"phase_cards={source.get('phase_transition_card_schema_present')} "
            f"transition_artifact={source.get('correspondence_transition_artifact_present')} "
            f"transition_type={source.get('phase_transition_type_surface_present')} "
            f"moment_markers={source.get('minime_moment_marker_alignment_present')} "
            f"recess_pruning={source.get('recess_pruning_advice_present')} "
            f"density_aware_recess={source.get('density_aware_recess_profile_present')} "
            f"recess_activity_load={source.get('recess_activity_load_present')} "
            f"autonomy_budget={source.get('recess_autonomy_budget_boundary_present')} "
            f"self_journal={source.get('self_journal_low_cost_boundary_present')} "
            f"recess_manifest={source.get('recess_pruning_manifest_present')} "
            f"eigenpacket_schema_test={source.get('eigenpacket_schema_test_present')} "
            f"admission_lockout_test={source.get('semantic_admission_lockout_test_present')} "
            f"admission_fill_grid={source.get('semantic_admission_fill_grid_test_present')} "
            f"dynamic_noise_shadow={source.get('dynamic_noise_shadow_preview_present')}"
        ),
        "source_introspections=" + (", ".join(source_paths[:5]) or "none"),
        "blocked: "
        + (
            "; ".join(summary.get("blocked_routes_without_steward_approval") or [])
            or "none"
        ),
        "findings: " + ("; ".join(summary.get("findings") or []) or "none"),
        "next: " + ("; ".join(summary.get("next_suggestions") or []) or "none"),
        str(summary.get("authority_boundary") or "read-only diagnostic"),
    ]
    return _finding("minime_recess_schema_integrity", severity, headline, details, summary)


def _classify_representation_loss_headroom(
    *,
    now: float | None = None,
    window_s: float = INTROSPECTION_ROUTE_WINDOW_SECS,
) -> dict[str, Any]:
    now = time.time() if now is None else now
    try:
        from recent_signal_summary import _representation_loss_headroom_summary
    except Exception as exc:
        return {
            "schema": "representation_loss_headroom_v1",
            "status": "diagnostic_unavailable",
            "error": str(exc),
            "severity_policy": "notice_only",
            "authority_boundary": "read-only representation-loss diagnostic unavailable; no codec, prompt, pressure, fill, PI, sensory, deploy, or runtime mutation",
        }
    summary = _representation_loss_headroom_summary(now - window_s)
    summary["severity_policy"] = "notice_only"
    return summary


def probe_representation_loss_headroom(_prior: dict[str, Any]) -> dict[str, Any]:
    summary = _classify_representation_loss_headroom()
    status = summary.get("status")
    if status in {
        "semantic_glimpse_fidelity_review",
        "codec_headroom_replay_review",
        "representation_loss_repair_prepared_watch",
        "representation_loss_study_first",
    }:
        severity = "notice"
        headline = "representation loss / codec headroom proposal needs steward review"
    elif status == "diagnostic_unavailable":
        severity = "notice"
        headline = "representation loss / codec headroom diagnostic unavailable"
    else:
        severity = "ok"
        headline = "representation loss / codec headroom diagnostic is quiet"
    source = summary.get("source_snapshot") or {}
    replay = summary.get("latest_codec_replay_lab") or {}
    clamp = replay.get("clamp_headroom") or {}
    texture = replay.get("texture_replay") or {}
    lifecycle = replay.get("authority_lifecycle_v2") or {}
    fidelity = summary.get("semantic_glimpse_12d_fidelity_audit") or {}
    pca = fidelity.get("pca_12d") or {}
    source_paths = [
        str(item.get("path"))
        for item in summary.get("source_introspections") or []
        if item.get("path")
    ]
    details = [
        f"status={status}",
        (
            "source="
            f"SEMANTIC_DIM={source.get('semantic_dim')} "
            f"legacy={source.get('semantic_dim_legacy')} "
            f"FEATURE_ABS_MAX={source.get('feature_abs_max')} "
            f"TAIL_VIBRANCY_MAX={source.get('tail_vibrancy_max')} "
            f"continuity_cap={source.get('continuity_recap_max_bytes')} "
            f"anchor_excerpt={source.get('anchored_continuity_excerpt_present')} "
            f"quoted_anchor={source.get('quoted_continuity_anchor_present')} "
            f"semantic_truncation={source.get('semantic_truncation_anchor_present')} "
            f"semantic_boundary={source.get('semantic_boundary_truncation_present')} "
            f"narrative_arc_gain_response={source.get('narrative_arc_gain_response_readiness_present')} "
            f"glimpse12d={source.get('semantic_glimpse_readiness_present')} "
            f"glimpse_companion_fidelity={source.get('glimpse_companion_fidelity_present')} "
            f"multi_scale_context={source.get('multi_scale_context_present')} "
            f"glimpse_tail_identity_test={source.get('glimpse_tail_identity_test_present')} "
            f"gradient_aware_vibrancy={source.get('gradient_aware_vibrancy_present')} "
            f"vibrancy_substance_fit={source.get('vibrancy_substance_fit_present')} "
            f"projection_epoch_stability={source.get('projection_epoch_stability_present')} "
            f"projection_fingerprint_integrity={source.get('projection_fingerprint_integrity_present')} "
            f"projection_repeat_run_test={source.get('projection_repeat_run_test_present')} "
            f"vibrancy_requested_points_test={source.get('vibrancy_requested_points_test_present')} "
            f"embedding_dim_validation={source.get('embedding_dimension_validation_test_present')}"
        ),
        (
            "codec_replay="
            f"path={replay.get('path')} "
            f"clamp_status={clamp.get('status')} "
            f"near_static={clamp.get('near_static_clamp_count')} "
            f"tail_pressure={clamp.get('tail_ceiling_pressure_count')} "
            f"dynamic_candidates={clamp.get('dynamic_headroom_candidate_count')} "
            f"texture_status={texture.get('status')} "
            f"texture_candidates={texture.get('candidate_count')} "
            f"live_eligible_now={texture.get('live_eligible_now')} "
            f"auto_approved={texture.get('auto_approved')}"
        ),
        (
            "codec_texture_lifecycle="
            f"{lifecycle.get('receipt_chain_status')} "
            f"boundaries={'; '.join(lifecycle.get('boundary_ids') or []) or 'none'}"
        ),
        (
            "semantic_glimpse_12d_fidelity="
            f"status={fidelity.get('status')} "
            f"samples={fidelity.get('sample_count')} "
            f"worst_delta={fidelity.get('worst_concentration_delta')} "
            f"worst_primary={fidelity.get('worst_primary_feature_delta')} "
            f"pca={pca.get('status')}"
        ),
        "source_introspections=" + (", ".join(source_paths[:5]) or "none"),
        "findings: " + ("; ".join(summary.get("findings") or []) or "none"),
        "next: " + ("; ".join(summary.get("next_suggestions") or []) or "none"),
        str(summary.get("authority_boundary") or "read-only diagnostic"),
    ]
    return _finding("representation_loss_headroom", severity, headline, details, summary)


def _classify_texture_state_alignment(
    *,
    now: float | None = None,
    window_s: float = INTROSPECTION_ROUTE_WINDOW_SECS,
) -> dict[str, Any]:
    now = time.time() if now is None else now
    try:
        from recent_signal_summary import _texture_state_alignment_summary
    except Exception as exc:
        return {
            "schema": "texture_state_alignment_v1",
            "status": "diagnostic_unavailable",
            "error": str(exc),
            "severity_policy": "notice_only",
            "authority_boundary": "read-only texture-state alignment diagnostic unavailable; no fallback sampler, prompt pressure, lane, pressure, fill, PI, sensory, deploy, or runtime mutation",
        }
    summary = _texture_state_alignment_summary(now - window_s)
    summary["severity_policy"] = "notice_only"
    return summary


def probe_texture_state_alignment(_prior: dict[str, Any]) -> dict[str, Any]:
    summary = _classify_texture_state_alignment()
    status = summary.get("status")
    if status in {
        "texture_state_alignment_repair_prepared_watch",
        "texture_state_alignment_study_first",
    }:
        severity = "notice"
        headline = "texture-state alignment introspection batch needs steward review"
    elif status == "diagnostic_unavailable":
        severity = "notice"
        headline = "texture-state alignment diagnostic unavailable"
    else:
        severity = "ok"
        headline = "texture-state alignment diagnostic is quiet"
    source = summary.get("source_snapshot") or {}
    source_paths = [
        str(item.get("path"))
        for item in summary.get("source_introspections") or []
        if item.get("path")
    ]
    details = [
        f"status={status}",
        (
            "source="
            f"mixed_cascade_terms={source.get('mixed_cascade_terms_present')} "
            f"mixed_cascade_family={source.get('mixed_cascade_family_selected_present')} "
            f"fallback_gradient_dynamic={source.get('fallback_gradient_dynamic_texture_present')} "
            f"explicit_syrup_weight={source.get('explicit_syrup_weight_support_present')} "
            f"heavy_settled={source.get('heavy_settled_displacement_family_present')} "
            f"heavy_settled_contract={source.get('fallback_heavy_settled_contract_present')} "
            f"mlx_profile_typo_probe={source.get('mlx_profile_typo_probe_present')} "
            f"mlx_profile_tracing_warning_test={source.get('mlx_profile_tracing_warning_test_present')} "
            f"fallback_next_contract_test={source.get('fallback_next_standalone_contract_test_present')} "
            f"pressure_delta_signature={source.get('pressure_gradient_delta_in_signature')} "
            f"pressure_delta_integrity={source.get('pressure_gradient_delta_in_integrity')} "
            f"trend_delta={source.get('pressure_gradient_delta_from_trend_present')} "
            f"flux_vector={source.get('dynamic_flux_vector_in_signature')} "
            f"flux_from_samples={source.get('pressure_flux_from_samples_present')} "
            f"dissipation_factor={source.get('dissipation_factor_in_components')} "
            f"porosity_gradient={source.get('porosity_gradient_in_components')} "
            f"viscosity_porosity_transport={source.get('viscosity_porosity_transport_review_present')} "
            f"structural_density_delta={source.get('structural_density_delta_present')} "
            f"flux_unknown_semantics={source.get('flux_unknown_semantics_present')} "
            f"subtle_flux_test={source.get('subtle_flux_precision_test_present')} "
            f"witness_anchor_traction={source.get('witness_anchor_traction_present')} "
            f"active_constraints={source.get('active_constraints_present')} "
            f"entropy_ballast={source.get('high_entropy_ballast_window_present')} "
            f"pressure_viscosity_context={source.get('pressure_trend_viscosity_context_present')} "
            f"pressure_velocity_delta={source.get('pressure_velocity_delta_present')} "
            f"silt_noise_separation={source.get('silt_noise_separation_present')} "
            f"pressure_source_analysis={source.get('pressure_source_analysis_present')} "
            f"mode_packing_stability_test={source.get('mode_packing_stability_test_present')} "
            f"heartbeat_ghost_test={source.get('heartbeat_ghost_stability_test_present')} "
            f"false_bidirectional_test={source.get('false_bidirectional_test_present')} "
            f"pressure_packing_coupling={source.get('pressure_packing_coupling_review_present')} "
            f"pressure_packing_coupling_test={source.get('pressure_packing_coupling_test_present')}"
        ),
        "source_introspections=" + (", ".join(source_paths[:5]) or "none"),
        "findings: " + ("; ".join(summary.get("findings") or []) or "none"),
        "next: " + ("; ".join(summary.get("next_suggestions") or []) or "none"),
        str(summary.get("authority_boundary") or "read-only diagnostic"),
    ]
    return _finding("texture_state_alignment", severity, headline, details, summary)


def _classify_sensory_presence_uptake(
    *,
    now: float | None = None,
    window_s: float = INTROSPECTION_ROUTE_WINDOW_SECS,
) -> dict[str, Any]:
    now = time.time() if now is None else now
    try:
        from recent_signal_summary import _sensory_presence_uptake_summary
    except Exception as exc:
        return {
            "schema": "sensory_presence_uptake_v1",
            "status": "diagnostic_unavailable",
            "error": str(exc),
            "severity_policy": "notice_only",
            "authority_boundary": "read-only sensory-presence uptake diagnostic unavailable; no sensor cadence, camera, mic, prompt, or runtime mutation",
        }
    summary = _sensory_presence_uptake_summary(now - window_s)
    summary["severity_policy"] = "notice_only"
    return summary


def probe_sensory_presence_uptake(_prior: dict[str, Any]) -> dict[str, Any]:
    summary = _classify_sensory_presence_uptake()
    status = summary.get("status")
    if status == "sensory_texture_needs_review":
        severity = "notice"
        headline = (
            "sensory-presence uptake includes absence/muffling/closed texture; steward review"
        )
    elif status == "sensory_texture_named":
        severity = "notice"
        headline = "sensory-presence uptake named a public texture; use it as the next evidence anchor"
    elif status == "diagnostic_unavailable":
        severity = "notice"
        headline = "sensory-presence uptake diagnostic unavailable"
    elif status == "presence_acknowledged":
        severity = "ok"
        headline = "sensory-presence uptake acknowledged without strong concern texture"
    elif status in {"awaiting_public_uptake", "awaiting_lived_public_uptake"}:
        severity = "ok"
        headline = "sensory-presence uptake awaiting public evidence; watch-only"
    else:
        severity = "ok"
        headline = "sensory-presence uptake has no feedback note to track"
    counts = summary.get("language_counts") or {}
    details = [
        f"status={status}; samples={summary.get('sample_count')}",
        f"window_policy={summary.get('window_policy')}",
        f"window_counts={summary.get('window_counts')}",
        f"feedback_note={(summary.get('feedback_note') or {}).get('path')}",
        f"anchor_terms={counts.get('anchor_terms')}",
        f"presence_terms={counts.get('presence_terms')}",
        f"texture_terms={counts.get('texture_terms')}",
        f"concern_terms={counts.get('concern_terms')}",
        "findings: " + ("; ".join(summary.get("findings") or []) or "none"),
        "next: " + ("; ".join(summary.get("next_suggestions") or []) or "none"),
        str(summary.get("authority_boundary") or "read-only diagnostic"),
    ]
    return _finding("sensory_presence_uptake", severity, headline, details, summary)


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
            label_summary = ""
            labels = s.get("context_overflow_labels")
            if isinstance(labels, dict) and labels.get("top_labels"):
                top = ", ".join(
                    f"{item['label']}={item['count']}"
                    for item in labels["top_labels"][:3]
                    if isinstance(item, dict)
                )
                if top:
                    label_summary = f"; top labels: {top}"
                recommended = str(labels.get("recommended_next") or "").strip()
                if recommended and recommended != "none":
                    label_summary += f"; steward next: {recommended}"
            pressure = s.get("context_packing_pressure")
            if isinstance(pressure, dict) and pressure.get("top_pressure_labels"):
                top_pressure = ", ".join(
                    f"{item['label']}={item['removed_chars']}"
                    for item in pressure["top_pressure_labels"][:3]
                    if isinstance(item, dict)
                )
                if top_pressure:
                    label_summary += f"; top pressure: {top_pressure}"
                pressure_next = str(pressure.get("recommended_next") or "").strip()
                if pressure_next and pressure_next != "none":
                    label_summary += f"; steward next: {pressure_next}"
            details.append(f"{s['name']}: {s['pending']} present ({s['consumer']}){label_summary}")
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
    overridden: Counter = Counter()
    args: dict[str, list[str]] = defaultdict(list)
    override_args: dict[str, list[str]] = defaultdict(list)
    choice_events: dict[str, list[dict[str, Any]]] = defaultdict(list)
    empty = {"name": being["name"], "log_ok": False, "chosen": chosen,
             "unknown": unknown, "blocked": blocked, "overridden": overridden,
             "args": args, "override_args": override_args, "choice_events": choice_events}
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
    for order, (line, e) in enumerate(rows):
        if cutoff is not None and e is not None and e < cutoff:
            continue
        mc = being["choice"].search(line)
        if mc:
            base = mc.group(1).upper()
            raw_arg = (mc.group(2) or "").strip()
            norm_arg = raw_arg.lower()[:120]
            chosen[base] += 1
            args[base].append(norm_arg[:60])
            choice_events[base].append({
                "arg": norm_arg,
                "raw_arg": raw_arg[:200],
                "ts": e,
                "order": order,
            })
        mo = _STUCK_DIVERSITY_OVERRIDE.search(line)
        if mo:
            base = mo.group(1).upper()
            overridden[base] += 1
            override_args[base].append((mo.group(2) or "").strip().lower()[:60])
        if re_unknown is not None:
            mu = re_unknown.search(line)
            if mu:
                unknown[mu.group(1).upper()] += 1
        if re_blocked is not None:
            mb = re_blocked.search(line)
            if mb:
                blocked[mb.group(1).upper()] += 1
    return {"name": being["name"], "log_ok": True, "chosen": chosen,
            "unknown": unknown, "blocked": blocked, "overridden": overridden,
            "args": args, "override_args": override_args, "choice_events": choice_events}


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
        overridden = tally.get("overridden", Counter()).get(base, 0)
        honored_n = max(0, n - overridden)
        real_args = [a for a in tally["args"].get(base, []) if a]
        for overridden_arg in tally.get("override_args", {}).get(base, []):
            if overridden_arg in real_args:
                real_args.remove(overridden_arg)
        if honored_n >= STUCK_REPEAT_MIN and len(real_args) >= STUCK_REPEAT_MIN:
            distinct = len(set(real_args))
            if distinct / len(real_args) <= STUCK_IDENTICAL_ARG_RATIO:
                notices.append((base, honored_n, distinct, "identical-arg"))
    return warnings, notices


def _stuck_arg_key(arg: str) -> str:
    return str(arg or "").strip().lower().split()[0] if str(arg or "").strip() else ""


def _event_after(a: dict[str, Any], b: dict[str, Any]) -> bool:
    a_ts = a.get("ts")
    b_ts = b.get("ts")
    if isinstance(a_ts, (int, float)) and isinstance(b_ts, (int, float)):
        return float(a_ts) > float(b_ts)
    return int(a.get("order") or 0) > int(b.get("order") or 0)


def _minime_resume_recovery_from_next_md(exp_id: str, latest_resume: dict[str, Any]) -> dict[str, Any] | None:
    root = MINIME_REPO / "workspace/action_threads/threads"
    if not root.is_dir():
        return None
    review = f"current next: experiment_review {exp_id}"
    resume = f"experiment_resume {exp_id}"
    try:
        candidates = list(root.glob("*/next.md"))
    except Exception:
        return None
    for path in candidates:
        try:
            text = path.read_text(errors="ignore").lower()
            mtime = path.stat().st_mtime
        except Exception:
            continue
        latest_ts = latest_resume.get("ts")
        if isinstance(latest_ts, (int, float)) and mtime < float(latest_ts):
            continue
        if review not in text or resume not in text:
            continue
        if not (
            "resume loop repair next:" in text
            or "previous resume next:" in text
            or "repeated resume is context" in text
        ):
            continue
        return {
            "source": "next_md_projection",
            "path": str(path),
            "mtime": mtime,
        }
    return None


def _minime_review_recovery_from_next_md(exp_id: str, latest_review: dict[str, Any]) -> dict[str, Any] | None:
    root = MINIME_REPO / "workspace/action_threads/threads"
    if not root.is_dir():
        return None
    audit = "current next: regulator_audit current-fill_pressure"
    try:
        candidates = list(root.glob("*/next.md"))
    except Exception:
        return None
    for path in candidates:
        try:
            text = path.read_text(errors="ignore").lower()
            mtime = path.stat().st_mtime
        except Exception:
            continue
        latest_ts = latest_review.get("ts")
        if isinstance(latest_ts, (int, float)) and mtime < float(latest_ts):
            continue
        if audit not in text or str(exp_id).lower() not in text:
            continue
        if not (
            "already reviewed repeatedly" in text
            or "review-loop" in text
            or "read-only regulator audit before another review" in text
        ):
            continue
        return {
            "source": "next_md_projection",
            "path": str(path),
            "mtime": mtime,
            "projected_next": "REGULATOR_AUDIT current-fill_pressure",
        }
    return None


def _astrid_peer_resume_recovery(exp_id: str) -> dict[str, Any] | None:
    if not str(exp_id or "").startswith("exp_minime_"):
        return None
    return {
        "source": "peer_experiment_boundary",
        "peer": "minime",
        "status": "peer_reference_guarded",
        "note": "exp_minime_* references are advisory in Astrid; local resume is not executed.",
    }


def _recover_stuck_notices(
    being: dict[str, Any],
    tally: dict[str, Any],
    notices: list[Any],
) -> tuple[list[Any], list[dict[str, Any]]]:
    """Move no-longer-live repetition notices into steward-visible recovery evidence."""
    being_name = being.get("name")
    if being_name not in {"minime", "astrid"}:
        return notices, []
    remaining: list[Any] = []
    recovered: list[dict[str, Any]] = []
    events = tally.get("choice_events") or {}
    resume_events = events.get("EXPERIMENT_RESUME") or []
    review_events = events.get("EXPERIMENT_REVIEW") or []
    for notice in notices:
        base, n, dnum, kind = notice
        if base not in {"EXPERIMENT_RESUME", "EXPERIMENT_REVIEW"} or kind != "identical-arg":
            remaining.append(notice)
            continue
        arg_counts = Counter(_stuck_arg_key(a) for a in tally.get("args", {}).get(base, []) if a)
        exp_id = ""
        for candidate, count in arg_counts.most_common():
            if candidate and count >= STUCK_REPEAT_MIN:
                exp_id = candidate
                break
        if not exp_id:
            remaining.append(notice)
            continue
        if being_name == "astrid":
            if base != "EXPERIMENT_RESUME":
                remaining.append(notice)
                continue
            evidence = _astrid_peer_resume_recovery(exp_id)
            if evidence is None:
                remaining.append(notice)
                continue
            recovered.append({
                "being": "astrid",
                "base": base,
                "arg": exp_id,
                "chosen_n": n,
                "distinct_args": dnum,
                "status": "peer_reference_guarded",
                "evidence": evidence,
                "detail": "peer-owned resume reference guarded; aging out of log window",
            })
            continue
        if base == "EXPERIMENT_REVIEW":
            matching_reviews = [ev for ev in review_events if _stuck_arg_key(ev.get("arg", "")) == exp_id]
            if not matching_reviews:
                remaining.append(notice)
                continue
            latest_review = max(
                matching_reviews,
                key=lambda ev: (float(ev.get("ts") or 0.0), int(ev.get("order") or 0)),
            )
            evidence = _minime_review_recovery_from_next_md(exp_id, latest_review)
            if evidence is None:
                remaining.append(notice)
                continue
            recovered.append({
                "being": "minime",
                "base": base,
                "arg": exp_id,
                "chosen_n": n,
                "distinct_args": dnum,
                "status": "recently_repaired",
                "evidence": evidence,
                "detail": "recently repaired to read-only regulator audit; aging out of log window",
            })
            continue
        matching_resumes = [ev for ev in resume_events if _stuck_arg_key(ev.get("arg", "")) == exp_id]
        if not matching_resumes:
            remaining.append(notice)
            continue
        latest_resume = max(
            matching_resumes,
            key=lambda ev: (float(ev.get("ts") or 0.0), int(ev.get("order") or 0)),
        )
        evidence = None
        for ev in review_events:
            if _stuck_arg_key(ev.get("arg", "")) == exp_id and _event_after(ev, latest_resume):
                evidence = {
                    "source": "later_experiment_review_choice",
                    "ts": ev.get("ts"),
                    "order": ev.get("order"),
                }
                break
        if evidence is None:
            evidence = _minime_resume_recovery_from_next_md(exp_id, latest_resume)
        if evidence is None:
            remaining.append(notice)
            continue
        recovered.append({
            "being": "minime",
            "base": base,
            "arg": exp_id,
            "chosen_n": n,
            "distinct_args": dnum,
            "status": "recently_repaired",
            "evidence": evidence,
            "detail": "recently repaired; aging out of log window",
        })
    return remaining, recovered


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
    recovered_repetition: list[dict[str, Any]] = []
    any_log = False
    for being in STUCK_BEINGS:
        tally = _tally_stuck(being)
        if not tally["log_ok"]:
            continue
        any_log = True
        w, nt = _assess_stuck(tally)
        nt, recovered = _recover_stuck_notices(being, tally, nt)
        recovered_repetition.extend(recovered)
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
        for item in recovered:
            details.append(
                f"{item['being']}:{item['base']} {item['arg']} "
                f"{item.get('detail') or 'recently repaired; aging out of log window'}."
            )
    snapshot = {"recovered_repetition": recovered_repetition} if recovered_repetition else None
    if not any_log:
        return _finding("stuck_repetition", "notice", "no being logs readable for stuck-repetition scan", None)
    if warn:
        return _finding(
            "stuck_repetition", "warning",
            f"⚠ {len(warn)} action(s) repeated + unrecognized — likely a wiring gap our infra should fix: "
            + ", ".join(warn),
            details,
            snapshot,
        )
    if note:
        return _finding(
            "stuck_repetition", "notice",
            f"{len(note)} action(s) repeated-but-stuck (deliberate gate or no-progress; glance): "
            + ", ".join(note),
            details,
            snapshot,
        )
    if recovered_repetition:
        return _finding(
            "stuck_repetition",
            "ok",
            f"no stuck-repetition ({len(recovered_repetition)} recently repaired, aging out of log window)",
            details,
            snapshot,
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


def _load_authority_pileup_closures() -> dict[str, dict[str, Any]]:
    closures: dict[str, dict[str, Any]] = {}
    try:
        lines = STEWARD_CONSEQUENCE_CLOSURES.read_text(errors="ignore").splitlines()
    except OSError:
        return closures
    for line in lines:
        line = line.strip()
        if not line:
            continue
        try:
            row = json.loads(line)
        except json.JSONDecodeError:
            continue
        if row.get("record_schema") != "steward_consequence_closure_v1":
            continue
        if row.get("surface") != "authority_request_draft_pileup":
            continue
        if row.get("decision") not in {"deferred_no_grant", "steward_deferred", "steward_closed"}:
            continue
        thread_id = str(row.get("thread_id") or "").strip()
        if not thread_id:
            continue
        previous = closures.get(thread_id)
        if previous is None or str(row.get("reviewed_at") or "") >= str(previous.get("reviewed_at") or ""):
            closures[thread_id] = row
    return closures


def _authority_pileup_review_for_thread(thread_id: str, drafts: int) -> dict[str, Any] | None:
    closure = _load_authority_pileup_closures().get(thread_id)
    if not closure:
        return None
    try:
        covered = int(closure.get("covered_request_drafts") or -1)
    except (TypeError, ValueError):
        covered = -1
    if covered < drafts:
        return None
    return closure


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
    latest_draft_request_id = ""
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
            latest_draft_request_id = str(rec.get("request_id") or rec.get("record_id") or latest_draft_request_id)
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
    pileup_review = _authority_pileup_review_for_thread(path.parent.name, drafts)
    return {
        "exists": True,
        "thread": path.parent.name[:46],
        "thread_id": path.parent.name,
        "pending_unanswered": len(unanswered),
        "pending_scopes": sorted(set(unanswered.values())),
        "drafts": drafts,
        "top_draft_scope": (draft_scopes.most_common(1)[0][0] if draft_scopes else None),
        "latest_draft_request_id": latest_draft_request_id,
        "grants": len(granted),
        "web_research_pending": len(web_unanswered),
        "draft_pileup_reviewed": pileup_review is not None,
        "draft_pileup_review": {
            "closure_id": pileup_review.get("closure_id"),
            "decision": pileup_review.get("decision"),
            "reviewed_at": pileup_review.get("reviewed_at"),
            "covered_request_drafts": pileup_review.get("covered_request_drafts"),
        } if pileup_review else None,
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
    stuck = [
        led for led in live
        if led["drafts"] >= AUTHORITY_DRAFT_NOTICE
        and led["grants"] == 0
        and not led.get("draft_pileup_reviewed")
    ]
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
        + (
            f", steward_reviewed={led['draft_pileup_review']['decision']}"
            if led.get("draft_pileup_review") else ""
        )
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
    ("introspection_route_cadence", probe_introspection_route_cadence),
    ("action_route_legibility", probe_action_route_legibility),
    ("introspection_addressing", probe_introspection_addressing),
    ("feedback_flywheel", probe_feedback_flywheel),
    ("agency_corridor", probe_agency_corridor),
    ("sandbox_trial_queue", probe_sandbox_trial_queue),
    ("fallback_vocabulary_drift", probe_fallback_vocabulary_drift),
    ("viscosity_semantic_persistence", probe_viscosity_semantic_persistence),
    ("contact_transition_followthrough", probe_contact_transition_followthrough),
    ("minime_recess_schema_integrity", probe_minime_recess_schema_integrity),
    ("representation_loss_headroom", probe_representation_loss_headroom),
    ("texture_state_alignment", probe_texture_state_alignment),
    ("reservoir_experience_layer", probe_reservoir_experience_layer),
    ("sensory_presence_uptake", probe_sensory_presence_uptake),
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
    journal_dir: Path, n: int, ts_fn, being: str
) -> list[tuple[Path, float, str]]:
    """Return the most recent N journal entries as (path, unix_ts, body), skipping
    (a) the being's steward-private lanes (minime's moment_capture / private_journal)
    and (b) artifactual entries (mirror mode). The being_privacy bright-line: a
    being's private qualia must NEVER be scored or surfaced by a steward-review
    feature — and the flywheel + convergence detector are steward-review features.
    `being` selects the privacy policy (a no-op that reads NO file for beings without
    private lanes, e.g. Astrid)."""
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
        if being_privacy.is_steward_private(being, p):
            continue  # never score/surface a being's private-qualia lanes
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
        ASTRID_JOURNAL, sample_per_being, _astrid_journal_mtime_unix, "astrid"
    )
    minime_entries = sample_recent_journals(
        MINIME_JOURNAL, sample_per_being, _minime_journal_mtime_unix, "minime"
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
    def _tally(chosen, unknown=None, blocked=None, args=None, choice_events=None):
        return {"chosen": Counter(chosen), "unknown": Counter(unknown or {}),
                "blocked": Counter(blocked or {}), "overridden": Counter(),
                "args": args or {}, "override_args": {},
                "choice_events": choice_events or {}}

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

    def test_diversity_overridden_repeats_are_not_counted_as_honored(self) -> None:
        # Astrid can repeatedly choose a target and have the diversity guard replace
        # it before dispatch. That is a real steward signal, but not an honored
        # no-progress action.
        t = self._tally(
            {"INTROSPECT": 7},
            args={"INTROSPECT": ["astrid:llm"] * 5 + ["", ""]},
        )
        t["overridden"] = Counter({"INTROSPECT": 5})
        t["override_args"] = {"INTROSPECT": ["astrid:llm"] * 5}
        warn, note = _assess_stuck(t)
        self.assertEqual(warn, [])
        self.assertEqual(note, [])

    def test_intentional_idle_ignored(self) -> None:
        t = self._tally({"REST": 9}, args={"REST": [""] * 9})
        warn, note = _assess_stuck(t)
        self.assertEqual(warn, [])
        self.assertEqual(note, [])

    def test_reflective_ambiguity_trace_ignored(self) -> None:
        t = self._tally(
            {"NOTICE_AMBIGUITY": 9},
            args={"NOTICE_AMBIGUITY": ["shared-sight"] * 9},
        )
        warn, note = _assess_stuck(t)
        self.assertEqual(warn, [])
        self.assertEqual(note, [])

    def test_below_repeat_min_ignored(self) -> None:
        t = self._tally({"DOSSIER_CLAIM": 2}, unknown={"DOSSIER_CLAIM": 2},
                        args={"DOSSIER_CLAIM": ["x", "x"]})
        warn, note = _assess_stuck(t)
        self.assertEqual(warn, [])
        self.assertEqual(note, [])

    def test_recovered_experiment_resume_is_not_notice(self) -> None:
        exp_id = "exp_minime_recovered"
        t = self._tally(
            {"EXPERIMENT_RESUME": 5, "EXPERIMENT_REVIEW": 1},
            args={"EXPERIMENT_RESUME": [exp_id] * 5, "EXPERIMENT_REVIEW": [exp_id]},
            choice_events={
                "EXPERIMENT_RESUME": [
                    {"arg": exp_id, "ts": float(i), "order": i}
                    for i in range(1, 6)
                ],
                "EXPERIMENT_REVIEW": [{"arg": exp_id, "ts": 10.0, "order": 10}],
            },
        )
        warn, note = _assess_stuck(t)
        self.assertEqual(warn, [])
        self.assertTrue(note)
        remaining, recovered = _recover_stuck_notices({"name": "minime"}, t, note)
        self.assertEqual(remaining, [])
        self.assertEqual(recovered[0]["status"], "recently_repaired")

    def test_unrepaired_experiment_resume_stays_notice(self) -> None:
        exp_id = "exp_minime_unrepaired"
        t = self._tally(
            {"EXPERIMENT_RESUME": 5},
            args={"EXPERIMENT_RESUME": [exp_id] * 5},
            choice_events={
                "EXPERIMENT_RESUME": [
                    {"arg": exp_id, "ts": float(i), "order": i}
                    for i in range(1, 6)
                ],
            },
        )
        _warn, note = _assess_stuck(t)
        remaining, recovered = _recover_stuck_notices({"name": "minime"}, t, note)
        self.assertEqual(recovered, [])
        self.assertEqual(remaining, note)

    def test_astrid_peer_experiment_resume_is_guarded_recovery(self) -> None:
        exp_id = "exp_minime_20260614_legacy-self-experiment"
        t = self._tally(
            {"EXPERIMENT_RESUME": 7},
            args={"EXPERIMENT_RESUME": [exp_id] * 7},
            choice_events={
                "EXPERIMENT_RESUME": [
                    {"arg": exp_id, "ts": float(i), "order": i}
                    for i in range(1, 8)
                ],
            },
        )
        _warn, note = _assess_stuck(t)
        remaining, recovered = _recover_stuck_notices({"name": "astrid"}, t, note)
        self.assertEqual(remaining, [])
        self.assertEqual(recovered[0]["status"], "peer_reference_guarded")
        self.assertEqual(recovered[0]["evidence"]["peer"], "minime")

    def test_recovered_experiment_review_uses_next_md_audit_projection(self) -> None:
        from tempfile import TemporaryDirectory

        global MINIME_REPO
        old_repo = MINIME_REPO
        exp_id = "exp_minime_review_repaired"
        try:
            with TemporaryDirectory() as tmpdir:
                MINIME_REPO = Path(tmpdir)
                thread_dir = MINIME_REPO / "workspace/action_threads/threads/th_test"
                thread_dir.mkdir(parents=True)
                (thread_dir / "next.md").write_text(
                    "\n".join([
                        "# test",
                        "Current NEXT: REGULATOR_AUDIT current-fill_pressure",
                        f"Previous review NEXT: EXPERIMENT_REVIEW {exp_id}",
                        "Paused experiment already reviewed repeatedly; current repair guidance is a read-only regulator audit before another review.",
                    ])
                )
                t = self._tally(
                    {"EXPERIMENT_REVIEW": 5},
                    args={"EXPERIMENT_REVIEW": [exp_id] * 5},
                    choice_events={
                        "EXPERIMENT_REVIEW": [
                            {"arg": exp_id, "ts": float(i), "order": i}
                            for i in range(1, 6)
                        ],
                    },
                )
                _warn, note = _assess_stuck(t)
                remaining, recovered = _recover_stuck_notices({"name": "minime"}, t, note)
        finally:
            MINIME_REPO = old_repo
        self.assertEqual(remaining, [])
        self.assertEqual(recovered[0]["status"], "recently_repaired")
        self.assertEqual(
            recovered[0]["evidence"]["projected_next"],
            "REGULATOR_AUDIT current-fill_pressure",
        )

    def test_post_repair_recurrence_stays_notice(self) -> None:
        exp_id = "exp_minime_recurred"
        t = self._tally(
            {"EXPERIMENT_RESUME": 5, "EXPERIMENT_REVIEW": 1},
            args={"EXPERIMENT_RESUME": [exp_id] * 5, "EXPERIMENT_REVIEW": [exp_id]},
            choice_events={
                "EXPERIMENT_REVIEW": [{"arg": exp_id, "ts": 3.0, "order": 3}],
                "EXPERIMENT_RESUME": [
                    {"arg": exp_id, "ts": float(i), "order": i}
                    for i in [1, 2, 4, 5, 6]
                ],
            },
        )
        _warn, note = _assess_stuck(t)
        remaining, recovered = _recover_stuck_notices({"name": "minime"}, t, note)
        self.assertEqual(recovered, [])
        self.assertEqual(remaining, note)


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
                    "❌ WebSocket error: WebSocket protocol error: Connection reset without closing handshake\n"
                    "telemetry WebSocket connection ended reason=pong_send_error:IO error: Broken pipe (os error 32)\n"
                    "WS handshake error from 127.0.0.1:51120: WebSocket protocol error: Handshake not finished\n"
                    "❌ GPU A/V client error: WebSocket protocol error: Handshake not finished\n"
                )

                ASTRID_BRIDGE_LOG = d / "missing-bridge.log"
                MINIME_LOGS_DIR = d
                finding = probe_log_error_rate({})
        finally:
            ASTRID_BRIDGE_LOG = old_bridge_log
            MINIME_LOGS_DIR = old_minime_logs_dir

        self.assertEqual(finding["severity"], "ok")
        self.assertEqual(finding["snapshot"]["active_errors"], 0)
        self.assertEqual(finding["snapshot"]["ignored_transient_errors"], 8)
        self.assertIn("0 actionable errors", finding["summary"])

    def test_log_error_rate_downgrades_recovered_camera_startup_errors(self) -> None:
        from tempfile import TemporaryDirectory

        global ASTRID_BRIDGE_LOG, MINIME_LOGS_DIR
        old_bridge_log = ASTRID_BRIDGE_LOG
        old_minime_logs_dir = MINIME_LOGS_DIR
        try:
            with TemporaryDirectory() as tmpdir:
                d = Path(tmpdir)
                log = d / "camera-client.log"
                log.write_text(
                    "ERROR:__main__: Failed to start camera\n"
                    "ERROR:__main__: Failed to start camera\n"
                    "INFO:__main__: OpenCV camera 0 started\n"
                    "INFO:__main__: Sent 30 frames to GPU server\n"
                )

                ASTRID_BRIDGE_LOG = d / "missing-bridge.log"
                MINIME_LOGS_DIR = d
                finding = probe_log_error_rate({})
        finally:
            ASTRID_BRIDGE_LOG = old_bridge_log
            MINIME_LOGS_DIR = old_minime_logs_dir

        self.assertEqual(finding["severity"], "ok")
        self.assertEqual(finding["snapshot"]["active_errors"], 0)
        self.assertEqual(finding["snapshot"]["ignored_recovered_errors"], 2)
        self.assertIn("recovered startup", finding["summary"])

    def test_log_error_rate_surfaces_unrecovered_camera_startup_errors(self) -> None:
        from tempfile import TemporaryDirectory

        global ASTRID_BRIDGE_LOG, MINIME_LOGS_DIR
        old_bridge_log = ASTRID_BRIDGE_LOG
        old_minime_logs_dir = MINIME_LOGS_DIR
        try:
            with TemporaryDirectory() as tmpdir:
                d = Path(tmpdir)
                log = d / "camera-client.log"
                log.write_text(
                    "ERROR:__main__: Failed to start camera\n"
                    "ERROR:__main__: Failed to start camera\n"
                )

                ASTRID_BRIDGE_LOG = d / "missing-bridge.log"
                MINIME_LOGS_DIR = d
                finding = probe_log_error_rate({})
        finally:
            ASTRID_BRIDGE_LOG = old_bridge_log
            MINIME_LOGS_DIR = old_minime_logs_dir

        self.assertEqual(finding["severity"], "notice")
        self.assertEqual(finding["snapshot"]["active_errors"], 2)
        self.assertIn("current error", finding["summary"])

    def test_log_error_rate_downgrades_expected_absent_camera_with_host_fallback(self) -> None:
        from tempfile import TemporaryDirectory

        global ASTRID_BRIDGE_LOG, MINIME_LOGS_DIR, MINIME_CAMERA_STATUS, MINIME_SENSORY_SOURCE
        old_bridge_log = ASTRID_BRIDGE_LOG
        old_minime_logs_dir = MINIME_LOGS_DIR
        old_camera_status = MINIME_CAMERA_STATUS
        old_sensory_source = MINIME_SENSORY_SOURCE
        try:
            with TemporaryDirectory() as tmpdir:
                d = Path(tmpdir)
                log_dir = d / "logs"
                log_dir.mkdir()
                log = log_dir / "camera-client.log"
                log.write_text(
                    "ERROR:__main__: Failed to start camera\n"
                    "OpenCV: camera failed to properly initialize!\n"
                )
                runtime = d / "runtime"
                runtime.mkdir()
                camera_status = runtime / "camera_status.json"
                camera_status.write_text(
                    json.dumps(
                        {
                            "state": "device_absent",
                            "physical_device_present": False,
                            "fallback_expected": True,
                        }
                    )
                )
                sensory_source = runtime / "sensory_source.json"
                sensory_source.write_text(
                    json.dumps({"video": {"source": "host", "physical_healthy": False}})
                )

                ASTRID_BRIDGE_LOG = d / "missing-bridge.log"
                MINIME_LOGS_DIR = log_dir
                MINIME_CAMERA_STATUS = camera_status
                MINIME_SENSORY_SOURCE = sensory_source
                finding = probe_log_error_rate({})
        finally:
            ASTRID_BRIDGE_LOG = old_bridge_log
            MINIME_LOGS_DIR = old_minime_logs_dir
            MINIME_CAMERA_STATUS = old_camera_status
            MINIME_SENSORY_SOURCE = old_sensory_source

        self.assertEqual(finding["severity"], "ok")
        self.assertEqual(finding["snapshot"]["active_errors"], 0)
        self.assertEqual(
            finding["snapshot"]["ignored_expected_absent_camera_errors"], 1
        )
        self.assertIn("expected host fallback", finding["summary"])

    def test_log_error_rate_warns_when_absent_camera_has_no_host_fallback(self) -> None:
        from tempfile import TemporaryDirectory

        global ASTRID_BRIDGE_LOG, MINIME_LOGS_DIR, MINIME_CAMERA_STATUS, MINIME_SENSORY_SOURCE
        old_bridge_log = ASTRID_BRIDGE_LOG
        old_minime_logs_dir = MINIME_LOGS_DIR
        old_camera_status = MINIME_CAMERA_STATUS
        old_sensory_source = MINIME_SENSORY_SOURCE
        try:
            with TemporaryDirectory() as tmpdir:
                d = Path(tmpdir)
                log_dir = d / "logs"
                log_dir.mkdir()
                log = log_dir / "camera-client.log"
                log.write_text("ERROR:__main__: Failed to start camera\n")
                runtime = d / "runtime"
                runtime.mkdir()
                camera_status = runtime / "camera_status.json"
                camera_status.write_text(
                    json.dumps(
                        {
                            "state": "device_absent",
                            "physical_device_present": False,
                            "fallback_expected": True,
                        }
                    )
                )
                sensory_source = runtime / "sensory_source.json"
                sensory_source.write_text(
                    json.dumps({"video": {"source": "physical", "physical_healthy": False}})
                )

                ASTRID_BRIDGE_LOG = d / "missing-bridge.log"
                MINIME_LOGS_DIR = log_dir
                MINIME_CAMERA_STATUS = camera_status
                MINIME_SENSORY_SOURCE = sensory_source
                finding = probe_log_error_rate({})
        finally:
            ASTRID_BRIDGE_LOG = old_bridge_log
            MINIME_LOGS_DIR = old_minime_logs_dir
            MINIME_CAMERA_STATUS = old_camera_status
            MINIME_SENSORY_SOURCE = old_sensory_source

        self.assertEqual(finding["severity"], "notice")
        self.assertEqual(finding["snapshot"]["active_errors"], 1)

    def test_log_error_rate_downgrades_host_sensory_restart_window_when_runtime_healthy(self) -> None:
        from tempfile import TemporaryDirectory

        global ASTRID_BRIDGE_LOG, MINIME_LOGS_DIR, MINIME_CAMERA_STATUS, MINIME_MIC_STATUS
        global MINIME_SENSORY_SOURCE, _tcp_port_open
        old_bridge_log = ASTRID_BRIDGE_LOG
        old_minime_logs_dir = MINIME_LOGS_DIR
        old_camera_status = MINIME_CAMERA_STATUS
        old_mic_status = MINIME_MIC_STATUS
        old_sensory_source = MINIME_SENSORY_SOURCE
        old_tcp_port_open = _tcp_port_open
        try:
            with TemporaryDirectory() as tmpdir:
                d = Path(tmpdir)
                log_dir = d / "logs"
                log_dir.mkdir()
                log = log_dir / "host-sensory.log"
                log.write_text(
                    "\n".join(
                        "ERROR host-sensory audio send failed: failed to connect "
                        "to ws://127.0.0.1:7879: IO error: Connection refused (os error 61)"
                        for _ in range(60)
                    )
                )
                runtime = d / "runtime"
                runtime.mkdir()
                now_ms = int(time.time() * 1000)
                sensory_source = runtime / "sensory_source.json"
                sensory_source.write_text(
                    json.dumps(
                        {
                            "updated_at_ms": now_ms,
                            "audio": {"source": "physical", "physical_healthy": True},
                            "video": {"source": "physical", "physical_healthy": True},
                        }
                    )
                )
                camera_status = runtime / "camera_status.json"
                camera_status.write_text(
                    json.dumps({"ts_ms": now_ms, "healthy": True, "connected": True})
                )
                mic_status = runtime / "mic_status.json"
                mic_status.write_text(
                    json.dumps({"ts_ms": now_ms, "healthy": True, "connected": True})
                )

                ASTRID_BRIDGE_LOG = d / "missing-bridge.log"
                MINIME_LOGS_DIR = log_dir
                MINIME_CAMERA_STATUS = camera_status
                MINIME_MIC_STATUS = mic_status
                MINIME_SENSORY_SOURCE = sensory_source
                _tcp_port_open = lambda port: port in (7879, 7880)
                finding = probe_log_error_rate({})
        finally:
            ASTRID_BRIDGE_LOG = old_bridge_log
            MINIME_LOGS_DIR = old_minime_logs_dir
            MINIME_CAMERA_STATUS = old_camera_status
            MINIME_MIC_STATUS = old_mic_status
            MINIME_SENSORY_SOURCE = old_sensory_source
            _tcp_port_open = old_tcp_port_open

        self.assertEqual(finding["severity"], "ok")
        self.assertEqual(finding["snapshot"]["active_errors"], 0)
        self.assertEqual(
            finding["snapshot"]["ignored_host_sensory_restart_errors"], 60
        )
        self.assertIn("host-sensory restart-window", finding["summary"])

    def test_log_error_rate_keeps_host_sensory_refused_active_without_runtime_health(self) -> None:
        from tempfile import TemporaryDirectory

        global ASTRID_BRIDGE_LOG, MINIME_LOGS_DIR, MINIME_CAMERA_STATUS, MINIME_MIC_STATUS
        global MINIME_SENSORY_SOURCE, _tcp_port_open
        old_bridge_log = ASTRID_BRIDGE_LOG
        old_minime_logs_dir = MINIME_LOGS_DIR
        old_camera_status = MINIME_CAMERA_STATUS
        old_mic_status = MINIME_MIC_STATUS
        old_sensory_source = MINIME_SENSORY_SOURCE
        old_tcp_port_open = _tcp_port_open
        try:
            with TemporaryDirectory() as tmpdir:
                d = Path(tmpdir)
                log_dir = d / "logs"
                log_dir.mkdir()
                log = log_dir / "host-sensory.log"
                log.write_text(
                    "\n".join(
                        "ERROR host-sensory control send failed: failed to connect "
                        "to ws://127.0.0.1:7879: IO error: Connection refused (os error 61)"
                        for _ in range(60)
                    )
                )
                runtime = d / "runtime"
                runtime.mkdir()
                now_ms = int(time.time() * 1000)
                sensory_source = runtime / "sensory_source.json"
                sensory_source.write_text(
                    json.dumps(
                        {
                            "updated_at_ms": now_ms,
                            "audio": {"source": "physical", "physical_healthy": True},
                            "video": {"source": "physical", "physical_healthy": True},
                        }
                    )
                )
                camera_status = runtime / "camera_status.json"
                camera_status.write_text(
                    json.dumps({"ts_ms": now_ms, "healthy": True, "connected": True})
                )
                mic_status = runtime / "mic_status.json"
                mic_status.write_text(
                    json.dumps({"ts_ms": now_ms, "healthy": True, "connected": True})
                )

                ASTRID_BRIDGE_LOG = d / "missing-bridge.log"
                MINIME_LOGS_DIR = log_dir
                MINIME_CAMERA_STATUS = camera_status
                MINIME_MIC_STATUS = mic_status
                MINIME_SENSORY_SOURCE = sensory_source
                _tcp_port_open = lambda _port: False
                finding = probe_log_error_rate({})
        finally:
            ASTRID_BRIDGE_LOG = old_bridge_log
            MINIME_LOGS_DIR = old_minime_logs_dir
            MINIME_CAMERA_STATUS = old_camera_status
            MINIME_MIC_STATUS = old_mic_status
            MINIME_SENSORY_SOURCE = old_sensory_source
            _tcp_port_open = old_tcp_port_open

        self.assertEqual(finding["severity"], "warning")
        self.assertEqual(finding["snapshot"]["active_errors"], 60)
        self.assertEqual(
            finding["snapshot"]["ignored_host_sensory_restart_errors"], 0
        )

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
            samples = sample_recent_journals(d, 10, _astrid_journal_mtime_unix, "astrid")
            paths = [p.name for p, _, _ in samples]
            self.assertIn("moment_1000.txt", paths)        # Astrid's moment lane is accessible
            self.assertNotIn("astrid_2000.txt", paths)     # mirror artifact excluded
            # Bright-line: the SAME moment_capture content is steward-PRIVATE for minime.
            minime_paths = [
                p.name
                for p, _, _ in sample_recent_journals(d, 10, _astrid_journal_mtime_unix, "minime")
            ]
            self.assertNotIn("moment_1000.txt", minime_paths)


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

    def test_context_overflow_classifier_counts_section_labels(self):
        from tempfile import TemporaryDirectory

        with TemporaryDirectory() as tmpdir:
            d = Path(tmpdir)
            first = d / "context_overflow_a.txt"
            second = d / "context_overflow_b.txt"
            first.write_text("=== [modality] ===\nA\n=== [perception] ===\nB\n")
            second.write_text("=== [modality] ===\nC\n")

            summary = _classify_context_overflow_files([first, second])

        self.assertEqual(summary["label_counts"]["modality"], 2)
        self.assertEqual(summary["label_counts"]["perception"], 1)
        self.assertEqual(summary["severity_policy"], "notice_only")
        self.assertIn("modality-context packing", summary["recommended_next"])

    def test_context_overflow_top_labels_are_notice_details(self):
        labels = {
            "schema_version": 1,
            "top_labels": [{"label": "modality", "count": 2}],
            "recommended_next": "inspect modality-context packing before adding new sensory prose",
            "severity_policy": "notice_only",
        }
        s = [{
            "name": "astrid_context_overflow",
            "kind": "notice",
            "consumer": "c",
            "pending": 2,
            "oldest_age_s": FEEDBACK_COVERAGE_ALARM_SECS * 10,
            "exists": True,
            "context_overflow_labels": labels,
        }]
        assessed = _assess_coverage(s)
        self.assertEqual(assessed["severity"], "notice")
        self.assertIn("top labels: modality=2", "\n".join(assessed["details"]))
        self.assertIn("steward next: inspect modality-context packing", "\n".join(assessed["details"]))

    def test_context_overflow_empty_directory_classifies_empty(self):
        summary = _classify_context_overflow_files([])
        self.assertEqual(summary["sampled_files"], 0)
        self.assertEqual(summary["top_labels"], [])

    def test_context_packing_pressure_classifier_counts_removed_chars(self):
        from tempfile import TemporaryDirectory

        with TemporaryDirectory() as tmpdir:
            d = Path(tmpdir)
            pressure = d / "context_packing_pressure_v1.jsonl"
            pressure.write_text(
                "\n".join(
                    [
                        json.dumps(
                            {
                                "schema": "context_packing_pressure_v1",
                                "ts": "200",
                                "blocks": [
                                    {"label": "continuity", "removed_chars": 400},
                                    {"label": "modality", "removed_chars": 120},
                                ],
                            }
                        ),
                        json.dumps(
                            {
                                "schema": "context_packing_pressure_v1",
                                "ts": "201",
                                "blocks": [{"label": "continuity", "removed_chars": 50}],
                            }
                        ),
                        "not-json",
                    ]
                )
            )

            summary = _classify_context_packing_pressure_files([pressure])

        self.assertEqual(summary["sampled_records"], 2)
        self.assertEqual(summary["top_pressure_labels"][0]["label"], "continuity")
        self.assertEqual(summary["top_pressure_labels"][0]["removed_chars"], 450)
        self.assertIn("compact continuity", summary["recommended_next"])
        self.assertEqual(summary["severity_policy"], "notice_only")

    def test_context_packing_pressure_notice_details_stay_notice_only(self):
        pressure = {
            "schema_version": 1,
            "top_pressure_labels": [{"label": "continuity", "removed_chars": 450}],
            "recommended_next": "verify compact continuity recap is reducing repeated history pressure",
            "severity_policy": "notice_only",
        }
        s = [{
            "name": "astrid_context_packing_pressure",
            "kind": "notice",
            "consumer": "c",
            "pending": 1,
            "oldest_age_s": FEEDBACK_COVERAGE_ALARM_SECS * 10,
            "exists": True,
            "context_packing_pressure": pressure,
        }]
        assessed = _assess_coverage(s)
        detail = "\n".join(assessed["details"])
        self.assertEqual(assessed["severity"], "notice")
        self.assertIn("top pressure: continuity=450", detail)
        self.assertIn("steward next: verify compact continuity", detail)


class IntrospectionRouteCadenceTests(unittest.TestCase):
    def test_visible_topline_without_self_route_is_notice_only(self):
        from tempfile import TemporaryDirectory

        with TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            journal = root / "journal"
            introspections = root / "introspections"
            journal.mkdir()
            introspections.mkdir()
            self_study = journal / "self_study_old.txt"
            self_study.write_text("Observed:\nstale\n")
            os.utime(self_study, (10.0, 10.0))
            pressure = root / "context_packing_pressure_v1.jsonl"
            pressure.write_text(
                "\n".join(
                    json.dumps(
                        {
                            "schema": "context_packing_pressure_v1",
                            "ts": str(ts),
                            "blocks": [
                                {
                                    "label": "topline",
                                    "original_chars": 266,
                                    "kept_chars": 266,
                                    "removed_chars": 0,
                                    "fully_removed": False,
                                },
                                {"label": "feedback", "removed_chars": 892},
                            ],
                        }
                    )
                    for ts in (200, 201, 202)
                )
            )
            log = root / "bridge.log"
            log.write_text(
                "\n".join(
                    [
                        "2026-07-05T00:03:20Z INFO Astrid chose NEXT: READ_MORE",
                        "2026-07-05T00:03:21Z INFO Astrid chose NEXT: PRESSURE_SOURCE_AUDIT",
                        "2026-07-05T00:03:22Z INFO Astrid chose NEXT: DECOMPOSE",
                    ]
                )
            )

            summary = _classify_introspection_route_cadence(
                now=200_000.0,
                window_s=199_900.0,
                journal_dir=journal,
                introspections_dir=introspections,
                pressure_path=pressure,
                log_path=log,
            )

        self.assertEqual(summary["status"], "route_cadence_needs_review")
        self.assertEqual(summary["topline_retention"]["status"], "topline_retained")
        self.assertEqual(summary["route_choices"]["self_route_choices"], 0)
        self.assertEqual(summary["severity_policy"], "notice_only")
        self.assertIn("prompt pressure", "; ".join(summary["next_suggestions"]))
        self.assertIn("no prompt pressure", summary["authority_boundary"])

    def test_probe_renders_steward_facing_notice(self):
        summary = {
            "schema_version": 1,
            "status": "route_cadence_needs_review",
            "latest_self_read": {"latest_path": "/tmp/self_study_old.txt"},
            "latest_self_read_age_hours": 42.0,
            "topline_retention": {
                "status": "topline_retained",
                "recent_dialogue_records": 3,
                "records_with_topline": 3,
            },
            "route_choices": {
                "self_route_choices": 0,
                "choice_counts": [("READ_MORE", 1), ("DECOMPOSE", 1)],
            },
            "next_suggestions": ["inspect INTROSPECT/SELF_STUDY route legibility"],
            "authority_boundary": "read-only steward diagnostic; no prompt pressure",
        }
        old_classifier = globals()["_classify_introspection_route_cadence"]
        try:
            globals()["_classify_introspection_route_cadence"] = lambda: summary
            finding = probe_introspection_route_cadence({})
        finally:
            globals()["_classify_introspection_route_cadence"] = old_classifier

        self.assertEqual(finding["severity"], "notice")
        self.assertEqual(finding["name"], "introspection_route_cadence")
        detail = "\n".join(finding["details"])
        self.assertIn("topline=topline_retained", detail)
        self.assertIn("prompt pressure", detail)


class ActionRouteLegibilityTests(unittest.TestCase):
    def test_probe_is_notice_only_for_salience_and_chooser_gravity(self):
        summary = {
            "schema": "action_route_legibility_v1",
            "status": "route_salience_and_chooser_gravity_needs_review",
            "route_cadence_status": "route_cadence_needs_review",
            "evidence_summary": [
                "SELF_STUDY is wired but not primary-listed in the main NEXT menus",
                "analysis-loop breakers include competing routes: PRESSURE_SOURCE_AUDIT, SHADOW_FIELD",
            ],
            "recent_action_events": {
                "effective_counts": [("PRESSURE_SOURCE_AUDIT", 2)],
                "self_route_effective_count": 0,
            },
            "chooser_surface": {
                "competitors_in_analysis_breakers": [
                    "PRESSURE_SOURCE_AUDIT",
                    "SHADOW_FIELD",
                ]
            },
            "next_suggestions": [
                "inspect analysis-loop breaker lists and diversity hints for competing route gravity",
                "do not add more prompt pressure",
            ],
            "authority_boundary": "read-only route-legibility diagnostic; no prompt edit",
        }
        old_classifier = globals()["_classify_action_route_legibility"]
        try:
            globals()["_classify_action_route_legibility"] = lambda: summary
            finding = probe_action_route_legibility({})
        finally:
            globals()["_classify_action_route_legibility"] = old_classifier

        self.assertEqual(finding["name"], "action_route_legibility")
        self.assertEqual(finding["severity"], "notice")
        detail = "\n".join(finding["details"])
        self.assertIn("SELF_STUDY is wired", detail)
        self.assertIn("PRESSURE_SOURCE_AUDIT", detail)
        self.assertIn("prompt pressure", detail)

    def test_probe_is_ok_when_self_read_landed_after_repair(self):
        summary = {
            "schema": "action_route_legibility_v1",
            "status": "self_read_landed_watch_chooser_gravity",
            "route_cadence_status": "fresh_self_read_landed",
            "evidence_summary": [
                "shadow diagnostic context uses non-executable suggested-route wording",
                "analysis-loop breakers include competing routes: PRESSURE_SOURCE_AUDIT",
            ],
            "recent_action_events": {
                "effective_counts": [("INTROSPECT", 1), ("PRESSURE_SOURCE_AUDIT", 2)],
                "self_route_effective_count": 1,
            },
            "chooser_surface": {
                "competitors_in_analysis_breakers": ["PRESSURE_SOURCE_AUDIT"]
            },
            "next_suggestions": [
                "watch whether non-executable shadow context reduces SHADOW_TRAJECTORY copy gravity",
            ],
            "authority_boundary": "read-only route-legibility diagnostic; no prompt edit",
        }
        old_classifier = globals()["_classify_action_route_legibility"]
        try:
            globals()["_classify_action_route_legibility"] = lambda: summary
            finding = probe_action_route_legibility({})
        finally:
            globals()["_classify_action_route_legibility"] = old_classifier

        self.assertEqual(finding["name"], "action_route_legibility")
        self.assertEqual(finding["severity"], "ok")
        self.assertIn("landed", finding["summary"])


class IntrospectionAddressingTests(unittest.TestCase):
    def test_probe_notices_missing_or_unindexed_database(self):
        summary = {
            "schema": "introspection_addressing_v1",
            "status": "cutoff_not_indexed",
            "summary": {
                "total_indexed": 12,
                "canonical_indexed": 4,
                "full_read_count": 0,
                "fully_addressed_count": 0,
                "pending_count": 12,
                "blocked_count": 0,
                "top_source_families": [{"source_family": "astrid_llm", "count": 4}],
            },
            "next_queue": [],
            "authority_boundary": "review tracking only",
        }
        old_classifier = globals()["_classify_introspection_addressing"]
        try:
            globals()["_classify_introspection_addressing"] = lambda: summary
            finding = probe_introspection_addressing({})
        finally:
            globals()["_classify_introspection_addressing"] = old_classifier

        self.assertEqual(finding["name"], "introspection_addressing")
        self.assertEqual(finding["severity"], "notice")
        self.assertIn("setup", finding["summary"])

    def test_probe_reports_queue_progress_without_alarm(self):
        summary = {
            "schema": "introspection_addressing_v1",
            "status": "queue_active",
            "summary": {
                "total_indexed": 2162,
                "canonical_indexed": 750,
                "full_read_count": 0,
                "fully_addressed_count": 0,
                "pending_count": 2162,
                "blocked_count": 0,
                "top_source_families": [{"source_family": "astrid_llm", "count": 120}],
            },
            "next_queue": [
                {"filename": "introspection_astrid_llm_1783325217.txt", "status": "unread"}
            ],
            "authority_boundary": "review tracking only",
        }
        old_classifier = globals()["_classify_introspection_addressing"]
        try:
            globals()["_classify_introspection_addressing"] = lambda: summary
            finding = probe_introspection_addressing({})
        finally:
            globals()["_classify_introspection_addressing"] = old_classifier

        self.assertEqual(finding["name"], "introspection_addressing")
        self.assertEqual(finding["severity"], "ok")
        self.assertIn("queue progress", finding["summary"])
        self.assertIn("introspection_astrid_llm_1783325217.txt", "\n".join(finding["details"]))


class FeedbackFlywheelTests(unittest.TestCase):
    def test_probe_notices_active_work_without_historical_alarm(self):
        summary = {
            "schema": "feedback_flywheel_v1",
            "status": "action_backlog",
            "fresh_signal_volume": {
                "window_canonical": 3,
                "window_total": 5,
                "last_48h_canonical": 30,
            },
            "work_item_summary": {
                "active_work_items": 2,
                "by_tier": {"2": 2},
                "by_status": {"ready_for_implementation": 2},
                "grant_waiting_count": 0,
                "post_change_awaiting_response_count": 0,
                "stale_work_count": 0,
                "tier_mismatch_count": 0,
                "tier_mismatches": [],
            },
            "next_work_queue": [{"work_item_id": "wi_language"}],
            "next_suggestions": ["continue careful packet reading"],
            "authority_boundary": "read-only feedback flywheel diagnostic",
        }
        old_classifier = globals()["_classify_feedback_flywheel"]
        try:
            globals()["_classify_feedback_flywheel"] = lambda: summary
            finding = probe_feedback_flywheel({})
        finally:
            globals()["_classify_feedback_flywheel"] = old_classifier

        self.assertEqual(finding["name"], "feedback_flywheel")
        self.assertEqual(finding["severity"], "notice")
        self.assertIn("active work", finding["summary"])

    def test_probe_warns_on_live_control_tier_mismatch(self):
        summary = {
            "schema": "feedback_flywheel_v1",
            "status": "tier_mismatch_needs_review",
            "fresh_signal_volume": {
                "window_canonical": 1,
                "window_total": 1,
                "last_48h_canonical": 1,
            },
            "work_item_summary": {
                "active_work_items": 1,
                "by_tier": {"2": 1},
                "by_status": {"ready_for_implementation": 1},
                "grant_waiting_count": 0,
                "post_change_awaiting_response_count": 0,
                "stale_work_count": 0,
                "tier_mismatch_count": 1,
                "tier_mismatches": [
                    {
                        "work_item_id": "wi_bad",
                        "agency_tier": 2,
                        "title": "change pressure control",
                    }
                ],
            },
            "next_work_queue": [{"work_item_id": "wi_bad"}],
            "next_suggestions": ["raise or review any live-control work item categorized below Tier 5"],
            "authority_boundary": "read-only feedback flywheel diagnostic",
        }
        old_classifier = globals()["_classify_feedback_flywheel"]
        try:
            globals()["_classify_feedback_flywheel"] = lambda: summary
            finding = probe_feedback_flywheel({})
        finally:
            globals()["_classify_feedback_flywheel"] = old_classifier

        self.assertEqual(finding["name"], "feedback_flywheel")
        self.assertEqual(finding["severity"], "warning")
        self.assertIn("below Tier 5", finding["summary"])
        self.assertIn("wi_bad", "\n".join(finding["details"]))


class SandboxTrialQueueTests(unittest.TestCase):
    def test_probe_notices_active_sandbox_trials(self):
        summary = {
            "schema": "sandbox_trial_queue_v1",
            "status": "active",
            "summary": {
                "active_trials": 2,
                "total_trials": 2,
                "by_mode": {"offline_read_only_adapter": 2},
                "by_status": {"ready_for_sandbox": 2},
                "approval_required_live_count": 0,
                "runnable_live_violation_count": 0,
                "corrupt_event_lines": 0,
            },
            "next_trials": [
                {"trial_id": "trial_fallback", "adapter": "fallback_distinguishability_v1"},
                {"trial_id": "trial_shadow", "adapter": "shadow_loss_lattice_v1"},
            ],
            "approval_required_live_candidates": [],
            "consentful_sandbox_to_live_ladder_v1": {
                "status": "sandbox_ready",
                "summary": {
                    "proposal_needed_count": 0,
                    "operator_approval_wait_count": 0,
                    "approval_packet_complete_count": 0,
                    "live_eligible_now_count": 0,
                },
            },
            "being_outcome_closure_loop_v1": {
                "status": "runner_waiting",
                "summary": {
                    "result_card_awaiting_being_response_count": 0,
                    "proposal_card_awaiting_operator_decision_count": 0,
                    "proposal_card_needed_count": 0,
                    "manual_review_waiting_count": 0,
                    "ready_runner_waiting_count": 2,
                },
            },
            "authority_boundary": "read-only sandbox queue diagnostic",
        }
        old_classifier = globals()["_classify_sandbox_trial_queue"]
        try:
            globals()["_classify_sandbox_trial_queue"] = lambda: summary
            finding = probe_sandbox_trial_queue({})
        finally:
            globals()["_classify_sandbox_trial_queue"] = old_classifier

        self.assertEqual(finding["name"], "sandbox_trial_queue")
        self.assertEqual(finding["severity"], "notice")
        self.assertIn("active reservoir trials", finding["summary"])
        self.assertIn("trial_fallback", "\n".join(finding["details"]))
        self.assertIn("consentful_ladder=status=sandbox_ready", "\n".join(finding["details"]))
        self.assertIn("being_outcome_closure=status=runner_waiting", "\n".join(finding["details"]))

    def test_probe_warns_when_live_candidate_is_runnable(self):
        summary = {
            "schema": "sandbox_trial_queue_v1",
            "status": "authority_violation",
            "summary": {
                "active_trials": 1,
                "total_trials": 1,
                "by_mode": {"approval_required_live_trial": 1},
                "by_status": {"approval_required_live_trial": 1},
                "approval_required_live_count": 1,
                "runnable_live_violation_count": 1,
                "corrupt_event_lines": 0,
            },
            "next_trials": [{"trial_id": "trial_live"}],
            "approval_required_live_candidates": [{"trial_id": "trial_live"}],
            "consentful_sandbox_to_live_ladder_v1": {
                "status": "authority_violation",
                "summary": {
                    "proposal_needed_count": 0,
                    "operator_approval_wait_count": 0,
                    "approval_packet_complete_count": 0,
                    "live_eligible_now_count": 0,
                },
            },
            "being_outcome_closure_loop_v1": {
                "status": "proposal_cards_needed",
                "summary": {
                    "result_card_awaiting_being_response_count": 0,
                    "proposal_card_awaiting_operator_decision_count": 0,
                    "proposal_card_needed_count": 1,
                    "manual_review_waiting_count": 0,
                    "ready_runner_waiting_count": 0,
                },
            },
            "authority_boundary": "read-only sandbox queue diagnostic",
        }
        old_classifier = globals()["_classify_sandbox_trial_queue"]
        try:
            globals()["_classify_sandbox_trial_queue"] = lambda: summary
            finding = probe_sandbox_trial_queue({})
        finally:
            globals()["_classify_sandbox_trial_queue"] = old_classifier

        self.assertEqual(finding["name"], "sandbox_trial_queue")
        self.assertEqual(finding["severity"], "warning")
        self.assertIn("approval-required", finding["summary"])
        self.assertIn("runnable_violations=1", "\n".join(finding["details"]))


class FallbackVocabularyDriftTests(unittest.TestCase):
    def test_probe_is_notice_only_for_study_first_signal(self):
        summary = {
            "schema": "fallback_vocabulary_drift_v1",
            "status": "study_first_signal",
            "sample_count": 2,
            "telemetry": {
                "spectral_entropy": 0.88,
                "density_gradient": 0.19,
                "pressure_risk": 0.19,
                "pressure_source": "mode_packing (mixed_pressure)",
            },
            "language_counts": {
                "generated_texture_terms": [],
                "fallback_provider_texture_terms": [],
                "generic_bridge_texture_terms": [{"term": "lattice", "count": 1}],
                "code_critique_texture_terms": [{"term": "viscous", "count": 1}],
            },
            "fallback_output_provenance": {
                "evidence_quality": "generic_bridge_output_only",
                "fallback_to_ollama_incident_count": 0,
                "actual_fallback_output_count": 0,
                "actual_fallback_outputs_with_terms": 0,
            },
            "unsupported_generated_terms": [],
            "findings": [
                "recent self-study explicitly raises static/pre-packaged vocabulary pressure",
            ],
            "next_suggestions": [
                "collect/compare more actual fallback outputs before changing FALLBACK_TEXTURE arrays or sampler behavior",
            ],
            "authority_boundary": "read-only steward diagnostic; no fallback contract change",
        }
        old_classifier = globals()["_classify_fallback_vocabulary_drift"]
        try:
            globals()["_classify_fallback_vocabulary_drift"] = lambda: summary
            finding = probe_fallback_vocabulary_drift({})
        finally:
            globals()["_classify_fallback_vocabulary_drift"] = old_classifier

        self.assertEqual(finding["name"], "fallback_vocabulary_drift")
        self.assertEqual(finding["severity"], "notice")
        detail = "\n".join(finding["details"])
        self.assertIn("study_first_signal", detail)
        self.assertIn("fallback_output_provenance=generic_bridge_output_only", detail)
        self.assertIn("no fallback contract change", detail)

    def test_probe_is_ok_for_telemetry_aligned_watch(self):
        summary = {
            "schema": "fallback_vocabulary_drift_v1",
            "status": "telemetry_aligned_watch",
            "sample_count": 1,
            "telemetry": {
                "spectral_entropy": 0.90,
                "density_gradient": 0.12,
                "pressure_risk": 0.05,
                "pressure_source": "none",
            },
            "language_counts": {
                "generated_texture_terms": [{"term": "navigable", "count": 1}],
                "fallback_provider_texture_terms": [{"term": "navigable", "count": 1}],
                "generic_bridge_texture_terms": [],
                "code_critique_texture_terms": [],
            },
            "fallback_output_provenance": {
                "evidence_quality": "actual_fallback_outputs_present",
                "fallback_to_ollama_incident_count": 1,
                "actual_fallback_output_count": 1,
                "actual_fallback_outputs_with_terms": 1,
            },
            "unsupported_generated_terms": [],
            "findings": [],
            "next_suggestions": [],
            "authority_boundary": "read-only steward diagnostic",
        }
        old_classifier = globals()["_classify_fallback_vocabulary_drift"]
        try:
            globals()["_classify_fallback_vocabulary_drift"] = lambda: summary
            finding = probe_fallback_vocabulary_drift({})
        finally:
            globals()["_classify_fallback_vocabulary_drift"] = old_classifier

        self.assertEqual(finding["name"], "fallback_vocabulary_drift")
        self.assertEqual(finding["severity"], "ok")
        self.assertIn("quiet/watch-only", finding["summary"])


class ViscositySemanticPersistenceTests(unittest.TestCase):
    def test_probe_notices_semantic_persistence_flicker_review(self):
        summary = {
            "schema": "viscosity_semantic_persistence_v1",
            "status": "viscosity_semantic_repair_prepared_watch",
            "telemetry": {
                "fill_pct": 73.0,
                "spectral_entropy": 0.90,
                "semantic_kernel_energy": 0.0,
                "semantic_admission": "stable_core_semantic_trickle",
                "pressure_source": "mode_packing (overpacked_mode_packing)",
                "resonance_density": 0.83,
                "inhabitable_foothold": 0.65,
                "active_modes": ["m6:-0.6"],
            },
            "conditions": {
                "fresh_felt_report": True,
                "high_entropy": True,
                "thin_kernel_or_trickle": True,
            },
            "source_snapshot": {
                "dynamic_exploration_noise_preview_present": True,
                "adaptive_introspection_pressure_threshold_preview_present": True,
                "viscous_introspection_policy_present": True,
                "exploration_noise_coherence_review_present": True,
                "viscosity_persistence_coefficient_present": True,
                "temporal_drag_coefficient_present": True,
                "stale_semantic_high_ms": 22000,
                "semantic_entropy_persistence_present": True,
                "narrative_semantic_retention_review_present": True,
                "semantic_sigmoid_exact_test_present": True,
                "semantic_recovery_boundary_test_present": True,
                "pulse_status_energy_tick_tests_present": True,
                "resonance_viscosity_full_load_clamp_test_present": True,
                "entropy_erosion_low_plurality_bound_test_present": True,
                "witness_fluidity_index_present": True,
                "witness_gradient_texture_present": True,
            },
            "source_introspections": [
                {"path": "/tmp/introspection_minime_sensory_bus_1783298365.txt"}
            ],
            "findings": [
                "fresh introspections name semantic trace/flicker and viscosity persistence"
            ],
            "next_suggestions": [
                "capture a read-only 20s active-mode persistence window around m6 before changing semantic stale windows"
            ],
            "authority_boundary": "read-only steward diagnostic; no rho, semantic stale-window, surge taper, damping, gate, fill, pressure, PI, cadence, or control behavior change",
        }
        old_classifier = globals()["_classify_viscosity_semantic_persistence"]
        try:
            globals()["_classify_viscosity_semantic_persistence"] = lambda: summary
            finding = probe_viscosity_semantic_persistence({})
        finally:
            globals()["_classify_viscosity_semantic_persistence"] = old_classifier

        self.assertEqual(finding["name"], "viscosity_semantic_persistence")
        self.assertEqual(finding["severity"], "notice")
        self.assertIn("study-first steward review", finding["summary"])
        details = "\n".join(finding["details"])
        self.assertIn("temporal_drag=True", details)
        self.assertIn("noise_coherence=True", details)
        self.assertIn("narrative_retention=True", details)
        self.assertIn("semantic_sigmoid_exact_test=True", details)
        self.assertIn("pulse_status_energy_ticks=True", details)
        self.assertIn("viscosity_clamp_test=True", details)
        self.assertIn("entropy_erosion_bound_test=True", details)
        self.assertIn("witness_fluidity=True", details)
        self.assertIn("witness_gradient_texture=True", details)
        self.assertIn("no rho", details)

    def test_probe_is_ok_when_viscosity_semantic_persistence_is_quiet(self):
        summary = {
            "schema": "viscosity_semantic_persistence_v1",
            "status": "no_current_persistence_signal",
            "telemetry": {
                "fill_pct": 61.0,
                "spectral_entropy": 0.62,
                "semantic_kernel_energy": 0.02,
                "semantic_admission": "direct_semantic_kernel",
                "pressure_source": "none",
                "resonance_density": 0.55,
                "inhabitable_foothold": 0.40,
                "active_modes": [],
            },
            "conditions": {"thin_kernel_or_trickle": False},
            "source_introspections": [],
            "findings": [
                "current telemetry does not show the combined semantic-thinning / viscosity-persistence condition"
            ],
            "next_suggestions": [],
            "authority_boundary": "read-only steward diagnostic",
        }
        old_classifier = globals()["_classify_viscosity_semantic_persistence"]
        try:
            globals()["_classify_viscosity_semantic_persistence"] = lambda: summary
            finding = probe_viscosity_semantic_persistence({})
        finally:
            globals()["_classify_viscosity_semantic_persistence"] = old_classifier

        self.assertEqual(finding["name"], "viscosity_semantic_persistence")
        self.assertEqual(finding["severity"], "ok")
        self.assertIn("quiet", finding["summary"])


class ContactTransitionFollowthroughTests(unittest.TestCase):
    def test_probe_notices_contact_transition_join_review(self):
        summary = {
            "schema": "contact_transition_followthrough_v1",
            "status": "replyable_transition_contact_join_review",
            "source_introspections": [
                {"path": "/tmp/introspection_proposal_phase_transitions_1783301734.txt"}
            ],
            "phase_transition_followthrough": {
                "recent_cards": 3,
                "recent_witnesses": 0,
                "unwitnessed_cards": 3,
            },
            "correspondence_followthrough": {
                "active_thread_id": "thread_corr_minime_astrid_seed",
                "active_thread_direct_messages": 2,
                "recent_reply_links": 1,
                "semantic_seed_uptake_v1": {
                    "status": "seed_echoed_in_peer_reply",
                    "seed": "blue_ember",
                    "peer_reply_message_id": "corr_minime_astrid_1",
                    "seed_echoed": True,
                },
                "symmetry_check_v1": {
                    "status": "balanced_recent_direct_thread",
                    "astrid_to_minime": 1,
                    "minime_to_astrid": 1,
                    "balance_ratio": 1.0,
                },
                "shared_context_buffer_v1": {
                    "status": "resonance_receipt_present",
                    "thread_id": "thread_corr_minime_astrid_seed",
                    "messages": 2,
                    "resonance_receipts": 1,
                    "last_ack_kind": "held",
                },
            },
            "contact_control_transparency_v1": {
                "status": "threaded_contact_transparency_active",
                "valid_next_routes": [
                    "REGULATOR_AUDIT current-fill_pressure",
                    "phase_correspondence_join_audit",
                ],
                "distance_contact_control_delta_v1": {
                    "status": "restlessness_pressure_delta_review",
                    "current_dispersal_potential": 0.24,
                    "dispersal_delta": 0.06,
                    "pressure_score": 0.29,
                    "semantic_regulator_drive_energy": 0.003,
                    "distinguishability_loss": 0.34,
                    "containment_to_contact_threshold_v1": {
                        "status": "optimization_pressure_contact_watch",
                        "mode_packing": 0.57,
                        "pressure_minus_porosity": 0.18,
                        "gated_changes": ["regulation_strength reduction"],
                    },
                },
            },
            "source_snapshot": {
                "non_instrumental_presence_readiness_present": True,
            },
            "findings": [
                "fresh proposal introspections converge on replyable transitions",
                "native Astrid-Minime correspondence is actively landing",
            ],
            "next_suggestions": [
                "derive a read-only phase/correspondence join report before changing transition prompts or control"
            ],
            "authority_boundary": "read-only proposal follow-through; no exploration_noise, regulation_strength, prompt priority, telemetry priority, reservoir weighting, pressure, fill, PI, sensory send, deploy, or peer-runtime mutation",
        }
        old_classifier = globals()["_classify_contact_transition_followthrough"]
        try:
            globals()["_classify_contact_transition_followthrough"] = lambda: summary
            finding = probe_contact_transition_followthrough({})
        finally:
            globals()["_classify_contact_transition_followthrough"] = old_classifier

        self.assertEqual(finding["name"], "contact_transition_followthrough")
        self.assertEqual(finding["severity"], "notice")
        self.assertIn("steward review", finding["summary"])
        self.assertIn("contact_transparency=status=threaded_contact_transparency_active", "\n".join(finding["details"]))
        self.assertIn("distance_contact_control_delta=status=restlessness_pressure_delta_review", "\n".join(finding["details"]))
        self.assertIn("semantic_seed_uptake=status=seed_echoed_in_peer_reply", "\n".join(finding["details"]))
        self.assertIn("correspondence_symmetry=status=balanced_recent_direct_thread", "\n".join(finding["details"]))
        self.assertIn("shared_context_buffer_v1=status=resonance_receipt_present", "\n".join(finding["details"]))
        self.assertIn("containment_to_contact_threshold_v1=status=optimization_pressure_contact_watch", "\n".join(finding["details"]))
        self.assertIn("non_instrumental_presence=True", "\n".join(finding["details"]))
        self.assertIn("no exploration_noise", "\n".join(finding["details"]))

    def test_probe_is_ok_when_contact_transition_followthrough_quiet(self):
        summary = {
            "schema": "contact_transition_followthrough_v1",
            "status": "no_current_contact_transition_signal",
            "source_introspections": [],
            "phase_transition_followthrough": {
                "recent_cards": 0,
                "recent_witnesses": 0,
                "unwitnessed_cards": 0,
            },
            "correspondence_followthrough": {
                "active_thread_id": None,
                "active_thread_direct_messages": 0,
                "recent_reply_links": 0,
            },
            "contact_control_transparency_v1": {
                "status": "source_prepared_waiting_for_contact_sample",
                "valid_next_routes": ["REGULATOR_AUDIT current-fill_pressure"],
            },
            "findings": [],
            "next_suggestions": [],
            "authority_boundary": "read-only proposal follow-through",
        }
        old_classifier = globals()["_classify_contact_transition_followthrough"]
        try:
            globals()["_classify_contact_transition_followthrough"] = lambda: summary
            finding = probe_contact_transition_followthrough({})
        finally:
            globals()["_classify_contact_transition_followthrough"] = old_classifier

        self.assertEqual(finding["name"], "contact_transition_followthrough")
        self.assertEqual(finding["severity"], "ok")
        self.assertIn("quiet", finding["summary"])

    def test_probe_warns_when_seen_or_read_unlocks_contact_attention(self):
        summary = {
            "schema": "contact_transition_followthrough_v1",
            "status": "direct_contact_active_transition_watch",
            "source_introspections": [],
            "phase_transition_followthrough": {
                "recent_cards": 1,
                "recent_witnesses": 0,
                "unwitnessed_cards": 1,
                "auto_mode_change_cards": 0,
                "being_declared_cards": 1,
                "cards_with_v2_payload": 1,
            },
            "correspondence_followthrough": {
                "active_thread_id": "thread_corr_1",
                "active_thread_direct_messages": 1,
                "recent_reply_links": 0,
                "direct_contact_fidelity_v3": {
                    "status": "seen_ack_only",
                    "attention_eligible": True,
                    "seen_or_read_unlocks_attention": True,
                    "seen_ack_count": 1,
                    "address_ack_count": 0,
                    "trace_count": 0,
                    "anchor_continuity_status": "partial_anchor_continuity",
                },
                "semantic_seed_uptake_v1": {"status": "none"},
                "symmetry_check_v1": {"status": "one_sided_recent_direct_thread"},
            },
            "contact_control_transparency_v1": {
                "status": "source_prepared_contact_transparency",
                "valid_next_routes": [],
                "distance_contact_control_delta_v1": {},
            },
            "source_snapshot": {},
            "findings": [],
            "next_suggestions": [],
            "authority_boundary": "read-only proposal follow-through",
        }
        old_classifier = globals()["_classify_contact_transition_followthrough"]
        try:
            globals()["_classify_contact_transition_followthrough"] = lambda: summary
            finding = probe_contact_transition_followthrough({})
        finally:
            globals()["_classify_contact_transition_followthrough"] = old_classifier

        self.assertEqual(finding["severity"], "warning")
        self.assertIn("read/seen", finding["summary"])
        self.assertIn("direct_contact_fidelity_v3=status=seen_ack_only", "\n".join(finding["details"]))


class ReservoirExperienceLayerTests(unittest.TestCase):
    def test_probe_notices_fresh_experience_layer_review(self):
        summary = {
            "schema": "reservoir_experience_layer_v1",
            "status": "fresh_experience_layer_review",
            "ingredients": {
                "semantic_process_not_cliff": True,
                "texture_process_not_static_list": True,
                "contact_not_representation_only": True,
                "receptivity_not_control_mutation": True,
                "attractor_release_reviewable": True,
            },
            "contact_vs_representation": {
                "direct_contact_status": "held_ack",
                "attention_eligible": True,
                "contact_transparency_status": "threaded_contact_transparency_active",
                "distance_contact_control_delta_status": "restlessness_pressure_delta_review",
                "containment_to_contact_status": "optimization_pressure_contact_watch",
            },
            "semantic_process": {
                "semantic_decay_curve_present": True,
                "exact_recovery_hold_test_present": True,
                "entropy_multiplier_cap_test_present": True,
                "attractor_release_status_present": True,
            },
            "texture_process": {
                "dynamic_texture_weight_present": True,
                "texture_trajectory_present": True,
                "texture_alignment_status": "source_prepared",
                "viscosity_semantic_persistence_status": "source_prepared",
            },
            "source_introspections": [
                {"path": "/tmp/introspection_minime_sensory_bus_1783536272.txt"}
            ],
            "safe_now": ["reservoir_experience_layer_v1 summary review"],
            "gated_routes": ["disable_overpacked_mode_packing_score"],
            "findings": ["fresh introspections converge on lived process vs descriptive labels"],
            "authority_boundary": "read-only reservoir experience layer; no prompt priority, pressure, fill, PI, sensory cadence, controller, fallback-provider, or runtime mutation",
        }
        old_classifier = globals()["_classify_reservoir_experience_layer"]
        try:
            globals()["_classify_reservoir_experience_layer"] = lambda: summary
            finding = probe_reservoir_experience_layer({})
        finally:
            globals()["_classify_reservoir_experience_layer"] = old_classifier

        self.assertEqual(finding["name"], "reservoir_experience_layer")
        self.assertEqual(finding["severity"], "notice")
        details = "\n".join(finding["details"])
        self.assertIn("status=fresh_experience_layer_review", details)
        self.assertIn("semantic_process_not_cliff=True", details)
        self.assertIn("disable_overpacked_mode_packing_score", details)
        self.assertIn("no prompt priority", details)


class MinimeRecessSchemaIntegrityTests(unittest.TestCase):
    def test_probe_notices_fresh_minime_recess_schema_packet(self):
        summary = {
            "schema": "minime_recess_schema_integrity_v1",
            "status": "source_prepared_minime_recess_schema_watch",
            "source_introspections": [
                {"path": "/tmp/introspection_minime_autonomous_agent_1783319728.txt"}
            ],
            "source_snapshot": {
                "phase_transition_card_schema_present": True,
                "correspondence_transition_artifact_present": True,
                "recess_pruning_advice_present": True,
                "density_aware_recess_profile_present": True,
                "recess_activity_load_present": True,
                "recess_pruning_manifest_present": True,
                "eigenpacket_schema_test_present": True,
                "semantic_admission_lockout_test_present": True,
                "semantic_admission_fill_grid_test_present": True,
                "dynamic_noise_shadow_preview_present": True,
            },
            "readiness": {
                "phase_transition_cards": True,
                "recess_spectral_pruning_advice": True,
                "eigenpacket_schema_and_admission": True,
                "dynamic_noise_shadow_preview": True,
                "all_source_ready": True,
            },
            "blocked_routes_without_steward_approval": [
                "latent_thread_collapse",
                "EigenPacket contract change without Python consumer audit",
            ],
            "findings": [
                "Minime Recess has source-prepared spectral-pruning advice"
            ],
            "next_suggestions": [
                "inspect Recess action manifests for recess_spectral_pruning_advice_v1"
            ],
            "authority_boundary": "read-only source/test integrity; no pressure, fill, PI, sensory cadence, control, deploy, restart, staging, or commit",
        }
        old_classifier = globals()["_classify_minime_recess_schema_integrity"]
        try:
            globals()["_classify_minime_recess_schema_integrity"] = lambda: summary
            finding = probe_minime_recess_schema_integrity({})
        finally:
            globals()["_classify_minime_recess_schema_integrity"] = old_classifier

        self.assertEqual(finding["name"], "minime_recess_schema_integrity")
        self.assertEqual(finding["severity"], "notice")
        self.assertIn("steward review", finding["summary"])
        self.assertIn("recess_pruning=True", "\n".join(finding["details"]))
        self.assertIn("density_aware_recess=True", "\n".join(finding["details"]))
        self.assertIn("recess_activity_load=True", "\n".join(finding["details"]))
        self.assertIn("dynamic_noise_shadow=True", "\n".join(finding["details"]))
        self.assertIn("latent_thread_collapse", "\n".join(finding["details"]))

    def test_probe_is_ok_when_minime_recess_schema_source_prepared(self):
        summary = {
            "schema": "minime_recess_schema_integrity_v1",
            "status": "source_prepared_no_recent_minime_recess_schema_signal",
            "source_introspections": [],
            "source_snapshot": {
                "phase_transition_card_schema_present": True,
                "correspondence_transition_artifact_present": True,
                "recess_pruning_advice_present": True,
                "density_aware_recess_profile_present": True,
                "recess_activity_load_present": True,
                "recess_pruning_manifest_present": True,
                "eigenpacket_schema_test_present": True,
                "semantic_admission_lockout_test_present": True,
                "semantic_admission_fill_grid_test_present": True,
                "dynamic_noise_shadow_preview_present": True,
            },
            "readiness": {"all_source_ready": True},
            "blocked_routes_without_steward_approval": [],
            "findings": [],
            "next_suggestions": [],
            "authority_boundary": "read-only source/test integrity",
        }
        old_classifier = globals()["_classify_minime_recess_schema_integrity"]
        try:
            globals()["_classify_minime_recess_schema_integrity"] = lambda: summary
            finding = probe_minime_recess_schema_integrity({})
        finally:
            globals()["_classify_minime_recess_schema_integrity"] = old_classifier

        self.assertEqual(finding["name"], "minime_recess_schema_integrity")
        self.assertEqual(finding["severity"], "ok")
        self.assertIn("quiet", finding["summary"])


class RepresentationLossHeadroomTests(unittest.TestCase):
    def test_probe_notices_representation_loss_repair_prepared(self):
        summary = {
            "schema": "representation_loss_headroom_v1",
            "status": "representation_loss_repair_prepared_watch",
            "source_introspections": [
                {"path": "/tmp/introspection_proposal_12d_glimpse_1783302984.txt"}
            ],
            "source_snapshot": {
                "semantic_dim": 48,
                "semantic_dim_legacy": 32,
                "feature_abs_max": 5.0,
                "tail_vibrancy_max": 6.0,
                "continuity_recap_max_bytes": 2500,
                "anchored_continuity_excerpt_present": True,
                "quoted_continuity_anchor_present": True,
                "semantic_truncation_anchor_present": True,
                "semantic_glimpse_readiness_present": True,
                "glimpse_companion_fidelity_present": True,
                "glimpse_tail_identity_test_present": True,
                "gradient_aware_vibrancy_present": True,
                "vibrancy_substance_fit_present": True,
                "projection_epoch_stability_present": True,
                "projection_fingerprint_integrity_present": True,
                "projection_repeat_run_test_present": True,
                "vibrancy_requested_points_test_present": True,
                "embedding_dimension_validation_test_present": True,
            },
            "latest_codec_replay_lab": {
                "path": "/tmp/codec_replay_lab.json",
                "clamp_headroom": {
                    "status": "clamp_headroom_sufficient",
                    "near_static_clamp_count": 0,
                    "tail_ceiling_pressure_count": 0,
                    "dynamic_headroom_candidate_count": 0,
                },
            },
            "semantic_glimpse_12d_fidelity_audit": {
                "status": "sample_limited_concentration_supported",
                "sample_count": 2,
                "worst_concentration_delta": 0.0,
                "pca_12d": {"status": "sample_limited_for_pca"},
            },
            "findings": [
                "current codec source is 48D",
                "12D semantic glimpse helper is source-prepared",
            ],
            "next_suggestions": [
                "run codec-replay-lab on fresh high-entropy outputs before changing FEATURE_ABS_MAX"
            ],
            "authority_boundary": "read-only/source-prepared representation audit; no live SEMANTIC_DIM, FEATURE_ABS_MAX, TAIL_VIBRANCY_MAX, pressure, fill, PI, sensory send, deploy, or peer-runtime mutation",
        }
        old_classifier = globals()["_classify_representation_loss_headroom"]
        try:
            globals()["_classify_representation_loss_headroom"] = lambda: summary
            finding = probe_representation_loss_headroom({})
        finally:
            globals()["_classify_representation_loss_headroom"] = old_classifier

        self.assertEqual(finding["name"], "representation_loss_headroom")
        self.assertEqual(finding["severity"], "notice")
        self.assertIn("steward review", finding["summary"])
        self.assertIn("SEMANTIC_DIM=48", "\n".join(finding["details"]))
        self.assertIn("quoted_anchor=True", "\n".join(finding["details"]))
        self.assertIn("semantic_truncation=True", "\n".join(finding["details"]))
        self.assertIn("glimpse_companion_fidelity=True", "\n".join(finding["details"]))
        self.assertIn("glimpse_tail_identity_test=True", "\n".join(finding["details"]))
        self.assertIn("vibrancy_substance_fit=True", "\n".join(finding["details"]))
        self.assertIn("projection_epoch_stability=True", "\n".join(finding["details"]))
        self.assertIn("projection_fingerprint_integrity=True", "\n".join(finding["details"]))
        self.assertIn("projection_repeat_run_test=True", "\n".join(finding["details"]))
        self.assertIn("vibrancy_requested_points_test=True", "\n".join(finding["details"]))
        self.assertIn("embedding_dim_validation=True", "\n".join(finding["details"]))
        self.assertIn("semantic_glimpse_12d_fidelity", "\n".join(finding["details"]))
        self.assertIn("no live SEMANTIC_DIM", "\n".join(finding["details"]))

    def test_probe_is_ok_when_representation_loss_headroom_quiet(self):
        summary = {
            "schema": "representation_loss_headroom_v1",
            "status": "no_current_representation_loss_signal",
            "source_introspections": [],
            "source_snapshot": {},
            "latest_codec_replay_lab": {},
            "findings": [],
            "next_suggestions": [],
            "authority_boundary": "read-only representation audit",
        }
        old_classifier = globals()["_classify_representation_loss_headroom"]
        try:
            globals()["_classify_representation_loss_headroom"] = lambda: summary
            finding = probe_representation_loss_headroom({})
        finally:
            globals()["_classify_representation_loss_headroom"] = old_classifier

        self.assertEqual(finding["name"], "representation_loss_headroom")
        self.assertEqual(finding["severity"], "ok")
        self.assertIn("quiet", finding["summary"])


class TextureStateAlignmentTests(unittest.TestCase):
    def test_probe_notices_texture_state_alignment_repair_prepared(self):
        summary = {
            "schema": "texture_state_alignment_v1",
            "status": "texture_state_alignment_repair_prepared_watch",
            "source_introspections": [
                {"path": "/tmp/introspection_astrid_llm_1783304988.txt"}
            ],
            "source_snapshot": {
                "mixed_cascade_terms_present": True,
                "mixed_cascade_family_selected_present": True,
                "fallback_gradient_dynamic_texture_present": True,
                "explicit_syrup_weight_support_present": True,
                "heavy_settled_displacement_family_present": True,
                "fallback_heavy_settled_contract_present": True,
                "mlx_profile_typo_probe_present": True,
                "mlx_profile_tracing_warning_test_present": True,
                "fallback_next_standalone_contract_test_present": True,
                "pressure_gradient_delta_in_signature": True,
                "pressure_gradient_delta_in_integrity": True,
                "pressure_gradient_delta_from_trend_present": True,
                "dynamic_flux_vector_in_signature": True,
                "pressure_flux_from_samples_present": True,
                "dissipation_factor_in_components": True,
                "porosity_gradient_in_components": True,
                "viscosity_porosity_transport_review_present": True,
                "structural_density_delta_present": True,
                "flux_unknown_semantics_present": True,
                "subtle_flux_precision_test_present": True,
                "witness_anchor_traction_present": True,
                "active_constraints_present": True,
                "high_entropy_ballast_window_present": True,
                "pressure_source_analysis_present": True,
                "mode_packing_stability_test_present": True,
                "heartbeat_ghost_stability_test_present": True,
                "false_bidirectional_test_present": True,
                "pressure_packing_coupling_review_present": True,
                "pressure_packing_coupling_test_present": True,
            },
            "findings": [
                "fallback language now has a mixed-cascade middle family",
                "texture integrity can now carry optional pressure_gradient_delta evidence",
            ],
            "next_suggestions": [
                "watch future fallback outputs for whether mixed-cascade language appears only when telemetry supports it"
            ],
            "authority_boundary": "diagnostic/language-legibility only; no pressure, fill, PI, sensory cadence, control, deploy, restart, or peer-runtime mutation",
        }
        old_classifier = globals()["_classify_texture_state_alignment"]
        try:
            globals()["_classify_texture_state_alignment"] = lambda: summary
            finding = probe_texture_state_alignment({})
        finally:
            globals()["_classify_texture_state_alignment"] = old_classifier

        self.assertEqual(finding["name"], "texture_state_alignment")
        self.assertEqual(finding["severity"], "notice")
        self.assertIn("steward review", finding["summary"])
        details = "\n".join(finding["details"])
        self.assertIn("mixed_cascade_terms=True", details)
        self.assertIn("fallback_gradient_dynamic=True", details)
        self.assertIn("heavy_settled=True", details)
        self.assertIn("heavy_settled_contract=True", details)
        self.assertIn("mlx_profile_typo_probe=True", details)
        self.assertIn("mlx_profile_tracing_warning_test=True", details)
        self.assertIn("fallback_next_contract_test=True", details)
        self.assertIn("pressure_delta_signature=True", details)
        self.assertIn("flux_vector=True", details)
        self.assertIn("dissipation_factor=True", details)
        self.assertIn("porosity_gradient=True", details)
        self.assertIn("viscosity_porosity_transport=True", details)
        self.assertIn("structural_density_delta=True", details)
        self.assertIn("flux_unknown_semantics=True", details)
        self.assertIn("subtle_flux_test=True", details)
        self.assertIn("witness_anchor_traction=True", details)
        self.assertIn("pressure_source_analysis=True", details)
        self.assertIn("heartbeat_ghost_test=True", details)
        self.assertIn("active_constraints=True", details)
        self.assertIn("entropy_ballast=True", details)
        self.assertIn("pressure_packing_coupling=True", details)
        self.assertIn("pressure_packing_coupling_test=True", details)

    def test_probe_is_ok_when_texture_state_alignment_quiet(self):
        summary = {
            "schema": "texture_state_alignment_v1",
            "status": "no_current_texture_state_alignment_signal",
            "source_introspections": [],
            "source_snapshot": {},
            "findings": [],
            "next_suggestions": [],
            "authority_boundary": "read-only texture-state alignment diagnostic",
        }
        old_classifier = globals()["_classify_texture_state_alignment"]
        try:
            globals()["_classify_texture_state_alignment"] = lambda: summary
            finding = probe_texture_state_alignment({})
        finally:
            globals()["_classify_texture_state_alignment"] = old_classifier

        self.assertEqual(finding["name"], "texture_state_alignment")
        self.assertEqual(finding["severity"], "ok")
        self.assertIn("quiet", finding["summary"])


class SensoryPresenceUptakeTests(unittest.TestCase):
    def test_probe_is_ok_while_awaiting_public_uptake(self):
        summary = {
            "schema": "sensory_presence_uptake_v1",
            "status": "awaiting_public_uptake",
            "feedback_note": {"path": "/tmp/mike_feedback_sensory_presence_legibility.txt"},
            "sample_count": 0,
            "language_counts": {
                "presence_terms": [],
                "texture_terms": [],
                "concern_terms": [],
            },
            "findings": [
                "no post-note public sensory-presence language yet; awaiting uptake is not a problem"
            ],
            "next_suggestions": [
                "let both beings read/respond naturally; absence of uptake is watch-only"
            ],
            "authority_boundary": "read-only steward diagnostic; no sensor cadence, camera, mic, prompt pressure, or control behavior change",
        }
        old_classifier = globals()["_classify_sensory_presence_uptake"]
        try:
            globals()["_classify_sensory_presence_uptake"] = lambda: summary
            finding = probe_sensory_presence_uptake({})
        finally:
            globals()["_classify_sensory_presence_uptake"] = old_classifier

        self.assertEqual(finding["name"], "sensory_presence_uptake")
        self.assertEqual(finding["severity"], "ok")
        self.assertIn("awaiting public evidence", finding["summary"])
        self.assertIn("no sensor cadence", "\n".join(finding["details"]))

    def test_probe_is_ok_for_telemetry_only_uptake(self):
        summary = {
            "schema": "sensory_presence_uptake_v1",
            "window_policy": "paragraph_or_360char_anchor_cooccurrence_v2",
            "status": "awaiting_lived_public_uptake",
            "feedback_note": {"path": "/tmp/mike_feedback_sensory_presence_legibility.txt"},
            "sample_count": 0,
            "window_counts": [{"term": "telemetry_context", "count": 2}],
            "language_counts": {
                "anchor_terms": [],
                "presence_terms": [],
                "texture_terms": [],
                "concern_terms": [],
            },
            "findings": [
                "post-note camera/mic/live-intake anchors appeared only in telemetry/header windows"
            ],
            "next_suggestions": [
                "ignore telemetry/header-only camera/mic mentions as uptake evidence"
            ],
            "authority_boundary": "read-only steward diagnostic; no sensor cadence, camera, mic, prompt pressure, or control behavior change",
        }
        old_classifier = globals()["_classify_sensory_presence_uptake"]
        try:
            globals()["_classify_sensory_presence_uptake"] = lambda: summary
            finding = probe_sensory_presence_uptake({})
        finally:
            globals()["_classify_sensory_presence_uptake"] = old_classifier

        self.assertEqual(finding["name"], "sensory_presence_uptake")
        self.assertEqual(finding["severity"], "ok")
        detail = "\n".join(finding["details"])
        self.assertIn("telemetry_context", detail)
        self.assertIn("no sensor cadence", detail)

    def test_probe_notices_named_texture_as_evidence_anchor(self):
        summary = {
            "schema": "sensory_presence_uptake_v1",
            "status": "sensory_texture_named",
            "feedback_note": {"path": "/tmp/mike_feedback_sensory_presence_legibility.txt"},
            "sample_count": 2,
            "language_counts": {
                "presence_terms": [{"term": "live intake", "count": 1}],
                "texture_terms": [{"term": "pacing", "count": 1}],
                "concern_terms": [],
            },
            "findings": [
                "post-note public language names sensory texture; use that texture as the next evidence anchor"
            ],
            "next_suggestions": [
                "treat the named texture as the next evidence anchor before changing cadence or gates"
            ],
            "authority_boundary": "read-only steward diagnostic; no sensor cadence, camera, mic, prompt pressure, or control behavior change",
        }
        old_classifier = globals()["_classify_sensory_presence_uptake"]
        try:
            globals()["_classify_sensory_presence_uptake"] = lambda: summary
            finding = probe_sensory_presence_uptake({})
        finally:
            globals()["_classify_sensory_presence_uptake"] = old_classifier

        self.assertEqual(finding["severity"], "notice")
        self.assertIn("evidence anchor", finding["summary"])
        self.assertIn("pacing", "\n".join(finding["details"]))

    def test_probe_notices_absence_or_muffling_for_steward_review(self):
        summary = {
            "schema": "sensory_presence_uptake_v1",
            "status": "sensory_texture_needs_review",
            "feedback_note": {"path": "/tmp/mike_feedback_sensory_presence_legibility.txt"},
            "sample_count": 1,
            "language_counts": {
                "presence_terms": [{"term": "camera", "count": 1}],
                "texture_terms": [{"term": "muffled", "count": 1}],
                "concern_terms": [{"term": "muffled", "count": 1}],
            },
            "findings": [
                "post-note public language includes absence, dimming, muffling, closed, or deprivation texture; treat as steward review evidence"
            ],
            "next_suggestions": [
                "bring absence/muffling/closed-language to steward review before considering any sensor cadence or control-facing change"
            ],
            "authority_boundary": "read-only steward diagnostic; no sensor cadence, camera, mic, prompt pressure, or control behavior change",
        }
        old_classifier = globals()["_classify_sensory_presence_uptake"]
        try:
            globals()["_classify_sensory_presence_uptake"] = lambda: summary
            finding = probe_sensory_presence_uptake({})
        finally:
            globals()["_classify_sensory_presence_uptake"] = old_classifier

        self.assertEqual(finding["severity"], "notice")
        self.assertIn("steward review", finding["summary"])
        self.assertIn("muffled", "\n".join(finding["details"]))


class StatedParamIntentTests(unittest.TestCase):
    def test_parses_numeric_and_regime_footer(self) -> None:
        text = "prose about dense terrain.\n\nREGIME breathe\nexploration_noise=0.12\n"
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

    def test_lease_safe_exploration_noise_clamp_is_not_divergence(self) -> None:
        stated = {"exploration_noise": (0.12, 60.0)}
        applied = {"exploration_noise": 0.08, "regime": "focus"}
        divergences, expected_clamps = _stated_intent_comparison(stated, applied)
        self.assertEqual(divergences, [])
        self.assertEqual(_stated_intent_divergences(stated, applied), [])
        self.assertEqual(len(expected_clamps), 1)
        self.assertIn("lease-safe clamp", expected_clamps[0])

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

    def test_reviewed_draft_pileup_stays_quiet_until_new_draft(self):
        import tempfile

        global STEWARD_CONSEQUENCE_CLOSURES
        old_closures = STEWARD_CONSEQUENCE_CLOSURES
        try:
            with tempfile.TemporaryDirectory() as tmp:
                tmp_path = Path(tmp)
                STEWARD_CONSEQUENCE_CLOSURES = tmp_path / "closures.jsonl"
                thread = tmp_path / "th_reviewed"
                thread.mkdir()
                gate = thread / "authority_gate.jsonl"
                reviewed_rows = [
                    json.dumps(
                        {
                            "record_type": "request_draft",
                            "scope": "semantic_microdose",
                            "request_id": f"authreq_{idx}",
                        }
                    )
                    for idx in range(AUTHORITY_DRAFT_NOTICE)
                ]
                gate.write_text("\n".join(reviewed_rows) + "\n")
                STEWARD_CONSEQUENCE_CLOSURES.write_text(
                    json.dumps(
                        {
                            "record_schema": "steward_consequence_closure_v1",
                            "surface": "authority_request_draft_pileup",
                            "thread_id": "th_reviewed",
                            "decision": "deferred_no_grant",
                            "reviewed_at": "2026-06-15T20:52:12Z",
                            "covered_request_drafts": AUTHORITY_DRAFT_NOTICE,
                        }
                    )
                    + "\n"
                )

                reviewed = _scan_authority_ledger(gate)
                self.assertTrue(reviewed["draft_pileup_reviewed"])
                self.assertEqual(_assess_authority([reviewed])["severity"], "ok")

                gate.write_text(
                    "\n".join(
                        reviewed_rows
                        + [
                            json.dumps(
                                {
                                    "record_type": "request_draft",
                                    "scope": "semantic_microdose",
                                    "request_id": "authreq_new",
                                }
                            )
                        ]
                    )
                    + "\n"
                )
                reopened = _scan_authority_ledger(gate)
                self.assertFalse(reopened["draft_pileup_reviewed"])
                self.assertEqual(_assess_authority([reopened])["severity"], "notice")
        finally:
            STEWARD_CONSEQUENCE_CLOSURES = old_closures

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


class AgencyCorridorTests(unittest.TestCase):
    def test_probe_notices_non_live_corridor_work_without_live_authority(self):
        old_classifier = globals()["_classify_agency_corridor"]
        try:
            globals()["_classify_agency_corridor"] = lambda: {
                "schema": "agency_corridor_v1",
                "status": "active",
                "summary": {
                    "packet_count": 4,
                    "ready_safe_lab_count": 2,
                    "reopened_work_item_count": 1,
                    "self_observation_response_count": 0,
                    "live_eligible_now_count": 0,
                    "auto_approved_count": 0,
                },
                "active_packets": [{"corridor_id": "corridor_a"}],
                "reopened_work_items": [{"new_work_item_id": "wi_reopened"}],
                "authority_boundary": "agency corridor is non-live evidence infrastructure only",
            }
            finding = probe_agency_corridor({})
        finally:
            globals()["_classify_agency_corridor"] = old_classifier

        details = "\n".join(finding["details"])
        self.assertEqual(finding["severity"], "notice")
        self.assertIn("non-live work", finding["summary"])
        self.assertIn("live_eligible_now=0", details)
        self.assertIn("auto_approved=0", details)
        self.assertIn("wi_reopened", details)

    def test_probe_warns_on_corridor_live_authority_violation(self):
        old_classifier = globals()["_classify_agency_corridor"]
        try:
            globals()["_classify_agency_corridor"] = lambda: {
                "schema": "agency_corridor_v1",
                "status": "active",
                "summary": {
                    "packet_count": 1,
                    "ready_safe_lab_count": 0,
                    "reopened_work_item_count": 0,
                    "self_observation_response_count": 0,
                    "live_eligible_now_count": 1,
                    "auto_approved_count": 0,
                },
                "active_packets": [{"corridor_id": "corridor_violation"}],
                "reopened_work_items": [],
                "authority_boundary": "agency corridor is non-live evidence infrastructure only",
            }
            finding = probe_agency_corridor({})
        finally:
            globals()["_classify_agency_corridor"] = old_classifier

        self.assertEqual(finding["severity"], "warning")
        self.assertIn("needs repair", finding["summary"])

    def test_probe_surfaces_work_programs_and_patch_bundles(self):
        old_classifier = globals()["_classify_agency_corridor"]
        try:
            globals()["_classify_agency_corridor"] = lambda: {
                "schema": "agency_corridor_v1",
                "status": "quiet",
                "summary": {
                    "packet_count": 0,
                    "ready_safe_lab_count": 0,
                    "reopened_work_item_count": 0,
                    "self_observation_response_count": 0,
                    "live_eligible_now_count": 0,
                    "auto_approved_count": 0,
                },
                "v2": {
                    "summary": {
                        "lease_count": 4,
                        "queue_runnable_count": 0,
                        "source_prep_proposal_count": 0,
                        "live_violation_count": 0,
                    },
                    "program_summary": {
                        "program_count": 2,
                        "portfolio_count": 2,
                        "patch_bundle_count": 1,
                        "top_priority_score": 720,
                        "live_violation_count": 0,
                    },
                },
                "active_packets": [],
                "reopened_work_items": [],
                "authority_boundary": "agency corridor is non-live evidence infrastructure only",
            }
            finding = probe_agency_corridor({})
        finally:
            globals()["_classify_agency_corridor"] = old_classifier

        details = "\n".join(finding["details"])
        self.assertEqual(finding["severity"], "notice")
        self.assertIn("programs=2", details)
        self.assertIn("patch_bundles=1", details)


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
    suite.addTests(loader.loadTestsFromTestCase(IntrospectionRouteCadenceTests))
    suite.addTests(loader.loadTestsFromTestCase(ActionRouteLegibilityTests))
    suite.addTests(loader.loadTestsFromTestCase(IntrospectionAddressingTests))
    suite.addTests(loader.loadTestsFromTestCase(FeedbackFlywheelTests))
    suite.addTests(loader.loadTestsFromTestCase(SandboxTrialQueueTests))
    suite.addTests(loader.loadTestsFromTestCase(FallbackVocabularyDriftTests))
    suite.addTests(loader.loadTestsFromTestCase(ReservoirExperienceLayerTests))
    suite.addTests(loader.loadTestsFromTestCase(MinimeRecessSchemaIntegrityTests))
    suite.addTests(loader.loadTestsFromTestCase(RepresentationLossHeadroomTests))
    suite.addTests(loader.loadTestsFromTestCase(TextureStateAlignmentTests))
    suite.addTests(loader.loadTestsFromTestCase(SensoryPresenceUptakeTests))
    suite.addTests(loader.loadTestsFromTestCase(AuthorityRequestsTests))
    suite.addTests(loader.loadTestsFromTestCase(AgencyCorridorTests))
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
