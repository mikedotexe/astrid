#!/usr/bin/env python3
"""Freeze clean, exact Git source for CPU-edge rehearsal, not deployment."""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import re
import subprocess
import tomllib


def git(repo: Path, *args: str) -> bytes:
    return subprocess.check_output(["git", "-C", str(repo), *args])


def resolve_commit(repo: Path, ref: str) -> str:
    return git(repo, "rev-parse", "--verify", "--end-of-options", f"{ref}^{{commit}}").decode().strip()


def require_clean(repo: Path, commit: str) -> None:
    if resolve_commit(repo, "HEAD") != commit:
        raise ValueError("checkout HEAD does not match the expected source commit")
    if git(repo, "status", "--porcelain", "--untracked-files=all"):
        raise ValueError("source checkout must have no tracked or untracked changes")


def is_build_input(path: str) -> bool:
    return (
        Path(path).name == "Cargo.lock"
        or path in {"Cargo.toml", "rust-toolchain.toml", ".github/workflows/cpu-edge.yml", ".github/workflows/release.yml"}
        or path.startswith("packaging/headless/")
    )


def prepare(repo: Path, source_ref: str, expected_commit: str, output: Path) -> dict:
    if not re.fullmatch(r"[0-9a-f]{40}", expected_commit):
        raise ValueError("expected commit must be a full lowercase SHA-1 commit ID")
    if not source_ref.startswith("refs/"):
        raise ValueError("source ref must be explicit, such as refs/tags/introspection")
    repo = repo.resolve(strict=True)
    output = output.resolve()
    if output == repo or repo in output.parents:
        raise ValueError("output must be outside the source checkout")
    commit = resolve_commit(repo, source_ref)
    if commit != expected_commit:
        raise ValueError("source ref does not resolve to the expected commit")
    require_clean(repo, commit)
    paths = git(repo, "ls-tree", "-rz", "--name-only", commit).decode().split("\0")
    inputs = {}
    for path in sorted(p for p in paths if p and is_build_input(p)):
        data = git(repo, "show", f"{commit}:{path}")
        inputs[path] = {"sha256": hashlib.sha256(data).hexdigest(), "bytes": len(data)}
    workspace = tomllib.loads(git(repo, "show", f"{commit}:Cargo.toml").decode())
    specifications = {}
    for name in ("astralis-cpu-edge-capsules.toml", "astralis-cpu-edge-baseline-capsules.toml"):
        specifications[name] = tomllib.loads(git(repo, "show", f"{commit}:packaging/headless/{name}").decode())

    # A new private directory is the transaction boundary; failed attempts remain
    # visibly incomplete and are never silently overwritten or called releases.
    output.mkdir(mode=0o700)
    archive = output / "source.tar.gz"
    with archive.open("xb") as stream:
        subprocess.run(
            ["git", "-C", str(repo), "archive", "--format=tar.gz", "--prefix=source/", commit],
            stdout=stream, check=True,
        )
    digest = hashlib.sha256()
    with archive.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    require_clean(repo, commit)
    if resolve_commit(repo, source_ref) != commit:
        raise ValueError("source ref changed during preparation")
    receipt = {
        "schema": "astrid_edge_source_candidate_v1",
        "status": "source_pinned_not_linux_release_qualified",
        "deploy_authorized": False,
        "source_ref": source_ref,
        "source_commit": commit,
        "source_tree": git(repo, "rev-parse", f"{commit}^{{tree}}").decode().strip(),
        "source_tree_state": "clean",
        "workspace_version": workspace["workspace"]["package"]["version"],
        "source_archive": {"file": archive.name, "sha256": digest.hexdigest(), "bytes": archive.stat().st_size},
        "build_inputs": inputs,
        "external_capsule_specifications": specifications,
        "edge_build_environment": {"ASTRID_EDGE_SOURCE_COMMIT": commit},
        "limitations": [
            "Source archive only; not a signed offline source/toolchain closure or installable bundle.",
            "External sources and dependencies are recorded, not fetched, vendored, built, or verified.",
            "No Linux ABI/ISA, capsule compatibility, live-state migration, or backup qualification.",
            "No deployment authority or appliance identity is created.",
        ],
    }
    with (output / "candidate.json").open("x") as stream:
        json.dump(receipt, stream, indent=2)
        stream.write("\n")
    return receipt


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", type=Path, required=True)
    parser.add_argument("--source-ref", required=True)
    parser.add_argument("--expected-commit", required=True)
    parser.add_argument("--output-dir", type=Path, required=True, help="new directory; parent must exist")
    args = parser.parse_args()
    try:
        result = prepare(args.repo, args.source_ref, args.expected_commit, args.output_dir)
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        parser.exit(1, f"error: {error}\n")
    print(json.dumps({key: result[key] for key in ("status", "source_commit", "source_archive")}, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
