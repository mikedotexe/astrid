#!/usr/bin/env python3
"""Tests for scripts/astrid_introspection_digest.py."""

from __future__ import annotations

import importlib.util
import json
import sys
import tempfile
import unittest
from pathlib import Path


SCRIPT = Path(__file__).resolve().with_name("astrid_introspection_digest.py")
SPEC = importlib.util.spec_from_file_location("astrid_introspection_digest", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
astrid_introspection_digest = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = astrid_introspection_digest
SPEC.loader.exec_module(astrid_introspection_digest)


def write_entry(root: Path, stamp: int, *, pressure: str, rewrite: float) -> None:
    payload = {
        "controller_regime": "sustain",
        "observer_report": {
            "controller_reason": "regime=sustain; steady",
            "dominant_pressure": pressure,
            "geometry_regime": "warming-up",
            "predicted_top_anchor": "reservoir-memory",
            "rewrite_issue_count": 1,
            "stability_score": 0.92,
        },
        "condition_vector": {
            "severity": 0.08,
            "continuity_deficit": 0.45 if pressure == "continuity_deficit" else 0.1,
            "truncation_pressure": 0.0,
            "structure_strain": 0.25,
        },
        "profiling": {
            "rewrite_seconds": rewrite,
            "candidate_generation_seconds": 40.0,
            "runtime_audit": {
                "generation": {
                    "first_token_seconds": 3.0,
                    "total_turn_seconds": rewrite + 50.0,
                }
            },
        },
    }
    (root / f"controller_astrid:autonomous_{stamp}.json").write_text(json.dumps(payload))


class AstridIntrospectionDigestTests(unittest.TestCase):
    def test_digest_detects_repeated_continuity_and_expensive_rewrite(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            workspace = Path(tmp)
            root = workspace / "introspections"
            root.mkdir()
            write_entry(root, 100, pressure="continuity_deficit", rewrite=150.0)
            write_entry(root, 101, pressure="continuity_deficit", rewrite=180.0)
            write_entry(root, 102, pressure="structure_strain", rewrite=60.0)

            digest = astrid_introspection_digest.build_digest(workspace, limit=3)

            self.assertEqual(digest["summary"]["entry_count"], 3)
            self.assertEqual(digest["summary"]["dominant_pressure"], "continuity_deficit")
            self.assertEqual(digest["summary"]["dominant_pressure_count"], 2)
            self.assertEqual(digest["summary"]["avg_rewrite_seconds"], 130.0)
            self.assertIn("continuity_deficit", " ".join(digest["suggested_next"]))
            self.assertIn("rewrite", " ".join(digest["suggested_next"]))

    def test_write_digest_emits_json_and_markdown(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            workspace = Path(tmp) / "workspace"
            root = workspace / "introspections"
            root.mkdir(parents=True)
            write_entry(root, 200, pressure="continuity_deficit", rewrite=10.0)
            digest = astrid_introspection_digest.build_digest(workspace, limit=1)

            paths = astrid_introspection_digest.write_digest(digest, Path(tmp) / "out")

            self.assertTrue(Path(paths["json"]).is_file())
            self.assertTrue(Path(paths["markdown"]).is_file())
            saved = json.loads(Path(paths["json"]).read_text())
            self.assertEqual(saved["artifacts"], paths)
            self.assertIn("Astrid Introspection Feedback Digest", Path(paths["markdown"]).read_text())


if __name__ == "__main__":
    unittest.main()
