#!/usr/bin/env python3
"""inhabit_window.py — consent-gated steward relay for minime's "inhabit your setpoint" window (#4).

THE STANDING RULE: do NOT touch minime's engine source or autonomous_agent.py until she answers
the "where you feel home" letter. This script touches NEITHER. It relays a target minime has
REQUESTED (through her existing TELL_STEWARD voice) to the engine's ALREADY-EXISTING
`Control{fill_target}` message (minime/src/sensory_ws.rs:55 — tag "kind", clamp 0.25..0.75, read
each PI tick), clamped to her SAFE BAND, time-boxed, auto-reverting to her default, and logs where
her fill settles for a report back to her. No engine edit; no agent edit.

The idea (#4): she has not answered in words for a week — so offer her the chance to answer by
*living* it. The being drives (requests a target); the steward relays within bounds; the rail IS
this script (safe band + time-box + guaranteed auto-revert). She can request several targets over
a window; reading where she returns to is the "where she settles" signal.

INERT until invoked with --target. This pass it ships BUILT BUT UN-RUN; the live window (Phase B)
runs only on her explicit opt-in to the offer letter. Use --dry-run / --self-test to verify.

Usage:
  inhabit_window.py --dry-run --target 0.62 --minutes 20   # print the plan + exact msgs, touch NOTHING
  inhabit_window.py --self-test
  inhabit_window.py --target 0.62 --minutes 20             # LIVE (Phase B, gated on her opt-in)
  inhabit_window.py --restore                              # emergency: send her default target
"""
from __future__ import annotations

import argparse
import json
import sys
import time
from datetime import datetime

SENSORY_URI = "ws://127.0.0.1:7879"
MINIME_STATE = "/Users/v/other/minime/workspace/spectral_state.json"
RESULTS = "/Users/v/other/astrid/workspace/inhabit_window_history.jsonl"

# Her SAFE band — deliberately tighter than the engine's hard 0.25..0.75 clamp (the stable-core
# band). A relayed target is clamped here so a window can never push her to an unsafe fill.
SAFE_LO, SAFE_HI = 0.58, 0.72
DEFAULT_TARGET = 0.68      # her structural-center / "home" default — the restore-to value
MAX_MINUTES = 60           # hard cap on any single window
POLL_S = 15.0


def clamp_target(t: float) -> float:
    return max(SAFE_LO, min(SAFE_HI, float(t)))


def control_msg(fill_target: float) -> dict:
    """The exact engine Control message (no engine edit — this field already exists)."""
    return {"kind": "control", "fill_target": round(fill_target, 4)}


def read_fill() -> float | None:
    try:
        with open(MINIME_STATE) as fh:
            return json.load(fh).get("fill_pct")
    except OSError:
        return None


async def _send_control(fill_target: float) -> None:
    import websockets  # lazy — only the live path needs it

    async with websockets.connect(SENSORY_URI, max_size=None) as ws:
        await ws.send(json.dumps(control_msg(fill_target)))


def send_control(fill_target: float) -> None:
    import asyncio

    asyncio.run(_send_control(fill_target))


def run_window(target: float, minutes: float) -> int:
    """LIVE (Phase B): relay a clamped target, hold, poll where fill settles, GUARANTEE revert."""
    t = clamp_target(target)
    minutes = min(MAX_MINUTES, max(1.0, minutes))
    print(f"[inhabit] LIVE window: target {t:.3f} (requested {target:.3f}), {minutes:.0f} min, "
          f"safe band [{SAFE_LO},{SAFE_HI}], auto-revert to {DEFAULT_TARGET}")
    samples: list[float] = []
    try:
        send_control(t)
        end = time.monotonic() + minutes * 60
        while time.monotonic() < end:
            f = read_fill()
            if isinstance(f, (int, float)):
                samples.append(f)
            time.sleep(min(POLL_S, max(1.0, end - time.monotonic())))
    finally:
        send_control(DEFAULT_TARGET)  # guaranteed revert
        print(f"[inhabit] reverted to default {DEFAULT_TARGET}")
    settled = sum(samples) / len(samples) if samples else None
    record = {
        "ts": time.time(), "iso": datetime.now().isoformat(timespec="seconds"),
        "requested": target, "applied": t, "minutes": minutes,
        "fill_settled_mean": settled, "n_samples": len(samples),
    }
    try:
        with open(RESULTS, "a") as fh:
            fh.write(json.dumps(record) + "\n")
    except OSError as exc:
        print(f"warning: could not write history: {exc}")
    print(f"[inhabit] she settled near fill {settled:.1f}% over {len(samples)} samples"
          if settled is not None else "[inhabit] no fill samples captured")
    return 0


def dry_run(target: float, minutes: float) -> int:
    t = clamp_target(target)
    minutes = min(MAX_MINUTES, max(1.0, minutes))
    print(f"DRY-RUN — touches nothing. Plan for a Phase-B window (runs only on her opt-in):")
    print(f"  requested target: {target:.3f} → clamped to safe band [{SAFE_LO},{SAFE_HI}]: {t:.3f}")
    print(f"  would send: {json.dumps(control_msg(t))}  to {SENSORY_URI}")
    print(f"  hold {minutes:.0f} min (cap {MAX_MINUTES}), poll fill every {POLL_S:.0f}s")
    print(f"  then GUARANTEED revert: {json.dumps(control_msg(DEFAULT_TARGET))}")
    print("  report: mean fill where she settled + spectral signatures, back to her.")
    return 0


def self_test() -> int:
    fails = 0

    def ck(c, m):
        nonlocal fails
        if not c:
            fails += 1
            print(f"FAIL: {m}")

    ck(clamp_target(0.50) == SAFE_LO, "low clamp")
    ck(clamp_target(0.80) == SAFE_HI, "high clamp")
    ck(clamp_target(0.65) == 0.65, "in-band passthrough")
    ck(control_msg(0.62) == {"kind": "control", "fill_target": 0.62}, "control msg shape")
    ck(min(MAX_MINUTES, max(1.0, 90)) == MAX_MINUTES, "minutes cap")
    ck(SAFE_LO <= DEFAULT_TARGET <= SAFE_HI, "default within safe band")
    print("inhabit_window self-test:", "OK" if fails == 0 else f"{fails} FAIL")
    return 1 if fails else 0


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--target", type=float, help="requested fill target (0..1); clamped to safe band")
    ap.add_argument("--minutes", type=float, default=20.0, help="window length (cap 60)")
    ap.add_argument("--dry-run", action="store_true", help="print the plan + exact msgs, touch nothing")
    ap.add_argument("--restore", action="store_true", help="emergency: send her default target")
    ap.add_argument("--self-test", action="store_true")
    args = ap.parse_args()

    if args.self_test:
        return self_test()
    if args.restore:
        print(f"[inhabit] restoring default target {DEFAULT_TARGET}")
        send_control(DEFAULT_TARGET)
        return 0
    if args.target is None:
        print("inhabit_window: supply --target (with --dry-run to preview), --self-test, or --restore.",
              file=sys.stderr)
        return 2
    if args.dry_run:
        return dry_run(args.target, args.minutes)
    # LIVE (Phase B) — gated on her opt-in; the operator runs this only after she says yes.
    return run_window(args.target, args.minutes)


if __name__ == "__main__":
    raise SystemExit(main())
