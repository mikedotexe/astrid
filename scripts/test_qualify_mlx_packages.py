from __future__ import annotations

import json
from pathlib import Path
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch, Mock

import astrid_mlx_memory as memory
import qualify_mlx_packages as packages
import qualify_mlx_offline as offline


def response(text="A complete answer."):
    return {"choices": [{"message": {"content": text}, "finish_reason": "stop"}],
            "usage": {"prompt_tokens": 20, "completion_tokens": 12},
            "coupled_generation_v1": {"schema": "coupled_generation_v1", "finish_reason": "stop",
                "controls": {"thinking": False, "wide_coupling_strength": 0., "temperature": 0.,
                             "sampling_mode": "greedy", "processor_order": ["reservoir", "additional_repetition"]},
                "prompt_tokens": 20, "completion_tokens": 12,
                "termination": {"kind": "model_eos", "model_eos_reached": True}}}


class OutputTests(unittest.TestCase):
    def test_successful_push_cannot_hide_failed_or_missing_checkout(self):
        health = {"reservoir": {"status": "connected", "operation": "push_state"}}
        for reservoir in ({}, {"checkout_succeeded": False}):
            with self.subTest(reservoir=reservoir):
                self.assertEqual(packages.validate_reservoir({"reservoir": reservoir}, health),
                                 ["reservoir_checkout_failed_or_missing"])
        self.assertEqual(packages.validate_reservoir({"reservoir": {"checkout_succeeded": True}}, health), [])

    def test_initial_coupling_must_match_journal_but_later_adaptation_is_allowed(self):
        value = response()
        value["coupled_generation_v1"]["controls"]["reservoir_coupling_strength"] = .02
        self.assertEqual(packages.validate_initial_coupling(0, value, .02), [])
        value["coupled_generation_v1"]["controls"]["reservoir_coupling_strength"] = .1
        self.assertEqual(packages.validate_initial_coupling(0, value, .02), ["initial_restored_coupling_mismatch"])
        self.assertEqual(packages.validate_initial_coupling(1, value, .02), [])

    def test_complete_long_response_is_valid(self):
        self.assertEqual(packages.validate_output({"min_words": 350}, response("word " * 500)), [])

    def test_native_eos_and_usage_required(self):
        for mutation in ("channel_boundary", "server_stop_special", "output_allowance"):
            with self.subTest(mutation=mutation):
                value = response()
                value["coupled_generation_v1"]["termination"] = {"kind": mutation, "model_eos_reached": False}
                self.assertIn("workload_did_not_complete_at_native_eos", packages.validate_output({}, value))
        value = response()
        value["usage"]["completion_tokens"] += 1
        self.assertIn("inconsistent_native_completion_evidence", packages.validate_output({}, value))

    def test_thinking_and_wide_coupling_must_remain_disabled(self):
        for key, value in (("thinking", True), ("wide_coupling_strength", .1)):
            changed = response()
            changed["coupled_generation_v1"]["controls"][key] = value
            self.assertIn("generation_controls_changed", packages.validate_output({}, changed))

    def test_artifact_and_incomplete_story_block(self):
        self.assertIn("raw_model_artifact", packages.validate_output({}, response("<|channel>analysis")))
        self.assertIn("incomplete_requested_output", packages.validate_output({"min_words": 350}, response()))

    def test_exact_and_final_next_contract(self):
        self.assertEqual(packages.validate_output({"expected": "OK"}, response("OK")), [])
        self.assertIn("exact_output", packages.validate_output({"expected": "OK"}, response("OK!")))
        self.assertEqual(packages.validate_output({"next": True}, response("Calm water.\nNEXT: LISTEN")), [])
        self.assertIn("next_contract", packages.validate_output({"next": True}, response("NEXT: LISTEN\nMore text")))

    def test_long_latency_not_universal_240_rejection(self):
        workload = {"id": "writing", "max_tokens": 1200, "deadline_s": 1200, "messages": []}
        rows = [{"workload_id": "writing", "request": {"prompt_sha256_12": "abc", "temperature": 0.},
                 "usage": {"prompt_tokens": 20, "completion_tokens": 800},
                 "generation": {"total_turn_s": 500, "first_token_s": 2,
                                "steady_tok_s": 10, "peak_memory_bytes": 1000}} for _ in range(3)]
        with patch.object(packages, "workloads", return_value=[workload]):
            self.assertEqual(packages.summarize(rows, 3)["writing"]["p95_total_turn_s"], 500)
            rows[0]["generation"]["total_turn_s"] = 1201
            with self.assertRaisesRegex(ValueError, "request-specific deadline"):
                packages.summarize(rows, 3)
            rows[0]["generation"]["total_turn_s"] = 500
            rows[0]["generation"]["first_token_s"] = None
            with self.assertRaisesRegex(ValueError, "metric"):
                packages.summarize(rows, 3)

    def test_rendered_prompt_drift_blocks(self):
        workload = {"id": "x", "messages": []}
        rows = [{"workload_id": "x", "request": {"prompt_sha256_12": str(i), "temperature": 0.},
                 "usage": {"prompt_tokens": 20}} for i in range(3)]
        with patch.object(packages, "workloads", return_value=[workload]):
            with self.assertRaisesRegex(ValueError, "rendered prompt"):
                packages.summarize(rows, 3)


class ProvenanceTests(unittest.TestCase):
    def test_one_hour_baseline_duration_gate(self):
        with tempfile.TemporaryDirectory() as temp:
            path = Path(temp) / "baseline.json"
            record = {"mode": "observe_only", "state": "completed", "duration_s": 3599,
                      "summary": {"automated_ok": True}}
            path.write_text(json.dumps(record))
            with self.assertRaisesRegex(ValueError, "one-hour"):
                packages.validate_baseline(path, stage="core", qualification_target="core")
            record["duration_s"] = 3600
            path.write_text(json.dumps(record))
            with patch.object(packages, "review_observation", return_value={"passed": True}):
                self.assertEqual(packages.validate_baseline(path, stage="core", qualification_target="core"),
                                 packages.hashlib.sha256(path.read_bytes()).hexdigest())
                with self.assertRaisesRegex(ValueError, "two-hour"):
                    packages.validate_baseline(path, stage="core", qualification_target="full")
                with self.assertRaisesRegex(ValueError, "two-hour"):
                    packages.validate_baseline(path, stage="full", qualification_target="full")
                with self.assertRaisesRegex(ValueError, "environment stage"):
                    packages.validate_baseline(path, stage="baseline", qualification_target="full")
                record["duration_s"] = 7200
                path.write_text(json.dumps(record))
                self.assertEqual(packages.validate_baseline(path, stage="core", qualification_target="full"),
                                 packages.hashlib.sha256(path.read_bytes()).hexdigest())
                self.assertEqual(packages.validate_baseline(path, stage="full", qualification_target="full"),
                                 packages.hashlib.sha256(path.read_bytes()).hexdigest())

    def test_missing_baseline_identity_blocks(self):
        with tempfile.TemporaryDirectory() as temp:
            path = Path(temp) / "baseline.json"
            path.write_text(json.dumps({"mode": "observe_only", "state": "completed", "duration_s": 7201,
                                        "summary": {"automated_ok": True}}))
            with self.assertRaisesRegex(ValueError, "passive baseline"):
                packages.validate_baseline(path, stage="baseline", qualification_target="core")

    def test_offline_receipt_bound_to_manifest_and_baseline(self):
        prep = {"stages": {"core": {"packages": {"mlx": "0.32.4"}}}, "source_hashes": {"a.py": "sha"}}
        receipt = {"schema": "mlx_offline_qualification_v1", "stage": "core",
                   "qualification_target": "core", "passed": True,
                   "predecessor_acceptance": None,
                   "completed_at": "date", "manifest_sha256": "manifest", "baseline_soak_sha256": "baseline",
                   "harness_hashes": packages.harness_hashes(),
                   "packages": prep["stages"]["core"]["packages"], "source_hashes": prep["source_hashes"]}
        with tempfile.TemporaryDirectory() as temp:
            path = Path(temp) / "offline.json"
            path.write_text(json.dumps(receipt))
            packages.validate_offline(path, prep, "core", "manifest", "baseline", "core", None)
            for manifest, baseline in (("changed", "baseline"), ("manifest", "changed")):
                with self.assertRaisesRegex(ValueError, "exact stage"):
                    packages.validate_offline(path, prep, "core", manifest, baseline, "core", None)
            with self.assertRaisesRegex(ValueError, "exact stage"):
                packages.validate_offline(path, prep, "core", "manifest", "baseline", "full", {})
            with self.assertRaisesRegex(ValueError, "exact stage"):
                packages.validate_offline(path, prep, "core", "manifest", "baseline", "core",
                                          {"foreign": "predecessor"})

    def test_stage2_prework_requires_complete_predecessor_before_effects(self):
        args = SimpleNamespace(qualification_target="full", baseline_soak=Path("/missing/soak"))
        with patch.object(packages, "validate_manifest", side_effect=AssertionError("effect")):
            with self.assertRaisesRegex(ValueError, "all accepted core predecessor"):
                packages.validate_predecessor_for_work(args, "sha", {"stages": {}})
        args.qualification_target = "core"
        args.core_proof = Path("/foreign/proof")
        with self.assertRaisesRegex(ValueError, "cannot borrow"):
            packages.validate_predecessor_for_work(args, "sha", {"stages": {}})

    def test_foreign_listener_blocks_requests(self):
        process = Mock(pid=100)
        process.poll.return_value = None
        with patch.object(packages.canary, "port_pids", side_effect=[["100"], ["101"]]):
            with self.assertRaisesRegex(RuntimeError, "listener"):
                packages.validate_ownership(process)

    def test_failed_offline_command_retains_receipt(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            prep = {"stages": {"core": {"python": "/fake/python", "source_root": str(root), "packages": {}}},
                    "source_hashes": {}, "model_snapshot": "/snapshot"}
            path = root / "preparation.json"
            path.write_text(json.dumps(prep))
            args = SimpleNamespace(baseline_soak=root / "baseline", preparation=path, stage="core",
                                   qualification_target="core",
                                   manifest=root / "manifest", output_dir=root / "output")
            with patch.object(offline, "validate_baseline", return_value="baseline"), \
                 patch.object(offline, "validate_stage", return_value=prep["stages"]["core"]), \
                 patch.object(offline, "validate_release", return_value="manifest"), \
                 patch.object(offline, "Monitor") as monitor, \
                 patch.object(offline.subprocess, "run", return_value=SimpleNamespace(returncode=7)):
                monitor.return_value.stop.return_value = {"automated_ok": True}
                with self.assertRaisesRegex(RuntimeError, "offline gate failed"):
                    offline.qualify(args)
            receipt = json.loads((args.output_dir / "offline.json").read_text())
            self.assertFalse(receipt["passed"])
            self.assertEqual(receipt["manifest_sha256"], "manifest")
            self.assertEqual(receipt["tests"][0]["returncode"], 7)
            self.assertEqual(len(receipt["tests"]), 1)
            self.assertTrue(receipt["completed_at"])


class MemoryTests(unittest.TestCase):
    def rows(self):
        return [{"t_s": i * 5., "pressure_level": 1, "swap_bytes": 1000.} for i in range(20)]

    def test_historical_swap_alone_does_not_fail(self):
        self.assertTrue(memory.evaluate(self.rows())["automated_ok"])

    def test_elevated_initial_pressure_cannot_become_a_passing_reference(self):
        rows = self.rows()
        for row in rows:
            row["pressure_level"] = 2
        self.assertIn("initial_host_memory_pressure_not_normal", memory.evaluate(rows)["failure_reasons"])

    def test_elevated_start_refuses_workload_and_still_retains_final_samples(self):
        monitor = memory.Monitor()
        with patch.object(memory, "sample", side_effect=[
                {"t_s": 0., "pressure_level": 2, "swap_bytes": 1000.},
                {"t_s": 1., "pressure_level": 1, "swap_bytes": 1000.}]):
            with self.assertRaisesRegex(ValueError, "normal host memory pressure"):
                monitor.start()
            self.assertFalse(monitor.thread.is_alive())
            final = monitor.stop()
        self.assertEqual(len(final["samples"]), 2)
        self.assertFalse(final["automated_ok"])

    def test_retained_growth_or_sustained_pressure_blocks(self):
        rows = self.rows()
        rows[-1]["swap_bytes"] += 1
        self.assertIn("retained_swap_growth", memory.evaluate(rows)["failure_reasons"])
        rows = self.rows()
        for row in rows[1:]:
            row["pressure_level"] = 2
        self.assertIn("sustained_additional_host_memory_pressure", memory.evaluate(rows)["failure_reasons"])

    def test_missing_samples_and_gaps_block(self):
        self.assertFalse(memory.evaluate([])["automated_ok"])
        self.assertFalse(memory.evaluate([{"error": "permission"}] * 2)["automated_ok"])
        rows = self.rows()
        rows[-1]["t_s"] += 20
        self.assertIn("host_memory_sampling_gap", memory.evaluate(rows)["failure_reasons"])

    def test_sustained_swap_blocks_even_if_recovered_at_exit(self):
        rows = self.rows()
        for row in rows[1:-1]:
            row["swap_bytes"] += 1024
        self.assertIn("sustained_additional_swap", memory.evaluate(rows)["failure_reasons"])


if __name__ == "__main__":
    unittest.main()
