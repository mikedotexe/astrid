#!/usr/bin/env python3
"""Arm and inspect bounded Causal Signal Spine vector capture windows."""

from __future__ import annotations

import argparse
from datetime import UTC, datetime
import json
import os
from pathlib import Path
import secrets
import sys
import tempfile
import time
import unittest
from typing import Any

DEFAULT_WORKSPACE = Path("/Users/v/other/astrid/capsules/spectral-bridge/workspace")
DEFAULT_DURATION_MINUTES = 30
DEFAULT_JOURNEY_LIMIT = 32
MAX_DURATION_MINUTES = 120
MAX_JOURNEY_LIMIT = 256


def capture_root(workspace: Path) -> Path:
    return workspace / "diagnostics/signal_spine_v1"


def capture_window_path(workspace: Path) -> Path:
    return capture_root(workspace) / "capture_window.json"


def utc_now() -> str:
    return datetime.now(tz=UTC).isoformat()


def atomic_owner_only_json(path: Path, value: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True, mode=0o700)
    os.chmod(path.parent, 0o700)
    payload = json.dumps(value, indent=2, sort_keys=True, ensure_ascii=False) + "\n"
    temporary = path.with_name(f".{path.name}.{os.getpid()}.tmp")
    fd = os.open(temporary, os.O_WRONLY | os.O_CREAT | os.O_TRUNC, 0o600)
    try:
        with os.fdopen(fd, "w", encoding="utf-8") as handle:
            handle.write(payload)
            handle.flush()
            os.fsync(handle.fileno())
        os.chmod(temporary, 0o600)
        os.replace(temporary, path)
    finally:
        if temporary.exists():
            temporary.unlink()


def authority_state() -> dict[str, Any]:
    return {
        "schema": "artifact_authority_state_v1",
        "schema_version": 1,
        "state": "evidence_only",
        "live_eligible_now": False,
        "auto_approved": False,
        "grants_approval": False,
        "edits_source_now": False,
    }


def request_validation_error(request: dict[str, Any]) -> str | None:
    authority = request.get("artifact_authority_state_v1")
    if request.get("schema") != "signal_spine_capture_window_v1":
        return "schema_mismatch"
    if request.get("schema_version") != 1:
        return "schema_version_mismatch"
    if not str(request.get("capture_window_id") or "").strip():
        return "capture_window_id_missing"
    if not str(request.get("actor") or "").strip():
        return "actor_missing"
    if not str(request.get("acknowledgement") or "").strip():
        return "acknowledgement_missing"
    if request.get("full_vector_dimensions") != 48:
        return "vector_dimensions_mismatch"
    if request.get("raw_response_prose_included") is not False:
        return "raw_response_prose_marker"
    if request.get("capture_can_delay_dispatch") is not False:
        return "dispatch_delay_marker"
    if request.get("witness_only") is not True:
        return "witness_only_marker"
    if not isinstance(authority, dict):
        return "authority_state_missing"
    if (
        authority.get("schema") != "artifact_authority_state_v1"
        or authority.get("schema_version") != 1
        or authority.get("state") != "evidence_only"
    ):
        return "authority_state_invalid"
    if any(
        authority.get(marker) is not False
        for marker in (
            "live_eligible_now",
            "auto_approved",
            "grants_approval",
            "edits_source_now",
        )
    ):
        return "authority_marker_invalid"
    return None


def arm_capture(
    workspace: Path,
    *,
    duration_minutes: int,
    journey_limit: int,
    actor: str,
    acknowledgement: str,
    now_ms: int | None = None,
) -> dict[str, Any]:
    if not 1 <= duration_minutes <= MAX_DURATION_MINUTES:
        raise ValueError(
            f"duration must be between 1 and {MAX_DURATION_MINUTES} minutes"
        )
    if not 1 <= journey_limit <= MAX_JOURNEY_LIMIT:
        raise ValueError(
            f"journey limit must be between 1 and {MAX_JOURNEY_LIMIT}"
        )
    if not actor.strip():
        raise ValueError("actor must not be empty")
    if not acknowledgement.strip():
        raise ValueError("acknowledgement must not be empty")
    started_at_ms = int(time.time() * 1_000) if now_ms is None else now_ms
    expires_at_ms = started_at_ms + duration_minutes * 60 * 1_000
    capture_id = (
        f"capture_{started_at_ms}_{secrets.token_hex(6)}"
        if now_ms is None
        else f"capture_{started_at_ms}_test"
    )
    request = {
        "schema": "signal_spine_capture_window_v1",
        "schema_version": 1,
        "capture_window_id": capture_id,
        "created_at": utc_now(),
        "started_at_unix_ms": started_at_ms,
        "expires_at_unix_ms": expires_at_ms,
        "duration_minutes": duration_minutes,
        "journey_limit": journey_limit,
        "actor": actor.strip(),
        "acknowledgement": acknowledgement.strip(),
        "full_vector_dimensions": 48,
        "raw_response_prose_included": False,
        "capture_can_delay_dispatch": False,
        "witness_only": True,
        "artifact_authority_state_v1": authority_state(),
        "authority": "capture_window_evidence_only_not_dispatch_or_live_control_authority",
    }
    atomic_owner_only_json(capture_window_path(workspace), request)
    return request


def read_status(workspace: Path, *, now_ms: int | None = None) -> dict[str, Any]:
    path = capture_window_path(workspace)
    if not path.is_file():
        return {
            "schema": "signal_spine_capture_status_v1",
            "schema_version": 1,
            "state": "not_armed",
            "path": str(path),
            "artifact_authority_state_v1": authority_state(),
        }
    try:
        request = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        return {
            "schema": "signal_spine_capture_status_v1",
            "schema_version": 1,
            "state": "invalid_request",
            "path": str(path),
            "error": str(error),
            "artifact_authority_state_v1": authority_state(),
        }
    if not isinstance(request, dict):
        validation_error = "request_not_object"
    else:
        validation_error = request_validation_error(request)
    if validation_error:
        return {
            "schema": "signal_spine_capture_status_v1",
            "schema_version": 1,
            "state": "invalid_request",
            "path": str(path),
            "error": validation_error,
            "artifact_authority_state_v1": authority_state(),
        }
    current_ms = int(time.time() * 1_000) if now_ms is None else now_ms
    expires_at = int(request.get("expires_at_unix_ms") or 0)
    capture_id = str(request.get("capture_window_id") or "")
    journeys_dir = capture_root(workspace) / "captures" / capture_id / "journeys"
    journey_count = (
        sum(1 for item in journeys_dir.iterdir() if item.is_file())
        if journeys_dir.is_dir()
        else 0
    )
    limit = int(request.get("journey_limit") or 0)
    state = "armed"
    if current_ms >= expires_at:
        state = "expired"
    elif limit > 0 and journey_count >= limit:
        state = "journey_limit_reached"
    return {
        "schema": "signal_spine_capture_status_v1",
        "schema_version": 1,
        "state": state,
        "path": str(path),
        "capture_window_id": capture_id,
        "expires_at_unix_ms": expires_at,
        "journey_count": journey_count,
        "journey_limit": limit,
        "raw_response_prose_included": False,
        "artifact_authority_state_v1": authority_state(),
    }


def disarm_capture(workspace: Path, *, actor: str, acknowledgement: str) -> dict[str, Any]:
    if not actor.strip() or not acknowledgement.strip():
        raise ValueError("actor and acknowledgement must not be empty")
    path = capture_window_path(workspace)
    previous = read_status(workspace)
    path.unlink(missing_ok=True)
    receipt = {
        "schema": "signal_spine_capture_disarm_receipt_v1",
        "schema_version": 1,
        "disarmed_at": utc_now(),
        "actor": actor.strip(),
        "acknowledgement": acknowledgement.strip(),
        "previous_state": previous.get("state"),
        "capture_window_id": previous.get("capture_window_id"),
        "vectors_deleted": False,
        "evidence_deleted": False,
        "artifact_authority_state_v1": authority_state(),
    }
    receipt_path = capture_root(workspace) / "capture_disarm_receipt.json"
    atomic_owner_only_json(receipt_path, receipt)
    return receipt


class SignalSpineCaptureTests(unittest.TestCase):
    def test_defaults_and_owner_only_permissions(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            workspace = Path(directory)
            request = arm_capture(
                workspace,
                duration_minutes=DEFAULT_DURATION_MINUTES,
                journey_limit=DEFAULT_JOURNEY_LIMIT,
                actor="test",
                acknowledgement="bounded test",
                now_ms=1_000,
            )
            path = capture_window_path(workspace)
            self.assertEqual(request["expires_at_unix_ms"], 1_801_000)
            self.assertEqual(path.stat().st_mode & 0o777, 0o600)
            self.assertEqual(path.parent.stat().st_mode & 0o777, 0o700)
            self.assertEqual(read_status(workspace, now_ms=2_000)["state"], "armed")

    def test_hard_limits_refuse_oversized_capture(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            workspace = Path(directory)
            with self.assertRaises(ValueError):
                arm_capture(
                    workspace,
                    duration_minutes=MAX_DURATION_MINUTES + 1,
                    journey_limit=1,
                    actor="test",
                    acknowledgement="too long",
                )
            with self.assertRaises(ValueError):
                arm_capture(
                    workspace,
                    duration_minutes=1,
                    journey_limit=MAX_JOURNEY_LIMIT + 1,
                    actor="test",
                    acknowledgement="too many",
                )

    def test_status_rejects_true_authority_marker(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            workspace = Path(directory)
            arm_capture(
                workspace,
                duration_minutes=1,
                journey_limit=1,
                actor="test",
                acknowledgement="arm",
                now_ms=1_000,
            )
            request = json.loads(
                capture_window_path(workspace).read_text(encoding="utf-8")
            )
            request["artifact_authority_state_v1"]["grants_approval"] = True
            atomic_owner_only_json(capture_window_path(workspace), request)
            status = read_status(workspace, now_ms=2_000)
            self.assertEqual(status["state"], "invalid_request")
            self.assertEqual(status["error"], "authority_marker_invalid")

    def test_disarm_preserves_evidence(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            workspace = Path(directory)
            arm_capture(
                workspace,
                duration_minutes=1,
                journey_limit=1,
                actor="test",
                acknowledgement="arm",
                now_ms=1_000,
            )
            receipt = disarm_capture(
                workspace, actor="test", acknowledgement="stop capture"
            )
            self.assertFalse(capture_window_path(workspace).exists())
            self.assertFalse(receipt["vectors_deleted"])
            self.assertFalse(receipt["evidence_deleted"])


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--workspace", type=Path, default=DEFAULT_WORKSPACE)
    parser.add_argument("--json", action="store_true")
    parser.add_argument("--self-test", action="store_true")
    commands = parser.add_subparsers(dest="command")

    arm = commands.add_parser("arm")
    arm.add_argument("--duration-minutes", type=int, default=DEFAULT_DURATION_MINUTES)
    arm.add_argument("--journey-limit", type=int, default=DEFAULT_JOURNEY_LIMIT)
    arm.add_argument("--actor", default="interactive-agent")
    arm.add_argument("--ack", required=True)

    commands.add_parser("status")

    disarm = commands.add_parser("disarm")
    disarm.add_argument("--actor", default="interactive-agent")
    disarm.add_argument("--ack", required=True)
    return parser


def emit(value: dict[str, Any], as_json: bool) -> None:
    del as_json
    print(json.dumps(value, indent=2, sort_keys=True, ensure_ascii=False))


def main() -> int:
    parser = build_parser()
    args = parser.parse_args()
    if args.self_test:
        suite = unittest.defaultTestLoader.loadTestsFromTestCase(SignalSpineCaptureTests)
        return 0 if unittest.TextTestRunner(verbosity=2).run(suite).wasSuccessful() else 1
    workspace = args.workspace.resolve()
    try:
        if args.command == "arm":
            emit(
                arm_capture(
                    workspace,
                    duration_minutes=args.duration_minutes,
                    journey_limit=args.journey_limit,
                    actor=args.actor,
                    acknowledgement=args.ack,
                ),
                args.json,
            )
            return 0
        if args.command == "status":
            emit(read_status(workspace), args.json)
            return 0
        if args.command == "disarm":
            emit(
                disarm_capture(
                    workspace, actor=args.actor, acknowledgement=args.ack
                ),
                args.json,
            )
            return 0
    except (OSError, ValueError) as error:
        print(f"signal_spine_capture: {error}", file=sys.stderr)
        return 2
    parser.print_help()
    return 2


if __name__ == "__main__":
    raise SystemExit(main())
