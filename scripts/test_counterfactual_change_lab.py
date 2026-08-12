#!/usr/bin/env python3
"""Tests for the offline Counterfactual Change Lab V1."""

from __future__ import annotations

import hashlib
import json
from pathlib import Path
import tempfile
import unittest

from scripts.counterfactual_change_lab.projector import (
    INPUT_DIM,
    _canonical_sha256,
    evaluate,
    fixed_legacy_basis,
    orthogonalized_basis,
    project,
)


class CounterfactualChangeLabTest(unittest.TestCase):
    def _inputs(self) -> tuple[dict, dict]:
        corpus = {
            "schema": "codec_basis_public_corpus_v1",
            "schema_version": 1,
            "corpus_id": "test_public_corpus",
            "private_material_included": False,
            "entries": [
                {"entry_id": "a", "text": "alpha"},
                {"entry_id": "b", "text": "beta"},
                {"entry_id": "c", "text": "repeat"},
                {"entry_id": "d", "text": "repeat"},
            ],
            "contrast_pairs": [["a", "b"]],
            "repeat_pairs": [["c", "d"]],
        }
        embeddings = []
        for index, row in enumerate(corpus["entries"]):
            source_index = 2 if index == 3 else index
            values = [((axis + 3) * (source_index + 5) % 29) / 29.0 for axis in range(INPUT_DIM)]
            embeddings.append(
                {
                    "entry_id": row["entry_id"],
                    "text_sha256": hashlib.sha256(row["text"].encode()).hexdigest(),
                    "embedding": values,
                }
            )
        fixture = {
            "schema": "codec_basis_public_embeddings_v1",
            "schema_version": 1,
            "corpus_id": corpus["corpus_id"],
            "corpus_sha256": _canonical_sha256(corpus),
            "model": "synthetic-test",
            "model_digest": "test",
            "embedding_dim_count": INPUT_DIM,
            "private_material_included": False,
            "entries": embeddings,
        }
        return corpus, fixture

    def test_orthogonal_candidate_reduces_legacy_correlation(self) -> None:
        legacy = fixed_legacy_basis()
        candidate = orthogonalized_basis()
        legacy_max = max(
            abs(sum(a * b for a, b in zip(legacy[left], legacy[right], strict=True)))
            for left in range(8)
            for right in range(left + 1, 8)
        )
        candidate_max = max(
            abs(sum(a * b for a, b in zip(candidate[left], candidate[right], strict=True)))
            for left in range(8)
            for right in range(left + 1, 8)
        )
        self.assertGreater(legacy_max, 0.15)
        self.assertLess(candidate_max, 1.0e-12)

    def test_projection_is_offline_and_default_off(self) -> None:
        corpus, fixture = self._inputs()
        status, packet, rows = evaluate(corpus, fixture)
        self.assertTrue(status["valid"])
        self.assertFalse(status["network_access_during_replay"])
        self.assertFalse(status["runtime_write"])
        self.assertFalse(status["live_basis_activation"])
        self.assertTrue(packet["comparison"]["operator_approval_required_for_live_activation"])
        self.assertEqual(3, len(rows))

    def test_project_writes_reviewable_packet(self) -> None:
        corpus, fixture = self._inputs()
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            corpus_path = root / "corpus.json"
            fixture_path = root / "fixture.json"
            corpus_path.write_text(json.dumps(corpus), encoding="utf-8")
            fixture_path.write_text(json.dumps(fixture), encoding="utf-8")
            status = project(
                root,
                corpus_path=corpus_path,
                fixture_path=fixture_path,
                write=True,
            )
            output = root / "diagnostics/counterfactual_change_lab_v1"
            self.assertTrue(status["valid"])
            self.assertTrue((output / "status.json").is_file())
            comparison = json.loads(
                (output / "campaigns/fixed_projection_basis_epoch_comparison_v1/comparison.json").read_text()
            )
            self.assertFalse(comparison["live_basis_activation"])
            self.assertFalse(comparison["runtime_write"])


if __name__ == "__main__":
    unittest.main()
