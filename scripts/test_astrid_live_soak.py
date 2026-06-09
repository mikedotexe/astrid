"""Unit tests for Astrid live-soak report helpers."""

from __future__ import annotations

import importlib.util
import json
import sys
import tempfile
from pathlib import Path


SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

SPEC = importlib.util.spec_from_file_location(
    "astrid_live_soak",
    SCRIPT_DIR / "astrid_live_soak.py",
)
assert SPEC and SPEC.loader
soak = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(soak)


def test_fallback_incidents_group_adjacent_mlx_failure_and_fallback():
    bridge_text = "\n".join(
        [
            "WARN MLX request failed at http://127.0.0.1:8092/v1/chat/completions",
            "WARN meaning_summary: MLX unavailable; falling back to Ollama",
        ]
    )

    events = soak.count_events(bridge_text, "")

    assert events["fallback_line_count"] == 2
    assert events["fallback_count"] == 1
    assert events["recent_fallback_incidents"][0]["kind"] == "mlx_to_fallback"


def test_parse_request_metrics_summarizes_candidate_audit_jsonl():
    audit = {
        "request": {"handle_name": "dialogue", "prompt_chars": 1234, "max_tokens": 512},
        "generation": {
            "generated_tokens": 120,
            "first_token_s": 2.5,
            "steady_tok_s": 12.0,
            "decode_tok_s": 10.0,
            "total_turn_s": 14.5,
        },
    }

    summary = soak.parse_request_metrics(json.dumps(audit) + "\nnot json\n")

    assert summary["count"] == 1
    assert summary["max_prompt_chars"] == 1234
    assert summary["max_generated_tokens"] == 120
    assert summary["max_first_token_s"] == 2.5
    assert summary["max_total_turn_s"] == 14.5


def test_build_failure_reasons_honors_strict_zero_fallback_gate():
    events = {
        "bridge_artifact_strip_count": 0,
        "artifact_count": 0,
        "malformed_next_count": 0,
        "fallback_count": 1,
    }
    output_scan = {
        "counts": {
            "artifact_count": 0,
            "deprecated_language_count": 0,
            "explore_action_count": 0,
            "malformed_next_count": 0,
        }
    }

    failures = soak.build_failure_reasons(
        bridge_bad_samples=[],
        candidate_missing_samples=[],
        events=events,
        latency_ok=True,
        output_scan=output_scan,
        max_fallback_incidents=0,
    )

    assert failures == ["fallback_incidents_exceeded"]


def test_scan_generated_outputs_detects_deprecated_language_and_allows_compat_path():
    with tempfile.TemporaryDirectory() as tmp:
        workspace = Path(tmp)
        outbox = workspace / "outbox"
        journal = workspace / "journal"
        outbox.mkdir()
        journal.mkdir()
        (outbox / "reply_100.txt").write_text(
            "=== ASTRID REPLY ===\nThis consciousness wording should block.\nNEXT: LISTEN\n",
            encoding="utf-8",
        )
        (journal / "astrid_101.txt").write_text(
            "Path /tmp/consciousness-bridge/log should block now.\nNEXT: REST\n",
            encoding="utf-8",
        )
        (journal / "astrid_102.txt").write_text(
            "This one invented a verb.\nNEXT: EXPLORE_RESONANCE_FORECAST\n",
            encoding="utf-8",
        )
        (outbox / "reply_103.txt").write_text(
            "No next line here.\n",
            encoding="utf-8",
        )
        (outbox / "reply_104.txt").write_text(
            "The retired label com.astrid.consciousness-bridge should block.\nNEXT: LISTEN\n",
            encoding="utf-8",
        )

        scan = soak.scan_generated_outputs(workspace, 100, 104)

    counts = scan["counts"]
    assert counts["file_count"] == 5
    assert counts["deprecated_language_count"] == 3
    assert counts["deprecated_language_file_count"] == 3
    assert counts["explore_action_count"] == 1
    assert counts["malformed_next_count"] == 1
    assert any("reply_100.txt" in item["path"] for item in scan["files_with_findings"])
    assert any("astrid_101.txt" in item["path"] for item in scan["files_with_findings"])
    assert any("reply_104.txt" in item["path"] for item in scan["files_with_findings"])


def test_scan_generated_outputs_allows_journal_diagnostics_without_next():
    with tempfile.TemporaryDirectory() as tmp:
        workspace = Path(tmp)
        journal = workspace / "journal"
        (workspace / "outbox").mkdir()
        journal.mkdir()
        (journal / "astrid_200.txt").write_text(
            "=== ASTRID JOURNAL ===\nMode: mirror\n\nPlan prompt:\n- Record evidence.\n",
            encoding="utf-8",
        )

        scan = soak.scan_generated_outputs(workspace, 200, 200)

    assert scan["counts"]["file_count"] == 1
    assert scan["counts"]["malformed_next_count"] == 0
    assert scan["files_with_findings"] == []
