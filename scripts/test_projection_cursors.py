"""Tests for owner-only incremental source cursors."""

from __future__ import annotations

from pathlib import Path
import stat
import tempfile
import unittest

from projection_cursors import (
    ProjectionCursorError,
    ProjectionInputCursor,
)


class ProjectionInputCursorTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)
        self.source = self.root / "source.jsonl"
        self.cursor_path = self.root / "state/cursor.json"

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def test_jsonl_tail_resumes_without_rereading_settled_rows(self) -> None:
        self.source.write_text('{"n":1}\n{"n":2}\n', encoding="utf-8")
        cursor = ProjectionInputCursor(self.cursor_path, "fixture")
        rows, state = cursor.jsonl_tail(self.source, key="source")
        self.assertEqual([line for line, _ in rows], [1, 2])
        cursor.commit_jsonl({"source": state})
        self.assertEqual(
            stat.S_IMODE(self.cursor_path.stat().st_mode),
            0o600,
        )

        with self.source.open("a", encoding="utf-8") as handle:
            handle.write('{"n":3}\n')
        resumed = ProjectionInputCursor(self.cursor_path, "fixture")
        rows, state = resumed.jsonl_tail(self.source, key="source")
        self.assertEqual(rows, [(3, '{"n":3}')])
        resumed.commit_jsonl({"source": state})
        rows, _ = resumed.jsonl_tail(self.source, key="source")
        self.assertEqual(rows, [])

    def test_settled_prefix_tampering_and_truncation_fail_closed(self) -> None:
        self.source.write_text('{"n":1}\n', encoding="utf-8")
        cursor = ProjectionInputCursor(self.cursor_path, "fixture")
        _, state = cursor.jsonl_tail(self.source, key="source")
        cursor.commit_jsonl({"source": state})

        self.source.write_text('{"n":9}\n', encoding="utf-8")
        with self.assertRaisesRegex(
            ProjectionCursorError,
            "settled_prefix_tampered",
        ):
            cursor.jsonl_tail(self.source, key="source")

        self.source.write_text("", encoding="utf-8")
        with self.assertRaisesRegex(
            ProjectionCursorError,
            "append_only_source_truncated",
        ):
            cursor.jsonl_tail(self.source, key="source")

    def test_file_manifest_returns_only_changed_content(self) -> None:
        left = self.root / "left.json"
        right = self.root / "right.json"
        left.write_text("left", encoding="utf-8")
        right.write_text("right", encoding="utf-8")
        cursor = ProjectionInputCursor(self.cursor_path, "fixture")
        changed, manifest, removed = cursor.changed_files(
            (left, right),
            root=self.root,
        )
        self.assertEqual(changed, [left, right])
        self.assertEqual(removed, [])
        cursor.commit_files(manifest)

        right.write_text("changed", encoding="utf-8")
        changed, manifest, removed = cursor.changed_files(
            (left, right),
            root=self.root,
        )
        self.assertEqual(changed, [right])
        self.assertEqual(removed, [])
        cursor.commit_files(manifest)

        changed, _, removed = cursor.changed_files((right,), root=self.root)
        self.assertEqual(changed, [])
        self.assertEqual(removed, ["left.json"])

    def test_world_readable_cursor_is_rejected(self) -> None:
        cursor = ProjectionInputCursor(self.cursor_path, "fixture")
        cursor.commit_jsonl({})
        self.cursor_path.chmod(0o644)
        with self.assertRaisesRegex(ProjectionCursorError, "owner-only"):
            ProjectionInputCursor(self.cursor_path, "fixture")


if __name__ == "__main__":
    unittest.main()
