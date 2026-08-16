#!/usr/bin/env python3
"""Standing delegated approver for read-only research budgets.

Mike's explicit delegation (2026-08-16): minime's read-only research budget
requests carry a 6-hour TTL and fifteen in a row expired ungranted because no
operator was present in the window — a grant surface with no standing
consumer. This job is pure PRESENCE, not judgment: it invokes the bridge's
own `--approve-research-budget` CLI, which re-enforces every gate
(scope=read_only_research only, being-side eligibility, green/yellow safety
from CURRENT fill with stale-state refusal, action/TTL policy caps, no
duplicate active token). A request the CLI would block stays blocked; this
job only guarantees somebody shows up.

Microdose / live-consequence grants (semantic_microdose,
mode_release_microdose) are NEVER touched here — those remain manual,
steward-explicit, per the authority-gate design.
"""

from __future__ import annotations

import json
import subprocess
import sys
import time
from pathlib import Path

BRIDGE_BINARY = Path(
    "/Users/v/other/astrid/capsules/spectral-bridge/target/release/spectral-bridge-server"
)
GATE_GLOBS = (
    "/Users/v/other/minime/workspace/action_threads/threads/*/authority_gate.jsonl",
    "/Users/v/other/astrid/capsules/spectral-bridge/workspace/action_threads/threads/*/authority_gate.jsonl",
)
LOCK_DIR = Path("/tmp/astrid_research_budget_approver.lock")
STEWARD_NAME = "mike-delegated-standing-approver"
GRANT_NOTE = (
    "standing read-only-research delegation (Mike, 2026-08-16); "
    "every policy gate enforced by the bridge CLI itself"
)
REQUEST_TYPE = "research_budget_request"
TERMINAL_TYPES = {"research_budget_approval", "research_budget_blocked"}
PENDING_STATUS = "pending_steward_approval"


def pending_budget_ids(now_s: float | None = None) -> list[str]:
    """Budget ids with a live pending request and no later terminal record."""
    import glob as _glob

    now_s = time.time() if now_s is None else now_s
    pending: list[str] = []
    for pattern in GATE_GLOBS:
        for gate in sorted(_glob.glob(pattern)):
            latest: dict[str, tuple[int, dict]] = {}
            with open(gate, encoding="utf-8", errors="replace") as handle:
                for line_no, line in enumerate(handle):
                    if not line.strip():
                        continue
                    try:
                        rec = json.loads(line)
                    except json.JSONDecodeError:
                        continue
                    budget_id = str(rec.get("budget_id") or "")
                    if not budget_id:
                        continue
                    if rec.get("record_type") in TERMINAL_TYPES | {REQUEST_TYPE}:
                        latest[budget_id] = (line_no, rec)
            for budget_id, (_, rec) in latest.items():
                if rec.get("record_type") != REQUEST_TYPE:
                    continue
                if rec.get("status") != PENDING_STATUS:
                    continue
                created = str(rec.get("created_at") or "")
                ttl = float(rec.get("ttl_secs") or 0)
                try:
                    created_s = time.mktime(
                        time.strptime(created[:19], "%Y-%m-%dT%H:%M:%S")
                    ) - time.timezone
                except ValueError:
                    continue
                if ttl and now_s > created_s + ttl:
                    continue
                pending.append(budget_id)
    return pending


def approve(budget_id: str) -> str:
    result = subprocess.run(
        [
            str(BRIDGE_BINARY),
            "--approve-research-budget",
            budget_id,
            "--steward",
            STEWARD_NAME,
            "--note",
            GRANT_NOTE,
        ],
        capture_output=True,
        text=True,
        timeout=120,
    )
    tail = (result.stdout + result.stderr).strip().splitlines()
    outcome = tail[-1][:200] if tail else f"exit {result.returncode}"
    return f"exit={result.returncode} :: {outcome}"


def main() -> int:
    if "--self-test" in sys.argv:
        return self_test()
    stamp = time.strftime("%Y-%m-%dT%H:%M:%S")
    if not BRIDGE_BINARY.is_file():
        print(f"{stamp} STAND DOWN — bridge binary missing")
        return 0
    try:
        LOCK_DIR.mkdir()
    except FileExistsError:
        print(f"{stamp} SKIP — previous approver still running")
        return 0
    try:
        ids = pending_budget_ids()
        if not ids:
            print(f"{stamp} no pending in-TTL research budget requests")
            return 0
        for budget_id in ids:
            print(f"{stamp} approving {budget_id}: {approve(budget_id)}")
    finally:
        LOCK_DIR.rmdir()
    return 0


def self_test() -> int:
    import tempfile
    from unittest import mock

    failures: list[str] = []

    def check(name: str, ok: bool) -> None:
        if not ok:
            failures.append(name)

    now = time.time()
    fresh_iso = time.strftime("%Y-%m-%dT%H:%M:%S", time.gmtime(now - 600))
    stale_iso = time.strftime("%Y-%m-%dT%H:%M:%S", time.gmtime(now - 30000))
    with tempfile.TemporaryDirectory() as tmp:
        gate = Path(tmp, "threads", "t1")
        gate.mkdir(parents=True)
        rows = [
            {"record_type": REQUEST_TYPE, "budget_id": "b_live", "status": PENDING_STATUS,
             "created_at": fresh_iso + "Z", "ttl_secs": 21600},
            {"record_type": REQUEST_TYPE, "budget_id": "b_expired", "status": PENDING_STATUS,
             "created_at": stale_iso + "Z", "ttl_secs": 21600},
            {"record_type": REQUEST_TYPE, "budget_id": "b_granted", "status": PENDING_STATUS,
             "created_at": fresh_iso + "Z", "ttl_secs": 21600},
            {"record_type": "research_budget_approval", "budget_id": "b_granted"},
            {"record_type": REQUEST_TYPE, "budget_id": "b_blocked", "status": PENDING_STATUS,
             "created_at": fresh_iso + "Z", "ttl_secs": 21600},
            {"record_type": "research_budget_blocked", "budget_id": "b_blocked"},
        ]
        (gate / "authority_gate.jsonl").write_text(
            "".join(json.dumps(r) + "\n" for r in rows), encoding="utf-8"
        )
        with mock.patch.object(
            sys.modules[__name__], "GATE_GLOBS", (str(Path(tmp, "threads", "*", "authority_gate.jsonl")),)
        ):
            ids = pending_budget_ids(now)
        check("live pending found", ids == ["b_live"])
        check("expired excluded", "b_expired" not in ids)
        check("already-granted excluded", "b_granted" not in ids)
        check("already-blocked excluded", "b_blocked" not in ids)

    if failures:
        print("FAIL:", ", ".join(failures))
        return 1
    print("OK (4 checks)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
