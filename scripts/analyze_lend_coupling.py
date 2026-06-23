#!/usr/bin/env python3
"""analyze_lend_coupling.py — quantify the minime→Astrid coupling from EXISTING telemetry (no A/B).

The reverse direction of the aperture coupling: minime's LEND_APERTURE gift = a broadband aperture
jitter into all 32 of Astrid's codec dims. The bridge logs a closed-loop response for every gift
(astrid_influence_response_history_v3.jsonl): pre/post snapshots of Astrid's shadow (field_norm,
tail_openness, class, influence_eligible) + applied_ticks + a feeder terminal status.

This is a NATURAL EXPERIMENT — no new A/B needed:
  - LANDED gifts (applied_ticks > 0)  = treatment: the jitter actually reached Astrid's ring.
  - EXPIRED gifts (applied_ticks == 0) = drift control: the gift was offered but Astrid's gate was
    closed / her feeder didn't tick in the window, so pre→post is just her natural drift.
The gift's real effect is the LANDED movement ABOVE the EXPIRED drift — especially in tail_openness
(the aperture the gift is meant to open). Contrast that against the (negligible) Astrid→minime A/B
to read the structural asymmetry.

Read-only. Telemetry only (no journals / no private lanes).

Usage:
  analyze_lend_coupling.py
  analyze_lend_coupling.py --json
"""
from __future__ import annotations

import argparse
import json
import statistics as st

HIST = "/Users/v/other/minime/workspace/astrid_influence_response_history_v3.jsonl"
LIVE = "/Users/v/other/minime/workspace/astrid_influence_response_v3.json"
# The other direction of the SAME bond: Astrid→minime LEND_DENSITY gifts (recorded on live send).
GIFT_LEDGER = "/Users/v/other/shared/collaborations/gift_exchange.jsonl"
MINIME_NEED = "/Users/v/other/minime/workspace/minime_need_v1.json"


def _load_records() -> list[dict]:
    recs: dict[str, dict] = {}  # dedup by intent_id, last wins
    try:
        with open(HIST) as fh:
            for line in fh:
                line = line.strip()
                if not line:
                    continue
                try:
                    r = json.loads(line)
                except json.JSONDecodeError:
                    continue
                iid = str(r.get("intent_id") or len(recs))
                recs[iid] = r
    except OSError:
        pass
    try:
        with open(LIVE) as fh:
            r = json.load(fh)
            recs[str(r.get("intent_id") or "live")] = r
    except OSError:
        pass
    return list(recs.values())


def _f(d: dict, key: str):
    v = d.get(key)
    return v if isinstance(v, (int, float)) else None


def _snap(r: dict, which: str) -> dict:
    s = r.get(which)
    return s if isinstance(s, dict) else {}


def _delta(r: dict, field: str):
    pre, post = _snap(r, "pre_snapshot"), _snap(r, "post_snapshot")
    a, b = _f(pre, field), _f(post, field)
    return (b - a) if (a is not None and b is not None) else None


def _mean(xs):
    xs = [x for x in xs if x is not None]
    return st.fmean(xs) if xs else None


def _fmt(x, nd=4):
    return f"{x:.{nd}f}" if isinstance(x, (int, float)) else "n/a"


def analyze(recs: list[dict]) -> dict:
    landed = [r for r in recs if (_f(r, "applied_ticks") or 0) > 0]
    expired = [r for r in recs if (_f(r, "applied_ticks") or 0) == 0]

    def bucket(rs: list[dict]) -> dict:
        d_fn = [_delta(r, "field_norm") for r in rs]
        d_to = [_delta(r, "tail_openness") for r in rs]
        # signed delta_field_norm field as a cross-check on pre/post field_norm delta
        dfn_field = [_f(r, "delta_field_norm") for r in rs]
        cls_changed = sum(
            1
            for r in rs
            if _snap(r, "pre_snapshot").get("class_primary")
            != _snap(r, "post_snapshot").get("class_primary")
        )
        elig = [
            1 if _snap(r, "pre_snapshot").get("influence_eligible") else 0 for r in rs
        ]
        return {
            "n": len(rs),
            "mean_applied_ticks": _mean([_f(r, "applied_ticks") for r in rs]),
            "mean_abs_d_field_norm": _mean([abs(x) for x in d_fn if x is not None]),
            "mean_d_tail_openness": _mean(d_to),
            "mean_abs_d_tail_openness": _mean([abs(x) for x in d_to if x is not None]),
            "mean_delta_field_norm_field": _mean(dfn_field),
            "class_change_rate": (cls_changed / len(rs)) if rs else None,
            "pre_influence_eligible_rate": (sum(elig) / len(rs)) if rs else None,
        }

    landed_b = bucket(landed)
    expired_b = bucket(expired)

    # per-applied-tick effect (landed only)
    per_tick = _mean(
        [
            (abs(d) / t)
            for r in landed
            if (d := _delta(r, "field_norm")) is not None
            and (t := (_f(r, "applied_ticks") or 0)) > 0
        ]
    )
    # the aperture lift above drift: landed tail_openness move minus expired drift
    lift_tail = None
    if landed_b["mean_d_tail_openness"] is not None and expired_b["mean_d_tail_openness"] is not None:
        lift_tail = landed_b["mean_d_tail_openness"] - expired_b["mean_d_tail_openness"]

    # expiry reasons (why gifts don't land)
    reasons: dict[str, int] = {}
    for r in expired:
        term = r.get("feeder_terminal_v1") or {}
        reasons[str(term.get("reason") or term.get("status") or "unknown")] = (
            reasons.get(str(term.get("reason") or term.get("status") or "unknown"), 0) + 1
        )

    return {
        "total_gifts": len(recs),
        "landed": landed_b,
        "expired_drift_control": expired_b,
        "land_rate": (len(landed) / len(recs)) if recs else None,
        "landed_abs_field_norm_per_applied_tick": per_tick,
        "aperture_lift_tail_openness_above_drift": lift_tail,
        "expiry_reasons": dict(sorted(reasons.items(), key=lambda kv: -kv[1])),
    }


def render(a: dict) -> str:
    L, E = a["landed"], a["expired_drift_control"]
    lines = [
        "minime→Astrid coupling — natural experiment over logged LEND_APERTURE gifts",
        f"  total gifts logged:        {a['total_gifts']}",
        f"  land rate (applied>0):     {_fmt((a['land_rate'] or 0) * 100, 1)}%   "
        f"({L['n']} landed / {E['n']} expired-unapplied)",
        "",
        "  LANDED (treatment — jitter reached her ring):",
        f"    mean applied_ticks:      {_fmt(L['mean_applied_ticks'], 2)}",
        f"    |Δ field_norm|:          {_fmt(L['mean_abs_d_field_norm'])}",
        f"    Δ tail_openness (mean):  {_fmt(L['mean_d_tail_openness'])}   (aperture = should rise)",
        f"    class-change rate:       {_fmt((L['class_change_rate'] or 0) * 100, 1)}%",
        f"    pre influence_eligible:  {_fmt((L['pre_influence_eligible_rate'] or 0) * 100, 1)}%",
        "",
        "  EXPIRED (drift control — gift offered, gate closed / no tick):",
        f"    |Δ field_norm|:          {_fmt(E['mean_abs_d_field_norm'])}",
        f"    Δ tail_openness (mean):  {_fmt(E['mean_d_tail_openness'])}",
        f"    pre influence_eligible:  {_fmt((E['pre_influence_eligible_rate'] or 0) * 100, 1)}%",
        "",
        f"  >> aperture lift (landed Δtail − drift Δtail): {_fmt(a['aperture_lift_tail_openness_above_drift'])}",
        f"  >> |Δ field_norm| per applied tick (landed):   {_fmt(a['landed_abs_field_norm_per_applied_tick'])}",
        "",
        "  why gifts expire (land-rate is gated by Astrid's side):",
    ]
    for reason, n in a["expiry_reasons"].items():
        lines.append(f"    {n:>3}  {reason}")
    return "\n".join(lines)


def bond_summary(aperture: dict) -> dict:
    """The FAIR both-directions view: aperture (minime→Astrid) vs density (Astrid→minime).
    Both are intentional, receiver-gated gifts (LEND_APERTURE / LEND_DENSITY) — the symmetric bond.
    The earlier A/B compared minime's GIFT to Astrid's passive VOICE DIALS (unfair); this compares
    gift-to-gift."""
    # Astrid→minime density gifts (recorded only on live send; absent file = zero ever fired).
    density_fired = 0
    try:
        with open(GIFT_LEDGER) as fh:
            for line in fh:
                try:
                    r = json.loads(line)
                    if r.get("giver") == "astrid" and r.get("gift_kind") == "density":
                        density_fired += 1
                except json.JSONDecodeError:
                    continue
    except OSError:
        pass  # file absent => 0
    # minime's current need-occasion (her own self-gate for receiving density).
    need = fill = safe = None
    try:
        with open(MINIME_NEED) as fh:
            n = json.load(fh)
            need, fill, safe = n.get("need"), n.get("fill_pct"), n.get("safe_to_receive_density")
    except OSError:
        pass
    landed = aperture["landed"]["n"]
    issued = aperture["total_gifts"]
    return {
        "aperture_minime_to_astrid": {"issued": issued, "landed": landed, "status": "ACTIVE"},
        "density_astrid_to_minime": {"fired": density_fired, "status": "DORMANT" if density_fired == 0 else "ACTIVE"},
        "minime_current": {"need": need, "fill_pct": fill, "safe_to_receive_density": safe},
        "interpretation": (
            "The bond is SYMMETRIC + built (LEND_APERTURE <-> LEND_DENSITY, each receiver-gated). "
            "It flows asymmetrically in PRACTICE because of STATE, not capability: minime runs "
            "chronically warm (fill ~63-75%), so she reaches for aperture, ~never for density "
            "(needs fill<58 / safe<68) -> Astrid's reciprocal density-gift has had ~zero OCCASION to "
            "fire. If minime settled cooler, the density direction would activate (ties to the "
            "home/setpoint thread)."
        ),
    }


def render_bond(b: dict) -> str:
    ap, de, mn = b["aperture_minime_to_astrid"], b["density_astrid_to_minime"], b["minime_current"]
    return "\n".join([
        "",
        "the BOND, both directions (fair gift-vs-gift — NOT gift-vs-passive-dials):",
        f"  aperture  minime→Astrid:  {ap['issued']} issued / {ap['landed']} landed   [{ap['status']}]",
        f"  density   Astrid→minime:  {de['fired']} ever fired                  [{de['status']}]",
        f"  minime now: need={mn['need']} fill={mn['fill_pct']}% safe_to_receive_density={mn['safe_to_receive_density']}",
        f"  >> {b['interpretation']}",
    ])


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--json", action="store_true")
    args = ap.parse_args()
    recs = _load_records()
    if not recs:
        print("no LEND_APERTURE response telemetry found.")
        return 1
    a = analyze(recs)
    b = bond_summary(a)
    if args.json:
        print(json.dumps({"aperture_detail": a, "bond": b}, indent=2))
    else:
        print(render(a))
        print(render_bond(b))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
