"""Exact durable-boundary self-exit wrapper. Never signals a worker.

Only explicitly constructed fixture executors install these wrappers. They call
the original core append first and preserve every original result/exception.
There is no default execution path or production flag mutation.
"""
from __future__ import annotations

from contextlib import contextmanager
import os
from pathlib import Path

from owned_fixture_records import check_reference, digest, identity, read, reference, require, write

EXPECTED_EXIT = 75
ALLOWED_STREAMS = ('host', 'controller', 'recovery', 'action_return', 'cleanup_return', 'pin')


def validate_manifest(value):
    require(value.get('schema') == 'owned_fixture_injections_v1'
            and value.get('scope') == 'owned_fixture_only' and value.get('production_qualified') is False,
            'fixture injection manifest required')
    points = value.get('points')
    require(isinstance(points, list) and 0 < len(points) <= 32, 'finite injection points required')
    names = set()
    for point in points:
        require(set(point) == {'id', 'stream', 'event', 'match'} and isinstance(point['id'], str)
                and point['id'] and point['id'] not in names and point['stream'] in ALLOWED_STREAMS
                and isinstance(point['event'], str) and point['event']
                and isinstance(point['match'], dict) and all(isinstance(k, str) for k in point['match']),
                'invalid or duplicate injection boundary')
        names.add(point['id'])
    return value


class Injection:
    def __init__(self, host, manifest_ref, point_id, executor_root, *, exit_self=None):
        self.host = host; self.root = Path(host.root)
        require(host.scope == 'owned_fixture', 'injection cannot target production')
        self.manifest_ref = manifest_ref
        manifest = validate_manifest(check_reference(manifest_ref))
        matches = [point for point in manifest['points'] if point['id'] == point_id]
        require(len(matches) == 1, 'unreviewed injection point')
        self.point = matches[0]; self.executor_root = Path(executor_root)
        require(self.executor_root.parent == self.root / 'executors'
                and self.executor_root == self.executor_root.resolve(strict=True), 'wrong executor evidence root')
        self.identity = identity(host.executor_identity())
        require(self.identity['pid'] == os.getpid(), 'injection does not own this executor')
        self.exit_self = os._exit if exit_self is None else exit_self
        self.hit = False

    def observe(self, stream, event_name, artifact):
        if stream != self.point['stream'] or event_name != self.point['event']: return
        path = Path(artifact['path'])
        require(path.is_relative_to(self.root) and path == path.resolve(strict=True), 'boundary outside owned fixture')
        record = check_reference(artifact)
        if any(record.get(key) != value for key, value in self.point['match'].items()): return
        require(not self.hit and self.host.executor_identity() == self.identity, 'executor identity or injection changed')
        require(check_reference(self.manifest_ref)['points'].count(self.point) == 1, 'injection manifest changed')
        from owned_recovery_core import loaded_origins
        from owned_fixture_coordinator import fixture_origins
        write(self.executor_root / 'injection-ack.json', {
            'schema': 'owned_fixture_injection_ack_v1', 'point': self.point, 'manifest': self.manifest_ref,
            'boundary': artifact, 'executor_identity': self.identity, 'expected_exit_code': EXPECTED_EXIT,
            'signals_sent_by_injection': 0, 'original_validation_replaced': False,
            'core_loaded_origins': loaded_origins(), 'fixture_loaded_origins': fixture_origins()})
        self.hit = True
        self.exit_self(EXPECTED_EXIT)
        raise RuntimeError('executor self-exit unexpectedly returned')

    def host_boundary(self, event_name, artifact): self.observe('host', event_name, artifact)

    def observed_return(self, stream, action_id, result):
        if self.point['stream'] != stream or self.point['event'] != action_id: return
        artifact = write(self.executor_root / 'observed-return.json', {
            'action_id': action_id, 'result': result, 'original_function_completed': True})
        self.observe(stream, action_id, artifact)


def appended_ref(directory, result):
    """Find exactly the newly retained core row without inventing its file name."""
    matches = [path for path in Path(directory).glob('*.json') if read(path) == result]
    require(len(matches) == 1, 'durable append boundary is absent or ambiguous')
    return reference(matches[0])


@contextmanager
def wrap_core(core, injection):
    """Scoped exact-core wrappers in the single owned executor process only."""
    journal_cls = core['m'].Journal; ledger_cls = core['recovery_ledger'].RecoveryLedger
    original_journal, original_ledger = journal_cls.append, ledger_cls.append
    driver = core['production_recovery_driver']; original_execute = driver.execute_one
    import owned_fixture_cleanup
    original_cleanup = owned_fixture_cleanup.perform
    old_hook = injection.host.boundary_hook

    def journal_append(instance, event_type, **data):
        result = original_journal(instance, event_type, **data)
        require(Path(instance.root) == injection.root / 'controller', 'foreign Journal in fixture executor')
        injection.observe('controller', event_type, appended_ref(instance.root, result))
        return result

    def ledger_append(instance, event_type, **data):
        result = original_ledger(instance, event_type, **data)
        injection.observe('recovery', event_type, appended_ref(injection.root / 'recovery', result))
        return result

    def execute_one(controller, checkpoint, action):
        require(controller.host is injection.host, 'foreign controller return wrapper')
        result = original_execute(controller, checkpoint, action)
        injection.observed_return('action_return', action['id'], result)
        return result

    def cleanup_perform(host, action, context, done):
        require(host is injection.host, 'foreign cleanup return wrapper')
        result = original_cleanup(host, action, context, done)
        injection.observed_return('cleanup_return', action, result)
        return result

    journal_cls.append = journal_append; ledger_cls.append = ledger_append
    driver.execute_one = execute_one; owned_fixture_cleanup.perform = cleanup_perform
    injection.host.boundary_hook = injection.host_boundary
    try:
        yield
    finally:
        journal_cls.append = original_journal; ledger_cls.append = original_ledger
        driver.execute_one = original_execute; owned_fixture_cleanup.perform = original_cleanup
        injection.host.boundary_hook = old_hook


def verify_executor_exit(start, exit_record, injection_ack, point, release_ref):
    """Absence alone never proves exit; coordinator must retain a natural reap."""
    require(start.get('release') == release_ref and exit_record.get('start') == start
            and exit_record.get('reaped') is True and exit_record.get('returncode') == EXPECTED_EXIT
            and exit_record.get('signals_sent') == 0 and exit_record.get('timed_out') is False,
            'executor did not naturally exit at expected boundary')
    require(injection_ack.get('point') == point and injection_ack.get('executor_identity') == start.get('identity')
            and injection_ack.get('schema') == 'owned_fixture_injection_ack_v1'
            and injection_ack.get('expected_exit_code') == EXPECTED_EXIT
            and injection_ack.get('signals_sent_by_injection') == 0
            and injection_ack.get('original_validation_replaced') is False,
            'executor boundary acknowledgement differs')
    release = check_reference(release_ref)
    require(injection_ack.get('manifest') == release['injections'], 'acknowledged injection manifest differs')
    manifest = validate_manifest(check_reference(injection_ack['manifest']))
    require(manifest['points'].count(point) == 1, 'acknowledged point not exactly in released manifest')
    boundary = check_reference(injection_ack['boundary'])
    path = Path(injection_ack['boundary']['path'])
    require(path.is_relative_to(Path(release['fixture']['root']))
            and all(boundary.get(key) == value for key, value in point['match'].items()),
            'acknowledged boundary path/match differs')
    return {'verified': True, 'production_qualified': False}
