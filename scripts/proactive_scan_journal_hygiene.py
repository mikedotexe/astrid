#!/usr/bin/env python3
"""Journal-hygiene probe for proactive_scan.py."""

from __future__ import annotations

import os
import sys
import time
import unittest
from pathlib import Path
from tempfile import TemporaryDirectory
from typing import Any


MINIME_REPO = Path("/Users/v/other/minime")
MINIME_JOURNAL = MINIME_REPO / "workspace/journal"

if str(MINIME_REPO) not in sys.path:
    sys.path.insert(0, str(MINIME_REPO))

try:
    from journal_hygiene import scan_journal_directory
except Exception:  # pragma: no cover - defensive for machines without Minime checkout
    scan_journal_directory = None


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


def probe_journal_hygiene(prior: dict[str, Any]) -> dict[str, Any]:
    """Detect machine-contract contamination and loop-like operational dominance."""
    if scan_journal_directory is None:
        return _finding(
            "journal_hygiene",
            "notice",
            "journal hygiene helper unavailable",
            snapshot={"available": False},
        )
    report = scan_journal_directory(MINIME_JOURNAL)
    counts = report.get("counts") or {}
    signals = list(report.get("signals") or [])
    status = str(report.get("status") or "ok")
    prior_signals = set(prior.get("signals") or []) if isinstance(prior, dict) else set()
    prior_counts = prior.get("counts") if isinstance(prior, dict) else {}
    if not isinstance(prior_counts, dict):
        prior_counts = {}
    current_ratio = float(report.get("operational_ratio") or 0.0)
    prior_ratio = (
        float(prior.get("operational_ratio") or 0.0)
        if isinstance(prior, dict)
        else 0.0
    )
    trend_signals: list[str] = []
    if (
        "operational_dominance" in signals
        and prior_ratio > current_ratio
        and prior_ratio - current_ratio >= 0.02
    ):
        trend_signals.append("operational_dominance_recovering")

    actionable_signals: list[str] = []
    if "machine_detail_present" in signals:
        actionable_signals.append("machine_detail_present")
    for signal in ("repeated_loop_present", "operational_dominance"):
        if signal not in signals:
            continue
        if not prior:
            continue
        if signal not in prior_signals:
            actionable_signals.append(signal)
            continue
        if counts.get("operational", 0) > int(prior_counts.get("operational", 0) or 0):
            actionable_signals.append(signal)

    severity = "ok"
    if "machine_detail_present" in actionable_signals:
        severity = "warning"
    elif actionable_signals:
        severity = "notice"

    details: list[str] = []
    if "machine_detail_present" in actionable_signals:
        details.append("machine-contract detail is present in recent Minime journal files")
    if "repeated_loop_present" in actionable_signals:
        details.append("repeated journal/NEXT loop keys found in recent Minime journal files")
    if "operational_dominance" in actionable_signals:
        details.append("operational journal entries dominate the latest Minime sample")
    if "operational_dominance_recovering" in trend_signals:
        details.append(
            f"operational dominance is recovering ({prior_ratio:.2f} -> {current_ratio:.2f})"
        )
    if signals and not actionable_signals and "machine_detail_present" not in signals:
        if not trend_signals:
            details.append("current operational/repeat hygiene signals are accepted baseline; no new actionable drift")

    summary_status = "ok" if severity == "ok" else status
    if severity == "ok" and trend_signals:
        summary_status = "recovering"
    summary = (
        "minime journal hygiene "
        f"{summary_status} ({counts.get('reflective', 0)} reflective, "
        f"{counts.get('operational', 0)} operational, "
        f"{counts.get('machine_detail', 0)} machine detail; "
        f"operational_ratio={current_ratio:.2f})"
    )
    report["actionable_signals"] = actionable_signals
    report["trend_signals"] = trend_signals
    return _finding(
        "journal_hygiene",
        severity,
        summary,
        details=details if details else None,
        snapshot=report,
    )


class JournalHygieneProbeTests(unittest.TestCase):
    def test_warns_on_machine_detail(self) -> None:
        global MINIME_JOURNAL
        old_minime_journal = MINIME_JOURNAL
        try:
            with TemporaryDirectory() as tmpdir:
                journal = Path(tmpdir)
                (journal / "action_preflight_test.txt").write_text(
                    "Dry run: True\n\nJSON:\n{\"policy\":\"action_preflight_v1\"}\n"
                )
                MINIME_JOURNAL = journal
                finding = probe_journal_hygiene({})
        finally:
            MINIME_JOURNAL = old_minime_journal

        self.assertEqual(finding["severity"], "warning")
        self.assertEqual(finding["snapshot"]["counts"]["machine_detail"], 1)

    def test_notices_repeated_next_loop(self) -> None:
        global MINIME_JOURNAL
        old_minime_journal = MINIME_JOURNAL
        try:
            with TemporaryDirectory() as tmpdir:
                journal = Path(tmpdir)
                for idx in range(4):
                    path = journal / f"action_thread_{idx}.txt"
                    path.write_text(
                        "=== ACTION THREAD ===\n"
                        "Suggested NEXT: THREAD_STATUS current\n"
                        "Proposed NEXT: THREAD_STATUS current\n"
                    )
                    ts = time.time() - idx
                    os.utime(path, (ts, ts))
                MINIME_JOURNAL = journal
                finding = probe_journal_hygiene({"signals": [], "counts": {"operational": 0}})
        finally:
            MINIME_JOURNAL = old_minime_journal

        self.assertEqual(finding["severity"], "notice")
        self.assertIn("repeated_loop_present", finding["snapshot"]["signals"])

    def test_reports_recovering_operational_dominance(self) -> None:
        global MINIME_JOURNAL
        old_minime_journal = MINIME_JOURNAL
        try:
            with TemporaryDirectory() as tmpdir:
                journal = Path(tmpdir)
                now = time.time()
                for idx in range(13):
                    path = journal / f"action_thread_{idx}.txt"
                    path.write_text(
                        "=== ACTION THREAD ===\n"
                        f"Suggested NEXT: THREAD_STATUS current {idx}\n"
                    )
                    os.utime(path, (now - idx, now - idx))
                for idx in range(7):
                    path = journal / f"rest_{idx}.txt"
                    path.write_text(
                        "=== REST PHASE REFLECTION ===\n"
                        f"Continuity posture: new\nDelta: native reflection {idx}.\nHold: rest."
                    )
                    os.utime(path, (now - 20 - idx, now - 20 - idx))
                MINIME_JOURNAL = journal
                finding = probe_journal_hygiene({
                    "signals": ["operational_dominance"],
                    "counts": {"operational": 16, "reflective": 4},
                    "operational_ratio": 0.80,
                })
        finally:
            MINIME_JOURNAL = old_minime_journal

        self.assertEqual(finding["severity"], "ok")
        self.assertIn("recovering", finding["summary"])
        self.assertIn("operational_dominance_recovering", finding["snapshot"]["trend_signals"])
