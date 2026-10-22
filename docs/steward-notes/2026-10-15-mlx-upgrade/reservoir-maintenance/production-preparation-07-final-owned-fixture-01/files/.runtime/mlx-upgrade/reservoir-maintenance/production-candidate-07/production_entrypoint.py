"""Fixed ordinary-user maintenance entrypoint; default is read-only checking.

Fresh execution invokes the existing controller once. Recovery invokes exactly
one existing execute/reconcile step with caller-supplied expected history heads.
No retry, cleanup, arbitrary target/command or privilege elevation exists here.
Both production flags remain independently authoritative, including for capture.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import sys

import production_preflight as p
import production_native as native
import production_native_attempt as native_attempt
import production_host as h
import production_recovery_inspect as inspection
import production_recovery_driver as driver
import recovery_ledger as ledger
import recovery_phases as phases
import reservoir_maintenance as m

HERE = Path(__file__).resolve().parent
MODES = ('check', 'inspect', 'fresh', 'resume-execute', 'resume-reconcile')
ACTION_MODES = frozenset(MODES[2:])


def record(path, expected_sha256):
    """Read one exact, bounded, ordinary-user-owned record without following links."""
    path = Path(path)
    m.require(path.is_absolute() and path == path.resolve(strict=True), 'record path must be canonical and absolute')
    m.require(m.hash_value(expected_sha256), 'exact record SHA256 required')
    raw, _ = p.stable_bytes(path, maximum=m.MAX_RECORD)
    m.require(hashlib.sha256(raw).hexdigest() == expected_sha256, 'selected record changed')
    value = json.loads(raw, parse_constant=lambda value: (_ for _ in ()).throw(m.Refusal('nonfinite record')))
    m.require(isinstance(value, dict), 'selected record must be an object')
    return value


def ordinary_user(*, interactive):
    m.require(os.getuid() > 0 and os.getuid() == os.geteuid(), 'entrypoint requires an ordinary non-elevated user')
    if interactive:
        m.require(sys.stdin.isatty(), 'explicit execution requires the user Terminal')


def execution_flags():
    m.require(m.PRODUCTION_CONTROLLER_ENABLED is True and h.PRODUCTION_EXECUTION_ENABLED is True,
              'production controller and Host execution remain disabled')


def source_and_interpreter(spec):
    """Bind fixed local module origins and both exact pinned framework binaries."""
    files = spec.get('immutable_files', {})
    helpers = set(spec.get('input_groups', {}).get('helpers', []))
    runtime_paths = {path for path in HERE.glob('*.py') if not path.name.startswith('test_')}
    m.require(runtime_paths and all(path == path.resolve(strict=True) for path in runtime_paths),
              'runtime helper path is aliased')
    for path in runtime_paths | {p.HOLD_SOURCE}:
        m.require(str(path) in helpers and m.hash_value(files.get(str(path)))
                  and p.sha(path) == files[str(path)], 'entrypoint helper closure changed: ' + str(path))
    for module in (p, h, inspection, driver, ledger, phases, m, native, native_attempt):
        m.require(Path(module.__file__).resolve(strict=True) == HERE / (module.__name__ + '.py'),
                  'loaded helper origin differs: ' + module.__name__)
    family = p.load_hold().interpreter_paths(h.HOLDER_PYTHON)
    m.require(Path(sys.executable).resolve(strict=True) in family, 'entrypoint interpreter is outside pinned family')
    for path in family:
        m.require(m.hash_value(files.get(str(path))) and p.sha(path) == files[str(path)],
                  'entrypoint interpreter changed: ' + str(path))


def run(preparation_path, preparation_sha256, *, mode='check', heads_path=None, heads_sha256=None, native_attempt_name=None):
    m.require(mode in MODES, 'unsupported entrypoint mode')
    action = mode in ACTION_MODES
    ordinary_user(interactive=action)
    # Disabled preparation cannot even create a transaction or capture anchor.
    if action:
        execution_flags()
    recovery = mode.startswith('resume-')
    m.require((heads_path is not None and heads_sha256 is not None) if recovery
              else heads_path is None and heads_sha256 is None, 'expected heads apply only to explicit one-step recovery')
    preparation_path = Path(preparation_path)
    spec = record(preparation_path, preparation_sha256)
    plan = m.validate_plan(spec['plan'])
    root = Path(spec['transaction_root'])
    m.require(root == p.CONTROL_ROOT / plan['transaction_id'], 'transaction root is not the fixed owned child')
    m.require(plan['host_binding']['uid'] == os.getuid(), 'plan belongs to another user')
    source_and_interpreter(spec)
    io=native_attempt.reserve(preparation_path,preparation_sha256,spec,mode,native_attempt_name,
               io_class=inspection.ReadOnlyIO if mode=='inspect' else p.ReadOnlySystem)
    if mode == 'check':
        return p.check_only(spec,io)
    journal_path = root / 'controller'
    if mode == 'inspect':
        report = inspection.inspect(preparation_path, journal_path, io=io)
        result = {'schema': 'reservoir_entrypoint_inspection_v1', 'inspection': report,
                  'mutation_performed': False, 'automatic_next_action': False}
        if report.get('evidence_valid') is True:
            checkpoint = phases.checkpoint_from_inspection(spec, report, ledger.read_ledger(root, plan))
            result['expected_heads'] = driver.expected_heads(checkpoint)
            result['action'] = checkpoint['action']
            result['requires_reconciliation'] = checkpoint['requires_reconciliation']
        return result
    if mode == 'fresh':
        m.require(not root.exists() and not root.is_symlink(), 'retained transaction exists; inspect it, never replay fresh')
        # Raw acceptance is checked before capture, and again by the controller
        # under its shared lock and each actual Host operation.
        import production_admission as admission
        m.require(Path(admission.__file__).resolve(strict=True) == HERE / 'production_admission.py',
                  'admission reader loaded origin differs')
        admission.validate_admission(spec, plan)
        host = h.ProductionHost(spec,io=io)
        host.preparation_path = preparation_path
        host.journal_path = journal_path
        m.require(record(preparation_path, preparation_sha256) == spec, 'preparation changed before capture')
        host.prepare_capture()
        journal = m.Journal(journal_path, plan=plan)
        m.require(record(preparation_path, preparation_sha256) == spec, 'preparation changed after capture; retain transaction')
        source_and_interpreter(spec)
        return m.Controller(journal, host).run()
    expected = record(heads_path, heads_sha256)
    m.require(set(expected) == set(driver.HEAD_KEYS) and all(m.hash_value(value) for value in expected.values()),
              'exact four recovery history hashes required')
    host = h.ProductionHost.from_verified_recovery(preparation_path, journal_path,io=io)
    m.require(host.spec == spec and host.plan == plan, 'retained preparation changed during attachment')
    journal = m.Journal(journal_path)
    m.require(record(preparation_path, preparation_sha256) == spec, 'preparation changed before one-step continuation')
    # resume_one recomputes the checkpoint under the existing shared lock.
    return driver.resume_one(journal, host, expected=expected, mode=mode.removeprefix('resume-'))


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--preparation', type=Path, required=True)
    parser.add_argument('--preparation-sha256', required=True)
    parser.add_argument('--mode', choices=MODES, default='check')
    parser.add_argument('--native-attempt-name',required=True)
    parser.add_argument('--expected-heads', type=Path)
    parser.add_argument('--expected-heads-sha256', dest='heads_sha256')
    args = parser.parse_args(argv)
    try:
        result = run(args.preparation, args.preparation_sha256, mode=args.mode,
                     heads_path=args.expected_heads, heads_sha256=args.heads_sha256,native_attempt_name=args.native_attempt_name)
        print(json.dumps(result, indent=2, allow_nan=False))
        return 1 if result.get('schema') == 'reservoir_entrypoint_inspection_v1' and not result['inspection'].get('evidence_valid') else 0
    except (Exception, KeyboardInterrupt) as error:
        print(json.dumps({'schema': 'reservoir_entrypoint_refusal_v1', 'error_type': type(error).__name__,
                          'error': str(error)[:1500], 'automatic_retry_permitted': False,
                          'retained_state_unchanged_by_cleanup': True,
                          'instruction': 'Preserve all transaction records, holds and state; inspect the exact failed boundary.'},
                         indent=2, allow_nan=False))
        return 1


if __name__ == '__main__':
    raise SystemExit(main())
