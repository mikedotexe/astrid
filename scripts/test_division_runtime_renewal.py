"""Ordered failures use synthetic hosts and owned fixtures, never live services."""
import copy
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import Mock, patch

from division_runtime_renewal_host import (LIFETIME_MS, Runtime, SERVICES, GATEWAY,
    SUPERVISOR, renewed_manifest, parent_authority, renew_ordered)
from division_runtime_renewal import RenewalAgent, RenewalHandoff
from minime_engine_transition_io import Holds
from restart_minime_agent import PROTECTED
from paired_minime_handoff import BRIDGE


class RenewalTests(unittest.TestCase):
    def test_renewal_changes_only_time_and_keeps_bounded_lifetime(self):
        before = dict(mode="dormant", candidate_hash="unbound", created_at_unix_ms=1,
                      expires_at_unix_ms=2, division_id="synthetic", endpoints={"private": "retained"})
        original = copy.deepcopy(before)
        after = renewed_manifest(before, 100)
        self.assertEqual(before, original)
        self.assertEqual(after, {**before, "created_at_unix_ms": 100, "expires_at_unix_ms": 100 + LIFETIME_MS})
        for change in ({"mode": "candidate_bound"}, {"candidate_hash": "candidate"},
                       {"expires_at_unix_ms": 100}, {"created_at_unix_ms": 3}):
            with self.subTest(change=change), self.assertRaises(ValueError):
                renewed_manifest({**before, **change}, 100)

    def test_authority_never_infers_parent_from_incomplete_or_stale_record(self):
        value = dict(schema="division.gateway_authority.v1", rail="parent", manifest_sha256="a" * 64,
                     division_id="d", live_authority_granted_by_record=False)
        parent_authority(value, "a" * 64, "d")
        for change in ({"rail": "daughters"}, {"manifest_sha256": "b" * 64},
                       {"division_id": "other"}, {"live_authority_granted_by_record": True},
                       {"switch_receipt_sha256": "c" * 64}, {"handoff_contract_receipt_sha256": "d" * 64}):
            with self.subTest(change=change), self.assertRaises(ValueError):
                parent_authority({**value, **change}, "a" * 64, "d")

    def test_every_transition_failure_stops_without_automatic_resume_or_rollback(self):
        phases = ["freeze_services", "publish_manifest", "start_supervisor", "start_gateway", "verify_ready"]
        for stop in range(len(phases) + 1):
            host, seen = Mock(), []
            for i, name in enumerate(phases):
                def step(i=i, name=name):
                    seen.append(name)
                    if i == stop:
                        raise OSError("synthetic interrupted publication")
                getattr(host, name).side_effect = step
            if stop < len(phases):
                with self.assertRaises(OSError):
                    renew_ordered(host)
                self.assertEqual(seen, phases[:stop + 1])
            else:
                renew_ordered(host)
                self.assertEqual(seen, phases)
            host.restore.assert_not_called()
            host.resume.assert_not_called()

    def test_protected_mapping_excludes_only_explicitly_owned_services(self):
        runtime = Mock()
        backend = RenewalAgent(511, "synthetic", runtime)
        backend.pid = lambda label: PROTECTED.index(label) + 10
        backend.identity = lambda pid: str(pid)
        result = backend.protected()
        self.assertEqual(set(result), set(PROTECTED) - {GATEWAY, SUPERVISOR})
        runtime.guards.assert_called_once()
        runtime.guards.side_effect = ValueError("gateway identity changed")
        with self.assertRaises(ValueError):
            backend.protected()

    def test_foreign_hold_and_missing_held_gateway_never_publish(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp).resolve()
            runtime = Runtime.__new__(Runtime)
            runtime.current = {label: {"pid": 100 + n} for n, label in enumerate(SERVICES)}
            runtime.guards = Mock()
            runtime.holds = Holds(root, "owned")
            runtime.manifest = root / "manifest.json"
            runtime.manifest.write_text("old")
            runtime.record = Mock()
            foreign = Holds(root, "foreign")
            foreign.create(SUPERVISOR)
            with patch("division_runtime_renewal_host.listeners", return_value=set()), self.assertRaises(ValueError):
                runtime.publish_manifest()
            self.assertEqual(runtime.manifest.read_text(), "old")
            runtime.record.assert_not_called()
            foreign.release(SUPERVISOR)
            runtime.holds.create(SUPERVISOR)
            with patch("division_runtime_renewal_host.listeners", return_value={42}), self.assertRaises(ValueError):
                runtime.publish_manifest()
            self.assertEqual(runtime.manifest.read_text(), "old")

    def test_consumer_restore_only_after_drain_and_verified_renewal(self):
        phases = ["seal", "drain", "renew", "restore"]
        for failed in (*phases, None):
            with self.subTest(failed=failed):
                handoff = RenewalHandoff.__new__(RenewalHandoff)
                handoff.runtime = Mock()
                handoff.assert_hold = Mock()
                handoff.inputs, handoff.config = {"source": "hash"}, {"config": "hash"}
                handoff.stage = Path("/synthetic/stage")
                handoff.next_identity = {"pid": 7}
                backend = Mock()
                backend.inputs.return_value, backend.config.return_value = handoff.inputs, handoff.config
                backend.jobs.return_value = {"active": []}
                protected = {BRIDGE: {"pid": 10}}
                backend.protected.return_value = protected
                seen = []
                def invoke(name, result=None):
                    seen.append(name)
                    if failed == name:
                        raise ValueError("synthetic refusal")
                    return result
                handoff.runtime.renew.side_effect = lambda: invoke("renew")
                backend.verify_next_seal.side_effect = lambda _: invoke("seal", {"verified": True})
                with patch("division_runtime_renewal.drain", side_effect=lambda *a: invoke("drain", {"phase": "drained"})), patch(
                        "paired_minime_handoff.PairedHandoff.transition", side_effect=lambda *a: invoke("restore", protected)):
                    if failed:
                        with self.assertRaises(ValueError):
                            handoff.transition(backend, protected, lambda e: None)
                        self.assertEqual(seen, phases[:phases.index(failed) + 1])
                    else:
                        self.assertEqual(handoff.transition(backend, protected, lambda e: None), protected)
                        self.assertEqual(seen, phases)

    def test_maintenance_change_stops_before_other_guards(self):
        runtime = Runtime.__new__(Runtime)
        runtime.args = Mock(pause_generation=511)
        for change in ({"paused": False}, {"pause_generation": 512}, {"actor": "other"}):
            control = dict(paused=True, pause_generation=511, actor="codex-astra-interactive")
            with patch("division_runtime_renewal_host.read_json", return_value={**control, **change}), self.assertRaises(ValueError):
                runtime.guards()

    def test_publication_failure_never_starts_services_or_rewrites_old_bytes(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp).resolve()
            runtime = Runtime.__new__(Runtime)
            runtime.current = {label: {"pid": 100 + n} for n, label in enumerate(SERVICES)}
            runtime.guards, runtime.record = Mock(), Mock()
            runtime.holds = Holds(root / "holds", "owned")
            for label in SERVICES:
                runtime.holds.create(label)
            runtime.transaction = root
            runtime.manifest = root / "manifest.json"
            runtime.manifest.write_text("old")
            runtime.new_bytes, runtime.new_sha, runtime.published = b"new", "new-hash", False
            (root / "new-manifest.json").write_bytes(runtime.new_bytes)
            with patch("division_runtime_renewal_host.listeners", return_value=set()), patch(
                    "division_runtime_renewal_host.command", return_value="bash"), patch(
                    "division_runtime_renewal_host.atomic", side_effect=OSError("disk full")), self.assertRaises(OSError):
                runtime.publish_manifest()
            self.assertEqual(runtime.manifest.read_text(), "old")
            self.assertFalse(runtime.published)
            for label in SERVICES:
                runtime.holds.verify(label)

    def test_stale_or_active_supervisor_status_is_not_readiness(self):
        runtime = Runtime.__new__(Runtime)
        runtime.current, runtime.new_sha = {SUPERVISOR: {"pid": 7}}, "a"
        value = dict(pid=7, manifest_sha256="a", mode="idle_parent_authoritative", parent_authoritative=True,
                     children={}, matching_intents=[], handoff_ready=False, commit_recommended=False,
                     live_authority_granted_by_record=False, updated_at_unix_ms=1000)
        with patch("division_runtime_renewal_host.time.time", return_value=1):
            runtime.verify_supervisor(value)
            for change in ({"pid": 8}, {"manifest_sha256": "b"}, {"children": {"a": {}}},
                           {"matching_intents": ["a"]}, {"updated_at_unix_ms": -15000},
                           {"handoff_ready": True}, {"live_authority_granted_by_record": True}):
                with self.subTest(change=change), self.assertRaises(ValueError):
                    runtime.verify_supervisor({**value, **change})

    def test_passive_telemetry_requires_finite_advancing_packets(self):
        runtime = Runtime.__new__(Runtime)
        runtime.verify_ready, runtime.record, runtime.health = Mock(), Mock(), Mock(return_value={})
        first = {"t_ms": 100, "eigenvalues": [1.0, 0.5]}
        for second, good in (({"t_ms": 200, "eigenvalues": [0.9]}, True),
                             ({"t_ms": 100, "eigenvalues": [0.9]}, False),
                             ({"t_ms": 200, "eigenvalues": [float("nan")]}, False)):
            with patch("websockets.sync.client.connect") as connect:
                connect.return_value.__enter__.return_value.recv.side_effect = [json.dumps(first), json.dumps(second)]
                if good:
                    runtime.observe_telemetry()
                else:
                    with self.assertRaises(ValueError):
                        runtime.observe_telemetry()
        self.assertEqual(runtime.record.call_count, 1)


if __name__ == "__main__":
    unittest.main()
