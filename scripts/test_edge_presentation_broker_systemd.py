#!/usr/bin/env python3
"""Static authority checks for the untrusted candidate presentation service."""

from __future__ import annotations

import json
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
UNIT = ROOT / "packaging/systemd/astrid-edge-presentation-broker@.service.in"
SOCKET = ROOT / "packaging/systemd/astrid-edge-presentation-broker.socket.in"
CONFIG = ROOT / "packaging/headless/edge-presentation-broker.json.in"


class PresentationBrokerSystemdTests(unittest.TestCase):
    def test_service_is_unprivileged_private_and_strictly_capped(self) -> None:
        text = UNIT.read_text()
        for exact in (
            "User=astrid-edge-presentation",
            "Group=astrid-edge-presentation",
            "StandardInput=socket",
            "StandardOutput=socket",
            "PrivateNetwork=yes",
            "ProtectHome=yes",
            "ProtectSystem=strict",
            "ProtectProc=invisible",
            "ProcSubset=pid",
            "NoNewPrivileges=yes",
            "CapabilityBoundingSet=",
            "MemoryMax=256M",
            "MemorySwapMax=0",
            "TasksMax=16",
            "CPUQuota=25%",
            "TimeoutStartSec=35s",
            "KillMode=control-group",
            "MemoryDenyWriteExecute=yes",
            "NoExecPaths=/",
            "ExecPaths=/usr/bin/python3 /usr/libexec/astrid-edge/immutable/astrid-edge-presentation-broker /usr/lib /lib /lib64",
            "BindReadOnlyPaths=@@RELEASE_PARENT@@:/opt/astrid-edge-presentation",
            "BindReadOnlyPaths=@@GENERATION_FILE@@:/run/astrid-edge-presentation-input/current-generation",
        ):
            self.assertIn(exact, text)
        self.assertNotIn("ReadWritePaths=", text)
        self.assertNotIn("EnvironmentFile=", text)
        self.assertNotIn("LoadCredential=", text)

    def test_socket_is_root_owned_runtime_readable_and_connection_scoped(self) -> None:
        text = SOCKET.read_text()
        for exact in (
            "ListenStream=/run/astrid-edge-presentation/broker.sock",
            "SocketUser=root",
            "SocketGroup=@@RUNTIME_GROUP@@",
            "SocketMode=0660",
            "DirectoryMode=0755",
            "Accept=yes",
            "MaxConnections=1",
            "Backlog=1",
        ):
            self.assertIn(exact, text)

    def test_config_contains_no_workspace_state_secret_or_output_path(self) -> None:
        value = json.loads(
            CONFIG.read_text()
            .replace("@@APPLIANCE_ID@@", "avado")
            .replace("@@TARGET@@", "x86_64-unknown-linux-gnu")
            .replace("@@PYTHON_PATH@@", "/usr/bin/python3.10")
            .replace("@@PYTHON_SHA256@@", "a" * 64)
        )
        encoded = json.dumps(value, sort_keys=True)
        self.assertNotIn("workspace", encoded)
        self.assertNotIn("secret", encoded)
        self.assertNotIn("key", encoded)
        self.assertNotIn("output_path", encoded)
        self.assertEqual(value["policy"]["memory_max_bytes"], 256 * 1024 * 1024)
        self.assertEqual(value["policy"]["maximum_stdout_bytes"], 32 * 1024)


if __name__ == "__main__":
    unittest.main()
