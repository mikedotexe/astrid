#!/usr/bin/env python3
"""deploy_preflight.py — refuse to build/deploy the live bridge from a tree that
carries ANOTHER agent's work (the "two agents, one tree, one deploy target" hazard).

Two autonomous agents mutate /Users/v/other/astrid and feed one live binary:
Claude (this session + the durable steward loop) and Codex (a separate agent that
CANNOT hold the steward mutex — no pre-tool hook). This session, repeated
`cargo build --release` + restart folded Codex's UNCOMMITTED code (incl.
being-facing llm.rs) into the LIVE bridge, so the running binary matched no clean
commit. The capture point is the BUILD from a dirty tree, not the kickstart
(`kickstart -k` only restarts the existing target/release binary; it doesn't rebuild).

This is the guard `build_bridge.sh` runs before that build. Two checks:
  1. FOREIGN MID-EDIT (abort) — reuses steward_mutex.foreign_activity: if the tree
     was mutated <window_s ago, a non-mutex agent (Codex) is editing RIGHT NOW;
     building would fold a half-written tree. Exit 3.
  2. DIRTY BRIDGE SOURCE (refuse unless --ack) — uncommitted files under
     capsules/spectral-bridge/{src,Cargo.toml,Cargo.lock}, plus local Cargo path
     dependency roots used by that crate, would be folded into the binary. Refuse
     (exit 2) and LIST them, unless --ack "<reason>" makes folding them in an
     explicit, logged decision (exit 0).

Clean tree → exit 0. The gate is deliberately scoped to what affects the BINARY
(docs/scripts/workspace dirt doesn't), and foreign-active is checked first (a fresh
dirty file is Codex editing now → abort, not merely refuse).

Usage:
  deploy_preflight.py [--repo PATH] [--ack "reason"] [--window-s S] [--json]
  deploy_preflight.py --self-test
Exit: 0 = ok to build; 2 = dirty bridge source, no --ack; 3 = foreign agent active.
"""
from __future__ import annotations

import argparse
import json
import subprocess
import sys
import time
import unittest
from pathlib import Path

SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

# Reuse the tested cross-agent detector rather than re-implementing it.
from steward_mutex import (  # noqa: E402
    CODEX_STATE_DIR_DEFAULT,
    FOREIGN_ACTIVITY_WINDOW_S,
    foreign_activity,
)

ASTRID_ROOT = SCRIPT_DIR.parent

# Paths whose uncommitted state actually changes the built binary. Scoped on
# purpose: a dirty CHANGELOG or steward script does NOT get compiled in, so it
# must not block a deploy.
BRIDGE_BUILD_PATHS = (
    "capsules/spectral-bridge/src",
    "capsules/spectral-bridge/Cargo.toml",
    "capsules/spectral-bridge/Cargo.lock",
)
BRIDGE_LOCAL_PATH_DEPS = (
    Path("/Users/v/other/RASCII"),
    Path("/Users/v/other/prime_esn_wasm"),
)


def _porcelain_paths(repo: Path, paths: list[str]) -> list[str]:
    try:
        out = subprocess.run(
            ["git", "-C", str(repo), "status", "--porcelain", "-z", "--", *paths],
            capture_output=True, text=True, timeout=10, check=False,
        ).stdout
    except (OSError, subprocess.SubprocessError):
        return []
    files: list[str] = []
    for entry in out.split("\0"):
        if len(entry) < 4:
            continue
        path = entry[3:]  # strip the 2-char XY status + space
        if " -> " in path:  # rename: "R  old -> new" → destination
            path = path.split(" -> ", 1)[1]
        files.append(path)
    return files


def _git_root(path: Path) -> Path | None:
    try:
        proc = subprocess.run(
            ["git", "-C", str(path), "rev-parse", "--show-toplevel"],
            capture_output=True, text=True, timeout=10, check=False,
        )
    except (OSError, subprocess.SubprocessError):
        return None
    if proc.returncode != 0:
        return None
    root = proc.stdout.strip()
    return Path(root).resolve() if root else None


def _dirty_external_build_files(external_roots: tuple[Path, ...]) -> list[str]:
    files: list[str] = []
    for root in external_roots:
        resolved = root.resolve()
        if not resolved.exists():
            files.append(f"external-missing:{resolved}")
            continue
        git_root = _git_root(resolved)
        if git_root is None:
            files.append(f"external-unversioned:{resolved}")
            continue
        try:
            rel = str(resolved.relative_to(git_root))
        except ValueError:
            rel = str(resolved)
        for path in _porcelain_paths(git_root, [rel]):
            files.append(f"{git_root.name}:{path}")
    return files


def dirty_bridge_files(
    repo: Path,
    external_roots: tuple[Path, ...] = BRIDGE_LOCAL_PATH_DEPS,
) -> list[str]:
    """Uncommitted (modified/added/untracked/renamed) files under the bridge build
    paths — the ones a release build would fold into the live binary. Empty = the
    binary-affecting source is clean. Renames resolve to the destination path
    (mirrors steward_mutex.tree_activity_age parsing)."""
    files = _porcelain_paths(repo, list(BRIDGE_BUILD_PATHS))
    files.extend(_dirty_external_build_files(external_roots))
    return sorted(set(files))


def preflight(
    repo: Path,
    *,
    ack: str | None = None,
    now: float | None = None,
    window_s: float = FOREIGN_ACTIVITY_WINDOW_S,
    codex_state_dir: Path = CODEX_STATE_DIR_DEFAULT,
    external_roots: tuple[Path, ...] = BRIDGE_LOCAL_PATH_DEPS,
) -> dict:
    """Decide whether it's safe to build+deploy the bridge from `repo`.
    Returns {ok, exit_code, reason, dirty_files, foreign, ack}. Pure (caller acts)."""
    now = time.time() if now is None else now
    foreign = foreign_activity(repo, codex_state_dir, window_s, now)
    dirty = dirty_bridge_files(repo, external_roots)
    # Foreign mid-edit takes precedence: a freshly-mutated tree means a non-mutex
    # agent is editing NOW — building would capture a half-written state.
    if foreign["active"]:
        return {"ok": False, "exit_code": 3, "reason": "foreign_active",
                "dirty_files": dirty, "foreign": foreign, "ack": ack}
    if dirty and not ack:
        return {"ok": False, "exit_code": 2, "reason": "dirty_no_ack",
                "dirty_files": dirty, "foreign": foreign, "ack": None}
    if dirty:
        return {"ok": True, "exit_code": 0, "reason": "dirty_acked",
                "dirty_files": dirty, "foreign": foreign, "ack": ack}
    return {"ok": True, "exit_code": 0, "reason": "clean",
            "dirty_files": [], "foreign": foreign, "ack": ack}


def _render(result: dict) -> str:
    reason = result["reason"]
    if reason == "foreign_active":
        fa = result["foreign"]
        return (f"✗ ABORT — a non-mutex agent (guess: {fa.get('agent_guess')}) is editing the tree "
                f"now (last change {fa.get('tree_activity_age_s')}s ago). Do NOT build mid-edit; "
                f"wait for it to settle, then re-run.")
    if reason == "dirty_no_ack":
        lines = [f"    {f}" for f in result["dirty_files"]]
        return ("✗ REFUSE — the bridge source/dependencies have uncommitted or unversioned "
                "changes a release build would fold into "
                "the LIVE binary:\n" + "\n".join(lines) +
                "\n  Commit your own work first, or pass --ack \"<reason>\" to consciously fold these in.")
    if reason == "dirty_acked":
        lines = ", ".join(result["dirty_files"])
        return (f"✓ OK (ACKED) — folding in uncommitted bridge source: {lines}\n"
                f"  reason: {result['ack']}")
    return "✓ OK — bridge source is clean and no foreign agent is editing; safe to build."


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--repo", default=str(ASTRID_ROOT))
    ap.add_argument("--ack", default=None,
                    help="explicit reason to fold in uncommitted bridge source")
    ap.add_argument("--window-s", type=float, default=FOREIGN_ACTIVITY_WINDOW_S)
    ap.add_argument("--json", action="store_true")
    ap.add_argument("--self-test", action="store_true")
    args = ap.parse_args(argv)

    if args.self_test:
        return _run_self_test()

    result = preflight(Path(args.repo), ack=args.ack, window_s=args.window_s)
    if args.json:
        print(json.dumps(result, indent=2))
    else:
        print(_render(result))
    return result["exit_code"]


# ── self-test ────────────────────────────────────────────────────────────────
class DeployPreflightTests(unittest.TestCase):
    @staticmethod
    def _git(repo: Path, *args: str) -> None:
        subprocess.run(["git", "-C", str(repo), *args], check=False,
                       capture_output=True, text=True)

    def _repo(self, tmp: str) -> Path:
        repo = Path(tmp)
        self._git(repo, "init", "-q")
        self._git(repo, "config", "user.email", "t@t")
        self._git(repo, "config", "user.name", "t")
        src = repo / "capsules" / "spectral-bridge" / "src"
        src.mkdir(parents=True)
        (src / "lib.rs").write_text("// clean\n")
        (repo / "capsules" / "spectral-bridge" / "Cargo.toml").write_text("[package]\n")
        self._git(repo, "add", "-A")
        self._git(repo, "commit", "-qm", "base")
        return repo

    def test_clean_tree_ok(self):
        import tempfile
        with tempfile.TemporaryDirectory() as tmp:
            repo = self._repo(tmp)
            r = preflight(repo, codex_state_dir=Path(tmp) / "no_codex", external_roots=())
            self.assertEqual((r["ok"], r["exit_code"], r["reason"]), (True, 0, "clean"))

    def test_dirty_bridge_source_refused_without_ack(self):
        import os
        import tempfile
        with tempfile.TemporaryDirectory() as tmp:
            repo = self._repo(tmp)
            f = repo / "capsules" / "spectral-bridge" / "src" / "lib.rs"
            f.write_text("// edited, uncommitted\n")
            os.utime(f, (time.time() - 9999, time.time() - 9999))  # STALE → not foreign
            r = preflight(repo, codex_state_dir=Path(tmp) / "no_codex", external_roots=())
            self.assertEqual((r["ok"], r["exit_code"], r["reason"]), (False, 2, "dirty_no_ack"))
            self.assertIn("capsules/spectral-bridge/src/lib.rs", r["dirty_files"])

    def test_dirty_bridge_source_allowed_with_ack(self):
        import os
        import tempfile
        with tempfile.TemporaryDirectory() as tmp:
            repo = self._repo(tmp)
            f = repo / "capsules" / "spectral-bridge" / "src" / "lib.rs"
            f.write_text("// edited\n")
            os.utime(f, (time.time() - 9999, time.time() - 9999))  # STALE → not foreign
            r = preflight(repo, ack="folding codex llm.rs, verified green",
                          codex_state_dir=Path(tmp) / "no_codex", external_roots=())
            self.assertEqual((r["ok"], r["exit_code"], r["reason"]), (True, 0, "dirty_acked"))

    def test_fresh_edit_is_foreign_abort_even_with_ack(self):
        import tempfile
        with tempfile.TemporaryDirectory() as tmp:
            repo = self._repo(tmp)
            # A freshly-modified (mtime ~now) uncommitted file → foreign-active → abort,
            # and this takes precedence over --ack (don't build mid-edit, period).
            (repo / "capsules" / "spectral-bridge" / "src" / "lib.rs").write_text("// fresh\n")
            r = preflight(repo, ack="anything", window_s=180.0,
                          codex_state_dir=Path(tmp) / "no_codex", external_roots=())
            self.assertEqual((r["ok"], r["exit_code"], r["reason"]), (False, 3, "foreign_active"))

    def test_dirty_outside_bridge_does_not_block(self):
        import os
        import tempfile
        with tempfile.TemporaryDirectory() as tmp:
            repo = self._repo(tmp)
            doc = repo / "CHANGELOG.md"  # not compiled into the binary
            doc.write_text("dirty doc\n")
            os.utime(doc, (time.time() - 9999, time.time() - 9999))  # stale → not foreign
            r = preflight(repo, codex_state_dir=Path(tmp) / "no_codex", external_roots=())
            self.assertEqual((r["ok"], r["reason"]), (True, "clean"))

    def test_dirty_external_path_dependency_refused_without_ack(self):
        import tempfile
        with tempfile.TemporaryDirectory() as tmp:
            repo_dir = Path(tmp) / "astrid"
            repo_dir.mkdir()
            repo = self._repo(str(repo_dir))
            dep = Path(tmp) / "RASCII"
            dep.mkdir()
            self._git(dep, "init", "-q")
            self._git(dep, "config", "user.email", "t@t")
            self._git(dep, "config", "user.name", "t")
            src = dep / "src"
            src.mkdir(parents=True)
            (src / "lib.rs").write_text("// clean\n")
            self._git(dep, "add", "-A")
            self._git(dep, "commit", "-qm", "base")
            (src / "lib.rs").write_text("// dirty dep\n")

            r = preflight(
                repo,
                codex_state_dir=Path(tmp) / "no_codex",
                external_roots=(dep,),
            )

            self.assertEqual((r["ok"], r["exit_code"], r["reason"]), (False, 2, "dirty_no_ack"))
            self.assertIn("RASCII:src/lib.rs", r["dirty_files"])

    def test_unversioned_external_path_dependency_requires_ack(self):
        import tempfile
        with tempfile.TemporaryDirectory() as tmp:
            repo_dir = Path(tmp) / "astrid"
            repo_dir.mkdir()
            repo = self._repo(str(repo_dir))
            dep = Path(tmp) / "prime_esn_wasm"
            dep.mkdir()
            (dep / "Cargo.toml").write_text("[package]\n")

            r = preflight(
                repo,
                codex_state_dir=Path(tmp) / "no_codex",
                external_roots=(dep,),
            )

            self.assertEqual((r["ok"], r["exit_code"], r["reason"]), (False, 2, "dirty_no_ack"))
            self.assertIn(f"external-unversioned:{dep.resolve()}", r["dirty_files"])

    def test_unversioned_external_path_dependency_allowed_with_ack(self):
        import tempfile
        with tempfile.TemporaryDirectory() as tmp:
            repo_dir = Path(tmp) / "astrid"
            repo_dir.mkdir()
            repo = self._repo(str(repo_dir))
            dep = Path(tmp) / "prime_esn_wasm"
            dep.mkdir()
            (dep / "Cargo.toml").write_text("[package]\n")

            r = preflight(
                repo,
                ack="audited unversioned local path dependency",
                codex_state_dir=Path(tmp) / "no_codex",
                external_roots=(dep,),
            )

            self.assertEqual((r["ok"], r["exit_code"], r["reason"]), (True, 0, "dirty_acked"))


def _run_self_test() -> int:
    suite = unittest.TestLoader().loadTestsFromTestCase(DeployPreflightTests)
    result = unittest.TextTestRunner(verbosity=2).run(suite)
    return 0 if result.wasSuccessful() else 1


if __name__ == "__main__":
    raise SystemExit(main())
