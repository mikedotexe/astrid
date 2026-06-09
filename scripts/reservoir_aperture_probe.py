#!/usr/bin/env python3
"""Clone-based live aperture probe — confirm the offline finding on the REAL
reservoir service, with ZERO impact on the beings' real handles.

Clones the live 'astrid' handle (inheriting its real weights/normalization/state),
drives the CLONE with impoverished (low-rank, smooth) vs enriched (full-rank,
fast-varying) input via `tick`, measures the participation-ratio delta, then
destroys the clone. We only ever tick the clone — never a real handle.

Run:  python3 /Users/v/other/astrid/scripts/reservoir_aperture_probe.py
"""

import asyncio
import base64
import json
import sys

import numpy as np

sys.path.insert(0, "/Users/v/other/astrid/scripts")
from reservoir_capacity_audit import cov_eigs, norm_spectral_entropy, participation_ratio  # noqa: E402

URI = "ws://127.0.0.1:7881"
SRC = "astrid"
CLONE = "astrid-aperture-probe"
DIM = 32
N_PER = 650
WASH = 120


def make_input(rng, T, dim, rank, var):
    P = rng.standard_normal((rank, dim))
    if var == "slow":
        latent = np.cumsum(rng.standard_normal((T, rank)) * 0.06, axis=0)
    else:  # iid
        latent = rng.standard_normal((T, rank))
    return np.tanh((latent @ P) / np.sqrt(rank)).astype(np.float32)


async def req(ws, msg):
    await ws.send(json.dumps(msg))
    return json.loads(await asyncio.wait_for(ws.recv(), timeout=10))


async def drive_collect(ws, name, X):
    h1s = []
    for row in X:
        await req(ws, {"type": "tick", "name": name, "input": row.tolist()})
        r = await req(ws, {"type": "pull_state", "name": name})
        if "h1" in r:
            h1s.append(np.frombuffer(base64.b64decode(r["h1"]), dtype=np.float32).copy())
    if len(h1s) <= WASH + 2:
        return None
    W = np.stack(h1s)[WASH:]
    ev = cov_eigs(W)
    return participation_ratio(ev), norm_spectral_entropy(ev), len(h1s)


async def _fresh_clone(ws):
    try:
        await req(ws, {"type": "destroy_handle", "name": CLONE})
    except Exception:
        pass
    return await req(ws, {"type": "clone_handle", "source": SRC, "name": CLONE, "mode": "quiet"})


async def main():
    import websockets

    rng = np.random.default_rng(1)
    async with websockets.connect(URI, max_size=None) as ws:
        r0 = await req(ws, {"type": "pull_state", "name": SRC})
        tc0 = r0.get("tick_count")

        cr = await _fresh_clone(ws)
        if cr.get("type") != "clone_handle_response":
            print("clone failed:", cr)
            return 1
        try:
            x_imp = make_input(rng, N_PER, DIM, 4, "slow")
            imp = await drive_collect(ws, CLONE, x_imp)
            await _fresh_clone(ws)  # reset to a fair fresh start
            x_rich = make_input(rng, N_PER, DIM, 32, "iid")
            rich = await drive_collect(ws, CLONE, x_rich)
        finally:
            try:
                await req(ws, {"type": "destroy_handle", "name": CLONE})
            except Exception:
                pass

        r1 = await req(ws, {"type": "pull_state", "name": SRC})
        tc1 = r1.get("tick_count")

    print("CLONE-BASED LIVE APERTURE PROBE (real service dynamics; real handles never ticked by us)")
    if imp:
        print(f"  impoverished (rank4 slow):  PR={imp[0]:.1f}  Hnorm={imp[1]:.2f}  (M={imp[2]})")
    if rich:
        print(f"  enriched     (rank32 iid):  PR={rich[0]:.1f}  Hnorm={rich[1]:.2f}  (M={rich[2]})")
    if imp and rich:
        print(f"  delta: PR {imp[0]:.1f} -> {rich[0]:.1f}  ({rich[0] / max(imp[0], 0.1):.1f}x)")
    fed = (tc1 - tc0) if (isinstance(tc0, int) and isinstance(tc1, int)) else "?"
    print(f"  real '{SRC}' handle tick_count: before={tc0} after={tc1} "
          f"(delta {fed} = its own feeder, NOT this probe — we ticked only the clone)")
    return 0


if __name__ == "__main__":
    sys.exit(asyncio.run(main()))
