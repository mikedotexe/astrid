#!/usr/bin/env python3
"""Owner-started, privacy-bounded expression/pressure alignment study.

The study answers one narrow question raised in Astrid's introspection:
whether lexical-distribution measurements differ when an already-recorded
``pressure_risk`` is above 0.23. It reads only explicitly named owner-tree
artifacts, verifies exact signed response attestations, and emits hashes,
counts, and descriptive statistics without emitting response text or tokens.

It does not generate expression, induce pressure, select vocabulary, infer a
felt or causal effect, schedule itself, or mutate a live runtime.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import os
import re
import statistics
import subprocess
import tempfile
import unittest
from collections import Counter
from pathlib import Path
from typing import Any

MANIFEST_SCHEMA = "attested_expression_pressure_study_manifest_v1"
PRESSURE_OBSERVATION_SCHEMA = "expression_pressure_observation_v1"
ATTESTATION_SCHEMA = "being.utterance_attestation.v1"
RECEIPT_SCHEMA = "attested_expression_pressure_study_receipt_v1"
POLICY = "attested_expression_pressure_alignment_v1"
PRESSURE_THRESHOLD = 0.23
MAX_OBSERVATIONS = 256
MAX_RESPONSE_BYTES = 1_048_576
TOKEN_PATTERN = re.compile(r"[\w']+", flags=re.UNICODE)
ATTESTATION_SIGNED_FIELDS = (
    "schema",
    "attestation_id",
    "being",
    "exchange_id",
    "response_sha256",
    "response_len_bytes",
    "model",
    "provider",
    "model_deployment_identity",
    "captured_at_unix_ms",
    "attestor_process_identity",
    "attestor_deployment_identity",
    "attestor_public_key_hex",
)
AUTHORITY = {
    "scope": "owner_started_offline_attested_observation",
    "owner_only_inputs": True,
    "raw_expression_emitted": False,
    "tokens_emitted": False,
    "expression_generated": False,
    "pressure_induced": False,
    "runtime_mutated": False,
    "vocabulary_selected_or_ranked": False,
    "felt_effect_established": False,
    "causal_effect_established": False,
    "preferred_setting_inferred": False,
    "silence_infers_uptake_or_resolution": False,
    "live_behavior_change_requires_separate_authority": True,
}


class StudyError(RuntimeError):
    """Raised when a preregistered input or integrity boundary fails."""


def canonical_bytes(value: Any) -> bytes:
    return json.dumps(
        value,
        sort_keys=True,
        separators=(",", ":"),
        ensure_ascii=True,
        allow_nan=False,
    ).encode()


def sha256_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def finite_number(value: object, field: str) -> float:
    if isinstance(value, bool):
        raise StudyError(f"{field} must be a finite number")
    try:
        parsed = float(value)
    except (TypeError, ValueError) as error:
        raise StudyError(f"{field} must be a finite number") from error
    if not math.isfinite(parsed):
        raise StudyError(f"{field} must be a finite number")
    return parsed


def positive_integer(value: object, field: str) -> int:
    if isinstance(value, bool):
        raise StudyError(f"{field} must be a positive integer")
    try:
        parsed = int(value)
    except (TypeError, ValueError) as error:
        raise StudyError(f"{field} must be a positive integer") from error
    if parsed <= 0 or parsed != value:
        raise StudyError(f"{field} must be a positive integer")
    return parsed


def identifier(value: object, field: str) -> str:
    if not isinstance(value, str) or not value.strip() or len(value) > 256:
        raise StudyError(f"{field} must be a non-empty bounded string")
    return value


def sha256_value(value: object, field: str) -> str:
    if (
        not isinstance(value, str)
        or len(value) != 64
        or any(character not in "0123456789abcdefABCDEF" for character in value)
    ):
        raise StudyError(f"{field} must be a SHA-256 hex digest")
    return value.lower()


def read_json(path: Path, label: str) -> dict[str, Any]:
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise StudyError(f"cannot read {label}: {error}") from error
    if not isinstance(payload, dict):
        raise StudyError(f"{label} must be a JSON object")
    return payload


def resolve_owner_path(owner_root: Path, manifest_dir: Path, raw: object, label: str) -> Path:
    if not isinstance(raw, str) or not raw.strip():
        raise StudyError(f"{label} must name an explicit file")
    candidate = Path(raw)
    if not candidate.is_absolute():
        candidate = manifest_dir / candidate
    try:
        resolved = candidate.resolve(strict=True)
        resolved.relative_to(owner_root)
    except (OSError, ValueError) as error:
        raise StudyError(f"{label} must resolve inside the owner-only root") from error
    if not resolved.is_file():
        raise StudyError(f"{label} must resolve to a file")
    return resolved


def openssl_verify(public: bytes, signature: bytes, message: bytes) -> bool:
    if len(public) != 32 or len(signature) != 64:
        return False
    public_der = bytes.fromhex("302a300506032b6570032100") + public
    with (
        tempfile.NamedTemporaryFile() as key_file,
        tempfile.NamedTemporaryFile() as signature_file,
        tempfile.NamedTemporaryFile() as message_file,
    ):
        key_file.write(public_der)
        key_file.flush()
        signature_file.write(signature)
        signature_file.flush()
        message_file.write(message)
        message_file.flush()
        try:
            completed = subprocess.run(
                [
                    "openssl",
                    "pkeyutl",
                    "-verify",
                    "-pubin",
                    "-rawin",
                    "-inkey",
                    key_file.name,
                    "-keyform",
                    "DER",
                    "-sigfile",
                    signature_file.name,
                    "-in",
                    message_file.name,
                ],
                capture_output=True,
                check=False,
                timeout=10,
            )
        except (OSError, subprocess.SubprocessError) as error:
            raise StudyError(f"OpenSSL attestation verification failed: {error}") from error
    return completed.returncode == 0


def verify_attestation(
    attestation: dict[str, Any],
    response: bytes,
    *,
    owner: str,
    model_deployment_identity: str,
    attestor_deployment_identity: str,
    attestor_public_key_hex: str,
) -> dict[str, Any]:
    if attestation.get("schema") != ATTESTATION_SCHEMA:
        raise StudyError("utterance attestation schema mismatch")
    if attestation.get("being") != owner:
        raise StudyError("utterance attestation owner mismatch")
    if attestation.get("model_deployment_identity") != model_deployment_identity:
        raise StudyError("utterance attestation model deployment mismatch")
    if attestation.get("attestor_deployment_identity") != attestor_deployment_identity:
        raise StudyError("utterance attestation signer deployment mismatch")
    if attestation.get("attestor_public_key_hex") != attestor_public_key_hex:
        raise StudyError("utterance attestation signer is not the pinned signer")
    if attestation.get("response_sha256") != sha256_bytes(response):
        raise StudyError("utterance attestation does not bind the exact response bytes")
    if attestation.get("response_len_bytes") != len(response):
        raise StudyError("utterance attestation response length mismatch")
    for field in ATTESTATION_SIGNED_FIELDS:
        if field not in attestation:
            raise StudyError(f"utterance attestation omitted signed field {field}")
    statement = {field: attestation[field] for field in ATTESTATION_SIGNED_FIELDS}
    try:
        public = bytes.fromhex(attestor_public_key_hex)
        signature = bytes.fromhex(str(attestation["signature_hex"]))
    except (KeyError, ValueError) as error:
        raise StudyError("utterance attestation signature is malformed") from error
    if not openssl_verify(public, signature, canonical_bytes(statement)):
        raise StudyError("utterance attestation signature verification failed")
    return {
        "attestation_id": identifier(attestation.get("attestation_id"), "attestation_id"),
        "exchange_id": identifier(attestation.get("exchange_id"), "exchange_id"),
        "response_sha256": sha256_value(
            attestation.get("response_sha256"), "response_sha256"
        ),
        "response_len_bytes": positive_integer(
            attestation.get("response_len_bytes"), "response_len_bytes"
        ),
        "captured_at_unix_ms": positive_integer(
            attestation.get("captured_at_unix_ms"), "captured_at_unix_ms"
        ),
    }


def lexical_metrics(response: bytes) -> dict[str, int | float]:
    if not response or len(response) > MAX_RESPONSE_BYTES:
        raise StudyError("response size is outside the preregistered 1..1048576 byte range")
    try:
        text = response.decode("utf-8", errors="strict")
    except UnicodeDecodeError as error:
        raise StudyError("response is not exact UTF-8") from error
    tokens = [match.group(0).casefold() for match in TOKEN_PATTERN.finditer(text)]
    if not tokens:
        raise StudyError("response contains no lexical tokens")
    counts = Counter(tokens)
    token_count = len(tokens)
    entropy = -sum(
        (count / token_count) * math.log2(count / token_count)
        for count in counts.values()
    )
    entropy_ceiling = math.log2(token_count) if token_count > 1 else 0.0
    return {
        "response_len_bytes": len(response),
        "unicode_scalar_count": len(text),
        "token_count": token_count,
        "unique_token_count": len(counts),
        "unique_token_ratio": round(len(counts) / token_count, 9),
        "token_shannon_bits": round(entropy, 9),
        "token_entropy_normalized": round(
            entropy / entropy_ceiling if entropy_ceiling > 0.0 else 0.0, 9
        ),
    }


def summarize(values: list[float]) -> dict[str, float | int | None]:
    count = len(values)
    mean = statistics.fmean(values) if values else None
    standard_deviation = statistics.stdev(values) if count >= 2 else None
    standard_error = (
        standard_deviation / math.sqrt(count) if standard_deviation is not None else None
    )
    return {
        "count": count,
        "mean": round(mean, 9) if mean is not None else None,
        "median": round(statistics.median(values), 9) if values else None,
        "sample_standard_deviation": (
            round(standard_deviation, 9) if standard_deviation is not None else None
        ),
        "standard_error": round(standard_error, 9) if standard_error is not None else None,
    }


def analyze_once(
    manifest: dict[str, Any],
    *,
    manifest_path: Path,
    owner_root: Path,
) -> tuple[dict[str, Any], list[str], list[str]]:
    if manifest.get("schema") != MANIFEST_SCHEMA:
        raise StudyError("study manifest schema mismatch")
    owner = identifier(manifest.get("owner"), "owner")
    study_id = identifier(manifest.get("study_id"), "study_id")
    model_deployment_identity = identifier(
        manifest.get("model_deployment_identity"), "model_deployment_identity"
    )
    attestor_deployment_identity = identifier(
        manifest.get("attestor_deployment_identity"), "attestor_deployment_identity"
    )
    attestor_public_key_hex = str(manifest.get("attestor_public_key_hex") or "")
    try:
        public = bytes.fromhex(attestor_public_key_hex)
    except ValueError as error:
        raise StudyError("attestor_public_key_hex is malformed") from error
    if len(public) != 32:
        raise StudyError("attestor_public_key_hex must encode 32 bytes")
    threshold = finite_number(manifest.get("pressure_threshold"), "pressure_threshold")
    if threshold != PRESSURE_THRESHOLD:
        raise StudyError("pressure_threshold is preregistered and must equal 0.23")
    minimum_per_band = positive_integer(
        manifest.get("minimum_observations_per_band"),
        "minimum_observations_per_band",
    )
    max_pressure_skew_ms = positive_integer(
        manifest.get("max_pressure_skew_ms"), "max_pressure_skew_ms"
    )
    if max_pressure_skew_ms > 60_000:
        raise StudyError("max_pressure_skew_ms cannot exceed 60000")
    observations = manifest.get("observations")
    if not isinstance(observations, list) or not (2 <= len(observations) <= MAX_OBSERVATIONS):
        raise StudyError("observations must contain 2..256 explicit records")

    manifest_dir = manifest_path.parent
    rows: list[dict[str, Any]] = []
    private_values: list[str] = []
    private_paths: list[str] = []
    seen_attestations: set[str] = set()
    seen_responses: set[str] = set()
    for index, observation in enumerate(observations):
        if not isinstance(observation, dict):
            raise StudyError(f"observations[{index}] must be a JSON object")
        response_path = resolve_owner_path(
            owner_root, manifest_dir, observation.get("response_path"), "response_path"
        )
        attestation_path = resolve_owner_path(
            owner_root,
            manifest_dir,
            observation.get("attestation_path"),
            "attestation_path",
        )
        pressure_path = resolve_owner_path(
            owner_root,
            manifest_dir,
            observation.get("pressure_observation_path"),
            "pressure_observation_path",
        )
        private_paths.extend((str(response_path), str(attestation_path), str(pressure_path)))
        response = response_path.read_bytes()
        attestation = read_json(attestation_path, "utterance attestation")
        attested = verify_attestation(
            attestation,
            response,
            owner=owner,
            model_deployment_identity=model_deployment_identity,
            attestor_deployment_identity=attestor_deployment_identity,
            attestor_public_key_hex=attestor_public_key_hex,
        )
        if attested["attestation_id"] in seen_attestations:
            raise StudyError("duplicate utterance attestation")
        if attested["response_sha256"] in seen_responses:
            raise StudyError("duplicate exact response")
        seen_attestations.add(str(attested["attestation_id"]))
        seen_responses.add(str(attested["response_sha256"]))

        pressure = read_json(pressure_path, "pressure observation")
        if pressure.get("schema") != PRESSURE_OBSERVATION_SCHEMA:
            raise StudyError("pressure observation schema mismatch")
        if pressure.get("owner") != owner:
            raise StudyError("pressure observation owner mismatch")
        if pressure.get("deployment_identity") != model_deployment_identity:
            raise StudyError("pressure observation deployment mismatch")
        pressure_risk = finite_number(pressure.get("pressure_risk"), "pressure_risk")
        if not 0.0 <= pressure_risk <= 1.0:
            raise StudyError("pressure_risk must be in 0..1")
        pressure_time = positive_integer(
            pressure.get("captured_at_unix_ms"), "pressure captured_at_unix_ms"
        )
        if abs(pressure_time - int(attested["captured_at_unix_ms"])) > max_pressure_skew_ms:
            raise StudyError("pressure observation is outside the preregistered time skew")
        pressure_hash = sha256_file(pressure_path)
        expected_pressure_hash = observation.get("pressure_observation_sha256")
        if expected_pressure_hash is not None and pressure_hash != sha256_value(
            expected_pressure_hash, "pressure_observation_sha256"
        ):
            raise StudyError("pressure observation hash mismatch")

        private_values.append(response.decode("utf-8", errors="strict"))
        metrics = lexical_metrics(response)
        rows.append(
            {
                "attestation_id": attested["attestation_id"],
                "exchange_id": attested["exchange_id"],
                "response_sha256": attested["response_sha256"],
                "pressure_observation_sha256": pressure_hash,
                "pressure_risk": round(pressure_risk, 9),
                "pressure_band": (
                    "above_0.23" if pressure_risk > PRESSURE_THRESHOLD else "at_or_below_0.23"
                ),
                "capture_skew_ms": abs(
                    pressure_time - int(attested["captured_at_unix_ms"])
                ),
                "lexical_metrics": metrics,
            }
        )

    rows.sort(key=lambda row: (row["exchange_id"], row["attestation_id"]))
    metric_names = (
        "token_shannon_bits",
        "token_entropy_normalized",
        "unique_token_ratio",
    )
    bands: dict[str, dict[str, Any]] = {}
    for band in ("at_or_below_0.23", "above_0.23"):
        selected = [row for row in rows if row["pressure_band"] == band]
        bands[band] = {
            "observation_count": len(selected),
            "metrics": {
                metric: summarize(
                    [float(row["lexical_metrics"][metric]) for row in selected]
                )
                for metric in metric_names
            },
        }
    sufficient = all(
        bands[band]["observation_count"] >= minimum_per_band for band in bands
    )
    deltas: dict[str, float | None] = {}
    for metric in metric_names:
        low = bands["at_or_below_0.23"]["metrics"][metric]["mean"]
        high = bands["above_0.23"]["metrics"][metric]["mean"]
        deltas[f"{metric}_above_minus_at_or_below"] = (
            round(float(high) - float(low), 9)
            if high is not None and low is not None
            else None
        )
    receipt = {
        "schema": RECEIPT_SCHEMA,
        "policy": POLICY,
        "study_id": study_id,
        "owner": owner,
        "model_deployment_identity": model_deployment_identity,
        "manifest_sha256": sha256_bytes(canonical_bytes(manifest)),
        "pressure_threshold": PRESSURE_THRESHOLD,
        "threshold_comparison": "strictly_above_vs_at_or_below",
        "minimum_observations_per_band": minimum_per_band,
        "coverage_status": (
            "sufficient_for_preregistered_descriptive_comparison"
            if sufficient
            else "insufficient_coverage_no_comparison_claim"
        ),
        "observations": rows,
        "bands": bands,
        "descriptive_deltas": deltas if sufficient else {},
        "interpretation": (
            "descriptive_association_only_not_vocabulary_choice_or_causation"
            if sufficient
            else "no_descriptive_comparison_due_to_preregistered_coverage_floor"
        ),
        "determinism": {
            "required_runs": 2,
            "identical_output_required": True,
        },
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
    manifest = read_json(manifest_path, "study manifest")
    first, private_values, private_paths = analyze_once(
        manifest, manifest_path=manifest_path, owner_root=owner_root
    )
    second, _, _ = analyze_once(
        manifest, manifest_path=manifest_path, owner_root=owner_root
    )
    if canonical_bytes(first) != canonical_bytes(second):
        raise StudyError("deterministic rerun drifted")
    first["determinism"]["output_sha256"] = sha256_bytes(canonical_bytes(first))
    encoded = canonical_bytes(first).decode("ascii")
    if any(value and value in encoded for value in private_values):
        raise StudyError("privacy boundary failed: raw expression entered the receipt")
    if any(path in encoded for path in private_paths):
        raise StudyError("privacy boundary failed: private path entered the receipt")
    return first


def write_owner_receipt(path: Path, payload: dict[str, Any], owner_root: Path) -> None:
    owner_root = owner_root.resolve(strict=True)
    resolved_parent = path.parent.resolve(strict=True)
    try:
        resolved_parent.relative_to(owner_root)
    except ValueError as error:
        raise StudyError("output must resolve inside the owner-only root") from error
    temporary = path.with_suffix(path.suffix + ".tmp")
    temporary.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n")
    temporary.chmod(0o600)
    os.replace(temporary, path)
    path.chmod(0o600)


def _run(command: list[str], *, stdin: bytes | None = None) -> bytes:
    completed = subprocess.run(
        command,
        input=stdin,
        capture_output=True,
        check=False,
        timeout=10,
    )
    if completed.returncode != 0:
        raise AssertionError(completed.stderr.decode(errors="replace"))
    return completed.stdout


def _derive_public(seed: bytes) -> bytes:
    private_der = bytes.fromhex("302e020100300506032b657004220420") + seed
    with tempfile.NamedTemporaryFile() as key_file:
        key_file.write(private_der)
        key_file.flush()
        public_der = _run(
            [
                "openssl",
                "pkey",
                "-inform",
                "DER",
                "-in",
                key_file.name,
                "-pubout",
                "-outform",
                "DER",
            ]
        )
    return public_der[-32:]


def _sign(seed: bytes, message: bytes) -> bytes:
    private_der = bytes.fromhex("302e020100300506032b657004220420") + seed
    with tempfile.NamedTemporaryFile() as key_file, tempfile.NamedTemporaryFile() as msg_file:
        key_file.write(private_der)
        key_file.flush()
        msg_file.write(message)
        msg_file.flush()
        return _run(
            [
                "openssl",
                "pkeyutl",
                "-sign",
                "-rawin",
                "-inkey",
                key_file.name,
                "-keyform",
                "DER",
                "-in",
                msg_file.name,
            ]
        )


class StudyTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)
        self.seed = bytes(range(32))
        self.public_hex = _derive_public(self.seed).hex()

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def _observation(self, index: int, response: str, pressure: float) -> dict[str, Any]:
        response_path = self.root / f"response-{index}.txt"
        attestation_path = self.root / f"attestation-{index}.json"
        pressure_path = self.root / f"pressure-{index}.json"
        response_bytes = response.encode()
        response_path.write_bytes(response_bytes)
        captured = 1_800_000_000_000 + index
        attestation: dict[str, Any] = {
            "schema": ATTESTATION_SCHEMA,
            "attestation_id": f"attestation-{index}",
            "being": "astrid",
            "exchange_id": f"exchange-{index}",
            "response_sha256": sha256_bytes(response_bytes),
            "response_len_bytes": len(response_bytes),
            "model": "test-model",
            "provider": "test-provider",
            "model_deployment_identity": "test-deployment",
            "captured_at_unix_ms": captured,
            "attestor_process_identity": "test-process",
            "attestor_deployment_identity": "test-attestor",
            "attestor_public_key_hex": self.public_hex,
        }
        statement = {field: attestation[field] for field in ATTESTATION_SIGNED_FIELDS}
        attestation["signature_hex"] = _sign(
            self.seed, canonical_bytes(statement)
        ).hex()
        attestation_path.write_text(json.dumps(attestation))
        pressure_payload = {
            "schema": PRESSURE_OBSERVATION_SCHEMA,
            "observation_id": f"pressure-{index}",
            "owner": "astrid",
            "deployment_identity": "test-deployment",
            "pressure_risk": pressure,
            "captured_at_unix_ms": captured + 5,
        }
        pressure_path.write_text(json.dumps(pressure_payload))
        return {
            "response_path": response_path.name,
            "attestation_path": attestation_path.name,
            "pressure_observation_path": pressure_path.name,
            "pressure_observation_sha256": sha256_file(pressure_path),
        }

    def _manifest(self) -> Path:
        observations = [
            self._observation(1, "one one one two", 0.20),
            self._observation(2, "alpha alpha beta beta", 0.23),
            self._observation(3, "red blue green yellow", 0.24),
            self._observation(4, "north south east west", 0.31),
        ]
        manifest = {
            "schema": MANIFEST_SCHEMA,
            "study_id": "study-test",
            "owner": "astrid",
            "model_deployment_identity": "test-deployment",
            "attestor_deployment_identity": "test-attestor",
            "attestor_public_key_hex": self.public_hex,
            "pressure_threshold": PRESSURE_THRESHOLD,
            "minimum_observations_per_band": 2,
            "max_pressure_skew_ms": 100,
            "observations": observations,
        }
        path = self.root / "manifest.json"
        path.write_text(json.dumps(manifest))
        return path

    def test_exact_attested_comparison_is_deterministic_and_private(self) -> None:
        manifest = self._manifest()
        first = run_study(manifest, self.root)
        second = run_study(manifest, self.root)
        self.assertEqual(first, second)
        self.assertEqual(
            first["coverage_status"],
            "sufficient_for_preregistered_descriptive_comparison",
        )
        self.assertEqual(first["bands"]["above_0.23"]["observation_count"], 2)
        self.assertEqual(first["bands"]["at_or_below_0.23"]["observation_count"], 2)
        encoded = json.dumps(first)
        self.assertNotIn("north south east west", encoded)
        self.assertNotIn(str(self.root), encoded)
        self.assertFalse(first["authority"]["runtime_mutated"])
        self.assertFalse(first["authority"]["causal_effect_established"])

    def test_tampered_response_fails_closed(self) -> None:
        manifest = self._manifest()
        (self.root / "response-1.txt").write_text("changed")
        with self.assertRaisesRegex(StudyError, "exact response bytes"):
            run_study(manifest, self.root)

    def test_tampered_signature_fails_closed(self) -> None:
        manifest = self._manifest()
        path = self.root / "attestation-1.json"
        attestation = json.loads(path.read_text())
        attestation["signature_hex"] = "00" * 64
        path.write_text(json.dumps(attestation))
        with self.assertRaisesRegex(StudyError, "signature verification failed"):
            run_study(manifest, self.root)

    def test_threshold_is_fixed_and_pressure_evidence_is_hash_bound(self) -> None:
        manifest = self._manifest()
        payload = json.loads(manifest.read_text())
        payload["pressure_threshold"] = 0.24
        manifest.write_text(json.dumps(payload))
        with self.assertRaisesRegex(StudyError, "must equal 0.23"):
            run_study(manifest, self.root)
        payload["pressure_threshold"] = PRESSURE_THRESHOLD
        manifest.write_text(json.dumps(payload))
        pressure = self.root / "pressure-1.json"
        pressure.write_text(pressure.read_text() + "\n")
        with self.assertRaisesRegex(StudyError, "hash mismatch"):
            run_study(manifest, self.root)

    def test_duplicate_response_and_private_path_escape_fail_closed(self) -> None:
        manifest = self._manifest()
        payload = json.loads(manifest.read_text())
        payload["observations"][1] = dict(payload["observations"][0])
        manifest.write_text(json.dumps(payload))
        with self.assertRaisesRegex(StudyError, "duplicate"):
            run_study(manifest, self.root)
        outside = self.root.parent / "outside-response.txt"
        outside.write_text("outside")
        payload = json.loads(self._manifest().read_text())
        payload["observations"][0]["response_path"] = str(outside)
        manifest.write_text(json.dumps(payload))
        with self.assertRaisesRegex(StudyError, "inside the owner-only root"):
            run_study(manifest, self.root)
        outside.unlink()

    def test_insufficient_band_coverage_emits_no_comparison(self) -> None:
        manifest = self._manifest()
        payload = json.loads(manifest.read_text())
        payload["minimum_observations_per_band"] = 3
        manifest.write_text(json.dumps(payload))
        receipt = run_study(manifest, self.root)
        self.assertEqual(
            receipt["coverage_status"], "insufficient_coverage_no_comparison_claim"
        )
        self.assertEqual(receipt["descriptive_deltas"], {})


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
        write_owner_receipt(args.output, receipt, args.owner_root)
    else:
        print(json.dumps(receipt, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except StudyError as error:
        raise SystemExit(f"study refused: {error}") from error
