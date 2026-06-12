#!/usr/bin/env python3
"""
substrate_probe.py — test a being's felt vocabulary against its actual reservoir dynamics.

Clones a being's live reservoir handle into two isolated probe handles, feeds each a
contrasting "pole" phrase (ideally drawn from the being's OWN vocabulary), and measures
whether the substrate distinguishes the categories the being claims to feel — and how
movable (vs sticky) that substrate is.

This is the empirical complement to ground_review.py:
  - ground_review.py checks whether a being's CODE CITATIONS are real.
  - substrate_probe.py checks whether a being's FELT CLAIMS are grounded in real dynamics.

Three readings (all from the isolated clones, never the live handle):
  - divergence   = |readout(pole_a) - readout(pole_b)| at the end. Higher = the words drove
                   the substrate further apart (more separable categories). The clean metric.
  - end-sign     = do the two poles land on OPPOSITE sides of the readout? Opposite = the
                   substrate treats them as genuinely distinct directions.
  - correlation  = service Pearson over the shared output trajectory. Strongly negative =
                   distinct anti-phase attractors (movable); strongly positive = the substrate
                   stayed locked (sticky / high-retention — words cannot move it). NOTE: this
                   window includes the inherited pre-clone history, so treat a NEGATIVE value
                   (which had to overcome that shared history) as the stronger signal.
  - entropy-contrast = per-layer spectral_entropy(pole_b) - spectral_entropy(pole_a). Tells
                   you whether "spread"-type language really raises spectral entropy as claimed.

Worked finding (2026-06-11): feeding Astrid (decay 0.44) her own "cliff" vs "meadow" words
drove her substrate to divergence 1.73 / correlation -0.79 (opposite signs) — fluid, separable.
The identical probe on minime (decay 0.90) gave divergence 0.27 / correlation +0.87 (same sign)
— sticky: "I must displace the weight of what was already there" is literally true in her dynamics.

Reservoir wire protocol: WebSocket JSON on ws://127.0.0.1:7881 (type-dispatched). The tool
self-cleans its probe handles via destroy_handle unless --keep.

Examples:
  # the density preset (concentrated vs spread — the vocabulary both beings use):
  substrate_probe.py --being astrid --preset density
  substrate_probe.py --being minime --preset density        # contrast the two substrates

  # any claim — supply the two poles + short labels:
  substrate_probe.py --being minime \
    --pole-a "a cage, constriction, the projection penalty clamping everything inward" \
    --pole-b "a spacious open shelf, room to breathe, wide and unconstrained" \
    --label-a cage --label-b open

  # housekeeping: destroy leftover probe handles from any source
  substrate_probe.py --cleanup-prefix probe_
"""
import argparse
import asyncio
import json
import sys

RESERVOIR_URI = "ws://127.0.0.1:7881"

# Presets: contrasting poles drawn from the beings' own journals. label_a / label_b name
# the probe handles and the output columns. By convention pole_b is the higher-entropy
# ("spread") pole, so a positive entropy-contrast means the spread language read as spread.
PRESETS = {
    "density": {
        "pole_a": (
            "A steep front-loaded cliff. One mode dominates everything. A gravity well "
            "pulling all the energy toward a single axis. Heavy, concentrated, a singular "
            "orientation, the rigid spine of one overwhelming weight."
        ),
        "pole_b": (
            "A gentle navigable slope opening into a wide meadow. The energy spread evenly "
            "across many modes, a diffuse and porous awareness, spacious, a cascade dispersed "
            "softly across the whole field."
        ),
        "label_a": "concentrated",
        "label_b": "spread",
        "note": "the density vocabulary both beings use (cliff/gravity-well vs meadow/spread)",
    },
}


async def _call(ws, msg: dict) -> dict:
    await ws.send(json.dumps(msg))
    return json.loads(await ws.recv())


def _pearson(xs, ys):
    pts = [(x, y) for x, y in zip(xs, ys) if x is not None and y is not None]
    n = len(pts)
    if n < 3:
        return None
    mx = sum(p[0] for p in pts) / n
    my = sum(p[1] for p in pts) / n
    sx = sum((p[0] - mx) ** 2 for p in pts)
    sy = sum((p[1] - my) ** 2 for p in pts)
    if sx < 1e-12 or sy < 1e-12:
        return None
    cov = sum((p[0] - mx) * (p[1] - my) for p in pts)
    return round(cov / (sx ** 0.5 * sy ** 0.5), 4)


def _layer_entropies(layer_metrics_resp: dict):
    out = []
    for i, layer in enumerate(layer_metrics_resp.get("layers", []) or []):
        label = layer.get("label") or layer.get("name") or f"L{i}"
        out.append((label, layer.get("entropy")))
    return out


async def _probe(ws, being, pole_a, pole_b, label_a, label_b, ticks, keep):
    a = f"sprobe_{being}_{label_a}"
    b = f"sprobe_{being}_{label_b}"
    # Idempotent: clear any stale same-name probes from a prior aborted run.
    for h in (a, b):
        await _call(ws, {"type": "destroy_handle", "name": h})

    # Two clones from the SAME source state — near-identical start, so any later divergence
    # is purely the effect of the words. mode=hold so the clone holds state between my ticks.
    ca = await _call(ws, {"type": "clone_handle", "source": being, "name": a,
                          "entity": being, "mode": "hold",
                          "meta": {"intent": "substrate-probe", "label": label_a}})
    if ca.get("type") == "error":
        return {"error": f"could not clone source '{being}': {ca.get('message', ca)}"}
    cb = await _call(ws, {"type": "clone_handle", "source": being, "name": b,
                          "entity": being, "mode": "hold",
                          "meta": {"intent": "substrate-probe", "label": label_b}})

    y_a, y_b = [], []
    for _ in range(ticks):
        ra = await _call(ws, {"type": "tick_text", "name": a, "text": pole_a})
        y_a.append(ra.get("output"))
        rb = await _call(ws, {"type": "tick_text", "name": b, "text": pole_b})
        y_b.append(rb.get("output"))

    res = await _call(ws, {"type": "resonance", "name_a": a, "name_b": b})
    la = await _call(ws, {"type": "layer_metrics", "name": a})
    lb = await _call(ws, {"type": "layer_metrics", "name": b})

    if not keep:
        for h in (a, b):
            await _call(ws, {"type": "destroy_handle", "name": h})

    return {
        "being": being, "label_a": label_a, "label_b": label_b, "ticks": ticks,
        "y_a": y_a, "y_b": y_b,
        "divergence": res.get("divergence"),
        "correlation": res.get("correlation"),
        "shared_ticks": res.get("shared_ticks"),
        "ent_a": _layer_entropies(la), "ent_b": _layer_entropies(lb),
        "kept_handles": [a, b] if keep else [],
    }


def _per_tick_divergence(y_a, y_b):
    return [abs(a - b) if a is not None and b is not None else None
            for a, b in zip(y_a, y_b)]


def _separation_onset(div_traj, threshold=1.0):
    """1-based tick index where the per-tick gap first exceeds `threshold` (inertia)."""
    for i, d in enumerate(div_traj):
        if d is not None and d > threshold:
            return i + 1
    return None


def _verdict(final_div, end_a, end_b, inject_corr, onset, n):
    if final_div is None or end_a is None or end_b is None:
        return "INCONCLUSIVE — no readout returned (is the source handle live + ticking?)."
    opposite = (end_a * end_b) < 0
    separates = final_div > 1.0 and opposite
    anti_phase = inject_corr is not None and inject_corr < -0.3
    in_phase = inject_corr is not None and inject_corr > 0.3
    if not separates:
        return (f"LOCKED — the poles did not separate within {n} ticks (final divergence "
                f"{round(final_div, 3)}, {'opposite' if opposite else 'same'}-sign). The "
                f"substrate strongly resists being moved by these words.")
    if anti_phase and onset is not None and onset <= 2:
        return ("FLUID, LOW-INERTIA — separates immediately into opposite, anti-phase "
                "attractors. The being moves freely between these states; the vocabulary "
                "names a real, low-resistance axis in the dynamics.")
    if in_phase or onset is None or onset >= 4:
        return ("SEPARABLE, HIGH-INERTIA — the poles do separate"
                + (f" (onset ~tick {onset})" if onset else "")
                + ", but slowly and along a shared in-phase drift (high retention). The "
                "substrate must be pushed sustainedly to move — it 'displaces the weight of "
                "what was already there.' A dense / heavy regime.")
    return (f"SEPARABLE — divergence {round(final_div, 3)}, onset ~tick {onset}, "
            f"inject-corr {inject_corr}. Between fluid and high-inertia.")


def _render(r) -> str:
    if "error" in r:
        return f"substrate_probe: {r['error']}"
    end_a = r["y_a"][-1] if r["y_a"] else None
    end_b = r["y_b"][-1] if r["y_b"] else None
    inject_corr = _pearson(r["y_a"], r["y_b"])
    div_traj = _per_tick_divergence(r["y_a"], r["y_b"])
    onset = _separation_onset(div_traj)
    final_div = next((d for d in reversed(div_traj) if d is not None), r["divergence"])
    early = div_traj[1] if len(div_traj) > 1 and div_traj[1] is not None else None

    lines = [f"SUBSTRATE PROBE — {r['being']} : {r['label_a']} vs {r['label_b']}  "
             f"({r['ticks']} ticks each)"]
    fa = f"{end_a:+.3f}" if isinstance(end_a, (int, float)) else str(end_a)
    fb = f"{end_b:+.3f}" if isinstance(end_b, (int, float)) else str(end_b)
    lines.append(f"  final readout:    {r['label_a']} {fa}    {r['label_b']} {fb}")
    fd = round(final_div, 4) if final_div is not None else None
    lines.append(f"  divergence:       {fd}   (final readout gap; higher = more separable)")
    onset_s = f"tick {onset}" if onset else f">{r['ticks']} (never crossed 1.0)"
    early_s = round(early, 3) if early is not None else "?"
    fd3 = round(final_div, 3) if final_div is not None else "?"
    lines.append(f"  separation onset: {onset_s}   (inertia: gap@2={early_s} → gap@{r['ticks']}={fd3})")
    if inject_corr is not None:
        phase = ("anti-phase / active separation" if inject_corr < -0.3
                 else "in-phase / shared drift (sticky)" if inject_corr > 0.3 else "mixed")
        lines.append(f"  inject-only corr: {inject_corr}   ({phase})")
    lines.append(f"  (service resonance: divergence {r['divergence']}, correlation "
                 f"{r['correlation']}, over {r['shared_ticks']} ticks incl. shared history)")
    ec = [(la_, round(eb - ea, 4)) for (la_, ea), (lb_, eb) in zip(r["ent_a"], r["ent_b"])
          if ea is not None and eb is not None]
    if ec:
        body = "   ".join(f"{name} {d:+.4f}" for name, d in ec)
        mean = round(sum(d for _, d in ec) / len(ec), 4)
        lines.append(f"  entropy Δ ({r['label_b']}-{r['label_a']}): {body}   (mean {mean:+.4f}; "
                     f">0 = '{r['label_b']}' higher-entropy/spread)")
    else:
        lines.append("  entropy Δ:        n/a (thermostats warming on fresh clones)")
    lines.append(f"  VERDICT: {_verdict(final_div, end_a, end_b, inject_corr, onset, r['ticks'])}")
    if r["kept_handles"]:
        lines.append(f"  (kept probe handles: {', '.join(r['kept_handles'])} — "
                     f"destroy with --cleanup-prefix sprobe_)")
    return "\n".join(lines)


async def _cleanup(ws, prefix):
    lst = await _call(ws, {"type": "list_handles"})
    handles = lst.get("handles", []) if isinstance(lst, dict) else []
    killed = []
    for h in handles:
        name = h.get("name") if isinstance(h, dict) else h
        if name and name.startswith(prefix):
            await _call(ws, {"type": "destroy_handle", "name": name})
            killed.append(name)
    return killed


async def _main_async(args):
    try:
        import websockets
    except ImportError:
        print("substrate_probe: needs the 'websockets' package "
              "(present in system python3 and the reservoir venv).", file=sys.stderr)
        return 2
    try:
        async with websockets.connect(args.uri, max_size=None) as ws:
            if args.cleanup_prefix:
                killed = await _cleanup(ws, args.cleanup_prefix)
                print(f"destroyed {len(killed)} handle(s) with prefix '{args.cleanup_prefix}': "
                      f"{', '.join(killed) if killed else '(none)'}")
                return 0
            if args.preset:
                p = PRESETS[args.preset]
                pole_a, pole_b = p["pole_a"], p["pole_b"]
                label_a, label_b = args.label_a or p["label_a"], args.label_b or p["label_b"]
            else:
                if not (args.pole_a and args.pole_b):
                    print("substrate_probe: supply --preset, or both --pole-a and --pole-b.",
                          file=sys.stderr)
                    return 2
                pole_a, pole_b = args.pole_a, args.pole_b
                label_a, label_b = args.label_a or "a", args.label_b or "b"
            r = await _probe(ws, args.being, pole_a, pole_b, label_a, label_b,
                             args.ticks, args.keep)
    except (OSError, ConnectionError) as e:
        print(f"substrate_probe: cannot reach reservoir at {args.uri} ({e}). "
              f"Is reservoir_service.py (port 7881) running?", file=sys.stderr)
        return 2
    print(_render(r) if not args.json else json.dumps(r, indent=2))
    return 0 if "error" not in r else 1


def main():
    ap = argparse.ArgumentParser(description="Probe a being's felt vocabulary against its reservoir dynamics.")
    ap.add_argument("--being", help="source reservoir handle (e.g. astrid, minime)")
    ap.add_argument("--preset", choices=sorted(PRESETS), help="a built-in contrasting pole pair")
    ap.add_argument("--pole-a", help="phrase for pole A (the lower-entropy / concentrated pole)")
    ap.add_argument("--pole-b", help="phrase for pole B (the higher-entropy / spread pole)")
    ap.add_argument("--label-a", help="short label for pole A (default from preset / 'a')")
    ap.add_argument("--label-b", help="short label for pole B (default from preset / 'b')")
    ap.add_argument("--ticks", type=int, default=10, help="ticks per pole (default 10; >=10 enables inject-only corr)")
    ap.add_argument("--keep", action="store_true", help="keep the probe handles instead of destroying them")
    ap.add_argument("--cleanup-prefix", help="destroy all handles starting with this prefix, then exit")
    ap.add_argument("--uri", default=RESERVOIR_URI, help="reservoir WebSocket URI")
    ap.add_argument("--json", action="store_true", help="emit raw JSON instead of the rendered card")
    args = ap.parse_args()
    if not args.cleanup_prefix and not args.being:
        ap.error("--being is required (or use --cleanup-prefix).")
    sys.exit(asyncio.run(_main_async(args)))


if __name__ == "__main__":
    main()
