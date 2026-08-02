#!/usr/bin/env python3
"""Operator-owned, one-shot relay for voluntary AVADO/ICP peer packets.

The appliances receive no credentials for one another. This process reads only
signed outbox packets and the public verification key over operator SSH, then
creates owner-only inbox/trust files on the other appliance. It never reads or
copies journals, databases, thread state, model output, or arbitrary artifacts.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import subprocess
import sys
from dataclasses import dataclass


PACKET = re.compile(r"^peer_[A-Za-z0-9_]{1,90}\.json$")
HEX_KEY = re.compile(r"^[0-9a-f]{64}$")


@dataclass(frozen=True)
class Appliance:
    ssh: str
    workspace: str


PRESETS = {
    "avado": Appliance("avado", "/home/avado/.astrid/home/default/edge"),
    "icp": Appliance(
        "icp", "/home/nativeplanet/.astrid-icp/state/home/default/edge"
    ),
}


def ssh_read(appliance: Appliance, relative: str) -> bytes | None:
    command = ["ssh", appliance.ssh, "--", "cat", f"{appliance.workspace}/{relative}"]
    result = subprocess.run(command, capture_output=True, check=False)
    return result.stdout if result.returncode == 0 else None


def ssh_list(appliance: Appliance) -> list[str]:
    command = [
        "ssh",
        appliance.ssh,
        "--",
        "find",
        f"{appliance.workspace}/peer/outbox",
        "-maxdepth",
        "1",
        "-type",
        "f",
        "-name",
        "peer_*.json",
        "-printf",
        "%f\n",
    ]
    result = subprocess.run(command, capture_output=True, text=True, check=False)
    if result.returncode != 0:
        return []
    return sorted(name for name in result.stdout.splitlines() if PACKET.fullmatch(name))


def install_if_absent(appliance: Appliance, relative: str, content: bytes) -> bool:
    target = f"{appliance.workspace}/{relative}"
    command = [
        "ssh",
        appliance.ssh,
        "--",
        "sh",
        "-c",
        'umask 077; target="$1"; mkdir -p "${target%/*}"; '
        'if test -e "$target"; then exit 3; fi; '
        'temporary="${target}.relay.$$"; trap \'rm -f "$temporary"\' EXIT; '
        'cat >"$temporary" && chmod 600 "$temporary" && mv -n "$temporary" "$target"',
        "relay-edge-peer-review",
        target,
    ]
    result = subprocess.run(command, input=content, capture_output=True, check=False)
    return result.returncode == 0


def validate_packet(content: bytes, expected_name: str) -> None:
    if len(content) > 16_384:
        raise ValueError("packet exceeds relay byte cap")
    value = json.loads(content)
    if value.get("schema") != "astrid_edge_peer_review_packet_v1":
        raise ValueError("unsupported packet schema")
    if f"{value.get('packet_id', '')}.json" != expected_name:
        raise ValueError("packet identifier does not match filename")
    if not HEX_KEY.fullmatch(str(value.get("signing_public_key", ""))):
        raise ValueError("packet contains no bounded Ed25519 public key")


def relay_direction(source: Appliance, destination: Appliance) -> tuple[int, int]:
    public_key = ssh_read(source, "peer/signing.pub")
    if public_key is None:
        return (0, 0)
    public_text = public_key.decode("ascii", "strict").strip()
    if not HEX_KEY.fullmatch(public_text):
        raise ValueError(f"{source.ssh} has an invalid peer public key")
    fingerprint = hashlib.sha256(bytes.fromhex(public_text)).hexdigest()
    install_if_absent(
        destination, f"peer/trusted/{fingerprint}.pub", f"{public_text}\n".encode()
    )

    considered = transferred = 0
    for name in ssh_list(source):
        considered += 1
        packet = ssh_read(source, f"peer/outbox/{name}")
        if packet is None:
            continue
        validate_packet(packet, name)
        if install_if_absent(destination, f"peer/inbox/{name}", packet):
            transferred += 1
    return considered, transferred


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--preset", default="avado-icp", choices=["avado-icp"])
    args = parser.parse_args()
    del args
    try:
        forward = relay_direction(PRESETS["avado"], PRESETS["icp"])
        reverse = relay_direction(PRESETS["icp"], PRESETS["avado"])
    except (OSError, UnicodeError, ValueError, json.JSONDecodeError) as error:
        print(f"peer relay failed: {error}", file=sys.stderr)
        return 1
    print(
        json.dumps(
            {
                "schema": "astrid_edge_operator_peer_relay_v1",
                "avado_to_icp": {"considered": forward[0], "transferred": forward[1]},
                "icp_to_avado": {"considered": reverse[0], "transferred": reverse[1]},
                "authority": "operator_owned_ssh_relay_no_appliance_peer_credentials",
            },
            sort_keys=True,
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
