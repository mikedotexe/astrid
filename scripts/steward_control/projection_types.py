"""Small immutable types shared by projection profiles and the runtime."""

from __future__ import annotations

from dataclasses import dataclass


@dataclass(frozen=True)
class ProjectionCommand:
    argv: tuple[str, ...]


@dataclass(frozen=True)
class ProjectionStep:
    step_id: str
    dependencies: tuple[str, ...]
    commands: tuple[ProjectionCommand, ...]
    input_streams: tuple[str, ...]
    source_globs: tuple[str, ...]
    outputs: tuple[str, ...]


@dataclass(frozen=True)
class CommandResult:
    return_code: int
    stdout: bytes
    stderr: bytes
    duration_ms: int
    timed_out: bool = False
    cancelled: bool = False
    output_too_large: bool = False


def command(*argv: str) -> ProjectionCommand:
    return ProjectionCommand(tuple(argv))
