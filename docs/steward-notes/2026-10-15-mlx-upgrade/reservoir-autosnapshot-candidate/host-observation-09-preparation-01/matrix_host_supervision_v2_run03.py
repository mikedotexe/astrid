"""Version2 finite evidence: consecutive swap samples must never increase.

The original supervisor remains preserved. This revision uses the exact same
reviewed sampler closure and fresh output/stop identities; it controls no case.
"""
from __future__ import annotations
import ast
import hashlib
import json
import math
import os
from pathlib import Path
import tempfile
import time
import stat
from datetime import datetime, timezone

ROOT = Path('/Users/v/other/astrid/.runtime/mlx-upgrade/reservoir-snapshot-candidate')
SOURCE = ROOT / 'host-settle-07-observer.py'
SOURCE_SHA = '005c5db21d9ff36dd2e867cb54a272cabdc2c1922f0a8bb0f0b5a7ddd5709d05'
OUTPUT = ROOT / 'matrix-host-supervision-03.json'
STOP = ROOT / 'matrix-host-supervision-03-stop.json'
TOKEN = 'mlx-matrix-20261016-528-supervision-03'
MAX_SECONDS = 3600


def read_functions():
    raw = SOURCE.read_bytes()
    if hashlib.sha256(raw).hexdigest() != SOURCE_SHA:
        raise RuntimeError('reviewed observation source changed')
    tree = ast.parse(raw)
    names = {'run', 'process_rows', 'identities', 'host_sample'}
    nodes = [n for n in tree.body if isinstance(n, (ast.Import, ast.ImportFrom))
             or isinstance(n, ast.FunctionDef) and n.name in names]
    if {n.name for n in nodes if isinstance(n, ast.FunctionDef)} != names:
        raise RuntimeError('observation function closure differs')
    namespace = {}
    exec(compile(ast.Module(body=nodes, type_ignores=[]), str(SOURCE), 'exec'), namespace)
    return namespace


def violations(row, *, initial_swap, previous_t, previous_swap):
    errors = []
    if type(initial_swap) not in (int, float) or not math.isfinite(initial_swap) or initial_swap < 0:
        errors.append('invalid_initial_swap')
    if row.get('pressure_level') != 1: errors.append('pressure_not_normal')
    if row.get('build_test_processes') != []: errors.append('build_test_activity_or_missing_observation')
    if row.get('production_identity_unchanged') is not True: errors.append('identity_changed_or_unobserved')
    for key in ('memory_error', 'process_error'):
        if key in row: errors.append(key)
    swap = row.get('swap_bytes')
    if type(swap) not in (int, float) or not math.isfinite(swap) or swap < 0 or swap > initial_swap:
        errors.append('swap_above_start_or_unobserved')
    if previous_t is not None:
        if type(previous_swap) not in (int, float) or not math.isfinite(previous_swap) or previous_swap < 0:
            errors.append('previous_swap_unobserved')
        elif type(swap) in (int, float) and math.isfinite(swap) and swap > previous_swap:
            errors.append('swap_increased_since_previous_sample')
    ready = row.get('readyz', {})
    body = ready.get('body', {})
    if ready.get('http_status') != 200 or body.get('ready') is not True or body.get('reservoir', {}).get('status') != 'connected':
        errors.append('provider_or_reservoir_not_ready')
    stamp = row.get('t_s')
    if type(stamp) not in (int, float) or not math.isfinite(stamp) or previous_t is not None and not 0 < stamp - previous_t <= 15:
        errors.append('observation_gap_or_invalid_time')
    return errors


def read_stop():
    before = STOP.lstat()
    if not stat.S_ISREG(before.st_mode) or before.st_uid != os.getuid() or before.st_nlink != 1 or before.st_mode & 0o022 or before.st_size > 512:
        raise RuntimeError('foreign or unsafe stop file')
    fd = os.open(STOP, os.O_RDONLY | os.O_NOFOLLOW)
    try:
        opened = os.fstat(fd)
        if (opened.st_dev, opened.st_ino) != (before.st_dev, before.st_ino):
            raise RuntimeError('stop file changed during open')
        raw = os.read(fd, 513)
        after = os.fstat(fd)
        if len(raw) > 512 or (after.st_size, after.st_mtime_ns) != (before.st_size, before.st_mtime_ns):
            raise RuntimeError('stop file changed during read')
    finally:
        os.close(fd)
    if json.loads(raw) != {'schema': 'reservoir_matrix_host_stop_v1', 'token': TOKEN, 'operation': 'finish'}:
        raise RuntimeError('foreign or malformed stop request')


def observe(functions, record):
    row = functions['host_sample']()
    try:
        row['production_identity_unchanged'] = functions['identities']() == record['identity_start']
    except Exception as error:
        row['production_identity_unchanged'] = False; row['identity_error'] = str(error)
    rows = record['samples']
    if not rows: record['initial_swap_bytes'] = row.get('swap_bytes', -1)
    row['violations'] = violations(row, initial_swap=record['initial_swap_bytes'],
            previous_t=rows[-1].get('t_s') if rows else None,
            previous_swap=rows[-1].get('swap_bytes') if rows else None)
    if row['violations']:
        record['invalidated'] = True
        record.setdefault('first_invalid_sample', len(rows))
    rows.append(row)


def covers_case(record, start, end):
    """Root acceptance must bind this interval to the actual retained case run."""
    rows = record.get('samples', [])
    if record.get('invalidated') is not False or record.get('state') not in ('running', 'completed') or len(rows) < 2:
        return False
    if not all(type(v) in (int, float) and math.isfinite(v) for v in (start, end)) or start >= end:
        return False
    if rows[0].get('t_s', float('inf')) > start or rows[-1].get('t_s', float('-inf')) < end:
        return False
    previous = previous_swap = None
    for row in rows:
        if violations(row, initial_swap=record['initial_swap_bytes'], previous_t=previous,
                      previous_swap=previous_swap): return False
        previous = row['t_s']; previous_swap = row.get('swap_bytes')
    return True


def ready_for_next_case(record, now):
    rows = record.get('samples', [])
    return (record.get('state') == 'running' and record.get('invalidated') is False
            and len(rows) >= 2 and 0 <= now - rows[-1]['t_s'] <= 15
            and covers_case(record, rows[0]['t_s'], rows[-1]['t_s']))


def publish(record):
    fd, name = tempfile.mkstemp(prefix='.matrix-host-', dir=ROOT)
    try:
        with os.fdopen(fd, 'w') as stream:
            json.dump(record, stream, indent=2, sort_keys=True)
            stream.write('\n'); stream.flush(); os.fsync(stream.fileno())
        os.replace(name, OUTPUT)
    finally:
        if os.path.exists(name): os.unlink(name)


def main():
    if os.path.lexists(STOP): raise RuntimeError('existing stop request; do not reuse this run')
    fd = os.open(OUTPUT, os.O_CREAT | os.O_EXCL | os.O_WRONLY, 0o600)
    os.close(fd)
    record = {'schema': 'reservoir_matrix_host_supervision_v2', 'state': 'starting',
              'token': TOKEN, 'observer_pid': os.getpid(), 'samples': [], 'invalidated': False,
              'started_at': datetime.now(timezone.utc).isoformat(), 'maximum_seconds': MAX_SECONDS,
              'source_sha256': SOURCE_SHA, 'self_sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
              'production_mutations': 0, 'qualification_cases_started': 0,
              'swap_policy': 'nonincrease_between_consecutive_samples_and_not_above_initial',
              'purpose': 'Evidence only. Any invalidation blocks further root case release; no signal, retry or child control.'}
    started = time.monotonic()
    try:
        functions = read_functions()
        record['identity_start'] = functions['identities']()
        record['state'] = 'running'; publish(record)
        next_due = started
        while True:
            if os.path.lexists(STOP):
                read_stop()
                if not record['samples']: raise RuntimeError('finish before any health observation')
                observe(functions, record)
                record['completion_reason'] = 'owned_finish_request'; break
            if time.monotonic() - started >= MAX_SECONDS:
                record['invalidated'] = True; record['completion_reason'] = 'finite_duration_exhausted'; break
            time.sleep(max(0, next_due - time.monotonic()))
            observe(functions, record); publish(record)
            next_due += 5
        record['identity_end'] = functions['identities']()
        if record['identity_end'] != record['identity_start']: record['invalidated'] = True
        record['state'] = 'completed'
    except BaseException as error:
        record['state'] = 'failed'; record['invalidated'] = True
        record['error'] = {'type': type(error).__name__, 'message': str(error)}
        raise
    finally:
        record['duration_s'] = time.monotonic() - started
        record['updated_at'] = datetime.now(timezone.utc).isoformat()
        publish(record)
    return 1 if record['invalidated'] else 0


if __name__ == '__main__':
    raise SystemExit(main())
