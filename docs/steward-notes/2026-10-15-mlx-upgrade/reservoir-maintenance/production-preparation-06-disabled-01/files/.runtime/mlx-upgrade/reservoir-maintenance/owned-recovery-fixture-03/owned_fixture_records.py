"""Small durable fixture records. No OS control, imports, or work on import."""
from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path
import re
import stat
import time

MAX_BYTES = 4 * 1024 * 1024
PREFIX = 'com.codex.reservoir-maintenance-fixture.'
ROLES = ('provider', 'reservoir')


def require(value, message):
    if not value:
        raise ValueError(message)


def digest(value):
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(',', ':'),
                                     allow_nan=False).encode()).hexdigest()


def identity(value):
    require(isinstance(value, dict) and set(value) == {'pid', 'started_at', 'argv'}
            and type(value['pid']) is int and value['pid'] > 1
            and isinstance(value['started_at'], str) and value['started_at']
            and isinstance(value['argv'], list) and value['argv']
            and all(isinstance(x, str) and x for x in value['argv']), 'exact identity required')
    return value


def target(uid, token, role):
    require(type(uid) is int and uid > 0 and re.fullmatch('[a-f0-9]{32}', token)
            and role in (*ROLES, 'pin'), 'invalid private target')
    return f'gui/{uid}/{PREFIX}{token}.{role}'


def directory(path):
    path = Path(path)
    require(path.is_absolute() and path == path.resolve(strict=True), 'noncanonical private directory')
    info = path.lstat()
    require(stat.S_ISDIR(info.st_mode) and info.st_uid == os.getuid()
            and not stat.S_IMODE(info.st_mode) & 0o077, 'unowned or public private directory')
    return path


def stamp(info):
    return (info.st_dev, info.st_ino, info.st_size, info.st_mtime_ns, info.st_mode, info.st_uid, info.st_nlink)


def raw(path, limit=MAX_BYTES):
    path = Path(path)
    require(path.is_absolute() and path == path.resolve(strict=True), 'noncanonical record')
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
    try:
        before = os.fstat(fd)
        require(stat.S_ISREG(before.st_mode) and before.st_uid == os.getuid()
                and before.st_nlink == 1 and before.st_size <= limit, 'unsafe or oversized record')
        chunks = []; size = 0
        while chunk := os.read(fd, min(65536, limit + 1 - size)):
            size += len(chunk); require(size <= limit, 'record grew past limit'); chunks.append(chunk)
        require(stamp(before) == stamp(os.fstat(fd)) == stamp(path.lstat()), 'record changed while reading')
        return b''.join(chunks)
    finally:
        os.close(fd)


def read(path):
    value = json.loads(raw(path), parse_constant=lambda _: require(False, 'nonfinite JSON'))
    require(isinstance(value, dict), 'object record required')
    return value


def reference(path):
    return {'path': str(Path(path)), 'sha256': hashlib.sha256(raw(path)).hexdigest()}


def check_reference(value):
    require(isinstance(value, dict) and set(value) == {'path', 'sha256'}
            and reference(value['path']) == value, 'artifact reference changed')
    return read(value['path'])


def sync(path):
    fd = os.open(path, os.O_RDONLY)
    try: os.fsync(fd)
    finally: os.close(fd)


def write(path, value):
    path = Path(path); directory(path.parent)
    data = (json.dumps(value, sort_keys=True, indent=2, allow_nan=False) + '\n').encode()
    require(len(data) <= MAX_BYTES, 'record exceeds bound')
    fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
    try:
        with os.fdopen(fd, 'wb', closefd=False) as stream:
            stream.write(data); stream.flush(); os.fsync(fd)
    finally: os.close(fd)
    sync(path.parent)
    return reference(path)


class Ledger:
    """One immutable ordered action chain, distinct from the core journal.

    Callers supply the appropriate exclusive setup/shared main lock. A failed or
    unfinished action is never executable again; only an explicit positive
    evidence reconciliation may finish it. The original claim remains intact.
    """
    def __init__(self, root, name, binding, *, create=False):
        self.root = directory(root)
        require(re.fullmatch('[a-z][a-z0-9-]{0,31}', name), 'invalid ledger name')
        self.path = self.root / name
        expected = {'schema': 'owned_fixture_ledger_v1', 'name': name, 'binding': binding}
        if create:
            self.path.mkdir(mode=0o700)
            write(self.path / 'header.json', expected)
        directory(self.path)
        require(read(self.path / 'header.json') == expected, 'ledger binding differs')
        self.header = expected
        self.rows = self.reload()

    def reload(self):
        names = sorted(self.path.glob('event-*.json'))
        rows = []; previous = digest(self.header)
        for i, path in enumerate(names, 1):
            require(path.name == f'event-{i:05d}.json', 'ledger has a gap')
            row = read(path)
            require(row.get('sequence') == i and row.get('previous') == previous
                    and row.get('type') in ('claim', 'result', 'failed', 'reconcile_claim', 'reconciled'),
                    'invalid fixture ledger chain')
            rows.append(row); previous = digest(row)
        self.validate(rows)
        self.rows = rows
        return rows

    @staticmethod
    def validate(rows):
        pending = {}; done = set(); ordinary = set(); failed = set()
        for row in rows:
            action = row.get('action'); require(isinstance(action, str) and action, 'action missing')
            if row['type'] in ('claim', 'reconcile_claim'):
                require(action not in done, 'completed action repeated')
                if row['type'] == 'claim':
                    require(action not in ordinary and action not in pending, 'action replay forbidden')
                    ordinary.add(action)
                else:
                    require(action in ordinary, 'reconciliation needs a retained action attempt')
                pending.setdefault(action, []).append(digest(row))
            else:
                require(action in pending and row.get('claim_sha256') == pending[action][-1], 'unmatched result')
                if row['type'] == 'failed':
                    require(row['claim_sha256'] not in failed, 'failure already recorded')
                    failed.add(row['claim_sha256']); continue
                if row['type'] == 'result':
                    require(len(pending[action]) == 1 and not set(pending[action]) & failed,
                            'ordinary result cannot resolve reconciliation or failure')
                else:
                    require(row.get('resolves_claims') == pending[action], 'incomplete reconciliation closure')
                    require(isinstance(row.get('proof'), dict) and row['proof'].get('passed') is True,
                            'positive reconciliation proof missing')
                require(isinstance(row.get('result'), dict), 'result object missing')
                done.add(action); pending.pop(action)

    @property
    def head(self):
        return digest(self.rows[-1]) if self.rows else digest(self.header)

    def append(self, kind, action, **values):
        expected = self.rows[:]
        require(self.reload() == expected, 'ledger changed')
        row = {'type': kind, 'action': action, 'sequence': len(self.rows) + 1,
               'previous': self.head, 'wall_time_ns': time.time_ns(), **values}
        self.validate([*self.rows, row])
        write(self.path / f'event-{row["sequence"]:05d}.json', row)
        self.rows.append(row)
        return row

    def completed(self):
        return {row['action']: row['result'] for row in self.rows if row['type'] in ('result', 'reconciled')}

    def attempted(self, action):
        return any(row['action'] == action for row in self.rows)

    def outstanding(self, action):
        return [digest(row) for row in self.rows if row['action'] == action
                and row['type'] in ('claim', 'reconcile_claim')] if action not in self.completed() else []
