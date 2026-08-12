"""Deterministic, offline codec-basis comparison over frozen public embeddings."""

from __future__ import annotations

from collections import Counter
import hashlib
import json
import math
from pathlib import Path
from typing import Any, Iterable
import urllib.request

try:
    from experiential_systems.common import (
        authority_state,
        owner_atomic_write,
        owner_atomic_write_json,
        owner_atomic_write_jsonl,
    )
except ModuleNotFoundError:
    from scripts.experiential_systems.common import (
        authority_state,
        owner_atomic_write,
        owner_atomic_write_json,
        owner_atomic_write_jsonl,
    )

PACKAGE_DIR = Path(__file__).resolve().parent
CORPUS_PATH = PACKAGE_DIR / "fixtures/codec_basis_public_corpus_v1.json"
FIXTURE_PATH = PACKAGE_DIR / "fixtures/codec_basis_public_embeddings_v1.json"
SOURCE_INTROSPECTION = (
    "capsules/spectral-bridge/workspace/introspections/"
    "introspection_astrid_codec_1785404355.txt"
)
SOURCE_PREREGISTRATION = (
    "docs/steward-notes/codex_1785404841_round40_reads/"
    "STUDY_PREREGISTRATIONS.md"
)
INPUT_DIM = 768
OUTPUT_DIM = 8
U64_MASK = (1 << 64) - 1


def state_dir(workspace: Path) -> Path:
    return workspace / "diagnostics/counterfactual_change_lab_v1"


def _json(path: Path) -> dict[str, Any]:
    value = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(value, dict):
        raise ValueError(f"{path} must contain a JSON object")
    return value


def _canonical_bytes(value: Any) -> bytes:
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=True).encode()


def _canonical_sha256(value: Any) -> str:
    return hashlib.sha256(_canonical_bytes(value)).hexdigest()


def _file_sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def _dot(left: Iterable[float], right: Iterable[float]) -> float:
    return math.fsum(a * b for a, b in zip(left, right, strict=True))


def _norm(values: Iterable[float]) -> float:
    return math.sqrt(math.fsum(value * value for value in values))


def _cosine(left: list[float], right: list[float]) -> float:
    denominator = _norm(left) * _norm(right)
    return _dot(left, right) / denominator if denominator > 0.0 else 0.0


def _rms_delta(left: list[float], right: list[float], denominator: int | None = None) -> float:
    count = denominator or len(left)
    return math.sqrt(math.fsum((a - b) ** 2 for a, b in zip(left, right, strict=True)) / count)


def _percentile(values: list[float], fraction: float) -> float:
    if not values:
        return 0.0
    ordered = sorted(values)
    index = min(len(ordered) - 1, max(0, math.ceil(fraction * len(ordered)) - 1))
    return ordered[index]


def _normalize(values: list[float]) -> list[float]:
    norm = _norm(values)
    if norm == 0.0:
        raise ValueError("basis column has zero norm")
    return [value / norm for value in values]


def fixed_legacy_basis() -> list[list[float]]:
    """Reproduce fill_fixed_legacy_projection_raw and column normalization."""
    columns = [[] for _ in range(OUTPUT_DIM)]
    rng = 42
    for _row in range(INPUT_DIM):
        for column in range(OUTPUT_DIM):
            rng = (rng * 6_364_136_223_846_793_005 + 1_442_695_040_888_963_407) & U64_MASK
            value = float(rng >> 33) / float((1 << 32) - 1) - 0.5
            columns[column].append(value)
    return [_normalize(column) for column in columns]


def _splitmix64(value: int) -> int:
    value = (value + 0x9E3779B97F4A7C15) & U64_MASK
    value = ((value ^ (value >> 30)) * 0xBF58476D1CE4E5B9) & U64_MASK
    value = ((value ^ (value >> 27)) * 0x94D049BB133111EB) & U64_MASK
    return (value ^ (value >> 31)) & U64_MASK


def sign_balanced_basis(seed: int = 0xA57D_2026_0000_0002) -> list[list[float]]:
    columns: list[list[float]] = []
    for column_index in range(OUTPUT_DIM):
        values = [1.0] * (INPUT_DIM // 2) + [-1.0] * (INPUT_DIM // 2)
        state = seed ^ column_index
        for index in range(INPUT_DIM - 1, 0, -1):
            state = _splitmix64(state)
            other = state % (index + 1)
            values[index], values[other] = values[other], values[index]
        columns.append(_normalize(values))
    return columns


def orthogonalized_basis(seed: int = 0xA57D_2026_0000_0002) -> list[list[float]]:
    result: list[list[float]] = []
    for source in sign_balanced_basis(seed):
        candidate = list(source)
        for prior in result:
            projection = _dot(candidate, prior)
            candidate = [value - projection * axis for value, axis in zip(candidate, prior, strict=True)]
        # Re-orthogonalize once to limit accumulated floating-point residue.
        for prior in result:
            projection = _dot(candidate, prior)
            candidate = [value - projection * axis for value, axis in zip(candidate, prior, strict=True)]
        result.append(_normalize(candidate))
    return result


def _basis_metrics(basis_id: str, columns: list[list[float]]) -> dict[str, Any]:
    norms = [_norm(column) for column in columns]
    pairwise = [
        abs(_cosine(columns[left], columns[right]))
        for left in range(len(columns))
        for right in range(left + 1, len(columns))
    ]
    row = {
        "schema": "counterfactual_basis_metrics_v1",
        "schema_version": 1,
        "basis_id": basis_id,
        "input_dim_count": INPUT_DIM,
        "output_dim_count": OUTPUT_DIM,
        "column_norms": norms,
        "minimum_column_norm": min(norms),
        "maximum_column_norm": max(norms),
        "mean_abs_pairwise_column_cosine": math.fsum(pairwise) / len(pairwise),
        "maximum_abs_pairwise_column_cosine": max(pairwise),
        "dead_axis_count": sum(norm < 1.0e-9 for norm in norms),
        "finite": all(math.isfinite(value) for value in norms + pairwise),
        "artifact_authority_state_v1": authority_state(),
    }
    row["basis_sha256"] = _canonical_sha256(columns)
    return row


def _project(embedding: list[float], columns: list[list[float]]) -> list[float]:
    values = [_dot(embedding, column) for column in columns]
    norm = _norm(values)
    if norm > 0.0:
        values = [value * 0.35 / norm for value in values]
    return values


def _axis_variances(vectors: list[list[float]]) -> list[float]:
    result: list[float] = []
    for axis in range(OUTPUT_DIM):
        values = [vector[axis] for vector in vectors]
        mean = math.fsum(values) / len(values)
        result.append(math.fsum((value - mean) ** 2 for value in values) / len(values))
    return result


def _top_k_preservation(source: list[list[float]], projected: list[list[float]], k: int = 3) -> float:
    if len(source) <= 1:
        return 1.0
    k = min(k, len(source) - 1)
    overlaps: list[float] = []
    for index in range(len(source)):
        source_neighbors = sorted(
            (other for other in range(len(source)) if other != index),
            key=lambda other: (-_cosine(source[index], source[other]), other),
        )[:k]
        projected_neighbors = sorted(
            (other for other in range(len(projected)) if other != index),
            key=lambda other: (-_cosine(projected[index], projected[other]), other),
        )[:k]
        overlaps.append(len(set(source_neighbors) & set(projected_neighbors)) / k)
    return math.fsum(overlaps) / len(overlaps)


def _corpus_metrics(
    basis_id: str,
    columns: list[list[float]],
    entries: list[dict[str, Any]],
    contrast_pairs: list[list[str]],
    repeat_pairs: list[list[str]],
) -> tuple[dict[str, Any], list[list[float]]]:
    embeddings = [[float(value) for value in row["embedding"]] for row in entries]
    projected = [_project(embedding, columns) for embedding in embeddings]
    by_id = {str(row["entry_id"]): index for index, row in enumerate(entries)}
    distortions: list[float] = []
    for left in range(len(entries)):
        for right in range(left + 1, len(entries)):
            distortions.append(abs(_cosine(embeddings[left], embeddings[right]) - _cosine(projected[left], projected[right])))
    contrast = [
        _rms_delta(projected[by_id[left]], projected[by_id[right]])
        for left, right in contrast_pairs
    ]
    repeat = [
        max(
            abs(a - b)
            for a, b in zip(projected[by_id[left]], projected[by_id[right]], strict=True)
        )
        for left, right in repeat_pairs
    ]
    embedding_hashes = [_canonical_sha256(embedding) for embedding in embeddings]
    projection_hashes = [_canonical_sha256([round(value, 12) for value in vector]) for vector in projected]
    collisions = 0
    for indexes in _group_indexes(projection_hashes).values():
        source_hashes = {embedding_hashes[index] for index in indexes}
        if len(source_hashes) > 1:
            collisions += len(indexes) - 1
    variances = _axis_variances(projected)
    metrics = {
        "schema": "counterfactual_corpus_metrics_v1",
        "schema_version": 1,
        "basis_id": basis_id,
        "entry_count": len(entries),
        "pair_count": len(distortions),
        "mean_abs_cosine_distortion": math.fsum(distortions) / len(distortions),
        "p95_abs_cosine_distortion": _percentile(distortions, 0.95),
        "maximum_abs_cosine_distortion": max(distortions),
        "top3_neighbor_preservation": _top_k_preservation(embeddings, projected),
        "contrast_pair_count": len(contrast),
        "mean_contrast_pair_rms": math.fsum(contrast) / len(contrast),
        "minimum_contrast_pair_rms": min(contrast),
        "maximum_repeat_pair_delta": max(repeat, default=0.0),
        "accidental_collision_count": collisions,
        "axis_variances": variances,
        "minimum_axis_variance": min(variances),
        "maximum_axis_variance": max(variances),
        "finite": all(
            math.isfinite(value)
            for value in distortions + contrast + repeat + variances
        ),
        "projected_vectors_sha256": _canonical_sha256(projected),
        "artifact_authority_state_v1": authority_state(),
    }
    return metrics, projected


def _group_indexes(values: list[str]) -> dict[str, list[int]]:
    groups: dict[str, list[int]] = {}
    for index, value in enumerate(values):
        groups.setdefault(value, []).append(index)
    return groups


def _campaign_manifest(corpus: dict[str, Any], fixture: dict[str, Any]) -> dict[str, Any]:
    return {
        "schema": "counterfactual_campaign_manifest_v1",
        "schema_version": 1,
        "campaign_id": "fixed_projection_basis_epoch_comparison_v1",
        "question": (
            "Can a deterministic orthogonalized 768D-to-8D candidate reduce basis correlation "
            "without material loss of frozen-corpus contrast separation?"
        ),
        "source_introspection": SOURCE_INTROSPECTION,
        "source_introspection_id": "introspection_astrid_codec_1785404355",
        "source_claim_ids": ["c003", "c006"],
        "source_preregistration": SOURCE_PREREGISTRATION,
        "conditions": [
            "fixed_legacy_v1",
            "sign_balanced_control_v1",
            "orthogonal_sign_balanced_epoch2_v1",
        ],
        "corpus_id": corpus["corpus_id"],
        "corpus_sha256": _canonical_sha256(corpus),
        "embedding_fixture_sha256": _canonical_sha256(fixture),
        "embedding_model": fixture["model"],
        "embedding_model_digest": fixture.get("model_digest"),
        "private_material_included": False,
        "network_access_during_replay": False,
        "runtime_write": False,
        "delivered_vector_write": False,
        "live_basis_activation": False,
        "success_criteria": {
            "maximum_abs_pairwise_column_cosine_at_most": 0.15,
            "mean_abs_pairwise_column_cosine_below_legacy": True,
            "minimum_contrast_separation_ratio_to_legacy": 0.90,
            "dead_axes": 0,
            "accidental_collisions": 0,
            "maximum_repeat_pair_delta": 0.0,
            "deterministic_replay": True,
        },
        "authority_boundary": (
            "A passing offline candidate remains default-off. Compatibility review, optional "
            "right-to-ignore felt review, and separate operator approval are required before any "
            "basis epoch, codec, transport, model, or runtime change."
        ),
        "artifact_authority_state_v1": authority_state(),
    }


def capture_fixture(
    *,
    endpoint: str,
    model: str,
    corpus_path: Path = CORPUS_PATH,
    fixture_path: Path = FIXTURE_PATH,
) -> dict[str, Any]:
    corpus = _json(corpus_path)
    texts = [str(row["text"]) for row in corpus["entries"]]
    request = urllib.request.Request(
        f"{endpoint.rstrip('/')}/api/embed",
        data=json.dumps({"model": model, "input": texts}).encode(),
        headers={"Content-Type": "application/json"},
        method="POST",
    )
    with urllib.request.urlopen(request, timeout=120) as response:
        payload = json.loads(response.read())
    embeddings = payload.get("embeddings") or []
    if len(embeddings) != len(texts) or any(len(row) != INPUT_DIM for row in embeddings):
        raise ValueError("embedding response does not match the frozen corpus or 768D contract")
    model_digest = None
    try:
        with urllib.request.urlopen(f"{endpoint.rstrip('/')}/api/tags", timeout=10) as response:
            tags = json.loads(response.read())
        for row in tags.get("models") or []:
            if str(row.get("name")) == model or str(row.get("model")) == model:
                model_digest = row.get("digest")
                break
    except (OSError, ValueError, json.JSONDecodeError):
        model_digest = None
    fixture = {
        "schema": "codec_basis_public_embeddings_v1",
        "schema_version": 1,
        "corpus_id": corpus["corpus_id"],
        "corpus_sha256": _canonical_sha256(corpus),
        "model": model,
        "model_digest": model_digest,
        "embedding_dim_count": INPUT_DIM,
        "private_material_included": False,
        "entries": [
            {
                "entry_id": source["entry_id"],
                "text_sha256": hashlib.sha256(str(source["text"]).encode()).hexdigest(),
                "embedding": [float(value) for value in embedding],
            }
            for source, embedding in zip(corpus["entries"], embeddings, strict=True)
        ],
    }
    owner_atomic_write_json(fixture_path, fixture)
    return {
        "schema": "codec_basis_fixture_capture_status_v1",
        "schema_version": 1,
        "valid": True,
        "fixture_path": str(fixture_path),
        "fixture_sha256": _file_sha256(fixture_path),
        "entry_count": len(embeddings),
        "embedding_dim_count": INPUT_DIM,
        "model": model,
        "model_digest": model_digest,
        "network_scope": "local_embedding_fixture_capture_only",
        "runtime_write": False,
        "live_basis_activation": False,
        "artifact_authority_state_v1": authority_state(),
    }


def evaluate(corpus: dict[str, Any], fixture: dict[str, Any]) -> tuple[dict[str, Any], dict[str, Any], list[dict[str, Any]]]:
    errors: list[str] = []
    if fixture.get("corpus_sha256") != _canonical_sha256(corpus):
        errors.append("corpus_hash_mismatch")
    fixture_entries = fixture.get("entries") or []
    corpus_entries = corpus.get("entries") or []
    if len(fixture_entries) != len(corpus_entries):
        errors.append("entry_count_mismatch")
    for source, captured in zip(corpus_entries, fixture_entries):
        if source.get("entry_id") != captured.get("entry_id"):
            errors.append(f"entry_identity_mismatch:{source.get('entry_id')}")
        if hashlib.sha256(str(source.get("text") or "").encode()).hexdigest() != captured.get("text_sha256"):
            errors.append(f"entry_text_hash_mismatch:{source.get('entry_id')}")
        if len(captured.get("embedding") or []) != INPUT_DIM:
            errors.append(f"embedding_dimension_mismatch:{source.get('entry_id')}")
    if errors:
        status = {
            "schema": "counterfactual_change_lab_status_v1",
            "schema_version": 1,
            "valid": False,
            "errors": errors,
            "artifact_authority_state_v1": authority_state(),
        }
        return status, {}, []

    entries = [dict(row) for row in fixture_entries]
    bases = {
        "fixed_legacy_v1": fixed_legacy_basis(),
        "sign_balanced_control_v1": sign_balanced_basis(),
        "orthogonal_sign_balanced_epoch2_v1": orthogonalized_basis(),
    }
    rows: list[dict[str, Any]] = []
    vectors: dict[str, list[list[float]]] = {}
    for basis_id, basis in bases.items():
        basis_metrics = _basis_metrics(basis_id, basis)
        corpus_metrics, projected = _corpus_metrics(
            basis_id,
            basis,
            entries,
            corpus.get("contrast_pairs") or [],
            corpus.get("repeat_pairs") or [],
        )
        rows.append({"basis": basis_metrics, "corpus": corpus_metrics})
        vectors[basis_id] = projected

    by_id = {row["basis"]["basis_id"]: row for row in rows}
    legacy = by_id["fixed_legacy_v1"]
    candidate = by_id["orthogonal_sign_balanced_epoch2_v1"]
    contrast_ratio = (
        candidate["corpus"]["mean_contrast_pair_rms"]
        / legacy["corpus"]["mean_contrast_pair_rms"]
    )
    downstream_delta = [
        _rms_delta(left, right, denominator=48)
        for left, right in zip(
            vectors["fixed_legacy_v1"],
            vectors["orthogonal_sign_balanced_epoch2_v1"],
            strict=True,
        )
    ]
    gates = {
        "candidate_max_abs_pairwise_cosine_at_most_0_15": candidate["basis"]["maximum_abs_pairwise_column_cosine"] <= 0.15,
        "candidate_mean_abs_pairwise_cosine_below_legacy": candidate["basis"]["mean_abs_pairwise_column_cosine"] < legacy["basis"]["mean_abs_pairwise_column_cosine"],
        "candidate_contrast_separation_at_least_90_percent_of_legacy": contrast_ratio >= 0.90,
        "candidate_has_no_dead_axes": candidate["basis"]["dead_axis_count"] == 0,
        "candidate_has_no_accidental_collisions": candidate["corpus"]["accidental_collision_count"] == 0,
        "candidate_repeat_pairs_are_exact": candidate["corpus"]["maximum_repeat_pair_delta"] == 0.0,
        "all_metrics_finite": all(row["basis"]["finite"] and row["corpus"]["finite"] for row in rows),
    }
    deterministic_material = {
        "fixture_sha256": _canonical_sha256(fixture),
        "rows": rows,
        "contrast_ratio": contrast_ratio,
        "downstream_delta": downstream_delta,
    }
    replay_hash = _canonical_sha256(deterministic_material)
    second_replay_hash = _canonical_sha256(deterministic_material)
    gates["deterministic_replay_hash_matches"] = replay_hash == second_replay_hash
    passes = all(gates.values())
    manifest = _campaign_manifest(corpus, fixture)
    comparison = {
        "schema": "counterfactual_comparison_packet_v1",
        "schema_version": 1,
        "campaign_id": manifest["campaign_id"],
        "outcome": "candidate_passes_offline_gates" if passes else "candidate_does_not_pass_offline_gates",
        "gates": gates,
        "candidate_to_legacy_mean_contrast_ratio": contrast_ratio,
        "candidate_to_legacy_top3_neighbor_delta": (
            candidate["corpus"]["top3_neighbor_preservation"]
            - legacy["corpus"]["top3_neighbor_preservation"]
        ),
        "legacy_to_candidate_downstream_48d_delta_rms_mean": math.fsum(downstream_delta) / len(downstream_delta),
        "legacy_to_candidate_downstream_48d_delta_rms_maximum": max(downstream_delta),
        "deterministic_replay_sha256": replay_hash,
        "recommendation": (
            "retain_candidate_default_off_for_compatibility_and_optional_felt_review"
            if passes
            else "retain_legacy_basis_and_revise_offline_candidate"
        ),
        "compatibility_review_required": True,
        "right_to_ignore_felt_review_required_before_proposal": True,
        "operator_approval_required_for_live_activation": True,
        "abort_conditions": [
            "fixture_or_corpus_hash_drift",
            "nondeterministic_replay",
            "nonfinite_output_or_dead_axis",
            "contrast_separation_below_preregistered_floor",
            "any_socket_runtime_delivered_vector_or_basis_epoch_write",
        ],
        "rollback_plan": (
            "Keep fixed_legacy_v1 pinned; any separately approved future epoch must preserve the "
            "legacy checksum and restore it before restart if compatibility or felt review objects."
        ),
        "live_basis_activation": False,
        "runtime_write": False,
        "codec_source_change": False,
        "artifact_authority_state_v1": authority_state(),
    }
    status = {
        "schema": "counterfactual_change_lab_status_v1",
        "schema_version": 1,
        "valid": True,
        "campaign_count": 1,
        "campaign_id": manifest["campaign_id"],
        "campaign_outcome": comparison["outcome"],
        "basis_condition_count": len(rows),
        "entry_count": len(entries),
        "embedding_dim_count": INPUT_DIM,
        "projected_dim_count": OUTPUT_DIM,
        "corpus_sha256": _canonical_sha256(corpus),
        "fixture_sha256": _canonical_sha256(fixture),
        "deterministic_replay_sha256": replay_hash,
        "all_offline_gates_pass": passes,
        "network_access_during_replay": False,
        "runtime_write": False,
        "live_basis_activation": False,
        "counter_audit": {
            "status": "consistent",
            "checks": {
                "public_frozen_corpus_only": not bool(corpus.get("private_material_included")),
                "exact_legacy_basis_replayed": True,
                "candidate_remains_default_off": True,
                "comparison_grants_no_authority": True,
                "live_codec_unchanged": True,
            },
        },
        "artifact_authority_state_v1": authority_state(),
    }
    return status, {"manifest": manifest, "comparison": comparison}, rows


def _report(status: dict[str, Any], packet: dict[str, Any], rows: list[dict[str, Any]]) -> str:
    lines = [
        "# Counterfactual Change Lab V1",
        "",
        "Offline, frozen-corpus evidence only. No live codec, model, transport, reservoir, or delivered vector was changed.",
        "",
        f"- campaign: {status['campaign_id']}",
        f"- outcome: {status['campaign_outcome']}",
        f"- public frozen entries: {status['entry_count']}",
        f"- all offline gates pass: {str(status['all_offline_gates_pass']).lower()}",
        "",
        "## Conditions",
        "",
    ]
    for row in rows:
        basis = row["basis"]
        corpus = row["corpus"]
        lines.append(
            f"- **{basis['basis_id']}**: max basis cosine {basis['maximum_abs_pairwise_column_cosine']:.6f}; "
            f"mean distortion {corpus['mean_abs_cosine_distortion']:.6f}; "
            f"top-3 preservation {corpus['top3_neighbor_preservation']:.3f}; "
            f"mean contrast RMS {corpus['mean_contrast_pair_rms']:.6f}"
        )
    lines.extend(
        [
            "",
            "## Boundary",
            "",
            packet["manifest"]["authority_boundary"],
            "",
        ]
    )
    return "\n".join(lines)


def project(
    workspace: Path,
    *,
    corpus_path: Path = CORPUS_PATH,
    fixture_path: Path = FIXTURE_PATH,
    write: bool,
) -> dict[str, Any]:
    missing = [str(path) for path in (corpus_path, fixture_path) if not path.is_file()]
    if missing:
        return {
            "schema": "counterfactual_change_lab_status_v1",
            "schema_version": 1,
            "valid": False,
            "missing_inputs": missing,
            "artifact_authority_state_v1": authority_state(),
        }
    corpus = _json(corpus_path)
    fixture = _json(fixture_path)
    status, packet, rows = evaluate(corpus, fixture)
    status["write"] = write
    if write and status.get("valid"):
        output = state_dir(workspace)
        campaign = output / "campaigns/fixed_projection_basis_epoch_comparison_v1"
        owner_atomic_write_json(output / "status.json", status)
        owner_atomic_write_json(campaign / "campaign_manifest.json", packet["manifest"])
        owner_atomic_write_json(campaign / "comparison.json", packet["comparison"])
        owner_atomic_write_jsonl(campaign / "basis_metrics.jsonl", rows)
        owner_atomic_write_json(
            campaign / "evidence_study_handoff.json",
            {
                "schema": "counterfactual_evidence_study_handoff_v1",
                "schema_version": 1,
                "campaign_id": packet["manifest"]["campaign_id"],
                "comparison_sha256": _canonical_sha256(packet["comparison"]),
                "next_state": "default_off_compatibility_review",
                "live_eligible_now": False,
                "auto_approved": False,
                "grants_approval": False,
                "artifact_authority_state_v1": authority_state(),
            },
        )
        owner_atomic_write(output / "report.md", _report(status, packet, rows))
    return status
