"""Candidate shutdown never escalates beyond a graceful signal."""
from unittest.mock import Mock, patch

import pytest

import astrid_model_canary as canary


def test_shutdown_waits_for_owned_candidate_to_exit():
    process = Mock(pid=12345)
    process.poll.side_effect = [None, None, 0]
    with patch.object(canary.os, "killpg") as signal_group, \
            patch.object(canary.time, "monotonic", side_effect=[10., 10., 11.]), \
            patch.object(canary.time, "sleep") as sleep:
        canary.stop_candidate_server(process, timeout_s=20.)
    signal_group.assert_called_once_with(12345, canary.signal.SIGTERM)
    sleep.assert_called_once_with(.5)


def test_shutdown_timeout_preserves_process_without_forced_signal():
    process = Mock(pid=12345)
    process.poll.return_value = None
    with patch.object(canary.os, "killpg") as signal_group, \
            patch.object(canary.time, "monotonic", side_effect=[10., 11., 31.]), \
            patch.object(canary.time, "sleep"), \
            pytest.raises(TimeoutError, match="no forced termination"):
        canary.stop_candidate_server(process, timeout_s=20.)
    signal_group.assert_called_once_with(12345, canary.signal.SIGTERM)
    process.kill.assert_not_called()


@pytest.mark.parametrize("exited", [True, False])
def test_already_exited_candidate_needs_no_further_signals(exited):
    process = Mock(pid=12345)
    process.poll.return_value = 0 if exited else None
    with patch.object(canary.os, "killpg", side_effect=ProcessLookupError) as signal_group, \
            patch.object(canary.time, "sleep") as sleep:
        canary.stop_candidate_server(process)
    assert signal_group.call_count == (0 if exited else 1)
    sleep.assert_not_called()
