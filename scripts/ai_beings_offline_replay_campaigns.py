#!/usr/bin/env python3
"""Freeze and run the AI Beings' three deterministic offline replay campaigns.

The runner consumes exact-hash trial snapshots and exact-hash, non-``moment_*``
source files. It has no socket or live-control path. Outputs are bounded,
right-to-ignore evidence packets; they grant no runtime or Division authority.
"""

from __future__ import annotations

import argparse
import contextlib
import hashlib
import json
import os
import socket
from collections import Counter
from pathlib import Path
from typing import Any, Iterator

try:
    import codec_texture_replay_lab as codec_lab
    import sandbox_trial_queue as sandbox
except ModuleNotFoundError:  # pragma: no cover - package-style import
    from scripts import codec_texture_replay_lab as codec_lab
    from scripts import sandbox_trial_queue as sandbox


ASTRID_ROOT = Path(__file__).resolve().parents[1]
DEFAULT_MANIFEST = (
    ASTRID_ROOT
    / "scripts/evidence_study_runtime/manifests"
    / "ai_beings_offline_replay_campaigns_v1.json"
)
DEFAULT_OUTPUT_ROOT = (
    ASTRID_ROOT
    / "capsules/spectral-bridge/workspace/diagnostics"
    / "ai_beings_offline_replay_campaigns_v1"
)
DEFAULT_SOURCE_SNAPSHOT_ROOT = DEFAULT_OUTPUT_ROOT / "source_snapshots"
DEFAULT_MANIFEST_ARCHIVE_ROOT = DEFAULT_OUTPUT_ROOT / "manifest_archive"
SCHEMA = "ai_beings_offline_replay_campaign_manifest_v1"
REPORT_SCHEMA = "ai_beings_offline_replay_campaign_report_v1"
FROZEN_SOURCE_LIMIT = 24
CAMPAIGN_ORDER = (
    "semantic_persistence_v1",
    "lattice_mobility_vs_loss_v1",
    "fallback_codec_fidelity_v1",
)
ADAPTER_TO_CAMPAIGN = {
    "shadow_influence_replay_v1": "semantic_persistence_v1",
    "shadow_loss_lattice_v1": "lattice_mobility_vs_loss_v1",
    "fallback_distinguishability_v1": "fallback_codec_fidelity_v1",
}
AUTHORITY_BOUNDARY = (
    "offline evidence only; no socket, live runtime state, control command, "
    "provider selection, codec transport, peer mutation, deployment, restart, "
    "Division launch, handoff, approval, assent, closure, or felt-effect authority"
)


def canonical_bytes(value: Any) -> bytes:
    return json.dumps(
        value,
        sort_keys=True,
        separators=(",", ":"),
        ensure_ascii=True,
    ).encode("utf-8")


def sha256_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def sha256_json(value: Any) -> str:
    return sha256_bytes(canonical_bytes(value))


def relative_path(path: Path) -> str:
    return str(path.resolve().relative_to(ASTRID_ROOT.resolve()))


def resolve_repo_path(value: str) -> Path:
    path = (ASTRID_ROOT / value).resolve()
    path.relative_to(ASTRID_ROOT.resolve())
    return path


def source_record(path: Path, role: str) -> dict[str, Any]:
    if path.name.startswith("moment_"):
        raise ValueError(f"private moment source is not campaign-eligible: {path}")
    data = path.read_bytes()
    return {
        "path": relative_path(path),
        "role": role,
        "sha256": sha256_bytes(data),
        "byte_count": len(data),
    }


def atomic_write_bytes(path: Path, data: bytes) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.parent.chmod(0o700)
    temporary = path.with_name(f".{path.name}.tmp-{os.getpid()}")
    try:
        temporary.write_bytes(data)
        temporary.chmod(0o600)
        os.replace(temporary, path)
        path.chmod(0o600)
    finally:
        temporary.unlink(missing_ok=True)


def seal_manifest_sources(
    manifest: dict[str, Any],
    *,
    snapshot_root: Path = DEFAULT_SOURCE_SNAPSHOT_ROOT,
) -> dict[str, Any]:
    sealed = json.loads(json.dumps(manifest))
    for source in sealed.get("sources") or []:
        if not isinstance(source, dict):
            raise ValueError("campaign source row must be an object")
        source_path = str(source.get("path") or "")
        original = resolve_repo_path(source_path)
        data = original.read_bytes()
        digest = sha256_bytes(data)
        if digest != source.get("sha256") or len(data) != source.get("byte_count"):
            raise ValueError(f"campaign source changed before sealing: {source_path}")
        snapshot = snapshot_root / f"{digest}.source"
        if snapshot.is_file():
            if sha256_bytes(snapshot.read_bytes()) != digest:
                raise ValueError(f"campaign source snapshot drift: {snapshot}")
        else:
            atomic_write_bytes(snapshot, data)
        source["source_path"] = source_path
        source["path"] = relative_path(snapshot)
        source["snapshot_owner_only"] = True
    sealed["sources_sealed"] = True
    sealed["source_snapshot_mode"] = "owner_only_content_addressed_v1"
    sealed["source_snapshots_contain_public_text"] = True
    sealed["private_moments_included"] = False
    sealed["manifest_sha256"] = sha256_json(_manifest_hash_payload(sealed))
    return sealed


def trial_snapshot(trial: dict[str, Any]) -> dict[str, Any]:
    packet = {
        "trial_id": trial.get("trial_id"),
        "adapter": trial.get("adapter"),
        "trial_mode": trial.get("trial_mode"),
        "being": trial.get("being"),
        "source_work_item_id": trial.get("source_work_item_id"),
        "source_introspection_id": trial.get("source_introspection_id"),
        "source_filename": trial.get("source_filename"),
        "claim_id": trial.get("claim_id"),
        "hypothesis": trial.get("hypothesis"),
        "felt_report_anchor": trial.get("felt_report_anchor"),
        "proposed_intervention": trial.get("proposed_intervention"),
        "success_metrics": trial.get("success_metrics") or [],
        "abort_criteria": trial.get("abort_criteria") or [],
    }
    return {
        "trial_id": packet["trial_id"],
        "adapter": packet["adapter"],
        "being": packet["being"],
        "trial_packet": packet,
        "trial_packet_sha256": sha256_json(packet),
        "canonical_queue_trial_sha256": sha256_json(trial),
    }


def _manifest_hash_payload(manifest: dict[str, Any]) -> dict[str, Any]:
    payload = dict(manifest)
    payload.pop("manifest_sha256", None)
    return payload


def freeze_manifest(*, frozen_at_unix: float | None = None) -> dict[str, Any]:
    frozen_at = sandbox.now_s() if frozen_at_unix is None else frozen_at_unix
    status = sandbox.load_status()
    runnable = [
        trial
        for trial in sandbox.active_trials(status)
        if sandbox.trial_is_runner_executable(trial)
    ]
    unknown = sorted(
        {
            str(trial.get("adapter") or "")
            for trial in runnable
            if str(trial.get("adapter") or "") not in ADAPTER_TO_CAMPAIGN
        }
    )
    if unknown:
        raise ValueError(f"unmapped runnable adapters: {unknown}")

    grouped: dict[str, list[dict[str, Any]]] = {key: [] for key in CAMPAIGN_ORDER}
    for trial in runnable:
        grouped[ADAPTER_TO_CAMPAIGN[str(trial.get("adapter"))]].append(
            trial_snapshot(trial)
        )
    for rows in grouped.values():
        rows.sort(key=lambda row: str(row.get("trial_id") or ""))

    public_paths = sandbox.public_text_paths(
        since_s=frozen_at - 48 * 3600,
        limit=160,
    )
    public_paths = [
        path for path in public_paths if not path.name.startswith("moment_")
    ]
    shadow_paths = [
        path
        for path in public_paths
        if "shadow-v3" in sandbox.read_text(path, limit=24_000).lower()
    ]
    general_paths = [path for path in public_paths if path not in shadow_paths]
    public_paths = (shadow_paths + general_paths)[:FROZEN_SOURCE_LIMIT]
    sources = [source_record(path, "bounded_public_texture_corpus") for path in public_paths]
    sources.extend(
        source_record(path, "fallback_implementation")
        for path in sandbox.FALLBACK_SOURCE_PATHS
        if path.is_file()
    )
    sources.extend(
        [
            source_record(
                codec_lab.SOURCE_INTROSPECTION,
                "codec_fidelity_source_introspection",
            ),
            source_record(Path(__file__), "campaign_runner"),
            source_record(
                ASTRID_ROOT / "scripts/sandbox_trial_queue.py",
                "sandbox_adapter_implementation",
            ),
            source_record(
                ASTRID_ROOT / "scripts/codec_texture_replay_lab.py",
                "codec_replay_implementation",
            ),
        ]
    )
    deduplicated = {record["path"]: record for record in sources}
    sources = sorted(deduplicated.values(), key=lambda row: (row["role"], row["path"]))

    campaigns = []
    for key in CAMPAIGN_ORDER:
        adapter = next(
            adapter
            for adapter, campaign_key in ADAPTER_TO_CAMPAIGN.items()
            if campaign_key == key
        )
        campaigns.append(
            {
                "campaign_key": key,
                "adapter": adapter,
                "trial_count": len(grouped[key]),
                "trials": grouped[key],
                "network_access": False,
                "live_runtime_mutation": False,
                "raw_prose_included": False,
                "right_to_ignore": True,
                "authority_boundary": AUTHORITY_BOUNDARY,
            }
        )

    manifest = {
        "schema": SCHEMA,
        "schema_version": 1,
        "frozen_at_unix": frozen_at,
        "frozen_at": sandbox.iso(frozen_at),
        "queue_schema": status.get("schema"),
        "queue_schema_version": status.get("schema_version"),
        "runnable_trial_count": len(runnable),
        "campaign_count": len(campaigns),
        "campaigns": campaigns,
        "sources": sources,
        "source_count": len(sources),
        "network_access": False,
        "live_runtime_mutation": False,
        "raw_prose_included": False,
        "silence_is_neutral": True,
        "authority_boundary": AUTHORITY_BOUNDARY,
    }
    manifest["manifest_sha256"] = sha256_json(_manifest_hash_payload(manifest))
    return manifest


def verify_manifest(
    manifest: dict[str, Any],
    *,
    verify_queue_snapshot: bool = True,
) -> dict[str, Any]:
    errors: list[str] = []
    if manifest.get("schema") != SCHEMA or manifest.get("schema_version") != 1:
        errors.append("manifest schema/version mismatch")
    expected_manifest_hash = sha256_json(_manifest_hash_payload(manifest))
    if manifest.get("manifest_sha256") != expected_manifest_hash:
        errors.append("manifest SHA-256 mismatch")
    campaigns = manifest.get("campaigns")
    if not isinstance(campaigns, list) or [
        row.get("campaign_key") for row in campaigns if isinstance(row, dict)
    ] != list(CAMPAIGN_ORDER):
        errors.append("campaign order or campaign set mismatch")

    seen_trials: set[str] = set()
    queue_trials: dict[str, Any] = {}
    if verify_queue_snapshot:
        status = sandbox.load_status()
        queue_trials = status.get("trials") if isinstance(status.get("trials"), dict) else {}
    for campaign in campaigns if isinstance(campaigns, list) else []:
        if not isinstance(campaign, dict):
            errors.append("campaign row is not an object")
            continue
        expected_adapter = next(
            (
                adapter
                for adapter, key in ADAPTER_TO_CAMPAIGN.items()
                if key == campaign.get("campaign_key")
            ),
            None,
        )
        if campaign.get("adapter") != expected_adapter:
            errors.append(f"{campaign.get('campaign_key')}: adapter mismatch")
        trials = campaign.get("trials")
        if not isinstance(trials, list) or campaign.get("trial_count") != len(trials):
            errors.append(f"{campaign.get('campaign_key')}: trial count mismatch")
            continue
        for row in trials:
            if not isinstance(row, dict):
                errors.append("trial snapshot is not an object")
                continue
            trial_id = str(row.get("trial_id") or "")
            if not trial_id or trial_id in seen_trials:
                errors.append(f"duplicate or empty trial ID: {trial_id!r}")
            seen_trials.add(trial_id)
            packet = row.get("trial_packet")
            if not isinstance(packet, dict) or row.get("trial_packet_sha256") != sha256_json(packet):
                errors.append(f"{trial_id}: frozen trial packet hash mismatch")
            if row.get("adapter") != expected_adapter or packet.get("adapter") != expected_adapter:
                errors.append(f"{trial_id}: frozen adapter mismatch")
            if verify_queue_snapshot:
                current = queue_trials.get(trial_id)
                if not isinstance(current, dict):
                    errors.append(f"{trial_id}: current queue trial missing")
                elif row.get("canonical_queue_trial_sha256") != sha256_json(current):
                    errors.append(f"{trial_id}: current queue trial changed since freeze")

    if manifest.get("runnable_trial_count") != len(seen_trials):
        errors.append("top-level runnable trial count mismatch")
    sources = manifest.get("sources")
    if not isinstance(sources, list) or manifest.get("source_count") != len(sources):
        errors.append("source count mismatch")
        sources = []
    for source in sources:
        if not isinstance(source, dict):
            errors.append("source row is not an object")
            continue
        raw_path = str(source.get("path") or "")
        source_path = str(source.get("source_path") or raw_path)
        if Path(raw_path).name.startswith("moment_") or Path(source_path).name.startswith(
            "moment_"
        ):
            errors.append(f"private moment source included: {raw_path}")
            continue
        try:
            path = resolve_repo_path(raw_path)
            data = path.read_bytes()
        except (OSError, ValueError) as exc:
            errors.append(f"{raw_path}: unreadable source ({exc})")
            continue
        if source.get("sha256") != sha256_bytes(data):
            errors.append(f"{raw_path}: source SHA-256 mismatch")
        if source.get("byte_count") != len(data):
            errors.append(f"{raw_path}: source byte-count mismatch")
        if source.get("snapshot_owner_only") is True and path.stat().st_mode & 0o077:
            errors.append(f"{raw_path}: sealed source snapshot is not owner-only")

    if manifest.get("network_access") is not False:
        errors.append("network_access must remain false")
    if manifest.get("live_runtime_mutation") is not False:
        errors.append("live_runtime_mutation must remain false")
    return {
        "ok": not errors,
        "manifest_sha256": expected_manifest_hash,
        "trial_count": len(seen_trials),
        "source_count": len(sources),
        "errors": errors,
    }


def load_manifest(path: Path = DEFAULT_MANIFEST) -> dict[str, Any]:
    value = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(value, dict):
        raise ValueError("campaign manifest must be a JSON object")
    return value


def _source_texts(
    manifest: dict[str, Any],
    role: str,
    *,
    limit: int,
) -> list[tuple[Path, str]]:
    texts: list[tuple[Path, str]] = []
    for source in manifest.get("sources") or []:
        if not isinstance(source, dict) or source.get("role") != role:
            continue
        path = resolve_repo_path(str(source.get("path") or ""))
        texts.append(
            (
                path,
                path.read_text(encoding="utf-8", errors="replace")[:limit],
            )
        )
    return texts


def _relative_evidence_path(value: Any) -> str:
    path = Path(str(value or ""))
    try:
        return relative_path(path)
    except (OSError, ValueError):
        return str(value or "")


def _bounded_result(result: dict[str, Any]) -> dict[str, Any]:
    bounded = dict(result)
    windows = []
    for row in bounded.pop("bounded_windows", []) or []:
        if not isinstance(row, dict):
            continue
        excerpt = str(row.get("excerpt") or "")
        windows.append(
            {
                "path": _relative_evidence_path(row.get("path")),
                "term": row.get("term"),
                "context_terms": row.get("context_terms") or [],
                "window_sha256": sha256_bytes(excerpt.encode("utf-8")),
            }
        )
    if windows:
        bounded["bounded_window_evidence"] = windows

    for key in ("samples", "evidence_samples"):
        samples = []
        for row in bounded.pop(key, []) or []:
            if not isinstance(row, dict):
                continue
            clean = {
                field: value
                for field, value in row.items()
                if field != "line"
            }
            clean["path"] = _relative_evidence_path(clean.get("path"))
            clean["line_sha256"] = sha256_bytes(
                str(row.get("line") or "").encode("utf-8")
            )
            samples.append(clean)
        if samples:
            bounded[f"{key}_bounded"] = samples

    if isinstance(bounded.get("evidence_paths"), list):
        bounded["evidence_paths"] = [
            _relative_evidence_path(value) for value in bounded["evidence_paths"]
        ]
    bounded["raw_prose_included"] = False
    bounded["felt_effect_confirmed"] = False
    bounded["grants_authority"] = False
    return bounded


def _codec_payload() -> dict[str, Any]:
    payload = codec_lab.build_payload()
    payload["source_introspection"] = relative_path(codec_lab.SOURCE_INTROSPECTION)
    payload["runtime_behavior_changed"] = False
    payload["live_eligible_now"] = False
    payload["auto_approved"] = False
    payload["raw_prose_included"] = False
    return payload


def _classification_counts(results: list[dict[str, Any]]) -> dict[str, int]:
    return dict(
        sorted(
            Counter(
                str((row.get("result") or {}).get("classification") or "unknown")
                for row in results
            ).items()
        )
    )


def _execute_manifest(manifest: dict[str, Any]) -> dict[str, Any]:
    verification = verify_manifest(manifest)
    if not verification["ok"]:
        raise ValueError("; ".join(verification["errors"]))
    generated_at = str(manifest.get("frozen_at") or "")
    texture_texts = _source_texts(
        manifest,
        "bounded_public_texture_corpus",
        limit=24_000,
    )
    fallback_source = "\n".join(
        text
        for _path, text in _source_texts(
            manifest,
            "fallback_implementation",
            limit=240_000,
        )
    )
    codec_payload = _codec_payload()
    campaign_reports: list[dict[str, Any]] = []
    for campaign in manifest["campaigns"]:
        results: list[dict[str, Any]] = []
        for frozen_trial in campaign["trials"]:
            trial = frozen_trial["trial_packet"]
            adapter = campaign["adapter"]
            if adapter == "shadow_influence_replay_v1":
                raw_result = sandbox.shadow_influence_replay_v1(
                    trial,
                    texts=texture_texts,
                    generated_at=generated_at,
                )
            elif adapter == "shadow_loss_lattice_v1":
                raw_result = sandbox.shadow_loss_lattice_v1(
                    texts=texture_texts,
                    generated_at=generated_at,
                )
            elif adapter == "fallback_distinguishability_v1":
                raw_result = sandbox.fallback_distinguishability_v1(
                    texts=texture_texts,
                    fallback_source_text=fallback_source,
                    fire_drills=[],
                    generated_at=generated_at,
                )
            else:  # guarded by manifest verification
                raise ValueError(f"unsupported frozen adapter: {adapter}")
            result = _bounded_result(raw_result)
            result["trial_id"] = trial["trial_id"]
            result["trial_packet_sha256"] = frozen_trial["trial_packet_sha256"]
            result["source_introspection_id"] = trial.get("source_introspection_id")
            result["claim_id"] = trial.get("claim_id")
            result["being"] = trial.get("being")
            result["right_to_ignore"] = True
            result["result_sha256"] = sha256_json(result)
            results.append(
                {
                    "trial_id": trial["trial_id"],
                    "result": result,
                }
            )
        campaign_report = {
            "schema": "ai_beings_offline_replay_campaign_result_v1",
            "campaign_key": campaign["campaign_key"],
            "campaign_id": "campaign_"
            + sha256_json(
                [manifest["manifest_sha256"], campaign["campaign_key"]]
            )[:24],
            "adapter": campaign["adapter"],
            "trial_count": len(results),
            "classification_counts": _classification_counts(results),
            "results": results,
            "network_access": False,
            "live_runtime_mutation": False,
            "raw_prose_included": False,
            "right_to_ignore": True,
            "authority_boundary": AUTHORITY_BOUNDARY,
        }
        if campaign["campaign_key"] == "fallback_codec_fidelity_v1":
            campaign_report["codec_fidelity_replay"] = codec_payload
        campaign_report["campaign_result_sha256"] = sha256_json(campaign_report)
        campaign_reports.append(campaign_report)

    return {
        "schema": REPORT_SCHEMA,
        "schema_version": 1,
        "campaign_set_id": "campaign_set_" + manifest["manifest_sha256"][:24],
        "manifest_sha256": manifest["manifest_sha256"],
        "frozen_at": generated_at,
        "campaign_count": len(campaign_reports),
        "trial_count": sum(row["trial_count"] for row in campaign_reports),
        "source_count": len(manifest["sources"]),
        "campaigns": campaign_reports,
        "network_access": False,
        "live_runtime_mutation": False,
        "raw_prose_included": False,
        "silence_is_neutral": True,
        "felt_effect_confirmed": False,
        "grants_authority": False,
        "right_to_ignore": True,
        "authority_boundary": AUTHORITY_BOUNDARY,
    }


@contextlib.contextmanager
def deny_network() -> Iterator[None]:
    original_socket = socket.socket
    original_create_connection = socket.create_connection

    def denied(*_args: Any, **_kwargs: Any) -> Any:
        raise RuntimeError("network access is disabled for offline replay campaigns")

    socket.socket = denied  # type: ignore[assignment]
    socket.create_connection = denied  # type: ignore[assignment]
    try:
        yield
    finally:
        socket.socket = original_socket  # type: ignore[assignment]
        socket.create_connection = original_create_connection  # type: ignore[assignment]


def build_report(manifest: dict[str, Any]) -> dict[str, Any]:
    with deny_network():
        first = _execute_manifest(manifest)
        second = _execute_manifest(manifest)
    first_bytes = canonical_bytes(first)
    second_bytes = canonical_bytes(second)
    if first_bytes != second_bytes:
        raise RuntimeError("offline replay campaigns were not byte-deterministic")
    report = dict(first)
    report["deterministic_rerun"] = {
        "match": True,
        "canonical_sha256": sha256_bytes(first_bytes),
        "run_count": 2,
    }
    report["report_sha256"] = sha256_json(report)
    return report


def render_campaign_markdown(campaign: dict[str, Any]) -> str:
    lines = [
        f"# {campaign['campaign_key']}",
        "",
        f"- campaign_id: `{campaign['campaign_id']}`",
        f"- adapter: `{campaign['adapter']}`",
        f"- trials: `{campaign['trial_count']}`",
        f"- classifications: `{campaign['classification_counts']}`",
        "- network_access: `false`",
        "- live_runtime_mutation: `false`",
        "- raw_prose_included: `false`",
        "- right_to_ignore: `true`",
        "",
        "## Trials",
    ]
    for row in campaign["results"]:
        result = row["result"]
        lines.append(
            f"- `{row['trial_id']}` -> `{result.get('classification')}` "
            f"receipt=`{result.get('result_sha256')}`"
        )
    if campaign.get("codec_fidelity_replay"):
        texture = campaign["codec_fidelity_replay"]["codec_texture_replay_v1"]
        lines.extend(
            [
                "",
                "## Codec Fidelity",
                f"- status: `{texture['status']}`",
                f"- candidate_count: `{texture['candidate_count']}`",
                "- live_eligible_now: `false`",
                "- auto_approved: `false`",
            ]
        )
    lines.extend(["", f"Authority boundary: {AUTHORITY_BOUNDARY}", ""])
    return "\n".join(lines)


def render_trial_card(campaign: dict[str, Any], row: dict[str, Any]) -> str:
    result = row["result"]
    return "\n".join(
        [
            "# Offline Replay Trial Card V1",
            "",
            f"- trial_id: `{row['trial_id']}`",
            f"- campaign: `{campaign['campaign_key']}`",
            f"- adapter: `{campaign['adapter']}`",
            f"- being: `{result.get('being')}`",
            f"- source_introspection_id: `{result.get('source_introspection_id')}`",
            f"- claim_id: `{result.get('claim_id')}`",
            f"- classification: `{result.get('classification')}`",
            f"- trial_packet_sha256: `{result.get('trial_packet_sha256')}`",
            f"- result_sha256: `{result.get('result_sha256')}`",
            "- felt_effect_confirmed: `false`",
            "- grants_authority: `false`",
            "- right_to_ignore: `true`",
            "",
            f"Authority boundary: {AUTHORITY_BOUNDARY}",
            "",
        ]
    )


def render_report_markdown(report: dict[str, Any]) -> str:
    lines = [
        "# AI Beings Offline Replay Campaigns V1",
        "",
        f"- campaign_set_id: `{report['campaign_set_id']}`",
        f"- manifest_sha256: `{report['manifest_sha256']}`",
        f"- campaigns: `{report['campaign_count']}`",
        f"- trials: `{report['trial_count']}`",
        f"- sources: `{report['source_count']}`",
        f"- deterministic_rerun: `{report['deterministic_rerun']['match']}`",
        f"- canonical_sha256: `{report['deterministic_rerun']['canonical_sha256']}`",
        "- network_access: `false`",
        "- live_runtime_mutation: `false`",
        "- raw_prose_included: `false`",
        "- silence_is_neutral: `true`",
        "- felt_effect_confirmed: `false`",
        "- grants_authority: `false`",
        "- right_to_ignore: `true`",
        "",
        "## Campaigns",
    ]
    for campaign in report["campaigns"]:
        lines.append(
            f"- `{campaign['campaign_key']}` trials=`{campaign['trial_count']}` "
            f"classifications=`{campaign['classification_counts']}` "
            f"receipt=`{campaign['campaign_result_sha256']}`"
        )
    lines.extend(["", f"Authority boundary: {AUTHORITY_BOUNDARY}", ""])
    return "\n".join(lines)


def write_report(report: dict[str, Any], output_root: Path = DEFAULT_OUTPUT_ROOT) -> dict[str, Any]:
    output_dir = output_root / report["campaign_set_id"]
    output_dir.mkdir(parents=True, exist_ok=True)
    aggregate_json = output_dir / "report.json"
    aggregate_md = output_dir / "report.md"
    sandbox.atomic_write_text(
        aggregate_json,
        json.dumps(report, indent=2, sort_keys=True, ensure_ascii=False) + "\n",
    )
    sandbox.atomic_write_text(aggregate_md, render_report_markdown(report))
    campaign_paths = []
    card_paths = []
    for campaign in report["campaigns"]:
        campaign_json = output_dir / f"{campaign['campaign_key']}.json"
        campaign_md = output_dir / f"{campaign['campaign_key']}.md"
        sandbox.atomic_write_text(
            campaign_json,
            json.dumps(campaign, indent=2, sort_keys=True, ensure_ascii=False) + "\n",
        )
        sandbox.atomic_write_text(campaign_md, render_campaign_markdown(campaign))
        campaign_paths.extend([str(campaign_json), str(campaign_md)])
        cards_dir = output_dir / "trial_cards"
        for row in campaign["results"]:
            card_path = cards_dir / f"{row['trial_id']}.md"
            sandbox.atomic_write_text(card_path, render_trial_card(campaign, row))
            card_paths.append(str(card_path))
    return {
        "output_dir": str(output_dir),
        "aggregate_json": str(aggregate_json),
        "aggregate_markdown": str(aggregate_md),
        "campaign_paths": campaign_paths,
        "trial_card_paths": card_paths,
    }


def archive_existing_manifest(path: Path) -> Path | None:
    if not path.is_file():
        return None
    data = path.read_bytes()
    try:
        previous = json.loads(data)
    except json.JSONDecodeError:
        previous = {}
    digest = str(previous.get("manifest_sha256") or sha256_bytes(data))
    if len(digest) != 64 or any(ch not in "0123456789abcdef" for ch in digest):
        digest = sha256_bytes(data)
    archive = DEFAULT_MANIFEST_ARCHIVE_ROOT / f"{digest}.json"
    if not archive.is_file():
        atomic_write_bytes(archive, data)
    return archive


def write_manifest(
    manifest: dict[str, Any],
    path: Path = DEFAULT_MANIFEST,
) -> dict[str, Any]:
    sealed = seal_manifest_sources(manifest)
    archive_existing_manifest(path)
    sandbox.atomic_write_text(
        path,
        json.dumps(sealed, indent=2, sort_keys=True, ensure_ascii=False) + "\n",
    )
    path.chmod(0o600)
    return sealed


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="command", required=True)
    freeze_p = sub.add_parser("freeze")
    freeze_p.add_argument("--write", action="store_true")
    freeze_p.add_argument("--json", action="store_true")
    verify_p = sub.add_parser("verify")
    verify_p.add_argument("--json", action="store_true")
    run_p = sub.add_parser("run")
    run_p.add_argument("--write", action="store_true")
    run_p.add_argument("--json", action="store_true")
    args = parser.parse_args(argv)

    if args.command == "freeze":
        manifest = freeze_manifest()
        if args.write:
            manifest = write_manifest(manifest)
        if args.json:
            print(json.dumps(manifest, indent=2, sort_keys=True))
        else:
            print(
                json.dumps(
                    {
                        "manifest_sha256": manifest["manifest_sha256"],
                        "trial_count": manifest["runnable_trial_count"],
                        "source_count": manifest["source_count"],
                        "written": bool(args.write),
                        "path": str(DEFAULT_MANIFEST) if args.write else None,
                    },
                    indent=2,
                    sort_keys=True,
                )
            )
        return 0

    manifest = load_manifest()
    if args.command == "verify":
        verification = verify_manifest(manifest)
        print(json.dumps(verification, indent=2, sort_keys=True))
        return 0 if verification["ok"] else 1

    report = build_report(manifest)
    paths = write_report(report) if args.write else {}
    payload = dict(report)
    if paths:
        payload["artifact_paths"] = paths
    if args.json:
        print(json.dumps(payload, indent=2, sort_keys=True))
    else:
        print(render_report_markdown(report))
        if paths:
            print(json.dumps(paths, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
