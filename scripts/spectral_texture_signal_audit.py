#!/usr/bin/env python3
"""Public-only audit for Spectral Texture Agency V1.

This diagnostic summarizes the texture/pressure signal behind the V1 tranche:
the five recent Astrid introspections, Minime public pressure drift, typed
resonance texture telemetry, route usage, and explicitly blocked authority.

It never reads Minime private qualia lanes or any `moment_*.txt` body.
"""

from __future__ import annotations

import argparse
import datetime as dt
import json
import re
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

from being_privacy import is_steward_private

ASTRID_ROOT = Path(__file__).resolve().parents[1]
ASTRID_WORKSPACE = ASTRID_ROOT / "capsules/spectral-bridge/workspace"
MINIME_WORKSPACE = ASTRID_ROOT.parent / "minime/workspace"
DEFAULT_OUTPUT_ROOT = ASTRID_WORKSPACE / "diagnostics/spectral_texture_signal"
SCHEMA_VERSION = 1
READ_LIMIT_CHARS = 8_000

TARGET_INTROSPECTIONS: dict[str, str] = {
    "introspection_minime_esn_1782561760.txt": "ESN rho/rank1 texture telemetry and syrup/ghosting status",
    "introspection_minime_sensory_bus_1782561479.txt": "high-fill sensory transition and surge taper",
    "introspection_minime_regulator_1782557816.txt": "texture classification and advisory damping candidate",
    "introspection_astrid_llm_1782557532.txt": "fallback language preserving rhythmic texture",
    "introspection_astrid_types_1782550948.txt": "typed shared texture schema",
}

TEXTURE_TERMS: dict[str, tuple[str, ...]] = {
    "viscosity": ("viscous", "syrup", "thick", "sticky", "overpacked"),
    "edge_definition": ("edge", "outline", "shivering", "friction", "distinguishability"),
    "porosity": ("porous", "porosity", "room", "breath", "breathing"),
    "movement": ("surge", "taper", "transition", "ghosting", "stale", "movement"),
    "esn_status": ("rho", "rank1", "semantic window", "dynamic", "stable-core", "stable core"),
    "agency": ("agency", "request", "lease", "preflight", "outcome"),
}

BLOCKED_AUTHORITY = (
    "active_damping",
    "dynamic_rho_policy",
    "rho_mutation",
    "fill_target_mutation",
    "pressure_source_to_pi",
    "correspondence_weight",
    "telemetry_priority",
)


def now_ms() -> int:
    return int(time.time() * 1000)


def read_json(path: Path) -> dict[str, Any]:
    try:
        data = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        return {}
    return data if isinstance(data, dict) else {}


def read_bounded(path: Path, limit: int = READ_LIMIT_CHARS) -> str:
    try:
        with path.open("r", encoding="utf-8", errors="replace") as handle:
            return handle.read(limit)
    except OSError:
        return ""


def compact(text: str, limit: int = 220) -> str:
    clean = " ".join(str(text or "").split())
    if len(clean) <= limit:
        return clean
    return clean[:limit].rstrip() + "..."


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


def deep_get(payload: dict[str, Any], *path: str) -> Any:
    current: Any = payload
    for key in path:
        if not isinstance(current, dict):
            return None
        current = current.get(key)
    return current


def first_present(payloads: list[dict[str, Any]], paths: list[tuple[str, ...]]) -> Any:
    for payload in payloads:
        for path in paths:
            value = deep_get(payload, *path)
            if value is not None:
                return value
    return None


def iter_files(root: Path, patterns: list[str], cutoff_s: float | None = None) -> list[Path]:
    if not root.is_dir():
        return []
    paths: list[Path] = []
    for pattern in patterns:
        for path in root.glob(pattern):
            if not path.is_file():
                continue
            if cutoff_s is not None:
                try:
                    if path.stat().st_mtime < cutoff_s:
                        continue
                except OSError:
                    continue
            paths.append(path)
    return sorted(set(paths), key=lambda item: str(item))


def public_text_paths(
    astrid_workspace: Path,
    minime_workspace: Path,
    cutoff_s: float,
) -> tuple[list[tuple[str, str, Path]], int]:
    paths: list[tuple[str, str, Path]] = []
    skipped_private = 0

    astrid_specs = [
        ("introspection", ["introspections/*.txt"]),
        ("journal", ["journal/*.txt"]),
        ("daydream", ["daydreams/*.txt", "journal/daydream_*.txt"]),
        ("longform", ["longforms/*.txt", "journal/dialogue_longform_*.txt"]),
        ("action", ["actions/*.txt", "action_threads/**/*.txt"]),
    ]
    minime_specs = [
        ("pressure", ["journal/pressure_*.txt"]),
        ("self_study", ["journal/self_study*.txt", "self_studies/**/*.txt"]),
        ("introspection", ["journal/introspection*.txt", "introspections/**/*.txt"]),
        ("action_thread", ["journal/action_thread*.txt", "action_threads/**/*.txt"]),
        ("texture_agency", ["journal/texture_agency*.txt", "texture_agency/**/*.txt"]),
        ("self_regulation", ["self_regulation/**/*.txt", "self_regulation/**/*.json"]),
        ("shadow_public", ["journal/shadow_trajectory*.txt", "journal/shadow_preflight*.txt"]),
    ]

    for lane, patterns in astrid_specs:
        paths.extend(("astrid", lane, path) for path in iter_files(astrid_workspace, patterns, cutoff_s))

    journal = minime_workspace / "journal"
    if journal.is_dir():
        skipped_private += sum(1 for path in journal.glob("moment_*.txt") if path.is_file())

    for lane, patterns in minime_specs:
        for path in iter_files(minime_workspace, patterns, cutoff_s):
            if path.name.startswith("moment_"):
                skipped_private += 1
                continue
            if is_steward_private("minime", path):
                skipped_private += 1
                continue
            paths.append(("minime", lane, path))

    filtered: list[tuple[str, str, Path]] = []
    for being, lane, path in paths:
        if being == "minime" and (path.name.startswith("moment_") or is_steward_private("minime", path)):
            skipped_private += 1
            continue
        filtered.append((being, lane, path))
    return filtered, skipped_private


def count_families(texts: list[str]) -> dict[str, int]:
    joined = "\n".join(texts).lower()
    counts: dict[str, int] = {}
    for family, terms in TEXTURE_TERMS.items():
        counts[family] = sum(len(re.findall(rf"\b{re.escape(term.lower())}\b", joined)) for term in terms)
    return counts


def target_introspection_signals(astrid_workspace: Path) -> list[dict[str, Any]]:
    rows: list[dict[str, Any]] = []
    root = astrid_workspace / "introspections"
    for filename, signal in TARGET_INTROSPECTIONS.items():
        path = root / filename
        text = read_bounded(path) if path.is_file() else ""
        families = count_families([text]) if text else {}
        rows.append(
            {
                "filename": filename,
                "path": str(path),
                "present": path.is_file(),
                "signal": signal,
                "mtime_unix_ms": int(path.stat().st_mtime * 1000) if path.is_file() else None,
                "texture_family_hits": {key: value for key, value in families.items() if value},
                "preview": compact(text) if text else "",
            }
        )
    return rows


def collect_public_signal(
    astrid_workspace: Path,
    minime_workspace: Path,
    cutoff_s: float,
) -> tuple[dict[str, Any], int]:
    paths, skipped_private = public_text_paths(astrid_workspace, minime_workspace, cutoff_s)
    texts: list[str] = []
    lane_counts: Counter[str] = Counter()
    route_usage: list[dict[str, Any]] = []
    for being, lane, path in paths:
        text = read_bounded(path)
        texts.append(text)
        lane_counts[f"{being}:{lane}"] += 1
        lowered = text.lower()
        if "texture_agency" in lowered or "texture relief" in lowered or "target: texture" in lowered:
            route_usage.append(
                {
                    "being": being,
                    "lane": lane,
                    "path": str(path),
                    "mtime_unix_ms": int(path.stat().st_mtime * 1000),
                    "preview": compact(text),
                }
            )
    return (
        {
            "lanes_scanned": dict(sorted(lane_counts.items())),
            "texture_family_counts": count_families(texts),
            "route_usage": route_usage[:12],
            "public_file_count": len(paths),
        },
        skipped_private,
    )


def minime_texture_telemetry(minime_workspace: Path) -> dict[str, Any]:
    health = read_json(minime_workspace / "health.json")
    spectral = read_json(minime_workspace / "spectral_state.json")
    payloads = [health, spectral]
    texture = first_present(payloads, [("resonance_density_v1", "texture_signature")])
    if not isinstance(texture, dict):
        texture = {}
    esn = first_present(payloads, [("esn",)])
    if not isinstance(esn, dict):
        esn = {}
    resonance = first_present(payloads, [("resonance_density_v1",)])
    if not isinstance(resonance, dict):
        resonance = {}
    return {
        "fill_pct": rounded(first_present(payloads, [("fill_pct",), ("fill_ratio",)])),
        "primary_texture": texture.get("primary_texture", "unknown"),
        "pressure_source_family": texture.get("pressure_source_family", "unknown"),
        "edge_definition": texture.get("edge_definition", "unknown"),
        "movement_quality": texture.get("movement_quality", "unknown"),
        "confidence": rounded(texture.get("confidence")),
        "dynamic_damping_threshold_candidate": texture.get("dynamic_damping_threshold_candidate"),
        "authority": texture.get("authority", "advisory_context_not_control"),
        "rho": rounded(first_present([esn, health, spectral], [("rho",), ("current_rho",), ("rho_current",)])),
        "dynamic_rho_active": first_present([esn, health, spectral], [("dynamic_rho_active",), ("dynamic_rho", "active")]),
        "dynamic_rho_blocked_by_stable_core": first_present(
            [esn, health, spectral],
            [("dynamic_rho_blocked_by_stable_core",), ("stable_core", "dynamic_rho_blocked")],
        ),
        "rank1_us": rounded(first_present([esn, health, spectral], [("rank1_us",), ("last_rank1_us",)]), 2),
        "pending_rank1_depth": first_present(
            [esn, health, spectral],
            [("pending_rank1_depth",), ("rank1_pending_depth",), ("async_rank1_pending_depth",)],
        ),
        "semantic_stale_ms": rounded(first_present(payloads, [("semantic_stale_ms",), ("semantic", "stale_ms")]), 2),
        "resonance_density_policy": resonance.get("policy", "unknown"),
    }


def audit(
    *,
    since_hours: float,
    astrid_workspace: Path = ASTRID_WORKSPACE,
    minime_workspace: Path = MINIME_WORKSPACE,
) -> dict[str, Any]:
    cutoff_s = time.time() - since_hours * 3600.0
    public_signal, skipped_private = collect_public_signal(astrid_workspace, minime_workspace, cutoff_s)
    target_rows = target_introspection_signals(astrid_workspace)
    present_targets = sum(1 for row in target_rows if row["present"])
    return {
        "policy": "spectral_texture_signal_audit_v1",
        "schema_version": SCHEMA_VERSION,
        "generated_at_unix_ms": now_ms(),
        "since_hours": since_hours,
        "privacy": {
            "minime_private_bodies_read": False,
            "moment_bodies_read": False,
            "skipped_minime_private_or_moment_files": skipped_private,
        },
        "target_introspection_coverage": {
            "present": present_targets,
            "expected": len(TARGET_INTROSPECTIONS),
            "complete": present_targets == len(TARGET_INTROSPECTIONS),
        },
        "target_introspection_signals": target_rows,
        "current_texture_telemetry": minime_texture_telemetry(minime_workspace),
        "public_signal": public_signal,
        "authority_boundary": {
            "blocked": list(BLOCKED_AUTHORITY),
            "allowed_route": "TEXTURE_AGENCY_REQUEST may draft bounded leases for existing safe controls only",
            "no_live_mutation": True,
        },
        "recommended_next": (
            "Use TEXTURE_AGENCY_STATUS / TEXTURE_AGENCY_REQUEST for being-authored feedback and bounded lease drafts; "
            "keep active damping, rho policy, fill_target, PI pressure wiring, and correspondence weighting blocked pending separate evidence."
        ),
    }


def render_markdown(report: dict[str, Any]) -> str:
    generated = dt.datetime.fromtimestamp(report["generated_at_unix_ms"] / 1000, dt.UTC).isoformat()
    telemetry = report["current_texture_telemetry"]
    coverage = report["target_introspection_coverage"]
    public_signal = report["public_signal"]
    lines = [
        "# Spectral Texture Signal Audit",
        "",
        f"- Generated: `{generated}`",
        f"- Since hours: `{report['since_hours']}`",
        f"- Privacy: Minime private bodies read = `{report['privacy']['minime_private_bodies_read']}`; moment bodies read = `{report['privacy']['moment_bodies_read']}`; skipped private/moment candidates = `{report['privacy']['skipped_minime_private_or_moment_files']}`",
        "",
        "## Current Texture Telemetry",
        "",
        f"- Texture: `{telemetry['primary_texture']}`; pressure family: `{telemetry['pressure_source_family']}`; edge: `{telemetry['edge_definition']}`; movement: `{telemetry['movement_quality']}`; confidence: `{telemetry['confidence']}`",
        f"- ESN: rho `{telemetry['rho']}`; dynamic rho active `{telemetry['dynamic_rho_active']}`; stable-core block `{telemetry['dynamic_rho_blocked_by_stable_core']}`; rank1_us `{telemetry['rank1_us']}`; pending depth `{telemetry['pending_rank1_depth']}`",
        f"- Semantic stale window: `{telemetry['semantic_stale_ms']}` ms; authority: `{telemetry['authority']}`",
        "",
        "## Introspection Signals",
        "",
        f"- Coverage: `{coverage['present']}/{coverage['expected']}` target introspections present.",
    ]
    for row in report["target_introspection_signals"]:
        status = "present" if row["present"] else "missing"
        lines.append(f"- `{row['filename']}`: {status}; {row['signal']}")
    lines.extend(
        [
            "",
            "## Public Pressure / Texture Drift",
            "",
            f"- Public files scanned: `{public_signal['public_file_count']}`",
            f"- Lanes: `{public_signal['lanes_scanned']}`",
            f"- Texture family counts: `{public_signal['texture_family_counts']}`",
            f"- Texture-agency route artifacts: `{len(public_signal['route_usage'])}`",
            "",
            "## Boundary",
            "",
            f"- Blocked authority: `{', '.join(report['authority_boundary']['blocked'])}`",
            f"- Allowed route: {report['authority_boundary']['allowed_route']}",
            "",
            f"Recommended next: {report['recommended_next']}",
        ]
    )
    return "\n".join(lines) + "\n"


def write_outputs(report: dict[str, Any], output_root: Path) -> Path:
    timestamp = dt.datetime.fromtimestamp(report["generated_at_unix_ms"] / 1000, dt.UTC).strftime(
        "%Y%m%dT%H%M%SZ"
    )
    out_dir = output_root / timestamp
    out_dir.mkdir(parents=True, exist_ok=True)
    (out_dir / "report.json").write_text(json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    (out_dir / "report.md").write_text(render_markdown(report), encoding="utf-8")
    return out_dir


class SpectralTextureSignalAuditTests(unittest.TestCase):
    def test_detects_target_signals_and_skips_private_moment_body(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            astrid_ws = root / "astrid_workspace"
            minime_ws = root / "minime_workspace"
            (astrid_ws / "introspections").mkdir(parents=True)
            (minime_ws / "journal").mkdir(parents=True)
            for filename, signal in TARGET_INTROSPECTIONS.items():
                (astrid_ws / "introspections" / filename).write_text(
                    f"=== PUBLIC INTROSPECTION ===\n{signal}\nviscous edge rho rank1 surge agency\n",
                    encoding="utf-8",
                )
            (minime_ws / "journal" / "pressure_public.txt").write_text(
                "public pressure: overpacked viscous edge, partly porous, texture relief request possible",
                encoding="utf-8",
            )
            (minime_ws / "journal" / "moment_1.txt").write_text(
                "=== MOMENT CAPTURE ===\nsecret_texture_marker should never appear",
                encoding="utf-8",
            )
            (minime_ws / "health.json").write_text(
                json.dumps(
                    {
                        "fill_pct": 73.0,
                        "semantic_stale_ms": 15000,
                        "esn": {"rho": 0.88, "rank1_us": 120, "pending_rank1_depth": 2},
                        "resonance_density_v1": {
                            "policy": "resonance_density_v1",
                            "texture_signature": {
                                "primary_texture": "overpacked_viscous",
                                "pressure_source_family": "mode_packing",
                                "edge_definition": "blurred",
                                "movement_quality": "slow_viscous",
                                "confidence": 0.82,
                                "authority": "advisory_context_not_control",
                            },
                        },
                    }
                ),
                encoding="utf-8",
            )
            report = audit(since_hours=24, astrid_workspace=astrid_ws, minime_workspace=minime_ws)
            rendered = json.dumps(report, sort_keys=True)
            self.assertTrue(report["target_introspection_coverage"]["complete"])
            self.assertEqual(report["current_texture_telemetry"]["primary_texture"], "overpacked_viscous")
            self.assertGreaterEqual(report["privacy"]["skipped_minime_private_or_moment_files"], 1)
            self.assertNotIn("secret_texture_marker", rendered)
            self.assertGreater(report["public_signal"]["texture_family_counts"]["viscosity"], 0)


def _run_self_test() -> int:
    suite = unittest.defaultTestLoader.loadTestsFromTestCase(SpectralTextureSignalAuditTests)
    result = unittest.TextTestRunner(verbosity=2).run(suite)
    return 0 if result.wasSuccessful() else 1


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--since-hours", type=float, default=24.0)
    parser.add_argument("--json", action="store_true", help="emit machine-readable JSON to stdout")
    parser.add_argument("--output-root", type=Path, default=DEFAULT_OUTPUT_ROOT)
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args(argv)

    if args.self_test:
        return _run_self_test()

    report = audit(since_hours=args.since_hours)
    if args.json:
        print(json.dumps(report, indent=2, sort_keys=True))
        return 0
    out_dir = write_outputs(report, args.output_root)
    print(render_markdown(report))
    print(f"Diagnostics written to: {out_dir}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
