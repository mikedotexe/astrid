"""Model-free verifier/import tests; actual installed generator runs are separate."""
from __future__ import annotations

from copy import deepcopy
from pathlib import Path
import json
import subprocess
import sys
import tempfile
import types
import unittest
from unittest.mock import patch

import astrid_mlx_generator_contract as contract


def valid_trace(history_mode="final_prompt_token_only"):
    """Small explicit protocol fixture reusable by the outer proof verifier."""
    prefix = [5] if history_mode == "final_prompt_token_only" else [1, 2, 3, 4, 5]
    events = [
        {"kind": "model", "tokens": [1, 2], "cache_before": 0, "cache_after": 2},
        {"kind": "model", "tokens": [3, 4], "cache_before": 2, "cache_after": 4},
    ]
    # (input, cumulative generated input, feedback state, selected next token)
    rows = [(5, [], 0, 6), (6, [6], 0, 6), (6, [6, 6], 1, 7),
            (7, [6, 6, 7], 2, 0), (0, [6, 6, 7, 0], 3, 1)]
    yielded = [6, 6, 7, 0]
    for prediction, (incoming, history, feedback, selected) in enumerate(rows):
        events.append({"kind": "model", "tokens": [incoming], "cache_before": prediction + 4,
                       "cache_after": prediction + 5})
        for name in ("feedback", "additional"):
            events.append({"kind": "processor", "name": name, "prediction": prediction,
                           "tokens": prefix + history, "feedback": feedback, "selected": selected})
        if prediction:
            index = prediction - 1
            events += [{"kind": "yield", "index": index, "token": yielded[index],
                        "logprobs_argmax": yielded[index]},
                       {"kind": "feedback", "after_yield": index, "value": prediction}]
    common = {"default_stream_before": "Stream(Device(cpu, 0), 0)",
              "default_stream_after": "Stream(Device(cpu, 0), 0)",
              "generation_stream_unchanged": True}
    return [{"case": "complete", **common, "events": events + [{"kind": "exhausted"}]},
            {"case": "early_close", **common, "events": deepcopy(events[:10]) + [{"kind": "closed"}]}]


class TraceTests(unittest.TestCase):
    def test_import_has_no_mlx_or_device_side_effect(self):
        code = ("import sys; sys.path.insert(0, " + repr(str(Path(contract.__file__).parent)) + "); "
                "import astrid_mlx_generator_contract; "
                "assert not any(n == 'mlx' or n.startswith('mlx.') or n == 'mlx_lm' "
                "or n.startswith('mlx_lm.') for n in sys.modules)")
        subprocess.run([sys.executable, "-B", "-c", code], check=True, timeout=10)

    def test_both_explicit_version_histories_are_accepted(self):
        for mode in contract.HISTORY_MODES.values():
            with self.subTest(mode=mode):
                self.assertEqual(contract.validate_trace(valid_trace(mode), mode),
                                 {key: True for key in contract.CHECK_KEYS})

    def test_prompt_history_difference_is_not_normalized(self):
        with self.assertRaisesRegex(ValueError, "trace differs"):
            contract.validate_trace(valid_trace("full_prefilled_prompt"), "final_prompt_token_only")
        with self.assertRaisesRegex(ValueError, "unsupported"):
            contract.validate_trace(valid_trace(), "auto_detect")

    def test_timing_order_cache_and_emission_regressions_fail(self):
        mutations = {
            "first_feedback_too_early": lambda e: e[6].update(feedback=1),
            "feedback_one_step_late": lambda e: e[11].update(feedback=0),
            "extra_token_emitted": lambda e: e.insert(-1, {"kind": "yield", "index": 4, "token": 1}),
            "dropped_final_yield": lambda e: e.pop(-3),
            "wrong_cache_offset": lambda e: e[2].update(cache_after=6),
            "processor_reversed": lambda e: e.__setitem__(slice(3, 5), e[3:5][::-1]),
            "wrong_second_processor_tokens": lambda e: e[4].update(tokens=[]),
            "boolean_cache_counter": lambda e: e[0].update(cache_before=False),
        }
        for name, change in mutations.items():
            with self.subTest(name=name):
                trace = valid_trace()
                change(trace[0]["events"])
                with self.assertRaises(ValueError):
                    contract.validate_trace(trace, "final_prompt_token_only")

    def test_close_must_restore_stream_and_prevent_further_work(self):
        for change in (
            lambda c: c.update(default_stream_after="another stream"),
            lambda c: c.update(generation_stream_unchanged=False),
            lambda c: c["events"].insert(-1, {"kind": "model", "tokens": [6]}),
            lambda c: c["events"].__setitem__(-1, {"kind": "exhausted"}),
        ):
            trace = valid_trace()
            change(trace[1])
            with self.assertRaises(ValueError):
                contract.validate_trace(trace, "final_prompt_token_only")


class AuditTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.output = self.root / "audit.json"
        self.operations = []
        self.version = "0.31.3"
        self.mx = types.SimpleNamespace(cpu="cpu", selected="gpu")
        self.mx.default_device = lambda: types.SimpleNamespace(type=self.mx.selected)
        def select(device):
            self.operations.append("select_cpu")
            self.mx.selected = device
        self.mx.set_default_device = select
        self.mx.__file__ = str(self.root / "core.fake.so")
        Path(self.mx.__file__).write_bytes(b"fake extension; never imported")
        self.module = types.ModuleType("mlx_lm.generate")
        self.module.__file__ = str(self.root / "generate.py")
        source = "def generate_step():\n    raise AssertionError('fake must not run')\n"
        Path(self.module.__file__).write_text(source)
        exec(compile(source, self.module.__file__, "exec"), self.module.__dict__)
        self.module.generation_stream = types.SimpleNamespace(device=types.SimpleNamespace(type="cpu"))
        self.package = types.ModuleType("mlx_lm")
        self.package.__file__ = str(self.root / "__init__.py")
        Path(self.package.__file__).write_text("# fake package\n")
        modules = patch.dict(sys.modules)
        modules.start()
        self.addCleanup(modules.stop)
        for name in list(sys.modules):
            if name == "mlx_lm" or name.startswith("mlx_lm."):
                sys.modules.pop(name)

    def run_audit(self, *, invalid_trace=False):
        def version(name):
            return self.version if name == "mlx-lm" else "fake-version"
        def importer(name):
            self.operations.append(name)
            if name == "mlx.core":
                return self.mx
            self.assertEqual(name, "mlx_lm.generate")
            self.assertEqual(self.mx.selected, "cpu")
            sys.modules["mlx_lm"] = self.package
            return self.module
        def toy_case(mx, module, *, early_close):
            self.assertIs(mx, self.mx)
            self.assertIs(module, self.module)
            case = valid_trace(contract.HISTORY_MODES[self.version])[int(early_close)]
            if invalid_trace:
                case["events"][3]["feedback"] = 7
            return case
        with patch.object(contract.metadata, "version", side_effect=version), \
             patch.object(contract.importlib, "import_module", side_effect=importer), \
             patch.object(contract, "run_case", side_effect=toy_case):
            status = contract.run(self.output)
        return status, json.loads(self.output.read_text())

    def test_cpu_is_selected_before_package_import_and_evidence_is_bound(self):
        status, audit = self.run_audit()
        self.assertEqual(status, 0)
        self.assertTrue(audit["passed"])
        self.assertEqual(self.operations, ["mlx.core", "select_cpu", "mlx_lm.generate"])
        self.assertEqual(audit["device"], {"default": "cpu", "generation_stream": "cpu"})
        self.assertTrue(audit["cpu_selected_before_generator_import"])
        self.assertEqual(len(audit["source_hashes"]), 3)
        for name, expected in audit["source_hashes"].items():
            self.assertEqual(contract.digest(Path(name)), expected)
        self.assertEqual(audit["checks"], contract.validate_trace(audit["trace"], audit["expected_history_mode"]))

    def test_preimported_package_refused_before_device_or_generation(self):
        sys.modules["mlx_lm.generate"] = self.module
        status, audit = self.run_audit()
        self.assertEqual(status, 1)
        self.assertIn("before CPU selection", audit["error"])
        self.assertEqual(self.operations, [])

    def test_unsupported_version_and_gpu_stream_fail_closed(self):
        self.version = "0.33.0"
        status, audit = self.run_audit()
        self.assertEqual(status, 1)
        self.assertEqual(self.operations, [])
        self.assertIn("unqualified", audit["error"])
        self.output = self.root / "gpu-stream.json"
        self.version = "0.31.3"
        self.module.generation_stream.device.type = "gpu"
        status, audit = self.run_audit()
        self.assertEqual(status, 1)
        self.assertIn("not CPU", audit["error"])

    def test_failed_trace_is_retained_and_previous_attempt_cannot_be_overwritten(self):
        status, audit = self.run_audit(invalid_trace=True)
        self.assertEqual(status, 1)
        self.assertFalse(audit["passed"])
        self.assertTrue(audit["trace"])
        before = self.output.read_bytes()
        with self.assertRaises(FileExistsError):
            contract.run(self.output)
        self.assertEqual(self.output.read_bytes(), before)


if __name__ == "__main__":
    unittest.main()
