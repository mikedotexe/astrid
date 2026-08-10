from __future__ import annotations

import hashlib
import json
import pathlib
import subprocess
import sys
import tempfile
import unittest


SCRIPT = pathlib.Path(__file__).with_name("migrate_edge_operator_harness_isolation.py")


class OperatorHarnessIsolationMigrationTests(unittest.TestCase):
    def fixture(self) -> tuple[tempfile.TemporaryDirectory[str], pathlib.Path, pathlib.Path, bytes]:
        temporary = tempfile.TemporaryDirectory()
        root = pathlib.Path(temporary.name)
        workspace = root / "edge"
        hindsight = root / "hindsight"
        (workspace / "web").mkdir(parents=True)
        hindsight.mkdir()
        prefix = b'{"call_id":"natural","phase":"completed"}\n'
        ledger = workspace / "web/receipts.jsonl"
        ledger.write_bytes(
            prefix
            + b'{"call_id":"edge-operator-inquiry-search-1","phase":"requested"}\n'
            + b'{"call_id":"natural-later","phase":"completed"}\n'
        )
        checkpoint = {
            "schema": "astrid_edge_hindsight_checkpoint_v2",
            "ledgers": {
                "web/receipts.jsonl": {
                    "inode": ledger.stat().st_ino,
                    "size_bytes": len(prefix),
                    "sha256": hashlib.sha256(prefix).hexdigest(),
                }
            },
        }
        (hindsight / "checkpoints.jsonl").write_text(json.dumps(checkpoint) + "\n")
        return temporary, workspace, hindsight, prefix

    def run_migration(self, workspace: pathlib.Path, hindsight: pathlib.Path) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            [
                sys.executable,
                str(SCRIPT),
                "--workspace",
                str(workspace),
                "--hindsight-root",
                str(hindsight),
            ],
            check=False,
            capture_output=True,
            text=True,
        )

    def test_removes_only_harness_tail_and_preserves_prefix_inode_and_later_activity(self) -> None:
        temporary, workspace, hindsight, prefix = self.fixture()
        self.addCleanup(temporary.cleanup)
        ledger = workspace / "web/receipts.jsonl"
        inode = ledger.stat().st_ino
        result = self.run_migration(workspace, hindsight)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(
            ledger.read_bytes(),
            prefix + b'{"call_id":"natural-later","phase":"completed"}\n',
        )
        self.assertEqual(ledger.stat().st_ino, inode)
        self.assertEqual(ledger.stat().st_mode & 0o777, 0o600)
        receipts = list((hindsight / "isolation-repairs").glob("repair_*.json"))
        self.assertEqual(len(receipts), 1)
        self.assertEqual(json.loads(receipts[0].read_text())["removed_records"], 1)

    def test_refuses_a_changed_captured_prefix(self) -> None:
        temporary, workspace, hindsight, _prefix = self.fixture()
        self.addCleanup(temporary.cleanup)
        ledger = workspace / "web/receipts.jsonl"
        data = ledger.read_bytes()
        ledger.write_bytes(b"X" + data[1:])
        result = self.run_migration(workspace, hindsight)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("captured hindsight prefix no longer verifies", result.stderr)


if __name__ == "__main__":
    unittest.main()
