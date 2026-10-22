"""Tiny stdlib-only fakes: no subprocess, launchctl, signal, network or model."""
import copy
import json
import os
from pathlib import Path
import tempfile
import unittest

import reservoir_maintenance as m


def fake_plan():
    return {"schema": "reservoir_maintenance_plan_v1", "transaction_id": "a" * 32,
            "actor": "fixture-owner", "pause_generation": 526,
            "host_binding": {"uid": 501, "boot_uuid": "fixture-boot"},
            "input_hashes": {name: m.digest(name) for name in m.REQUIRED_INPUTS},
            "roles": {role: {"label": m.LABELS[role], "port": m.PORTS[role],
                             "identity": {"pid": index + 10, "started_at": "old-start",
                                          "argv": ["/fixture/python", f"/fixture/{role}.py"]}}
                      for index, role in enumerate(m.ROLES)},
            "protected": {"bridge": {"pid": 90, "started_at": "protected-start", "argv": ["/fixture/bridge"]}},
            "timeout_s": 2}


class FakeHost:
    scope = "stdlib_fake"

    def __init__(self, plan):
        self.plan = copy.deepcopy(plan)
        self.inputs = copy.deepcopy(plan["input_hashes"])
        self.protected = copy.deepcopy(plan["protected"])
        self.roles = {role: {"target": f"fake:{role}", "job_loaded": True,
                             "process": copy.deepcopy(plan["roles"][role]["identity"]),
                             "listener_pids": [plan["roles"][role]["identity"]["pid"]], "held": None}
                      for role in m.ROLES}
        self.holds = {}
        self.calls = []
        self.clock = 0
        self.pending = {}
        self.fail = None
        self.bad_witness = None
        self.witnesses = []
        self.never_exit = False
        self.pause_generation = plan["pause_generation"]
        self.after_inspect = None

    def inspect(self):
        self.calls.append(("inspect", None))
        result = {"host_binding": self.plan["host_binding"], "paused": True,
                  "pause_generation": self.pause_generation, "pause_actor": self.plan["actor"], "lease": None,
                  "input_hashes": self.inputs, "protected": self.protected,
                  "fixture_mode_authorized": True, "originals_preserved": True, "rollback_software_ready": True,
                  "roles": {role: self.inspect_role(role) for role in m.ROLES}}
        if self.after_inspect:
            self.after_inspect()
        return copy.deepcopy(result)

    def inspect_role(self, role):
        if role in self.pending and self.clock >= self.pending[role] and not self.never_exit:
            old = self.plan["roles"][role]["identity"]
            held_identity = {"pid": old["pid"] + 100, "started_at": "held-start", "argv": ["/fixture/python", "hold"]}
            self.roles[role].update(process=held_identity, listener_pids=[],
                                    held={"guard": copy.deepcopy(self.holds[role]), "identity": held_identity,
                                          "hold_only": True, "stdlib_only": True, "state_touched": False})
            del self.pending[role]
        return copy.deepcopy(self.roles[role])

    def event(self, operation, role=None):
        self.calls.append((operation, role))
        if self.fail == (operation, role):
            raise RuntimeError("injected failure")

    def create_hold(self, role, guard):
        self.event("create_hold", role)
        if role in self.holds:
            raise RuntimeError("foreign/existing hold")
        self.holds[role] = copy.deepcopy(guard)
        return {"guard": guard}

    def arm_next_spawn(self, role, guard, expected):
        assert self.holds[role] == guard and self.roles[role]["process"] == expected
        # A fault here represents successful arming with an unknown outcome.
        self.event("arm_next_spawn", role)
        return {"armed": True}

    def signal_stop(self, role, expected):
        self.event("signal_stop", role)
        assert self.roles[role]["process"] == expected
        if role == "reservoir":
            assert not self.roles["provider"]["job_loaded"] and "provider_drain" in self.witnesses
        self.pending[role] = self.clock + .5
        return {"signal": "SIGTERM", "identity": expected, "force_used": False}

    def remove_held(self, role, expected, guard):
        self.event("remove_held", role)
        view = self.inspect_role(role)
        assert view["held"] and view["process"] == expected and view["held"]["guard"] == guard
        self.roles[role].update(job_loaded=False, process=None, listener_pids=[], held=None)
        return {"removed_identity": expected}

    def collect_witness(self, kind, context):
        self.event("witness", kind)
        self.witnesses.append(kind)
        result = {"schema": "reservoir_maintenance_witness_v1", "kind": kind,
                  "transaction_id": context["transaction_id"], "passed": True,
                  "bindings": copy.deepcopy(context["bindings"]), "evidence": {"fixture": True}}
        if self.bad_witness and self.bad_witness[0] == kind:
            result.update(self.bad_witness[1])
        return result

    def install_repair(self, context):
        self.event("install_repair")
        assert all(not view["job_loaded"] for view in self.roles.values())
        assert "private_restore" in self.witnesses
        self.inputs["reservoir_source"] = self.inputs["reservoir_candidate"]
        return {"source_sha256": self.inputs["reservoir_source"]}

    def bootstrap_original(self, role, context):
        self.event("bootstrap_original", role)
        assert not self.roles[role]["job_loaded"]
        if role == "provider":
            assert "startup" in self.witnesses
        new = copy.deepcopy(self.plan["roles"][role]["identity"])
        new.update(pid=new["pid"] + 200, started_at="new-start")
        self.roles[role].update(job_loaded=True, process=new, listener_pids=[new["pid"]], held=None)
        return {"identity": new}

    def release_hold(self, role, guard):
        self.event("release_hold", role)
        assert self.holds[role] == guard
        del self.holds[role]
        return {"released": True}

    def monotonic(self):
        return self.clock

    def sleep(self, seconds):
        self.clock += seconds


class MaintenanceTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.plan = fake_plan()
        self.host = FakeHost(self.plan)
        self.path = Path(self.temporary.name).resolve() / "transaction"

    def controller(self):
        self.journal = m.Journal(self.path, plan=self.plan)
        return m.Controller(self.journal, self.host)

    def test_preflight_is_read_only_and_not_production_qualification(self):
        before = copy.deepcopy(self.host.roles)
        result = m.preflight(self.plan, self.host)
        self.assertTrue(result["passed"])
        self.assertFalse(result["production_qualified"])
        self.assertEqual(self.host.calls, [("inspect", None)])
        self.assertEqual(before, self.host.roles)
        self.assertFalse(self.path.exists())

    def test_success_obeys_order_and_restores_same_provider_command(self):
        controller = self.controller()
        result = controller.run()
        calls = self.host.calls
        self.assertLess(calls.index(("witness", "provider_drain")), calls.index(("signal_stop", "reservoir")))
        self.assertLess(calls.index(("witness", "private_restore")), calls.index(("install_repair", None)))
        self.assertLess(calls.index(("witness", "startup")), calls.index(("bootstrap_original", "provider")))
        self.assertEqual(result["provider"]["argv"], self.plan["roles"]["provider"]["identity"]["argv"])
        self.assertFalse(result["force_used"])
        self.assertFalse(result["runtime_state_rollback"])
        self.assertFalse(result["production_qualified"])
        self.assertFalse(self.host.holds)
        loaded = m.Journal(self.path)
        self.assertEqual(loaded.events, self.journal.events)
        self.assertEqual(loaded.events[-1]["type"], "completed")

    def test_fixed_roles_and_production_targets_refused(self):
        for change in (lambda: self.plan["roles"].update(bridge={}),
                       lambda: self.plan["roles"]["provider"].update(label="foreign"),
                       lambda: setattr(self.host, "scope", "production"),
                       lambda: self.host.roles["provider"].update(target="gui/501/com.reservoir.coupled-astrid")):
            with self.subTest(change=change):
                self.plan = fake_plan(); self.host = FakeHost(self.plan)
                change()
                with self.assertRaises(m.Refusal): m.preflight(self.plan, self.host)
                self.assertFalse(self.host.holds)

    def test_owned_fixture_allowlist_is_exact(self):
        self.host.scope = "owned_fixture"
        for role in m.ROLES:
            self.host.roles[role]["target"] = f"gui/501/{m.FIXTURE_PREFIX}{self.plan['transaction_id']}.{role}"
        self.assertTrue(m.preflight(self.plan, self.host)["passed"])
        self.host.roles["reservoir"]["target"] += ".extra"
        with self.assertRaises(m.Refusal): m.preflight(self.plan, self.host)

    def test_preflight_refuses_missing_input_or_changed_pause(self):
        self.host.inputs.pop("provider_packages")
        with self.assertRaises(m.Refusal): self.controller().run()
        self.assertFalse(any(call[0] == "create_hold" for call in self.host.calls))

    def test_failed_provider_checkin_never_stops_reservoir_and_retains_hold(self):
        self.host.bad_witness = ("provider_drain", {"passed": False})
        with self.assertRaises(m.Refusal): self.controller().run()
        self.assertNotIn(("signal_stop", "reservoir"), self.host.calls)
        self.assertNotIn(("remove_held", "provider"), self.host.calls)
        self.assertEqual(set(self.host.holds), {"provider"})

    def test_missing_common_binding_blocks(self):
        self.host.bad_witness = ("provider_drain", {"bindings": {}})
        with self.assertRaises(m.Refusal): self.controller().run()
        self.assertNotIn(("signal_stop", "reservoir"), self.host.calls)

    def test_private_restore_false_blocks_install_and_bootstrap(self):
        self.host.bad_witness = ("private_restore", {"passed": False})
        with self.assertRaises(m.Refusal): self.controller().run()
        self.assertNotIn(("install_repair", None), self.host.calls)
        self.assertFalse(any(call[0] == "bootstrap_original" for call in self.host.calls))
        self.assertEqual(set(self.host.holds), set(m.ROLES))

    def test_bad_startup_retains_provider_absent_and_both_holds(self):
        self.host.bad_witness = ("startup", {"passed": False})
        with self.assertRaises(m.Refusal): self.controller().run()
        self.assertNotIn(("bootstrap_original", "provider"), self.host.calls)
        self.assertFalse(self.host.roles["provider"]["job_loaded"])
        self.assertEqual(set(self.host.holds), set(m.ROLES))

    def test_uncertain_debug_intent_is_durable_and_recovery_is_read_only(self):
        self.host.fail = ("arm_next_spawn", "provider")
        with self.assertRaises(RuntimeError): self.controller().run()
        before = list(self.host.calls)
        recovery = m.inspect_recovery(m.Journal(self.path), self.host)
        self.assertEqual(recovery["pending_mutation"]["operation"], "arm_next_spawn")
        self.assertEqual(recovery["possibly_pending_debug_roles"], ["provider"])
        self.assertFalse(recovery["automatic_resume_allowed"])
        self.assertEqual(self.host.calls, before)
        self.assertIn("provider", self.host.holds)

    def test_unconfirmed_debug_never_signals(self):
        self.host.arm_next_spawn = lambda *_: {"armed": False}
        with self.assertRaisesRegex(m.Refusal, "not positively armed"): self.controller().run()
        self.assertFalse(any(call[0] == "signal_stop" for call in self.host.calls))
        self.assertIn("provider", self.host.holds)

    def test_armed_timeout_has_no_escalation_or_repeated_signal(self):
        self.host.never_exit = True
        with self.assertRaisesRegex(m.Refusal, "timed out"): self.controller().run()
        self.assertEqual(self.host.calls.count(("signal_stop", "provider")), 1)
        self.assertLessEqual(self.host.clock, 2)
        self.assertIn("provider", self.host.holds)
        recovery = m.inspect_recovery(self.journal, self.host)
        self.assertEqual(recovery["possibly_pending_debug_roles"], ["provider"])

    def test_existing_transaction_cannot_replay_signals(self):
        controller = self.controller()
        controller.run()
        count = len(self.host.calls)
        with self.assertRaisesRegex(m.Refusal, "never replay"): m.Controller(m.Journal(self.path), self.host).run()
        self.assertEqual(len(self.host.calls), count)

    def test_foreign_holder_not_removed(self):
        original = self.host.inspect_role
        def foreign(role):
            view = original(role)
            if view["held"]:
                view["held"]["guard"]["actor"] = "foreign"
            return view
        self.host.inspect_role = foreign
        with self.assertRaises(m.Refusal): self.controller().run()
        self.assertFalse(any(call[0] == "remove_held" for call in self.host.calls))

    def test_pause_change_after_provider_removal_blocks_reservoir_signal(self):
        original = self.host.remove_held
        def remove(role, expected, guard):
            result = original(role, expected, guard)
            if role == "provider":
                self.host.pause_generation += 1
            return result
        self.host.remove_held = remove
        with self.assertRaises(m.Refusal): self.controller().run()
        self.assertNotIn(("signal_stop", "reservoir"), self.host.calls)

    def test_loaded_launchd_gap_is_bounded_and_tolerated(self):
        original = self.host.inspect_role
        def gap(role):
            view = original(role)
            if role in self.host.pending and self.host.clock < self.host.pending[role]:
                view.update(process=None, listener_pids=[], held=None)
            return view
        self.host.inspect_role = gap
        self.assertEqual(self.controller().run()["status"], "verified")

    def test_unloaded_job_during_replacement_is_refused(self):
        original = self.host.inspect_role
        def missing(role):
            view = original(role)
            if role in self.host.pending:
                view.update(job_loaded=False, process=None, listener_pids=[], held=None)
            return view
        self.host.inspect_role = missing
        with self.assertRaisesRegex(m.Refusal, "unexpected provider replacement"): self.controller().run()
        self.assertIn("provider", self.host.holds)

    def test_parked_provider_cannot_reappear_before_reservoir_stop(self):
        original = self.host.create_hold
        def respawn(role, guard):
            result = original(role, guard)
            if role == "reservoir":
                old = self.plan["roles"]["provider"]["identity"]
                self.host.roles["provider"].update(job_loaded=True, process=old, listener_pids=[old["pid"]])
            return result
        self.host.create_hold = respawn
        with self.assertRaisesRegex(m.Refusal, "provider is not absent"): self.controller().run()
        self.assertNotIn(("signal_stop", "reservoir"), self.host.calls)

    def test_final_snapshot_failure_preserves_reservoir_holder(self):
        self.host.bad_witness = ("final_snapshot", {"passed": False})
        with self.assertRaises(m.Refusal): self.controller().run()
        self.assertNotIn(("remove_held", "reservoir"), self.host.calls)
        self.assertNotIn(("witness", "backup"), self.host.calls)
        self.assertEqual(set(self.host.holds), set(m.ROLES))

    def test_journal_existing_path_and_corrupt_chain_refused(self):
        journal = m.Journal(self.path, plan=self.plan)
        with self.assertRaises(FileExistsError): m.Journal(self.path, plan=self.plan)
        journal.append("one")
        journal.append("two")
        first = self.path / "event-00001.json"
        value = json.loads(first.read_text()); value["type"] = "tampered"
        first.write_text(json.dumps(value))
        with self.assertRaisesRegex(m.Refusal, "chain mismatch"): m.Journal(self.path)

    def test_readiness_requires_same_new_pid_and_listener(self):
        original = self.host.collect_witness
        def wrong_listener(kind, context):
            result = original(kind, context)
            if kind == "startup": self.host.roles["reservoir"]["listener_pids"] = [999]
            return result
        self.host.collect_witness = wrong_listener
        with self.assertRaisesRegex(m.Refusal, "listener mismatch"): self.controller().run()
        self.assertNotIn(("bootstrap_original", "provider"), self.host.calls)

    def test_bootstrap_cannot_change_original_provider_command(self):
        original = self.host.bootstrap_original
        def changed_model(role, context):
            result = original(role, context)
            if role == "provider":
                result["identity"]["argv"].extend(["--model", "wrong-model"])
            return result
        self.host.bootstrap_original = changed_model
        with self.assertRaisesRegex(m.Refusal, "original provider command"): self.controller().run()
        self.assertEqual(set(self.host.holds), set(m.ROLES))

    def test_bad_receipt_kind_or_transaction_never_passes(self):
        for field, value in (("kind", "startup"), ("transaction_id", "b" * 32), ("passed", 1)):
            with self.subTest(field=field):
                plan = fake_plan()
                host = FakeHost(plan)
                host.bad_witness = ("provider_drain", {field: value})
                journal = m.Journal(self.path.parent / field, plan=plan)
                with self.assertRaises(m.Refusal): m.Controller(journal, host).run()
                self.assertNotIn(("signal_stop", "reservoir"), host.calls)


if __name__ == "__main__":
    unittest.main()
