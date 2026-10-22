"""Append-only records for one fixed retained maintenance transaction."""
from __future__ import annotations
import copy
from pathlib import Path
import re

import reservoir_maintenance as m
from transaction_execution import private_root, require_active

TYPES = {'action_claim', 'action_result', 'action_failed',
         'reconciliation_claim', 'reconciliation_result'}


def session_shape(value, root, plan):
    """Pure shape/binding check; read_ledger also verifies the retained file."""
    m.require(isinstance(value, dict) and value.get('schema') == 'reservoir_execution_session_v1'
              and re.fullmatch('[a-f0-9]{32}', value.get('session_id', '')) is not None
              and value.get('root') == str(root) and value.get('transaction_id') == plan['transaction_id']
              and value.get('plan_sha256') == m.digest(plan) and value.get('host_binding') == plan['host_binding']
              and value.get('mode') == 'resume_one' and type(value.get('started_wall_time_ns')) is int
              and value['started_wall_time_ns'] > 0, 'recovery executor session binding differs')
    m.identity(value.get('executor_identity'))
    return value


def retained_session(value, root, plan):
    value = session_shape(value, root, plan)
    directory = private_root(Path(root) / 'execution-sessions')
    begin = directory / (value['session_id'] + '-begin.json')
    m.require(m.read_record(begin) == value, 'recovery executor begin record differs')
    end = directory / (value['session_id'] + '-end.json')
    if end.exists() or end.is_symlink():
        row = m.read_record(end)
        m.require(row.get('schema') == 'reservoir_execution_session_end_v1'
                  and row.get('session_id') == value['session_id'] and row.get('begin_sha256') == m.digest(value)
                  and row.get('transaction_id') == plan['transaction_id']
                  and row.get('executor_identity') == value['executor_identity']
                  and type(row.get('completed_wall_time_ns')) is int
                  and row['completed_wall_time_ns'] >= value['started_wall_time_ns']
                  and row.get('external_action_success_inferred') is False, 'recovery executor end record differs')


def read_ledger(root, plan):
    """Read only. Missing ledger is an empty history, never permission to act."""
    root = Path(root)
    directory = root / 'recovery'
    if not directory.exists():
        m.require(not directory.is_symlink(), 'symlink recovery ledger')
        return {'header': None, 'records': [], 'head_sha256': m.digest(plan)}
    private_root(directory)
    header = m.read_record(directory / 'header.json')
    m.require(header.get('schema') == 'reservoir_recovery_ledger_v1'
              and header.get('transaction_id') == plan['transaction_id']
              and header.get('plan_sha256') == m.digest(plan)
              and header.get('root') == str(root), 'recovery ledger belongs to another transaction')
    records = []; previous = m.digest(header)
    paths = sorted(directory.glob('event-*.json'))
    m.require(len(paths) <= 10000, 'too many recovery records')
    for index, path in enumerate(paths, 1):
        m.require(path.name == f'event-{index:05d}.json', 'recovery sequence incomplete')
        row = m.read_record(path)
        m.require(row.get('schema') == 'reservoir_recovery_event_v1'
                  and row.get('sequence') == index and row.get('previous_sha256') == previous
                  and row.get('plan_sha256') == m.digest(plan)
                  and row.get('transaction_id') == plan['transaction_id']
                  and row.get('type') in TYPES, 'recovery ledger chain differs')
        retained_session(row.get('executor_session'), root, plan)
        records.append(row); previous = m.digest(row)
    return {'header': header, 'records': records, 'head_sha256': previous}


class RecoveryLedger:
    def __init__(self, root, plan, preparation_sha256, controller_head_sha256):
        self.root, self.plan = Path(root), copy.deepcopy(plan)
        require_active(self.root, self.plan)
        current = read_ledger(self.root, self.plan)
        expected = {'schema': 'reservoir_recovery_ledger_v1', 'root': str(self.root),
                    'transaction_id': plan['transaction_id'], 'plan_sha256': m.digest(plan),
                    'preparation_sha256': preparation_sha256,
                    'original_controller_head_sha256': controller_head_sha256}
        if current['header'] is None:
            directory = self.root / 'recovery'; directory.mkdir(mode=0o700)
            m.write_new(directory / 'header.json', expected)
            current = read_ledger(self.root, self.plan)
        m.require(current['header'] == expected, 'original recovery anchor changed')
        self.view = current

    def append(self, event_type, **details):
        session = require_active(self.root, self.plan)
        m.require(event_type in TYPES, 'unknown recovery record type')
        current = read_ledger(self.root, self.plan)
        m.require(current == self.view, 'recovery ledger changed since last read')
        row = {'schema': 'reservoir_recovery_event_v1', 'sequence': len(current['records']) + 1,
               'previous_sha256': current['head_sha256'], 'plan_sha256': m.digest(self.plan),
               'transaction_id': self.plan['transaction_id'], 'type': event_type,
               'executor_session': copy.deepcopy(session.begin), **copy.deepcopy(details)}
        m.write_new(self.root / 'recovery' / f'event-{row["sequence"]:05d}.json', row)
        self.view = read_ledger(self.root, self.plan)
        return row
