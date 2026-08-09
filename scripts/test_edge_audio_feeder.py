#!/usr/bin/env python3
"""Focused tests for the immutable direct-ALSA numeric feeder."""

from __future__ import annotations

import importlib.util
import json
import math
import sys
import unittest
from pathlib import Path


MODULE_PATH = Path(__file__).with_name("edge_audio_feeder.py")
REPOSITORY_ROOT = Path(__file__).resolve().parents[1]
SERVICE_UNIT = REPOSITORY_ROOT / "packaging/systemd/astrid-edge-audio-feeder.service"
SOCKET_UNIT = REPOSITORY_ROOT / "packaging/systemd/astrid-edge-audio-feeder.socket.in"
SPEC = importlib.util.spec_from_file_location("edge_audio_feeder", MODULE_PATH)
assert SPEC is not None and SPEC.loader is not None
feeder = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = feeder
SPEC.loader.exec_module(feeder)


def config() -> object:
    return feeder.Config.parse(
        {
            "schema": feeder.CONFIG_SCHEMA,
            "appliance_id": "avado-edge",
            "device": "hw:1,0",
            "sample_rate": 16_000,
            "channels": 1,
            "expected_peer_uid": 1001,
            "libasound_path": "/usr/lib/x86_64-linux-gnu/libasound.so.2.0.0",
            "libasound_sha256": "a" * 64,
        }
    )


class AudioFeederTests(unittest.TestCase):
    def test_systemd_authority_is_direct_alsa_and_dedicated_client_socket_only(self) -> None:
        service = SERVICE_UNIT.read_text()
        socket_unit = SOCKET_UNIT.read_text()
        for directive in (
            "User=astrid-edge-audio",
            "PrivateNetwork=yes",
            "DevicePolicy=closed",
            "DeviceAllow=char-alsa rw",
            "NoExecPaths=/",
            "ExecPaths=/usr/bin/python3 /usr/lib /lib /lib64",
        ):
            self.assertIn(directive, service)
        self.assertIn("SocketUser=root", socket_unit)
        self.assertIn("SocketGroup=@AUDIO_CLIENT_GROUP@", socket_unit)
        self.assertNotIn("@RUNTIME_GROUP@", socket_unit)
        self.assertNotIn("arecord", service)

    def test_configuration_is_exact_and_bounded(self) -> None:
        valid = config()
        self.assertEqual(valid.device, "hw:1,0")
        value = {
            "schema": feeder.CONFIG_SCHEMA,
            "appliance_id": "avado-edge",
            "device": "../../dev/snd",
            "sample_rate": 16_000,
            "channels": 1,
            "expected_peer_uid": 1001,
            "libasound_path": "/tmp/libasound.so",
            "libasound_sha256": "a" * 64,
        }
        with self.assertRaises(feeder.FeederError):
            feeder.Config.parse(value)
        value["device"] = "hw:1,0"
        value["command"] = "arecord"
        with self.assertRaises(feeder.FeederError):
            feeder.Config.parse(value)
        del value["command"]
        value["libasound_path"] = "/usr/lib/../tmp/libasound.so.2"
        with self.assertRaises(feeder.FeederError):
            feeder.Config.parse(value)

    def test_features_are_numeric_bounded_and_distinguish_tone(self) -> None:
        silence = [0.0] * 1_600
        tone = [
            0.4 * math.sin(2.0 * math.pi * 1_000.0 * index / 16_000.0)
            for index in range(1_600)
        ]
        silence_features = feeder.extract_features(silence, 16_000)
        tone_features = feeder.extract_features(tone, 16_000)
        self.assertEqual(len(tone_features), 8)
        self.assertTrue(all(math.isfinite(value) and -1 <= value <= 1 for value in tone_features))
        self.assertGreater(tone_features[0], silence_features[0])
        self.assertGreater(tone_features[3], tone_features[2])

    def test_frame_contains_no_pcm_command_path_or_free_text(self) -> None:
        payload = feeder.frame_bytes(config(), 1, [0.25] * 8)
        self.assertLessEqual(len(payload), feeder.MAX_FRAME_BYTES)
        value = json.loads(payload)
        self.assertEqual(
            set(value),
            {"channels", "device", "features", "sample_rate", "schema", "sequence", "source"},
        )
        rendered = payload.decode("ascii")
        self.assertNotIn("pcm", rendered.lower())
        self.assertNotIn("arecord", rendered.lower())
        self.assertNotIn("prompt", rendered.lower())

    def test_nonfinite_and_wrong_width_frames_are_rejected(self) -> None:
        with self.assertRaises(feeder.FeederError):
            feeder.frame_bytes(config(), 1, [0.0] * 7)
        with self.assertRaises(feeder.FeederError):
            feeder.frame_bytes(config(), 1, [float("nan")] * 8)


if __name__ == "__main__":
    unittest.main()
