#!/usr/bin/env python3
"""Refuse to build/deploy a live stack component from a changing tree.

Multiple interactive agents and the durable steward loop can share this checkout
and feed one live binary. The capture point is the build from a dirty tree, not
the kickstart (`kickstart -k` only restarts an existing binary). This gate does
not assign a privileged committer or deploy actor; it checks source stability and
requires explicit acknowledgement whenever uncommitted binary inputs are folded
into a build.

This is the guard `build_bridge.sh` runs before that build. Two checks:
  1. RECENT INPUT EDIT (abort) — a selected component input changed <window_s ago.
     This is a source-stability heuristic, not evidence of another actor. Exit 3.
     The installer may supply exact, owned installation evidence in-process;
     no new CLI exemption is added.
  2. DIRTY COMPONENT SOURCE (refuse unless --ack) — uncommitted component build
     inputs and local path dependencies would be folded into the binary or
     service. Refuse (exit 2) and list them unless --ack "<reason>" makes folding
     them in an explicit, logged decision (exit 0).

Clean inputs -> exit 0. Unrelated edits do not delay a component deployment.
Git/status failures refuse inspection rather than masquerading as clean inputs.

Usage:
  deploy_preflight.py [--component bridge|minime|model] [--repo PATH]
                      [--ack "reason"] [--window-s S] [--json]
  deploy_preflight.py --self-test
Exit: 0 = input preflight passed; 2 = dirty inputs, no --ack;
      3 = recent input activity or failed source inspection.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import stat
import subprocess
import sys
import time
import unittest
from pathlib import Path

SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

# Keep the controller's broader activity detector unchanged. Deployment uses
# the selected component scope instead, and retains these public defaults.
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
    "capsules/spectral-bridge/build.rs",
    "capsules/spectral-bridge/.cargo",
    "capsules/.cargo",
    "capsules/shared/managed_dir.rs",
    "crates/astrid-capabilities",
    "crates/astrid-core",
    "crates/astrid-crypto",
    "crates/astrid-storage",
    "crates/astrid-minime-protocol",
    "crates/astrid-source-study",
    "crates/astrid-public-web",
    "Cargo.toml",
    "Cargo.lock",
    ".cargo",
    "rust-toolchain",
    "rust-toolchain.toml",
)
BRIDGE_LOCAL_PATH_DEPS = (
    Path("/Users/v/other/RASCII"),
    Path("/Users/v/other/prime_esn_wasm"),
)
COMPONENTS = {
    "bridge": {
        "repo": ASTRID_ROOT,
        "build_paths": BRIDGE_BUILD_PATHS,
        "external_roots": BRIDGE_LOCAL_PATH_DEPS,
    },
    "minime": {
        "repo": Path("/Users/v/other/minime"),
        "activity_scope": "tree",
        "build_paths": (
            "minime/src",
            "minime/Cargo.toml",
            "minime/Cargo.lock",
        ),
        "external_roots": (),
    },
    "model": {
        "repo": Path("/Users/v/other/neural-triple-reservoir"),
        "activity_scope": "tree",
        "build_paths": (
            "coupled_astrid_server.py",
            "coupled_http_gateway.py",
            "mlx_reservoir.py",
            "launchd/com.reservoir.coupled-astrid.plist",
        ),
        "external_roots": (),
    },
    "minime-agent": {
        "repo": Path("/Users/v/other/minime"),
        "build_paths": (
            ":(top,glob)*.py", "minime_autonomy", "mikemind",
            "scripts/launchd_autonomous_agent.sh",
            "scripts/minime_rescue_investigation.py",
            "launchd/com.minime.autonomous-agent.plist",
            "launchd/autonomous-agent.env",
        ),
        "external_roots": (),
    },
}


def _porcelain_entries(repo: Path, paths: list[str]) -> list[tuple[str, str]]:
    try:
        out = subprocess.run(
            ["git", "-C", str(repo), "status", "--porcelain=v1", "-z",
             "--untracked-files=all", "--", *paths],
            capture_output=True, text=True, timeout=10, check=True,
        ).stdout
    except (OSError, subprocess.SubprocessError) as error:
        raise RuntimeError(f"cannot inspect component Git status: {repo}") from error
    if out and not out.endswith("\0"):
        raise RuntimeError("incomplete component Git status")
    entries = iter(out.split("\0")[:-1])
    result = []
    for entry in entries:
        if len(entry) < 4 or entry[2] != " ":
            raise RuntimeError("malformed component Git status")
        code, path = entry[:2], entry[3:]
        result.append((code, path))
        if "R" in code or "C" in code:
            # In -z format the destination comes first, then a bare source path.
            original = next(entries, None)
            if not original:
                raise RuntimeError("incomplete component rename status")
            result.append((code, original))
    return result


def _porcelain_paths(repo: Path, paths: list[str]) -> list[str]:
    return [path for _, path in _porcelain_entries(repo, paths)]


def _file_stamp(value):
    return (value.st_dev, value.st_ino, value.st_size, value.st_mtime_ns,
            value.st_ctime_ns, value.st_mode)


def reviewed_source_signatures(repo: Path, inputs: dict[str, str]) -> dict:
    """Pin exact installed regular files; no symlinks, modes or edits can drift."""
    root = repo.resolve(strict=True)
    signatures = {}
    for name, expected in inputs.items():
        relative = Path(name)
        if relative.is_absolute() or ".." in relative.parts or not relative.parts:
            raise RuntimeError("invalid reviewed source path")
        path = root / relative
        if path.resolve(strict=True) != path:
            raise RuntimeError(f"reviewed source is not a canonical file: {name}")
        fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
        with os.fdopen(fd, "rb") as handle:
            before = os.fstat(handle.fileno())
            if not stat.S_ISREG(before.st_mode):
                raise RuntimeError(f"reviewed source is not regular: {name}")
            digest = hashlib.file_digest(handle, "sha256").hexdigest()
            stamp = _file_stamp(before)
            if (digest != expected or stamp != _file_stamp(os.fstat(handle.fileno()))
                    or stamp != _file_stamp(path.lstat())):
                raise RuntimeError(f"reviewed source changed: {name}")
        signatures[name] = stamp
    return signatures


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


def _external_sources(external_roots: tuple[Path, ...]) -> list[tuple[str, Path | None, str]]:
    files = []
    for root in external_roots:
        resolved = root.resolve()
        if not resolved.exists():
            files.append((f"external-missing:{resolved}", None, ""))
            continue
        git_root = _git_root(resolved)
        if git_root is None:
            files.append((f"external-unversioned:{resolved}", None, ""))
            continue
        try:
            rel = str(resolved.relative_to(git_root))
        except ValueError:
            rel = str(resolved)
        for code, path in _porcelain_entries(git_root, [rel]):
            files.append((f"{git_root.name}:{path}", git_root / path, code))
    return files


def dirty_build_files(
    repo: Path,
    build_paths: tuple[str, ...],
    external_roots: tuple[Path, ...],
) -> list[str]:
    """Return uncommitted files that a component build or reload would consume."""
    files = _porcelain_paths(repo, list(build_paths))
    files.extend(label for label, _, _ in _external_sources(external_roots))
    return sorted(set(files))


def dirty_bridge_files(
    repo: Path,
    external_roots: tuple[Path, ...] = BRIDGE_LOCAL_PATH_DEPS,
) -> list[str]:
    """Compatibility wrapper for bridge-specific callers and tests."""
    return dirty_build_files(repo, BRIDGE_BUILD_PATHS, external_roots)


def preflight(
    repo: Path,
    *,
    ack: str | None = None,
    now: float | None = None,
    window_s: float = FOREIGN_ACTIVITY_WINDOW_S,
    codex_state_dir: Path = CODEX_STATE_DIR_DEFAULT,
    build_paths: tuple[str, ...] = BRIDGE_BUILD_PATHS,
    external_roots: tuple[Path, ...] = BRIDGE_LOCAL_PATH_DEPS,
    reviewed_inputs: dict[str, str] | None = None,
    activity_scope: str = "component",
) -> dict:
    """Decide whether it's safe to build+deploy the bridge from `repo`.
    Returns {ok, exit_code, reason, dirty_files, foreign, ack}. Pure (caller acts)."""
    now = time.time() if now is None else now
    # codex_state_dir remains API-compatible; agent liveness never established
    # ownership of an edit. An owned installer supplies reviewed_inputs only
    # while its complete source inventory and launch hold still match.
    foreign = {"active": False, "tree_activity_age_s": None, "window_s": window_s,
               "agent_guess": None, "scope": "component_inputs", "recent_files": [],
               "reviewed_files": []}
    dirty = []
    try:
        if activity_scope not in {"component", "tree"} or (
                activity_scope == "tree" and reviewed_inputs is not None):
            raise ValueError("unsupported source activity scope")
        sources = [(name, repo / name, code) for code, name in
                   _porcelain_entries(repo, list(build_paths))]
        sources.extend(_external_sources(external_roots))
        dirty = sorted({name for name, _, _ in sources})
        if reviewed_inputs is not None:
            reviewed_source_signatures(repo, reviewed_inputs)
        ages = []
        for name, path, code in sources:
            if path is None:
                continue  # Preserve explicit acknowledgement of unversioned dependencies.
            if reviewed_inputs is not None and name in reviewed_inputs and path == repo / name:
                foreign["reviewed_files"].append(name)
                continue
            try:
                modified = path.lstat().st_mtime
            except FileNotFoundError:
                if not any(flag in code for flag in "DRC"):
                    raise RuntimeError(f"component input disappeared: {name}")
                # Deleted/renamed source has no file mtime. Its nearest surviving
                # directory gives a conservative edit-age hint, not actor identity.
                parent = path.parent
                while not parent.exists():
                    parent = parent.parent
                modified = parent.stat().st_mtime
            age = now - modified
            ages.append(age)
            if age <= window_s:
                foreign["recent_files"].append(name)
        foreign["tree_activity_age_s"] = min(ages) if ages else None
        foreign["active"] = bool(foreign["recent_files"])
        if activity_scope == "tree":
            # Provider/engine dependency-scope qualification is separate. Keep
            # their legacy activity policy until their input closure is reviewed.
            foreign = {**foreign_activity(repo, codex_state_dir, window_s, now),
                       "scope": "legacy_tree", "agent_guess": None,
                       "recent_files": [], "reviewed_files": []}
    except (OSError, RuntimeError, ValueError) as error:
        return {"ok": False, "exit_code": 3, "reason": "source_inspection_failed",
                "dirty_files": dirty, "foreign": foreign, "ack": ack, "error": str(error)}
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
    if reason == "source_inspection_failed":
        return f"REFUSE - component source inspection failed: {result['error']}"
    if reason == "foreign_active":
        fa = result["foreign"]
        scope = ("working tree (legacy engine/provider scope)" if fa["scope"] == "legacy_tree"
                 else "selected component inputs")
        return (f"REFUSE - {scope} changed recently "
                f"({fa.get('tree_activity_age_s')}s ago): {', '.join(fa['recent_files'])}. "
                "Actor unknown; finish/review those edits before retrying.")
    if reason == "dirty_no_ack":
        lines = [f"    {f}" for f in result["dirty_files"]]
        return ("✗ REFUSE — binary source/dependencies have uncommitted or unversioned "
                "changes a release build would fold into "
                "the LIVE binary:\n" + "\n".join(lines) +
                "\n  Commit your own work first, or pass --ack \"<reason>\" to consciously fold these in.")
    if reason == "dirty_acked":
        lines = ", ".join(result["dirty_files"])
        return (f"✓ OK (ACKED) — folding in uncommitted component source: {lines}\n"
                f"  reason: {result['ack']}")
    return "OK - selected component inputs are clean; other deployment checks still apply."


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--component", choices=sorted(COMPONENTS), default="bridge")
    ap.add_argument("--repo", default=None)
    ap.add_argument("--ack", default=None,
                    help="explicit reason to fold in uncommitted component source")
    ap.add_argument("--window-s", type=float, default=FOREIGN_ACTIVITY_WINDOW_S)
    ap.add_argument("--json", action="store_true")
    ap.add_argument("--self-test", action="store_true")
    args = ap.parse_args(argv)

    if args.self_test:
        return _run_self_test()

    component = COMPONENTS[args.component]
    repo = Path(args.repo) if args.repo else component["repo"]
    result = preflight(
        repo,
        ack=args.ack,
        window_s=args.window_s,
        build_paths=component["build_paths"],
        external_roots=component["external_roots"],
        activity_scope=component.get("activity_scope", "component"),
    )
    result["component"] = args.component
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

    def test_dirty_shared_web_transport_requires_ack(self):
        import os
        import tempfile
        with tempfile.TemporaryDirectory() as tmp:
            repo = self._repo(tmp)
            path = repo / "crates/astrid-public-web/src/lib.rs"
            path.parent.mkdir(parents=True)
            path.write_text("// synthetic transport baseline\n")
            self._git(repo, "add", "crates/astrid-public-web/src/lib.rs")
            self._git(repo, "commit", "-qm", "transport baseline")
            path.write_text("// synthetic transport change\n")
            os.utime(path, (time.time() - 9999, time.time() - 9999))
            result = preflight(repo, codex_state_dir=Path(tmp) / "no_codex", external_roots=())
            self.assertEqual(result["reason"], "dirty_no_ack")
            self.assertTrue(any(p.startswith("crates/astrid-public-web") for p in result["dirty_files"]))

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
        import tempfile
        with tempfile.TemporaryDirectory() as tmp:
            repo = self._repo(tmp)
            doc = repo / "CHANGELOG.md"  # not compiled into the binary
            doc.write_text("dirty doc\n")
            r = preflight(repo, codex_state_dir=Path(tmp) / "no_codex", external_roots=())
            self.assertEqual((r["ok"], r["reason"]), (True, "clean"))

    def test_dirty_external_path_dependency_refused_without_ack(self):
        import os
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

            recent = preflight(repo, ack="review dependency", external_roots=(dep,))
            self.assertEqual(recent["reason"], "foreign_active")
            self.assertEqual(recent["foreign"]["recent_files"], ["RASCII:src/lib.rs"])
            os.utime(src / "lib.rs", (time.time() - 9999, time.time() - 9999))

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

    def test_model_preflight_includes_imported_logit_processor(self):
        import tempfile
        with tempfile.TemporaryDirectory() as tmp:
            repo = self._repo(tmp)
            (repo / "mlx_reservoir.py").write_text("# changed processor\n")
            files = dirty_build_files(repo, COMPONENTS["model"]["build_paths"], ())
            self.assertEqual(files, ["mlx_reservoir.py"])

    def test_agent_preflight_includes_python_imports_not_engine_or_tests(self):
        import tempfile
        with tempfile.TemporaryDirectory() as tmp:
            repo = self._repo(tmp)
            (repo / "minime_autonomy").mkdir()
            (repo / "tests").mkdir()
            (repo / "autonomous_agent.py").write_text("# facade\n")
            (repo / "minime_autonomy/journal_context.py").write_text("# helper\n")
            (repo / "tests/test_example.py").write_text("# fixture\n")
            files = dirty_build_files(repo, COMPONENTS["minime-agent"]["build_paths"], ())
            self.assertEqual(files, ["autonomous_agent.py", "minime_autonomy/journal_context.py"])

    def test_component_build_paths_are_scoped_independently(self):
        import os
        import tempfile
        with tempfile.TemporaryDirectory() as tmp:
            repo = self._repo(tmp)
            component = repo / "minime" / "src"
            component.mkdir(parents=True)
            source = component / "main.rs"
            source.write_text("fn main() {}\n")
            self._git(repo, "add", "minime/src/main.rs")
            self._git(repo, "commit", "-qm", "add minime")
            source.write_text('fn main() { println!("changed"); }\n')
            os.utime(source, (time.time() - 9999, time.time() - 9999))

            result = preflight(
                repo,
                build_paths=("minime/src",),
                external_roots=(),
                codex_state_dir=Path(tmp) / "no_codex",
            )

            self.assertEqual(result["reason"], "dirty_no_ack")
            self.assertEqual(result["dirty_files"], ["minime/src/main.rs"])


def _run_self_test() -> int:
    suite = unittest.TestLoader().loadTestsFromTestCase(DeployPreflightTests)
    result = unittest.TextTestRunner(verbosity=2).run(suite)
    return 0 if result.wasSuccessful() else 1


if __name__ == "__main__":
    raise SystemExit(main())
