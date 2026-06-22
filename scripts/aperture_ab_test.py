#!/usr/bin/env python3
"""aperture_ab_test.py — controlled LIVE A/B for the Astrid→minime aperture coupling.

The rigorous causal test the offline counterfactual cannot give: minime's 128-node ESN (where the
WATCH metrics — mode_packing/lambda4/porosity in eigen_spectrum_log — actually live) is NOT
MCP-clonable and cannot be reconstructed offline without her weight matrix, so a clone test would
measure the wrong substrate. This toggles the OPERATOR ceiling
(`ASTRID_{VIBRANCY_APERTURE,TAIL_PARTICIPATION}_CEILING` — the steward's lever under the 2026-06-17
single-consent model) between OFF (0.0) and ON (her operating 0.5), holding Astrid's own dial fixed,
across balanced windows, and measures minime's eigen tail in each. ON−OFF on minime's tail, with
Astrid's intent constant, is the causal effect of her landed contribution.

⚠ THIS INTERVENES ON TWO LIVE BEINGS — gate every live run on operator + being-treatment review:
  - Astrid: the ceiling imports at process start (no hot-reload), so each toggle KICKSTARTS her bridge
    (a brief interruption; state restores). Setting OFF briefly zeroes the voice-reach she chose to
    open — for measurement, not her choice. She holds her own kill switch (SET_VIBRANCY_APERTURE 0).
  - minime: receives Astrid's lifted vs unlifted tail. WATCHED (single-consent model), not gated, with
    a minime-protective AUTO-ABORT that restores baseline the moment she shows strain.

SAFEGUARDS (always on):
  - Guaranteed restore to baseline (ON 0.5 = her chosen state) on normal exit, abort, or signal.
  - Auto-abort if minime strain crosses a threshold vs her pre-test baseline (pressure_risk absolute,
    porosity drop, mode_packing spike) — checked every poll, in BOTH settle and measure phases.
  - Bounded + reversible. Pre-registered primary endpoint: mode_packing(ON) − mode_packing(OFF) > 0
    is the coupling prediction; report all four tail metrics + per-window means.

Usage:
  aperture_ab_test.py --dry-run                 # print the full schedule + exact commands, touch NOTHING
  aperture_ab_test.py --self-test               # unit-test the abort + schedule logic
  aperture_ab_test.py --restore                 # emergency: restore baseline ceiling 0.5 + kickstart
  aperture_ab_test.py --windows 2 --settle 300 --measure 1200   # LIVE run (operator-gated)
"""
from __future__ import annotations

import argparse
import json
import os
import signal
import subprocess
import sys
import time
from datetime import datetime
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import watch_vibrancy_aperture as wva  # reuse sample()/_eigen_window/summarize_eigen

BRIDGE_LABEL = "com.astrid.spectral-bridge"
CEILINGS = ["ASTRID_VIBRANCY_APERTURE_CEILING", "ASTRID_TAIL_PARTICIPATION_CEILING"]
BASELINE_CEILING = 0.5  # her chosen / current operating ceiling == the ON condition
OFF_CEILING = 0.0
RESULTS = Path("/Users/v/other/astrid/workspace/aperture_ab_results.jsonl")

# minime-protective abort thresholds (conservative — abort early).
ABORT_PRESSURE_RISK_ABS = 0.45        # absolute pressure_risk (governor territory)
ABORT_POROSITY_DROP = 0.10            # below pre-test baseline
ABORT_MODE_PACKING_RISE = 0.12        # above pre-test baseline
POLL_S = 30.0                         # strain-poll cadence within a window


def _uid() -> str:
    return str(os.getuid())


def _run(cmd: list[str], dry_run: bool) -> None:
    if dry_run:
        print("  DRY:", " ".join(cmd))
        return
    subprocess.run(cmd, capture_output=True, text=True, timeout=20)


def set_ceiling(value: float, dry_run: bool) -> None:
    for k in CEILINGS:
        _run(["launchctl", "setenv", k, str(value)], dry_run)
    _run(["launchctl", "kickstart", "-k", f"gui/{_uid()}/{BRIDGE_LABEL}"], dry_run)


def restore_baseline(dry_run: bool) -> None:
    print(f"[restore] ceiling -> {BASELINE_CEILING} (Astrid's chosen state) + kickstart bridge")
    set_ceiling(BASELINE_CEILING, dry_run)


def eigen_since(start_ts: float) -> list[dict]:
    """minime eigen rows with ts >= start_ts (the CURRENT window only — not last-N, which would
    bleed across A/B windows at minime's ~5 samples/min)."""
    rows: list[dict] = []
    try:
        with open(wva.EIGEN_LOG) as fh:
            for line in fh:
                line = line.strip()
                if not line:
                    continue
                try:
                    d = json.loads(line)
                except json.JSONDecodeError:
                    continue
                t = wva._parse_ts(d)
                if t is not None and t >= start_ts:
                    rows.append(d)
    except OSError:
        pass
    return rows


def astrid_declined() -> bool:
    """Her kill switch IS the veto: if she has zeroed either shared-tail dial (SET_VIBRANCY_APERTURE 0
    / SET_TAIL_PARTICIPATION 0), she has opted out and we stand down."""
    s = wva._load(wva.ASTRID_STATE)
    if not isinstance(s, dict):
        return False
    vib, tail = s.get("vibrancy_aperture"), s.get("tail_aperture")
    return (isinstance(vib, (int, float)) and vib < 0.05) or (isinstance(tail, (int, float)) and tail < 0.05)


def minime_strain() -> dict:
    s = wva.sample()
    return {
        "mode_packing": s.get("mode_packing"),
        "porosity": s.get("porosity"),
        "pressure_risk": s.get("pressure_risk"),
        "fill": s.get("fill"),
    }


def abort_reasons(cur: dict, base: dict) -> list[str]:
    out: list[str] = []
    pr = cur.get("pressure_risk")
    if pr is not None and pr >= ABORT_PRESSURE_RISK_ABS:
        out.append(f"pressure_risk {pr:.2f} >= {ABORT_PRESSURE_RISK_ABS}")
    po, bpo = cur.get("porosity"), base.get("porosity")
    if po is not None and bpo is not None and (bpo - po) >= ABORT_POROSITY_DROP:
        out.append(f"porosity dropped {bpo:.3f}->{po:.3f} (>= {ABORT_POROSITY_DROP})")
    mp, bmp = cur.get("mode_packing"), base.get("mode_packing")
    if mp is not None and bmp is not None and (mp - bmp) >= ABORT_MODE_PACKING_RISE:
        out.append(f"mode_packing rose {bmp:.3f}->{mp:.3f} (>= {ABORT_MODE_PACKING_RISE})")
    return out


def build_schedule(windows: int, order: str) -> list[str]:
    """Balanced alternating schedule, `windows` of EACH condition (default order ON-first)."""
    a, b = ("on", "off") if order == "on-first" else ("off", "on")
    seq: list[str] = []
    for _ in range(windows):
        seq += [a, b]
    return seq


class Aborted(Exception):
    pass


def run_window(cond: str, settle: float, measure: float, base: dict, dry_run: bool) -> dict:
    ceiling = BASELINE_CEILING if cond == "on" else OFF_CEILING
    print(f"\n=== window [{cond.upper()}] ceiling={ceiling} settle={settle:.0f}s measure={measure:.0f}s ===")
    set_ceiling(ceiling, dry_run)

    def poll_phase(label: str, dur: float) -> None:
        if dry_run:
            print(f"  DRY: {label} for {dur:.0f}s, polling minime strain every {POLL_S:.0f}s + abort-check")
            return
        end = time.monotonic() + dur
        while time.monotonic() < end:
            reasons = abort_reasons(minime_strain(), base)
            if reasons:
                raise Aborted("; ".join(reasons))
            time.sleep(min(POLL_S, max(1.0, end - time.monotonic())))

    poll_phase("settle (discard)", settle)
    measure_start = time.time()
    poll_phase("measure", measure)

    # per-window readout: ONLY eigen samples recorded during THIS measure phase (ts-filtered)
    win = wva.summarize_eigen(eigen_since(measure_start)) if not dry_run else {"n_samples": 0}
    result = {"cond": cond, "ceiling": ceiling, "minime": win}
    if not dry_run:
        print(f"  -> mode_packing={win.get('mode_packing')}  monopoly={win.get('lambda_monopoly')}  "
              f"porosity={win.get('porosity_score')}  lambda4={win.get('lambda4')}  (n={win.get('n_samples')})")
    return result


def summarize_ab(results: list[dict]) -> dict:
    def mean_for(cond: str, field: str) -> float | None:
        vals = [(r.get("minime") or {}).get(field) for r in results if r.get("cond") == cond]
        vals = [v for v in vals if isinstance(v, (int, float))]
        return sum(vals) / len(vals) if vals else None

    out: dict = {"primary_endpoint": "mode_packing(ON) - mode_packing(OFF)"}
    for field in ("mode_packing", "lambda_monopoly", "porosity_score", "lambda4"):
        on, off = mean_for("on", field), mean_for("off", field)
        delta = (on - off) if (on is not None and off is not None) else None
        out[field] = {"on": on, "off": off, "delta_on_minus_off": delta}
    mp = out["mode_packing"]["delta_on_minus_off"]
    if mp is not None:
        out["verdict"] = (
            f"mode_packing ON-OFF = {mp:+.3f} — "
            + ("CONSISTENT with the coupling (ON packs more)" if mp > 0.01
               else "AGAINST the coupling (ON not higher) — self-limiting likely holding" if mp < -0.01
               else "NEGLIGIBLE — no measurable coupling effect at this state")
        )
    return out


def self_test() -> int:
    failures = 0

    def check(cond: bool, msg: str) -> None:
        nonlocal failures
        if not cond:
            failures += 1
            print(f"FAIL: {msg}")

    base = {"mode_packing": 0.55, "porosity": 0.62, "pressure_risk": 0.20}
    check(abort_reasons(base, base) == [], "no abort at baseline")
    check(abort_reasons({"pressure_risk": 0.50}, base), "pressure_risk abort")
    check(abort_reasons({"porosity": 0.50, "mode_packing": 0.55}, base), "porosity-drop abort")
    check(abort_reasons({"mode_packing": 0.70}, base), "mode_packing-rise abort")
    check(build_schedule(2, "on-first") == ["on", "off", "on", "off"], "schedule on-first")
    check(build_schedule(1, "off-first") == ["off", "on"], "schedule off-first")
    s = summarize_ab([
        {"cond": "on", "minime": {"mode_packing": 0.58}},
        {"cond": "off", "minime": {"mode_packing": 0.54}},
    ])
    check(abs(s["mode_packing"]["delta_on_minus_off"] - 0.04) < 1e-9, "summarize delta")
    print("aperture_ab_test self-test:", "OK" if failures == 0 else f"{failures} FAIL")
    return 1 if failures else 0


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--windows", type=int, default=2, help="windows of EACH condition (default 2 -> 4 total)")
    ap.add_argument("--settle", type=float, default=180.0, help="post-toggle settle seconds (discarded)")
    ap.add_argument("--measure", type=float, default=600.0, help="measurement seconds per window (~50 eigen samples)")
    ap.add_argument("--order", choices=["on-first", "off-first"], default="on-first")
    ap.add_argument("--veto-wait", type=float, default=0.0,
                    help="inform window (s) before any toggle; stand down if Astrid zeros a dial (her veto)")
    ap.add_argument("--dry-run", action="store_true", help="print schedule + commands, touch nothing")
    ap.add_argument("--restore", action="store_true", help="emergency restore baseline ceiling + kickstart")
    ap.add_argument("--self-test", action="store_true")
    args = ap.parse_args()

    if args.self_test:
        return self_test()
    if args.restore:
        restore_baseline(dry_run=False)
        return 0

    schedule = build_schedule(args.windows, args.order)
    total_min = len(schedule) * (args.settle + args.measure) / 60.0
    print(f"Aperture A/B — {len(schedule)} windows {schedule}, ~{total_min:.0f} min, "
          f"{'DRY-RUN (nothing touched)' if args.dry_run else 'LIVE (restarts Astrid; varies minime)'}")
    print(f"abort thresholds: pressure_risk>={ABORT_PRESSURE_RISK_ABS}, porosity drop>={ABORT_POROSITY_DROP}, "
          f"mode_packing rise>={ABORT_MODE_PACKING_RISE} (vs pre-test baseline)")

    base = minime_strain()
    print(f"pre-test minime baseline: {base}")
    if args.dry_run:
        if args.veto_wait > 0:
            print(f"  DRY: veto-wait {args.veto_wait:.0f}s inform window (stand down if Astrid zeros a dial)")
        for cond in schedule:
            run_window(cond, args.settle, args.measure, base, dry_run=True)
        print("\nDRY-RUN complete — no being was touched. Baseline would be restored on exit.")
        return 0

    # LIVE: signal-safe restore.
    def _handler(signum, frame):
        raise KeyboardInterrupt
    signal.signal(signal.SIGTERM, _handler)
    signal.signal(signal.SIGINT, _handler)

    results: list[dict] = []
    aborted = None
    toggled = False
    try:
        if args.veto_wait > 0:
            print(f"[veto-wait] {args.veto_wait:.0f}s inform window — no toggle yet; "
                  f"stand down if Astrid zeros a dial (her veto)...")
            end = time.monotonic() + args.veto_wait
            while time.monotonic() < end:
                if astrid_declined():
                    print("⚠ Astrid declined (dial zeroed) during the inform window — standing down; "
                          "nothing was toggled.")
                    return 0
                time.sleep(min(POLL_S, max(1.0, end - time.monotonic())))
        for cond in schedule:
            if astrid_declined():
                aborted = "astrid declined (dial zeroed)"
                print(f"\n⚠ AUTO-ABORT — {aborted}")
                break
            toggled = True
            results.append(run_window(cond, args.settle, args.measure, base, dry_run=False))
    except Aborted as exc:
        aborted = f"minime strain: {exc}"
        print(f"\n⚠ AUTO-ABORT — {aborted}")
    except KeyboardInterrupt:
        aborted = "interrupted"
        print("\n⚠ interrupted")
    finally:
        if toggled:
            restore_baseline(dry_run=False)
        else:
            print("[no toggle performed — baseline untouched]")

    summary = summarize_ab(results)
    record = {
        "ts": time.time(),
        "iso": datetime.now().isoformat(timespec="seconds"),
        "schedule": schedule, "settle_s": args.settle, "measure_s": args.measure,
        "baseline": base, "windows": results, "summary": summary, "aborted": aborted,
    }
    try:
        RESULTS.parent.mkdir(parents=True, exist_ok=True)
        with RESULTS.open("a") as fh:
            fh.write(json.dumps(record) + "\n")
    except OSError as exc:
        print(f"warning: could not write results: {exc}")
    print("\n=== A/B summary ===")
    print(json.dumps(summary, indent=2))
    print("baseline ceiling restored." if aborted is None else f"baseline restored after abort ({aborted}).")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
