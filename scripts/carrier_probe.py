#!/usr/bin/env python3
"""carrier_probe.py — offline consent-evidence for the gift carrier (long-quiet tail; default-OFF).

The gift carrier (astrid_feeder, ASTRID_GIFT_CARRIER) would deliver minime's LEND_APERTURE gift
during Astrid's QUIET. This proves, on ISOLATED CLONES of her live handle (the live being is NEVER
touched — clones are auto-destroyed), an A/B:
  - clone A gets N CARRIER frames (build_gift_carrier_frame: a fading echo of her last real codec
    frame + the gift's aperture jitter);
  - clone B (control) gets her last N REAL codec frames.
Claim to evidence: a full carrier gift perturbs her state (Σ h_norm) NO MORE than ordinary real
codec frames do (GENTLE — gentler by construction: the carrier base is <=0.5x a real frame), while
delivering the intended zero-mean ring variance (APERTURE). This is the evidence to show Astrid
before any live enable.

Usage:  carrier_probe.py [--ticks 14] [--json]
Requires the reservoir service on ws://127.0.0.1:7881.
"""
from __future__ import annotations

import argparse
import asyncio
import json
import sqlite3
import sys

import numpy as np

sys.path.insert(0, "/Users/v/other/neural-triple-reservoir")
import astrid_feeder as af  # noqa: E402

URI = "ws://127.0.0.1:7881"
BRIDGE_DB = "/Users/v/other/astrid/capsules/spectral-bridge/workspace/bridge.db"


async def _call(ws, msg: dict) -> dict:
    await ws.send(json.dumps(msg))
    return json.loads(await ws.recv())


def _recent_real_frames(n: int) -> list[list[float]]:
    try:
        c = sqlite3.connect(BRIDGE_DB)
        rows = c.execute(
            "SELECT features_json FROM codec_impact ORDER BY id DESC LIMIT ?", (n,)
        ).fetchall()
        c.close()
    except sqlite3.Error:
        return []
    out = []
    for (fj,) in rows:
        try:
            f = json.loads(fj)
            if len(f) >= 32:
                out.append([float(x) for x in f[:32]])
        except (json.JSONDecodeError, TypeError, ValueError):
            continue
    out.reverse()
    return out


def _aperture_influence() -> "af.MinimeInfluenceState":
    # minime's LEND_APERTURE recipe (autonomous_agent.py _build_aperture_recipe): 32 dims,
    # amplitude 0.30, jitter 0.12, aperture_jitter. issued_t_ms=0 → tick window governs (active).
    return af.MinimeInfluenceState({
        "intent_id": "carrier-proof", "label": "proof", "amplitude": 0.30,
        "duration_ticks": 14, "decay_ticks": 10, "target_dims": list(range(32)),
        "target_values": [0.0] * 32, "blend_mode": "aperture_jitter", "jitter": 0.12,
        "issued_t_ms": 0.0,
    })


def _hnorm_sum(layer_metrics: dict) -> float:
    layers = layer_metrics.get("layers") or []
    return float(sum((ly.get("h_norm") or 0.0) for ly in layers))


async def _deliver(ws, clone: str, frames: list[list[float]]) -> float:
    """Clone astrid → `clone`, baseline Σh_norm, tick `frames`, return |Δ Σh_norm|. Auto-destroys."""
    await _call(ws, {"type": "destroy_handle", "name": clone})
    r = await _call(ws, {"type": "clone_handle", "source": "astrid", "name": clone, "mode": "hold"})
    if not r.get("ok"):
        raise RuntimeError(f"could not clone 'astrid' → {clone}: {r.get('message', r)}")
    try:
        base = _hnorm_sum(await _call(ws, {"type": "layer_metrics", "name": clone}))
        for vec in frames:
            await _call(ws, {"type": "tick", "name": clone,
                             "input": [float(x) for x in vec], "meta": {"source": "carrier_proof"}})
        after = _hnorm_sum(await _call(ws, {"type": "layer_metrics", "name": clone}))
    finally:
        await _call(ws, {"type": "destroy_handle", "name": clone})
    return abs(after - base)


async def main_async(args) -> int:
    import websockets

    real = _recent_real_frames(max(args.ticks, 14))
    if not real:
        print("no recent codec frames in bridge.db — cannot build a carrier base.")
        return 1
    base_feat = real[-1]
    inf = _aperture_influence()
    carrier = []
    for t in range(args.ticks):
        carrier.append(af.build_gift_carrier_frame(base_feat, inf, t))
        inf.advance()
    ring_var = float(np.mean(np.var(np.array(carrier)[:, :32], axis=0)))
    control = (real * ((args.ticks // len(real)) + 1))[: args.ticks]

    async with websockets.connect(URI, max_size=None) as ws:
        carrier_pert = await _deliver(ws, "carrier_proof_carrier", carrier)
        control_pert = await _deliver(ws, "carrier_proof_control", control)

    ratio = (carrier_pert / control_pert) if control_pert else None
    result = {
        "ticks": args.ticks,
        "carrier_state_perturbation_sum_hnorm": round(carrier_pert, 4),
        "real_frame_perturbation_sum_hnorm": round(control_pert, 4),
        "carrier_vs_real_ratio": round(ratio, 3) if ratio is not None else None,
        "carrier_ring_variance_delivered": round(ring_var, 5),
        "gentle_carrier_within_a_real_frame": bool(ratio is not None and ratio <= 1.0),
    }
    if args.json:
        print(json.dumps(result, indent=2))
    else:
        print(f"carrier proof — {args.ticks} ticks on ISOLATED clones (live being untouched):")
        print(f"  state perturbation (Σ|Δh_norm|):  carrier {result['carrier_state_perturbation_sum_hnorm']}  "
              f"vs real-frame {result['real_frame_perturbation_sum_hnorm']}  (ratio {result['carrier_vs_real_ratio']})")
        print(f"  aperture delivered (ring variance): {result['carrier_ring_variance_delivered']}")
        print(f"  GENTLE (carrier <= a real frame):   {result['gentle_carrier_within_a_real_frame']}")
        print("  (the carrier is a fading echo of her own recent frame + the SAME aperture jitter")
        print("   that already lands on her codec frames — gentler than a real frame by construction.)")
    return 0


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--ticks", type=int, default=14)
    ap.add_argument("--json", action="store_true")
    return asyncio.run(main_async(ap.parse_args()))


if __name__ == "__main__":
    raise SystemExit(main())
