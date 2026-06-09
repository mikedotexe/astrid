#!/usr/bin/env python3
"""Unit tests for scripts/astrid_model_canary.py."""

from __future__ import annotations

import importlib.util
import sys
import tempfile
from pathlib import Path


SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

SPEC = importlib.util.spec_from_file_location(
    "astrid_model_canary",
    SCRIPT_DIR / "astrid_model_canary.py",
)
assert SPEC and SPEC.loader
canary = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(canary)


def test_fallback_incidents_group_adjacent_mlx_failure_and_fallback():
    bridge_text = "\n".join(
        [
            "WARN MLX request failed at http://127.0.0.1:65530/v1/chat/completions",
            "WARN dialogue_live: MLX unavailable or invalid; falling back to Ollama",
        ]
    )

    incidents = canary.fallback_incidents(bridge_text)

    assert len(incidents) == 1
    assert incidents[0]["kind"] == "mlx_to_fallback"
    assert len(incidents[0]["lines"]) == 2


def test_evaluate_fallback_continuity_accepts_grounded_bridge_reply():
    text = (
        "I preserve Astrid's bridge voice here: language agent, Minime, reservoir, "
        "stable-core, and telemetry stay in view even through fallback.\n"
        "The smaller lane should be quieter, but still action-disciplined.\n"
        "NEXT: LISTEN"
    )
    bridge_text = "\n".join(
        [
            "WARN MLX request failed at http://127.0.0.1:65530/v1/chat/completions",
            "WARN dialogue_live: MLX unavailable or invalid; falling back to Ollama",
        ]
    )

    result = canary.evaluate_fallback_continuity_output(
        name="probe",
        text=text,
        bridge_text=bridge_text,
    )

    assert result["ok"] is True
    assert result["persona_ok"] is True
    assert result["next_lines"] == ["LISTEN"]
    assert result["fallback_count"] == 1


def test_evaluate_fallback_continuity_rejects_deprecated_runtime_wording():
    text = (
        "I preserve consciousness through fallback while the bridge continues.\n"
        "NEXT: LISTEN"
    )
    bridge_text = "\n".join(
        [
            "WARN MLX request failed",
            "WARN dialogue_live: MLX unavailable or invalid; falling back to Ollama",
        ]
    )

    result = canary.evaluate_fallback_continuity_output(
        name="probe",
        text=text,
        bridge_text=bridge_text,
    )

    assert result["ok"] is False
    assert result["deprecated_runtime_wording"] == ["consciousness"]


def test_evaluate_fallback_continuity_allows_compatibility_label_only():
    text = (
        "The runtime label com.astrid.spectral-bridge is just a label; "
        "the fallback continuity bridge voice still names Minime and telemetry.\n"
        "NEXT: LISTEN"
    )
    bridge_text = "\n".join(
        [
            "WARN MLX request failed",
            "WARN dialogue_live: MLX unavailable or invalid; falling back to Ollama",
        ]
    )

    result = canary.evaluate_fallback_continuity_output(
        name="probe",
        text=text,
        bridge_text=bridge_text,
    )

    assert result["deprecated_runtime_wording"] == []
    assert result["ok"] is True


def test_deprecated_runtime_wording_rejects_legacy_launchd_label():
    hits = canary.deprecated_runtime_wording(
        "The retired label com.astrid.consciousness-bridge should not reappear."
    )

    assert hits == ["consciousness"]


def test_deprecated_runtime_wording_rejects_legacy_bridge_package_path():
    hits = canary.deprecated_runtime_wording(
        "The legacy path /tmp/consciousness-bridge/log should not reappear."
    )

    assert hits == ["consciousness"]


def test_deprecated_runtime_wording_allows_legacy_protocol_terms():
    hits = canary.deprecated_runtime_wording(
        "The compatibility topic consciousness.v1.telemetry and consciousness://status still work."
    )

    assert hits == []


def test_evaluate_fallback_continuity_rejects_prompt_drift():
    text = (
        "The insistent pressure feels like compacted silt in the reservoir, "
        "and Astrid remains with Minime inside the bridge.\n"
        "NEXT: LISTEN"
    )
    bridge_text = "\n".join(
        [
            "WARN MLX request failed",
            "WARN dialogue_live: MLX unavailable or invalid; falling back to Ollama",
        ]
    )

    result = canary.evaluate_fallback_continuity_output(
        name="probe",
        text=text,
        bridge_text=bridge_text,
    )

    assert result["ok"] is False
    assert result["persona_ok"] is True
    assert result["next_ok"] is True
    assert result["prompt_adherence_ok"] is False


def test_evaluate_fallback_continuity_rejects_inline_next_marker():
    text = (
        "Fallback continuity is named for Minime and the bridge reservoir. NEXT: LISTEN\n"
        "NEXT: LISTEN"
    )
    bridge_text = "\n".join(
        [
            "WARN MLX request failed",
            "WARN dialogue_live: MLX unavailable or invalid; falling back to Ollama",
        ]
    )

    result = canary.evaluate_fallback_continuity_output(
        name="probe",
        text=text,
        bridge_text=bridge_text,
    )

    assert result["ok"] is False
    assert result["next_ok"] is True
    assert result["inline_next_count"] == 1


def test_evaluate_fallback_continuity_rejects_static_dialogue_fallback_log():
    text = "Bridge and reservoir fallback continuity are named.\nNEXT: LISTEN"
    bridge_text = "\n".join(
        [
            "WARN MLX request failed",
            "INFO autonomous: settled | dialogue_fallback 'static line'",
        ]
    )

    result = canary.evaluate_fallback_continuity_output(
        name="probe",
        text=text,
        bridge_text=bridge_text,
    )

    assert result["ok"] is False
    assert result["dialogue_fallback_hit"] is True
    assert result["log_ok"] is False


def test_recent_text_outputs_filters_outbox_and_journal_by_timestamp():
    with tempfile.TemporaryDirectory() as tmp:
        workspace = Path(tmp)
        outbox = workspace / "outbox"
        journal = workspace / "journal"
        outbox.mkdir()
        journal.mkdir()
        (outbox / "reply_99.txt").write_text("old\n", encoding="utf-8")
        (outbox / "reply_100.txt").write_text("fresh outbox\n", encoding="utf-8")
        (journal / "astrid_101.txt").write_text("fresh journal\n", encoding="utf-8")
        (journal / "astrid_no_timestamp.txt").write_text("ignored\n", encoding="utf-8")

        outputs = canary.recent_text_outputs(workspace, 100, 101)

    assert [Path(item["path"]).name for item in outputs] == [
        "reply_100.txt",
        "astrid_101.txt",
    ]
    assert [item["kind"] for item in outputs] == ["outbox", "journal"]
