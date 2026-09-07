#!/usr/bin/env python3
"""Tests for the deterministic being-test-proposal applier (Stage 1)."""

from __future__ import annotations

import json
from pathlib import Path
import subprocess
import tempfile
import unittest

try:
    import test_proposal_applier as applier
except ModuleNotFoundError:  # pragma: no cover
    from scripts import test_proposal_applier as applier

TARGET = "capsules/spectral-bridge/src/codec/tests.rs"

MOD_WRAPPED = """#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn existing_test() {
        assert_eq!(1, 1);
    }
}
"""

PROPOSED_CODE = """#[test]
fn being_authored_example() {
    assert_eq!(2_u32.saturating_add(2), 4);
}"""


def _git(repo: Path, *argv: str) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        ["git", *argv], cwd=repo, capture_output=True, text=True, check=False
    )


class AppendConstructionTests(unittest.TestCase):
    def test_mod_wrapped_inserts_before_final_brace(self) -> None:
        result = applier.build_appended(MOD_WRAPPED, PROPOSED_CODE, "mod_wrapped")
        self.assertTrue(result.endswith("}\n"))
        self.assertIn("    #[test]\n    fn being_authored_example()", result)
        self.assertLess(
            result.find("existing_test"), result.find("being_authored_example")
        )
        # The module close brace remains the last non-empty line.
        self.assertEqual(result.rstrip().rsplit("\n", 1)[-1], "}")

    def test_eof_appends_at_zero_indent(self) -> None:
        content = "use super::*;\n\n#[test]\nfn first() {}\n"
        result = applier.build_appended(content, PROPOSED_CODE, "eof")
        self.assertTrue(result.endswith("fn being_authored_example() {\n    assert_eq!(2_u32.saturating_add(2), 4);\n}\n"))
        self.assertIn("\n\n#[test]\nfn being_authored_example", result)

    def test_indent_block_preserves_blank_lines(self) -> None:
        block = applier.indent_block("a\n\nb", 4)
        self.assertEqual(block, "    a\n\n    b")


class StaticValidationTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temp = tempfile.TemporaryDirectory()
        self.repo = Path(self.temp.name)
        target = self.repo / TARGET
        target.parent.mkdir(parents=True)
        target.write_text(MOD_WRAPPED, encoding="utf-8")

    def tearDown(self) -> None:
        self.temp.cleanup()

    def proposal(self, **overrides) -> dict:
        base = {
            "schema": "being_test_proposal_v1",
            "proposal_id": "proposal_1_being_authored_example",
            "being": "astrid",
            "target_path": TARGET,
            "test_name": "being_authored_example",
            "code": PROPOSED_CODE,
        }
        base.update(overrides)
        return base

    def test_accepts_wellformed_proposal(self) -> None:
        self.assertIsNone(applier.validate_static(self.proposal(), self.repo))

    def test_rejects_offlist_target_and_collisions_and_denylist(self) -> None:
        self.assertIn(
            "allowlist",
            applier.validate_static(
                self.proposal(target_path="capsules/spectral-bridge/src/codec/core.rs"),
                self.repo,
            ),
        )
        self.assertIn(
            "already exists",
            applier.validate_static(
                self.proposal(
                    test_name="existing_test",
                    code="#[test]\nfn existing_test() {}",
                ),
                self.repo,
            ),
        )
        self.assertIn(
            "not allowed",
            applier.validate_static(
                self.proposal(code="#[test]\nfn being_authored_example() { unsafe {} }"),
                self.repo,
            ),
        )
        self.assertIn(
            "#[test]",
            applier.validate_static(
                self.proposal(code="fn being_authored_example() {}"), self.repo
            ),
        )


class LandingFlowTests(unittest.TestCase):
    """End-to-end (cargo skipped): hunk isolation, authorship, lifecycle."""

    def setUp(self) -> None:
        self.temp = tempfile.TemporaryDirectory()
        root = Path(self.temp.name)
        self.repo = root / "repo"
        target = self.repo / TARGET
        target.parent.mkdir(parents=True)
        target.write_text(MOD_WRAPPED, encoding="utf-8")
        _git(self.repo, "init", "-q")
        _git(self.repo, "config", "user.name", "Fixture")
        _git(self.repo, "config", "user.email", "fixture@example.com")
        _git(self.repo, "add", "-A")
        _git(self.repo, "commit", "-q", "-m", "seed")
        # Foreign INTERIOR edit that must never be committed by the applier.
        content = target.read_text(encoding="utf-8")
        target.write_text(
            content.replace("use super::*;", "use super::*; // foreign WIP"),
            encoding="utf-8",
        )
        # Unrelated foreign dirty file.
        (self.repo / "foreign.txt").write_text("foreign\n", encoding="utf-8")

        # Redirect the module's workspace surfaces into the fixture.
        self._saved = {
            name: getattr(applier, name)
            for name in ("PROPOSALS_DIR", "INBOX_DIR", "CHANGELOG", "LEDGER", "LOG_PATH")
        }
        applier.PROPOSALS_DIR = root / "test_proposals"
        applier.INBOX_DIR = root / "inbox"
        applier.CHANGELOG = root / "CHANGELOG.md"
        applier.LEDGER = root / "LEDGER.md"
        applier.LOG_PATH = root / "applier.log"
        applier.CHANGELOG.write_text("# Log\n\n## [Unreleased]\n\n", encoding="utf-8")
        applier.LEDGER.write_text("# Ledger\n\n## Ledger\n\n", encoding="utf-8")
        applier.PROPOSALS_DIR.mkdir()

    def tearDown(self) -> None:
        for name, value in self._saved.items():
            setattr(applier, name, value)
        self.temp.cleanup()

    def file_proposal(self, **overrides) -> Path:
        proposal = {
            "schema": "being_test_proposal_v1",
            "proposal_id": "proposal_1_being_authored_example",
            "being": "astrid",
            "target_path": TARGET,
            "test_name": "being_authored_example",
            "code": PROPOSED_CODE,
            "status": "pending",
        }
        proposal.update(overrides)
        path = applier.PROPOSALS_DIR / f"{proposal['proposal_id']}.json"
        path.write_text(json.dumps(proposal), encoding="utf-8")
        return path

    def test_lands_with_being_author_and_hunk_isolation(self) -> None:
        self.file_proposal()
        summary = applier.apply_next_proposal(self.repo, skip_cargo=True)
        self.assertIn("LANDED", summary)

        author = _git(self.repo, "log", "--format=%an <%ae>", "-1").stdout.strip()
        self.assertEqual(author, "Astrid <astrid@spectral-bridge.local>")
        changed = _git(
            self.repo, "show", "--name-only", "--format=", "HEAD"
        ).stdout.split()
        self.assertEqual(changed, [TARGET])

        committed = _git(self.repo, "show", f"HEAD:{TARGET}").stdout
        self.assertIn("being_authored_example", committed)
        self.assertNotIn("foreign WIP", committed)

        # Foreign edits remain uncommitted in the worktree.
        dirty = _git(self.repo, "status", "--porcelain").stdout
        self.assertIn("foreign.txt", dirty)
        self.assertIn(TARGET, dirty)
        worktree = (self.repo / TARGET).read_text(encoding="utf-8")
        self.assertIn("foreign WIP", worktree)
        self.assertIn("being_authored_example", worktree)

        # Lifecycle + letters + bookkeeping.
        landed = list((applier.PROPOSALS_DIR / "reviewed/landed").glob("*.json"))
        self.assertEqual(len(landed), 1)
        record = json.loads(landed[0].read_text(encoding="utf-8"))
        self.assertEqual(record["status"], "landed")
        self.assertIn("commit", record)
        letters = list(applier.INBOX_DIR.glob("mike_feedback_test_proposal_*.txt"))
        self.assertEqual(len(letters), 1)
        self.assertIn("author line", letters[0].read_text(encoding="utf-8"))
        self.assertIn(
            "being_authored_example",
            applier.CHANGELOG.read_text(encoding="utf-8"),
        )

    def test_static_rejection_moves_to_failed_with_letter(self) -> None:
        self.file_proposal(code="#[test]\nfn being_authored_example() { unsafe {} }")
        summary = applier.apply_next_proposal(self.repo, skip_cargo=True)
        self.assertIn("static rejection", summary)
        failed = list((applier.PROPOSALS_DIR / "reviewed/failed").glob("*.json"))
        self.assertEqual(len(failed), 1)
        letters = list(applier.INBOX_DIR.glob("mike_feedback_test_proposal_*.txt"))
        self.assertEqual(len(letters), 1)
        self.assertIn("not allowed", letters[0].read_text(encoding="utf-8"))
        # Nothing landed.
        self.assertEqual(
            _git(self.repo, "log", "--oneline").stdout.count("\n"), 1
        )

    def test_refuses_dirty_index(self) -> None:
        (self.repo / "staged.txt").write_text("x\n", encoding="utf-8")
        _git(self.repo, "add", "staged.txt")
        self.file_proposal()
        with self.assertRaises(RuntimeError):
            applier.apply_next_proposal(self.repo, skip_cargo=True)


class StandDownTests(unittest.TestCase):
    def test_lease_file_blocks(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            lease = Path(tmp) / "lease.json"
            lease.write_text("{}", encoding="utf-8")
            saved = applier.LEASE_PATH
            applier.LEASE_PATH = lease
            try:
                reason = applier.stand_down_reason(Path(tmp))
                self.assertIn("lease", reason)
            finally:
                applier.LEASE_PATH = saved


if __name__ == "__main__":
    unittest.main()
