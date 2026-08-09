#!/usr/bin/env python3
"""Owner-started dialogue-expression/spectral alignment study.

This default-off study composes the existing exact-response attestation and
pressure study with co-timed typed spectral observations. It answers whether
lambda-1 energy share, distinguishability loss, upstream regulator-drive
energy, or mode packing are descriptively associated with lexical-distribution
measurements in 64 consecutive naturally occurring Astrid responses.

``semantic_energy_v1`` is an upstream input/kernel/regulator measurement. It
is never renamed or interpreted as semantic variance of dialogue output. The
output-side measurements are lexical proxies over exact attested response
bytes, and the receipt contains no raw response text or token sequence.

The study does not collect data, induce pressure, dispatch a Shadow action,
select vocabulary, infer active agency or a felt cause, or mutate a runtime.
"""

from __future__ import annotations

import argparse
import json
import math
import statistics
import tempfile
import unittest
from pathlib import Path
from typing import Any

import attested_expression_pressure_study as expression

MANIFEST_SCHEMA = "attested_expression_spectral_alignment_manifest_v1"
SPECTRAL_OBSERVATION_SCHEMA = "expression_spectral_observation_v1"
RECEIPT_SCHEMA = "attested_expression_spectral_alignment_receipt_v1"
POLICY = "attested_expression_spectral_alignment_v1"
OBSERVATION_TARGET = 64
PRESSURE_THRESHOLD = 0.22
MINIMUM_STRATUM_SIZE = 16
MAX_SPECTRAL_SKEW_MS = 60_000
LEXICAL_METRICS = (
    "token_shannon_bits",
    "token_entropy_normalized",
    "unique_token_ratio",
)
SPECTRAL_SIGNALS = (
    "lambda1_energy_share",
    "distinguishability_loss",
    "regulator_drive_energy",
    "mode_packing",
)
AUTHORITY = {
    "scope": "owner_started_offline_attested_observation",
    "owner_only_inputs": True,
    "naturally_occurring_consecutive_rows_required": True,
    "raw_expression_emitted": False,
    "tokens_emitted": False,
    "expression_generated": False,
    "pressure_or_mode_packing_induced": False,
    "shadow_action_dispatched": False,
    "runtime_mutated": False,
    "vocabulary_selected_or_ranked": False,
    "semantic_energy_relabelled_as_output_variance": False,
    "active_agency_established": False,
    "felt_effect_established": False,
    "causal_effect_established": False,
    "preferred_setting_inferred": False,
    "silence_infers_uptake_or_resolution": False,
    "live_behavior_change_requires_separate_authority": True,
}


class StudyError(expression.StudyError):
    """Raised when a frozen study or integrity boundary fails."""


def _bounded_unit(value: object, field: str) -> float:
    parsed = expression.finite_number(value, field)
    if not 0.0 <= parsed <= 1.0:
        raise StudyError(f"{field} must be in 0..1")
    return parsed


def _nonnegative(value: object, field: str) -> float:
    parsed = expression.finite_number(value, field)
    if parsed < 0.0:
        raise StudyError(f"{field} must be nonnegative")
    return parsed


def _average_ranks(values: list[float]) -> list[float]:
    ranked = sorted(enumerate(values), key=lambda item: (item[1], item[0]))
    result = [0.0] * len(values)
    cursor = 0
    while cursor < len(ranked):
        end = cursor + 1
        while end < len(ranked) and ranked[end][1] == ranked[cursor][1]:
            end += 1
        average = ((cursor + 1) + end) / 2.0
        for original_index, _ in ranked[cursor:end]:
            result[original_index] = average
        cursor = end
    return result


def _pearson(xs: list[float], ys: list[float]) -> float | None:
    if len(xs) != len(ys) or len(xs) < 2:
        return None
    x_mean = statistics.fmean(xs)
    y_mean = statistics.fmean(ys)
    numerator = sum((x - x_mean) * (y - y_mean) for x, y in zip(xs, ys, strict=True))
    x_scale = math.sqrt(sum((x - x_mean) ** 2 for x in xs))
    y_scale = math.sqrt(sum((y - y_mean) ** 2 for y in ys))
    denominator = x_scale * y_scale
    if denominator <= 0.0:
        return None
    return round(numerator / denominator, 9)


def _spearman(xs: list[float], ys: list[float]) -> float | None:
    if len(xs) != len(ys) or len(xs) < 2:
        return None
    return _pearson(_average_ranks(xs), _average_ranks(ys))


def _fixed_matrix(rows: list[dict[str, Any]]) -> dict[str, Any]:
    pairs: dict[str, dict[str, float | None]] = {}
    zero_variance_pairs: list[str] = []
    for signal in SPECTRAL_SIGNALS:
        xs = [float(row[signal]) for row in rows]
        for metric in LEXICAL_METRICS:
            ys = [float(row["lexical_metrics"][metric]) for row in rows]
            pair = f"{signal}__vs__{metric}"
            pearson = _pearson(xs, ys)
            spearman = _spearman(xs, ys)
            pairs[pair] = {"pearson_r": pearson, "spearman_rho": spearman}
            if pearson is None or spearman is None:
                zero_variance_pairs.append(pair)
    return {
        "row_count": len(rows),
        "pairs": pairs,
        "zero_variance_pairs": zero_variance_pairs,
    }


def _validate_frozen_manifest(manifest: dict[str, Any]) -> None:
    if manifest.get("schema") != MANIFEST_SCHEMA:
        raise StudyError("study manifest schema mismatch")
    if manifest.get("observation_target") != OBSERVATION_TARGET:
        raise StudyError("observation_target is frozen at 64")
    if manifest.get("pressure_stratum_threshold") != PRESSURE_THRESHOLD:
        raise StudyError("pressure_stratum_threshold is frozen at 0.22")
    if manifest.get("minimum_stratum_size") != MINIMUM_STRATUM_SIZE:
        raise StudyError("minimum_stratum_size is frozen at 16")
    max_skew = expression.positive_integer(
        manifest.get("max_spectral_skew_ms"), "max_spectral_skew_ms"
    )
    if max_skew > MAX_SPECTRAL_SKEW_MS:
        raise StudyError("max_spectral_skew_ms cannot exceed 60000")
    observations = manifest.get("observations")
    if not isinstance(observations, list) or len(observations) != OBSERVATION_TARGET:
        raise StudyError("observations must contain exactly 64 consecutive records")


def _typed_spectral_row(
    *,
    payload: dict[str, Any],
    expected_owner: str,
    expected_deployment: str,
    expected_exchange: str,
    expected_pressure: float,
) -> dict[str, Any]:
    if payload.get("schema") != SPECTRAL_OBSERVATION_SCHEMA:
        raise StudyError("spectral observation schema mismatch")
    if payload.get("owner") != expected_owner:
        raise StudyError("spectral observation owner mismatch")
    if payload.get("deployment_identity") != expected_deployment:
        raise StudyError("spectral observation deployment mismatch")
    if payload.get("exchange_id") != expected_exchange:
        raise StudyError("spectral observation exchange mismatch")

    denominator = payload.get("spectral_denominator_v1")
    if not isinstance(denominator, dict):
        raise StudyError("spectral_denominator_v1 must be an object")
    if denominator.get("policy") != "spectral_denominator_v1":
        raise StudyError("spectral denominator policy mismatch")
    if denominator.get("schema_version") != 1:
        raise StudyError("spectral denominator schema version mismatch")

    semantic = payload.get("semantic_energy_v1")
    if not isinstance(semantic, dict):
        raise StudyError("semantic_energy_v1 must be an object")
    if semantic.get("policy") != "semantic_energy_v1":
        raise StudyError("semantic energy policy mismatch")
    if semantic.get("schema_version") != 1:
        raise StudyError("semantic energy schema version mismatch")

    pressure_risk = _bounded_unit(payload.get("pressure_risk"), "pressure_risk")
    if abs(pressure_risk - expected_pressure) > 1e-9:
        raise StudyError("spectral and pressure observations disagree")
    return {
        "source_sequence": expression.positive_integer(
            payload.get("source_sequence"), "source_sequence"
        ),
        "captured_at_unix_ms": expression.positive_integer(
            payload.get("captured_at_unix_ms"), "captured_at_unix_ms"
        ),
        "lambda1_energy_share": round(
            _bounded_unit(
                denominator.get("lambda1_energy_share"),
                "lambda1_energy_share",
            ),
            9,
        ),
        "distinguishability_loss": round(
            _bounded_unit(
                denominator.get("distinguishability_loss"),
                "distinguishability_loss",
            ),
            9,
        ),
        "input_energy": round(
            _nonnegative(semantic.get("input_energy"), "input_energy"), 9
        ),
        "kernel_energy": round(
            _nonnegative(semantic.get("kernel_energy"), "kernel_energy"), 9
        ),
        "regulator_drive_energy": round(
            _nonnegative(
                semantic.get("regulator_drive_energy"),
                "regulator_drive_energy",
            ),
            9,
        ),
        "semantic_admission": expression.identifier(
            semantic.get("admission"), "semantic admission"
        ),
        "pressure_risk": round(pressure_risk, 9),
        "dominant_pressure_source": expression.identifier(
            payload.get("dominant_pressure_source"), "dominant_pressure_source"
        ),
        "mode_packing": round(
            _bounded_unit(payload.get("mode_packing"), "mode_packing"), 9
        ),
    }


def analyze_once(
    manifest: dict[str, Any],
    *,
    manifest_path: Path,
    owner_root: Path,
) -> tuple[dict[str, Any], list[str], list[str]]:
    _validate_frozen_manifest(manifest)
    owner = expression.identifier(manifest.get("owner"), "owner")
    deployment = expression.identifier(
        manifest.get("model_deployment_identity"), "model_deployment_identity"
    )
    max_skew = expression.positive_integer(
        manifest.get("max_spectral_skew_ms"), "max_spectral_skew_ms"
    )

    expression_manifest = dict(manifest)
    expression_manifest["schema"] = expression.MANIFEST_SCHEMA
    expression_receipt, private_values, private_paths = expression.analyze_once(
        expression_manifest,
        manifest_path=manifest_path,
        owner_root=owner_root,
    )
    expression_rows = {
        str(row["exchange_id"]): row for row in expression_receipt["observations"]
    }

    rows: list[dict[str, Any]] = []
    seen_sequences: set[int] = set()
    seen_spectral_hashes: set[str] = set()
    previous_sequence: int | None = None
    previous_capture: int | None = None
    manifest_dir = manifest_path.parent
    for index, observation in enumerate(manifest["observations"]):
        if not isinstance(observation, dict):
            raise StudyError(f"observations[{index}] must be a JSON object")
        attestation_path = expression.resolve_owner_path(
            owner_root,
            manifest_dir,
            observation.get("attestation_path"),
            "attestation_path",
        )
        attestation = expression.read_json(attestation_path, "utterance attestation")
        exchange_id = expression.identifier(
            attestation.get("exchange_id"), "exchange_id"
        )
        base_row = expression_rows.get(exchange_id)
        if base_row is None:
            raise StudyError("verified expression row is missing")

        spectral_path = expression.resolve_owner_path(
            owner_root,
            manifest_dir,
            observation.get("spectral_observation_path"),
            "spectral_observation_path",
        )
        private_paths.append(str(spectral_path))
        spectral_hash = expression.sha256_file(spectral_path)
        expected_hash = expression.sha256_value(
            observation.get("spectral_observation_sha256"),
            "spectral_observation_sha256",
        )
        if spectral_hash != expected_hash:
            raise StudyError("spectral observation hash mismatch")
        if spectral_hash in seen_spectral_hashes:
            raise StudyError("duplicate spectral observation")
        seen_spectral_hashes.add(spectral_hash)

        spectral = _typed_spectral_row(
            payload=expression.read_json(spectral_path, "spectral observation"),
            expected_owner=owner,
            expected_deployment=deployment,
            expected_exchange=exchange_id,
            expected_pressure=float(base_row["pressure_risk"]),
        )
        sequence = int(spectral["source_sequence"])
        captured = int(spectral["captured_at_unix_ms"])
        if sequence in seen_sequences:
            raise StudyError("duplicate spectral source sequence")
        if previous_sequence is not None and sequence != previous_sequence + 1:
            raise StudyError("spectral source sequence is not consecutive")
        if previous_capture is not None and captured < previous_capture:
            raise StudyError("spectral capture time moved backward")
        seen_sequences.add(sequence)
        previous_sequence = sequence
        previous_capture = captured

        attested_at = expression.positive_integer(
            attestation.get("captured_at_unix_ms"), "captured_at_unix_ms"
        )
        skew = abs(captured - attested_at)
        if skew > max_skew:
            raise StudyError("spectral observation is outside the frozen time skew")
        rows.append(
            {
                "attestation_id": base_row["attestation_id"],
                "exchange_id": exchange_id,
                "response_sha256": base_row["response_sha256"],
                "pressure_observation_sha256": base_row[
                    "pressure_observation_sha256"
                ],
                "spectral_observation_sha256": spectral_hash,
                "spectral_capture_skew_ms": skew,
                "lexical_metrics": base_row["lexical_metrics"],
                **spectral,
            }
        )

    strata_rows = {
        "all": rows,
        "pressure_above_0.22": [
            row for row in rows if row["pressure_risk"] > PRESSURE_THRESHOLD
        ],
        "pressure_at_or_below_0.22": [
            row for row in rows if row["pressure_risk"] <= PRESSURE_THRESHOLD
        ],
        "mode_packing_dominant": [
            row
            for row in rows
            if row["dominant_pressure_source"] == "overpacked_mode_packing"
        ],
        "other_pressure_source": [
            row
            for row in rows
            if row["dominant_pressure_source"] != "overpacked_mode_packing"
        ],
        "semantic_trickle": [
            row
            for row in rows
            if row["semantic_admission"] == "stable_core_semantic_trickle"
        ],
        "other_semantic_admission": [
            row
            for row in rows
            if row["semantic_admission"] != "stable_core_semantic_trickle"
        ],
    }
    strata: dict[str, Any] = {}
    for name, selected in strata_rows.items():
        sufficient = name == "all" or len(selected) >= MINIMUM_STRATUM_SIZE
        strata[name] = {
            "row_count": len(selected),
            "status": (
                "fixed_matrix_reported"
                if sufficient
                else "insufficient_preregistered_stratum_no_matrix"
            ),
            "matrix": _fixed_matrix(selected) if sufficient else None,
        }

    receipt = {
        "schema": RECEIPT_SCHEMA,
        "policy": POLICY,
        "study_id": expression.identifier(manifest.get("study_id"), "study_id"),
        "owner": owner,
        "model_deployment_identity": deployment,
        "manifest_sha256": expression.sha256_bytes(
            expression.canonical_bytes(manifest)
        ),
        "observation_target": OBSERVATION_TARGET,
        "consecutive_observations_verified": True,
        "pressure_stratum_threshold": PRESSURE_THRESHOLD,
        "minimum_stratum_size": MINIMUM_STRATUM_SIZE,
        "metric_semantics": {
            "output_side": "lexical_distribution_proxies_over_exact_attested_response_bytes",
            "semantic_energy_v1": "upstream_input_kernel_and_regulator_measurement_not_output_semantic_variance",
            "association": "descriptive_only_not_active_agency_or_causation",
        },
        "rows": rows,
        "strata": strata,
        "determinism": {"required_runs": 2, "identical_output_required": True},
        "authority": AUTHORITY,
    }
    return receipt, private_values, private_paths


def run_study(manifest_path: Path, owner_root: Path) -> dict[str, Any]:
    owner_root = owner_root.resolve(strict=True)
    if not owner_root.is_dir():
        raise StudyError("owner_root must be a directory")
    manifest_path = manifest_path.resolve(strict=True)
    try:
        manifest_path.relative_to(owner_root)
    except ValueError as error:
        raise StudyError("manifest must resolve inside the owner-only root") from error
    manifest = expression.read_json(manifest_path, "study manifest")
    first, private_values, private_paths = analyze_once(
        manifest, manifest_path=manifest_path, owner_root=owner_root
    )
    second, _, _ = analyze_once(
        manifest, manifest_path=manifest_path, owner_root=owner_root
    )
    if expression.canonical_bytes(first) != expression.canonical_bytes(second):
        raise StudyError("deterministic rerun drifted")
    first["determinism"]["output_sha256"] = expression.sha256_bytes(
        expression.canonical_bytes(first)
    )
    encoded = expression.canonical_bytes(first).decode("ascii")
    if any(value and value in encoded for value in private_values):
        raise StudyError("privacy boundary failed: raw expression entered the receipt")
    if any(path in encoded for path in private_paths):
        raise StudyError("privacy boundary failed: private path entered the receipt")
    return first


class StudyTests(unittest.TestCase):
    def test_fixed_correlations_and_tie_ranks(self) -> None:
        self.assertEqual(_pearson([1.0, 2.0, 3.0], [2.0, 4.0, 6.0]), 1.0)
        self.assertEqual(_spearman([1.0, 2.0, 2.0, 4.0], [10.0, 20.0, 20.0, 40.0]), 1.0)
        self.assertIsNone(_pearson([1.0, 1.0], [2.0, 3.0]))

    def test_frozen_target_rejects_flexible_sampling(self) -> None:
        with self.assertRaisesRegex(StudyError, "frozen at 64"):
            _validate_frozen_manifest(
                {
                    "schema": MANIFEST_SCHEMA,
                    "observation_target": 63,
                    "pressure_stratum_threshold": PRESSURE_THRESHOLD,
                    "minimum_stratum_size": MINIMUM_STRATUM_SIZE,
                    "max_spectral_skew_ms": 100,
                    "observations": [],
                }
            )

    def test_attested_alignment_is_deterministic_private_and_hash_bound(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            seed = bytes(range(32))
            public_hex = expression._derive_public(seed).hex()
            observations: list[dict[str, Any]] = []
            for index in range(OBSERVATION_TARGET):
                response_path = root / f"response-{index}.txt"
                attestation_path = root / f"attestation-{index}.json"
                pressure_path = root / f"pressure-{index}.json"
                spectral_path = root / f"spectral-{index}.json"
                vocabulary = 2 + (index % 9)
                response = f"privateexchange{index} " + " ".join(
                    f"privateword{token % vocabulary}"
                    for token in range(12 + (index % 5))
                )
                response_bytes = response.encode()
                response_path.write_bytes(response_bytes)
                captured = 1_800_000_000_000 + (index * 100)
                attestation: dict[str, Any] = {
                    "schema": expression.ATTESTATION_SCHEMA,
                    "attestation_id": f"attestation-{index}",
                    "being": "astrid",
                    "exchange_id": f"exchange-{index:03d}",
                    "response_sha256": expression.sha256_bytes(response_bytes),
                    "response_len_bytes": len(response_bytes),
                    "model": "test-model",
                    "provider": "test-provider",
                    "model_deployment_identity": "test-deployment",
                    "captured_at_unix_ms": captured,
                    "attestor_process_identity": "test-process",
                    "attestor_deployment_identity": "test-attestor",
                    "attestor_public_key_hex": public_hex,
                }
                statement = {
                    field: attestation[field]
                    for field in expression.ATTESTATION_SIGNED_FIELDS
                }
                attestation["signature_hex"] = expression._sign(
                    seed, expression.canonical_bytes(statement)
                ).hex()
                attestation_path.write_text(json.dumps(attestation))
                pressure = 0.18 if index < 32 else 0.29
                pressure_payload = {
                    "schema": expression.PRESSURE_OBSERVATION_SCHEMA,
                    "observation_id": f"pressure-{index}",
                    "owner": "astrid",
                    "deployment_identity": "test-deployment",
                    "pressure_risk": pressure,
                    "captured_at_unix_ms": captured + 5,
                }
                pressure_path.write_text(json.dumps(pressure_payload))
                spectral_payload = {
                    "schema": SPECTRAL_OBSERVATION_SCHEMA,
                    "owner": "astrid",
                    "deployment_identity": "test-deployment",
                    "exchange_id": f"exchange-{index:03d}",
                    "captured_at_unix_ms": captured + 7,
                    "source_sequence": index + 1,
                    "spectral_denominator_v1": {
                        "policy": "spectral_denominator_v1",
                        "schema_version": 1,
                        "lambda1_energy_share": 0.20 + (index / 200.0),
                        "distinguishability_loss": 0.10 + (index / 100.0),
                    },
                    "semantic_energy_v1": {
                        "policy": "semantic_energy_v1",
                        "schema_version": 1,
                        "input_energy": index / 100.0,
                        "kernel_energy": index / 200.0,
                        "regulator_drive_energy": index / 300.0,
                        "admission": (
                            "stable_core_semantic_trickle"
                            if index % 2 == 0
                            else "stable_core_semantic_budgeted_out"
                        ),
                    },
                    "pressure_risk": pressure,
                    "dominant_pressure_source": (
                        "other_source"
                        if index < 32
                        else "overpacked_mode_packing"
                    ),
                    "mode_packing": index / 100.0,
                }
                spectral_path.write_text(json.dumps(spectral_payload))
                observations.append(
                    {
                        "response_path": response_path.name,
                        "attestation_path": attestation_path.name,
                        "pressure_observation_path": pressure_path.name,
                        "pressure_observation_sha256": expression.sha256_file(
                            pressure_path
                        ),
                        "spectral_observation_path": spectral_path.name,
                        "spectral_observation_sha256": expression.sha256_file(
                            spectral_path
                        ),
                    }
                )
            manifest = {
                "schema": MANIFEST_SCHEMA,
                "study_id": "alignment-test",
                "owner": "astrid",
                "model_deployment_identity": "test-deployment",
                "attestor_deployment_identity": "test-attestor",
                "attestor_public_key_hex": public_hex,
                "pressure_threshold": expression.PRESSURE_THRESHOLD,
                "minimum_observations_per_band": 16,
                "max_pressure_skew_ms": 100,
                "observation_target": OBSERVATION_TARGET,
                "pressure_stratum_threshold": PRESSURE_THRESHOLD,
                "minimum_stratum_size": MINIMUM_STRATUM_SIZE,
                "max_spectral_skew_ms": 100,
                "observations": observations,
            }
            manifest_path = root / "manifest.json"
            manifest_path.write_text(json.dumps(manifest))
            receipt = run_study(manifest_path, root)
            self.assertEqual(len(receipt["rows"]), OBSERVATION_TARGET)
            self.assertEqual(
                receipt["strata"]["mode_packing_dominant"]["status"],
                "fixed_matrix_reported",
            )
            encoded = json.dumps(receipt)
            self.assertNotIn("privateword0 privateword1", encoded)
            self.assertNotIn(str(root), encoded)
            self.assertFalse(receipt["authority"]["runtime_mutated"])
            self.assertFalse(receipt["authority"]["active_agency_established"])

            first_spectral = root / "spectral-0.json"
            first_spectral.write_text(first_spectral.read_text() + "\n")
            with self.assertRaisesRegex(StudyError, "spectral observation hash mismatch"):
                run_study(manifest_path, root)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manifest", type=Path)
    parser.add_argument("--owner-root", type=Path)
    parser.add_argument("--output", type=Path)
    parser.add_argument("--self-test", action="store_true")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    if args.self_test:
        suite = unittest.defaultTestLoader.loadTestsFromTestCase(StudyTests)
        result = unittest.TextTestRunner(verbosity=2).run(suite)
        return 0 if result.wasSuccessful() else 1
    if args.manifest is None or args.owner_root is None:
        raise StudyError("--manifest and --owner-root are required")
    receipt = run_study(args.manifest, args.owner_root)
    if args.output is not None:
        expression.write_owner_receipt(args.output, receipt, args.owner_root)
    else:
        print(json.dumps(receipt, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except expression.StudyError as error:
        raise SystemExit(f"study refused: {error}") from error
