"""Credential-confined interactive stewardship lifecycle."""

from __future__ import annotations

import json
import os
import selectors
import signal
import sys
import time
from types import FrameType
from typing import Any, TextIO

from .controller import StewardController
from .errors import ProjectionCancelledError


SESSION_READY_SCHEMA = "steward_session_ready_v1"
SESSION_RESPONSE_SCHEMA = "steward_session_response_v1"
SESSION_EVENT_SCHEMA = "steward_session_event_v1"
MAX_REQUEST_BYTES = 64 * 1024
READ_CHUNK_BYTES = 16 * 1024


class _SessionInterrupted(BaseException):
    def __init__(self, signum: int):
        super().__init__(signum)
        self.signum = signum


def read_lease_token(stream: TextIO) -> str:
    """Read one lease token without placing it in process arguments."""

    raw = stream.readline()
    if raw == "":
        raise ValueError("lease token stdin reached EOF")
    token = raw.rstrip("\r\n")
    if not token:
        raise ValueError("lease token stdin was empty")
    return token


def _without_lease_token(value: Any) -> Any:
    if isinstance(value, dict):
        return {
            key: _without_lease_token(item)
            for key, item in value.items()
            if key != "lease_token"
        }
    if isinstance(value, list):
        return [_without_lease_token(item) for item in value]
    if isinstance(value, tuple):
        return [_without_lease_token(item) for item in value]
    return value


def _contains_secret(value: Any, secret: str) -> bool:
    if isinstance(value, str):
        return secret in value
    if isinstance(value, dict):
        return any(
            _contains_secret(key, secret) or _contains_secret(item, secret)
            for key, item in value.items()
        )
    if isinstance(value, (list, tuple)):
        return any(_contains_secret(item, secret) for item in value)
    return False


def _public_result(value: Any, secret: str) -> Any:
    sanitized = _without_lease_token(value)
    if _contains_secret(sanitized, secret):
        raise RuntimeError("controller response crossed the credential boundary")
    return sanitized


def _request_id(request: dict[str, Any]) -> str | int | None:
    value = request.get("request_id")
    if isinstance(value, bool) or (
        value is not None and not isinstance(value, (str, int))
    ):
        raise ValueError("request_id must be a string or integer")
    return value


def _unexpected_fields(
    request: dict[str, Any],
    allowed: set[str],
) -> list[str]:
    return sorted(str(key) for key in request if key not in allowed)


class CredentialSafeSession:
    """Own a steward lease without returning its credential to the client."""

    def __init__(self, controller: StewardController, *, actor: str):
        if not actor.strip():
            raise ValueError("session actor must be non-empty")
        self.controller = controller
        self.actor = actor
        self.run_id: str | None = None
        self._lease_token: str | None = None
        self.finished = False

    def begin(self) -> dict[str, Any]:
        if self.run_id is not None:
            raise RuntimeError("session already began")
        begun = self.controller.begin(
            actor=self.actor,
            adapter_kind="session",
            pid=os.getpid(),
        )
        self.run_id = str(begun["run_id"])
        self._lease_token = str(begun.pop("lease_token"))
        return {
            "schema": SESSION_READY_SCHEMA,
            "schema_version": 1,
            "type": "ready",
            "run_id": self.run_id,
            "projection_generation_id": begun.get("projection_generation_id"),
            "expires_at_unix": begun["expires_at_unix"],
            "heartbeat_interval_secs": begun["heartbeat_interval_secs"],
            "credential_transport": "controller_process_memory",
            "lease_token_included": False,
            "automatic_heartbeat": True,
            "max_request_bytes": MAX_REQUEST_BYTES,
        }

    def _ownership(self) -> tuple[str, str]:
        if self.run_id is None or self._lease_token is None or self.finished:
            raise RuntimeError("session does not own an active lease")
        return self.run_id, self._lease_token

    def heartbeat(self) -> dict[str, Any]:
        run_id, token = self._ownership()
        result = self.controller.heartbeat(run_id=run_id, lease_token=token)
        return _public_result(result, token)

    def project(
        self,
        *,
        full_rebuild: bool = False,
        resume_generation: str | None = None,
    ) -> dict[str, Any]:
        run_id, token = self._ownership()
        result = self.controller.project(
            run_id=run_id,
            lease_token=token,
            actor=self.actor,
            phase="manual",
            full_rebuild=full_rebuild,
            resume_generation=resume_generation,
        )
        return _public_result(result, token)

    def finish(
        self,
        *,
        outcome: str,
        exit_code: int | None = None,
        summary_ref: str | None = None,
    ) -> dict[str, Any]:
        run_id, token = self._ownership()
        result = self.controller.finish(
            run_id=run_id,
            lease_token=token,
            outcome=outcome,
            exit_code=exit_code,
            summary_ref=summary_ref,
        )
        try:
            return _public_result(result, token)
        finally:
            self._lease_token = None
            self.finished = True

    def dispatch(
        self,
        request: Any,
    ) -> tuple[dict[str, Any], bool]:
        request_id: str | int | None = None
        op = "invalid"
        try:
            if not isinstance(request, dict):
                raise ValueError("session request must be a JSON object")
            request_id = _request_id(request)
            op_value = request.get("op")
            if not isinstance(op_value, str) or not op_value:
                raise ValueError("session request requires a non-empty op")
            op = op_value
            if any(
                "lease_token" in str(key).lower()
                or "credential" in str(key).lower()
                for key in request
            ):
                raise ValueError("session requests must not carry lease credentials")

            allowed = {
                "heartbeat": {"op", "request_id"},
                "status": {"op", "request_id"},
                "project": {
                    "op",
                    "request_id",
                    "full_rebuild",
                    "resume_generation",
                },
                "finish": {
                    "op",
                    "request_id",
                    "outcome",
                    "exit_code",
                    "summary_ref",
                },
            }.get(op)
            if allowed is None:
                raise ValueError(f"unsupported session op: {op}")
            unexpected = _unexpected_fields(request, allowed)
            if unexpected:
                raise ValueError(
                    "unexpected session request fields: " + ", ".join(unexpected)
                )

            heartbeat = self.heartbeat()
            if heartbeat["stop_requested"]:
                result = self.finish(
                    outcome="cancelled",
                    summary_ref="session_pause_requested",
                )
                return self._response(
                    request_id=request_id,
                    op=op,
                    result=result,
                    terminal_reason="pause_requested",
                ), True

            if op == "heartbeat":
                result = heartbeat
            elif op == "status":
                result = {
                    "run_id": self.run_id,
                    "active": not self.finished,
                    "heartbeat": heartbeat,
                    "lease_token_included": False,
                }
            elif op == "project":
                full_rebuild = request.get("full_rebuild", False)
                if not isinstance(full_rebuild, bool):
                    raise ValueError("full_rebuild must be boolean")
                resume_generation = request.get("resume_generation")
                if resume_generation is not None and not isinstance(
                    resume_generation, str
                ):
                    raise ValueError("resume_generation must be a string")
                result = self.project(
                    full_rebuild=full_rebuild,
                    resume_generation=resume_generation,
                )
                after = self.heartbeat()
                if after["stop_requested"]:
                    result = self.finish(
                        outcome="cancelled",
                        summary_ref="session_pause_after_projection",
                    )
                    return self._response(
                        request_id=request_id,
                        op=op,
                        result=result,
                        terminal_reason="pause_requested",
                    ), True
            else:
                outcome = request.get("outcome")
                if outcome not in {
                    "success",
                    "failed",
                    "cancelled",
                    "policy_violation",
                }:
                    raise ValueError("finish requires a supported outcome")
                exit_code = request.get("exit_code")
                if isinstance(exit_code, bool) or (
                    exit_code is not None and not isinstance(exit_code, int)
                ):
                    raise ValueError("exit_code must be an integer")
                summary_ref = request.get("summary_ref")
                if summary_ref is not None and not isinstance(summary_ref, str):
                    raise ValueError("summary_ref must be a string")
                result = self.finish(
                    outcome=outcome,
                    exit_code=exit_code,
                    summary_ref=summary_ref,
                )
                return self._response(
                    request_id=request_id,
                    op=op,
                    result=result,
                    terminal_reason="explicit_finish",
                ), True
            return self._response(
                request_id=request_id,
                op=op,
                result=result,
            ), False
        except ValueError as error:
            return _protocol_error(
                request_id=request_id,
                op=op,
                error=error,
            ), False

    def _response(
        self,
        *,
        request_id: str | int | None,
        op: str,
        result: dict[str, Any],
        terminal_reason: str | None = None,
    ) -> dict[str, Any]:
        return {
            "schema": SESSION_RESPONSE_SCHEMA,
            "schema_version": 1,
            "type": "response",
            "request_id": request_id,
            "op": op,
            "ok": True,
            "terminal": terminal_reason is not None,
            "terminal_reason": terminal_reason,
            "result": _without_lease_token(result),
            "lease_token_included": False,
        }


def _protocol_error(
    *,
    request_id: str | int | None,
    op: str,
    error: BaseException,
) -> dict[str, Any]:
    return {
        "schema": SESSION_RESPONSE_SCHEMA,
        "schema_version": 1,
        "type": "response",
        "request_id": request_id,
        "op": op,
        "ok": False,
        "terminal": False,
        "error": type(error).__name__,
        "message": str(error),
        "lease_token_included": False,
    }


def _emit_line(stream: TextIO, value: dict[str, Any]) -> None:
    stream.write(json.dumps(value, sort_keys=True, ensure_ascii=False) + "\n")
    stream.flush()


def _terminal_event(
    *,
    reason: str,
    result: dict[str, Any],
) -> dict[str, Any]:
    return {
        "schema": SESSION_EVENT_SCHEMA,
        "schema_version": 1,
        "type": "terminal",
        "reason": reason,
        "result": _without_lease_token(result),
        "lease_token_included": False,
    }


def _heartbeat_event(result: dict[str, Any]) -> dict[str, Any]:
    return {
        "schema": SESSION_EVENT_SCHEMA,
        "schema_version": 1,
        "type": "heartbeat",
        "result": result,
        "lease_token_included": False,
    }


def _exit_code_for(result: dict[str, Any]) -> int:
    outcome = (result.get("receipt") or {}).get("outcome")
    if outcome == "success":
        return 0
    if outcome == "cancelled":
        return 3
    return 5


def _install_signal_handlers() -> dict[int, Any]:
    previous: dict[int, Any] = {}

    def interrupt(signum: int, _frame: FrameType | None) -> None:
        raise _SessionInterrupted(signum)

    for name in ("SIGINT", "SIGTERM", "SIGHUP"):
        signum = getattr(signal, name, None)
        if signum is None:
            continue
        try:
            previous[signum] = signal.getsignal(signum)
            signal.signal(signum, interrupt)
        except (OSError, ValueError):
            previous.pop(signum, None)
    return previous


def _restore_signal_handlers(previous: dict[int, Any]) -> None:
    for signum, handler in previous.items():
        try:
            signal.signal(signum, handler)
        except (OSError, ValueError):
            pass


def _signal_name(signum: int) -> str:
    try:
        return signal.Signals(signum).name.lower()
    except ValueError:
        return f"signal_{signum}"


def _finish_terminal(
    session: CredentialSafeSession,
    destination: TextIO,
    *,
    reason: str,
    exit_code: int | None = None,
) -> dict[str, Any]:
    result = session.finish(
        outcome="cancelled",
        exit_code=exit_code,
        summary_ref=f"session_{reason}",
    )
    _emit_line(destination, _terminal_event(reason=reason, result=result))
    return result


def _decode_request(raw_line: bytes) -> Any:
    try:
        text = raw_line.removesuffix(b"\r").decode("utf-8")
    except UnicodeDecodeError as error:
        raise ValueError("session request was not valid UTF-8") from error
    try:
        return json.loads(text)
    except json.JSONDecodeError as error:
        raise ValueError("session request was not valid JSON") from error


def run_session(
    controller: StewardController,
    *,
    actor: str,
    max_secs: int | None = None,
    input_stream: TextIO | None = None,
    output_stream: TextIO | None = None,
) -> int:
    """Serve bounded NDJSON requests while retaining the token in memory."""

    limit = controller.config.max_run_secs if max_secs is None else max_secs
    if isinstance(limit, bool) or limit <= 0:
        raise ValueError("session max-secs must be a positive integer")

    source = sys.stdin if input_stream is None else input_stream
    destination = sys.stdout if output_stream is None else output_stream
    session = CredentialSafeSession(controller, actor=actor)
    ready = session.begin()
    selector: selectors.BaseSelector | None = None
    previous_handlers = _install_signal_handlers()
    try:
        _emit_line(destination, ready)
        started = time.monotonic()
        heartbeat_period = max(
            0.1,
            float(controller.config.heartbeat_interval_secs) / 2.0,
        )
        next_heartbeat = started + heartbeat_period
        input_fd = source.fileno()
        selector = selectors.DefaultSelector()
        selector.register(input_fd, selectors.EVENT_READ)
        pending = bytearray()
        discarding_oversized = False

        while not session.finished:
            now = time.monotonic()
            if now - started >= limit:
                _finish_terminal(session, destination, reason="timeout")
                return 124

            if now >= next_heartbeat:
                heartbeat = session.heartbeat()
                _emit_line(destination, _heartbeat_event(heartbeat))
                if heartbeat["stop_requested"]:
                    _finish_terminal(
                        session,
                        destination,
                        reason="pause_requested",
                    )
                    return 3
                next_heartbeat = time.monotonic() + heartbeat_period
                continue

            timeout = max(
                0.0,
                min(next_heartbeat - now, limit - (now - started)),
            )
            events = selector.select(timeout)
            if not events:
                continue

            chunk = os.read(input_fd, READ_CHUNK_BYTES)
            if not chunk:
                reason = (
                    "eof_with_partial_request"
                    if pending or discarding_oversized
                    else "eof"
                )
                _finish_terminal(session, destination, reason=reason)
                return 3
            pending.extend(chunk)

            while pending:
                if discarding_oversized:
                    newline = pending.find(b"\n")
                    if newline < 0:
                        pending.clear()
                        break
                    del pending[: newline + 1]
                    discarding_oversized = False
                    continue

                newline = pending.find(b"\n")
                if newline < 0:
                    if len(pending) > MAX_REQUEST_BYTES:
                        _emit_line(
                            destination,
                            _protocol_error(
                                request_id=None,
                                op="invalid",
                                error=ValueError("session request exceeded size limit"),
                            ),
                        )
                        pending.clear()
                        discarding_oversized = True
                    break

                raw_line = bytes(pending[:newline])
                del pending[: newline + 1]
                if len(raw_line) > MAX_REQUEST_BYTES:
                    _emit_line(
                        destination,
                        _protocol_error(
                            request_id=None,
                            op="invalid",
                            error=ValueError("session request exceeded size limit"),
                        ),
                    )
                    continue
                try:
                    request = _decode_request(raw_line)
                except ValueError as error:
                    _emit_line(
                        destination,
                        _protocol_error(
                            request_id=None,
                            op="invalid",
                            error=error,
                        ),
                    )
                    continue

                response, terminal = session.dispatch(request)
                _emit_line(destination, response)
                next_heartbeat = time.monotonic() + heartbeat_period
                if terminal:
                    return _exit_code_for(response["result"])
    except _SessionInterrupted as interruption:
        exit_code = 128 + interruption.signum
        if interruption.signum == getattr(signal, "SIGINT", None):
            exit_code = 130
        if not session.finished:
            _finish_terminal(
                session,
                destination,
                reason=_signal_name(interruption.signum),
                exit_code=exit_code,
            )
        return exit_code
    except KeyboardInterrupt:
        if not session.finished:
            _finish_terminal(
                session,
                destination,
                reason="sigint",
                exit_code=130,
            )
        return 130
    except BaseException as error:
        if not session.finished:
            try:
                session.finish(
                    outcome=(
                        "cancelled"
                        if isinstance(error, ProjectionCancelledError)
                        else "failed"
                    ),
                    summary_ref="session_exception",
                )
            except Exception:
                pass
        raise
    finally:
        if selector is not None:
            selector.close()
        _restore_signal_handlers(previous_handlers)

    return 0
