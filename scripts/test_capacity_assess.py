#!/usr/bin/env python3
"""Lock the reservoir_capacity assess severity logic (steward-only probe).

Regression target (2026-06-13): minime PR utilization oscillates ~41-80% under
varying load, so a single-sample >=0.70 threshold flapped WARNING ~half the
cycles. The probe must warn only on *sustained* saturation (recent median
>=0.70), treat an isolated high sample as a notice (signal preserved, never
dropped), and still escalate when saturation actually persists.

Run: python3 scripts/test_capacity_assess.py
"""
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from proactive_scan import _capacity_assess  # noqa: E402


def _rec(util: float) -> dict:
    return {
        "minime": {"utilization": util, "pr": round(util * 128, 1), "N": 128, "verdict": "moderate"},
        "triple": [],
    }


def _assert(cond: bool, msg: str) -> None:
    if not cond:
        print(f"FAIL: {msg}")
        sys.exit(1)
    print(f"ok: {msg}")


def test_capacity_assess_sustained_only() -> None:
    # Real-world oscillating window (last day of live history) ending on a high
    # transient sample -> must NOT warn (recent median is not sustained).
    osc = [0.599, 0.591, 0.578, 0.805, 0.467, 0.782, 0.453, 0.607, 0.557, 0.697, 0.774, 0.424, 0.653, 0.795]
    a = _capacity_assess([_rec(u) for u in osc])
    _assert(a["severity"] == "notice", f"oscillating window ending on 0.795 -> notice, got {a['severity']}")

    # Same window ending on a low sample -> ok.
    a = _capacity_assess([_rec(u) for u in osc + [0.408]])
    _assert(a["severity"] == "ok", f"oscillating window ending on 0.408 -> ok, got {a['severity']}")

    # Genuinely sustained saturation -> warning (real signal must survive).
    sustained = [0.72, 0.78, 0.75, 0.81, 0.74, 0.79]
    a = _capacity_assess([_rec(u) for u in sustained])
    _assert(a["severity"] == "warning", f"sustained >=0.70 -> warning, got {a['severity']}")
    _assert(any("sustained" in d for d in a["details"]), "sustained warning mentions 'sustained'")

    # Single high sample with no prior history -> recent median == that sample ->
    # warning is correct (one data point can't distinguish transient from real).
    a = _capacity_assess([_rec(0.82)])
    _assert(a["severity"] == "warning", f"lone high sample -> warning, got {a['severity']}")

    # Steady comfortable utilization -> ok.
    a = _capacity_assess([_rec(u) for u in [0.45, 0.50, 0.48, 0.52, 0.47]])
    _assert(a["severity"] == "ok", f"steady moderate -> ok, got {a['severity']}")

    print("\nall capacity-assess severity cases pass")


if __name__ == "__main__":
    test_capacity_assess_sustained_only()
