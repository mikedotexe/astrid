#!/usr/bin/env python3
"""Tests for executable Spectral Bridge domain boundaries."""

from __future__ import annotations

import json
from pathlib import Path
import tempfile
import textwrap
import unittest

try:
    from domain_boundary_audit import audit
except ModuleNotFoundError:
    from scripts.domain_boundary_audit import audit


class DomainBoundaryAuditTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name)
        self.bridge = self.root / "bridge"
        (self.bridge / "src/dispatch").mkdir(parents=True)
        (self.bridge / "src/facade.rs").write_text(
            '#[path = "owner.rs"]\nmod owner;\npub use owner::*;\n',
            encoding="utf-8",
        )
        (self.bridge / "src/owner.rs").write_text("fn owner() {}\n" * 6, encoding="utf-8")
        (self.bridge / "src/legacy.rs").write_text("fn legacy() {}\n" * 11, encoding="utf-8")
        (self.bridge / "src/dispatch/path.rs").write_text("fn dispatch() {}\n", encoding="utf-8")
        (self.bridge / "DOMAIN_BOUNDARIES.md").write_text("`src/legacy.rs`\n", encoding="utf-8")
        (self.bridge / "baseline.json").write_text(
            json.dumps({"files": {"src/legacy.rs": 11}}), encoding="utf-8"
        )
        self.manifest = self.bridge / "boundaries.toml"
        self.manifest.write_text(
            textwrap.dedent(
                """
                schema = "test"
                schema_version = 1
                source_root = "bridge"
                documentation = "bridge/DOMAIN_BOUNDARIES.md"
                legacy_large_file_baseline = "bridge/baseline.json"
                facade_line_limit = 10
                large_file_review_threshold = 10
                test_path_markers = ["/tests.rs"]

                [[stable_facades]]
                path = "src/facade.rs"
                canonical_target = "owner.rs"

                [[cohesion_exceptions]]
                path = "src/legacy.rs"
                maximum_lines = 12
                maximum_unique_fn_signatures = 1

                [[forbidden_edges]]
                edge_id = "interpretation_dispatch"
                from_globs = ["src/dispatch/**/*.rs"]
                forbidden_symbols = ["AstridInterpretationV1"]
                """
            ),
            encoding="utf-8",
        )

    def tearDown(self) -> None:
        self.temp.cleanup()

    def test_declared_facade_and_legacy_ratchet_pass(self) -> None:
        status, large, violations = audit(self.root, self.manifest)
        self.assertTrue(status["valid"])
        self.assertEqual(len(large), 1)
        self.assertFalse(violations)

    def test_exception_signature_growth_fails_within_line_ceiling(self) -> None:
        (self.bridge / "src/legacy.rs").write_text(
            "fn legacy() {}\n" * 10 + "fn extra() {}\n", encoding="utf-8"
        )
        status, _, violations = audit(self.root, self.manifest)
        self.assertFalse(status["valid"])
        self.assertEqual(
            {row["kind"] for row in violations}, {"exception_signature_growth"}
        )
        self.assertFalse(
            status["counter_audit"]["checks"][
                "documented_exception_signature_ceilings_hold"
            ]
        )
        self.assertEqual(
            status["exception_signature_counts"]["src/legacy.rs"],
            {"unique_fn_signatures": 2, "maximum_unique_fn_signatures": 1},
        )

    def test_forbidden_edge_and_new_large_file_fail(self) -> None:
        (self.bridge / "src/dispatch/path.rs").write_text(
            "use crate::AstridInterpretationV1;\n", encoding="utf-8"
        )
        (self.bridge / "src/new_large.rs").write_text("fn new() {}\n" * 11, encoding="utf-8")
        status, _, violations = audit(self.root, self.manifest)
        self.assertFalse(status["valid"])
        self.assertEqual(
            {row["kind"] for row in violations},
            {"forbidden_dependency_edge", "new_large_production_file"},
        )


if __name__ == "__main__":
    unittest.main()
