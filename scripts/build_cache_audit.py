#!/usr/bin/env python3
"""Read-only Cargo cache budget check. This tool never deletes or authorizes cleanup."""

from __future__ import annotations

import argparse
import json
import math
import os
from pathlib import Path
import shutil
import subprocess


GIB = 1024**3
ROOT = Path(__file__).resolve().parents[1]
TAG = b"Signature: 8a477f597d28d172789f06886806bc55"
DEFAULT_TARGETS = (ROOT / "target", ROOT / "capsules/spectral-bridge/target")


def positive_gib(value: str) -> float:
    number = float(value)
    if not math.isfinite(number) or number <= 0:
        raise argparse.ArgumentTypeError("expected a finite, positive GiB value")
    return number


def existing_parent(path: Path) -> Path:
    while not path.exists():
        if path == path.parent:
            raise ValueError("no existing ancestor")
        path = path.parent
    return path


def validate_target(path: Path) -> Path:
    path = Path(os.path.abspath(path.expanduser()))
    if path.resolve() != path:
        raise ValueError(f"symlinked target or parent is not supported: {path}")
    if path.exists():
        tag = path / "CACHEDIR.TAG"
        if not path.is_dir() or tag.is_symlink() or not tag.is_file():
            raise ValueError(f"not a tagged Cargo target directory: {path}")
        with tag.open("rb") as source:
            if source.read(len(TAG)) != TAG:
                raise ValueError(f"unrecognized cache tag: {path}")
    return path


def directory_bytes(path: Path) -> int:
    if not path.exists():
        return 0
    result = subprocess.run(
        ["du", "-s", "-k", "-x", str(path)], check=True,
        capture_output=True, text=True, timeout=300,
    )
    # A diagnostic makes this measurement incomplete, even if du exits zero.
    if result.stderr.strip():
        raise ValueError(f"incomplete measurement: {path}")
    count = int(result.stdout.split(maxsplit=1)[0])
    if count < 0:
        raise ValueError(f"invalid measurement: {path}")
    return count * 1024


def debug_scopes(target: Path) -> list[Path]:
    """Recognize Cargo host, cross-target and trybuild debug layouts only."""
    scopes = [target / "debug", target / "tests/trybuild/debug"]
    if target.exists():
        for child in target.iterdir():
            if child.name in {"debug", "release", "tests"}:
                continue
            if child.is_symlink():
                raise ValueError(f"unclassified target symlink: {child}")
            if child.is_dir():
                scopes.append(child / "debug")
    trybuild = target / "tests/trybuild"
    if trybuild.exists():
        if trybuild.is_symlink() or not trybuild.is_dir():
            raise ValueError(f"unsupported trybuild layout: {trybuild}")
        for child in trybuild.iterdir():
            if child.name in {"debug", "release"}:
                continue
            if child.is_symlink():
                raise ValueError(f"unclassified trybuild symlink: {child}")
            if child.is_dir():
                scopes.append(child / "debug")
    selected = []
    for scope in sorted(set(scopes)):
        if scope.is_symlink() or scope.resolve() != scope:
            raise ValueError(f"symlinked debug scope is not measured: {scope}")
        if scope.exists():
            if not scope.is_dir():
                raise ValueError(f"debug scope is not a directory: {scope}")
            selected.append(scope)
    return selected


def audit(targets: list[Path], max_debug_gib: float, min_free_gib: float) -> dict:
    resolved = sorted({validate_target(path) for path in targets})
    for index, path in enumerate(resolved):
        if any(path.is_relative_to(parent) for parent in resolved[:index]):
            raise ValueError("overlapping target roots would double-count usage")
    results = []
    volumes = {}
    warnings = []
    for target in resolved:
        parent = existing_parent(target)
        device = str(parent.stat().st_dev)
        if device not in volumes:
            free = shutil.disk_usage(parent).free
            volumes[device] = {"probe": str(parent), "free_bytes": free}
            if free < min_free_gib * GIB:
                warnings.append({"reason": "free_space_below_reserve", "device": device})
        scopes = [{"path": str(path), "allocated_bytes": directory_bytes(path)}
                  for path in debug_scopes(target)]
        total = sum(scope["allocated_bytes"] for scope in scopes)
        if total > max_debug_gib * GIB:
            warnings.append({"reason": "debug_cache_over_budget", "target": str(target)})
        results.append({"target": str(target), "exists": target.exists(),
                        "debug_allocated_bytes": total, "scopes": scopes})
    return {
        "schema": "build_cache_audit_v1", "read_only": True,
        "cleanup_authorized": False, "status": "review_needed" if warnings else "within_budget",
        "max_debug_gib_per_target": max_debug_gib, "min_free_gib": min_free_gib,
        "targets": results, "volumes": volumes, "warnings": warnings,
        "limits": "Explicit roots only; non-atomic snapshot. du totals can include shared APFS blocks. "
                  "No release, source, authored-state, process-use or rollback eligibility assessment.",
    }


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--target-dir", type=Path, action="append",
                        help="exact Cargo target; repeat for multiple roots (no recursive discovery)")
    parser.add_argument("--max-debug-gib", type=positive_gib, default=40.0)
    parser.add_argument("--min-free-gib", type=positive_gib, default=50.0)
    parser.add_argument("--check", action="store_true", help="exit 2 when budgets need review")
    args = parser.parse_args(argv)
    try:
        report = audit(args.target_dir or list(DEFAULT_TARGETS),
                       args.max_debug_gib, args.min_free_gib)
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        print(json.dumps({"schema": "build_cache_audit_v1", "status": "incomplete",
                          "read_only": True, "cleanup_authorized": False,
                          "error": str(error)}))
        return 3
    print(json.dumps(report, indent=2))
    return 2 if args.check and report["warnings"] else 0


if __name__ == "__main__":
    raise SystemExit(main())
