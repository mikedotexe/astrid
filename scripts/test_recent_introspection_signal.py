#!/usr/bin/env python3
"""Tests for scripts/recent_introspection_signal.py."""

from __future__ import annotations

import importlib.util
import json
import os
import sys
import tempfile
import unittest
from pathlib import Path


SCRIPT = Path(__file__).resolve().with_name("recent_introspection_signal.py")
SPEC = importlib.util.spec_from_file_location("recent_introspection_signal", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
recent_introspection_signal = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = recent_introspection_signal
SPEC.loader.exec_module(recent_introspection_signal)


def write(path: Path, text: str, *, mtime: float | None = None) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text)
    if mtime is not None:
        os.utime(path, (mtime, mtime))


class RecentIntrospectionSignalTests(unittest.TestCase):
    def test_bounded_scan_uses_known_shallow_lanes_only(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            astrid_ws = root / "astrid_ws"
            minime_ws = root / "minime_ws"
            write(
                astrid_ws / "diagnostics/introspection_feedback_digest/latest.md",
                "PRESSURE_RELIEF surfaced again near fill pressure.",
                mtime=100.0,
            )
            write(
                minime_ws / "actions/entry_self_study.json",
                json.dumps({"text": "unknown NEXT fallthrough means a muffled action"}),
                mtime=101.0,
            )
            write(
                minime_ws / "actions/deep/hidden_introspect.json",
                "PRESSURE_RELIEF should not be found by a shallow scan",
                mtime=102.0,
            )

            report = recent_introspection_signal.build_signal(
                astrid_ws,
                minime_ws,
                limit=10,
                per_location_limit=10,
            )

            paths = [entry["path"] for entry in report["sources"]]
            self.assertEqual(report["source_count"], 2)
            self.assertFalse(any("deep/hidden" in path for path in paths))
            self.assertGreater(report["signal_counts"]["pressure_relief"], 0)
            self.assertGreater(report["signal_counts"]["muffled_action"], 0)
            self.assertIn("coverage guard", " ".join(report["suggested_next"]))

    def test_recent_order_and_limit_are_deterministic(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            astrid_ws = root / "astrid_ws"
            minime_ws = root / "minime_ws"
            write(astrid_ws / "introspections/controller_astrid:autonomous_1.json", "old rewrite_seconds", mtime=1.0)
            write(astrid_ws / "introspections/controller_astrid:autonomous_2.json", "newer rewrite cap", mtime=2.0)
            write(minime_ws / "introspections/note.txt", "newest triadic chamber CHAMBER_SEEN", mtime=3.0)

            report = recent_introspection_signal.build_signal(
                astrid_ws,
                minime_ws,
                limit=2,
                per_location_limit=10,
            )

            self.assertEqual(report["source_count"], 2)
            self.assertTrue(report["sources"][0]["path"].endswith("note.txt"))
            self.assertTrue(report["sources"][1]["path"].endswith("controller_astrid:autonomous_2.json"))
            self.assertEqual(report["signal_counts"].get("rewrite_latency"), 1)

    def test_render_markdown_names_bounded_policy(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            astrid_ws = root / "astrid_ws"
            minime_ws = root / "minime_ws"
            write(
                minime_ws / "inbox/read/astrid_self_study_1.txt",
                "continuity_deficit and re-entry memory need care",
            )

            report = recent_introspection_signal.build_signal(astrid_ws, minime_ws)
            rendered = recent_introspection_signal.render_markdown(report)

            self.assertIn("Read-only bounded diagnostic context", rendered)
            self.assertIn("continuity_deficit", rendered)
            self.assertIn("Per-file read cap", rendered)


if __name__ == "__main__":
    unittest.main()
