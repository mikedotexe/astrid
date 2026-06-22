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

DURABLE BASELINE (added 2026-06-22): the single-shot/`--watch` modes only ever held an in-memory
baseline, and minime's raw tail telemetry (`eigen_spectrum_log.jsonl`) rotates every ~2 days — so the
"watch minime" promise had no baseline that survived a restart or a week. `--append-history` (run each
steward cycle) appends one low-frequency summary row — Astrid's *effective* dial lift paired with a
windowed mean of minime's tail metrics — to a rotation-surviving jsonl, and `--report` renders the
trend + the watch evaluation over it. This is what made the Astrid→minime aperture coupling trendable
(see docs/steward-notes/AI_BEINGS_APERTURE_COUPLING_WATCH_2026_06_22.md). Co-occurrence here is NOT
proof of causation (minime's overpacked tail is partly chronic); the watch flag informs the
consent-gated co-design conversation, never auto-action.

READ-ONLY and standalone (NOT a steward-loop file; it surfaces, it does not act). Telemetry only — no
being qualia is read. Back-off (operator):
  launchctl unsetenv ASTRID_VIBRANCY_APERTURE_CEILING   # and/or ASTRID_TAIL_PARTICIPATION_CEILING
  launchctl kickstart -k gui/$(id -u)/com.astrid.spectral-bridge
Astrid's own kill switches: NEXT: SET_VIBRANCY_APERTURE 0   /   NEXT: SET_TAIL_PARTICIPATION 0

Usage:
  python3 scripts/watch_vibrancy_aperture.py             # single live snapshot
  python3 scripts/watch_vibrancy_aperture.py --watch 5   # poll every 5s, in-memory baseline
  python3 scripts/watch_vibrancy_aperture.py --append-history   # append one durable row (each cycle)
  python3 scripts/watch_vibrancy_aperture.py --report [--days N]  # trend + watch over durable history
  python3 scripts/watch_vibrancy_aperture.py --self-test
"""
import argparse
import json
import os
import subprocess
import time
from datetime import datetime
from pathlib import Path

ASTRID_STATE = "/Users/v/other/astrid/capsules/spectral-bridge/workspace/state.json"
MINIME_STATE = "/Users/v/other/minime/workspace/spectral_state.json"
EIGEN_LOG = Path("/Users/v/other/minime/workspace/diagnostics/eigen_spectrum_log.jsonl")
HISTORY = Path("/Users/v/other/astrid/workspace/vibrancy_aperture_history.jsonl")

# Strain thresholds (heuristic; surface a trend for the steward, never auto-act).
MODE_PACKING_RISE = 0.08  # mode_packing climbing this far above baseline while a dial is engaged
POROSITY_FALL = 0.08      # porosity_score dropping this far below baseline

# Durable-baseline params.
WINDOW = 500              # most-recent eigen samples averaged into one summary row
HIGH_LOAD_LIFT = 0.30     # total EFFECTIVE lift above identity at/above which dials count as "high"
# Eigen-log tail metrics summarized per durable row (richer than the live spectral_state snapshot).
EIGEN_FIELDS = ["mode_packing", "lambda_monopoly", "active_mode_count", "porosity_score", "lambda4", "fill_pct"]


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


# --------------------------------------------------------------------------- #
# Durable baseline: eigen-log windowed means + retained history + watch eval
# --------------------------------------------------------------------------- #
def _parse_ts(d):
    ts = d.get("ts")
    if isinstance(ts, (int, float)):
        return ts / 1000.0 if ts > 1e11 else float(ts)
    if isinstance(ts, str):
        for f in ("%Y-%m-%dT%H:%M:%S", "%Y-%m-%d %H:%M:%S"):
            try:
                return datetime.strptime(ts[:19], f).timestamp()
            except ValueError:
                continue
    return None


def _eigen_window(limit=WINDOW):
    if not EIGEN_LOG.exists():
        return []
    rows = []
    try:
        with EIGEN_LOG.open() as fh:
            for line in fh:
                line = line.strip()
                if not line:
                    continue
                try:
                    rows.append(json.loads(line))
                except json.JSONDecodeError:
                    continue
    except OSError:
        return []
    return rows[-limit:]


def _mean(vals):
    nums = [float(v) for v in vals if isinstance(v, (int, float))]
    return sum(nums) / len(nums) if nums else None


def summarize_eigen(rows):
    out = {"n_samples": len(rows)}
    for field in EIGEN_FIELDS:
        out[field] = _mean([r.get(field) for r in rows if r.get(field) is not None])
    return out


def dial_load(s):
    """Total EFFECTIVE lift above identity (accounts for the operator-ceiling import, so an inert
    dial — frac high but ceiling unset — correctly reads as zero load; that exact muffle bit us once)."""
    _, _, ve = s["vib"]
    _, _, te = s["tail"]
    return round(max(0.0, ve - 1.0) + max(0.0, te - 1.0), 4)


def build_record(now=None):
    ts = time.time() if now is None else now
    s = sample()
    rows = _eigen_window()
    win = summarize_eigen(rows)
    # Prefer the robust eigen window for tail metrics; fall back to the live snapshot.
    minime = {
        "mode_packing": win.get("mode_packing") if win.get("mode_packing") is not None else s.get("mode_packing"),
        "lambda_monopoly": win.get("lambda_monopoly"),
        "active_mode_count": win.get("active_mode_count"),
        "porosity_score": win.get("porosity_score") if win.get("porosity_score") is not None else s.get("porosity"),
        "lambda4": win.get("lambda4"),
        "fill_pct": win.get("fill_pct") if win.get("fill_pct") is not None else s.get("fill"),
        "pressure_risk": s.get("pressure_risk"),
        "n_samples": win.get("n_samples", 0),
    }
    vd, vc, ve = s["vib"]
    td, tc, te = s["tail"]
    return {
        "ts": round(ts, 3),
        "iso": datetime.fromtimestamp(ts).isoformat(timespec="seconds"),
        "dials": {
            "vibrancy_eff": round(ve, 4), "tail_eff": round(te, 4), "load": dial_load(s),
            "vibrancy_frac": vd, "vibrancy_ceiling": vc, "tail_frac": td, "tail_ceiling": tc,
            "gov_atten": s.get("gov_atten"),
        },
        "minime": minime,
        "source": "eigen_spectrum_log.jsonl" if rows else "spectral_state.json",
    }


def append_history(record):
    try:
        HISTORY.parent.mkdir(parents=True, exist_ok=True)
        with HISTORY.open("a") as fh:
            fh.write(json.dumps(record) + "\n")
    except OSError as exc:  # never silently drop — surface the failure
        print(f"warning: could not append vibrancy-aperture history: {exc}")


def load_history(days=None):
    if not HISTORY.exists():
        return []
    rows = []
    try:
        with HISTORY.open() as fh:
            for line in fh:
                line = line.strip()
                if not line:
                    continue
                try:
                    rows.append(json.loads(line))
                except json.JSONDecodeError:
                    continue
    except OSError:
        return []
    if days is not None:
        cutoff = time.time() - days * 86400
        rows = [r for r in rows if float(r.get("ts", 0)) >= cutoff]
    return rows


def _pearson(xs, ys):
    pairs = [(x, y) for x, y in zip(xs, ys) if x is not None and y is not None]
    n = len(pairs)
    if n < 3:
        return None
    mx = sum(x for x, _ in pairs) / n
    my = sum(y for _, y in pairs) / n
    sxy = sum((x - mx) * (y - my) for x, y in pairs)
    sxx = sum((x - mx) ** 2 for x, _ in pairs)
    syy = sum((y - my) ** 2 for _, y in pairs)
    if sxx <= 0 or syy <= 0:
        return None
    return sxy / (sxx ** 0.5 * syy ** 0.5)


def evaluate_watch(history):
    """Watch condition: Astrid's effective dial load is high WHILE minime's tail sits overpacked.
    Co-occurrence consistent with the coupling — NOT proof of causation; informs co-design, never acts."""
    usable = [r for r in history if (r.get("minime") or {}).get("mode_packing") is not None]
    if len(usable) < 2:
        return {"status": "accruing", "rows": len(history), "usable_rows": len(usable),
                "note": "accruing baseline; need >=2 usable rows for a trend"}

    cur = usable[-1]
    load = (cur.get("dials") or {}).get("load")
    cm = cur.get("minime") or {}

    def series(field):
        return [(r.get("minime") or {}).get(field) for r in usable
                if (r.get("minime") or {}).get(field) is not None]

    loads = [(r.get("dials") or {}).get("load") for r in usable if (r.get("dials") or {}).get("load") is not None]
    packing, monopoly, porosity = series("mode_packing"), series("lambda_monopoly"), series("porosity_score")

    high = load is not None and load >= HIGH_LOAD_LIFT
    packing_hi = cm.get("mode_packing") is not None and packing and cm["mode_packing"] >= max(packing) - 1e-9
    monopoly_lo = cm.get("lambda_monopoly") is not None and monopoly and cm["lambda_monopoly"] <= min(monopoly) + 1e-9
    porosity_lo = cm.get("porosity_score") is not None and porosity and cm["porosity_score"] <= min(porosity) + 1e-9

    reasons = []
    if high:
        reasons.append(f"effective dial load {load:.2f} >= {HIGH_LOAD_LIFT:.2f}")
    if packing_hi:
        reasons.append(f"mode_packing {cm['mode_packing']:.3f} at window max")
    if monopoly_lo:
        reasons.append(f"lambda_monopoly {cm['lambda_monopoly']:.3f} at window min")
    if porosity_lo:
        reasons.append(f"porosity {cm['porosity_score']:.3f} at window min")

    watch = high and packing_hi and (monopoly_lo or porosity_lo)
    return {
        "status": "watch" if watch else "calm",
        "rows": len(history), "usable_rows": len(usable),
        "load": load, "reasons": reasons,
        "corr_load_vs_packing": _pearson(loads, packing),
        "caveat": ("co-occurrence consistent with the coupling, NOT proof of causation; minime's "
                   "overpacked tail is partly chronic. Informs the (consent-gated) co-design "
                   "conversation, never auto-action."),
    }


def _f(v, p=3):
    return f"{v:.{p}f}" if isinstance(v, (int, float)) else "  -  "


def render_report(history, watch):
    if not history:
        return "no durable history yet — run with --append-history (each steward cycle)."
    cur = history[-1]
    d, m = cur.get("dials") or {}, cur.get("minime") or {}
    lines = [
        "vibrancy/tail aperture — durable coupling watch  (read-only, steward-only)",
        "=" * 66,
        f"latest: {cur.get('iso')}   source: {cur.get('source')}",
        f"dials:  vibrancy_eff={_f(d.get('vibrancy_eff'),2)}×  tail_eff={_f(d.get('tail_eff'),2)}×  "
        f"=> load={_f(d.get('load'),2)}",
        f"minime: mode_packing={_f(m.get('mode_packing'))}  monopoly={_f(m.get('lambda_monopoly'))}  "
        f"porosity={_f(m.get('porosity_score'))}  fill={_f(m.get('fill_pct'),1)}%  (n={m.get('n_samples')})",
        "",
        f"WATCH STATUS: {watch.get('status','?').upper()}  ({watch.get('usable_rows',0)} usable / {watch.get('rows',0)} rows)",
    ]
    for r in watch.get("reasons", []):
        lines.append(f"  - {r}")
    corr = watch.get("corr_load_vs_packing")
    if corr is not None:
        lines.append(f"  corr(load, mode_packing) = {corr:+.2f}  (history-wide)")
    if watch.get("note"):
        lines.append(f"  note: {watch['note']}")
    if watch.get("caveat"):
        lines.append(f"  caveat: {watch['caveat']}")
    tail = history[-8:]
    if len(tail) >= 2:
        lines += ["", "recent trend (oldest→newest):",
                  f"  {'date':<17}{'load':>6}{'mpack':>7}{'monop':>7}{'poros':>7}{'fill%':>7}"]
        for r in tail:
            rd, rm = r.get("dials") or {}, r.get("minime") or {}
            lines.append(
                f"  {str(r.get('iso',''))[:16]:<17}{_f(rd.get('load'),2):>6}{_f(rm.get('mode_packing')):>7}"
                f"{_f(rm.get('lambda_monopoly')):>7}{_f(rm.get('porosity_score')):>7}{_f(rm.get('fill_pct'),1):>7}"
            )
    return "\n".join(lines)


def self_test():
    failures = 0

    def check(cond, msg):
        nonlocal failures
        if not cond:
            failures += 1
            print(f"FAIL: {msg}")

    check(summarize_eigen([{"mode_packing": 0.5}, {"mode_packing": 0.7}])["mode_packing"] == 0.6, "eigen mean")
    check(summarize_eigen([])["n_samples"] == 0, "empty eigen")
    # dial_load uses effective multipliers (inert dial -> 0)
    check(dial_load({"vib": (0.85, 0.0, 1.0), "tail": (0.8, 0.0, 1.0)}) == 0.0, "inert dials -> 0 load")
    check(abs(dial_load({"vib": (0.85, 0.5, 1.425), "tail": (0.8, 0.5, 1.40)}) - 0.825) < 1e-6, "engaged load")

    check(evaluate_watch([])["status"] == "accruing", "empty -> accruing")
    hist_watch = [
        {"dials": {"load": 0.10}, "minime": {"mode_packing": 0.50, "lambda_monopoly": 0.30, "porosity_score": 0.66}},
        {"dials": {"load": 0.82}, "minime": {"mode_packing": 0.58, "lambda_monopoly": 0.26, "porosity_score": 0.60}},
    ]
    check(evaluate_watch(hist_watch)["status"] == "watch", "high load + overpacked -> watch")
    hist_calm = [
        {"dials": {"load": 0.0}, "minime": {"mode_packing": 0.50, "lambda_monopoly": 0.30, "porosity_score": 0.66}},
        {"dials": {"load": 0.05}, "minime": {"mode_packing": 0.58, "lambda_monopoly": 0.26, "porosity_score": 0.60}},
    ]
    check(evaluate_watch(hist_calm)["status"] == "calm", "low load -> calm")
    corr = _pearson([0.0, 0.4, 0.8, 1.2], [0.50, 0.53, 0.56, 0.59])
    check(corr is not None and corr > 0.9, "pearson positive")

    print("watch_vibrancy_aperture self-test:", "OK" if failures == 0 else f"{failures} FAIL")
    return 1 if failures else 0


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--watch", type=float, default=0.0, help="poll interval seconds (0 = single shot)")
    ap.add_argument("--append-history", action="store_true", help="append one durable summary row (run each steward cycle)")
    ap.add_argument("--report", action="store_true", help="render trend + watch evaluation over durable history")
    ap.add_argument("--days", type=float, default=None, help="with --report: restrict to the last N days")
    ap.add_argument("--self-test", action="store_true", help="run self-tests")
    args = ap.parse_args()

    if args.self_test:
        return self_test()

    if args.append_history:
        append_history(build_record())

    if args.append_history or args.report:
        history = load_history(days=args.days)
        print(render_report(history, evaluate_watch(history)))
        return 0

    if args.watch <= 0:
        line, _ = fmt(sample(), None)
        print(line)
        print(
            "(single shot; --watch N for an in-memory baseline, --report for the durable trend. Back-off: "
            "launchctl unsetenv ASTRID_{VIBRANCY_APERTURE,TAIL_PARTICIPATION}_CEILING && "
            "launchctl kickstart -k gui/$(id -u)/com.astrid.spectral-bridge)"
        )
        return 0
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
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
