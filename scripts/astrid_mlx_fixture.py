"""Frozen reservoir transport fixture for isolated MLX qualification.

This is deliberately not the live reservoir service: no feeders, rehearsal,
thermostats, persistence, or external connections run here. Every new handle
starts from the same captured state, and every process starts a fresh trial.
"""
from __future__ import annotations

import base64
import copy
import json
import threading
from pathlib import Path


FIXTURE_HOST = "127.0.0.1"
FIXTURE_PORT = 17881
FIXTURE_URL = f"ws://{FIXTURE_HOST}:{FIXTURE_PORT}"
MAX_HANDLES = 128
N_NODES = 192


class FrozenReservoir:
    def __init__(self, snapshot: Path):
        import numpy as np

        with np.load(snapshot, allow_pickle=False) as data:
            if str(data["entity"]) != "astrid":
                raise ValueError("fixture requires an Astrid snapshot")
            layers = {key: np.asarray(data[key], dtype=np.float32) for key in ("h1", "h2", "h3")}
            if any(value.shape != (1, N_NODES) or not np.isfinite(value).all() for value in layers.values()):
                raise ValueError("fixture layers must be finite 1x192 arrays")
            ticks = int(data["tick_count"])
            if ticks < 0:
                raise ValueError("snapshot tick count must be nonnegative")
        self.initial = {key: base64.b64encode(value.tobytes()).decode("ascii") for key, value in layers.items()}
        self.initial.update(n_nodes=N_NODES, tick_count=ticks)
        self.handles = {"astrid": copy.deepcopy(self.initial)}
        self.lock = threading.Lock()

    def dispatch(self, message):
        import numpy as np

        if not isinstance(message, dict):
            raise ValueError("request must be an object")
        operation = message.get("type")
        if operation == "fixture_status":
            return {"type": "fixture_status", "isolated": True, "handles": len(self.handles)}
        name = message.get("name")
        if not isinstance(name, str) or not 1 <= len(name) <= 160:
            raise ValueError("invalid handle name")
        with self.lock:
            if operation == "create_handle":
                if name in self.handles or len(self.handles) >= MAX_HANDLES:
                    raise ValueError("handle exists or fixture capacity reached")
                self.handles[name] = copy.deepcopy(self.initial)
                return {"type": "create_handle_response", "name": name, "ticks": self.initial["tick_count"]}
            if name not in self.handles:
                raise ValueError(f"handle '{name}' not found")
            if operation == "pull_state":
                return {"type": "pull_state_response", "name": name, **self.handles[name]}
            if operation != "push_state":
                raise ValueError("unsupported fixture operation")
            delta = message.get("tick_delta", 0)
            if isinstance(delta, bool) or not isinstance(delta, int) or not 0 <= delta <= 100_000:
                raise ValueError("invalid tick delta")
            layers = {}
            norms = []
            for key in ("h1", "h2", "h3"):
                encoded = message.get(key)
                if not isinstance(encoded, str) or len(encoded) > 2048:
                    raise ValueError("invalid encoded reservoir layer")
                raw = base64.b64decode(encoded, validate=True)
                if len(raw) != N_NODES * 4:
                    raise ValueError("wrong reservoir layer size")
                values = np.frombuffer(raw, dtype=np.float32)
                if not np.isfinite(values).all():
                    raise ValueError("nonfinite reservoir layer")
                layers[key] = encoded
                norms.append(float(np.linalg.norm(values)))
            self.handles[name] = {**layers, "n_nodes": N_NODES,
                                  "tick_count": self.handles[name]["tick_count"] + delta}
            return {"type": "push_state_response", "ok": True, "h_norms": norms}


def start_fixture(snapshot: Path):
    """Bind the sole private fixture port; fail before model import if occupied."""
    from websockets.sync.server import serve

    fixture = FrozenReservoir(snapshot)

    def handler(connection):
        try:
            message = json.loads(connection.recv(timeout=5))
            response = fixture.dispatch(message)
        except (ValueError, KeyError, TypeError, TimeoutError) as error:
            response = {"type": "error", "message": str(error)}
        connection.send(json.dumps(response, allow_nan=False))

    server = serve(handler, FIXTURE_HOST, FIXTURE_PORT, max_size=128 * 1024,
                   open_timeout=5, close_timeout=2)
    thread = threading.Thread(target=server.serve_forever, name="private-reservoir", daemon=True)
    thread.start()
    return server, thread
