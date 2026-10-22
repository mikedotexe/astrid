"""Finite root permits for one seed and nine private restore children.

This module only reads/writes owned evidence and waits for permits. It never
starts, observes, signals, retries or resumes a process. The frozen supervisor
policy is imported for its pure readiness/coverage functions only.
"""
from __future__ import annotations

import ast
import copy
import hashlib
import importlib.util
import json
import math
import os
from pathlib import Path
import stat
import time


def require(value, message):
    if not value:
        raise ValueError(message)


def number(value):
    return type(value) in (int, float) and math.isfinite(value)


def stable(path):
    """Owned regular evidence, including an atomically published monitor file."""
    path = Path(path)
    require(path.is_absolute() and path.parent == path.parent.resolve(), 'canonical absolute evidence required')
    before = path.lstat()
    require(stat.S_ISREG(before.st_mode) and before.st_uid == os.getuid()
            and before.st_nlink == 1 and not before.st_mode & 0o022
            and before.st_size <= 32 * 1024 * 1024, 'unsafe admission evidence')
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW)
    try:
        opened = os.fstat(fd)
        require((opened.st_dev, opened.st_ino) == (before.st_dev, before.st_ino), 'evidence replaced during open')
        with os.fdopen(fd, 'rb', closefd=False) as stream:
            raw = stream.read(32 * 1024 * 1024 + 1)
        after = os.fstat(fd)
        require(len(raw) == before.st_size and (after.st_size, after.st_mtime_ns) ==
                (before.st_size, before.st_mtime_ns), 'evidence changed during read')
    finally:
        os.close(fd)
    value = json.loads(raw, parse_constant=lambda value: (_ for _ in ()).throw(ValueError('nonfinite evidence')))
    return value, {'path': str(path), 'sha256': hashlib.sha256(raw).hexdigest()}


def checked(reference):
    value, actual = stable(reference['path'])
    require(actual == reference, 'immutable admission reference changed')
    return value


def check_bytes(reference):
    """Stream a retained log/request/witness hash without interpreting its text."""
    path = Path(reference['path']); before = path.lstat()
    require(path.is_absolute() and stat.S_ISREG(before.st_mode) and before.st_uid == os.getuid()
            and before.st_nlink == 1 and not before.st_mode & 0o022, 'unsafe retained child artifact')
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW)
    try:
        opened = os.fstat(fd)
        require((opened.st_dev, opened.st_ino) == (before.st_dev, before.st_ino), 'retained artifact replaced')
        with os.fdopen(fd, 'rb', closefd=False) as stream:
            sha = hashlib.file_digest(stream, 'sha256').hexdigest()
        after = os.fstat(fd)
        require((after.st_size, after.st_mtime_ns) == (before.st_size, before.st_mtime_ns)
                and sha == reference['sha256'], 'retained child artifact changed')
    finally:
        os.close(fd)


def result_dependencies(result):
    references = {}
    def collect(value):
        if isinstance(value, dict):
            if set(value) == {'path', 'sha256'}:
                check_bytes(value); references[value['path']] = value
            else:
                for nested in value.values(): collect(nested)
        elif isinstance(value, list):
            for nested in value: collect(nested)
    collect(result)  # Exact request, child stdout/stderr, seed or witness refs.
    if result.get('witness'):
        witness = checked(result['witness'])
        comparison = witness.get('evidence', {}).get('comparison')
        if comparison is not None: collect(comparison)
    return [references[key] for key in sorted(references)]


def validate_acceptance(reference):
    acceptance = checked(reference)
    child, result = checked(acceptance['child_exit']), checked(acceptance['result'])
    require(acceptance['passed'] is True and result.get('passed') is True and child.get('reaped') is True,
            'previous child result not accepted')
    require(result_dependencies(result) == acceptance['raw_dependencies'], 'previous result dependencies differ')
    return acceptance, child


def load_policy(inputs):
    reference = inputs['admission_supervision']['helper']
    path = Path(reference['path'])
    require(hashlib.sha256(path.read_bytes()).hexdigest() == reference['sha256'], 'supervisor policy source changed')
    spec = importlib.util.spec_from_file_location('restore_child_supervisor_policy', path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)  # main/read_functions/observe are never called.
    return module


def verify_monitor_revision(source, inputs, binding):
    """Only fresh output/stop/token constants may differ from reviewed v2."""
    base = ast.parse(Path(inputs['admission_supervision']['helper']['path']).read_text())
    fresh = ast.parse(source.read_text())
    names = {'OUTPUT', 'STOP', 'TOKEN'}
    def assignments(tree):
        return {node.targets[0].id: node for node in tree.body if isinstance(node, ast.Assign)
                and len(node.targets) == 1 and isinstance(node.targets[0], ast.Name)
                and node.targets[0].id in names}
    old, new = assignments(base), assignments(fresh)
    require(set(old) == names and set(new) == names, 'monitor constant seam differs')
    def rooted_path(node):
        value = node.value
        require(isinstance(value, ast.BinOp) and isinstance(value.op, ast.Div)
                and isinstance(value.left, ast.Name) and value.left.id == 'ROOT'
                and isinstance(value.right, ast.Constant) and isinstance(value.right.value, str)
                and Path(value.right.value).name == value.right.value, 'monitor path constant differs')
        return source.parent / value.right.value
    require(str(rooted_path(new['OUTPUT'])) == binding['record_path']
            and rooted_path(new['STOP']) != rooted_path(new['OUTPUT']), 'monitor output binding differs')
    require(isinstance(new['TOKEN'].value, ast.Constant)
            and new['TOKEN'].value.value == binding['token'], 'monitor token binding differs')
    for name in names:
        new[name].value = copy.deepcopy(old[name].value)
    require(ast.dump(fresh, include_attributes=False) == ast.dump(base, include_attributes=False),
            'fresh monitor changed beyond reviewed output/stop/token constants')


def validate_release_supervision(release, inputs):
    """Bind a reviewed fresh monitor run; never start/reset a supervisor."""
    require(release.get('schema') == 'reservoir_restore_qualification_release_v2', 'v2 per-child release required')
    wait = release.get('permit_wait_seconds')
    require(type(wait) is int and 1 <= wait <= 300, 'finite permit wait must be 1..300 seconds')
    binding = release['supervision']
    require(set(binding) == {'record_path', 'token', 'observer_pid', 'started_at', 'identity_start',
                            'source', 'source_review', 'sampler', 'bindings'}, 'complete supervisor identity required')
    require(type(binding['observer_pid']) is int and binding['observer_pid'] > 0
            and isinstance(binding['token'], str) and binding['token']
            and isinstance(binding['started_at'], str) and binding['started_at'], 'invalid supervisor identity')
    root = Path(inputs['admission_supervision']['helper']['path']).parent
    require(Path(binding['record_path']).parent == root and Path(binding['source']['path']).parent == root,
            'supervisor must remain in reviewed private observation directory')
    fresh_bindings = checked(binding['bindings'])
    original_bindings = checked(inputs['admission_supervision']['bindings'])
    groups = ('identity_files', 'protected_processes', 'expected_launchd_pids',
              'expected_listener_pids', 'coordination')
    require(all(fresh_bindings.get(key) == original_bindings.get(key) and fresh_bindings.get(key)
                for key in groups), 'protected observer binding maps differ')
    source = Path(binding['source']['path'])
    require(source.is_file() and not source.is_symlink()
            and hashlib.sha256(source.read_bytes()).hexdigest() == binding['source']['sha256'], 'monitor source changed')
    review = checked(binding['source_review'])
    require(review.get('schema') == 'reservoir_matrix_supervision_source_review_v1'
            and review.get('approved_source') is True
            and review.get('status') == 'accepted_new_instance_source_only_not_executed'
            and isinstance(review.get('reviewer'), str) and review['reviewer'].strip()
            and review.get('unresolved_source_blockers') == []
            and review.get('source') == binding['source']
            and review.get('prior_source') == inputs['admission_supervision']['helper']
            and review.get('prior_source_review') == inputs['admission_supervision']['review']
            and review.get('future_observer_binding') == binding['bindings']
            and review.get('scope', {}).get('review_only') is True
            and review.get('scope', {}).get('numerical_work_authorized') is False
            and review.get('scope', {}).get('production_mutations') is False,
            'reviewed exact supervisor source required')
    verify_monitor_revision(source, inputs, binding)
    policy = load_policy(inputs)
    require(binding['sampler'] == {'path': str(policy.SOURCE), 'sha256': policy.SOURCE_SHA}, 'sampler family differs')
    require(review.get('sampler_source') == binding['sampler'], 'review sampler source differs')
    sampler_bindings = checked(review['sampler_binding'])
    require(all(sampler_bindings.get(key) == fresh_bindings[key] for key in groups),
            'actual sampler protected maps differ from accepted host gate')
    require(hashlib.sha256(Path(policy.SOURCE).read_bytes()).hexdigest() == policy.SOURCE_SHA, 'sampler changed')
    raw = checked(release['host_gate_raw'])
    require(binding['identity_start'] == raw['production_identity_end']
            and raw.get('input_binding_path') == binding['bindings']['path']
            and raw.get('input_binding_sha256') == binding['bindings']['sha256'],
            'supervisor protected identities differ from accepted gate')
    return binding


def validate_monitor(record, binding, policy, *, now, ready):
    require(record.get('schema') == 'reservoir_matrix_host_supervision_v2'
            and record.get('swap_policy') == 'nonincrease_between_consecutive_samples_and_not_above_initial',
            'exact v2 supervisor semantics required')
    for key in ('token', 'observer_pid', 'started_at', 'identity_start'):
        require(record.get(key) == binding[key], 'supervisor run identity changed: ' + key)
    require(record.get('self_sha256') == binding['source']['sha256']
            and record.get('source_sha256') == binding['sampler']['sha256'], 'supervisor source identity changed')
    require(record.get('production_mutations') == 0 and record.get('qualification_cases_started') == 0,
            'supervisor is not evidence-only')
    if ready:
        require(policy.ready_for_next_case(record, now), 'supervisor not fresh/ready for next child')
    return record


class AdmissionRun:
    """Single invocation only. All retained attempts are append-only, never resumed."""

    def __init__(self, api, inputs, release, run_root, *, release_ref, clock=time.time,
                 monotonic=time.monotonic, sleep=time.sleep, policy=None):
        self.api, self.inputs, self.release, self.root = api, inputs, release, Path(run_root)
        self.release_ref = dict(release_ref)
        require(api.check_ref(self.release_ref) == release, 'admission release differs from original entry bytes')
        self.binding = release['supervision']
        self.policy = policy if policy is not None else load_policy(inputs)
        self.clock, self.monotonic, self.sleep = clock, monotonic, sleep
        self.directory = self.root / 'admissions'; self.directory.mkdir(mode=0o700)
        self.ordinal, self.previous, self.pending = 0, None, None

    def monitor(self, *, ready):
        row, reference = stable(self.binding['record_path'])
        validate_monitor(row, self.binding, self.policy, now=self.clock(), ready=ready)
        return row, reference

    def previous_valid(self, monitor):
        if self.previous is None:
            require(self.ordinal == 0, 'previous child acceptance missing')
            return
        acceptance, child = validate_acceptance(self.previous)
        require(self.policy.covers_case(monitor, child['started_wall'], child['completed_wall']),
                'previous child lacks current complete supervisor coverage')
        old = checked(acceptance['coverage'])
        require(monitor['samples'][:len(old['samples'])] == old['samples'], 'supervisor prior coverage rewritten')

    def request(self, case, argv, directory, request_ref):
        require(self.pending is None and self.ordinal < 10, 'prior admission unresolved or too many children')
        expected = ('seed',) + self.api.CASES
        require(case == expected[self.ordinal], 'child order changed')
        directory = Path(directory)
        require(directory == self.root / case and directory == directory.resolve(), 'private child directory differs')
        if request_ref is not None:
            checked(request_ref)
        require((case == 'seed') == (request_ref is None), 'seed/request binding differs')
        self.api.check_ref(self.release_ref)
        row, _ = self.monitor(ready=True)
        self.previous_valid(row)
        waiting = {'schema': 'reservoir_restore_child_request_v1', 'run_id': self.release['run_id'],
                   'ordinal': self.ordinal, 'case': case, 'argv': list(argv), 'directory': str(directory),
                   'request': request_ref, 'inputs': self.api.ref(self.api.INPUTS),
                   'run_release': self.release_ref, 'artifacts': self.api.harness_hashes(),
                   'input_artifacts': self.inputs['artifacts'], 'supervision': self.binding,
                   'previous_acceptance': self.previous, 'production_qualified': False}
        stem = f'{self.ordinal:02d}-{case}'
        path = self.directory / (stem + '-request.json'); self.api.write(path, waiting)
        self.pending = ChildGate(self, waiting, self.api.ref(path), stem)
        return self.pending

    def accept(self, gate, child, result_path):
        require(gate is self.pending and gate.claimed, 'unclaimed child cannot be accepted')
        require(child.get('reaped') is True and number(child.get('started_wall'))
                and number(child.get('completed_wall')) and child['started_wall'] < child['completed_wall'],
                'natural reaped child interval required')
        exit_ref = self.api.ref(Path(gate.waiting['directory']) / 'child-exit.json')
        require(checked(exit_ref) == child, 'child exit evidence changed')
        result_ref = self.api.ref(result_path); result = checked(result_ref)
        require(result.get('passed') is True, 'strict child result failed')
        deadline = self.monotonic() + self.release['permit_wait_seconds']
        while True:
            self.api.check_ref(self.release_ref)
            require(self.clock() <= self.release['expires_wall'], 'run release expired waiting for coverage')
            row, _ = self.monitor(ready=True)
            if row['samples'][-1]['t_s'] >= child['completed_wall']:
                break
            require(self.monotonic() < deadline, 'post-child coverage wait exhausted')
            self.sleep(.25)
        require(self.policy.covers_case(row, child['started_wall'], child['completed_wall']),
                'actual child interval invalidated')
        coverage_path = self.directory / (gate.stem + '-coverage.json'); self.api.write(coverage_path, row)
        acceptance = {'schema': 'reservoir_restore_child_acceptance_v1', 'passed': True,
                      'run_release': self.release_ref, 'request': gate.request_ref,
                      'permit': gate.permit_ref, 'claim': self.api.ref(gate.claim_path),
                      'child_exit': exit_ref, 'result': result_ref, 'coverage': self.api.ref(coverage_path),
                      'raw_dependencies': result_dependencies(result),
                      'production_qualified': False}
        accepted_path = self.directory / (gate.stem + '-accepted.json'); self.api.write(accepted_path, acceptance)
        self.previous = self.api.ref(accepted_path); self.ordinal += 1; self.pending = None
        return self.previous


class ChildGate:
    def __init__(self, run, waiting, request_ref, stem):
        self.run, self.waiting, self.request_ref, self.stem = run, waiting, request_ref, stem
        self.permit_path = run.directory / (stem + '-permit.json')
        self.claim_path = run.directory / (stem + '-launch-claim.json')
        self.permit_ref = None; self.claimed = False

    def wait(self):
        run = self.run; deadline = run.monotonic() + run.release['permit_wait_seconds']
        while not os.path.lexists(self.permit_path):
            run.api.check_ref(run.release_ref)
            require(run.clock() <= run.release['expires_wall'], 'run release expired waiting for permit')
            run.monitor(ready=True)
            require(run.monotonic() < deadline, 'root permit wait exhausted; no child launched')
            run.sleep(.25)
        permit, self.permit_ref = stable(self.permit_path)
        self.validate_permit(permit)
        return self

    def validate_permit(self, permit):
        run = self.run
        require(permit.get('schema') == 'reservoir_restore_child_permit_v1'
                and permit.get('accepted') is True and isinstance(permit.get('reviewer'), str)
                and permit['reviewer'].strip(), 'explicit root child permit required')
        require(permit.get('request') == self.request_ref and permit.get('run_release') == run.release_ref
                and permit.get('previous_acceptance') == run.previous, 'child permit binding differs')
        require(number(permit.get('expires_wall'))
                and run.clock() <= permit['expires_wall'] <= run.release['expires_wall'], 'child permit expired/extends run')
        snapshot = checked(permit['supervisor_snapshot'])
        validate_monitor(snapshot, run.binding, run.policy, now=run.clock(), ready=True)
        run.previous_valid(snapshot)
        return snapshot

    def claim(self, argv, directory):
        """Called inside run_child immediately before Popen; no process capability."""
        run = self.run
        require(not self.claimed and run.pending is self and self.permit_ref is not None,
                'one-use waited admission required')
        require(list(argv) == self.waiting['argv'] and str(directory) == self.waiting['directory'], 'launch argv/directory changed')
        require(checked(self.request_ref) == self.waiting, 'waiting request changed')
        run.api.check_ref(run.release_ref)
        run.api.check()
        run.api.validate_release(Path(run.release_ref['path']), run.inputs, run.release['run_id'], fresh=False)
        require(run.api.harness_hashes() == self.waiting['artifacts'], 'harness changed before launch')
        if self.waiting['request'] is not None:
            checked(self.waiting['request'])
        permit = checked(self.permit_ref); snapshot = self.validate_permit(permit)
        before, reference = run.monitor(ready=True)
        require(before['samples'][:len(snapshot['samples'])] == snapshot['samples'], 'supervisor permit history rewritten')
        run.previous_valid(before)
        run.api.write(self.claim_path, {'schema': 'reservoir_restore_child_launch_claim_v1',
                      'request': self.request_ref, 'permit': self.permit_ref, 'run_release': run.release_ref,
                      'supervisor_observed': reference, 'claimed_wall': run.clock(),
                      'automatic_retry_permitted': False, 'production_qualified': False})
        self.claimed = True  # Any following failure is a retained attempted boundary.
        final, _ = run.monitor(ready=True)
        require(final['samples'][:len(before['samples'])] == before['samples'], 'supervisor changed across claim')
        require(run.clock() <= permit['expires_wall'], 'permit expired before Popen')
        return run.api.ref(self.claim_path)
