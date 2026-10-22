"""One fixed transaction lock shared by fresh execution and explicit recovery.

Import is inert. No process lookup, spawn, signal or service call exists here.
The caller supplies an exact current executor identity from its read-only Host.
"""
from __future__ import annotations

import copy
import fcntl
import os
from pathlib import Path
import stat
import time
import uuid

import reservoir_maintenance as m

_ACTIVE = {}


def private_root(root):
    root = Path(root)
    m.require(root.is_absolute() and root == root.resolve(strict=True) and root.is_dir(), 'noncanonical execution root')
    info = root.stat()
    m.require(info.st_uid == os.getuid() and not stat.S_IMODE(info.st_mode) & 0o077,
              'execution root must be private and owned')
    return root


def require_active(root, plan):
    root = Path(root)
    session = _ACTIVE.get(str(root))
    m.require(session is not None and session.active and session.pid == os.getpid()
              and session.plan == plan, 'no current shared transaction execution lock')
    info = os.fstat(session.fd)
    current = (root / 'execution.lock').lstat()
    m.require((info.st_dev, info.st_ino) == (current.st_dev, current.st_ino)
              and stat.S_ISREG(current.st_mode) and current.st_nlink == 1,
              'execution lock was replaced')
    return session


class ExecutionSession:
    """Nonblocking flock plus immutable executor records. Never deletes a lock."""
    def __init__(self, root, plan, executor_identity, mode):
        self.root = Path(root)
        self.plan = m.validate_plan(plan)
        self.executor = copy.deepcopy(m.identity(executor_identity))
        m.require(self.executor['pid'] == os.getpid(), 'executor identity is not this process')
        m.require(mode in ('fresh', 'resume_one'), 'unknown execution mode')
        self.mode, self.pid = mode, os.getpid()
        self.fd = None; self.active = False; self.begin = None

    def __enter__(self):
        root = private_root(self.root)
        m.require(str(root) not in _ACTIVE, 'transaction already executing in this process')
        fd = os.open(root / 'execution.lock', os.O_RDWR | os.O_CREAT | os.O_NOFOLLOW, 0o600)
        self.fd = fd
        try:
            info = os.fstat(fd)
            m.require(stat.S_ISREG(info.st_mode) and info.st_nlink == 1 and info.st_uid == os.getuid()
                      and not stat.S_IMODE(info.st_mode) & 0o077, 'foreign/shared execution lock')
            try:
                fcntl.flock(fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
            except BlockingIOError as error:
                raise m.Refusal('transaction already executing; no action or claim') from error
            records = root / 'execution-sessions'
            if not records.exists(): records.mkdir(mode=0o700)
            private_root(records)
            self.session_id = uuid.uuid4().hex
            self.begin = {'schema': 'reservoir_execution_session_v1', 'session_id': self.session_id,
                          'transaction_id': self.plan['transaction_id'], 'plan_sha256': m.digest(self.plan),
                          'root': str(root), 'host_binding': self.plan['host_binding'], 'mode': self.mode,
                          'executor_identity': self.executor, 'started_wall_time_ns': time.time_ns()}
            self.begin_path = records / (self.session_id + '-begin.json')
            m.write_new(self.begin_path, self.begin)
            self.active = True; _ACTIVE[str(root)] = self
            require_active(root, self.plan)
            return self
        except BaseException:
            self.active = False; _ACTIVE.pop(str(root), None)
            os.close(fd); self.fd = None
            raise

    def __exit__(self, error_type, error, traceback):
        try:
            require_active(self.root, self.plan)
            m.write_new(self.begin_path.with_name(self.session_id + '-end.json'), {
                'schema': 'reservoir_execution_session_end_v1', 'session_id': self.session_id,
                'begin_sha256': m.digest(self.begin), 'transaction_id': self.plan['transaction_id'],
                'executor_identity': self.executor, 'completed_wall_time_ns': time.time_ns(),
                'error_type': None if error_type is None else error_type.__name__,
                'external_action_success_inferred': False})
        finally:
            self.active = False; _ACTIVE.pop(str(self.root), None)
            if self.fd is not None:
                os.close(self.fd); self.fd = None
        return False


def controller_session(journal, host, mode):
    """Same entry used by fresh controller and recovery; no fake identities."""
    m.require(host.scope in ('production', 'owned_fixture'), 'execution lock requires concrete owned scope')
    m.require(journal.root.name == 'controller', 'fixed controller journal child required')
    return ExecutionSession(journal.root.parent, journal.plan, host.executor_identity(), mode)
