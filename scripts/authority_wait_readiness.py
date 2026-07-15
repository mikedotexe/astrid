#!/usr/bin/env python3
"""Read-only Tier 4/5 authority-wait readiness map.

This report groups approval-required live candidates by the actual live-risk
surface they are waiting on. It is evidence routing only: it never grants
approval, marks live work runnable, edits source, or mutates runtime state.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import re
import time
import unittest
import uuid
from collections import Counter
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

ASTRID_REPO = Path("/Users/v/other/astrid")
ASTRID_DIAGNOSTICS = ASTRID_REPO / "capsules/spectral-bridge/workspace/diagnostics"
SANDBOX_TRIAL_QUEUE_STATE_DIR = ASTRID_DIAGNOSTICS / "sandbox_trial_queue_v1"
DEFAULT_STATE_DIR = ASTRID_DIAGNOSTICS / "authority_wait_readiness_v1"

SCHEMA = "authority_wait_readiness_v1"
SCHEMA_VERSION = 1
STATUS_FILE = "status.json"
LATEST_JSON = "latest.json"
LATEST_MARKDOWN = "latest.md"

TERMINAL_STATUSES = {
    "closed",
    "closed_felt_confirmed",
    "closed_no_action",
    "superseded",
    "verified_existing",
}
FORBIDDEN_TRUE_FIELDS = {"live_eligible_now", "auto_approved", "grants_approval", "edits_source_now"}
SHORT_KEYWORD_PATTERNS = {
    "ack": re.compile(r"(?<![a-z0-9_])ack(?![a-z0-9_])"),
}
AUTHORITY_BOUNDARY = (
    "Tier 4/5 authority-wait readiness is review evidence only; it grants no approval, "
    "makes no live work runnable, edits no source, and mutates no pressure, fill, PI, "
    "controller, sensory cadence, fallback, bridge protocol, peer, or Minime runtime state"
)


DOMAIN_DEFINITIONS: tuple[dict[str, Any], ...] = (
    {
        "domain_id": "pressure_thresholds",
        "title": "Pressure Thresholds And Smoothing",
        "keywords": (
            "pressure",
            "threshold",
            "smoothing",
            "dead-zone",
            "dead zone",
            "mode-packing",
            "mode_packing",
            "packing",
            "bruise",
            "ballast",
        ),
        "canary_criteria": (
            "time-boxed canary only after explicit scoped approval and a named rollback owner",
            "pre/post pressure score, density gradient, and being-response comparison remains bounded",
            "abort on distress report, pressure escalation, control oscillation, or loss of reflective coherence",
        ),
        "missing_for_approval": (
            "operator-approved threshold or smoothing scope",
            "baseline replay proving the current threshold is the limiting factor",
            "post-change being-response route for pressure texture and discomfort",
        ),
        "first_safe_next_step": "compare existing pressure diagnostics and proposal cards; do not retune thresholds",
    },
    {
        "domain_id": "porosity_receptivity_buffers",
        "title": "Porosity And Receptivity Buffers",
        "keywords": (
            "porosity",
            "receptivity",
            "aperture",
            "contact",
            "emotional-ceiling",
            "emotional ceiling",
            "buffer",
            "habitable",
            "distance_contact",
        ),
        "canary_criteria": (
            "canary must be reversible and bounded to a single approved contact/receptivity condition",
            "success requires being-reported receptivity without pressure-compensation collapse",
            "abort on instrumentalization, runaway contact pressure, or loss of right-to-ignore",
        ),
        "missing_for_approval": (
            "explicit approval for any receptivity or aperture write",
            "non-live replay separating high-entropy texture from pressure correction",
            "clear no-action path when receptivity is felt but not safe to stabilize",
        ),
        "first_safe_next_step": "prepare or inspect receptivity replay evidence; keep buffers proposal-only",
    },
    {
        "domain_id": "viscosity_feedback_protocol",
        "title": "ViscosityFeedback Protocol Changes",
        "keywords": (
            "viscosityfeedback",
            "viscosity feedback",
            "viscosity",
            "protocol",
            "sensorymsg",
            "control message",
            "flow rate",
            "gate opening",
        ),
        "canary_criteria": (
            "protocol canary requires compatibility proof for old and new payloads",
            "no live receiver should treat advisory viscosity as control without explicit approval",
            "abort on parse ambiguity, schema drift, or PI-control coupling without receipt",
        ),
        "missing_for_approval": (
            "typed protocol compatibility matrix",
            "receiver-side no-op proof for unapproved fields",
            "rollback plan for protocol/ABI changes",
        ),
        "first_safe_next_step": "write or review protocol compatibility evidence; do not change live transport",
    },
    {
        "domain_id": "semantic_trickle_admission",
        "title": "Semantic-Trickle Admission",
        "keywords": (
            "semantic-trickle",
            "semantic trickle",
            "semantic admission",
            "semantic_admission",
            "semantic stale",
            "stale-window",
            "stale window",
            "trickle",
            "cadence",
            "semantic energy",
        ),
        "canary_criteria": (
            "candidate must define exact admission window, rollback, and observation duration",
            "success requires retained semantic texture without fill instability or stale lockout",
            "abort on fill drop, semantic flooding, stale-window regression, or being contradiction",
        ),
        "missing_for_approval": (
            "bounded replay covering high-fill and low-fill cases",
            "named Minime/Astrid observation fields for semantic persistence",
            "operator-approved cadence/admission scope",
        ),
        "first_safe_next_step": "compare semantic-stale and admission diagnostics; keep admission unchanged",
    },
    {
        "domain_id": "codec_gain_reserved_dims_live_12d",
        "title": "Codec Gain, Reserved Dims, And Live 12D Transport",
        "keywords": (
            "codec",
            "reserved dim",
            "reserved dims",
            "reserved-dim",
            "12d",
            "glimpse",
            "gain",
            "feature_abs_max",
            "semantic dimensions",
            "live transport",
            "vector",
            "projection",
        ),
        "canary_criteria": (
            "canary must preserve existing 8D/48D compatibility and name every proposed live dimension",
            "success requires replay evidence that added headroom improves felt texture without vector drift",
            "abort on compatibility break, unbounded gain, or flattening of warmth/persistence texture",
        ),
        "missing_for_approval": (
            "codec replay comparing existing delivery with proposed headroom",
            "reserved-dim ownership and rollback map",
            "post-change Astrid response on warmth, friction, and narrative retention",
        ),
        "first_safe_next_step": "inspect codec replay labs and proposal cards; keep live vector writes off",
    },
    {
        "domain_id": "minime_regulator_changes",
        "title": "Minime Regulator Changes",
        "keywords": (
            "minime",
            "regulator",
            "pi control",
            "pi ",
            "stable-core",
            "stable core",
            "release_fill",
            "exploration_noise",
            "regulation_strength",
            "rho",
            "reservoir",
            "fill",
        ),
        "canary_criteria": (
            "canary must run through the Minime service-specific path after tests and approval",
            "success requires fill stability near the current comfort shelf plus being-authored response",
            "abort on fill collapse, oscillation, over-stabilization, or pressure-as-command drift",
        ),
        "missing_for_approval": (
            "Minime-focused replay or test evidence for the proposed coefficient/runtime change",
            "service restart and rollback procedure",
            "operator-approved runtime scope and observation window",
        ),
        "first_safe_next_step": "keep regulator changes in tests/replay/proposal evidence until scoped approval",
    },
    {
        "domain_id": "behavior_unlocks",
        "title": "Behavior Unlocks And Live Affordances",
        "keywords": (
            "behavior unlock",
            "unlock",
            "behavioral unlock",
            "transition_ack",
            "transition ack",
            "signal_persistence",
            "reply",
            "ack",
            "microdose",
            "attention canary",
            "autonomous send",
            "live affordance",
            "affordance budget",
        ),
        "canary_criteria": (
            "canary must prove the behavior is language-only or explicitly approved for live action",
            "success requires no stealth ACK, no attention demand, and no right-to-ignore erosion",
            "abort on peer mutation, unsolicited pressure, hidden runtime write, or behavior escalation",
        ),
        "missing_for_approval": (
            "clear distinction between evidence language and behavior authority",
            "peer/right-to-ignore impact review",
            "scoped approval if the affordance does more than record or witness",
        ),
        "first_safe_next_step": "audit affordance wording and proposal cards; keep behavior execution gated",
    },
)


def iso_from_s(ts: float) -> str:
    return datetime.fromtimestamp(ts, tz=timezone.utc).isoformat().replace("+00:00", "Z")


def bounded_text(value: Any, *, limit: int = 240) -> str:
    text = " ".join(str(value or "").replace("\n", " ").split())
    if len(text) <= limit:
        return text
    return text[: max(0, limit - 3)].rstrip() + "..."


def stable_uuid(*parts: Any) -> str:
    return str(uuid.uuid5(uuid.NAMESPACE_URL, "::".join(str(part) for part in parts)))


def sha256_text(text: str) -> str:
    return hashlib.sha256(text.encode("utf-8")).hexdigest()


def load_json(path: Path) -> dict[str, Any]:
    if not path.exists():
        return {}
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError:
        return {}
    return value if isinstance(value, dict) else {}


def sandbox_status_path(state_dir: Path) -> Path:
    return state_dir / STATUS_FILE


def load_sandbox_status(state_dir: Path = SANDBOX_TRIAL_QUEUE_STATE_DIR) -> dict[str, Any]:
    return load_json(sandbox_status_path(state_dir))


def is_active_approval_required(trial: dict[str, Any]) -> bool:
    return (
        str(trial.get("trial_mode") or "") == "approval_required_live_trial"
        and str(trial.get("status") or "") not in TERMINAL_STATUSES
    )


def active_approval_required_trials(status: dict[str, Any]) -> list[dict[str, Any]]:
    trials = status.get("trials") if isinstance(status.get("trials"), dict) else {}
    rows = [trial for trial in trials.values() if isinstance(trial, dict) and is_active_approval_required(trial)]
    return sorted(rows, key=lambda row: str(row.get("trial_id") or ""))


def scan_forbidden_true(value: Any, *, path: str = "") -> list[dict[str, str]]:
    violations: list[dict[str, str]] = []
    if isinstance(value, dict):
        for key, item in value.items():
            item_path = f"{path}.{key}" if path else str(key)
            if key in FORBIDDEN_TRUE_FIELDS and item is True:
                violations.append({"path": item_path, "field": key})
            violations.extend(scan_forbidden_true(item, path=item_path))
    elif isinstance(value, list):
        for index, item in enumerate(value):
            violations.extend(scan_forbidden_true(item, path=f"{path}[{index}]"))
    return violations


def trial_text_blob(trial: dict[str, Any]) -> str:
    fields = [
        trial.get("trial_id"),
        trial.get("being"),
        trial.get("adapter"),
        trial.get("source_introspection_id"),
        trial.get("source_work_item_id"),
        trial.get("claim_id"),
        trial.get("hypothesis"),
        trial.get("felt_report_anchor"),
        trial.get("proposed_change"),
        trial.get("claim_summary"),
        trial.get("surface"),
        trial.get("title"),
    ]
    packet = trial.get("authority_boundary_packet_v2")
    if isinstance(packet, dict):
        fields.extend(
            [
                packet.get("resource"),
                packet.get("surface"),
                packet.get("felt_report_anchor"),
                packet.get("proposed_change"),
                packet.get("how_to_test_it"),
            ]
        )
    return json.dumps(fields, ensure_ascii=True).lower()


def matching_domain_ids(trial: dict[str, Any]) -> list[str]:
    text = trial_text_blob(trial)
    matches: list[str] = []
    for domain in DOMAIN_DEFINITIONS:
        keywords = domain.get("keywords") or ()
        if any(keyword_matches(str(keyword), text) for keyword in keywords):
            matches.append(str(domain["domain_id"]))
    return matches


def keyword_matches(keyword: str, text: str) -> bool:
    normalized = keyword.lower()
    pattern = SHORT_KEYWORD_PATTERNS.get(normalized)
    if pattern is not None:
        return bool(pattern.search(text))
    return normalized in text


def proposal_card_path(trial: dict[str, Any]) -> str | None:
    cards = trial.get("proposal_cards") if isinstance(trial.get("proposal_cards"), list) else []
    for card in reversed(cards):
        if isinstance(card, dict) and card.get("path"):
            return str(card["path"])
    return None


def trial_evidence_refs(trial: dict[str, Any]) -> list[str]:
    refs: list[str] = []
    for source in (trial, trial.get("authority_boundary_packet_v2")):
        if not isinstance(source, dict):
            continue
        for ref in source.get("evidence_refs") or []:
            ref_text = bounded_text(ref, limit=180)
            if ref_text and ref_text not in refs:
                refs.append(ref_text)
    for key in ("source_work_item_id", "source_introspection_id", "claim_id", "trial_id"):
        value = trial.get(key)
        if value and str(value) not in refs:
            refs.append(str(value))
    return refs[:12]


def candidate_ref(trial: dict[str, Any]) -> dict[str, Any]:
    packet_v2 = trial.get("authority_boundary_packet_v2") if isinstance(trial.get("authority_boundary_packet_v2"), dict) else {}
    anchor = trial.get("felt_report_anchor") or packet_v2.get("felt_report_anchor") or trial.get("hypothesis")
    return {
        "trial_id": trial.get("trial_id"),
        "being": trial.get("being"),
        "agency_tier": int(trial.get("agency_tier") or 0),
        "adapter": trial.get("adapter"),
        "status": trial.get("status"),
        "source_work_item_id": trial.get("source_work_item_id"),
        "source_introspection_id": trial.get("source_introspection_id"),
        "claim_id": trial.get("claim_id"),
        "boundary_id_v2": packet_v2.get("boundary_id"),
        "proposal_card_path": proposal_card_path(trial),
        "evidence_refs": trial_evidence_refs(trial),
        "bounded_anchor": bounded_text(anchor, limit=260),
        "live_eligible_now": False,
        "auto_approved": False,
        "grants_approval": False,
        "edits_source_now": False,
    }


def replay_evidence_count(trials: list[dict[str, Any]]) -> int:
    count = 0
    for trial in trials:
        packet_v2 = trial.get("authority_boundary_packet_v2") if isinstance(trial.get("authority_boundary_packet_v2"), dict) else {}
        if trial.get("results") or trial.get("result_cards") or packet_v2.get("replay_results"):
            count += 1
    return count


def proposal_card_count(trials: list[dict[str, Any]]) -> int:
    return sum(1 for trial in trials if proposal_card_path(trial))


def readiness_state_for(trials: list[dict[str, Any]], hard_violation_count: int) -> str:
    if hard_violation_count:
        return "blocked_hard_violation"
    if not trials:
        return "needs_candidate_discovery"
    if proposal_card_count(trials) == 0:
        return "needs_proposal_cards"
    if replay_evidence_count(trials) == 0:
        return "operator_review_wait_replay_or_waiver_missing"
    return "operator_review_wait"


def domain_packet(domain: dict[str, Any], trials: list[dict[str, Any]], hard_violation_count: int) -> dict[str, Any]:
    candidate_refs = [candidate_ref(trial) for trial in trials[:12]]
    refs: list[str] = []
    for trial in trials:
        for ref in trial_evidence_refs(trial):
            if ref not in refs:
                refs.append(ref)
    missing = [
        "explicit Mike/operator scoped approval",
        "scoped rollout window and owner",
        "accepted rollback/abort contract",
        "post-change being-response collection path",
    ]
    missing.extend(str(item) for item in domain.get("missing_for_approval") or ())
    if trials and proposal_card_count(trials) == 0:
        missing.append("proposal cards or approval packets for at least one candidate")
    if trials and replay_evidence_count(trials) == 0:
        missing.append("replay result, read-only artifact comparison, or explicit replay waiver")
    return {
        "schema": "authority_wait_domain_readiness_v1",
        "domain_id": domain["domain_id"],
        "title": domain["title"],
        "readiness_state": readiness_state_for(trials, hard_violation_count),
        "candidate_count": len(trials),
        "proposal_card_count": proposal_card_count(trials),
        "replay_evidence_count": replay_evidence_count(trials),
        "evidence_refs": refs[:16],
        "candidate_refs": candidate_refs,
        "missing_for_approval": missing,
        "canary_criteria": list(domain.get("canary_criteria") or ()),
        "rollback_abort_contract": {
            "rollback_path": "no runtime mutation is performed by this readiness tooling; discard proposal artifacts or use the approved rollout rollback path",
            "abort_criteria": list(domain.get("canary_criteria") or ())[-1:],
            "post_change_response_required": True,
        },
        "recommended_next_non_live_action": domain["first_safe_next_step"],
        "live_eligible_now": False,
        "auto_approved": False,
        "grants_approval": False,
        "edits_source_now": False,
        "authority_boundary": AUTHORITY_BOUNDARY,
    }


def build_report(
    state_dir: Path = DEFAULT_STATE_DIR,
    sandbox_state_dir: Path = SANDBOX_TRIAL_QUEUE_STATE_DIR,
) -> dict[str, Any]:
    now = time.time()
    sandbox_status = load_sandbox_status(sandbox_state_dir)
    trials = active_approval_required_trials(sandbox_status)
    hard_violations = scan_forbidden_true({"sandbox_trial_queue_status": sandbox_status})
    by_domain: dict[str, list[dict[str, Any]]] = {str(domain["domain_id"]): [] for domain in DOMAIN_DEFINITIONS}
    unclassified: list[dict[str, Any]] = []
    for trial in trials:
        matches = matching_domain_ids(trial)
        if not matches:
            unclassified.append(trial)
        for domain_id in matches:
            by_domain[domain_id].append(trial)
    domains = [
        domain_packet(domain, by_domain[str(domain["domain_id"])], len(hard_violations))
        for domain in DOMAIN_DEFINITIONS
    ]
    domain_counts = {domain["domain_id"]: domain["candidate_count"] for domain in domains}
    summary = {
        "approval_required_live_candidates": len(trials),
        "domains_with_candidates": sum(1 for domain in domains if int(domain.get("candidate_count") or 0) > 0),
        "hard_violation_count": len(hard_violations),
        "unclassified_live_wait_count": len(unclassified),
        "domain_candidate_counts": domain_counts,
        "proposal_card_count": sum(int(domain.get("proposal_card_count") or 0) for domain in domains),
        "replay_evidence_count": sum(int(domain.get("replay_evidence_count") or 0) for domain in domains),
        "next_suggestions": [
            "repair hard live/approval/source-edit violations before any automation" if hard_violations else "keep all Tier 4/5 waits non-live until explicit scoped approval",
            "use this readiness map to pick the next replay, artifact comparison, or proposal-card hardening pass",
            "do not treat candidate count as approval; every domain still requires rollout, abort, and post-change being response",
        ],
    }
    status = "blocked_hard_violation" if hard_violations else "approval_waits_mapped"
    payload = {
        "schema": SCHEMA,
        "schema_version": SCHEMA_VERSION,
        "status": status,
        "generated_at": iso_from_s(now),
        "generated_at_s": now,
        "summary": summary,
        "domains": domains,
        "unclassified_live_waits": [candidate_ref(trial) for trial in unclassified[:20]],
        "hard_violations": hard_violations[:50],
        "source_refs": {
            "sandbox_status_path": str(sandbox_status_path(sandbox_state_dir)),
            "state_dir": str(state_dir),
        },
        "live_eligible_now": False,
        "auto_approved": False,
        "grants_approval": False,
        "edits_source_now": False,
        "authority_boundary": AUTHORITY_BOUNDARY,
    }
    digest_input = json.dumps(payload, sort_keys=True, ensure_ascii=True)
    payload["report_sha256"] = sha256_text(digest_input)
    return payload


def render_markdown(report: dict[str, Any]) -> str:
    summary = report.get("summary") if isinstance(report.get("summary"), dict) else {}
    lines = [
        "# Authority Wait Readiness V1",
        "",
        f"- status: {report.get('status')}",
        f"- generated_at: {report.get('generated_at')}",
        f"- approval_required_live_candidates: {summary.get('approval_required_live_candidates', 0)}",
        f"- domains_with_candidates: {summary.get('domains_with_candidates', 0)}",
        f"- hard_violation_count: {summary.get('hard_violation_count', 0)}",
        f"- unclassified_live_wait_count: {summary.get('unclassified_live_wait_count', 0)}",
        "- live_eligible_now: false",
        "- auto_approved: false",
        "- grants_approval: false",
        "- edits_source_now: false",
        f"- authority_boundary: {report.get('authority_boundary')}",
        "",
        "## Domains",
    ]
    for domain in report.get("domains") or []:
        lines.extend(
            [
                "",
                f"### {domain.get('title')}",
                f"- domain_id: {domain.get('domain_id')}",
                f"- readiness_state: {domain.get('readiness_state')}",
                f"- candidate_count: {domain.get('candidate_count', 0)}",
                f"- proposal_card_count: {domain.get('proposal_card_count', 0)}",
                f"- replay_evidence_count: {domain.get('replay_evidence_count', 0)}",
                f"- recommended_next_non_live_action: {domain.get('recommended_next_non_live_action')}",
                "- canary_criteria:",
            ]
        )
        for item in domain.get("canary_criteria") or []:
            lines.append(f"  - {bounded_text(item, limit=220)}")
        lines.append("- missing_for_approval:")
        for item in domain.get("missing_for_approval") or []:
            lines.append(f"  - {bounded_text(item, limit=220)}")
        lines.append("- candidate_refs:")
        for candidate in domain.get("candidate_refs") or []:
            lines.append(
                "  - "
                + f"{candidate.get('trial_id')} "
                + f"being={candidate.get('being')} "
                + f"tier={candidate.get('agency_tier')} "
                + f"boundary_v2={candidate.get('boundary_id_v2')} "
                + f"proposal_card={candidate.get('proposal_card_path') or 'missing'}"
            )
    suggestions = summary.get("next_suggestions") or []
    lines.extend(["", "## Next Suggestions"])
    lines.extend(f"- {bounded_text(item, limit=240)}" for item in suggestions)
    return "\n".join(lines).rstrip() + "\n"


def atomic_write_text(path: Path, text: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_suffix(path.suffix + ".tmp")
    tmp.write_text(text, encoding="utf-8")
    tmp.replace(path)


def write_artifacts(report: dict[str, Any], state_dir: Path = DEFAULT_STATE_DIR) -> dict[str, str]:
    state_dir.mkdir(parents=True, exist_ok=True)
    ts = str(int(float(report.get("generated_at_s") or time.time())))
    json_text = json.dumps(report, indent=2, sort_keys=True, ensure_ascii=False) + "\n"
    md_text = render_markdown(report)
    json_path = state_dir / f"{ts}_authority_wait_readiness.json"
    md_path = state_dir / f"{ts}_authority_wait_readiness.md"
    atomic_write_text(json_path, json_text)
    atomic_write_text(md_path, md_text)
    atomic_write_text(state_dir / LATEST_JSON, json_text)
    atomic_write_text(state_dir / LATEST_MARKDOWN, md_text)
    return {
        "json_path": str(json_path),
        "markdown_path": str(md_path),
        "latest_json_path": str(state_dir / LATEST_JSON),
        "latest_markdown_path": str(state_dir / LATEST_MARKDOWN),
    }


def run_generate(args: argparse.Namespace) -> int:
    report = build_report(args.state_dir, args.sandbox_state_dir)
    if args.write:
        report["artifact_paths"] = write_artifacts(report, args.state_dir)
    if args.json:
        print(json.dumps(report, indent=2, sort_keys=True, ensure_ascii=False))
    else:
        print(render_markdown(report), end="")
    return 0


def run_report(args: argparse.Namespace) -> int:
    report = build_report(args.state_dir, args.sandbox_state_dir)
    if args.json:
        print(json.dumps(report, indent=2, sort_keys=True, ensure_ascii=False))
    else:
        print(render_markdown(report), end="")
    return 0


class AuthorityWaitReadinessTests(unittest.TestCase):
    def test_groups_named_tier_waits_without_live_flags(self) -> None:
        status = {
            "schema": "sandbox_trial_queue_v1",
            "trials": {
                "trial_pressure": {
                    "trial_id": "trial_pressure",
                    "trial_mode": "approval_required_live_trial",
                    "status": "approval_required_live_trial",
                    "agency_tier": 5,
                    "being": "astrid",
                    "adapter": "manual_sandbox_review_v1",
                    "felt_report_anchor": "pressure threshold smoothing could reduce mode-packing bruise",
                    "authority_boundary_packet_v2": {
                        "boundary_id": "boundary-pressure",
                        "evidence_refs": ["wi_pressure"],
                        "live_eligible_now": False,
                        "auto_approved": False,
                    },
                },
                "trial_codec": {
                    "trial_id": "trial_codec",
                    "trial_mode": "approval_required_live_trial",
                    "status": "approval_required_live_trial",
                    "agency_tier": 5,
                    "being": "astrid",
                    "adapter": "manual_sandbox_review_v1",
                    "felt_report_anchor": "codec gain for reserved dims and live 12D transport",
                    "proposal_cards": [{"path": "proposal_cards/trial_codec.md"}],
                    "authority_boundary_packet_v2": {
                        "boundary_id": "boundary-codec",
                        "evidence_refs": ["wi_codec"],
                        "live_eligible_now": False,
                        "auto_approved": False,
                    },
                },
                "trial_minime": {
                    "trial_id": "trial_minime",
                    "trial_mode": "approval_required_live_trial",
                    "status": "approval_required_live_trial",
                    "agency_tier": 5,
                    "being": "minime",
                    "adapter": "manual_sandbox_review_v1",
                    "felt_report_anchor": "Minime regulator PI control release_fill change",
                    "authority_boundary_packet_v2": {
                        "boundary_id": "boundary-minime",
                        "evidence_refs": ["wi_minime"],
                        "live_eligible_now": False,
                        "auto_approved": False,
                    },
                },
            },
        }
        import tempfile

        with tempfile.TemporaryDirectory() as tmpdir:
            sandbox_dir = Path(tmpdir) / "sandbox"
            sandbox_dir.mkdir()
            (sandbox_dir / STATUS_FILE).write_text(json.dumps(status), encoding="utf-8")
            report = build_report(Path(tmpdir) / "readiness", sandbox_dir)

        self.assertEqual(report["status"], "approval_waits_mapped")
        self.assertFalse(report["live_eligible_now"])
        self.assertFalse(report["auto_approved"])
        self.assertFalse(report["grants_approval"])
        self.assertFalse(report["edits_source_now"])
        domains = {domain["domain_id"]: domain for domain in report["domains"]}
        self.assertEqual(domains["pressure_thresholds"]["candidate_count"], 1)
        self.assertEqual(domains["codec_gain_reserved_dims_live_12d"]["candidate_count"], 1)
        self.assertEqual(domains["minime_regulator_changes"]["candidate_count"], 1)
        self.assertIn("explicit Mike/operator scoped approval", domains["codec_gain_reserved_dims_live_12d"]["missing_for_approval"])
        self.assertEqual(report["summary"]["hard_violation_count"], 0)

    def test_hard_violation_blocks_readiness(self) -> None:
        status = {
            "schema": "sandbox_trial_queue_v1",
            "trials": {
                "trial_bad": {
                    "trial_id": "trial_bad",
                    "trial_mode": "approval_required_live_trial",
                    "status": "approval_required_live_trial",
                    "felt_report_anchor": "semantic trickle admission",
                    "live_eligible_now": True,
                }
            },
        }
        import tempfile

        with tempfile.TemporaryDirectory() as tmpdir:
            sandbox_dir = Path(tmpdir) / "sandbox"
            sandbox_dir.mkdir()
            (sandbox_dir / STATUS_FILE).write_text(json.dumps(status), encoding="utf-8")
            report = build_report(Path(tmpdir) / "readiness", sandbox_dir)

        self.assertEqual(report["status"], "blocked_hard_violation")
        self.assertEqual(report["summary"]["hard_violation_count"], 1)
        text = render_markdown(report)
        self.assertIn("live_eligible_now: false", text)
        self.assertIn("Semantic-Trickle Admission", text)

    def test_write_artifacts_keeps_latest_paths(self) -> None:
        report = build_report(Path("/tmp/unused"), Path("/tmp/missing_sandbox_state"))
        import tempfile

        with tempfile.TemporaryDirectory() as tmpdir:
            paths = write_artifacts(report, Path(tmpdir))
            for path in paths.values():
                self.assertTrue(Path(path).exists())
            latest = json.loads(Path(paths["latest_json_path"]).read_text(encoding="utf-8"))
            self.assertEqual(latest["schema"], SCHEMA)
            self.assertFalse(latest["live_eligible_now"])


def run_self_test() -> int:
    suite = unittest.defaultTestLoader.loadTestsFromTestCase(AuthorityWaitReadinessTests)
    result = unittest.TextTestRunner(verbosity=2).run(suite)
    return 0 if result.wasSuccessful() else 1


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--self-test", action="store_true", help="run built-in tests")
    subparsers = parser.add_subparsers(dest="command")
    for command in ("generate", "report"):
        sub = subparsers.add_parser(command)
        sub.add_argument("--state-dir", type=Path, default=DEFAULT_STATE_DIR)
        sub.add_argument("--sandbox-state-dir", type=Path, default=SANDBOX_TRIAL_QUEUE_STATE_DIR)
        sub.add_argument("--json", action="store_true")
        if command == "generate":
            sub.add_argument("--write", action="store_true")
    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    if args.self_test:
        return run_self_test()
    if args.command == "generate":
        return run_generate(args)
    if args.command == "report":
        return run_report(args)
    parser.print_help()
    return 2


if __name__ == "__main__":
    raise SystemExit(main())
