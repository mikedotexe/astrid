#!/usr/bin/env python3
"""Immutable direct-ALSA numeric feature feeder for CPU-edge Astrid.

The process opens libasound directly through ``ctypes``. It never invokes
``arecord`` or another process, and raw PCM never leaves this process.
"""

from __future__ import annotations

import argparse
import ctypes
import hashlib
import json
import math
import os
import re
import socket
import stat
import struct
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Iterable


CONFIG_SCHEMA = "astrid.edge.audio_feeder.config.v1"
FRAME_SCHEMA = "astrid.edge.audio_features.v1"
FRAME_SOURCE = "physical_alsa_numeric_feeder"
MAX_CONFIG_BYTES = 16 * 1024
MAX_LIBRARY_BYTES = 16 * 1024 * 1024
MAX_FRAME_BYTES = 1_024
DEVICE_PATTERN = re.compile(r"[A-Za-z0-9_:,.-]{1,64}\Z")
HEX64 = re.compile(r"[0-9a-f]{64}\Z")
SND_PCM_STREAM_CAPTURE = 1
SND_PCM_FORMAT_S16_LE = 2
SND_PCM_ACCESS_RW_INTERLEAVED = 3
SO_PEERCRED = 17


class FeederError(RuntimeError):
    """A fail-closed immutable feeder error."""


@dataclass(frozen=True)
class Config:
    appliance_id: str
    device: str
    sample_rate: int
    channels: int
    expected_peer_uid: int
    libasound_path: Path
    libasound_sha256: str

    @classmethod
    def parse(cls, value: Any) -> "Config":
        if not isinstance(value, dict) or set(value) != {
            "schema",
            "appliance_id",
            "device",
            "sample_rate",
            "channels",
            "expected_peer_uid",
            "libasound_path",
            "libasound_sha256",
        }:
            raise FeederError("audio feeder configuration fields are not exact")
        if value["schema"] != CONFIG_SCHEMA:
            raise FeederError("audio feeder configuration schema is unsupported")
        appliance_id = value["appliance_id"]
        device = value["device"]
        sample_rate = value["sample_rate"]
        channels = value["channels"]
        expected_peer_uid = value["expected_peer_uid"]
        library = value["libasound_path"]
        digest = value["libasound_sha256"]
        if (
            not isinstance(appliance_id, str)
            or not re.fullmatch(r"[a-z0-9][a-z0-9._-]{0,63}", appliance_id)
            or not isinstance(device, str)
            or DEVICE_PATTERN.fullmatch(device) is None
            or device.lower() in {"off", "none"}
            or type(sample_rate) is not int
            or not 8_000 <= sample_rate <= 192_000
            or type(channels) is not int
            or not 1 <= channels <= 8
            or type(expected_peer_uid) is not int
            or not 1 <= expected_peer_uid <= 2**32 - 1
            or not isinstance(library, str)
            or not library.startswith("/")
            or library.startswith("//")
            or "\x00" in library
            or library != os.path.normpath(library)
            or not isinstance(digest, str)
            or HEX64.fullmatch(digest) is None
        ):
            raise FeederError("audio feeder configuration escaped immutable bounds")
        return cls(
            appliance_id=appliance_id,
            device=device,
            sample_rate=sample_rate,
            channels=channels,
            expected_peer_uid=expected_peer_uid,
            libasound_path=Path(library),
            libasound_sha256=digest,
        )


def _validate_root_path(path: Path, *, mode: int, maximum: int) -> os.stat_result:
    if not path.is_absolute():
        raise FeederError("trusted path must be absolute")
    cursor = Path("/")
    for component in path.parts[1:]:
        cursor /= component
        metadata = cursor.lstat()
        if stat.S_ISLNK(metadata.st_mode):
            raise FeederError("trusted path contains a symlink")
        if cursor != path and (
            not stat.S_ISDIR(metadata.st_mode)
            or metadata.st_uid != 0
            or stat.S_IMODE(metadata.st_mode) & 0o022
        ):
            raise FeederError("trusted path ancestors are not immutable")
    metadata = path.lstat()
    if (
        not stat.S_ISREG(metadata.st_mode)
        or metadata.st_uid != 0
        or metadata.st_nlink != 1
        or stat.S_IMODE(metadata.st_mode) != mode
        or metadata.st_size > maximum
    ):
        raise FeederError("trusted file ownership, mode, link count, or size is invalid")
    return metadata


def _stable_read(path: Path, *, mode: int, maximum: int) -> bytes:
    before = _validate_root_path(path, mode=mode, maximum=maximum)
    descriptor = os.open(path, os.O_RDONLY | os.O_CLOEXEC | os.O_NOFOLLOW)
    try:
        opened = os.fstat(descriptor)
        if (opened.st_dev, opened.st_ino) != (before.st_dev, before.st_ino):
            raise FeederError("trusted file identity changed while opening")
        chunks: list[bytes] = []
        remaining = maximum + 1
        while remaining:
            chunk = os.read(descriptor, min(remaining, 64 * 1024))
            if not chunk:
                break
            chunks.append(chunk)
            remaining -= len(chunk)
        payload = b"".join(chunks)
    finally:
        os.close(descriptor)
    after = _validate_root_path(path, mode=mode, maximum=maximum)
    identity = lambda value: (
        value.st_dev,
        value.st_ino,
        value.st_size,
        value.st_mtime_ns,
        value.st_ctime_ns,
    )
    if identity(before) != identity(after) or len(payload) > maximum:
        raise FeederError("trusted file changed while reading")
    return payload


def load_config(path: Path) -> Config:
    payload = _stable_read(path, mode=0o440, maximum=MAX_CONFIG_BYTES)
    canonical_payload = payload[:-1] if payload.endswith(b"\n") else payload
    if b"\n" in canonical_payload:
        raise FeederError("audio feeder configuration is not one canonical JSON line")
    try:
        value = json.loads(canonical_payload)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise FeederError("audio feeder configuration is not canonical JSON") from error
    if (
        json.dumps(value, sort_keys=True, separators=(",", ":")).encode("ascii")
        != canonical_payload
    ):
        raise FeederError("audio feeder configuration is not canonical JSON")
    config = Config.parse(value)
    library = _stable_read(config.libasound_path, mode=0o644, maximum=MAX_LIBRARY_BYTES)
    if hashlib.sha256(library).hexdigest() != config.libasound_sha256:
        raise FeederError("libasound digest differs from immutable configuration")
    return config


class AlsaCapture:
    """A fixed-parameter direct libasound capture handle."""

    def __init__(self, config: Config) -> None:
        self._api = ctypes.CDLL(str(config.libasound_path), use_errno=True)
        self._configure_api()
        self._handle = ctypes.c_void_p()
        result = self._api.snd_pcm_open(
            ctypes.byref(self._handle),
            config.device.encode("ascii"),
            SND_PCM_STREAM_CAPTURE,
            0,
        )
        self._check(result, "snd_pcm_open")
        try:
            result = self._api.snd_pcm_set_params(
                self._handle,
                SND_PCM_FORMAT_S16_LE,
                SND_PCM_ACCESS_RW_INTERLEAVED,
                config.channels,
                config.sample_rate,
                1,
                100_000,
            )
            self._check(result, "snd_pcm_set_params")
        except BaseException:
            self.close()
            raise
        self.channels = config.channels
        self.frames = max(1, config.sample_rate // 10)
        self._buffer = (ctypes.c_int16 * (self.frames * self.channels))()

    def _configure_api(self) -> None:
        self._api.snd_pcm_open.argtypes = [
            ctypes.POINTER(ctypes.c_void_p),
            ctypes.c_char_p,
            ctypes.c_int,
            ctypes.c_int,
        ]
        self._api.snd_pcm_open.restype = ctypes.c_int
        self._api.snd_pcm_set_params.argtypes = [
            ctypes.c_void_p,
            ctypes.c_int,
            ctypes.c_int,
            ctypes.c_uint,
            ctypes.c_uint,
            ctypes.c_int,
            ctypes.c_uint,
        ]
        self._api.snd_pcm_set_params.restype = ctypes.c_int
        self._api.snd_pcm_readi.argtypes = [
            ctypes.c_void_p,
            ctypes.c_void_p,
            ctypes.c_ulong,
        ]
        self._api.snd_pcm_readi.restype = ctypes.c_long
        self._api.snd_pcm_recover.argtypes = [ctypes.c_void_p, ctypes.c_int, ctypes.c_int]
        self._api.snd_pcm_recover.restype = ctypes.c_int
        self._api.snd_pcm_close.argtypes = [ctypes.c_void_p]
        self._api.snd_pcm_close.restype = ctypes.c_int
        self._api.snd_strerror.argtypes = [ctypes.c_int]
        self._api.snd_strerror.restype = ctypes.c_char_p

    def _check(self, result: int, operation: str) -> None:
        if result < 0:
            detail = self._api.snd_strerror(result)
            message = detail.decode("utf-8", "replace") if detail else "unknown ALSA error"
            raise FeederError(f"{operation} failed: {message}")

    def read_mono(self) -> list[float]:
        while True:
            frames = int(self._api.snd_pcm_readi(self._handle, self._buffer, self.frames))
            if frames >= 0:
                break
            recovered = int(self._api.snd_pcm_recover(self._handle, frames, 1))
            self._check(recovered, "snd_pcm_recover")
        mono: list[float] = []
        for frame in range(frames):
            start = frame * self.channels
            total = sum(int(self._buffer[start + channel]) for channel in range(self.channels))
            mono.append(total / self.channels / 32_768.0)
        return mono

    def close(self) -> None:
        if self._handle:
            self._api.snd_pcm_close(self._handle)
            self._handle = ctypes.c_void_p()

    def __enter__(self) -> "AlsaCapture":
        return self

    def __exit__(self, *_: object) -> None:
        self.close()


def _goertzel(samples: list[float], sample_rate: int, frequency: float) -> float:
    if not samples or frequency >= sample_rate / 2:
        return 0.0
    coefficient = 2.0 * math.cos(2.0 * math.pi * frequency / sample_rate)
    first = 0.0
    second = 0.0
    for sample in samples:
        current = sample + coefficient * first - second
        second = first
        first = current
    power = max(0.0, first * first + second * second - coefficient * first * second)
    return math.sqrt(power) / len(samples)


def extract_features(samples: list[float], sample_rate: int) -> list[float]:
    if not samples:
        return [0.0] * 8
    count = len(samples)
    rms = math.sqrt(sum(sample * sample for sample in samples) / count)
    normalized_rms = min(1.0, max(0.0, (max(-6.0, math.log(rms + 1.0e-10)) + 6.0) / 6.0))
    crossings = sum(
        (left >= 0.0) != (right >= 0.0)
        for left, right in zip(samples, samples[1:])
    )
    zcr = crossings / max(1, count - 1)
    bands = [_goertzel(samples, sample_rate, frequency) for frequency in (250, 1_000, 3_000, 6_000)]
    band_total = sum(bands) + 1.0e-12
    band_ratios = [min(1.0, max(0.0, value / band_total)) for value in bands]
    peak = min(1.0, max(abs(sample) for sample in samples))
    mean = min(1.0, max(-1.0, sum(samples) / count))
    return [normalized_rms, min(1.0, zcr), *band_ratios, peak, mean]


def frame_bytes(config: Config, sequence: int, features: Iterable[float]) -> bytes:
    values = list(features)
    if (
        sequence < 1
        or len(values) != 8
        or any(not math.isfinite(value) or not -1.0 <= value <= 1.0 for value in values)
    ):
        raise FeederError("numeric feature frame escaped immutable bounds")
    value = {
        "channels": config.channels,
        "device": config.device,
        "features": values,
        "sample_rate": config.sample_rate,
        "schema": FRAME_SCHEMA,
        "sequence": sequence,
        "source": FRAME_SOURCE,
    }
    payload = json.dumps(value, sort_keys=True, separators=(",", ":"), allow_nan=False).encode(
        "ascii"
    ) + b"\n"
    if len(payload) > MAX_FRAME_BYTES:
        raise FeederError("numeric feature frame exceeded immutable size bound")
    return payload


def peer_uid(connection: socket.socket) -> int:
    credentials = connection.getsockopt(socket.SOL_SOCKET, SO_PEERCRED, 12)
    pid, uid, _gid = struct.unpack("3i", credentials)
    if pid <= 0 or uid <= 0:
        raise FeederError("audio consumer omitted valid Unix peer credentials")
    return uid


def serve(config: Config, listener: socket.socket) -> None:
    if listener.family != socket.AF_UNIX or listener.getsockopt(socket.SOL_SOCKET, socket.SO_TYPE) != socket.SOCK_STREAM:
        raise FeederError("audio feeder requires one systemd-activated AF_UNIX stream listener")
    sequence = 0
    with AlsaCapture(config) as capture:
        while True:
            connection, _ = listener.accept()
            with connection:
                if peer_uid(connection) != config.expected_peer_uid:
                    continue
                while True:
                    samples = capture.read_mono()
                    sequence += 1
                    try:
                        connection.sendall(frame_bytes(config, sequence, extract_features(samples, config.sample_rate)))
                    except (BrokenPipeError, ConnectionResetError):
                        break


def parse_args(arguments: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--config", type=Path, required=True)
    return parser.parse_args(arguments)


def main(arguments: list[str] | None = None) -> int:
    options = parse_args(sys.argv[1:] if arguments is None else arguments)
    config = load_config(options.config)
    listener = socket.fromfd(0, socket.AF_UNIX, socket.SOCK_STREAM)
    serve(config, listener)
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (FeederError, OSError) as error:
        print(f"astrid-edge-audio-feeder: {error}", file=sys.stderr)
        raise SystemExit(1) from error
