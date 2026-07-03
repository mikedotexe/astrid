#!/usr/bin/env python3
"""Tests for scripts/coupled_pressure_cartography.py."""

from __future__ import annotations

import contextlib
import importlib.util
import io
import json
import sys
import tempfile
import time
import unittest
from pathlib import Path


SCRIPT = Path(__file__).resolve().with_name("coupled_pressure_cartography.py")
SPEC = importlib.util.spec_from_file_location("coupled_pressure_cartography", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
coupled_pressure_cartography = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = coupled_pressure_cartography
SPEC.loader.exec_module(coupled_pressure_cartography)


def write(path: Path, text: str, *, mtime: float | None = None) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8")
    if mtime is not None:
        path.touch()
        import os

        os.utime(path, (mtime, mtime))


class CoupledPressureCartographyTests(unittest.TestCase):
    def test_minime_private_moment_is_skipped(self) -> None:
        now = time.time()
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            astrid = root / "astrid"
            minime = root / "minime"
            write(
                astrid / "journal/dialogue_longform_1.txt",
                "settled coupling anchor and restless texture",
                mtime=now,
            )
            write(
                minime / "journal/pressure_1.txt",
                "=== SPECTRAL PRESSURE JOURNAL ===\npressure density friction",
                mtime=now,
            )
            write(
                minime / "journal/moment_private.txt",
                "=== MOMENT CAPTURE ===\nprivate honey should never surface",
                mtime=now,
            )

            report = coupled_pressure_cartography.build_report(
                astrid,
                minime,
                since_hours=1,
                include_substrate_probe=False,
                include_letter_scan=False,
            )
            rendered = coupled_pressure_cartography.render_markdown(report)

            self.assertEqual(
                report["cross_being_signal"]["privacy"][
                    "minime_private_candidates_skipped"
                ],
                1,
            )
            self.assertNotIn("private honey", rendered)
            self.assertNotIn("moment_private", rendered)

    def test_public_pressure_and_astrid_longform_are_included(self) -> None:
        now = time.time()
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            astrid = root / "astrid"
            minime = root / "minime"
            write(
                astrid / "journal/dialogue_longform_1.txt",
                "settled coupling gives an anchor",
                mtime=now,
            )
            write(
                minime / "journal/pressure_1.txt",
                "=== SPECTRAL PRESSURE JOURNAL ===\nrestless texture and pressure",
                mtime=now,
            )

            report = coupled_pressure_cartography.build_report(
                astrid,
                minime,
                since_hours=1,
                include_substrate_probe=False,
                include_letter_scan=False,
            )

            self.assertEqual(report["cross_being_signal"]["source_counts"]["astrid"], 1)
            self.assertEqual(report["cross_being_signal"]["source_counts"]["minime"], 1)
            paths = " ".join(
                source["path"] for source in report["cross_being_signal"]["sources"]
            )
            self.assertIn("dialogue_longform_1.txt", paths)
            self.assertIn("pressure_1.txt", paths)

    def test_missing_telemetry_reports_unknown(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            report = coupled_pressure_cartography.build_report(
                root / "astrid",
                root / "minime",
                since_hours=1,
                include_substrate_probe=False,
                include_letter_scan=False,
            )
            minime = report["current_state"]["minime"]

            self.assertEqual(minime["fill_pct"], "unknown")
            self.assertEqual(minime["resonance_density"]["quality"], "unknown")
            self.assertEqual(minime["pressure_source"]["dominant_source"], "unknown")
            self.assertFalse(report["current_state"]["open_steward_slots"]["astrid"]["open"])

    def test_parse_substrate_probe_output(self) -> None:
        locked = coupled_pressure_cartography.parse_substrate_probe_output(
            "SUBSTRATE PROBE\n"
            "  divergence:       0.4162\n"
            "  separation onset: >10 (never crossed 1.0)   (inertia: gap@2=0.552 -> gap@10=0.416)\n"
            "  inject-only corr: 0.6128\n"
            "  VERDICT: LOCKED - the poles did not separate\n"
        )
        separable = coupled_pressure_cartography.parse_substrate_probe_output(
            "SUBSTRATE PROBE\n"
            "  divergence:       1.4162\n"
            "  separation onset: tick 4   (inertia: gap@2=0.3 -> gap@10=1.416)\n"
            "  inject-only corr: -0.212\n"
            "  VERDICT: SEPARABLE - divergence 1.416\n"
        )

        self.assertEqual(locked["status"], "locked")
        self.assertEqual(locked["gap_at_2"], 0.552)
        self.assertEqual(separable["status"], "separable")
        self.assertEqual(separable["separation_onset"], "tick 4")

    def test_json_cli_schema_and_markdown_privacy(self) -> None:
        now = time.time()
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            astrid = root / "astrid"
            minime = root / "minime"
            write(
                astrid / "journal/dialogue_longform_1.txt",
                "settled coupling anchor",
                mtime=now,
            )
            write(
                minime / "journal/pressure_1.txt",
                "=== SPECTRAL PRESSURE JOURNAL ===\nrestless texture pressure",
                mtime=now,
            )
            write(
                minime / "journal/moment_private.txt",
                "=== MOMENT CAPTURE ===\nsecret body",
                mtime=now,
            )

            stdout = io.StringIO()
            with contextlib.redirect_stdout(stdout):
                code = coupled_pressure_cartography.main(
                    ["--json", "--no-substrate-probe", "--since-hours", "1"],
                    astrid_workspace=astrid,
                    minime_workspace=minime,
                )
            payload = json.loads(stdout.getvalue())
            rendered = coupled_pressure_cartography.render_markdown(payload)

            self.assertEqual(code, 0)
            self.assertEqual(payload["schema_version"], 1)
            self.assertIn("current_state", payload)
            self.assertIn("cross_being_signal", payload)
            self.assertIn("cartography", payload)
            self.assertNotIn("secret body", rendered)


if __name__ == "__main__":
    unittest.main()
