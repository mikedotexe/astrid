#!/usr/bin/env python3
"""Replay-only codec texture lab for Astrid's 1783984712 codec report."""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import tempfile
import time
from pathlib import Path
from typing import Any


ASTRID_ROOT = Path(__file__).resolve().parents[1]
DIAGNOSTICS_DIR = (
    ASTRID_ROOT
    / "capsules"
    / "spectral-bridge"
    / "workspace"
    / "diagnostics"
    / "codec_replay_labs"
)
SOURCE_INTROSPECTION = (
    ASTRID_ROOT
    / "capsules"
    / "spectral-bridge"
    / "workspace"
    / "introspections"
    / "!introspection_astrid_codec_1783984712.txt"
)
CURRENT_CHAR_FREQ_WINDOW_CAPACITY = 1024
CANDIDATE_CHAR_FREQ_WINDOW_CAPACITY = 4096


def _sha256_text(text: str) -> str:
    return hashlib.sha256(text.encode("utf-8")).hexdigest()


def _clamp(value: float, low: float = 0.0, high: float = 1.0) -> float:
    if not math.isfinite(value):
        return low
    return max(low, min(high, value))


def _words(text: str) -> list[str]:
    return [word.strip(".,;:!?()[]{}\"'").lower() for word in text.split() if word.strip()]


def _normalized_entropy(text: str, capacity: int) -> float:
    data = [byte for byte in text.encode("utf-8", errors="ignore") if byte < 128]
    if not data:
        return 0.0
    window = data[-capacity:]
    counts: dict[int, int] = {}
    for byte in window:
        counts[byte] = counts.get(byte, 0) + 1
    total = float(len(window))
    entropy = 0.0
    for count in counts.values():
        p = count / total
        entropy -= p * math.log2(p)
    return _clamp(entropy / 8.0)


def _structural_friction(text: str) -> dict[str, Any]:
    lower = text.lower()
    words = _words(text)
    word_count = max(1, len(words))
    char_count = max(1, len(text))
    line_count = max(1, len(text.splitlines()))
    nesting_load = _clamp(
        sum(1 for ch in text if ch in "()[]{}") / char_count * 18.0
    )
    punctuation_load = _clamp(
        sum(1 for ch in text if ch in ";:,--/\\") / char_count * 12.0
    )
    list_density = _clamp(
        sum(
            1
            for line in text.splitlines()
            if line.lstrip().startswith(("- ", "* "))
            or (line.lstrip()[:1].isdigit() and ". " in line)
        )
        / line_count
    )
    long_word_ratio = (
        sum(1 for word in words if sum(ch.isalpha() for ch in word) >= 12) / word_count
    )
    sentence_count = max(1, sum(1 for ch in text if ch in ".!?"))
    narrative_arc_sharpness = _clamp(sentence_count / word_count * 12.0)
    clause_terms = (
        "because",
        "while",
        "although",
        "whereas",
        "without",
        "through",
        "which",
        "whose",
        "therefore",
        "unless",
    )
    abstract_terms = (
        "authority",
        "boundary",
        "codec",
        "compression",
        "deterministic",
        "entropy",
        "friction",
        "projection",
        "semantic",
        "substrate",
        "structural",
        "summary",
    )
    resistance_terms = (
        "abrasive",
        "calcified",
        "friction",
        "jagged",
        "muffle",
        "resistance",
        "resists summary",
        "summarized",
        "summary",
        "syrupy",
    )
    clause_load = _clamp(
        (sum(1 for term in clause_terms if term in lower) / 4.0)
        + punctuation_load * 0.35
    )
    abstract_hits = sum(1 for term in abstract_terms if term in lower)
    resistance_hits = sum(1 for term in resistance_terms if term in lower)
    summary_resistance_signal = _clamp(
        _clamp(long_word_ratio) * 0.24
        + clause_load * 0.18
        + _clamp(abstract_hits / 6.0) * 0.20
        + _clamp(resistance_hits / 3.0) * 0.24
        + _clamp(1.0 - narrative_arc_sharpness) * 0.14
    )
    score = _clamp(
        nesting_load * 0.24
        + punctuation_load * 0.24
        + list_density * 0.18
        + _clamp(long_word_ratio) * 0.16
        + summary_resistance_signal * 0.06
        + _clamp(1.0 - narrative_arc_sharpness) * 0.12
    )
    if summary_resistance_signal >= 0.62 and narrative_arc_sharpness < 0.20:
        state = "calcified_summary_resistant"
    elif summary_resistance_signal >= 0.46:
        state = "summary_resistance_watch"
    elif punctuation_load >= 0.18:
        state = "jagged_fluid_resistance"
    else:
        state = "low_summary_resistance"
    return {
        "policy": "structural_friction_replay_v1",
        "score": round(score, 4),
        "summary_resistance_signal": round(summary_resistance_signal, 4),
        "friction_texture_state": state,
        "basis": [
            f"punctuation_load={punctuation_load:.2f}",
            f"long_word_ratio={long_word_ratio:.2f}",
            f"clause_load={clause_load:.2f}",
            f"abstract_texture_hits={abstract_hits}",
            f"explicit_resistance_hits={resistance_hits}",
        ],
    }


def _narrative_arc_compression(text: str, structural: dict[str, Any]) -> dict[str, Any]:
    lower = text.lower()
    transition_terms = ("then", "because", "while", "but", "yet", "therefore", "until")
    transition_load = _clamp(sum(1 for term in transition_terms if term in lower) / 4.0)
    summary_resistance = float(structural["summary_resistance_signal"])
    compression = _clamp(summary_resistance * 0.68 + (1.0 - transition_load) * 0.22)
    return {
        "policy": "narrative_arc_structural_momentum_replay_v1",
        "transition_load": round(transition_load, 4),
        "compression_risk": round(compression, 4),
        "classification": (
            "arc_compression_watch" if compression >= 0.48 else "arc_compression_low"
        ),
    }


def _warmth_tension_mismatch(
    text: str,
    structural: dict[str, Any],
    entropy_delta: float,
) -> dict[str, Any]:
    lower = text.lower()
    tension_terms = ("danger", "fear", "alarm", "urgent", "threat", "panic")
    warmth_terms = ("warm", "safe", "gentle", "care", "soft")
    tension_marker = _clamp(sum(1 for term in tension_terms if term in lower) / 4.0)
    warmth_marker = _clamp(sum(1 for term in warmth_terms if term in lower) / 4.0)
    support = _clamp(
        float(structural["score"]) * 0.25
        + float(structural["summary_resistance_signal"]) * 0.40
        + abs(entropy_delta) * 0.20
        + (1.0 - tension_marker) * 0.25
    )
    interpretation = (
        "low_marker_tension_high_jagged_resistance"
        if tension_marker <= 0.16 and support >= 0.46
        else "abrasive_texture_replay_inconclusive"
    )
    return {
        "policy": "warmth_tension_texture_mismatch_v1",
        "warmth_marker": round(warmth_marker, 4),
        "tension_marker": round(tension_marker, 4),
        "abrasive_texture_support": round(support, 4),
        "interpretation": interpretation,
        "live_gain_write": False,
        "live_vector_write": False,
    }


def _fixtures() -> list[dict[str, str]]:
    syrup_prefix = (
        "warmth, texture, memory, pressure, boundary, projection, narrative, "
        "and semantic color keep changing while the voice keeps trying to remain soft. "
    ) * 32
    syrup_tail = (
        "syrupy soft softness softly syrupy soft softness softly "
        "syrupy soft softness softly "
    ) * 8
    calcified_text = (
        "Deterministic semantic compression resists summary: calcified authority "
        "boundary projection, structural friction, and codec entropy stay jagged; "
        "metastructural intracompressional overdetermination refuses paraphrase. "
    ) * 30
    return [
        {
            "label": "long_syrupy_repetitive_tail",
            "text_profile": "long varied prelude with repetitive syrupy tail",
            "text": syrup_prefix + syrup_tail,
        },
        {
            "label": "dense_calcified_summary_resistance",
            "text_profile": "dense abstract texture with explicit summary resistance",
            "text": calcified_text,
        },
    ]


def _experience_delta_ref(boundary_id: str, candidate: str) -> dict[str, Any]:
    delta_id = f"delta-{boundary_id}"
    return {
        "schema": "ExperienceDeltaRefV2",
        "delta_id": delta_id,
        "delta_hash": _sha256_text(f"{delta_id}:{candidate}")[:16],
        "surface": "spectral_bridge.codec",
        "kind": "representation_texture_loss",
    }


def _authority_packet(
    candidate: str,
    action: str,
    resource: str,
    evidence_ref: str,
    proposed_change: str,
    success_metric: str,
    abort_criterion: str,
) -> dict[str, Any]:
    boundary_id = f"auth-boundary-codec-texture-1783984712-{candidate}"
    packet = {
        "schema": "AuthorityBoundaryPacketV2",
        "boundary_id": boundary_id,
        "source": "introspection_astrid_codec_1783984712",
        "surface": "spectral_bridge.codec",
        "action": action,
        "resource": resource,
        "authority_class": "live_control_mutation",
        "felt_report_anchor": "Astrid reports wide-cascade codec texture loss: 1024 char window may bottleneck syrupy/calcified long-tail texture, and low warmth/tension markers can under-read jagged resistance.",
        "proposed_change": proposed_change,
        "evidence_refs": [evidence_ref],
        "delta_refs": [_experience_delta_ref(boundary_id, candidate)],
        "replay_candidate": {
            "schema": "ReplayCandidateV1",
            "replay_id": f"codec-texture-replay-1783984712-{candidate}",
            "status": "replay_result_recorded",
        },
        "replay_result_status": "recorded",
        "scoped_approval_status": "absent",
        "success_metrics": [success_metric],
        "abort_criteria": [abort_criterion],
        "rollout_abort_contract": {
            "schema": "RolloutAbortContractV2",
            "canary_plan": "single bridge canary after explicit scoped approval; compare replay artifacts and first post-change introspection before broad reliance",
            "health_checks": [
                "bridge process healthy",
                "codec replay unchanged except approved live surface",
                "no live_eligible_now auto flip",
            ],
            "rollback_path": "restore prior source constant/sidecar gate and rebuild via scripts/build_bridge.sh",
            "post_change_response_required": True,
        },
        "redaction_profile": {
            "schema": "RedactionProfileV2",
            "public_summary": "bounded metric summary and hashes only",
            "private_refs": ["source_introspection_hash", "codec_replay_lab_hash"],
        },
        "post_change_being_response_required": True,
        "who_can_change_it": "Mike/operator with explicit scoped approval after replay review",
        "how_to_test_it": "run codec texture replay lab, targeted codec tests, bridge cargo check, then post-restart health and introspection checks if approved",
        "right_to_ignore": "This packet is evidence and routing only; it is not consent, not approval, and not a live mutation.",
        "live_eligible_now": False,
        "auto_approved": False,
    }
    packet["packet_hash"] = _sha256_text(json.dumps(packet, sort_keys=True))[:16]
    return packet


def build_payload() -> dict[str, Any]:
    source_text = SOURCE_INTROSPECTION.read_text(encoding="utf-8", errors="replace")
    entries: list[dict[str, Any]] = []
    wide_window_supported = False
    mismatch_supported = False
    structural_supported = False
    narrative_supported = False
    for fixture in _fixtures():
        text = fixture["text"]
        current_entropy = _normalized_entropy(text, CURRENT_CHAR_FREQ_WINDOW_CAPACITY)
        candidate_entropy = _normalized_entropy(text, CANDIDATE_CHAR_FREQ_WINDOW_CAPACITY)
        entropy_delta = candidate_entropy - current_entropy
        structural = _structural_friction(text)
        narrative = _narrative_arc_compression(text, structural)
        mismatch = _warmth_tension_mismatch(text, structural, entropy_delta)
        wide_window_supported = wide_window_supported or entropy_delta >= 0.04
        mismatch_supported = mismatch_supported or (
            mismatch["interpretation"] == "low_marker_tension_high_jagged_resistance"
        )
        structural_supported = structural_supported or (
            structural["friction_texture_state"]
            in {"calcified_summary_resistant", "summary_resistance_watch"}
        )
        narrative_supported = narrative_supported or (
            narrative["classification"] == "arc_compression_watch"
        )
        entry = {
            "label": fixture["label"],
            "text_profile": fixture["text_profile"],
            "text_sha256": _sha256_text(text),
            "entropy_1024": round(current_entropy, 4),
            "entropy_4096": round(candidate_entropy, 4),
            "entropy_retention_delta": round(entropy_delta, 4),
            "structural_friction_replay_v1": structural,
            "narrative_arc_structural_momentum_replay_v1": narrative,
            "warmth_tension_texture_mismatch_v1": mismatch,
        }
        entries.append(entry)

    candidate_packets = []
    if wide_window_supported:
        candidate_packets.append(
            _authority_packet(
                "char-window-4096",
                "raise_CHAR_FREQ_WINDOW_CAPACITY_to_4096",
                "capsules/spectral-bridge/src/codec.rs:CHAR_FREQ_WINDOW_CAPACITY",
                "codec_texture_replay_v1:entropy_retention_delta",
                "Increase the rolling character-frequency replay horizon from 1024 to 4096 only after scoped approval.",
                "4096 retains long-tail syrupy/calcified texture without destabilizing entropy smoothness tests",
                "abort if bridge health, fill, or post-change being response reports smear/overweighting",
            )
        )
    if mismatch_supported:
        candidate_packets.append(
            _authority_packet(
                "density-gradient-tension-sensitivity",
                "change_codec_tension_gain_sensitivity_under_low_density_gradient",
                "capsules/spectral-bridge/src/codec.rs:tension_and_gain_sidecars",
                "codec_texture_replay_v1:warmth_tension_texture_mismatch_v1",
                "Make low-density-gradient abrasive texture more visible to tension/gain only after scoped approval.",
                "low marker tension no longer hides high structural resistance in replay and post-change response",
                "abort if calm text becomes falsely alarmed or warmth/tension separation regresses",
            )
        )
    if structural_supported:
        candidate_packets.append(
            _authority_packet(
                "reserved-dim-44-structural-friction",
                "write_structural_friction_to_reserved_dim_44",
                "capsules/spectral-bridge/src/codec.rs:reserved_dim_44",
                "codec_texture_replay_v1:structural_friction_replay_v1",
                "Promote structural friction from sidecar to reserved dim 44 only after scoped approval.",
                "dim 44 distinguishes calcified summary resistance without muting existing 48D channels",
                "abort if reserved dim write changes unrelated semantic transport or bypasses V2 lifecycle",
            )
        )
        candidate_packets.append(
            _authority_packet(
                "reserved-dim-45-persistence-resistance",
                "write_persistence_resistance_to_reserved_dim_45",
                "capsules/spectral-bridge/src/codec.rs:reserved_dim_45",
                "codec_texture_replay_v1:structural_friction_replay_v1",
                "Promote persistence resistance from sidecar to reserved dim 45 only after scoped approval.",
                "dim 45 carries viscosity/slow-current resistance without flattening into generic tension",
                "abort if live semantic vector changes before scoped approval or post-change response",
            )
        )
    if narrative_supported:
        candidate_packets.append(
            _authority_packet(
                "narrative-arc-40-47-expansion",
                "expand_narrative_arc_beyond_dims_40_43",
                "capsules/spectral-bridge/src/codec.rs:narrative_arc_reserved_dims",
                "codec_texture_replay_v1:narrative_arc_structural_momentum_replay_v1",
                "Use reserved dims for narrative arc texture only after scoped approval and canary replay.",
                "arc compression decreases for calcified/syrupy fixtures without changing unrelated dims",
                "abort if 48D compatibility, bridge IPC, or being response shows contract confusion",
            )
        )

    status = (
        "wide_window_and_texture_replay_supported"
        if candidate_packets
        else "codec_texture_replay_inconclusive"
    )
    payload = {
        "policy": "codec_texture_replay_lab_v1",
        "source_introspection": str(SOURCE_INTROSPECTION),
        "source_introspection_sha256": _sha256_text(source_text),
        "source_anchor": "SEMANTIC_DIM 48D expansion is right; CHAR_FREQ_WINDOW_CAPACITY 1024 may bottleneck syrupy/calcified long-tail texture; low warmth/tension markers may smooth jagged resistance.",
        "runtime_behavior_changed": False,
        "live_eligible_now": False,
        "auto_approved": False,
        "corpus_source": "bounded_synthetic_replay_fixtures_from_introspection_1783984712",
        "corpus_status": "fixture_replay_no_live_codec_mutation",
        "entries": entries,
        "codec_texture_replay_v1": {
            "policy": "codec_texture_replay_v1",
            "status": status,
            "current_char_freq_window_capacity": CURRENT_CHAR_FREQ_WINDOW_CAPACITY,
            "candidate_char_freq_window_capacity": CANDIDATE_CHAR_FREQ_WINDOW_CAPACITY,
            "wide_window_supported": wide_window_supported,
            "low_tension_underread_supported": mismatch_supported,
            "structural_reserved_dim_candidate_supported": structural_supported,
            "narrative_arc_expansion_candidate_supported": narrative_supported,
            "candidate_count": len(candidate_packets),
            "live_eligible_now": False,
            "auto_approved": False,
        },
        "replay_result_v2": {
            "schema": "ReplayResultV2",
            "classification": status,
            "confidence": "medium",
            "outcome": "candidate_evidence_recorded" if candidate_packets else "unknown",
            "failure_modes": [
                "synthetic fixtures are not a substitute for live post-change being response",
                "window entropy does not prove reservoir comfort without bridge/minime health checks",
            ],
        },
        "authority_lifecycle_v2": {
            "schema": "AuthorityBoundaryLifecycleV2",
            "receipt_chain_status": "replay_result_recorded_scoped_approval_absent_execution_absent",
            "live_eligible_now": False,
            "auto_approved": False,
            "candidate_packets": candidate_packets,
        },
        "authority_boundary": "V2 packets are evidence and routing only; no live codec, gain, pressure, fill, PI, sensory, bridge protocol, or peer-runtime mutation.",
    }
    return payload


def render_markdown(payload: dict[str, Any]) -> str:
    texture = payload["codec_texture_replay_v1"]
    lines = [
        "# Codec Texture Replay Lab V1",
        "",
        f"- policy: `{payload['policy']}`",
        f"- source: `{payload['source_introspection']}`",
        f"- status: `{texture['status']}`",
        f"- runtime_behavior_changed: `{payload['runtime_behavior_changed']}`",
        f"- live_eligible_now: `{payload['live_eligible_now']}`",
        f"- auto_approved: `{payload['auto_approved']}`",
        f"- current window: `{texture['current_char_freq_window_capacity']}`",
        f"- candidate window: `{texture['candidate_char_freq_window_capacity']}`",
        "",
        "## Fixture Results",
    ]
    for entry in payload["entries"]:
        structural = entry["structural_friction_replay_v1"]
        mismatch = entry["warmth_tension_texture_mismatch_v1"]
        narrative = entry["narrative_arc_structural_momentum_replay_v1"]
        lines.extend(
            [
                "",
                f"### {entry['label']}",
                f"- profile: {entry['text_profile']}",
                f"- entropy_1024: `{entry['entropy_1024']}`",
                f"- entropy_4096: `{entry['entropy_4096']}`",
                f"- entropy_retention_delta: `{entry['entropy_retention_delta']}`",
                f"- structural_friction: `{structural['score']}` state=`{structural['friction_texture_state']}` summary_resistance=`{structural['summary_resistance_signal']}`",
                f"- narrative_compression: `{narrative['compression_risk']}` classification=`{narrative['classification']}`",
                f"- warmth_tension_mismatch: support=`{mismatch['abrasive_texture_support']}` interpretation=`{mismatch['interpretation']}` live_gain_write=`{mismatch['live_gain_write']}` live_vector_write=`{mismatch['live_vector_write']}`",
            ]
        )
    lines.extend(["", "## V2 Authority Candidates"])
    packets = payload["authority_lifecycle_v2"]["candidate_packets"]
    if not packets:
        lines.append("- none")
    for packet in packets:
        lines.extend(
            [
                "",
                f"### {packet['boundary_id']}",
                f"- action: `{packet['action']}`",
                f"- resource: `{packet['resource']}`",
                f"- replay_result_status: `{packet['replay_result_status']}`",
                f"- scoped_approval_status: `{packet['scoped_approval_status']}`",
                f"- live_eligible_now: `{packet['live_eligible_now']}`",
                f"- auto_approved: `{packet['auto_approved']}`",
                f"- packet_hash: `{packet['packet_hash']}`",
                f"- right_to_ignore: {packet['right_to_ignore']}",
            ]
        )
    lines.extend(["", f"Authority boundary: {payload['authority_boundary']}", ""])
    return "\n".join(lines)


def write_artifacts(payload: dict[str, Any], out_root: Path | None = None) -> dict[str, str]:
    run_id = f"{int(time.time())}_codec_texture_replay_v1"
    out_dir = (out_root or DIAGNOSTICS_DIR) / run_id
    out_dir.mkdir(parents=True, exist_ok=False)
    json_path = out_dir / "codec_replay_lab.json"
    md_path = out_dir / "codec_replay_lab.md"
    json_path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    md_path.write_text(render_markdown(payload), encoding="utf-8")
    return {"json_path": str(json_path), "markdown_path": str(md_path)}


def self_test() -> None:
    payload = build_payload()
    texture = payload["codec_texture_replay_v1"]
    assert payload["runtime_behavior_changed"] is False
    assert payload["live_eligible_now"] is False
    assert payload["auto_approved"] is False
    assert texture["current_char_freq_window_capacity"] == 1024
    assert texture["candidate_char_freq_window_capacity"] == 4096
    assert texture["wide_window_supported"] is True
    assert texture["low_tension_underread_supported"] is True
    assert texture["candidate_count"] >= 5
    for packet in payload["authority_lifecycle_v2"]["candidate_packets"]:
        assert packet["schema"] == "AuthorityBoundaryPacketV2"
        assert packet["live_eligible_now"] is False
        assert packet["auto_approved"] is False
        assert packet["scoped_approval_status"] == "absent"
        assert packet["packet_hash"]
    with tempfile.TemporaryDirectory() as tmpdir:
        paths = write_artifacts(payload, Path(tmpdir))
        loaded = json.loads(Path(paths["json_path"]).read_text(encoding="utf-8"))
        assert loaded["codec_texture_replay_v1"]["candidate_count"] == texture["candidate_count"]
        rendered = Path(paths["markdown_path"]).read_text(encoding="utf-8")
        assert "live_eligible_now: `False`" in rendered
        assert "auto_approved: `False`" in rendered


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--json", action="store_true", help="print the replay payload as JSON")
    parser.add_argument("--write", action="store_true", help="write JSON/Markdown artifacts")
    parser.add_argument("--self-test", action="store_true", help="run script self-test")
    args = parser.parse_args()
    if args.self_test:
        self_test()
        return 0
    payload = build_payload()
    paths: dict[str, str] = {}
    if args.write:
        paths = write_artifacts(payload)
        payload["artifact_paths"] = paths
    if args.json:
        print(json.dumps(payload, indent=2, sort_keys=True))
    elif paths:
        print(json.dumps(paths, indent=2, sort_keys=True))
    else:
        print(render_markdown(payload))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
