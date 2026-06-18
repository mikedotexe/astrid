#!/usr/bin/env python3
"""Assemble a steward-only offline wider-readout A/B packet.

The packet is evidence and review prep only. It does not issue invitations,
change runtime behavior, edit env vars, or restart services.
"""
from __future__ import annotations

import argparse
import json
import sys
import time
import unittest
from pathlib import Path
from typing import Any, TextIO

SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

import shared_substrate_workbench

ASTRID_ROOT = Path("/Users/v/other/astrid")
ASTRID_BRIDGE = ASTRID_ROOT / "capsules/spectral-bridge"
DEFAULT_OUTPUT_ROOT = ASTRID_BRIDGE / "workspace/diagnostics/wider_readout_ab_packets"
READY_OFFLINE = "ready_for_steward_offline_comparison"
RUNTIME_CHANGE_NONE = "none"


def _now_iso() -> str:
    return time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())


def _default_run_id(now: str) -> str:
    safe = now.replace(":", "").replace("-", "")
    return f"wider-readout-ab-{safe}"


def _as_dict(value: Any) -> dict[str, Any]:
    return value if isinstance(value, dict) else {}


def _packet_status(probe: dict[str, Any]) -> str:
    if probe.get("offline_readiness") == READY_OFFLINE:
        return "ready_for_steward_packet_review"
    return "blocked_by_workbench_readiness"


def build_packet(report: dict[str, Any], *, now: str | None = None, run_id: str | None = None) -> dict[str, Any]:
    """Build a read-only packet from a Shared Substrate Workbench report."""
    now = now or _now_iso()
    probe = _as_dict(report.get("wider_readout_ab_probe"))
    evidence = _as_dict(probe.get("evidence"))
    run_id = run_id or _default_run_id(now)
    status = _packet_status(probe)
    return {
        "schema_version": 1,
        "policy": "wider_readout_ab_packet_v1",
        "run_id": run_id,
        "generated_at": now,
        "source_workbench_generated_at": report.get("generated_at"),
        "status": status,
        "runtime_change": RUNTIME_CHANGE_NONE,
        "pressure_target": "steward",
        "being_obligation": "none",
        "offline_readiness": probe.get("offline_readiness"),
        "runtime_readiness": probe.get("runtime_readiness"),
        "runtime_blockers": probe.get("runtime_blockers") or [],
        "caution_flags": probe.get("caution_flags") or [],
        "comparison_arms": probe.get("comparison_arms") or [],
        "evidence": {
            "projection_variance_check": evidence.get("projection_variance_check") or {},
            "aperture_gift_queue": evidence.get("aperture_gift_queue") or {},
            "astrid_readout_locus": evidence.get("astrid_readout_locus") or {},
            "identity_anchor_locus": evidence.get("identity_anchor_locus") or {},
            "relational_gift_locus": evidence.get("relational_gift_locus") or {},
            "minime_modal_porosity_locus": evidence.get("minime_modal_porosity_locus") or {},
            "minime_pressure_porosity": evidence.get("minime_pressure_porosity") or {},
            "pressure_source_audit": evidence.get("pressure_source_audit") or {},
            "mode_packing_feeder_audit": evidence.get("mode_packing_feeder_audit") or {},
            "mode_share_pressure_source_probe": evidence.get("mode_share_pressure_source_probe") or {},
            "resistance_gradient_full_review": evidence.get("resistance_gradient_full_review") or {},
            "astrid_current_read": evidence.get("astrid_current_read") or {},
        },
        "evaluation_questions": probe.get("evaluation_questions") or [],
        "steward_review_steps": [
            "Read this packet as product evidence, not deployment permission.",
            "Compare the current and candidate arms for identity anchor, clarity, and lane-contract risk.",
            "If the packet remains promising, issue one targeted both-being grounded design review.",
            "Do not change voice/readout, codec, density, tail participation, env vars, or services from this packet alone.",
        ],
    }


def render_markdown(packet: dict[str, Any]) -> str:
    evidence = _as_dict(packet.get("evidence"))
    projection = _as_dict(evidence.get("projection_variance_check"))
    projection_metrics = _as_dict(projection.get("metrics"))
    gift_queue = _as_dict(evidence.get("aperture_gift_queue"))
    pressure = _as_dict(evidence.get("minime_pressure_porosity"))
    mode_share_probe = _as_dict(evidence.get("mode_share_pressure_source_probe"))
    resistance_review = _as_dict(evidence.get("resistance_gradient_full_review"))
    astrid = _as_dict(evidence.get("astrid_current_read"))
    lines = [
        "# Wider Readout A/B Packet",
        "",
        f"- run_id: `{packet.get('run_id')}`",
        f"- generated_at: `{packet.get('generated_at')}`",
        f"- source_workbench_generated_at: `{packet.get('source_workbench_generated_at')}`",
        f"- status: `{packet.get('status')}`",
        f"- runtime_change: `{packet.get('runtime_change')}`",
        f"- pressure_target: `{packet.get('pressure_target')}`",
        f"- being_obligation: `{packet.get('being_obligation')}`",
        "",
        "## Gate",
        "",
        f"- offline_readiness: `{packet.get('offline_readiness')}`",
        f"- runtime_readiness: `{packet.get('runtime_readiness')}`",
        f"- runtime_blockers: `{', '.join(packet.get('runtime_blockers') or []) or 'none'}`",
        f"- caution_flags: `{', '.join(packet.get('caution_flags') or []) or 'none'}`",
        "",
        "## Comparison Arms",
        "",
    ]
    for arm in packet.get("comparison_arms") or []:
        if isinstance(arm, dict):
            lines.append(
                f"- `{arm.get('id')}`: runtime_change=`{arm.get('runtime_change')}`; {arm.get('description')}"
            )
    lines.extend(
        [
            "",
            "## Evidence Snapshot",
            "",
            f"- projection status: `{projection.get('status', 'missing')}`",
            f"- hidden/visible/dynamic variance: `{projection_metrics.get('hidden_projected_variance')}` / "
            f"`{projection_metrics.get('visible_projected_variance')}` / "
            f"`{projection_metrics.get('dynamic_variance_delta')}`",
            f"- aperture gift queue: open=`{gift_queue.get('open_count')}`, "
            f"actionable=`{gift_queue.get('actionable_open_count')}`, "
            f"unclosed_issued=`{gift_queue.get('unclosed_issued_count')}`",
            f"- Minime pressure/porosity: `{pressure.get('pressure_quality')}` / "
            f"`{pressure.get('porosity_score')}`",
            f"- mode-share pressure probe: `{mode_share_probe.get('verdict', 'missing')}`",
            f"- resistance gradient review: status=`{resistance_review.get('status', 'missing')}`, "
            f"include=`{resistance_review.get('include_in_both_being_review')}`, "
            f"scope=`{resistance_review.get('recommended_scope')}`, "
            f"shapes=`{resistance_review.get('review_shape_counts')}`",
            f"- Astrid aperture/readout/tail: `{astrid.get('aperture')}` / "
            f"`{astrid.get('response_length')}` / "
            f"`{astrid.get('effective_tail_participation')}`",
            "",
            "## Evaluation Questions",
            "",
        ]
    )
    for question in packet.get("evaluation_questions") or []:
        lines.append(f"- {question}")
    lines.extend(["", "## Steward Review Steps", ""])
    for step in packet.get("steward_review_steps") or []:
        lines.append(f"- {step}")
    return "\n".join(lines).rstrip() + "\n"


def write_packet(packet: dict[str, Any], output_root: Path) -> dict[str, Any]:
    target_dir = output_root / str(packet["run_id"])
    target_dir.mkdir(parents=True, exist_ok=True)
    json_path = target_dir / "packet.json"
    md_path = target_dir / "packet.md"
    json_path.write_text(json.dumps(packet, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    md_path.write_text(render_markdown(packet), encoding="utf-8")
    packet["output_dir"] = str(target_dir)
    packet["packet_json"] = str(json_path)
    packet["packet_md"] = str(md_path)
    return packet


def emit(packet: dict[str, Any], *, as_json: bool, stdout: TextIO) -> None:
    if as_json:
        stdout.write(json.dumps(packet, indent=2, sort_keys=True) + "\n")
    else:
        stdout.write(render_markdown(packet))


class WiderReadoutAbPacketTests(unittest.TestCase):
    def _report(self, readiness: str = READY_OFFLINE) -> dict[str, Any]:
        return {
            "generated_at": "2026-06-16T00:00:00Z",
            "wider_readout_ab_probe": {
                "offline_readiness": readiness,
                "runtime_readiness": "blocked_until_both_being_grounding_and_operator_flag",
                "runtime_blockers": ["wider-voice-readout-codesign"],
                "caution_flags": ["minime_pressure_window_not_clean"],
                "comparison_arms": [{"id": "control_current_readout", "runtime_change": "none"}],
                "evidence": {
                    "projection_variance_check": {"status": "present", "metrics": {}},
                    "aperture_gift_queue": {"open_count": 0, "unclosed_issued_count": 0},
                    "minime_pressure_porosity": {"pressure_quality": "mixed_pressure"},
                    "mode_share_pressure_source_probe": {
                        "verdict": "WATCH - keep runtime nudges held",
                        "steward_actions_now": [
                            {"id": "simplify_active_thread_context", "runtime_change": "none"}
                        ],
                    },
                    "resistance_gradient_full_review": {
                        "status": "present",
                        "include_in_both_being_review": True,
                        "recommended_scope": "compact_match_partial_miss_appendix",
                        "review_shape_counts": {"match": 13, "partial_match": 11},
                        "runtime_change": "none",
                    },
                    "astrid_current_read": {"aperture": 1.0},
                },
                "evaluation_questions": ["Does it preserve identity?"],
            },
        }

    def test_ready_packet_is_runtime_none_and_steward_only(self) -> None:
        packet = build_packet(self._report(), now="2026-06-16T00:00:00Z")
        self.assertEqual(packet["status"], "ready_for_steward_packet_review")
        self.assertEqual(packet["runtime_change"], "none")
        self.assertIn("mode_share_pressure_source_probe", packet["evidence"])
        self.assertIn("resistance_gradient_full_review", packet["evidence"])
        self.assertTrue(
            packet["evidence"]["resistance_gradient_full_review"][
                "include_in_both_being_review"
            ]
        )
        encoded = json.dumps(packet)
        self.assertNotIn("must respond", encoded)
        self.assertNotIn("being follow-up", encoded)

    def test_hold_readiness_blocks_default_write_policy(self) -> None:
        packet = build_packet(self._report("hold_for_aperture_gift_closure"))
        self.assertEqual(packet["status"], "blocked_by_workbench_readiness")

    def test_markdown_contains_gate_and_evidence(self) -> None:
        markdown = render_markdown(build_packet(self._report()))
        self.assertIn("Wider Readout A/B Packet", markdown)
        self.assertIn("offline_readiness", markdown)
        self.assertIn("Evidence Snapshot", markdown)
        self.assertIn("mode-share pressure probe", markdown)
        self.assertIn("resistance gradient review", markdown)
        self.assertIn("compact_match_partial_miss_appendix", markdown)

    def test_write_packet_writes_json_and_markdown_only(self) -> None:
        import tempfile

        packet = build_packet(self._report(), run_id="test-run")
        with tempfile.TemporaryDirectory() as td:
            written = write_packet(packet, Path(td))
            self.assertTrue(Path(written["packet_json"]).exists())
            self.assertTrue(Path(written["packet_md"]).exists())
            self.assertEqual(sorted(path.name for path in Path(td, "test-run").iterdir()), ["packet.json", "packet.md"])


def run_self_tests() -> int:
    suite = unittest.TestLoader().loadTestsFromTestCase(WiderReadoutAbPacketTests)
    result = unittest.TextTestRunner(verbosity=2).run(suite)
    return 0 if result.wasSuccessful() else 1


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Build an offline wider-readout A/B packet")
    parser.add_argument("--json", action="store_true", help="Emit packet JSON")
    parser.add_argument("--write", action="store_true", help="Write packet files")
    parser.add_argument("--allow-hold", action="store_true", help="Allow writing a blocked packet")
    parser.add_argument("--run-id", help="Override packet run id")
    parser.add_argument("--out-dir", type=Path, default=DEFAULT_OUTPUT_ROOT, help="Output root for --write")
    parser.add_argument("--self-test", action="store_true", help="Run unit tests and exit")
    args = parser.parse_args(argv)

    if args.self_test:
        return run_self_tests()

    report = shared_substrate_workbench.build_workbench()
    packet = build_packet(report, run_id=args.run_id)
    if args.write:
        if packet["status"] != "ready_for_steward_packet_review" and not args.allow_hold:
            sys.stderr.write(
                f"refusing to write packet while status={packet['status']} "
                f"offline_readiness={packet.get('offline_readiness')}\n"
            )
            return 2
        packet = write_packet(packet, args.out_dir)
    emit(packet, as_json=args.json, stdout=sys.stdout)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
