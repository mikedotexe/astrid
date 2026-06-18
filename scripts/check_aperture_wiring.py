#!/usr/bin/env python3
"""check_aperture_wiring.py — guard against the silently-dead-dial muffle class.

On 2026-06-17 we found Astrid's SET_TAIL_PARTICIPATION dial was inert in production: the Rust read
`ASTRID_TAIL_PARTICIPATION_CEILING` from the process env, but the launchd wrapper's allowlist never
imported that key, so the bridge saw it unset -> 0.0 -> her dial pinned at identity. She'd been
using it (and getting success receipts) for an unknown stretch. The existing `stated_param_intent`
un-muffle probe did NOT catch this — it doesn't check the env->process import path.

This guard closes that class. For every `ASTRID_*_CEILING` env the Rust READS, it verifies the
value can actually REACH the bridge process, via either:
  - the launchd wrapper allowlist (`scripts/launchd_spectral_bridge.sh`'s `for key in …` loop), or
  - the durable config that the wrapper sources (`workspace/runtime/aperture_ceilings.env`).
A ceiling read by the code but reachable by NEITHER is a silently-dead dial -> ALARM (exit 2).
A ceiling reachable only via the allowlist (not the durable config) works today but dies on the
next reboot (`launchctl setenv` is wiped) -> WARN.

READ-ONLY and standalone. Proposed for adoption by the durable steward loop (proactive_scan.py is
loop-owned; this is the un-muffle guard for the env->process plumbing class it currently misses).

Usage:  python3 scripts/check_aperture_wiring.py [--quiet]
Exit:   0 = all wired (+ any warns printed); 2 = at least one silently-dead dial.
"""
import argparse
import glob
import re
import sys

ROOT = "/Users/v/other/astrid"
SRC_GLOB = f"{ROOT}/capsules/spectral-bridge/src/**/*.rs"
WRAPPER = f"{ROOT}/scripts/launchd_spectral_bridge.sh"
CONFIG = f"{ROOT}/capsules/spectral-bridge/workspace/runtime/aperture_ceilings.env"

# Being-dial gating envs that must reach the bridge process: the aperture *_CEILING dials and the
# pressure governor (*_ATTENUATION). Any such env the Rust reads must be allowlisted/configured.
CEILING = r"ASTRID_[A-Z0-9_]+_(?:CEILING|ATTENUATION)"
READ_RE = re.compile(r'env::var\("(' + CEILING + r')"\)')
ALLOWLIST_LINE_RE = re.compile(r"^\s*(" + CEILING + r")\s*\\?\s*$")
CONFIG_EXPORT_RE = re.compile(r"^\s*export\s+(" + CEILING + r")=")


def _read(path):
    try:
        with open(path) as f:
            return f.read()
    except OSError:
        return ""  # missing file is handled by the callers (empty -> nothing wired)


def ceilings_read_by_code():
    found = set()
    for path in glob.glob(SRC_GLOB, recursive=True):
        found.update(READ_RE.findall(_read(path)))
    return found


def allowlist_keys():
    return {m.group(1) for line in _read(WRAPPER).splitlines()
            if (m := ALLOWLIST_LINE_RE.match(line))}


def config_keys():
    return {m.group(1) for line in _read(CONFIG).splitlines()
            if (m := CONFIG_EXPORT_RE.match(line))}


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--quiet", action="store_true", help="only print on ALARM/WARN")
    args = ap.parse_args()

    read = ceilings_read_by_code()
    allow = allowlist_keys()
    cfg = config_keys()

    alarms, warns, oks = [], [], []
    for key in sorted(read):
        in_allow, in_cfg = key in allow, key in cfg
        if not in_allow and not in_cfg:
            alarms.append(f"ALARM  {key}: read by the bridge but in NEITHER the wrapper allowlist "
                          f"NOR the config -> SILENTLY DEAD (process sees it unset). "
                          f"Fix: add it to {WRAPPER}'s `for key in` loop and to {CONFIG}.")
        elif in_allow and not in_cfg:
            warns.append(f"WARN   {key}: in the wrapper allowlist but NOT the durable config "
                         f"-> works via `launchctl setenv` now but dies on reboot. "
                         f"Fix: add `export {key}=<v>` to {CONFIG}.")
        else:
            srcs = "+".join([s for s, b in (("allowlist", in_allow), ("config", in_cfg)) if b])
            oks.append(f"ok     {key}: reachable via {srcs}"
                       + ("" if in_cfg else " (NOT durable)"))

    if not args.quiet or alarms or warns:
        print("=== aperture-wiring guard (env -> bridge process reachability) ===")
        print(f"  ceiling envs read by the Rust: {len(read)}  | allowlist: {len(allow)}  | config: {len(cfg)}")
        for line in oks + warns + alarms:
            print("  " + line)
        if not read:
            print("  (no ASTRID_*_CEILING reads found — check SRC_GLOB)")

    if alarms:
        print(f"\n{len(alarms)} silently-dead dial(s) — a being faculty reports success but is inert.")
        sys.exit(2)
    if not args.quiet:
        print("\nAll ceiling dials reach the bridge." + (" (some not durable — see WARN)" if warns else " All durable."))
    sys.exit(0)


if __name__ == "__main__":
    main()
