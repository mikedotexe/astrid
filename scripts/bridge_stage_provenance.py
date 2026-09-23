#!/usr/bin/env python3
"""Link a staged bridge release to the commits its (possibly dirty) inputs became.

A stage manifest records `repository.head` (the commit checked out at build
time), `repository.dirty_paths` (uncommitted paths folded into the build) and
`source-inputs.json` (sha256 of every build input). The running process then
carries a deployment identity of the form `astrid:<head>:bridge:<binary>`. When
the dirty inputs are committed AFTER activation (the shared-tree flow), the
identity's <head> is one commit behind the bytes that are actually running.

`record` verifies, by content hash, that each dirty BUILD INPUT equals the blob
at one of the given commits, and writes `<stage>/committed_as.json`. `verify`
re-checks an existing receipt. Read-only apart from that one receipt; no live
state, launch selection or git mutation.

Usage:
    python3 scripts/bridge_stage_provenance.py record --stage DIR --commit SHA [--commit SHA ...] [--actor NAME] [--write]
    python3 scripts/bridge_stage_provenance.py verify --stage DIR
    python3 scripts/bridge_stage_provenance.py --self-test
"""
from __future__ import annotations

import argparse
import hashlib
import json
import subprocess
import sys
import time
import unittest
from pathlib import Path
from typing import Callable

ASTRID_REPO = Path("/Users/v/other/astrid")
RECEIPT_NAME = "committed_as.json"
SCHEMA = "bridge_stage_committed_as_v1"

BlobSha = Callable[[str, str], str | None]


def git_blob_sha256(commit: str, rel_path: str, repo: Path = ASTRID_REPO) -> str | None:
    """sha256 of `rel_path` as committed at `commit`, or None if absent there."""
    res = subprocess.run(
        ["git", "-C", str(repo), "show", f"{commit}:{rel_path}"],
        capture_output=True,
    )
    if res.returncode != 0:
        return None
    return hashlib.sha256(res.stdout).hexdigest()


def git_is_ancestor(ancestor: str, descendant: str, repo: Path = ASTRID_REPO) -> bool:
    res = subprocess.run(
        ["git", "-C", str(repo), "merge-base", "--is-ancestor", ancestor, descendant],
        capture_output=True,
    )
    return res.returncode == 0


def load_stage(stage: Path) -> tuple[dict, dict[str, str]]:
    manifest = json.loads((stage / "manifest.json").read_text(encoding="utf-8"))
    inputs_path = Path((manifest.get("source_inputs") or {}).get("path") or (stage / "source-inputs.json"))
    if not inputs_path.is_absolute():
        inputs_path = stage / inputs_path
    inventory = json.loads(inputs_path.read_text(encoding="utf-8"))
    files: dict[str, str] = {}
    for entry in inventory.get("files") or []:
        path = str(entry.get("path") or "")
        sha = str(entry.get("sha256") or "")
        if path and sha:
            files[path] = sha
    return manifest, files


def _relative(path: str, repo_root: str) -> str:
    root = repo_root.rstrip("/") + "/"
    return path[len(root):] if path.startswith(root) else path


def link_dirty_inputs(
    manifest: dict,
    files: dict[str, str],
    commits: list[str],
    blob_sha: BlobSha,
) -> dict:
    """Pure core: match each dirty build input's staged sha to one of `commits`."""
    repository = manifest.get("repository") or {}
    repo_root = str(repository.get("path") or "")
    head = str(repository.get("head") or "")
    dirty_paths = [str(p) for p in (repository.get("dirty_paths") or [])]
    by_relative = {_relative(path, repo_root): sha for path, sha in files.items()}
    verified: dict[str, str] = {}
    unverified: dict[str, str] = {}
    not_build_inputs: list[str] = []
    for rel in dirty_paths:
        staged_sha = by_relative.get(rel)
        if staged_sha is None:
            not_build_inputs.append(rel)
            continue
        match = next((c for c in commits if blob_sha(c, rel) == staged_sha), None)
        if match is None:
            unverified[rel] = staged_sha[:16]
        else:
            verified[rel] = match
    return {
        "schema": SCHEMA,
        "head_at_build": head,
        "dirty_at_build": bool(repository.get("dirty")),
        "commits": commits,
        "verified_paths": verified,
        "unverified_paths": unverified,
        "not_build_inputs": not_build_inputs,
        "complete": not unverified,
    }


def record(stage: Path, commits: list[str], actor: str, write: bool, blob_sha: BlobSha = git_blob_sha256) -> dict:
    manifest, files = load_stage(stage)
    receipt = link_dirty_inputs(manifest, files, commits, blob_sha)
    head = receipt["head_at_build"]
    receipt["commits_contain_head"] = {
        c: (git_is_ancestor(head, c) if head else False) for c in commits
    }
    receipt["manifest_sha256"] = hashlib.sha256((stage / "manifest.json").read_bytes()).hexdigest()
    receipt["binary_sha256"] = manifest.get("binary_sha256")
    receipt["stage"] = str(stage)
    receipt["actor"] = actor
    receipt["recorded_at_unix_s"] = time.time()
    receipt["authority"] = "provenance_witness_only_no_launch_or_git_change"
    if write:
        (stage / RECEIPT_NAME).write_text(json.dumps(receipt, indent=1), encoding="utf-8")
    return receipt


def verify(stage: Path, blob_sha: BlobSha = git_blob_sha256) -> dict:
    receipt_path = stage / RECEIPT_NAME
    if not receipt_path.is_file():
        return {"ok": False, "reason": "no committed_as.json receipt in this stage"}
    receipt = json.loads(receipt_path.read_text(encoding="utf-8"))
    manifest, files = load_stage(stage)
    fresh = link_dirty_inputs(manifest, files, list(receipt.get("commits") or []), blob_sha)
    ok = (
        fresh["complete"]
        and fresh["verified_paths"] == receipt.get("verified_paths")
        and hashlib.sha256((stage / "manifest.json").read_bytes()).hexdigest() == receipt.get("manifest_sha256")
    )
    return {"ok": ok, "receipt": receipt, "fresh": fresh}


def summary_line(stage: Path) -> str | None:
    """One line for operator tools: head at build, dirty inputs, and what they became."""
    try:
        manifest, _files = load_stage(stage)
    except (OSError, ValueError, json.JSONDecodeError):
        return None
    repository = manifest.get("repository") or {}
    head = str(repository.get("head") or "")[:10]
    dirty = repository.get("dirty_paths") or []
    receipt_path = stage / RECEIPT_NAME
    if receipt_path.is_file():
        try:
            receipt = json.loads(receipt_path.read_text(encoding="utf-8"))
        except (OSError, ValueError, json.JSONDecodeError):
            receipt = {}
        commits = ", ".join(str(c)[:10] for c in receipt.get("commits") or [])
        state = "verified" if receipt.get("complete") else "INCOMPLETE"
        return (
            f"stage built from {head} with {len(dirty)} dirty path(s); "
            f"committed as {commits} ({state}, {len(receipt.get('verified_paths') or {})} build input(s) matched by hash)"
        )
    if dirty:
        return f"stage built from {head} with {len(dirty)} dirty path(s); no committed_as.json receipt yet"
    return f"stage built from {head} (clean tree)"


class ProvenanceTests(unittest.TestCase):
    def _manifest(self, dirty):
        return {
            "repository": {"path": "/repo", "head": "aaaa", "dirty": bool(dirty), "dirty_paths": dirty},
            "binary_sha256": "bin",
        }

    def test_dirty_build_inputs_link_to_the_commit_holding_the_same_bytes(self):
        files = {"/repo/src/a.rs": "sha_a", "/repo/src/b.rs": "sha_b"}
        blobs = {("c1", "src/a.rs"): "sha_a", ("c2", "src/b.rs"): "sha_b"}
        receipt = link_dirty_inputs(
            self._manifest(["src/a.rs", "src/b.rs", "docs/note.md"]), files, ["c1", "c2"],
            lambda c, p: blobs.get((c, p)),
        )
        self.assertTrue(receipt["complete"])
        self.assertEqual(receipt["verified_paths"], {"src/a.rs": "c1", "src/b.rs": "c2"})
        self.assertEqual(receipt["not_build_inputs"], ["docs/note.md"])

    def test_unmatched_input_marks_receipt_incomplete(self):
        files = {"/repo/src/a.rs": "sha_a"}
        receipt = link_dirty_inputs(self._manifest(["src/a.rs"]), files, ["c1"], lambda c, p: "different")
        self.assertFalse(receipt["complete"])
        self.assertIn("src/a.rs", receipt["unverified_paths"])

    def test_clean_build_has_nothing_to_link(self):
        receipt = link_dirty_inputs(self._manifest([]), {"/repo/src/a.rs": "x"}, ["c1"], lambda c, p: None)
        self.assertTrue(receipt["complete"])
        self.assertEqual(receipt["verified_paths"], {})


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--self-test", action="store_true")
    sub = parser.add_subparsers(dest="cmd")
    rec = sub.add_parser("record")
    rec.add_argument("--stage", type=Path, required=True)
    rec.add_argument("--commit", action="append", required=True)
    rec.add_argument("--actor", default="interactive-agent")
    rec.add_argument("--write", action="store_true")
    ver = sub.add_parser("verify")
    ver.add_argument("--stage", type=Path, required=True)
    args = parser.parse_args(argv)
    if args.self_test:
        suite = unittest.defaultTestLoader.loadTestsFromTestCase(ProvenanceTests)
        return 0 if unittest.TextTestRunner(verbosity=1).run(suite).wasSuccessful() else 1
    if args.cmd == "record":
        receipt = record(args.stage.resolve(), args.commit, args.actor, args.write)
        print(json.dumps({k: receipt[k] for k in ("head_at_build", "commits", "verified_paths", "unverified_paths", "not_build_inputs", "complete", "commits_contain_head")}, indent=1))
        print("written" if args.write else "dry-run (add --write)")
        return 0 if receipt["complete"] else 2
    if args.cmd == "verify":
        result = verify(args.stage.resolve())
        print(json.dumps({"ok": result.get("ok"), "reason": result.get("reason"), "summary": summary_line(args.stage.resolve())}, indent=1))
        return 0 if result.get("ok") else 2
    parser.print_help()
    return 64


if __name__ == "__main__":
    sys.exit(main())
