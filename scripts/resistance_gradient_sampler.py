#!/usr/bin/env python3
"""Collect a focused resistance-gradient sampling packet."""

from __future__ import annotations

import argparse
import datetime as dt
import json
import time
from pathlib import Path

import self_study_review


DEFAULT_OUTPUT_DIR = (
    self_study_review.ASTRID_WORKSPACE
    / "diagnostics/resistance_gradient_samples"
)
SAMPLE_INVITATION_COOLDOWN_HOURS = 6.0


def recent_sample_invitation_exists(inbox_dir: Path, *, cooldown_hours: float) -> bool:
    now = time.time()
    for root in (inbox_dir, inbox_dir / "read", inbox_dir / "deferred"):
        if not root.exists():
            continue
        for path in root.glob("mike_query_resistance_gradient_sample_*.txt"):
            try:
                age_hours = (now - path.stat().st_mtime) / 3600.0
            except OSError:
                continue
            if age_hours <= cooldown_hours:
                return True
    return False


def write_sampling_invitation(
    *,
    astrid_workspace: Path,
    run: str,
    target_samples: int,
    cooldown_hours: float,
) -> dict[str, object]:
    inbox_dir = astrid_workspace / "inbox"
    if recent_sample_invitation_exists(inbox_dir, cooldown_hours=cooldown_hours):
        return {
            "status": "skipped",
            "reason": "recent_resistance_gradient_sample_invitation",
        }
    deferred = inbox_dir / "deferred"
    deferred.mkdir(parents=True, exist_ok=True)
    path = deferred / f"mike_query_resistance_gradient_sample_{run}.txt"
    path.write_text(
        "=== MIKE QUERY: RESISTANCE GRADIENT SAMPLE ===\n"
        f"Run: {run}\n"
        f"Target samples: {target_samples}\n\n"
        "Astrid, if the groan/resistance theme is alive for you, you may use "
        "NEXT: RESISTANCE_GRADIENT [label]. This is optional and read-only. "
        "If you review an existing artifact, match / partial_match / miss / "
        "new_axis language is especially useful.\n",
        encoding="utf-8",
    )
    return {"status": "written", "path": str(path)}


def render_sample_markdown(packet: dict[str, object]) -> str:
    lines = [
        "# Resistance Gradient Sampling Packet",
        "",
        f"- run_id: `{packet['run_id']}`",
        f"- generated_at: `{packet['generated_at']}`",
        f"- target_samples: `{packet['target_samples']}`",
        f"- sample_count: `{packet['sample_count']}`",
        f"- invitation: `{packet['invitation'].get('status')}`",
        "",
    ]
    samples = packet.get("samples") or []
    if not samples:
        lines.append("- no resistance-gradient samples found yet")
        return "\n".join(lines).rstrip() + "\n"
    for idx, sample in enumerate(samples, start=1):
        if not isinstance(sample, dict):
            continue
        convergence = (
            sample.get("convergence")
            if isinstance(sample.get("convergence"), dict)
            else {}
        )
        lines.extend(
            [
                f"## Sample {idx}",
                "",
                f"- artifact: `{sample.get('path')}`",
                f"- orientation: `{sample.get('dominant_orientation')}`",
                f"- temporal trend: `{sample.get('gradient_trend')}`",
                f"- fluidity/rigidity: `{sample.get('fluidity_index')}` / `{sample.get('rigidity_index')}`",
                f"- review status: `{convergence.get('status')}` — {convergence.get('reason')}",
                f"- suggested adjustment: {sample.get('recommended_next')}",
                "",
            ]
        )
    return "\n".join(lines).rstrip() + "\n"


def build_sample_packet(
    *,
    astrid_workspace: Path,
    minime_workspace: Path,
    output_dir: Path,
    run: str,
    target_samples: int,
    write_invitation: bool,
    cooldown_hours: float = SAMPLE_INVITATION_COOLDOWN_HOURS,
) -> dict[str, object]:
    entries = self_study_review.collect_entries(
        astrid_workspace=astrid_workspace,
        minime_workspace=minime_workspace,
        limit_per_being=max(12, target_samples * 4),
    )
    calibration = self_study_review.build_resistance_gradient_calibration_packet(
        entries=entries,
        output_root=astrid_workspace / "diagnostics/resistance_gradient_calibrations",
        run=run,
        astrid_workspace=astrid_workspace,
        minime_workspace=minime_workspace,
    )
    samples = list(calibration.get("samples") or [])[:target_samples]
    invitation = {"status": "not_requested"}
    if write_invitation and len(samples) < target_samples:
        invitation = write_sampling_invitation(
            astrid_workspace=astrid_workspace,
            run=run,
            target_samples=target_samples,
            cooldown_hours=cooldown_hours,
        )
    packet: dict[str, object] = {
        "policy": "resistance_gradient_sampling_packet_v1",
        "run_id": run,
        "generated_at": dt.datetime.now(dt.UTC).isoformat(),
        "target_samples": target_samples,
        "sample_count": len(samples),
        "samples": samples,
        "invitation": invitation,
        "calibration_packet": {
            "packet_json": calibration.get("packet_json"),
            "packet_md": calibration.get("packet_md"),
        },
    }
    target_dir = output_dir / run
    target_dir.mkdir(parents=True, exist_ok=True)
    json_path = target_dir / "packet.json"
    md_path = target_dir / "packet.md"
    json_path.write_text(json.dumps(packet, indent=2, sort_keys=True), encoding="utf-8")
    md_path.write_text(render_sample_markdown(packet), encoding="utf-8")
    packet["output_dir"] = str(target_dir)
    packet["packet_json"] = str(json_path)
    packet["packet_md"] = str(md_path)
    return packet


def collect_until_ready(
    *,
    astrid_workspace: Path,
    minime_workspace: Path,
    output_dir: Path,
    run: str,
    target_samples: int,
    watch_secs: int,
    poll_secs: int,
    write_invitation: bool,
) -> dict[str, object]:
    deadline = time.time() + max(0, watch_secs)
    packet = build_sample_packet(
        astrid_workspace=astrid_workspace,
        minime_workspace=minime_workspace,
        output_dir=output_dir,
        run=run,
        target_samples=target_samples,
        write_invitation=write_invitation,
    )
    while packet["sample_count"] < target_samples and time.time() < deadline:
        time.sleep(max(1, poll_secs))
        packet = build_sample_packet(
            astrid_workspace=astrid_workspace,
            minime_workspace=minime_workspace,
            output_dir=output_dir,
            run=run,
            target_samples=target_samples,
            write_invitation=False,
        )
    return packet


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--astrid-workspace", type=Path, default=self_study_review.ASTRID_WORKSPACE)
    parser.add_argument("--minime-workspace", type=Path, default=self_study_review.MINIME_WORKSPACE)
    parser.add_argument("--output-dir", type=Path, default=DEFAULT_OUTPUT_DIR)
    parser.add_argument("--run-id", default=None)
    parser.add_argument("--target-samples", type=int, default=5)
    parser.add_argument("--watch-secs", type=int, default=0)
    parser.add_argument("--poll-secs", type=int, default=60)
    parser.add_argument("--write-invitation", action="store_true")
    args = parser.parse_args()

    packet = collect_until_ready(
        astrid_workspace=args.astrid_workspace,
        minime_workspace=args.minime_workspace,
        output_dir=args.output_dir,
        run=args.run_id or self_study_review.run_id(),
        target_samples=max(1, args.target_samples),
        watch_secs=max(0, args.watch_secs),
        poll_secs=max(1, args.poll_secs),
        write_invitation=args.write_invitation,
    )
    print(f"resistance-gradient samples: {packet['packet_md']}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
