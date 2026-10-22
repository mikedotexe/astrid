"""Read-only host memory sampling for an isolated model qualification batch."""
from __future__ import annotations

import math
import re
import subprocess
import threading
import time

STRICT_POLICY = "mlx_host_memory_strict_v1"
SWAP_DIAGNOSTIC_POLICY = "mlx_host_memory_swap_diagnostic_v2"
STAGE2_DIAGNOSTIC_POLICY = "mlx_stage2_host_memory_diagnostic_v5"
POLICIES = (STRICT_POLICY, SWAP_DIAGNOSTIC_POLICY, STAGE2_DIAGNOSTIC_POLICY)


def sample():
    result = subprocess.run(["sysctl", "kern.memorystatus_vm_pressure_level", "vm.swapusage"],
                            capture_output=True, text=True, timeout=5, check=True)
    pressure = re.search(r"memorystatus_vm_pressure_level:\s*(\d+)", result.stdout)
    swap = re.search(r"used\s*=\s*([\d.]+)([KMGT])", result.stdout)
    if not pressure or not swap:
        raise ValueError("host memory evidence is unavailable")
    return {"t_s": time.time(), "pressure_level": int(pressure.group(1)),
            "swap_bytes": float(swap.group(1)) * 1024 ** ("KMGT".index(swap.group(2)) + 1)}


def evaluate(samples, *, policy=STRICT_POLICY):
    if policy not in POLICIES:
        raise ValueError("unknown host memory policy")
    if policy == STAGE2_DIAGNOSTIC_POLICY:
        valid = [row for row in samples if (isinstance(row, dict) and "error" not in row
                 and all(type(row.get(key)) in (int, float) and math.isfinite(row[key])
                         for key in ("t_s", "pressure_level", "swap_bytes"))
                 and row["pressure_level"] >= 1 and row["swap_bytes"] >= 0)]
        ordered = all(a["t_s"] < b["t_s"] <= a["t_s"] + 15
                      for a, b in zip(valid, valid[1:]))
        telemetry_complete = len(valid) == len(samples) and len(valid) >= 2 and ordered
        return {"automated_ok": True, "failure_reasons": [], "policy": policy,
                "swap_is_diagnostic": True, "pressure_is_diagnostic": True,
                "telemetry_complete": telemetry_complete,
                "telemetry_error_count": len(samples) - len(valid),
                "sample_count": len(samples),
                "peak_pressure_level": max((row["pressure_level"] for row in valid), default=None),
                "retained_swap_growth_bytes": valid[-1]["swap_bytes"] - valid[0]["swap_bytes"]
                    if valid else None,
                "scope": "Stage 2 host pressure, swap and telemetry are diagnostics; service/output/OOM gates are separate"}
    diagnostic = policy == SWAP_DIAGNOSTIC_POLICY
    failures = []
    if len(samples) < 2 or any("error" in row for row in samples):
        return {"automated_ok": False, "failure_reasons": ["missing_host_memory_samples"]}
    if any(not isinstance(row.get(key), (int, float)) or isinstance(row.get(key), bool)
           or not math.isfinite(row[key]) for row in samples
           for key in ("t_s", "pressure_level", "swap_bytes")):
        return {"automated_ok": False, "failure_reasons": ["invalid_host_memory_samples"]}
    if (any(row["pressure_level"] < 1 or row["pressure_level"] != int(row["pressure_level"])
            or row["swap_bytes"] < 0 for row in samples)
            or any(b["t_s"] <= a["t_s"] for a, b in zip(samples, samples[1:]))):
        return {"automated_ok": False, "failure_reasons": ["invalid_host_memory_samples"]}
    if any(b["t_s"] - a["t_s"] > 15 for a, b in zip(samples, samples[1:])):
        failures.append("host_memory_sampling_gap")
    start = None
    sustained = 0.
    initial_pressure = samples[0]["pressure_level"]
    if diagnostic and any(row["pressure_level"] >= 3 for row in samples):
        failures.append("critical_host_memory_pressure")
    if not diagnostic and initial_pressure != 1:
        failures.append("initial_host_memory_pressure_not_normal")
    for row in samples:
        if (row["pressure_level"] >= 2 if diagnostic else row["pressure_level"] > initial_pressure):
            start = row["t_s"] if start is None else start
            sustained = max(sustained, row["t_s"] - start)
        else:
            start = None
    if sustained >= 60:
        failures.append("sustained_additional_host_memory_pressure")
    growth = samples[-1]["swap_bytes"] - samples[0]["swap_bytes"]
    if growth > 0 and not diagnostic:
        failures.append("retained_swap_growth")
    swap_start = None
    sustained_swap = 0.
    for row in samples:
        if row["swap_bytes"] > samples[0]["swap_bytes"]:
            swap_start = row["t_s"] if swap_start is None else swap_start
            sustained_swap = max(sustained_swap, row["t_s"] - swap_start)
        else:
            swap_start = None
    if sustained_swap >= 60 and not diagnostic:
        failures.append("sustained_additional_swap")
    return {"automated_ok": not failures, "failure_reasons": failures, "policy": policy,
            "swap_is_diagnostic": diagnostic,
            "initial_swap_bytes": samples[0]["swap_bytes"],
            "final_swap_bytes": samples[-1]["swap_bytes"],
            "initial_pressure_level": initial_pressure,
            "peak_pressure_level": max(row["pressure_level"] for row in samples),
            "additional_pressure_longest_s": sustained,
            "retained_swap_growth_bytes": growth,
            "additional_swap_longest_s": sustained_swap,
            "peak_additional_swap_bytes": max(row["swap_bytes"] for row in samples) - samples[0]["swap_bytes"],
            "scope": "whole host; live workloads can confound attribution; failure requires a new reviewed plan"}


class Monitor:
    def __init__(self, *, policy=STRICT_POLICY):
        if policy not in POLICIES:
            raise ValueError("unknown host memory policy")
        self.policy = policy
        self.samples = []
        self.stop_event = threading.Event()
        self.thread = threading.Thread(target=self._run, daemon=True)

    def _sample(self):
        try:
            self.samples.append(sample())
        except Exception as error:
            self.samples.append({"t_s": time.time(), "error": str(error)})

    def _run(self):
        while not self.stop_event.wait(5):
            self._sample()

    def start(self):
        self._sample()
        if self.policy == STAGE2_DIAGNOSTIC_POLICY:
            self.thread.start()
            return
        row = self.samples[-1]
        valid = (not row.get("error")
                 and all(isinstance(row.get(k), (int, float)) and not isinstance(row.get(k), bool)
                         and math.isfinite(row[k]) for k in ("t_s", "pressure_level", "swap_bytes"))
                 and row["swap_bytes"] >= 0)
        allowed = ((1,) if self.policy == STRICT_POLICY else
                   (1, 2) if self.policy == SWAP_DIAGNOSTIC_POLICY else None)
        if (not valid or row.get("pressure_level", 0) < 1
                or (allowed is not None and row["pressure_level"] not in allowed)):
            raise ValueError("qualification requires normal host memory pressure before workload startup"
                             if self.policy == STRICT_POLICY else
                             "qualification requires valid noncritical host memory telemetry")
        self.thread.start()

    def stop(self):
        self.stop_event.set()
        if self.thread.is_alive():
            self.thread.join(timeout=10)
        self._sample()
        return {**evaluate(self.samples, policy=self.policy), "samples": self.samples}
