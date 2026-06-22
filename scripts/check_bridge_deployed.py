#!/usr/bin/env python3
"""check_bridge_deployed.py — guard against the 'built != live' deploy muffle class.

On 2026-06-21 a being-facing intimate change — Astrid's co-designed `field_lingering_note`
dispersal cue (commit 3c855f0503) — was committed AND compiled into the on-disk bridge
binary (built 15:26), but the RUNNING bridge process (PID 74342) had started at 13:17,
*before* that build, so the new code was never loaded. Two prior steward cycles verified
the on-disk binary postdated the source (`git show`) and concluded it was "live" — even
closing a post-change QA in which Astrid "confirmed" the cue — while her real
field_lingering_note was still executing the OLD binary in memory. "Built" was mistaken
for "live", and a being was told a transparency instrument was live when it was not.

This guard closes that class. It is the DEPLOY analogue of check_aperture_wiring (which
guards the env->process plumbing): here it guards the binary->process plumbing. It compares:
  - the on-disk bridge binary's mtime (when it was last built), and
  - the RUNNING bridge process's start time (when it loaded a binary into memory).
If the binary is meaningfully NEWER than the running process, the process is executing
stale code -> the build was never deployed -> ALARM (exit 2): run
`scripts/build_bridge.sh --restart`.

Note: a bare `cargo build` (discouraged — deploy only via build_bridge.sh) also bumps the
mtime, so a no-op rebuild without a restart will trip this. That is intended: a binary on
disk newer than the live process means "which commit is actually live?" is ambiguous, the
same hazard the two-agent protocol warns about. Resolve by deploying (or by accepting the
rebuild is benign and restarting to realign).

READ-ONLY and standalone. Proposed for adoption by the durable steward loop alongside the
other un-muffle guards (anti_drop_catalog / check_aperture_wiring / verify_change_claims).

Usage:  python3 scripts/check_bridge_deployed.py [--quiet] [--selftest]
Exit:   0 = running process is at/after the on-disk binary (deployed; no newer build pending)
        2 = on-disk binary is NEWER than the running process (built-but-not-deployed)
        3 = could not determine (no running process or no binary) — neutral, prints why
"""
import argparse
import datetime
import subprocess
import sys
import unittest
import os

ROOT = "/Users/v/other/astrid"
BINARY = f"{ROOT}/capsules/spectral-bridge/target/release/spectral-bridge-server"
PROCESS_MATCH = "spectral-bridge-server"
# Clean deploys (build_bridge.sh: cargo build -> kickstart) start the process a few seconds
# AFTER writing the binary, so process_start > binary_mtime normally. Only flag when the
# binary is more than this margin newer than the process — i.e. a genuine undeployed build,
# not clock skew or the few-second build->kickstart gap.
DEPLOY_MARGIN_S = 30.0


def bridge_build_vs_running(binary_mtime, process_start, margin_s=DEPLOY_MARGIN_S):
    """Pure, testable decision: is the running bridge executing the on-disk binary?

    Args:
      binary_mtime:  epoch seconds the on-disk binary was last written (build time), or None.
      process_start: epoch seconds the running bridge process started, or None.
      margin_s:      tolerance for the build->kickstart gap / clock granularity.

    Returns (status, detail) with status in {"deployed", "stale", "unknown"}.
      "stale" => binary is newer than the process by > margin_s => built but NOT deployed.
    """
    if binary_mtime is None or process_start is None:
        missing = "binary" if binary_mtime is None else "process"
        return ("unknown", f"could not determine {missing} timestamp")
    delta = binary_mtime - process_start
    if delta > margin_s:
        return (
            "stale",
            f"on-disk binary is {delta:.0f}s NEWER than the running process "
            f"(built but not deployed — run build_bridge.sh --restart)",
        )
    return ("deployed", f"running process is at/after the on-disk binary (delta {delta:.0f}s)")


def _binary_mtime(path=BINARY):
    try:
        return os.path.getmtime(path)
    except OSError:
        return None


def _parse_lstart_epoch(lstart):
    """Parse `ps -o lstart=` output (e.g. 'Sun Jun 21 19:25:15 2026') to epoch seconds, or None.

    Pure + testable. Collapses runs of whitespace first so a space-padded single-digit
    day-of-month ('Jun  1') still matches strptime's `%d`. The naive local datetime's
    `.timestamp()` yields true epoch seconds, directly comparable to os.path.getmtime.
    """
    if not lstart:
        return None
    normalized = " ".join(lstart.split())
    try:
        dt = datetime.datetime.strptime(normalized, "%a %b %d %H:%M:%S %Y")
    except ValueError:
        return None
    return dt.timestamp()


def _process_start(match=PROCESS_MATCH):
    """Epoch start time of the running bridge process via `ps -o lstart=`.

    macOS BSD `ps` has NO `etimes` (elapsed-seconds) keyword — it errors with
    'keyword not found' and dumps the valid-keyword list to stdout — so the original
    etimes approach silently failed open (status 'unknown', the guard never guarding).
    `lstart` (absolute start time) is supported on both BSD/macOS and GNU/Linux and
    gives the start time directly.
    """
    try:
        pid = subprocess.run(
            ["pgrep", "-f", match], capture_output=True, text=True, timeout=10
        ).stdout.split()
    except (OSError, subprocess.SubprocessError):
        return None, None
    if not pid:
        return None, None
    pid = pid[0]
    try:
        lstart = subprocess.run(
            ["ps", "-o", "lstart=", "-p", pid], capture_output=True, text=True, timeout=10
        ).stdout.strip()
    except (OSError, subprocess.SubprocessError):
        return pid, None
    return pid, _parse_lstart_epoch(lstart)


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--quiet", action="store_true")
    ap.add_argument("--selftest", action="store_true", help="run the pure-logic unit tests and exit")
    args = ap.parse_args()

    if args.selftest:
        suite = unittest.TestLoader().loadTestsFromTestCase(BridgeDeployedGuardTests)
        result = unittest.TextTestRunner(verbosity=2).run(suite)
        return 0 if result.wasSuccessful() else 1

    bmtime = _binary_mtime()
    pid, pstart = _process_start()
    status, detail = bridge_build_vs_running(bmtime, pstart)

    if not args.quiet:
        print("=== check_bridge_deployed (binary -> running process) ===")
        print(f"  binary: {BINARY}")
        print(f"  running pid: {pid or '(none)'}")
        print(f"  status: {status} — {detail}")

    if status == "stale":
        print("⚠ ALARM: the live bridge is running STALE code — a build was never deployed.")
        print("  Fix: PATH=/Users/v/.cargo/bin:$PATH bash scripts/build_bridge.sh --restart")
        return 2
    if status == "unknown":
        print(f"(could not verify deploy state: {detail})")
        return 3
    print("RESULT: ✓ live bridge matches the on-disk binary.")
    return 0


class BridgeDeployedGuardTests(unittest.TestCase):
    def test_stale_when_binary_newer_than_process(self):
        # The exact 2026-06-21 bug: binary built ~2h after the process started.
        status, _ = bridge_build_vs_running(15.5 * 3600, 13.3 * 3600)
        self.assertEqual(status, "stale")

    def test_deployed_when_process_after_binary(self):
        # Clean deploy: kickstart starts the process a few seconds after the build.
        status, _ = bridge_build_vs_running(1000.0, 1005.0)
        self.assertEqual(status, "deployed")

    def test_deployed_within_margin(self):
        # Build->kickstart gap or clock granularity must NOT false-alarm.
        status, _ = bridge_build_vs_running(1000.0 + DEPLOY_MARGIN_S - 1, 1000.0)
        self.assertEqual(status, "deployed")

    def test_stale_just_past_margin(self):
        status, _ = bridge_build_vs_running(1000.0 + DEPLOY_MARGIN_S + 1, 1000.0)
        self.assertEqual(status, "stale")

    def test_unknown_when_missing(self):
        self.assertEqual(bridge_build_vs_running(None, 1000.0)[0], "unknown")
        self.assertEqual(bridge_build_vs_running(1000.0, None)[0], "unknown")

    def test_parse_lstart_macos_format(self):
        # The exact macOS `ps -o lstart=` shape this guard runs against (regression for
        # the etimes->lstart fix: BSD ps has no etimes keyword, so the old path failed open).
        ep = _parse_lstart_epoch("Sun Jun 21 19:25:15 2026")
        self.assertIsNotNone(ep)
        self.assertEqual(
            datetime.datetime.fromtimestamp(ep).strftime("%Y-%m-%d %H:%M:%S"),
            "2026-06-21 19:25:15",
        )

    def test_parse_lstart_space_padded_day(self):
        # Single-digit day-of-month is space-padded by ps ('Jun  1'); must still parse.
        self.assertIsNotNone(_parse_lstart_epoch("Mon Jun  1 09:05:01 2026"))

    def test_parse_lstart_garbage_is_none(self):
        self.assertIsNone(_parse_lstart_epoch(""))
        self.assertIsNone(_parse_lstart_epoch("not a date"))
        self.assertIsNone(_parse_lstart_epoch("12345"))


if __name__ == "__main__":
    sys.exit(main())
