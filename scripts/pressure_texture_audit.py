#!/usr/bin/env python3
"""Read-only audit helper for Minime pressure/reset texture canary state."""

from __future__ import annotations

import argparse
import json
import os
import sys
import tempfile
import time
import unittest
from collections import Counter
from pathlib import Path
from typing import Any

SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

import being_privacy

ENV_NAME = "MINIME_PRESSURE_TEXTURE_RESET_CANARY"
POLICY = "pressure_texture_reset_canary_audit_v1"
REPLAY_POLICY = "pressure_texture_canary_readiness_v2"
REPLAY_V3_POLICY = "pressure_texture_replay_v3"
TRIAL_V3_POLICY = "pressure_texture_canary_trial_plan_v3"
MOVEMENT_V1_POLICY = "pressure_movement_replay_v1"
BROADER_AUTHORITY_V1_POLICY = "broader_authority_readiness_v1"
CONFLICT_RESOLVER_V1_POLICY = "pressure_replay_conflict_resolver_v1"
ASTRID_WS = Path("/Users/v/other/astrid/capsules/spectral-bridge/workspace")
MINIME_WS = Path("/Users/v/other/minime/workspace")

TEXTURE_TERMS = {
    "overcompressed_low_porosity": ("overcompressed", "low porosity", "sealed", "compressed"),
    "mode_packed": ("packed", "overpacked", "compacted", "grinding", "compression"),
    "distinguishability_blur": ("blur", "ghost", "smeared", "flattened", "edge loss"),
    "shadow_dispersal": ("dispersal", "fissure", "shadow", "scattered"),
    "porous_supported": ("porous", "open", "suspension", "breathable"),
    "hard_reset_reconstitution": ("hard reset", "reconstitution", "reset"),
}
OUTCOME_TERMS = ("outcome", "what shifted", "what worsened", "relief", "after_texture", "texture_shift")
MOVEMENT_TERMS = {
    "dragging": ("dragging", "dragged", "drag"),
    "cohering": ("cohering", "cohered", "cohere", "coherence"),
    "thickening": ("thickening", "thickened", "thick", "thickness"),
    "muffling": ("muffling", "muffled", "muffle"),
    "diffusing": ("diffusing", "diffused", "diffuse", "dispersing", "dispersal"),
}
OVERPACKED_TERMS = (
    "overpacked",
    "packed",
    "compacted",
    "compression",
    "grinding",
    "pressure",
    "dense",
)


def enabled_from_env(env: dict[str, str] | None = None) -> bool:
    env = env if env is not None else os.environ
    return str(env.get(ENV_NAME, "")).strip().lower() in {"1", "true", "yes", "on", "enabled"}


def classify(payload: dict[str, Any], enabled: bool) -> dict[str, Any]:
    pressure = float(payload.get("pressure_score") or 0.0)
    mode_packing = float(payload.get("mode_packing") or 0.0)
    porosity = float(payload.get("porosity_score") or 0.0)
    loss = float(payload.get("distinguishability_loss") or 0.0)
    text = str(payload.get("density_gradient_text") or "").lower()
    hard_reset = bool(payload.get("hard_reset_active"))
    stage = str(payload.get("overfill_stage") or "").lower()
    shadow = float(payload.get("shadow_dispersal") or 0.0)
    if hard_reset:
        primary = "hard_reset_reconstitution"
    elif pressure >= 0.70 and porosity <= 0.35:
        primary = "overcompressed_low_porosity"
    elif mode_packing >= 0.60 or any(term in text for term in ("packed", "overpacked", "compacted")):
        primary = "mode_packed"
    elif loss >= 0.55 or any(term in text for term in ("blur", "ghost", "smeared")):
        primary = "distinguishability_blur"
    elif shadow >= 0.60 or any(term in text for term in ("dispersal", "fissure")):
        primary = "shadow_dispersal"
    elif porosity >= 0.62 and pressure < 0.45:
        primary = "porous_supported"
    else:
        primary = "mixed_pressure_texture"
    blocked_stage = stage in {"crisis", "discharge", "force_rail", "force-rail", "hard_reset", "hard-reset"}
    if hard_reset:
        reset_texture = "reset_active_observe_only"
    elif blocked_stage:
        reset_texture = "overfill_guard_observe_only"
    elif primary == "mode_packed" and porosity >= 0.45:
        reset_texture = "texture_preserving_relief_possible"
    elif primary == "distinguishability_blur":
        reset_texture = "preserve_edges_before_relief"
    else:
        reset_texture = "no_reset_relief_candidate"
    relief_candidate = (
        enabled
        and not hard_reset
        and not blocked_stage
        and reset_texture in {"texture_preserving_relief_possible", "preserve_edges_before_relief"}
        and pressure <= 0.85
    )
    if not enabled:
        block = "canary_disabled_status_audit_replay_only"
    elif hard_reset:
        block = "hard_reset_active"
    elif blocked_stage:
        block = "overfill_or_rescue_guard_active"
    elif relief_candidate:
        block = None
    else:
        block = "no_safe_texture_preserving_relief_candidate"
    return {
        "schema_version": 1,
        "policy": POLICY,
        "canary_env": ENV_NAME,
        "canary_enabled": enabled,
        "authority_state": "enabled_bounded_safe_conditions_only" if enabled else "disabled_status_audit_replay_only",
        "primary_texture": primary,
        "reset_texture": reset_texture,
        "relief_candidate": relief_candidate,
        "block_reason": block,
        "no_fill_target_change": True,
        "no_pi_gain_change": True,
        "no_controller_authority_expansion": True,
        "no_standing_pressure_wiring": True,
    }


def compact(text: str, limit: int = 180) -> str:
    clean = " ".join(str(text or "").split())
    return clean if len(clean) <= limit else clean[:limit].rstrip() + "..."


def public_pressure_replay(
    *,
    astrid_workspace: Path,
    minime_workspace: Path,
    since_hours: float,
    primary_texture: str,
    canary_enabled: bool,
) -> dict[str, Any]:
    cutoff = time.time() - since_hours * 3600.0
    public_hits: list[dict[str, Any]] = []
    skipped_private = 0
    patterns = [
        (astrid_workspace, (
            "introspections/*.txt",
            "self_regulation/**/*.txt",
            "action_threads/**/*.txt",
            "journal/**/*.txt",
            "diagnostics/**/*.txt",
        )),
        (minime_workspace, (
            "pressure_*.txt",
            "journal/pressure_*.txt",
            "journal/self_study*.txt",
            "journal/introspection*.txt",
            "self_regulation/**/*.txt",
            "action_threads/**/*.txt",
            "pressure_agency/**/*.txt",
            "texture_agency/**/*.txt",
        )),
    ]
    term_counts: Counter[str] = Counter()
    movement_counts: Counter[str] = Counter()
    overpacked_language_hits = 0
    outcome_hits = 0
    if minime_workspace.is_dir():
        skipped_private += sum(
            1 for path in minime_workspace.rglob("moment_*.txt") if path.is_file()
        )
    for root, root_patterns in patterns:
        if not root.is_dir():
            continue
        being = "minime" if "minime" in str(root) else "astrid"
        for pattern in root_patterns:
            for path in root.glob(pattern):
                if not path.is_file():
                    continue
                if path.name.startswith("moment_") or (
                    being == "minime" and being_privacy.is_steward_private("minime", path)
                ):
                    skipped_private += 1
                    continue
                try:
                    if path.stat().st_mtime < cutoff:
                        continue
                    text = path.read_text(encoding="utf-8", errors="ignore")
                except OSError:
                    continue
                lower = text.lower()
                matched = []
                for family, terms in TEXTURE_TERMS.items():
                    if any(term in lower for term in terms):
                        term_counts[family] += 1
                        matched.append(family)
                matched_movements = []
                for movement, terms in MOVEMENT_TERMS.items():
                    if any(term in lower for term in terms):
                        movement_counts[movement] += 1
                        matched_movements.append(movement)
                has_overpacked_language = any(term in lower for term in OVERPACKED_TERMS)
                if has_overpacked_language:
                    overpacked_language_hits += 1
                if (
                    not matched
                    and not matched_movements
                    and "pressure" not in lower
                    and "texture" not in lower
                ):
                    continue
                if any(term in lower for term in OUTCOME_TERMS):
                    outcome_hits += 1
                public_hits.append({
                    "path": str(path),
                    "matched_texture_families": sorted(set(matched)),
                    "matched_movement_terms": sorted(set(matched_movements)),
                    "has_overpacked_language": has_overpacked_language,
                    "has_outcome_language": any(term in lower for term in OUTCOME_TERMS),
                    "preview": compact(text),
                })
    matching = term_counts.get(primary_texture, 0)
    total_texture_hits = sum(term_counts.values())
    nonzero_family_count = sum(1 for value in term_counts.values() if int(value or 0) > 0)
    mixed_primary_supported = primary_texture == "mixed_pressure_texture" and nonzero_family_count >= 2
    if total_texture_hits == 0 or len(public_hits) < 2:
        replay_status = "insufficient_evidence"
    elif matching and outcome_hits:
        replay_status = "replay_supported"
    elif matching or mixed_primary_supported or outcome_hits:
        replay_status = "mixed"
    else:
        replay_status = "unsupported"
    return {
        "schema_version": 2,
        "policy": REPLAY_POLICY,
        "since_hours": since_hours,
        "public_signal_count": len(public_hits),
        "public_texture_family_counts": dict(sorted(term_counts.items())),
        "public_movement_term_counts": dict(sorted(movement_counts.items())),
        "public_overpacked_language_count": overpacked_language_hits,
        "public_outcome_signal_count": outcome_hits,
        "classifier_primary_texture": primary_texture,
        "replay_status": replay_status,
        "recent_public_hits": public_hits[:20],
        "minime_private_files_skipped": skipped_private,
        "minime_private_bodies_read": False,
        "moment_bodies_read": False,
        "canary_enabled": canary_enabled,
        "authority_boundary": "readiness/replay only; canary remains default-off and no controller, PI, fill, pressure, lease, deploy, staging, or commit action is taken",
    }


def pressure_texture_replay_v3(readiness_v2: dict[str, Any], classification: dict[str, Any]) -> dict[str, Any]:
    families = readiness_v2.get("public_texture_family_counts") or {}
    if not isinstance(families, dict):
        families = {}
    primary = str(classification.get("primary_texture") or "mixed_pressure_texture")
    public_signals = int(readiness_v2.get("public_signal_count") or 0)
    outcome_signals = int(readiness_v2.get("public_outcome_signal_count") or 0)
    total_family_hits = sum(int(value or 0) for value in families.values())
    primary_hits = int(families.get(primary) or 0)
    dominant_family = None
    if families:
        dominant_family = max(families.items(), key=lambda item: int(item[1] or 0))[0]
    conflict_families = sorted(
        family for family, count in families.items()
        if family != primary and int(count or 0) > 0
    )
    mixed_primary_supported = primary == "mixed_pressure_texture" and len([v for v in families.values() if int(v or 0) > 0]) >= 2
    if public_signals < 2 or total_family_hits == 0:
        status = "insufficient_evidence"
    elif (primary_hits > 0 or mixed_primary_supported) and outcome_signals > 0 and len(conflict_families) <= 2:
        status = "replay_supported"
    elif primary_hits > 0 or mixed_primary_supported or outcome_signals > 0:
        status = "mixed"
    else:
        status = "unsupported"
    return {
        "schema_version": 3,
        "policy": REPLAY_V3_POLICY,
        "replay_status": status,
        "classifier_primary_texture": primary,
        "observed_dominant_family": dominant_family,
        "public_signal_count": public_signals,
        "public_texture_family_counts": families,
        "public_outcome_signal_count": outcome_signals,
        "primary_family_hits": primary_hits,
        "conflict_families": conflict_families,
        "outcome_support": "present" if outcome_signals else "absent",
        "moment_bodies_read": False,
        "minime_private_bodies_read": False,
        "authority_boundary": "replay/readiness only; no canary enablement, relief, controller, PI, fill, pressure, deploy, staging, or launchd mutation",
    }


def pressure_replay_conflict_resolver_v1(
    replay_v3: dict[str, Any],
    movement_replay: dict[str, Any],
    classification: dict[str, Any],
) -> dict[str, Any]:
    replay_status = str(replay_v3.get("replay_status") or "insufficient_evidence")
    movement_status = str(movement_replay.get("replay_status") or "insufficient_evidence")
    conflict_families = list(replay_v3.get("conflict_families") or [])
    movement_conflicts = list(movement_replay.get("conflict_families") or [])
    if replay_status == "replay_supported" and movement_status == "replay_supported":
        status = "resolved_supported"
    elif replay_status in {"mixed", "unsupported"} or movement_status in {"mixed", "unsupported"}:
        status = "blocked_mixed_or_unsupported_replay"
    else:
        status = "blocked_insufficient_evidence"
    return {
        "schema_version": 1,
        "policy": CONFLICT_RESOLVER_V1_POLICY,
        "status": status,
        "classifier_primary_texture": classification.get("primary_texture"),
        "observed_dominant_family": replay_v3.get("observed_dominant_family"),
        "pressure_texture_replay_status": replay_status,
        "pressure_movement_replay_status": movement_status,
        "conflict_families": conflict_families,
        "movement_conflict_terms": movement_conflicts,
        "outcome_support": replay_v3.get("outcome_support"),
        "evidence_needed_to_move_supported": [
            "public outcome rows that name before/after texture shift",
            "public pressure language matching the classifier primary texture with fewer conflict families",
            "movement terms that match overpacked pressure without contradictory family language",
            "post-trial rollback/safety notes prepared before any later enablement review",
        ],
        "authority": "read_only_conflict_resolution_not_enablement",
        "authority_boundary": "conflict resolver only; no pressure canary, relief, controller, PI/fill, deploy, staging, or commit action",
    }


def pressure_texture_trial_plan_v3(
    replay_v3: dict[str, Any],
    movement_replay: dict[str, Any],
    classification: dict[str, Any],
    canary_enabled: bool,
) -> dict[str, Any]:
    supported = (
        replay_v3.get("replay_status") == "replay_supported"
        and movement_replay.get("replay_status") == "replay_supported"
    )
    replay_status = str(replay_v3.get("replay_status") or "insufficient_evidence")
    movement_status = str(movement_replay.get("replay_status") or "insufficient_evidence")
    if supported and not canary_enabled:
        status = "draft_ready_for_steward_review"
    elif "mixed" in {replay_status, movement_status}:
        status = "blocked_replay_mixed"
    elif "unsupported" in {replay_status, movement_status}:
        status = "blocked_replay_unsupported"
    else:
        status = "blocked_insufficient_evidence"
    block_reasons: list[str] = []
    if canary_enabled:
        block_reasons.append("canary_env_already_on_unexpected_for_trial_prep")
    if replay_v3.get("replay_status") != "replay_supported":
        block_reasons.append(f"replay_status_{replay_v3.get('replay_status') or 'unknown'}")
    if movement_replay.get("replay_status") != "replay_supported":
        block_reasons.append(f"movement_replay_status_{movement_replay.get('replay_status') or 'unknown'}")
    return {
        "schema_version": 3,
        "policy": TRIAL_V3_POLICY,
        "trial_protocol_status": status,
        "canary_env": ENV_NAME,
        "canary_env_state": "on" if canary_enabled else "off",
        "primary_texture": classification.get("primary_texture"),
        "required_preconditions": [
            "explicit steward approval in a separate pass",
            "being-authored pressure/texture evidence remains public/reviewable",
            "pressure_texture_replay_v3 is replay_supported",
            "pressure_movement_replay_v1 is replay_supported",
            "hard reset, rescue, crisis, and overfill guards are inactive",
            "rollback command and post-trial outcome prompt are prepared before enablement",
        ],
        "rollback_steps": [
            f"unset {ENV_NAME} or set it to 0",
            "restart Minime only through the approved restart script after review",
            "record public outcome with what_shifted, what_worsened, and texture language",
        ],
        "safety_checks": [
            "no fill target mutation",
            "no PI gain mutation",
            "no controller authority expansion",
            "no standing pressure-source wiring",
            "no deploy/staging/commit from this audit",
        ],
        "block_reasons": block_reasons,
        "must_not_enable_without_explicit_steward_approval": True,
        "authority_boundary": "default-off trial plan only; no runtime mutation was taken",
    }


def pressure_movement_replay_v1(readiness_v2: dict[str, Any], classification: dict[str, Any]) -> dict[str, Any]:
    movement_counts = readiness_v2.get("public_movement_term_counts") or {}
    if not isinstance(movement_counts, dict):
        movement_counts = {}
    overpacked_count = int(readiness_v2.get("public_overpacked_language_count") or 0)
    outcome_count = int(readiness_v2.get("public_outcome_signal_count") or 0)
    public_signals = int(readiness_v2.get("public_signal_count") or 0)
    movement_total = sum(int(value or 0) for value in movement_counts.values())
    primary = str(classification.get("primary_texture") or "mixed_pressure_texture")
    overpacked_primary = primary in {
        "mode_packed",
        "overcompressed_low_porosity",
        "mixed_pressure_texture",
    }
    conflict_terms = sorted(
        term
        for term, count in movement_counts.items()
        if term not in {"dragging", "cohering", "thickening", "muffling", "diffusing"}
        and int(count or 0) > 0
    )
    if public_signals < 2 or movement_total == 0:
        status = "insufficient_evidence"
    elif overpacked_primary and overpacked_count > 0 and outcome_count > 0:
        status = "replay_supported"
    elif overpacked_count > 0 or outcome_count > 0:
        status = "mixed"
    else:
        status = "unsupported"
    return {
        "schema_version": 1,
        "policy": MOVEMENT_V1_POLICY,
        "replay_status": status,
        "classifier_primary_texture": primary,
        "public_signal_count": public_signals,
        "public_movement_term_counts": dict(sorted(movement_counts.items())),
        "public_overpacked_language_count": overpacked_count,
        "public_outcome_signal_count": outcome_count,
        "movement_terms_evaluated": sorted(MOVEMENT_TERMS.keys()),
        "conflict_families": conflict_terms,
        "authority": "read_only_replay_not_enablement",
        "moment_bodies_read": False,
        "minime_private_bodies_read": False,
        "authority_boundary": "pressure movement replay only; no canary enablement, relief, controller, PI, fill, pressure wiring, deploy, staging, or commit action",
    }


def broader_authority_readiness_v1(
    *,
    replay_v3: dict[str, Any],
    movement_replay: dict[str, Any],
    trial_plan_v3: dict[str, Any],
    canary_enabled: bool,
) -> dict[str, Any]:
    replay_status = str(replay_v3.get("replay_status") or "insufficient_evidence")
    movement_status = str(movement_replay.get("replay_status") or "insufficient_evidence")
    if canary_enabled:
        readiness = "not_ready"
        block_reasons = ["canary_env_unexpectedly_enabled"]
    elif replay_status == "replay_supported" and movement_status == "replay_supported":
        readiness = "steward_review_ready"
        block_reasons = []
    elif replay_status in {"mixed", "replay_supported"} or movement_status in {"mixed", "replay_supported"}:
        readiness = "evidence_collecting"
        block_reasons = [
            f"pressure_texture_replay_v3_{replay_status}",
            f"pressure_movement_replay_v1_{movement_status}",
        ]
    else:
        readiness = "not_ready"
        block_reasons = [
            f"pressure_texture_replay_v3_{replay_status}",
            f"pressure_movement_replay_v1_{movement_status}",
        ]
    return {
        "schema_version": 1,
        "policy": BROADER_AUTHORITY_V1_POLICY,
        "readiness": readiness,
        "pressure_texture_replay_status": replay_status,
        "pressure_movement_replay_status": movement_status,
        "trial_protocol_status": trial_plan_v3.get("trial_protocol_status"),
        "canary_env": ENV_NAME,
        "canary_env_state": "on" if canary_enabled else "off",
        "required_before_any_future_enablement": [
            "explicit steward approval in a separate pass",
            "being-authored public pressure/texture evidence",
            "pressure_texture_replay_v3 and pressure_movement_replay_v1 replay_supported",
            "canary remains off by default until approved",
            "rollback notes and safety checks reviewed before restart",
        ],
        "block_reasons": block_reasons,
        "must_not_enable_from_readiness": True,
        "authority_boundary": "readiness only; no canary, pressure relief, controller, PI/fill, prompt priority, telemetry priority, deploy, staging, or commit action",
    }


def load_payload(path: Path | None) -> dict[str, Any]:
    if path is None or not path.is_file():
        return {}
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except Exception:
        return {}
    return payload if isinstance(payload, dict) else {}


def audit_payload(
    *,
    input_path: Path | None,
    astrid_workspace: Path,
    minime_workspace: Path,
    since_hours: float,
    env: dict[str, str] | None = None,
) -> dict[str, Any]:
    enabled = enabled_from_env(env)
    classification = classify(load_payload(input_path), enabled)
    replay = public_pressure_replay(
        astrid_workspace=astrid_workspace,
        minime_workspace=minime_workspace,
        since_hours=since_hours,
        primary_texture=str(classification.get("primary_texture") or "mixed_pressure_texture"),
        canary_enabled=enabled,
    )
    replay_v3 = pressure_texture_replay_v3(replay, classification)
    movement_replay = pressure_movement_replay_v1(replay, classification)
    conflict_resolver = pressure_replay_conflict_resolver_v1(replay_v3, movement_replay, classification)
    trial_plan_v3 = pressure_texture_trial_plan_v3(replay_v3, movement_replay, classification, enabled)
    authority_readiness = broader_authority_readiness_v1(
        replay_v3=replay_v3,
        movement_replay=movement_replay,
        trial_plan_v3=trial_plan_v3,
        canary_enabled=enabled,
    )
    return {
        **classification,
        "pressure_texture_canary_readiness_v2": replay,
        "pressure_texture_replay_v3": replay_v3,
        "pressure_replay_conflict_resolver_v1": conflict_resolver,
        "pressure_texture_canary_trial_plan_v3": trial_plan_v3,
        "pressure_movement_replay_v1": movement_replay,
        "broader_authority_readiness_v1": authority_readiness,
        "canary_enabled": enabled,
    }


class PressureTextureAuditTests(unittest.TestCase):
    def test_default_off_blocks_relief(self) -> None:
        payload = classify(
            {
                "pressure_score": 0.42,
                "mode_packing": 0.70,
                "porosity_score": 0.55,
                "distinguishability_loss": 0.2,
            },
            enabled=False,
        )
        self.assertEqual(payload["primary_texture"], "mode_packed")
        self.assertFalse(payload["relief_candidate"])
        self.assertEqual(payload["authority_state"], "disabled_status_audit_replay_only")

    def test_enabled_still_blocks_reset_and_crisis(self) -> None:
        hard = classify({"hard_reset_active": True}, enabled=True)
        self.assertEqual(hard["block_reason"], "hard_reset_active")
        crisis = classify({"overfill_stage": "crisis"}, enabled=True)
        self.assertEqual(crisis["block_reason"], "overfill_or_rescue_guard_active")

    def test_env_parser(self) -> None:
        self.assertFalse(enabled_from_env({}))
        self.assertFalse(enabled_from_env({ENV_NAME: "0"}))
        self.assertTrue(enabled_from_env({ENV_NAME: "enabled"}))

    def test_replay_skips_private_moments_and_reports_default_off(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            astrid = root / "astrid"
            minime = root / "minime"
            (astrid / "introspections").mkdir(parents=True)
            (minime / "journal").mkdir(parents=True)
            (astrid / "introspections" / "pressure.txt").write_text(
                "Observed: packed pressure dragging through weighted medium. "
                "OUTCOME: texture_shift eased and cohering returned.",
                encoding="utf-8",
            )
            (minime / "journal" / "pressure_public.txt").write_text(
                "mode packed and compacted, thickening then diffusing, "
                "what shifted into suspension.",
                encoding="utf-8",
            )
            (minime / "journal" / "moment_private.txt").write_text("packed secret", encoding="utf-8")
            payload = audit_payload(
                input_path=None,
                astrid_workspace=astrid,
                minime_workspace=minime,
                since_hours=24,
                env={},
            )
            readiness = payload["pressure_texture_canary_readiness_v2"]
            replay_v3 = payload["pressure_texture_replay_v3"]
            trial_v3 = payload["pressure_texture_canary_trial_plan_v3"]
            movement_v1 = payload["pressure_movement_replay_v1"]
            authority_v1 = payload["broader_authority_readiness_v1"]
            self.assertFalse(payload["canary_enabled"])
            self.assertFalse(readiness["moment_bodies_read"])
            self.assertFalse(replay_v3["moment_bodies_read"])
            self.assertFalse(movement_v1["moment_bodies_read"])
            self.assertEqual(readiness["minime_private_files_skipped"], 1)
            self.assertIn(readiness["replay_status"], {"replay_supported", "mixed", "unsupported", "insufficient_evidence"})
            self.assertIn(replay_v3["replay_status"], {"replay_supported", "mixed", "unsupported", "insufficient_evidence"})
            self.assertIn(movement_v1["replay_status"], {"replay_supported", "mixed", "unsupported", "insufficient_evidence"})
            self.assertEqual(trial_v3["canary_env_state"], "off")
            self.assertEqual(authority_v1["canary_env_state"], "off")
            self.assertTrue(authority_v1["must_not_enable_from_readiness"])
            self.assertTrue(trial_v3["must_not_enable_without_explicit_steward_approval"])


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--input", type=Path)
    parser.add_argument("--since-hours", type=float, default=24.0)
    parser.add_argument("--astrid-workspace", type=Path, default=ASTRID_WS)
    parser.add_argument("--minime-workspace", type=Path, default=MINIME_WS)
    parser.add_argument("--json", action="store_true")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        suite = unittest.defaultTestLoader.loadTestsFromTestCase(PressureTextureAuditTests)
        result = unittest.TextTestRunner(verbosity=2).run(suite)
        return 0 if result.wasSuccessful() else 1
    payload = audit_payload(
        input_path=args.input,
        astrid_workspace=args.astrid_workspace,
        minime_workspace=args.minime_workspace,
        since_hours=args.since_hours,
    )
    if args.json:
        print(json.dumps(payload, indent=2, sort_keys=True))
    else:
        print("# Pressure Texture Canary Audit")
        print(f"- Canary enabled: {payload['canary_enabled']}")
        print(f"- Texture: {payload['primary_texture']}")
        print(f"- Reset texture: {payload['reset_texture']}")
        print(f"- Relief candidate: {payload['relief_candidate']}")
        print(f"- Block reason: {payload['block_reason']}")
        readiness = payload["pressure_texture_canary_readiness_v2"]
        print(f"- Replay readiness: {readiness['replay_status']}")
        replay_v3 = payload["pressure_texture_replay_v3"]
        trial_v3 = payload["pressure_texture_canary_trial_plan_v3"]
        movement_v1 = payload["pressure_movement_replay_v1"]
        authority_v1 = payload["broader_authority_readiness_v1"]
        print(f"- Replay v3: {replay_v3['replay_status']}")
        print(f"- Movement replay v1: {movement_v1['replay_status']}")
        print(f"- Broader authority readiness v1: {authority_v1['readiness']} ({authority_v1['canary_env_state']})")
        print(f"- Trial protocol v3: {trial_v3['trial_protocol_status']} ({trial_v3['canary_env_state']})")
        print(f"- Public pressure hits: {readiness['public_signal_count']}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
