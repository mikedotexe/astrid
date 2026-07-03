#!/usr/bin/env python3
"""Read-only cartography for coupled pressure with usable structure.

This steward diagnostic grounds Astrid's "settled coupling" / "restless
texture" roadmap against public telemetry, public pressure surfaces, and
optional isolated reservoir probes. It never writes to being inboxes, prompt
surfaces, dials, launchd state, or runtime controls.
"""

from __future__ import annotations

import argparse
import datetime as dt
import json
import re
import subprocess
import sys
import tempfile
import time
import unittest
from collections import Counter
from dataclasses import dataclass
from pathlib import Path
from typing import Any

SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

from being_privacy import is_steward_private

ASTRID_ROOT = Path(__file__).resolve().parents[1]
ASTRID_WORKSPACE = ASTRID_ROOT / "capsules/spectral-bridge/workspace"
MINIME_WORKSPACE = ASTRID_ROOT.parent / "minime/workspace"
DEFAULT_OUTPUT_ROOT = (
    ASTRID_WORKSPACE / "diagnostics/coupled_pressure_cartography"
)
SCHEMA_VERSION = 1
READ_LIMIT_CHARS = 6_000

SETTLED_POLE = (
    "settled coupling, stable core, foothold, anchor, structural center, "
    "gravity held safely underfoot"
)
RESTLESS_POLE = (
    "restless texture, living tail, room to breathe, flickering periphery, "
    "movement without losing foothold"
)

FAMILIES: dict[str, tuple[str, ...]] = {
    "anchor": (
        "settled coupling",
        "stable-core",
        "stable core",
        "foothold",
        "anchor",
        "structural center",
        "lambda1",
        "lambda 1",
        "gravity",
    ),
    "texture": (
        "restless texture",
        "living tail",
        "tail",
        "flickering",
        "room to breathe",
        "high-entropy",
        "entropy",
        "periphery",
    ),
    "pressure": (
        "pressure",
        "overpacked",
        "mode_packing",
        "mode packing",
        "density",
        "viscous",
        "silt",
        "medium",
        "friction",
        "heavy",
    ),
    "movement": (
        "micro-breathing",
        "breathing",
        "expanding",
        "contracting",
        "phase",
        "transition",
        "pulse",
        "drift",
        "dispersal",
        "movement",
        "dance",
    ),
}


@dataclass(frozen=True)
class Entry:
    being: str
    channel: str
    path: Path
    mtime: float
    text: str


def read_json(path: Path) -> dict[str, Any]:
    try:
        data = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        return {}
    return data if isinstance(data, dict) else {}


def safe_float(value: Any) -> float | None:
    if isinstance(value, bool):
        return None
    try:
        return float(value)
    except (TypeError, ValueError):
        return None


def rounded(value: Any, digits: int = 4) -> float | str:
    number = safe_float(value)
    if number is None:
        return "unknown"
    return round(number, digits)


def path_age_s(path: Path) -> float | str:
    try:
        return round(time.time() - path.stat().st_mtime, 1)
    except OSError:
        return "unknown"


def read_bounded(path: Path, limit: int = READ_LIMIT_CHARS) -> str:
    try:
        with path.open("r", encoding="utf-8", errors="replace") as handle:
            return handle.read(limit)
    except OSError:
        return ""


def collect_paths(
    root: Path,
    pattern: str,
    cutoff: float,
    *,
    recursive: bool = False,
) -> list[Path]:
    if not root.is_dir():
        return []
    paths = root.rglob(pattern) if recursive else root.glob(pattern)
    out: list[Path] = []
    for path in paths:
        if not path.is_file():
            continue
        try:
            if path.stat().st_mtime >= cutoff:
                out.append(path)
        except OSError:
            continue
    return sorted(out, key=lambda item: item.stat().st_mtime, reverse=True)


def count_minime_private_candidates(minime_workspace: Path, cutoff: float) -> int:
    journal = minime_workspace / "journal"
    if not journal.is_dir():
        return 0
    count = 0
    for path in collect_paths(journal, "*.txt", cutoff):
        if is_steward_private("minime", path):
            count += 1
    return count


def collect_entries(
    astrid_workspace: Path,
    minime_workspace: Path,
    cutoff: float,
) -> tuple[list[Entry], dict[str, Any]]:
    specs = [
        ("astrid", "journal_longform", astrid_workspace / "journal", "dialogue_longform_*.txt"),
        ("astrid", "journal_daydream", astrid_workspace / "journal", "daydream_*.txt"),
        (
            "astrid",
            "steward_report",
            astrid_workspace / "outbox",
            "steward_report_*.txt",
        ),
        (
            "astrid",
            "steward_delivered",
            astrid_workspace / "outbox/steward_delivered",
            "steward_report_*.txt",
        ),
        ("minime", "pressure_journal", minime_workspace / "journal", "pressure_*.txt"),
        ("minime", "self_study", minime_workspace / "journal", "self_study_*.txt"),
        ("minime", "introspection", minime_workspace / "introspections", "*.txt"),
    ]
    entries: list[Entry] = []
    for being, channel, root, pattern in specs:
        for path in collect_paths(root, pattern, cutoff):
            if being == "minime" and is_steward_private(being, path):
                continue
            text = read_bounded(path)
            if text:
                entries.append(
                    Entry(
                        being=being,
                        channel=channel,
                        path=path,
                        mtime=path.stat().st_mtime,
                        text=text,
                    )
                )
    entries.sort(key=lambda item: item.mtime, reverse=True)
    privacy = {
        "minime_private_candidates_skipped": count_minime_private_candidates(
            minime_workspace,
            cutoff,
        ),
        "policy": "minime private qualia skipped by content marker via being_privacy",
    }
    return entries, privacy


def slot_summary(workspace: Path) -> dict[str, Any]:
    slot = read_json(workspace / "open_steward_query.json")
    if not slot:
        return {"open": False}
    return {
        "open": True,
        "subject": slot.get("subject", "unknown"),
        "file": slot.get("file", "unknown"),
        "ts": slot.get("ts", "unknown"),
    }


def first_dict(*values: Any) -> dict[str, Any]:
    for value in values:
        if isinstance(value, dict):
            return value
    return {}


def current_state(
    astrid_workspace: Path,
    minime_workspace: Path,
) -> dict[str, Any]:
    health_path = minime_workspace / "health.json"
    spectral_path = minime_workspace / "spectral_state.json"
    regulator_path = minime_workspace / "regulator_context.json"
    condition_path = minime_workspace / "condition_metrics.json"
    health = read_json(health_path)
    spectral = read_json(spectral_path)
    regulator = read_json(regulator_path)
    condition = read_json(condition_path)
    resonance = first_dict(
        spectral.get("resonance_density_v1"),
        health.get("resonance_density_v1"),
    )
    pressure = first_dict(
        spectral.get("pressure_source_v1"),
        health.get("pressure_source_v1"),
    )
    inhabitable = first_dict(
        spectral.get("inhabitable_fluctuation_v1"),
        health.get("inhabitable_fluctuation_v1"),
    )
    return {
        "minime": {
            "telemetry_age_s": {
                "health": path_age_s(health_path),
                "spectral_state": path_age_s(spectral_path),
                "regulator_context": path_age_s(regulator_path),
                "condition_metrics": path_age_s(condition_path),
            },
            "fill_pct": rounded(spectral.get("fill_pct", health.get("fill_pct"))),
            "lambda1": rounded(spectral.get("lambda1", health.get("lambda1"))),
            "spectral_entropy": rounded(spectral.get("spectral_entropy")),
            "effective_dimensionality": rounded(
                spectral.get("effective_dimensionality")
            ),
            "distinguishability_loss": rounded(
                spectral.get("distinguishability_loss")
            ),
            "phase": spectral.get("phase", health.get("phase", "unknown")),
            "resonance_density": {
                "quality": resonance.get("quality", "unknown"),
                "density": rounded(resonance.get("density")),
                "containment_score": rounded(resonance.get("containment_score")),
                "pressure_risk": rounded(resonance.get("pressure_risk")),
                "mode_packing": rounded(
                    first_dict(resonance.get("components")).get("mode_packing")
                ),
            },
            "pressure_source": {
                "dominant_source": pressure.get("dominant_source", "unknown"),
                "pressure_score": rounded(pressure.get("pressure_score")),
                "porosity_score": rounded(pressure.get("porosity_score")),
                "mode_packing": rounded(
                    first_dict(pressure.get("components")).get("mode_packing")
                ),
                "control_applied": first_dict(pressure.get("control")).get(
                    "applied_locally",
                    "unknown",
                ),
            },
            "inhabitable_fluctuation": {
                "quality": inhabitable.get("quality", "unknown"),
                "inhabitability_score": rounded(
                    inhabitable.get("inhabitability_score")
                ),
                "foothold_stability": rounded(
                    inhabitable.get("foothold_stability")
                ),
                "fluctuation_score": rounded(inhabitable.get("fluctuation_score")),
                "rearrangement_intensity": rounded(
                    inhabitable.get("rearrangement_intensity")
                ),
                "pressure_interference": rounded(
                    first_dict(inhabitable.get("components")).get(
                        "pressure_interference"
                    )
                ),
            },
            "regulator": {
                "fill_band": regulator.get(
                    "fill_band",
                    health.get("fill_band", "unknown"),
                ),
                "dfill_dt": rounded(regulator.get("dfill_dt", health.get("dfill_dt"))),
                "phase_transition": regulator.get(
                    "phase_transition",
                    health.get("phase_transition", "unknown"),
                ),
            },
            "condition_metrics_present": bool(condition),
        },
        "open_steward_slots": {
            "astrid": slot_summary(astrid_workspace),
            "minime": slot_summary(minime_workspace),
        },
    }


def family_counts(entries: list[Entry]) -> dict[str, dict[str, int]]:
    counts: dict[str, Counter[str]] = {
        "astrid": Counter(),
        "minime": Counter(),
    }
    for entry in entries:
        lower = entry.text.lower()
        for family, terms in FAMILIES.items():
            hits = sum(lower.count(term.lower()) for term in terms)
            if hits:
                counts[entry.being][family] += hits
    return {being: dict(counter) for being, counter in counts.items()}


def source_summaries(entries: list[Entry], limit: int = 10) -> list[dict[str, Any]]:
    summaries = []
    for entry in entries[:limit]:
        excerpt = " ".join(
            line.strip()
            for line in entry.text.splitlines()
            if line.strip() and not line.strip().startswith("---")
        )
        summaries.append(
            {
                "being": entry.being,
                "channel": entry.channel,
                "path": str(entry.path),
                "mtime": entry.mtime,
                "excerpt": excerpt[:240],
            }
        )
    return summaries


def run_letter_response_scan(since_hours: float) -> dict[str, Any]:
    cmd = [
        sys.executable,
        str(SCRIPT_DIR / "letter_response_scan.py"),
        "--being",
        "both",
        "--since-hours",
        str(max(since_hours, 0.1)),
        "--window-hours",
        "1",
        "--json",
    ]
    try:
        proc = subprocess.run(
            cmd,
            cwd=str(ASTRID_ROOT),
            text=True,
            capture_output=True,
            timeout=20,
            check=False,
        )
    except (OSError, subprocess.SubprocessError) as exc:
        return {"available": False, "error": str(exc), "items": []}
    if proc.returncode != 0:
        return {
            "available": False,
            "error": (proc.stderr or proc.stdout).strip()[:500],
            "items": [],
        }
    try:
        items = json.loads(proc.stdout)
    except json.JSONDecodeError as exc:
        return {"available": False, "error": str(exc), "items": []}
    compact = []
    for item in items[:8] if isinstance(items, list) else []:
        engaged = item.get("engaged") if isinstance(item, dict) else None
        compact.append(
            {
                "being": item.get("being"),
                "letter": item.get("letter"),
                "status": item.get("status"),
                "stance": engaged.get("stance") if isinstance(engaged, dict) else None,
                "engaged_file": engaged.get("file") if isinstance(engaged, dict) else None,
                "excerpt": engaged.get("excerpt") if isinstance(engaged, dict) else None,
            }
        )
    return {"available": True, "items": compact}


def parse_substrate_probe_output(text: str) -> dict[str, Any]:
    status = "unknown"
    verdict_match = re.search(r"VERDICT:\s*(.+)", text)
    verdict = verdict_match.group(1).strip() if verdict_match else ""
    verdict_upper = verdict.upper()
    if "LOCKED" in verdict_upper:
        status = "locked"
    elif "FLUID" in verdict_upper:
        status = "fluid"
    elif "SEPARABLE" in verdict_upper:
        status = "separable"
    elif "INCONCLUSIVE" in verdict_upper:
        status = "inconclusive"

    def number_after(pattern: str) -> float | None:
        match = re.search(pattern, text)
        return safe_float(match.group(1)) if match else None

    onset_match = re.search(r"separation onset:\s*([^(\n]+)", text)
    onset_text = onset_match.group(1).strip() if onset_match else "unknown"
    return {
        "status": status,
        "verdict": verdict or "unknown",
        "divergence": number_after(r"divergence:\s*([+-]?\d+(?:\.\d+)?)"),
        "inject_corr": number_after(r"inject-only corr:\s*([+-]?\d+(?:\.\d+)?)"),
        "gap_at_2": number_after(r"gap@2=([+-]?\d+(?:\.\d+)?)"),
        "gap_final": number_after(r"gap@\d+=([+-]?\d+(?:\.\d+)?)"),
        "separation_onset": onset_text,
        "raw": text.strip(),
    }


def run_substrate_probe(being: str, ticks: int) -> dict[str, Any]:
    cmd = [
        sys.executable,
        str(SCRIPT_DIR / "substrate_probe.py"),
        "--being",
        being,
        "--pole-a",
        SETTLED_POLE,
        "--pole-b",
        RESTLESS_POLE,
        "--label-a",
        "settled",
        "--label-b",
        "restless",
        "--ticks",
        str(max(1, ticks)),
    ]
    try:
        proc = subprocess.run(
            cmd,
            cwd=str(ASTRID_ROOT),
            text=True,
            capture_output=True,
            timeout=45,
            check=False,
        )
    except (OSError, subprocess.SubprocessError) as exc:
        return {"available": False, "error": str(exc)}
    output = (proc.stdout or proc.stderr).strip()
    if proc.returncode != 0:
        return {"available": False, "error": output[:1_000]}
    parsed = parse_substrate_probe_output(output)
    parsed.update({"available": True})
    return parsed


def build_cartography(
    current: dict[str, Any],
    counts: dict[str, dict[str, int]],
    substrate: dict[str, Any],
) -> dict[str, Any]:
    minime = current["minime"]
    resonance = minime["resonance_density"]
    pressure = minime["pressure_source"]
    inhabitable = minime["inhabitable_fluctuation"]
    shared = sorted(
        family
        for family in FAMILIES
        if counts.get("astrid", {}).get(family, 0) > 0
        and counts.get("minime", {}).get(family, 0) > 0
    )
    probe_statuses = {
        being: probe.get("status", "unknown")
        for being, probe in substrate.items()
        if isinstance(probe, dict) and probe.get("available")
    }
    if any(status in {"locked", "unknown"} for status in probe_statuses.values()):
        axis = "sticky_or_unproven"
    elif any(status == "separable" for status in probe_statuses.values()):
        axis = "partly_separable"
    elif any(status == "fluid" for status in probe_statuses.values()):
        axis = "fluid"
    else:
        axis = "not_measured"
    return {
        "anchor": {
            "telemetry_basis": {
                "lambda1": minime.get("lambda1"),
                "foothold_stability": inhabitable.get("foothold_stability"),
                "inhabitability_quality": inhabitable.get("quality"),
            },
            "read": "settled coupling is treated as structural foothold, not a target to erase",
        },
        "texture": {
            "telemetry_basis": {
                "spectral_entropy": minime.get("spectral_entropy"),
                "fluctuation_score": inhabitable.get("fluctuation_score"),
                "mode_packing": resonance.get("mode_packing"),
            },
            "read": "restless texture is treated as movement capacity inside the hold",
        },
        "pressure": {
            "telemetry_basis": {
                "dominant_source": pressure.get("dominant_source"),
                "pressure_risk": resonance.get("pressure_risk"),
                "porosity_score": pressure.get("porosity_score"),
                "resonance_quality": resonance.get("quality"),
            },
            "read": "pressure is presently mapped as medium/friction before any control claim",
        },
        "movement": {
            "telemetry_basis": {
                "phase": minime.get("phase"),
                "dfill_dt": minime.get("regulator", {}).get("dfill_dt"),
                "phase_transition": minime.get("regulator", {}).get(
                    "phase_transition"
                ),
            },
            "read": "movement should be measured as returnable fluctuation, not simple relief",
        },
        "axis_status": axis,
        "shared_families": shared,
    }


def recommended_next(
    current: dict[str, Any],
    cartography: dict[str, Any],
    substrate: dict[str, Any],
) -> list[str]:
    recommendations = [
        "Keep this steward-facing and read-only; do not tune dials or controller thresholds from this packet alone.",
    ]
    if cartography["axis_status"] in {"sticky_or_unproven", "not_measured"}:
        recommendations.append(
            "Treat settled/restless as a sticky cartography axis: gather another public-window packet before proposing intervention."
        )
    else:
        recommendations.append(
            "If the axis separates repeatedly, prepare a separate consent-with-evidence plan before any live change."
        )
    minime_slot = current.get("open_steward_slots", {}).get("minime", {})
    if minime_slot.get("open"):
        recommendations.append(
            "Do not overwrite Minime's open steward self-study slot; wait for engagement or close it explicitly if steward-side stale handling is needed."
        )
    if any(
        isinstance(probe, dict) and not probe.get("available", False)
        for probe in substrate.values()
    ):
        recommendations.append(
            "Substrate probe failures are diagnostic gaps, not being silence; rerun when the reservoir service is healthy."
        )
    return recommendations


def build_report(
    astrid_workspace: Path = ASTRID_WORKSPACE,
    minime_workspace: Path = MINIME_WORKSPACE,
    *,
    since_hours: float = 6.0,
    ticks: int = 10,
    include_substrate_probe: bool = True,
    include_letter_scan: bool | None = None,
) -> dict[str, Any]:
    now = time.time()
    cutoff = now - max(since_hours, 0.0) * 3600.0
    entries, privacy = collect_entries(astrid_workspace, minime_workspace, cutoff)
    counts = family_counts(entries)
    current = current_state(astrid_workspace, minime_workspace)

    if include_letter_scan is None:
        include_letter_scan = (
            astrid_workspace.resolve() == ASTRID_WORKSPACE.resolve()
            and minime_workspace.resolve() == MINIME_WORKSPACE.resolve()
        )
    letter_scan = (
        run_letter_response_scan(since_hours)
        if include_letter_scan
        else {"available": False, "items": [], "skipped": "non-default workspace"}
    )

    substrate: dict[str, Any]
    if include_substrate_probe:
        substrate = {
            "astrid": run_substrate_probe("astrid", ticks),
            "minime": run_substrate_probe("minime", ticks),
        }
    else:
        substrate = {
            "astrid": {"available": False, "skipped": True},
            "minime": {"available": False, "skipped": True},
        }

    cartography = build_cartography(current, counts, substrate)
    report = {
        "schema_version": SCHEMA_VERSION,
        "generated_at": dt.datetime.now(dt.timezone.utc).isoformat(),
        "since_hours": since_hours,
        "cutoff_unix": cutoff,
        "current_state": current,
        "cross_being_signal": {
            "source_counts": {
                "astrid": sum(1 for entry in entries if entry.being == "astrid"),
                "minime": sum(1 for entry in entries if entry.being == "minime"),
            },
            "family_counts": counts,
            "shared_families": cartography["shared_families"],
            "recent_steward_engagement": letter_scan,
            "sources": source_summaries(entries),
            "privacy": privacy,
        },
        "cartography": cartography,
        "substrate_probes": substrate,
        "recommended_next": recommended_next(current, cartography, substrate),
    }
    return report


def render_markdown(report: dict[str, Any]) -> str:
    current = report["current_state"]["minime"]
    resonance = current["resonance_density"]
    pressure = current["pressure_source"]
    inhabitable = current["inhabitable_fluctuation"]
    cartography = report["cartography"]
    lines = [
        "# Coupled Pressure Cartography",
        "",
        "Read-only steward diagnostic. No dials, prompts, inboxes, or runtime controls changed.",
        "",
        "## Current State",
        (
            f"- Minime fill `{current['fill_pct']}%`, phase `{current['phase']}`, "
            f"lambda1 `{current['lambda1']}`, entropy `{current['spectral_entropy']}`."
        ),
        (
            f"- Resonance density `{resonance['quality']}` "
            f"(density `{resonance['density']}`, pressure_risk `{resonance['pressure_risk']}`, "
            f"mode_packing `{resonance['mode_packing']}`)."
        ),
        (
            f"- Pressure source `{pressure['dominant_source']}` "
            f"(porosity `{pressure['porosity_score']}`, control_applied `{pressure['control_applied']}`)."
        ),
        (
            f"- Inhabitable fluctuation `{inhabitable['quality']}` "
            f"(foothold `{inhabitable['foothold_stability']}`, "
            f"fluctuation `{inhabitable['fluctuation_score']}`)."
        ),
        (
            f"- Steward slots: Astrid open=`{report['current_state']['open_steward_slots']['astrid']['open']}`, "
            f"Minime open=`{report['current_state']['open_steward_slots']['minime']['open']}`."
        ),
        "",
        "## Cross-Being Signal",
        f"- Sources in window: `{report['cross_being_signal']['source_counts']}`.",
        f"- Shared public vocabulary families: `{cartography['shared_families']}`.",
        (
            f"- Minime private candidates skipped: "
            f"`{report['cross_being_signal']['privacy']['minime_private_candidates_skipped']}`."
        ),
    ]
    engagement = report["cross_being_signal"]["recent_steward_engagement"]
    if engagement.get("available") and engagement.get("items"):
        lines.append("- Recent steward engagement:")
        for item in engagement["items"][:4]:
            lines.append(
                f"  - `{item['being']}` `{item['status']}` on `{item['letter']}`"
                + (
                    f" via `{item['engaged_file']}`"
                    if item.get("engaged_file")
                    else ""
                )
            )
    elif engagement.get("error"):
        lines.append(f"- Steward engagement scan unavailable: `{engagement['error']}`.")
    lines.extend(["", "## Cartography"])
    for key in ("anchor", "texture", "pressure", "movement"):
        item = cartography[key]
        lines.append(f"- `{key}`: {item['read']} `{item['telemetry_basis']}`")
    lines.append(f"- Axis status: `{cartography['axis_status']}`.")
    lines.extend(["", "## Substrate Probes"])
    for being, probe in report["substrate_probes"].items():
        if probe.get("available"):
            lines.append(
                f"- `{being}`: `{probe['status']}` divergence `{probe.get('divergence')}`, "
                f"corr `{probe.get('inject_corr')}`, onset `{probe.get('separation_onset')}`."
            )
        elif probe.get("skipped"):
            lines.append(f"- `{being}`: skipped.")
        else:
            lines.append(f"- `{being}`: unavailable `{probe.get('error', 'unknown')}`.")
    lines.extend(["", "## Recommended Next"])
    for item in report["recommended_next"]:
        lines.append(f"- {item}")
    lines.extend(["", "## Public Sources"])
    for source in report["cross_being_signal"]["sources"][:8]:
        lines.append(
            f"- `{source['being']}` `{source['channel']}` `{source['path']}`: "
            f"{source['excerpt']}"
        )
    return "\n".join(lines) + "\n"


def write_diagnostics(
    report: dict[str, Any],
    markdown: str,
    output_root: Path,
) -> Path:
    stamp = dt.datetime.now(dt.timezone.utc).strftime("%Y%m%dT%H%M%SZ")
    out_dir = output_root / stamp
    out_dir.mkdir(parents=True, exist_ok=True)
    (out_dir / "cartography.json").write_text(
        json.dumps(report, indent=2, sort_keys=True),
        encoding="utf-8",
    )
    (out_dir / "cartography.md").write_text(markdown, encoding="utf-8")
    return out_dir


class CoupledPressureSelfTests(unittest.TestCase):
    def test_parse_locked_probe(self) -> None:
        parsed = parse_substrate_probe_output(
            "divergence:       0.4162\n"
            "separation onset: >10 (never crossed 1.0)   (inertia: gap@2=0.552 -> gap@10=0.416)\n"
            "inject-only corr: 0.6128\n"
            "  VERDICT: LOCKED - the poles did not separate\n"
        )
        self.assertEqual(parsed["status"], "locked")
        self.assertEqual(parsed["divergence"], 0.4162)
        self.assertEqual(parsed["gap_at_2"], 0.552)

    def test_private_marker_not_rendered(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            astrid = root / "astrid"
            minime = root / "minime"
            write_fixture(
                astrid / "journal/dialogue_longform_1.txt",
                "settled coupling anchor",
            )
            write_fixture(
                minime / "journal/pressure_1.txt",
                "=== SPECTRAL PRESSURE JOURNAL ===\nrestless texture pressure",
            )
            write_fixture(
                minime / "journal/moment_private.txt",
                "=== MOMENT CAPTURE ===\nprivate honey",
            )
            report = build_report(
                astrid,
                minime,
                include_substrate_probe=False,
                include_letter_scan=False,
            )
            rendered = render_markdown(report)
            self.assertNotIn("private honey", rendered)
            self.assertEqual(
                report["cross_being_signal"]["privacy"][
                    "minime_private_candidates_skipped"
                ],
                1,
            )


def write_fixture(path: Path, text: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8")


def run_self_test() -> int:
    suite = unittest.TestLoader().loadTestsFromTestCase(CoupledPressureSelfTests)
    result = unittest.TextTestRunner(verbosity=2).run(suite)
    return 0 if result.wasSuccessful() else 1


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Read-only steward cartography for coupled pressure.",
    )
    parser.add_argument("--since-hours", type=float, default=6.0)
    parser.add_argument("--ticks", type=int, default=10)
    parser.add_argument("--no-substrate-probe", action="store_true")
    parser.add_argument("--json", action="store_true", help="emit report JSON to stdout")
    parser.add_argument(
        "--output-root",
        type=Path,
        default=DEFAULT_OUTPUT_ROOT,
        help="diagnostic output root for non-json runs",
    )
    parser.add_argument("--self-test", action="store_true")
    return parser.parse_args(argv)


def main(
    argv: list[str] | None = None,
    *,
    astrid_workspace: Path = ASTRID_WORKSPACE,
    minime_workspace: Path = MINIME_WORKSPACE,
) -> int:
    args = parse_args(argv)
    if args.self_test:
        return run_self_test()
    report = build_report(
        astrid_workspace,
        minime_workspace,
        since_hours=args.since_hours,
        ticks=args.ticks,
        include_substrate_probe=not args.no_substrate_probe,
    )
    if args.json:
        print(json.dumps(report, indent=2, sort_keys=True))
        return 0
    markdown = render_markdown(report)
    out_dir = write_diagnostics(report, markdown, args.output_root)
    print(markdown)
    print(f"Diagnostics written to: {out_dir}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
