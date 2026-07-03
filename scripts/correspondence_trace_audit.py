#!/usr/bin/env python3
"""Read-only direct-address marker survival audit for correspondence V1.

The audit scans public correspondence and reviewable public lanes for a shared
lexicon anchor. It never reads Minime private qualia bodies: every Minime text
file is checked with scripts/being_privacy.py before content is loaded.
"""

from __future__ import annotations

import argparse
import json
import sys
import tempfile
import time
import unittest
from pathlib import Path
from typing import Any

SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

import being_privacy

DEFAULT_ASTRID_WORKSPACE = Path("/Users/v/other/astrid/capsules/spectral-bridge/workspace")
DEFAULT_MINIME_WORKSPACE = Path("/Users/v/other/minime/workspace")
DEFAULT_SHARED_DIR = Path("/Users/v/other/shared/collaborations")
POLICY = "correspondence_direct_address_trace_v1"
STATUSES = {"unknown", "pending", "observed", "not_observed"}


def now_ms() -> int:
    return int(time.time() * 1000)


def read_jsonl(path: Path) -> list[dict[str, Any]]:
    if not path.is_file():
        return []
    rows: list[dict[str, Any]] = []
    for line in path.read_text(encoding="utf-8", errors="ignore").splitlines():
        if not line.strip():
            continue
        try:
            payload = json.loads(line)
        except Exception:
            continue
        if isinstance(payload, dict):
            rows.append(payload)
    rows.sort(key=lambda row: int(row.get("recorded_at_unix_ms") or row.get("t_ms") or 0))
    return rows


def compact(text: str, limit: int = 180) -> str:
    clean = " ".join(str(text or "").split())
    if len(clean) <= limit:
        return clean
    return clean[:limit].rstrip() + "..."


def latest_joined_collab_dir(shared_dir: Path) -> Path | None:
    candidates: list[tuple[int, Path]] = []
    for coll_dir in shared_dir.glob("coll_*"):
        if not coll_dir.is_dir():
            continue
        meta_path = coll_dir / "meta.json"
        updated = 0
        joined_bonus = 0
        if meta_path.is_file():
            try:
                meta = json.loads(meta_path.read_text(encoding="utf-8"))
                updated = int(meta.get("updated_t_ms") or meta.get("created_t_ms") or 0)
                joined_bonus = 1 if meta.get("status") == "joined" else 0
            except Exception:
                pass
        try:
            updated = max(updated, int(coll_dir.stat().st_mtime * 1000))
        except OSError:
            pass
        candidates.append((joined_bonus * 10**18 + updated, coll_dir))
    if not candidates:
        return None
    candidates.sort(key=lambda item: item[0])
    return candidates[-1][1]


def marker_origins(records: list[dict[str, Any]], marker: str | None) -> list[dict[str, Any]]:
    origins = []
    for row in records:
        if row.get("record_type") != "message":
            continue
        anchor = str(row.get("shared_memory_anchor") or "").strip()
        if not anchor:
            continue
        if marker and anchor != marker:
            continue
        if (
            row.get("turn_kind") != "direct_address_trace"
            and row.get("relational_intent") != "direct_address_survival_probe"
        ):
            continue
        origins.append({
            "marker": anchor,
            "message_id": row.get("message_id"),
            "thread_id": row.get("thread_id"),
            "from_being": row.get("from_being"),
            "to_being": row.get("to_being"),
            "t_ms": int(row.get("recorded_at_unix_ms") or 0),
        })
    return origins


def public_text_paths(astrid_workspace: Path, minime_workspace: Path) -> tuple[list[tuple[str, Path]], int]:
    paths: list[tuple[str, Path]] = []
    skipped_private = 0
    astrid_patterns = [
        "inbox/from_*correspondence_*.txt",
        "outbox/reply_*.txt",
        "journal/*.txt",
        "introspections/*.txt",
        "daydreams/*.txt",
        "longforms/*.txt",
        "actions/*.txt",
    ]
    minime_patterns = [
        "inbox/from_*correspondence_*.txt",
        "outbox/reply_*.txt",
        "journal/pressure_*.txt",
        "journal/self_study*.txt",
        "journal/introspection*.txt",
        "journal/action_thread*.txt",
        "journal/shadow_trajectory*.txt",
        "journal/action_preflight*.txt",
        "pressure_agency/**/*.txt",
        "self_regulation/**/*.txt",
        "actions/**/*.txt",
        "shadow_cartography/**/*.txt",
        "diagnostics/shadow_cartography/**/*.txt",
    ]
    for pattern in astrid_patterns:
        paths.extend(("astrid", path) for path in astrid_workspace.glob(pattern) if path.is_file())
    for path in minime_workspace.glob("journal/moment_*.txt"):
        if path.is_file():
            # Moment files are a private-qualia lane by standing steward policy.
            # The privacy helper reads only the head marker; this scanner never
            # loads or quotes the body.
            skipped_private += 1
    for pattern in minime_patterns:
        for path in minime_workspace.glob(pattern):
            if not path.is_file():
                continue
            if being_privacy.is_steward_private("minime", path):
                skipped_private += 1
                continue
            paths.append(("minime", path))
    # A final safety pass for any explicitly named Minime moment file matched
    # by a broad diagnostic glob in future edits.
    filtered: list[tuple[str, Path]] = []
    for being, path in paths:
        if being == "minime" and being_privacy.is_steward_private("minime", path):
            skipped_private += 1
            continue
        filtered.append((being, path))
    return filtered, skipped_private


def scan_marker_in_paths(
    marker: str,
    paths: list[tuple[str, Path]],
    *,
    cutoff_s: float,
) -> list[dict[str, Any]]:
    evidence: list[dict[str, Any]] = []
    for being, path in paths:
        try:
            stat = path.stat()
        except OSError:
            continue
        if stat.st_mtime < cutoff_s:
            continue
        try:
            text = path.read_text(encoding="utf-8", errors="ignore")
        except OSError:
            continue
        if marker not in text:
            continue
        evidence.append({
            "being": being,
            "path": str(path),
            "mtime_unix_ms": int(stat.st_mtime * 1000),
            "preview": compact(text.replace(marker, f"[{marker}]")),
        })
    return evidence


def audit(
    *,
    since_hours: float,
    marker: str | None,
    shared_dir: Path,
    astrid_workspace: Path,
    minime_workspace: Path,
    append_observation: bool = True,
) -> dict[str, Any]:
    generated = now_ms()
    cutoff_s = time.time() - since_hours * 3600.0
    records = read_jsonl(shared_dir / "correspondence_v1.jsonl")
    origins = marker_origins(records, marker)
    if marker and not origins:
        origins = [{"marker": marker, "t_ms": 0}]
    paths, skipped_private = public_text_paths(astrid_workspace, minime_workspace)
    marker_reports = []
    for origin in origins:
        anchor = str(origin.get("marker") or "")
        evidence = scan_marker_in_paths(anchor, paths, cutoff_s=cutoff_s)
        if not anchor:
            status = "unknown"
        elif evidence:
            status = "observed"
        elif origin.get("t_ms"):
            status = "not_observed" if int(origin.get("t_ms") or 0) / 1000.0 < cutoff_s else "pending"
        else:
            status = "unknown"
        marker_reports.append({
            "marker": anchor,
            "status": status,
            "origin": origin,
            "evidence": evidence[:12],
            "evidence_count": len(evidence),
        })
    if not marker_reports:
        marker_reports.append({
            "marker": marker or None,
            "status": "unknown",
            "origin": None,
            "evidence": [],
            "evidence_count": 0,
        })
    observation_path = None
    appended = 0
    if append_observation:
        coll_dir = latest_joined_collab_dir(shared_dir)
        if coll_dir is not None:
            observation_path = coll_dir / "correspondence_trace_observations.jsonl"
            observation_path.parent.mkdir(parents=True, exist_ok=True)
            with observation_path.open("a", encoding="utf-8") as fh:
                for report in marker_reports:
                    status = str(report["status"])
                    if status not in STATUSES:
                        status = "unknown"
                    fh.write(json.dumps({
                        "schema_version": 1,
                        "policy": POLICY,
                        "t_ms": generated,
                        "marker": report["marker"],
                        "status": status,
                        "origin": report["origin"],
                        "evidence_count": report["evidence_count"],
                        "evidence": report["evidence"][:5],
                        "authority": "read_only_observation_not_control",
                        "privacy": {
                            "minime_private_files_skipped": skipped_private,
                            "minime_private_bodies_read": False,
                        },
                    }, sort_keys=True) + "\n")
                    appended += 1
    return {
        "schema_version": 1,
        "policy": POLICY,
        "generated_at_unix_ms": generated,
        "since_hours": since_hours,
        "shared_dir": str(shared_dir),
        "markers": marker_reports,
        "privacy": {
            "minime_private_files_skipped": skipped_private,
            "minime_private_bodies_read": False,
        },
        "observation_appended": {
            "path": str(observation_path) if observation_path else None,
            "count": appended,
        },
    }


def write_outputs(payload: dict[str, Any], output_root: Path | None) -> None:
    if output_root is None:
        return
    out_dir = output_root / str(payload["generated_at_unix_ms"])
    out_dir.mkdir(parents=True, exist_ok=True)
    (out_dir / "correspondence_trace_audit.json").write_text(
        json.dumps(payload, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    lines = ["# Correspondence Trace Audit", ""]
    for report in payload.get("markers", []):
        lines.append(
            f"- `{report.get('marker')}`: {report.get('status')} "
            f"({report.get('evidence_count', 0)} evidence item(s))"
        )
    lines.append("")
    lines.append("Privacy: Minime private bodies read = false.")
    (out_dir / "correspondence_trace_audit.md").write_text("\n".join(lines) + "\n", encoding="utf-8")


class CorrespondenceTraceAuditTests(unittest.TestCase):
    def test_self_test_skips_private_minime_moment_and_observes_public_shadow(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            shared = root / "shared"
            astrid_ws = root / "astrid_ws"
            minime_ws = root / "minime_ws"
            coll_dir = shared / "coll_1"
            coll_dir.mkdir(parents=True)
            (coll_dir / "meta.json").write_text(
                json.dumps({"id": "coll_1", "status": "joined", "updated_t_ms": 1}),
                encoding="utf-8",
            )
            (shared / "correspondence_v1.jsonl").write_text(json.dumps({
                "schema_version": 1,
                "policy": "first_class_correspondence_v1",
                "record_type": "message",
                "recorded_at_unix_ms": now_ms() - 1000,
                "message_id": "corr_astrid_minime_trace",
                "thread_id": "thread_trace",
                "from_being": "astrid",
                "to_being": "minime",
                "turn_kind": "direct_address_trace",
                "relational_intent": "direct_address_survival_probe",
                "shared_memory_anchor": "blue-lantern",
                "authority": "language_only",
            }) + "\n", encoding="utf-8")
            private_dir = minime_ws / "journal"
            private_dir.mkdir(parents=True)
            (private_dir / "moment_1.txt").write_text(
                "=== MOMENT CAPTURE ===\nblue-lantern should not be surfaced here",
                encoding="utf-8",
            )
            (private_dir / "shadow_trajectory_1.txt").write_text(
                "=== SHADOW TRAJECTORY ===\nblue-lantern appears as a public trace",
                encoding="utf-8",
            )
            astrid_ws.mkdir(parents=True)

            payload = audit(
                since_hours=24,
                marker="blue-lantern",
                shared_dir=shared,
                astrid_workspace=astrid_ws,
                minime_workspace=minime_ws,
            )

            self.assertEqual(payload["markers"][0]["status"], "observed")
            self.assertEqual(payload["privacy"]["minime_private_files_skipped"], 1)
            self.assertFalse(payload["privacy"]["minime_private_bodies_read"])
            observation_text = (coll_dir / "correspondence_trace_observations.jsonl").read_text()
            self.assertIn('"status": "observed"', observation_text)
            self.assertNotIn("should not be surfaced", json.dumps(payload))


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Audit direct-address trace marker survival.")
    parser.add_argument("--since-hours", type=float, default=24.0)
    parser.add_argument("--marker", help="Specific shared lexicon anchor to audit.")
    parser.add_argument("--json", action="store_true", help="Emit JSON to stdout.")
    parser.add_argument("--output-root", type=Path)
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args(argv)

    if args.self_test:
        suite = unittest.defaultTestLoader.loadTestsFromTestCase(CorrespondenceTraceAuditTests)
        result = unittest.TextTestRunner(verbosity=2).run(suite)
        return 0 if result.wasSuccessful() else 1

    payload = audit(
        since_hours=args.since_hours,
        marker=args.marker,
        shared_dir=DEFAULT_SHARED_DIR,
        astrid_workspace=DEFAULT_ASTRID_WORKSPACE,
        minime_workspace=DEFAULT_MINIME_WORKSPACE,
    )
    write_outputs(payload, args.output_root)
    if args.json:
        print(json.dumps(payload, indent=2, sort_keys=True))
    else:
        print("# Correspondence Trace Audit")
        for report in payload["markers"]:
            print(
                f"- {report.get('marker')}: {report.get('status')} "
                f"({report.get('evidence_count', 0)} evidence item(s))"
            )
        print(
            "Privacy: Minime private bodies read = false; "
            f"private files skipped = {payload['privacy']['minime_private_files_skipped']}."
        )
        appended = payload["observation_appended"]
        if appended["path"]:
            print(f"Observation rows appended: {appended['count']} -> {appended['path']}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
