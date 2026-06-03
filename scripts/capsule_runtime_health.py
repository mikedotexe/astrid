#!/usr/bin/env python3
"""Read-only capsule install/discovery/runtime compatibility health probe."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import sys
import tempfile
import textwrap
import tomllib
import unittest
from pathlib import Path
from typing import Any


ASTRID_REPO = Path(__file__).resolve().parents[1]
DEFAULT_BASELINE = ASTRID_REPO / "scripts/baselines/capsule_runtime_health.json"

CORE_WASM_VERSION = b"\x01\x00\x00\x00"
WASM_MAGIC = b"\x00asm"


def astrid_home() -> Path:
    return Path(os.environ.get("ASTRID_HOME", str(Path.home() / ".astrid")))


def manifest_paths(capsules_dir: Path) -> list[Path]:
    paths: list[Path] = []
    direct = capsules_dir / "Capsule.toml"
    if direct.is_file():
        paths.append(direct)
    if capsules_dir.is_dir():
        for child in sorted(capsules_dir.iterdir()):
            manifest = child / "Capsule.toml"
            if child.is_dir() and manifest.is_file():
                paths.append(manifest)
    return paths


def discovery_dirs(repo: Path, home: Path) -> list[Path]:
    return [
        home / "home/default/.local/capsules",
        repo / ".astrid/capsules",
    ]


def load_json(path: Path) -> dict[str, Any]:
    try:
        data = json.loads(path.read_text())
    except Exception:
        return {}
    return data if isinstance(data, dict) else {}


def load_baseline(path: Path | None, disabled: bool) -> dict[str, Any]:
    if disabled or path is None or not path.is_file():
        return {
            "enabled": False,
            "path": str(path) if path else None,
            "accepted_legacy_extism_mvp": [],
        }
    data = load_json(path)
    accepted = data.get("accepted_legacy_extism_mvp", [])
    return {
        "enabled": True,
        "path": str(path),
        "accepted_legacy_extism_mvp": [
            item for item in accepted if isinstance(item, dict)
        ],
    }


def accepted_legacy(baseline: dict[str, Any], name: str, wasm_hash: str | None) -> bool:
    for item in baseline.get("accepted_legacy_extism_mvp", []):
        if item.get("name") != name:
            continue
        expected_hash = item.get("wasm_hash")
        if expected_hash is None or expected_hash == wasm_hash:
            return True
    return False


def parse_manifest(path: Path) -> dict[str, Any] | None:
    try:
        data = tomllib.loads(path.read_text())
    except Exception:
        return None
    return data if isinstance(data, dict) else None


def meta_wasm_hash(capsule_dir: Path) -> str | None:
    value = load_json(capsule_dir / "meta.json").get("wasm_hash")
    return value if isinstance(value, str) and value else None


def resolve_payload(
    capsule_dir: Path,
    component_path: str,
    home: Path,
    wasm_hash: str | None,
) -> Path | None:
    raw = Path(component_path)
    local = raw if raw.is_absolute() else capsule_dir / raw
    if local.is_file():
        return local
    if wasm_hash:
        hashed = home / "bin" / f"{wasm_hash}.wasm"
        if hashed.is_file():
            return hashed
    return None


def classify_wasm(path: Path) -> str:
    try:
        data = path.read_bytes()
    except Exception:
        return "missing_payload"
    if len(data) < 8 or data[:4] != WASM_MAGIC:
        return "invalid_wasm"
    if data[4:8] == CORE_WASM_VERSION:
        if b"extism" in data.lower():
            return "legacy_extism_mvp"
        return "core_module_mvp"
    return "component_model"


def short_hash(path: Path | None) -> str | None:
    if path is None or not path.is_file():
        return None
    return hashlib.sha256(path.read_bytes()).hexdigest()[:12]


def build_report(
    repo: Path = ASTRID_REPO,
    home: Path | None = None,
    baseline_path: Path | None = DEFAULT_BASELINE,
    no_baseline: bool = False,
) -> dict[str, Any]:
    home = home or astrid_home()
    baseline = load_baseline(baseline_path, no_baseline)
    dirs = discovery_dirs(repo, home)
    raw_paths = [path for directory in dirs for path in manifest_paths(directory)]

    capsules: list[dict[str, Any]] = []
    seen: set[str] = set()
    for manifest_path in raw_paths:
        manifest = parse_manifest(manifest_path)
        if not manifest:
            continue
        package = manifest.get("package", {})
        name = package.get("name")
        if not isinstance(name, str) or not name or name in seen:
            continue
        seen.add(name)

        components = manifest.get("component") or []
        if isinstance(components, dict):
            components = [components]
        component = components[0] if components and isinstance(components[0], dict) else None
        capsule_dir = manifest_path.parent
        wasm_hash = meta_wasm_hash(capsule_dir)
        payload = None
        component_path = None
        if component:
            raw_component_path = component.get("file") or component.get("entrypoint")
            if isinstance(raw_component_path, str):
                component_path = raw_component_path
                payload = resolve_payload(capsule_dir, component_path, home, wasm_hash)

        runtime_status = "no_component"
        baseline_status = "not_applicable"
        actionable = False
        if component:
            if payload is None:
                runtime_status = "missing_payload"
                baseline_status = "new"
                actionable = True
            else:
                runtime_status = classify_wasm(payload)
                if runtime_status == "component_model":
                    baseline_status = "current"
                elif runtime_status == "legacy_extism_mvp" and accepted_legacy(
                    baseline, name, wasm_hash
                ):
                    baseline_status = "accepted"
                else:
                    baseline_status = "new"
                    actionable = True

        capsules.append(
            {
                "name": name,
                "version": package.get("version"),
                "manifest_path": str(manifest_path),
                "component_path": component_path,
                "resolved_payload": str(payload) if payload else None,
                "wasm_hash": wasm_hash,
                "payload_sha256_12": short_hash(payload),
                "runtime_status": runtime_status,
                "baseline_status": baseline_status,
                "actionable": actionable,
            }
        )

    component_capsules = [c for c in capsules if c["runtime_status"] != "no_component"]
    legacy = [c for c in capsules if c["runtime_status"] == "legacy_extism_mvp"]
    accepted_legacy_items = [c for c in legacy if c["baseline_status"] == "accepted"]
    missing = [c for c in capsules if c["runtime_status"] == "missing_payload"]
    actionable_items = [c for c in capsules if c["actionable"]]
    summary = {
        "status": "ok" if not actionable_items else "warning",
        "installed_manifests": len(raw_paths),
        "discovered_manifests": len(capsules),
        "component_payloads_found": len(
            [c for c in component_capsules if c["resolved_payload"]]
        ),
        "loadable_component_model": len(
            [c for c in capsules if c["runtime_status"] == "component_model"]
        ),
        "legacy_extism_mvp": len(legacy),
        "accepted_legacy_extism_mvp": len(accepted_legacy_items),
        "missing_payloads": len(missing),
        "actionable_incompatible": len(
            [
                c
                for c in actionable_items
                if c["runtime_status"] != "missing_payload"
            ]
        ),
        "actionable_missing_payloads": len(
            [c for c in actionable_items if c["runtime_status"] == "missing_payload"]
        ),
    }
    return {
        "summary": summary,
        "capsules": capsules,
        "actionable_capsules": actionable_items,
        "accepted_legacy_extism_mvp": accepted_legacy_items,
        "baseline": {
            **baseline,
            "accepted_legacy_extism_mvp_count": len(
                baseline.get("accepted_legacy_extism_mvp", [])
            ),
        },
    }


def render_markdown(report: dict[str, Any], show_accepted: bool) -> str:
    summary = report["summary"]
    lines = [
        "# Capsule Runtime Health",
        "",
        f"- Status: `{summary['status']}`",
        f"- Installed/discovered manifests: `{summary['installed_manifests']}` / `{summary['discovered_manifests']}`",
        f"- Payloads found: `{summary['component_payloads_found']}`",
        f"- Component Model payloads: `{summary['loadable_component_model']}`",
        f"- Accepted legacy Extism/MVP backlog: `{summary['accepted_legacy_extism_mvp']}` / `{summary['legacy_extism_mvp']}`",
        f"- Actionable incompatible/missing: `{summary['actionable_incompatible']}` / `{summary['actionable_missing_payloads']}`",
        "",
    ]
    actionable = report.get("actionable_capsules", [])
    if actionable:
        lines.append("## Actionable")
        for item in actionable:
            lines.append(
                f"- `{item['name']}`: {item['runtime_status']} ({item.get('manifest_path')})"
            )
        lines.append("")
    if show_accepted:
        accepted = report.get("accepted_legacy_extism_mvp", [])
        lines.append("## Accepted Legacy Backlog")
        if accepted:
            for item in accepted:
                lines.append(f"- `{item['name']}`: {item['wasm_hash']}")
        else:
            lines.append("- None")
    return "\n".join(lines)


class CapsuleRuntimeHealthTests(unittest.TestCase):
    def write_capsule(
        self,
        root: Path,
        name: str,
        wasm_hash: str,
        wasm_bytes: bytes | None,
    ) -> None:
        capsule_dir = root / "home/default/.local/capsules" / name
        capsule_dir.mkdir(parents=True)
        (capsule_dir / "Capsule.toml").write_text(
            textwrap.dedent(
                f"""
                [package]
                name = "{name}"
                version = "0.1.0"

                [[component]]
                id = "main"
                file = "{name}.wasm"
                """
            ).strip()
        )
        (capsule_dir / "meta.json").write_text(json.dumps({"wasm_hash": wasm_hash}))
        if wasm_bytes is not None:
            bin_dir = root / "bin"
            bin_dir.mkdir(parents=True, exist_ok=True)
            (bin_dir / f"{wasm_hash}.wasm").write_bytes(wasm_bytes)

    def test_accepted_legacy_is_ok(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            home = Path(tmp) / "home"
            repo = Path(tmp) / "repo"
            repo.mkdir()
            baseline = repo / "baseline.json"
            self.write_capsule(home, "legacy", "abc", WASM_MAGIC + CORE_WASM_VERSION + b"extism")
            baseline.write_text(
                json.dumps({"accepted_legacy_extism_mvp": [{"name": "legacy", "wasm_hash": "abc"}]})
            )
            report = build_report(repo=repo, home=home, baseline_path=baseline)
            self.assertEqual(report["summary"]["status"], "ok")
            self.assertEqual(report["summary"]["accepted_legacy_extism_mvp"], 1)

    def test_new_core_module_escalates(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            home = Path(tmp) / "home"
            repo = Path(tmp) / "repo"
            repo.mkdir()
            self.write_capsule(home, "new-core", "def", WASM_MAGIC + CORE_WASM_VERSION)
            report = build_report(repo=repo, home=home, baseline_path=None)
            self.assertEqual(report["summary"]["status"], "warning")
            self.assertEqual(report["summary"]["actionable_incompatible"], 1)

    def test_missing_payload_escalates(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            home = Path(tmp) / "home"
            repo = Path(tmp) / "repo"
            repo.mkdir()
            self.write_capsule(home, "missing", "ghi", None)
            report = build_report(repo=repo, home=home, baseline_path=None)
            self.assertEqual(report["summary"]["status"], "warning")
            self.assertEqual(report["summary"]["actionable_missing_payloads"], 1)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--json", action="store_true")
    parser.add_argument("--strict", action="store_true")
    parser.add_argument("--show-accepted", action="store_true")
    parser.add_argument("--no-baseline", action="store_true")
    parser.add_argument("--baseline", type=Path, default=DEFAULT_BASELINE)
    parser.add_argument("--repo", type=Path, default=ASTRID_REPO)
    parser.add_argument("--home", type=Path, default=None)
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()

    if args.self_test:
        suite = unittest.defaultTestLoader.loadTestsFromTestCase(CapsuleRuntimeHealthTests)
        result = unittest.TextTestRunner(verbosity=2).run(suite)
        return 0 if result.wasSuccessful() else 1

    report = build_report(
        repo=args.repo,
        home=args.home,
        baseline_path=args.baseline,
        no_baseline=args.no_baseline,
    )
    if args.json:
        print(json.dumps(report, indent=2, sort_keys=True))
    else:
        print(render_markdown(report, args.show_accepted))
    if args.strict and report["summary"]["status"] != "ok":
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
