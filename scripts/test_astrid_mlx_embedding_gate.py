"""CPU-only loading-policy tests using fake MLX modules and a tiny frozen test."""
from __future__ import annotations

from contextlib import contextmanager
from datetime import datetime
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import types
import unittest
from unittest.mock import patch

import astrid_mlx_embedding_gate as gate


class EmbeddingGateTests(unittest.TestCase):
    def setUp(self):
        # Other CPU suites may import the live server module in this pytest
        # process. Preserve their state while each fake frozen test starts fresh.
        modules = patch.dict(sys.modules)
        modules.start()
        self.addCleanup(modules.stop)
        for name in gate.SOURCE_NAMES:
            sys.modules.pop(Path(name).stem, None)
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name).resolve()
        self.source = self.root / "frozen source"
        self.source.mkdir()
        self.model = self.root / "model snapshot"
        self.model.mkdir()
        self.audit = self.root / "audit.json"
        self.forwarded = []
        self.reset_count = 0
        self.loader_failure = None
        self.mx = types.ModuleType("mlx.core")
        self.mx.get_active_memory = lambda: 20
        self.mx.get_cache_memory = lambda: 3
        self.mx.get_peak_memory = lambda: 23
        def reset():
            self.reset_count += 1
        self.mx.reset_peak_memory = reset
        self.lm = types.ModuleType("mlx_lm")
        def load(*args, **kwargs):
            self.forwarded.append((args, kwargs))
            if self.loader_failure:
                raise self.loader_failure
            return ("embedding-only", "tokenizer")
        self.original_load = load
        self.lm.load = load
        self.write_sources()

    def write_sources(self, *, shim_kwargs="model_config={'model_type':'gemma4'}", body=None):
        (self.source / "wide_coupling.py").write_text("SENTINEL = 'unchanged'\n")
        (self.source / "coupled_astrid_server.py").write_text(
            "def _load_mlx_runtime(model_name, *, memory_map_requested):\n"
            "    from mlx_lm import load\n"
            f"    return load(model_name, {shim_kwargs})\n")
        (self.source / "test_wide_coupling_offline.py").write_text(
            "import sys\nimport coupled_astrid_server as cas\nimport wide_coupling\n"
            "def main():\n" + (body or
            "    assert sys.argv[1] == '--model'\n"
            "    result = cas._load_mlx_runtime(sys.argv[2], memory_map_requested=False)\n"
            "    assert result == ('embedding-only', 'tokenizer')\n"
            "    assert wide_coupling.SENTINEL == 'unchanged'\n"))

    @contextmanager
    def fake_mlx(self):
        package = types.ModuleType("mlx")
        package.__path__ = []
        with patch.dict(sys.modules, {"mlx": package, "mlx.core": self.mx, "mlx_lm": self.lm}):
            yield

    def run_gate(self):
        before_path, before_argv = list(sys.path), list(sys.argv)
        with self.fake_mlx():
            status = gate.run_gate(self.source, self.model, self.audit)
        self.assertIs(self.lm.load, self.original_load)
        self.assertEqual(sys.path, before_path)
        self.assertEqual(sys.argv, before_argv)
        self.assertNotIn(gate.TEST_NAME, sys.modules)
        return status, json.loads(self.audit.read_text())

    def test_module_import_does_not_import_mlx(self):
        program = ("import sys; sys.path.insert(0, " + repr(str(Path(gate.__file__).parent)) + "); "
                   "import astrid_mlx_embedding_gate; "
                   "assert not any(n == 'mlx' or n.startswith('mlx.') or n == 'mlx_lm' for n in sys.modules)")
        subprocess.run([sys.executable, "-B", "-c", program], check=True, timeout=10)

    def test_exact_frozen_main_and_shim_load_once_lazily_with_bound_audit(self):
        before = {name: (self.source / name).read_bytes() for name in gate.SOURCE_NAMES}
        status, audit = self.run_gate()
        self.assertEqual(status, 0)
        self.assertTrue(audit["passed"])
        self.assertEqual(audit["schema"], "astrid_mlx_embedding_gate_v1")
        self.assertEqual(audit["loading_policy"], "lazy_embeddings_only")
        self.assertEqual((audit["loader_calls"], audit["load_attempts"]), (1, 1))
        self.assertEqual(self.forwarded, [((str(self.model),), {"model_config": {"model_type": "gemma4"}, "lazy": True})])
        self.assertTrue(audit["loader_restored"])
        self.assertEqual(self.reset_count, 1)
        self.assertEqual(audit["source_hashes"], {name: hashlib.sha256(raw).hexdigest() for name, raw in before.items()})
        self.assertEqual(before, {name: (self.source / name).read_bytes() for name in gate.SOURCE_NAMES})
        self.assertEqual(set(audit["memory"]), {"before_load", "after_load", "after_test"})
        self.assertTrue(all(row == {"active_bytes": 20, "cache_bytes": 3, "peak_bytes": 23}
                            for row in audit["memory"].values()))
        self.assertEqual(audit["pid"], os.getpid())
        self.assertEqual(set(audit["memory_timestamps"]), set(audit["memory"]))
        times = [datetime.fromisoformat(audit["started_at"])]
        times.extend(datetime.fromisoformat(audit["memory_timestamps"][stage])
                     for stage in ("before_load", "after_load", "after_test"))
        times.append(datetime.fromisoformat(audit["completed_at"]))
        self.assertTrue(all(timestamp.utcoffset().total_seconds() == 0 for timestamp in times))
        self.assertEqual(times, sorted(times))

    def test_original_test_assertion_failure_is_not_suppressed(self):
        self.write_sources(body="    cas._load_mlx_runtime(sys.argv[2], memory_map_requested=False)\n    assert False, 'original assertion'\n")
        status, audit = self.run_gate()
        self.assertEqual(status, 1)
        self.assertFalse(audit["passed"])
        self.assertIn("AssertionError: original assertion", audit["error"])
        self.assertEqual(audit["loader_calls"], 1)

    def test_loader_failure_and_interrupt_restore_loader_and_record_memory(self):
        for error in (RuntimeError("loader failed"), KeyboardInterrupt()):
            with self.subTest(error=type(error).__name__):
                self.audit = self.root / (type(error).__name__ + ".json")
                self.loader_failure = error
                status, audit = self.run_gate()
                self.assertNotEqual(status, 0)
                self.assertFalse(audit["passed"])
                self.assertTrue(audit["loader_restored"])
                self.assertIn("after_load", audit["memory"])
                self.assertIn("after_test", audit["memory"])

    def test_unexpected_overrides_and_wrong_model_never_reach_loader(self):
        for kwargs in ("lazy=False", "lazy=True", "adapter_path='other'", "model_config={'model_type':'other'}"):
            with self.subTest(kwargs=kwargs):
                self.audit = self.root / (str(len(list(self.root.glob('*.json')))) + ".json")
                self.write_sources(shim_kwargs=kwargs)
                status, audit = self.run_gate()
                self.assertEqual(status, 1)
                self.assertEqual(audit["loader_calls"], 0)
                self.assertIn("unexpected loader overrides", audit["error"])
        self.audit = self.root / "wrong-model.json"
        self.write_sources(body="    cas._load_mlx_runtime('/different-model', memory_map_requested=False)\n")
        status, audit = self.run_gate()
        self.assertEqual(status, 1)
        self.assertEqual(audit["loader_calls"], 0)
        self.assertEqual(self.forwarded, [])

    def test_second_load_remains_failure_even_when_frozen_test_catches_it(self):
        self.write_sources(body=("    cas._load_mlx_runtime(sys.argv[2], memory_map_requested=False)\n"
                                 "    try: cas._load_mlx_runtime(sys.argv[2], memory_map_requested=False)\n"
                                 "    except ValueError: pass\n"))
        status, audit = self.run_gate()
        self.assertEqual(status, 1)
        self.assertFalse(audit["passed"])
        self.assertEqual((audit["loader_calls"], audit["load_attempts"]), (1, 2))
        self.assertEqual(len(self.forwarded), 1)

    def test_direct_loader_call_cannot_bypass_frozen_server_shim(self):
        self.write_sources(body="    import mlx_lm\n    mlx_lm.load(sys.argv[2])\n")
        status, audit = self.run_gate()
        self.assertEqual(status, 1)
        self.assertEqual(audit["loader_calls"], 0)

    def test_preimported_foreign_source_and_symlinked_source_are_refused(self):
        with patch.dict(sys.modules, {"coupled_astrid_server": types.ModuleType("coupled_astrid_server")}):
            with self.fake_mlx():
                self.assertEqual(gate.run_gate(self.source, self.model, self.audit), 1)
        self.assertEqual(self.forwarded, [])
        self.assertIn("already imported", json.loads(self.audit.read_text())["error"])
        self.audit = self.root / "symlink.json"
        original = self.source / "wide_coupling.py"
        other = self.root / "other.py"
        original.rename(other)
        original.symlink_to(other)
        status, audit = self.run_gate()
        self.assertEqual(status, 1)
        self.assertIn("symlinked frozen source", audit["error"])

    def test_missing_load_and_source_mutation_fail_closed(self):
        self.write_sources(body="    pass\n")
        status, audit = self.run_gate()
        self.assertEqual(status, 1)
        self.assertEqual(audit["loader_calls"], 0)
        self.audit = self.root / "mutated.json"
        self.write_sources(body=("    cas._load_mlx_runtime(sys.argv[2], memory_map_requested=False)\n"
                                 "    from pathlib import Path\n"
                                 "    Path(wide_coupling.__file__).write_text('changed')\n"))
        status, audit = self.run_gate()
        self.assertEqual(status, 1)
        self.assertIn("frozen source changed", audit["error"])

    def test_missing_memory_evidence_cannot_pass_and_prior_audit_is_preserved(self):
        self.mx.get_peak_memory = lambda: None
        status, audit = self.run_gate()
        self.assertEqual(status, 1)
        self.assertFalse(audit["passed"])
        before = self.audit.read_bytes()
        with self.assertRaises(FileExistsError):
            gate.run_gate(self.source, self.model, self.audit)
        self.assertEqual(self.audit.read_bytes(), before)


if __name__ == "__main__":
    unittest.main()
