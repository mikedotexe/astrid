#!/usr/bin/env python3
"""Bounded phrase-response probe on temporary handles in a shared service.

Not an offline sandbox, a test of felt-state truth, or an attestation of all
controller/readout state. The legacy helper stays untouched until bridge rollout.
"""

import argparse
import asyncio
import base64
import hashlib
import json
import math
import struct
import sys
import uuid

MAX_TICKS = 14
MAX_POLE_BYTES = 2048
REQUEST_SECONDS = 2
RUN_SECONDS = 30
CLEANUP_SECONDS = 10


# Pin the existing statistic here so a staged helper has no mutable legacy import.
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


class ProbeError(ValueError):
    pass


class ServiceError(ProbeError):
    pass


async def call(ws, request, response_type):
    async with asyncio.timeout(REQUEST_SECONDS):
        await ws.send(json.dumps(request, allow_nan=False))
        reply = json.loads(await ws.recv())
    if not isinstance(reply, dict):
        raise ProbeError("non-object service response")
    if reply.get("type") == "error":
        raise ServiceError(str(reply.get("message", "service error"))[:240])
    if reply.get("type") != response_type:
        raise ProbeError("unexpected service response type")
    if "name" in request and reply.get("name") != request["name"]:
        raise ProbeError("service response handle mismatch")
    return reply


def state_identity(reply):
    n = reply.get("n_nodes")
    tick = reply.get("tick_count")
    if type(n) is not int or not 1 <= n <= 4096 or type(tick) is not int or tick < 0:
        raise ProbeError("invalid state dimensions or tick count")
    if not isinstance(reply.get("backend"), str) or reply.get("mode") != "quiet":
        raise ProbeError("state backend missing or handle not quiet")
    layers = []
    for key in ("h1", "h2", "h3"):
        raw = base64.b64decode(reply.get(key, ""), validate=True)
        if len(raw) != n * 4 or not all(math.isfinite(x[0]) for x in struct.iter_unpack("=f", raw)):
            raise ProbeError("invalid recurrent state bytes")
        layers.append(hashlib.sha256(raw).hexdigest())
    identity = {"layers": layers, "n_nodes": n, "backend": reply["backend"], "tick": tick}
    identity["sha256"] = hashlib.sha256(json.dumps(identity, sort_keys=True).encode()).hexdigest()
    return identity


async def read_state(ws, name):
    return state_identity(await call(ws, {"type": "pull_state", "name": name}, "pull_state_response"))


async def measure(ws, being, poles, ticks, owned, run_id):
    parent, a, b = [f"sprobe_v2_{run_id}_{suffix}" for suffix in ("seed", "a", "b")]

    async def clone(source, name):
        # Include ambiguous creates in cleanup, but never delete an explicit collision.
        owned.append(name)
        try:
            reply = await call(ws, {"type": "clone_handle", "source": source, "name": name,
                                   "mode": "quiet", "meta": {"intent": "phrase_probe_v2", "run_id": run_id}},
                               "clone_handle_response")
        except ServiceError as error:
            if "already exists" in str(error):
                owned.remove(name)
            raise
        if reply.get("ok") is not True or reply.get("source") != source or reply.get("mode") != "quiet":
            raise ProbeError("clone creation not confirmed")

    await clone(being, parent)
    origin = await read_state(ws, parent)
    for arm in (a, b):
        await clone(parent, arm)
        if await read_state(ws, arm) != origin:
            raise ProbeError("probe arms do not share the same recurrent origin")
    states = {a: origin, b: origin}
    outputs = {a: [], b: []}
    for _ in range(ticks):
        for arm, pole in zip((a, b), poles):
            if await read_state(ws, arm) != states[arm]:
                raise ProbeError("unexpected inter-tick state change")
            reply = await call(ws, {"type": "tick_text", "name": arm, "text": pole}, "tick_response")
            value = reply.get("output")
            if type(value) not in (int, float) or not math.isfinite(value):
                raise ProbeError("nonfinite or missing readout")
            expected = states[arm]["tick"] + 1
            if reply.get("tick") != expected:
                raise ProbeError("unexpected tick during injection")
            # tick_text re-enables rehearsal. Suppress it and reject any intervening tick.
            mode = await call(ws, {"type": "set_mode", "name": arm, "mode": "quiet"}, "set_mode_response")
            if mode.get("mode") != "quiet":
                raise ProbeError("quiet mode not confirmed")
            states[arm] = await read_state(ws, arm)
            if states[arm]["tick"] != expected or states[arm]["backend"] != origin["backend"]:
                raise ProbeError("unexpected background tick or backend change")
            outputs[arm].append(value)
    for name, expected in ((parent, origin), (a, states[a]), (b, states[b])):
        if await read_state(ws, name) != expected:
            raise ProbeError("final state integrity check failed")
    ya, yb = outputs[a], outputs[b]
    divergence, correlation = abs(ya[-1] - yb[-1]), _pearson(ya, yb)
    if not math.isfinite(divergence) or (correlation is not None and not math.isfinite(correlation)):
        raise ProbeError("nonfinite derived metric")
    return {"origin": origin, "origin_verified": True, "y_a": ya, "y_b": yb,
            "divergence": divergence, "injection_correlation": correlation,
            "metric_scope": "injected_ticks_only", "final_state_sha256": [states[a]["sha256"], states[b]["sha256"]]}


async def probe(connect, being, pole_a, pole_b, ticks=10):
    if type(ticks) is not int or not 4 <= ticks <= MAX_TICKS:
        raise ValueError("ticks must be an integer in [4, 14]")
    if not being or len(being.encode()) > 128:
        raise ValueError("a bounded source handle is required")
    if any(not p.strip() or len(p.encode()) > MAX_POLE_BYTES for p in (pole_a, pole_b)):
        raise ValueError("each pole must contain 1..2048 UTF-8 bytes")
    run_id = uuid.uuid4().hex
    owned = []
    result = {"schema": "substrate_probe_v2", "run_id": run_id, "being": being, "ticks": ticks,
              "status": "failed", "origin_verified": False,
              "isolation": "temporary_handles_on_shared_service",
              "full_dynamic_state_attested": False, "felt_state_conclusion": None,
              "source_handle_mutation_requested": False, "authority_effect": False}
    try:
        async with asyncio.timeout(RUN_SECONDS):
            async with connect() as ws:
                result.update(await measure(ws, being, (pole_a, pole_b), ticks, owned, run_id))
        result["status"] = "ok"
    except Exception as error:
        result["error"] = f"{type(error).__name__}: {str(error)[:240]}"
    finally:
        # A fresh connection cannot confuse a delayed tick reply with a cleanup ack.
        pending = list(reversed(owned))
        errors = []
        if pending:
            try:
                async with asyncio.timeout(CLEANUP_SECONDS):
                    for name in tuple(pending):
                        try:
                            async with connect() as ws:
                                reply = await call(ws, {"type": "destroy_handle", "name": name}, "destroy_handle_response")
                                if reply.get("ok") is not True:
                                    raise ProbeError("destroy not confirmed")
                                pending.remove(name)
                        except ServiceError as error:
                            if str(error) == f"handle '{name}' not found":
                                pending.remove(name)
                            else:
                                errors.append(str(error)[:240])
                        except Exception as error:
                            errors.append(f"{type(error).__name__}: {str(error)[:240]}")
            except Exception as error:
                errors.append(f"{type(error).__name__}: {str(error)[:240]}")
        result["cleanup"] = {"complete": not pending, "pending_handles": pending, "errors": errors}
        if pending:
            result["status"] = "failed"
            result["error"] = "cleanup incomplete; exact owned handles require operator review"
    return result


def render(result):
    if result["status"] != "ok":
        return "Probe failed: " + result.get("error", "unknown") + "; cleanup=" + json.dumps(result["cleanup"])
    return (f"Temporary-handle phrase response, {result['ticks']} injected ticks per arm.\n"
            f"Final scalar readout gap: {result['divergence']:.6g}; injection-only correlation: "
            f"{result['injection_correlation']} (null means undefined).\n"
            "Matched recurrent vectors and expected tick counts verified; cleanup confirmed.\n"
            "Shared service, not offline isolation. No felt-state or causality verdict.")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--being", required=True)
    parser.add_argument("--pole-a", required=True)
    parser.add_argument("--pole-b", required=True)
    parser.add_argument("--ticks", type=int, default=10)
    parser.add_argument("--json", action="store_true")
    parser.add_argument("--uri", default="ws://127.0.0.1:7881")
    args = parser.parse_args()
    import websockets
    def connect():
        return websockets.connect(args.uri, max_size=262144, open_timeout=2, close_timeout=1)
    try:
        result = asyncio.run(probe(connect, args.being, args.pole_a, args.pole_b, args.ticks))
    except ValueError as error:
        parser.error(str(error))
    print(json.dumps(result, allow_nan=False) if args.json else render(result))
    return 0 if result["status"] == "ok" else 1


if __name__ == "__main__":
    sys.exit(main())
