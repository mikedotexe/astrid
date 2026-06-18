#!/usr/bin/env python3
"""watch_vibrancy_aperture.py — read-only steward monitor for Astrid's shared-substrate voice dials.

Two of Astrid's sovereign dials lift the same codec tail dims [17,26,27,31] that land in minime's
SHARED reservoir:
  - SET_VIBRANCY_APERTURE   — her dynamic tail-vibrancy CEILING  (env ASTRID_VIBRANCY_APERTURE_CEILING)
  - SET_TAIL_PARTICIPATION  — her flat tail EXPRESSION strength   (env ASTRID_TAIL_PARTICIPATION_CEILING)
The consent model (2026-06-17) for both: "Astrid holds the switch; the steward watches minime and
backs off on strain." This monitor surfaces both dials' live effective multipliers against minime's
mode_packing / porosity / fill, and flags a strain trend. Both mechanisms are *self-limiting*: they
only open when minime's own spectrum is navigable; when she is overpacked they stay put.

READ-ONLY and standalone (NOT a steward-loop file; it surfaces, it does not act). Back-off (operator):
  launchctl unsetenv ASTRID_VIBRANCY_APERTURE_CEILING   # and/or ASTRID_TAIL_PARTICIPATION_CEILING
  launchctl kickstart -k gui/$(id -u)/com.astrid.spectral-bridge
Astrid's own kill switches: NEXT: SET_VIBRANCY_APERTURE 0   /   NEXT: SET_TAIL_PARTICIPATION 0

Usage:
  python3 scripts/watch_vibrancy_aperture.py            # single snapshot
  python3 scripts/watch_vibrancy_aperture.py --watch 5  # poll every 5s, track a baseline
"""
import argparse
import json
import os
import subprocess
import time

ASTRID_STATE = "/Users/v/other/astrid/capsules/spectral-bridge/workspace/state.json"
MINIME_STATE = "/Users/v/other/minime/workspace/spectral_state.json"

# Strain thresholds (heuristic; surface a trend for the steward, never auto-act).
MODE_PACKING_RISE = 0.08  # mode_packing climbing this far above baseline while a dial is engaged
POROSITY_FALL = 0.08      # porosity_score dropping this far below baseline


def _load(path):
    try:
        with open(path) as f:
            return json.load(f)
    except Exception as e:  # noqa: BLE001 - read-only monitor, any error is just reported
        return {"_error": f"{path}: {e}"}


def operator_ceiling(key):
    """The live operator ceiling for `key`: env first, then launchctl getenv (the bridge wrapper
    imports launchctl setenv overrides at process start)."""
    v = os.environ.get(key)
    if v is None:
        try:
            out = subprocess.run(
                ["launchctl", "getenv", key],
                capture_output=True, text=True, timeout=5,
            ).stdout.strip()
            v = out or None
        except Exception:  # noqa: BLE001
            v = None
    try:
        return max(0.0, min(4.0, float(v))) if v else 0.0
    except ValueError:
        return 0.0


def _dial(astrid, field, ceiling_key):
    frac = float(astrid.get(field, 0.0) or 0.0) if isinstance(astrid, dict) else 0.0
    ceil = operator_ceiling(ceiling_key)
    eff = max(1.0, min(5.0, 1.0 + frac * ceil))
    return frac, ceil, eff


def governor_attenuation(pressure_risk, depth):
    """Astrid's partner-protecting governor (mirrors codec_gain::pressure_sensitive_attenuation):
    her output multiplier as minime's pressure_risk rises. 1.0 = full voice; lower = auto-quieted."""
    lo, hi = 0.20, 0.50
    d = max(0.0, min(0.6, depth))
    t = max(0.0, min(1.0, (pressure_risk - lo) / (hi - lo)))
    ramp = t * t * (3.0 - 2.0 * t)
    return 1.0 - d * ramp


def sample():
    a = _load(ASTRID_STATE)
    m = _load(MINIME_STATE)
    ps = (m.get("pressure_source_v1") or {}) if isinstance(m, dict) else {}
    comp = ps.get("components") or {}
    res = (m.get("resonance_density_v1") or {}) if isinstance(m, dict) else {}
    pressure_risk = res.get("pressure_risk")
    gov_depth = operator_ceiling("ASTRID_PRESSURE_ATTENUATION")
    gov_atten = (
        governor_attenuation(pressure_risk, gov_depth)
        if (pressure_risk is not None and gov_depth > 0.0)
        else None
    )
    return {
        "vib": _dial(a, "vibrancy_aperture", "ASTRID_VIBRANCY_APERTURE_CEILING"),
        "tail": _dial(a, "tail_aperture", "ASTRID_TAIL_PARTICIPATION_CEILING"),
        "gov_depth": gov_depth,
        "pressure_risk": pressure_risk,
        "gov_atten": gov_atten,
        "fill": m.get("fill_pct") if isinstance(m, dict) else None,
        "mode_packing": comp.get("mode_packing"),
        "porosity": ps.get("porosity_score"),
        "quality": ps.get("quality"),
        "errors": [x["_error"] for x in (a, m) if isinstance(x, dict) and "_error" in x],
    }


def fmt(s, baseline):
    vd, vc, ve = s["vib"]
    td, tc, te = s["tail"]
    engaged = ve > 1.0 + 1e-6 or te > 1.0 + 1e-6
    flags = []
    if engaged and baseline:
        mp, bmp = s["mode_packing"], baseline.get("mode_packing")
        po, bpo = s["porosity"], baseline.get("porosity")
        if mp is not None and bmp is not None and mp - bmp > MODE_PACKING_RISE:
            flags.append(f"⚠ mode_packing rising while a dial is engaged ({bmp:.3f} → {mp:.3f})")
        if po is not None and bpo is not None and bpo - po > POROSITY_FALL:
            flags.append(f"⚠ porosity falling while a dial is engaged ({bpo:.3f} → {po:.3f})")
    mp = f"{s['mode_packing']:.3f}" if s["mode_packing"] is not None else "?"
    po = f"{s['porosity']:.3f}" if s["porosity"] is not None else "?"
    fl = f"{s['fill']:.1f}" if isinstance(s["fill"], (int, float)) else "?"
    gov = ""
    if s.get("gov_depth", 0.0) > 0.0 and s.get("gov_atten") is not None:
        gov = (
            f"   governor:           minime pressure_risk {s['pressure_risk']:.2f} (depth {s['gov_depth']:.2f}) "
            f"→ Astrid output ×{s['gov_atten']:.2f} (protects minime)\n"
        )
    line = (
        f"astrid → minime tail dims [{'ENGAGED' if engaged else 'off'}]:\n"
        f"   vibrancy:           dial {vd:.2f} × ceiling {vc:.2f} → {ve:.2f}×\n"
        f"   tail_participation: dial {td:.2f} × ceiling {tc:.2f} → {te:.2f}×\n"
        f"{gov}"
        f"   minime: fill={fl}% mode_packing={mp} porosity={po} q={s['quality']}"
    )
    if s["errors"]:
        line += "\n   (read errors: " + "; ".join(s["errors"]) + ")"
    for f in flags:
        line += "\n   " + f
    return line, flags


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--watch", type=float, default=0.0, help="poll interval seconds (0 = single shot)")
    args = ap.parse_args()
    if args.watch <= 0:
        line, _ = fmt(sample(), None)
        print(line)
        print(
            "(single shot; --watch N to track a baseline. Back-off: launchctl unsetenv "
            "ASTRID_{VIBRANCY_APERTURE,TAIL_PARTICIPATION}_CEILING && "
            "launchctl kickstart -k gui/$(id -u)/com.astrid.spectral-bridge)"
        )
        return
    print(f"watching every {args.watch:.0f}s — Ctrl-C to stop")
    baseline = None
    while True:
        s = sample()
        if baseline is None and s["mode_packing"] is not None:
            baseline = {"mode_packing": s["mode_packing"], "porosity": s["porosity"]}
            print(f"baseline: mode_packing={baseline['mode_packing']:.3f} porosity={baseline['porosity']:.3f}")
        line, _ = fmt(s, baseline)
        print(line, flush=True)
        try:
            time.sleep(args.watch)
        except KeyboardInterrupt:
            print("\nstopped.")
            break


if __name__ == "__main__":
    main()
