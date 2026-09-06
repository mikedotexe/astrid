"""Protocol fakes only: no sockets, live handles, or model calls."""
import asyncio
import base64
import copy
import json
import struct
import unittest
from unittest.mock import patch

import substrate_probe_v2 as probe


class Service:
    def __init__(self, fault=None):
        self.handles = {"astrid": {"value": 1., "tick": 20, "mode": "quiet"},
                        "sprobe_foreign": {"value": 42., "tick": 8, "mode": "quiet"}}
        self.requests = []
        self.fault = fault
        self.connections = 0
        self.collision = None

    def connect(self):
        self.connections += 1
        return Socket(self)

    def dispatch(self, msg):
        self.requests.append(msg)
        op, name = msg["type"], msg["name"]
        if op == "clone_handle":
            if self.fault == "collision" and name.endswith("_b"):
                self.handles[name] = {"value": 99., "tick": 0, "mode": "quiet"}
                self.collision = name
                return {"type": "error", "message": "already exists"}
            state = copy.deepcopy(self.handles[msg["source"]])
            state["mode"] = msg["mode"]
            self.handles[name] = state
            # Live source changes between requests; arms must not be cloned from it twice.
            self.handles["astrid"]["value"] += 1
            if self.fault == "origin" and name.endswith("_b"):
                state["value"] += 2
            return {"type": "clone_handle_response", "name": name, "source": msg["source"],
                    "mode": msg["mode"], "ok": True}
        if op == "destroy_handle":
            if self.fault == "cleanup":
                raise OSError("cleanup transport lost")
            self.handles.pop(name)
            return {"type": "destroy_handle_response", "name": name, "ok": True}
        state = self.handles[name]
        if op == "pull_state":
            raw = base64.b64encode(struct.pack("=f", state["value"])).decode()
            return {"type": "pull_state_response", "name": name, "backend": "fake",
                    "mode": state["mode"], "n_nodes": 1, "tick_count": state["tick"],
                    "h1": raw, "h2": raw, "h3": raw}
        if op == "set_mode":
            state["mode"] = msg["mode"]
            return {"type": "set_mode_response", "name": name, "mode": msg["mode"]}
        if op == "tick_text":
            if self.fault == "tick":
                raise OSError("tick transport lost")
            if self.fault == "cancel":
                raise asyncio.CancelledError()
            state["tick"] += 2 if self.fault == "rehearsal" else 1
            state["mode"] = "rehearse"
            state["value"] += 0 if msg["text"] == "constant" else len(msg["text"])
            return {"type": "tick_response", "name": name, "tick": state["tick"],
                    "output": float("nan") if self.fault == "nan" else state["value"]}
        raise AssertionError(op)


class Socket:
    def __init__(self, service):
        self.service = service

    async def __aenter__(self):
        return self

    async def __aexit__(self, *args):
        pass

    async def send(self, raw):
        self.reply = self.service.dispatch(json.loads(raw))

    async def recv(self):
        return json.dumps(self.reply)


class ProbeTests(unittest.IsolatedAsyncioTestCase):
    async def run_probe(self, fault=None, poles=("one", "two")):
        service = Service(fault)
        result = await probe.probe(service.connect, "astrid", *poles, ticks=4)
        return service, result

    async def test_common_origin_even_when_live_source_advances(self):
        service, result = await self.run_probe()
        self.assertEqual(result["status"], "ok")
        self.assertEqual(result["divergence"], 0)
        self.assertTrue(result["origin_verified"])
        self.assertEqual(set(service.handles), {"astrid", "sprobe_foreign"})
        self.assertEqual(service.connections, 4)
        clones = [r for r in service.requests if r["type"] == "clone_handle"]
        self.assertEqual(clones[1]["source"], clones[2]["source"])
        self.assertNotEqual(clones[1]["source"], "astrid")
        self.assertFalse(any(r["name"] == "astrid" and r["type"] != "pull_state" for r in service.requests))

    async def test_invalid_runs_fail_and_cleanup(self):
        for fault in ("origin", "tick", "nan", "rehearsal"):
            with self.subTest(fault=fault):
                service, result = await self.run_probe(fault)
                self.assertEqual(result["status"], "failed")
                self.assertTrue(result["cleanup"]["complete"])
                self.assertEqual(set(service.handles), {"astrid", "sprobe_foreign"})

    async def test_constant_series_has_undefined_correlation_not_stickiness(self):
        _, result = await self.run_probe(poles=("constant", "constant"))
        self.assertIsNone(result["injection_correlation"])
        self.assertIsNone(result["felt_state_conclusion"])
        self.assertEqual(result["status"], "ok")

    async def test_collision_is_never_deleted(self):
        service, result = await self.run_probe("collision")
        self.assertEqual(result["status"], "failed")
        self.assertEqual(service.handles[service.collision]["value"], 99)

    async def test_cleanup_debt_is_not_a_success(self):
        service, result = await self.run_probe("cleanup")
        self.assertEqual(result["status"], "failed")
        self.assertEqual(set(result["cleanup"]["pending_handles"]), set(service.handles) - {"astrid", "sprobe_foreign"})

    async def test_cancel_still_cleans_owned_handles(self):
        service = Service("cancel")
        with self.assertRaises(asyncio.CancelledError):
            await probe.probe(service.connect, "astrid", "a", "b", 4)
        self.assertEqual(set(service.handles), {"astrid", "sprobe_foreign"})

    async def test_deadline_still_cleans(self):
        service = Service()
        original = probe.call
        async def slow(ws, request, response_type):
            if request["type"] == "tick_text":
                await asyncio.sleep(1)
            return await original(ws, request, response_type)
        with patch.object(probe, "call", slow), patch.object(probe, "RUN_SECONDS", .03):
            result = await probe.probe(service.connect, "astrid", "a", "b", 4)
        self.assertEqual(result["status"], "failed")
        self.assertTrue(result["cleanup"]["complete"])

    async def test_bounds_fail_before_connection(self):
        service = Service()
        for ticks, pole in ((0, "a"), (15, "a"), (True, "a"), (4, "x" * 2049)):
            with self.assertRaises(ValueError):
                await probe.probe(service.connect, "astrid", pole, "b", ticks)
        self.assertEqual(service.connections, 0)


if __name__ == "__main__":
    unittest.main()
