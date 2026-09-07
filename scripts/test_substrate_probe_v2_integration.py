"""Real local WebSocket dispatch + NumPy reservoir; synthetic handles only.

Run with the neural-triple-reservoir virtualenv. No live snapshots, model calls,
service ports or live handles. Rehearsal interference is injected deliberately
in the negative case, through the real background loop.
"""
import asyncio
import os
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

import substrate_probe_v2 as probe

NEURAL = Path(os.environ.get("NEURAL_RESERVOIR_ROOT", "/Users/v/other/neural-triple-reservoir")).resolve()
sys.path.insert(0, str(NEURAL))
import reservoir_service as reservoir
import websockets
from websockets.asyncio.server import serve


class RealProbeIntegrationTests(unittest.IsolatedAsyncioTestCase):
    async def exercise(self, interference=False):
        with tempfile.TemporaryDirectory(prefix="probe-real-service-") as tmp:
            service = reservoir.ReservoirService(input_dim=32, n_nodes=32, state_dir=Path(tmp), backend="numpy")
            service.create_handle("synthetic_source", "test")
            service.tick_text("synthetic_source", "synthetic initial history")
            service.set_mode("synthetic_source", "quiet")
            source_before = probe.state_identity(service.pull_handle_state("synthetic_source"))
            async with serve(service.ws_handler, "127.0.0.1", 0) as server:
                port = server.sockets[0].getsockname()[1]
                self.assertNotIn(port, {7878, 7879, 7880, 7881, 8090, 11434})
                uri = f"ws://127.0.0.1:{port}"
                def connect():
                    return websockets.connect(uri, max_size=262144, open_timeout=2, close_timeout=1)
                original = probe.call
                async def delayed_mode(ws, request, response_type):
                    if request["type"] == "set_mode" and interference:
                        await asyncio.sleep(0.025)
                    return await original(ws, request, response_type)
                with patch.object(reservoir, "REHEARSAL_INTERVAL", 0.005):
                    rehearsal = asyncio.create_task(service.rehearsal_loop()) if interference else None
                    try:
                        with patch.object(probe, "call", delayed_mode):
                            result = await probe.probe(connect, "synthetic_source", "alpha", "beta", ticks=4)
                    finally:
                        service._shutdown.set()
                        if rehearsal:
                            await rehearsal
            self.assertEqual(set(service.handles), {"synthetic_source"})
            self.assertTrue(result["cleanup"]["complete"])
            self.assertEqual(source_before, probe.state_identity(service.pull_handle_state("synthetic_source")))
            self.assertFalse(result["source_handle_mutation_requested"])
            self.assertIsNone(result["felt_state_conclusion"])
            return result

    async def test_real_dispatch_recurrent_origin_and_cleanup(self):
        result = await self.exercise()
        self.assertEqual(result["status"], "ok", result)
        self.assertTrue(result["origin_verified"])
        self.assertEqual(result["origin"]["backend"], "numpy")
        self.assertEqual(len(result["y_a"]), 4)

    async def test_real_rehearsal_interference_fails_and_cleans(self):
        result = await self.exercise(interference=True)
        self.assertEqual(result["status"], "failed")
        self.assertIn("background tick", result["error"])


if __name__ == "__main__":
    unittest.main()
