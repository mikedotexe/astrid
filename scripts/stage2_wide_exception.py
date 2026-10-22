"""Prospective, source-bound exception for disabled wide-path timing only.

This does not turn the original test into a pass. A caller must retain its
nonzero exit, log, and embedding audit and bind this decision in a new receipt.
"""
from __future__ import annotations

import math
import re


FINAL_FAILURE = re.compile(
    r"\n={60}\nFAIL:\n  - latency: k=16 (?P<failure_ms>\d+\.\d{2})ms > 1\.5ms\n\Z"
)
MEASUREMENT = re.compile(r"^  k=16: (?P<measured_ms>\d+\.\d{3}) ms/token  \[high\]$", re.M)
REQUIRED_SECTIONS = (
    "[a] coherence", "[b] bounded", "[c] latency per token",
    "[d] discriminative", "[e] off-is-off",
)


def evaluate(log: str, audit: dict, *, stage: str, qualification_target: str,
             actual_returncode: int, wide_coupling_strength: float, expected_source_root: str,
             expected_source_hashes: dict[str, str], expected_wrapper_sha256: str,
             expected_model: str) -> dict:
    reasons: list[str] = []
    if qualification_target != "full" or stage not in {"core", "full"}:
        reasons.append("wrong_qualification_scope")
    if type(actual_returncode) is not int or actual_returncode != 1:
        reasons.append("actual_test_exit_not_one")
    if type(wide_coupling_strength) not in (int, float) or wide_coupling_strength != 0:
        reasons.append("wide_path_not_disabled")
    if (audit.get("schema") != "astrid_mlx_embedding_gate_v1"
            or audit.get("passed") is not False
            or audit.get("error") != "SystemExit: 1"
            or audit.get("loader_calls") != 1
            or audit.get("load_attempts") != 1
            or audit.get("loader_restored") is not True
            or audit.get("policy_errors") != []
            or audit.get("memory_error")
            or audit.get("loading_policy") != "lazy_embeddings_only"):
        reasons.append("embedding_audit_not_exact_latency_failure")
    if (audit.get("source_root") != expected_source_root
            or audit.get("source_hashes") != expected_source_hashes
            or audit.get("wrapper_sha256") != expected_wrapper_sha256
            or audit.get("model") != expected_model):
        reasons.append("embedding_source_or_model_mismatch")
    positions = [log.find(section) for section in REQUIRED_SECTIONS]
    if any(position < 0 for position in positions) or positions != sorted(positions):
        reasons.append("missing_or_reordered_functional_sections")
    tail = FINAL_FAILURE.search(log)
    measured = MEASUREMENT.findall(log)
    if not tail or len(measured) != 1:
        reasons.append("not_exactly_one_disabled_wide_latency_failure")
        latency_ms = None
    else:
        latency_ms = float(measured[0])
        failure_ms = float(tail.group("failure_ms"))
        if not (math.isfinite(latency_ms) and latency_ms > 1.5
                and abs(latency_ms - failure_ms) <= 0.011):
            reasons.append("latency_measurement_disagrees_with_failure")
    return {"accepted_exception": not reasons, "reasons": reasons,
            "actual_test_passed": False, "actual_test_exit_code": actual_returncode,
            "measured_k16_ms_per_token": latency_ms,
            "scope": "disabled wide-coupling path timing only; functional gates and active workloads remain mandatory"}
