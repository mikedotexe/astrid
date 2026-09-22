"""No live signals or workspaces: exercise the visual reload boundary."""
from copy import deepcopy
import unittest

from restart_visual_service import CONTRACT, digest, reload_service


class Backend:
    def __init__(self):
        self.now = 0
        self.current_pid = 10
        self.signals = []
        self.source = {"visual_frame_service.py": "fixture"}
        self.settings = {"environment": "fixture"}
        self.others = {"engine": 20, "agent": 30}
        self.quiet = True
        self.contract = "legacy_no_handshake"
        self.advance = True
        self.receipt_valid = True
        self.replace = True
        self.on_validate = lambda: None
        self.on_sleep = lambda: None
        self.on_observe = lambda: None

    def validate(self):
        self.on_validate()

    def maintenance(self):
        pass

    def inputs(self):
        return deepcopy(self.source)

    def config(self):
        return deepcopy(self.settings)

    def protected(self):
        return deepcopy(self.others)

    def pid(self):
        return self.current_pid

    def identity(self, pid):
        return f"start-{pid}" if pid == self.current_pid else None

    def observe(self, pid):
        self.on_observe()
        return {"quiet": self.quiet, "ts_ms": self.now if self.advance else 0,
                "contract": self.contract,
                "source_inputs_sha256_at_start": digest(self.source) if self.receipt_valid else "bad"}

    def terminate(self, pid):
        self.signals.append(pid)
        if self.replace:
            self.current_pid = 11
            self.contract = CONTRACT

    def sleep(self, seconds):
        self.now += seconds
        self.on_sleep()


class ReloadTests(unittest.TestCase):
    def call(self, backend, **options):
        return reload_service(backend, 10, timeout_s=30, now=lambda: backend.now,
                              sleep=backend.sleep, **options)

    def test_legacy_requires_explicit_ack(self):
        backend = Backend()
        with self.assertRaisesRegex(RuntimeError, "not acknowledged"):
            self.call(backend)
        self.assertEqual(backend.signals, [])

    def test_success_one_signal_and_exact_protected_identities(self):
        backend = Backend()
        result = self.call(backend, allow_legacy=True)
        self.assertEqual(result["outcome"], "success")
        self.assertFalse(result["atomic_admission_drain_claimed"])
        self.assertFalse(result["forced_termination"])
        self.assertEqual(result["protected"], backend.others)
        self.assertEqual(backend.signals, [10])

    def test_new_contract_requires_no_legacy_ack(self):
        backend = Backend()
        backend.contract = CONTRACT
        self.assertEqual(self.call(backend)["new_pid"], 11)

    def test_busy_or_stalled_status_sends_no_signal(self):
        for attr in ("quiet", "advance"):
            with self.subTest(attr=attr):
                backend = Backend()
                setattr(backend, attr, False)
                with self.assertRaisesRegex(RuntimeError, "no bounded"):
                    self.call(backend, allow_legacy=True)
                self.assertEqual(backend.signals, [])

    def test_source_config_and_protected_drift_refused(self):
        for attr in ("source", "settings", "others"):
            with self.subTest(attr=attr):
                backend = Backend()
                backend.on_sleep = lambda: getattr(backend, attr).update(changed=1)
                with self.assertRaisesRegex(RuntimeError, "drift"):
                    self.call(backend, allow_legacy=True)
                self.assertEqual(backend.signals, [])

    def test_foreign_pause_change_or_preflight_denial(self):
        backend = Backend()
        def denied():
            if backend.now:
                raise RuntimeError("preflight denied")
        backend.on_validate = denied
        with self.assertRaisesRegex(RuntimeError, "denied"):
            self.call(backend, allow_legacy=True)
        self.assertEqual(backend.signals, [])

    def test_pid_replaced_while_waiting(self):
        backend = Backend()
        backend.on_sleep = lambda: setattr(backend, "current_pid", 12)
        with self.assertRaisesRegex(RuntimeError, "PID changed"):
            self.call(backend, allow_legacy=True)
        self.assertEqual(backend.signals, [])

    def test_request_arrives_at_signal_boundary(self):
        backend = Backend()
        backend.on_validate = lambda: setattr(backend, "quiet", backend.now == 0)
        with self.assertRaisesRegex(RuntimeError, "boundary moved"):
            self.call(backend, allow_legacy=True)
        self.assertEqual(backend.signals, [])

    def test_readiness_failure_never_forces(self):
        for attr in ("receipt_valid", "replace"):
            with self.subTest(attr=attr):
                backend = Backend()
                setattr(backend, attr, False)
                with self.assertRaisesRegex(RuntimeError, "no forced fallback"):
                    self.call(backend, allow_legacy=True)
                self.assertEqual(backend.signals, [10])

    def test_receipt_persistence_failure_prevents_signal(self):
        backend = Backend()
        def emit(event):
            if event.get("phase") == "signal_boundary":
                raise OSError("receipt fsync failed")
        with self.assertRaises(OSError):
            self.call(backend, allow_legacy=True, emit=emit)
        self.assertEqual(backend.signals, [])

    def test_unknown_contract_refused(self):
        backend = Backend()
        backend.contract = "future-contract"
        with self.assertRaisesRegex(RuntimeError, "unknown"):
            self.call(backend, allow_legacy=True)
        self.assertEqual(backend.signals, [])


if __name__ == "__main__":
    unittest.main()
