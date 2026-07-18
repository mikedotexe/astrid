"""Wrapped subprocess execution under the steward lease."""

from __future__ import annotations

import os
import signal
import subprocess
import time
from typing import Any, Sequence

from .controller import StewardController


def run_subprocess(
    controller: StewardController,
    *,
    actor: str,
    argv: Sequence[str],
    max_secs: int | None = None,
) -> tuple[int, dict[str, Any]]:
    if not argv:
        raise ValueError("run requires a command after --")
    begin = controller.begin(actor=actor, adapter_kind="subprocess", pid=os.getpid())
    run_id = begin["run_id"]
    token = begin["lease_token"]
    process: subprocess.Popen[bytes] | None = None
    interrupted = False
    timed_out = False
    try:
        process = subprocess.Popen(list(argv), shell=False)
        started = time.monotonic()
        next_heartbeat = started
        limit = max_secs or controller.config.max_run_secs
        while process.poll() is None:
            now = time.monotonic()
            if now >= next_heartbeat:
                heartbeat = controller.heartbeat(run_id=run_id, lease_token=token)
                next_heartbeat = now + controller.config.heartbeat_interval_secs
                if heartbeat["stop_requested"] and not interrupted:
                    process.send_signal(signal.SIGINT)
                    interrupted = True
            if now - started >= limit and not interrupted:
                process.send_signal(signal.SIGINT)
                interrupted = True
                timed_out = True
            time.sleep(0.05)
        return_code = int(process.returncode or 0)
        outcome = (
            "cancelled"
            if interrupted
            else "success"
            if return_code == 0
            else "failed"
        )
        finished = controller.finish(
            run_id=run_id,
            lease_token=token,
            outcome=outcome,
            exit_code=return_code,
            summary_ref="watchdog_timeout" if timed_out else None,
        )
        return return_code, finished
    except BaseException:
        if process is not None and process.poll() is None and not interrupted:
            process.send_signal(signal.SIGINT)
        controller.finish(
            run_id=run_id,
            lease_token=token,
            outcome="failed",
            exit_code=None,
            summary_ref="executor_exception",
        )
        raise
