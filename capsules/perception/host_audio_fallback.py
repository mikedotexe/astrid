"""Host-state synthetic audio fallback for Astrid perception.

This is not speech synthesis and does not claim that the room made sound.
It turns fresh host telemetry into a compact audio-like perception when the
physical microphone/transcription lane is unavailable.
"""

from __future__ import annotations

from datetime import datetime
from typing import Any


HOST_AUDIO_KEYS = (
    "cpu",
    "cpu_imbalance",
    "mem",
    "swap",
    "process_density",
    "load",
    "net_flux",
    "disk_flux",
)


def _clamp_unit(value: float) -> float:
    return max(0.0, min(1.0, value))


def _snapshot_value(snapshot: dict[str, Any], key: str) -> float:
    try:
        return _clamp_unit(float(snapshot.get(key, 0.0) or 0.0))
    except (TypeError, ValueError):
        return 0.0


def _label(key: str) -> str:
    return key.replace("_", " ")


def host_audio_features(telemetry: dict[str, Any]) -> dict[str, Any]:
    """Map host telemetry into the same feature vocabulary as mic audio."""
    snapshot = telemetry.get("snapshot") or {}
    values = {key: _snapshot_value(snapshot, key) for key in HOST_AUDIO_KEYS}

    activity = (
        values["cpu"]
        + values["load"]
        + values["process_density"]
        + values["mem"]
    ) / 4.0
    flux = (values["net_flux"] + values["disk_flux"]) / 2.0
    pressure = (values["swap"] + values["cpu_imbalance"] + values["mem"]) / 3.0
    spread = max(values.values()) - min(values.values())

    return {
        "rms_energy": round(0.015 + (activity * 0.12) + (flux * 0.04), 4),
        "zero_crossing_rate": round(0.01 + (flux * 0.08), 4),
        "dynamic_range": round(1.0 + (spread * 4.0), 2),
        "temporal_variation": round(0.005 + (pressure * 0.08), 4),
        "is_music_likely": False,
        "synthetic": True,
        "components": values,
    }


def host_audio_transcript(telemetry: dict[str, Any], reason: str) -> str:
    snapshot = telemetry.get("snapshot") or {}
    values = {key: _snapshot_value(snapshot, key) for key in HOST_AUDIO_KEYS}
    dominant = sorted(values.items(), key=lambda item: item[1], reverse=True)[:3]
    dominant_text = ", ".join(f"{_label(key)} {value:.2f}" for key, value in dominant)
    return (
        "Host-state sonification fallback: synthetic system sound, not speech. "
        f"Physical audio lane unavailable ({reason}). "
        f"Dominant tones: {dominant_text}."
    )


def build_host_audio_perception(
    telemetry: dict[str, Any],
    *,
    reason: str,
    telemetry_path: str,
    timestamp: str | None = None,
) -> dict[str, Any]:
    ts = timestamp or datetime.now().isoformat()
    return {
        "type": "audio",
        "timestamp": ts,
        "source": "host_synthetic",
        "backend": "host_telemetry_sonification",
        "synthetic": True,
        "transcript": host_audio_transcript(telemetry, reason),
        "duration_s": 5,
        "features": host_audio_features(telemetry),
        "fallback_reason": reason,
        "telemetry_path": telemetry_path,
    }
