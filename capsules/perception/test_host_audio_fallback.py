import unittest
import json
import tempfile
import time
from pathlib import Path

from host_audio_fallback import build_host_audio_perception, host_audio_features


class HostAudioFallbackTests(unittest.TestCase):
    def test_builds_synthetic_audio_perception_from_host_telemetry(self):
        telemetry = {
            "updated_at_ms": 123,
            "snapshot": {
                "cpu": 0.25,
                "cpu_imbalance": 0.20,
                "mem": 0.60,
                "swap": 0.0,
                "process_density": 0.75,
                "load": 0.40,
                "net_flux": 0.90,
                "disk_flux": 0.80,
            },
        }

        perception = build_host_audio_perception(
            telemetry,
            reason="mic_recording_unavailable",
            telemetry_path="/tmp/host_telemetry.json",
            timestamp="2026-05-09T08:00:00",
        )

        self.assertEqual(perception["type"], "audio")
        self.assertEqual(perception["source"], "host_synthetic")
        self.assertTrue(perception["synthetic"])
        self.assertIn("not speech", perception["transcript"])
        self.assertIn("mic_recording_unavailable", perception["transcript"])
        self.assertGreater(perception["features"]["rms_energy"], 0.0)
        self.assertGreater(perception["features"]["dynamic_range"], 1.0)

    def test_features_are_clamped_and_stable(self):
        telemetry = {
            "snapshot": {
                "cpu": 4.0,
                "cpu_imbalance": -2.0,
                "mem": "0.5",
                "swap": None,
                "process_density": 0.25,
                "load": 0.25,
                "net_flux": 0.25,
                "disk_flux": 0.25,
            }
        }

        features = host_audio_features(telemetry)
        components = features["components"]

        self.assertEqual(components["cpu"], 1.0)
        self.assertEqual(components["cpu_imbalance"], 0.0)
        self.assertEqual(components["mem"], 0.5)
        self.assertEqual(components["swap"], 0.0)
        self.assertFalse(features["is_music_likely"])

    def test_perception_loop_fallback_writes_temp_host_audio_artifact(self):
        import perception

        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            perceptions = root / "perceptions"
            perceptions.mkdir()
            telemetry_path = root / "host_telemetry.json"
            telemetry_path.write_text(json.dumps({
                "updated_at_ms": int(time.time() * 1000),
                "snapshot": {
                    "cpu": 0.2,
                    "cpu_imbalance": 0.1,
                    "mem": 0.4,
                    "swap": 0.0,
                    "process_density": 0.3,
                    "load": 0.25,
                    "net_flux": 0.6,
                    "disk_flux": 0.5,
                },
            }))

            old = {
                "WHISPER_AVAILABLE": perception.WHISPER_AVAILABLE,
                "PERCEPTIONS_DIR": perception.PERCEPTIONS_DIR,
                "AUDIO_SOURCE_STATUS_PATH": perception.AUDIO_SOURCE_STATUS_PATH,
                "HOST_TELEMETRY_PATH": perception.HOST_TELEMETRY_PATH,
            }
            try:
                perception.WHISPER_AVAILABLE = False
                perception.PERCEPTIONS_DIR = perceptions
                perception.AUDIO_SOURCE_STATUS_PATH = root / "audio_source_status.json"
                perception.HOST_TELEMETRY_PATH = telemetry_path

                result = perception.perceive_audio_with_fallback()

                self.assertIsNotNone(result)
                self.assertEqual(result["source"], "host_synthetic")
                self.assertTrue(result["synthetic"])
                artifacts = list(perceptions.glob("audio_*.json"))
                self.assertEqual(len(artifacts), 1)
                status = json.loads(perception.AUDIO_SOURCE_STATUS_PATH.read_text())
                self.assertEqual(status["source"], "host_synthetic")
                self.assertFalse(status["physical_healthy"])
                self.assertEqual(status["reason"], "whisper_unavailable")
            finally:
                for key, value in old.items():
                    setattr(perception, key, value)


if __name__ == "__main__":
    unittest.main()
