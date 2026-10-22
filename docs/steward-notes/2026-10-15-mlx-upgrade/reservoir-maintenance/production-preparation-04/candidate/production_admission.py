"""Read/hash-only production prerequisites. This module grants no execution authority.

Reuses frozen pure reviewers with scoped module aliases; never invokes their run,
execute, main, process, socket, or numerical paths. No work occurs on import.
"""
from __future__ import annotations

from contextlib import contextmanager
import copy
import hashlib
import importlib.util
import math
from pathlib import Path
import sys

import reservoir_maintenance as m
import production_preflight as p

HERE = Path(__file__).resolve().parent
SERVICE_HARNESS = HERE.parent.parent / 'reservoir-snapshot-candidate/qualification-harness'
RESTORE_HARNESS = HERE.parent / 'restore-qualification-01'
RUNTIME_HELPERS = ('reservoir_maintenance.py', 'production_admission.py', 'production_host.py',
                   'production_preflight.py', 'production_recovery.py', 'production_recovery_inspect.py',
                   'production_restore_runner.py', 'production_retained.py', 'transaction_execution.py',
                   'recovery_ledger.py', 'recovery_phases.py', 'production_recovery_driver.py', 'reservoir_maintenance_state.py', 'restore_verifier.py')
SERVICE_MODULES = ('common', 'fixture', 'process_identity', 'result_checks', 'driver', 'review_matrix')
RESTORE_MODULES = ('reservoir_maintenance_state', 'restore_verifier', 'qualify_restore')
RESTORE_CASES = ('complete', 'missing-sidecar', 'malformed-sidecar', 'corrupt-npz',
                 'schema-mismatch', 'config-mismatch', 'source-drift', 'package-drift', 'backup-drift')


def file_hash(path):
    with Path(path).open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def reference(value):
    m.require(isinstance(value, dict) and set(value) == {'path', 'sha256'}
              and isinstance(value['path'], str) and Path(value['path']).is_absolute()
              and m.hash_value(value['sha256']), 'invalid prerequisite reference')
    path = Path(value['path'])
    m.require(path.is_file() and not path.is_symlink() and file_hash(path) == value['sha256'],
              'prerequisite artifact changed: ' + str(path))
    return m.read_record(path)


def ref(path):
    return {'path': str(Path(path).absolute()), 'sha256': file_hash(path)}


def finite(value):
    return type(value) in (int, float) and math.isfinite(value)


def hash_files(values):
    m.require(isinstance(values, dict) and values, 'artifact closure empty')
    for path, expected in values.items():
        m.require(isinstance(path, str) and Path(path).is_absolute() and m.hash_value(expected)
                  and file_hash(path) == expected, 'bound artifact changed: ' + str(path))


@contextmanager
def modules(directory, names, bound):
    """Temporarily load only hash-bound stdlib-only reader modules, then restore aliases."""
    paths = {name: directory / (name + '.py') for name in names}
    for path in paths.values():
        m.require(bound.get(str(path)) == file_hash(path), 'unbound proof reader: ' + str(path))
    prior = {name: sys.modules.get(name) for name in names}
    previous_path, previous_bytecode = list(sys.path), sys.dont_write_bytecode
    loaded = {}
    try:
        for name in names:
            sys.modules.pop(name, None)
        sys.path.insert(0, str(directory)); sys.dont_write_bytecode = True
        for name, path in paths.items():
            spec = importlib.util.spec_from_file_location(name, path)
            module = importlib.util.module_from_spec(spec)
            sys.modules[name] = module; spec.loader.exec_module(module); loaded[name] = module
        yield loaded
        for path in paths.values():
            m.require(bound[str(path)] == file_hash(path), 'proof reader changed while validating')
    finally:
        sys.path[:] = previous_path; sys.dont_write_bytecode = previous_bytecode
        for name, value in prior.items():
            if value is None: sys.modules.pop(name, None)
            else: sys.modules[name] = value


def validate_service_matrix(spec, review, artifacts):
    m.require(review.get('schema') == 'reservoir_private_matrix_review_v1'
              and review.get('accepted_for_private_numpy_only') is True
              and review.get('deployment_accepted') is False and review.get('mlx_upgrade_qualified') is False,
              'actual private service matrix review missing')
    receipts = review.get('receipts', [])
    m.require(len(receipts) == 14 and len({item['path'] for item in receipts}) == 14,
              'complete fourteen-case matrix required')
    records = [reference(item) for item in receipts]
    with modules(SERVICE_HARNESS, SERVICE_MODULES, artifacts) as loaded:
        actual = loaded['review_matrix'].review([Path(item['path']) for item in receipts])
        m.require(actual == review, 'retained service matrix fails frozen raw re-review')
        runtime = spec['restore_runtime_manifest']; source = spec['restore_source_manifest']['files']
        for row, receipt in zip(records, receipts):
            manifest = reference({'path': row['manifest'], 'sha256': row['manifest_sha256']})
            original = dict(source); original['reservoir_service.py'] = spec['plan']['input_hashes']['reservoir_source']
            m.require(manifest['variants'] == {'original': original, 'candidate': source},
                      'matrix original/candidate source closure differs from preparation')
            interpreter = manifest['expected_interpreter']
            m.require(interpreter['executable'] == runtime['python']
                      and interpreter['files'] == runtime['interpreter_files'], 'matrix interpreter differs')
            package_names = ('numpy', 'torch', 'scikit-learn', 'websockets')
            expected_packages = {name: runtime['packages'][name]['version'] for name in package_names}
            m.require(manifest['expected_packages'] == expected_packages, 'matrix package versions differ')
            child = m.read_record(Path(receipt['path']).parent / 'child.json')
            m.require(child['distribution_files'] == {name: runtime['packages'][name]['files'] for name in package_names},
                      'matrix original installed package bytes differ')
            m.require(row['interpreter'] == interpreter, 'matrix execution interpreter differs')
    return {'case_count': 14, 'source_files': source}


def validate_child(child, directory, argv, expected_code, before, after):
    m.require(isinstance(child, dict) and child.get('reaped') is True
              and child.get('returncode') == expected_code and type(child.get('pid')) is int and child['pid'] > 1
              and child.get('argv') == argv and finite(child.get('started_wall'))
              and finite(child.get('completed_wall'))
              and before <= child['started_wall'] <= child['completed_wall'] <= after,
              'child exit/identity/order/command evidence differs')
    m.require(not (directory / 'child-unresolved.json').exists(), 'unresolved qualification child retained')
    for key, filename in (('stdout', 'stdout.txt'), ('stderr', 'stderr.txt')):
        m.require(child[key]['path'] == str(directory / filename), 'child stream outside owned case')
        ref_value = child[key]
        m.require(set(ref_value) == {'path', 'sha256'} and file_hash(ref_value['path']) == ref_value['sha256'],
                  'child stream hash changed')
    m.require(m.read_record(directory / 'child-exit.json') == child, 'child-exit receipt differs')
    start = m.read_record(directory / 'child-start.json')
    m.require(start == {'pid': child['pid'], 'argv': argv, 'started_wall': child['started_wall'],
                       'scope': 'owned_synthetic_cpu_child', 'signals_permitted': False}, 'child-start receipt differs')


def validate_case_inputs(v, inputs, request, case, directory):
    backup = directory / 'backup'
    actual = v.state.inventory_tree(backup)
    expected = request['backup_witness']['evidence']['backup_inventory']
    if case == 'backup-drift':
        drift = 'intentional-private-drift.txt'
        m.require(drift not in expected['entries'] and drift in actual['entries']
                  and (backup / drift).read_bytes() == b'not in bound inventory\n', 'backup drift control changed')
        actual = copy.deepcopy(actual); actual['entries'].pop(drift)
        m.require(v.file_content(actual) == v.file_content(expected), 'backup changed beyond named drift control')
    else:
        m.require(actual == expected, 'retained negative/positive backup changed')
    if case == 'source-drift':
        source = directory / 'source'; original = Path(inputs['source_manifest']['root'])
        names = set(inputs['source_manifest']['files'])
        m.require({p.name for p in source.iterdir()} == names, 'extra source drift artifacts')
        for name in names:
            expected_bytes = (original / name).read_bytes()
            if name == 'persistence.py': expected_bytes += b'\n# intentional private drift, never imported\n'
            m.require((source / name).is_file() and not (source / name).is_symlink()
                      and (source / name).read_bytes() == expected_bytes, 'source drift control changed')


def validate_restore_result(spec, result, artifacts):
    m.require(result.get('schema') == 'reservoir_restore_qualification_v1' and result.get('passed') is True
              and result.get('scope') == 'synthetic_cpu_fixture_only' and result.get('production_qualified') is False
              and result.get('errors') == [] and result.get('final_runtime_and_positive_witness_revalidated') is True,
              'actual nine-case restore qualification incomplete')
    m.require(finite(result.get('started_wall')) and finite(result.get('completed_wall'))
              and result['started_wall'] <= result['completed_wall'], 'qualification timing missing')
    rows = result.get('cases', [])
    m.require([row.get('case') for row in rows] == list(RESTORE_CASES), 'exact ordered nine restore cases required')
    for path, sha in result.get('artifacts', {}).items():
        m.require(artifacts.get(path) == sha, 'restore harness is not part of exact execution review')
    inputs = reference(result['inputs']); release = reference(result['release']); seed = reference(result['seed'])
    run_root = Path(spec['restore_qualification']['path']).parent
    m.require(run_root.parent == RESTORE_HARNESS / 'runs'
              and result['inputs']['path'] == str(RESTORE_HARNESS / 'inputs.json')
              and result['seed']['path'] == str(run_root / 'seed/seed-receipt.json'), 'unexpected restore proof location')
    with modules(RESTORE_HARNESS, RESTORE_MODULES, artifacts) as loaded:
        q, v = loaded['qualify_restore'], loaded['restore_verifier']
        m.require(q.check() == inputs and q.harness_hashes() == result['artifacts'], 'frozen restore input closure changed')
        m.require(m.read_record(RESTORE_HARNESS / 'launch-guard.json') == {
            'run_id': run_root.name, 'release': result['release'], 'artifacts': result['artifacts'],
            'automatic_retry_permitted': False}, 'qualification launch guard differs')
        m.require(inputs['runtime_manifest'] == spec['restore_runtime_manifest']
                  and inputs['source_manifest']['files'] == spec['restore_source_manifest']['files']
                  and inputs['config'] == {key: spec['reservoir_config'][key] for key in ('input_dim','n_nodes','backend')},
                  'restore qualification used a different runtime/source/configuration')
        for name in ('restore_verifier.py', 'reservoir_maintenance_state.py'):
            m.require(file_hash(RESTORE_HARNESS / name) == file_hash(HERE / name), 'qualified helper differs from production helper')
        m.require(release.get('schema') == 'reservoir_restore_qualification_release_v1'
                  and release.get('accepted') is True and bool(release.get('reviewer'))
                  and release.get('scope') == 'synthetic_cpu_fixture_only'
                  and release.get('purpose') == 'release_restore_qualification_cpu'
                  and release.get('run_id') == run_root.name and release.get('inputs') == result['inputs']
                  and release.get('artifacts') == result['artifacts'] and release.get('input_artifacts') == inputs['artifacts']
                  and finite(release.get('expires_wall')) and result['started_wall'] <= release['expires_wall'],
                  'historical restore release differs')
        raw = reference(release['host_gate_raw']); gate = reference(release['host_gate_review'])
        q.validate_host_gate(raw, gate, release['host_gate_raw'], now=result['started_wall'], fresh=True)
        m.require(seed.get('schema') == 'reservoir_restore_seed_v1' and seed.get('passed') is True
                  and seed.get('production_qualified') is False and seed.get('scope') == 'synthetic_cpu_fixture_only'
                  and seed.get('errors') == [] and seed.get('inputs') == result['inputs']
                  and seed.get('tasks_started') is False and seed.get('listener_opened') is False
                  and seed.get('torch_device') == 'cpu' and seed.get('torch_threads') == 1
                  and {name: row['backend'] for name, row in seed['expected_startup'].items()}
                      == {'qualification_numpy': 'numpy', 'qualification_shadow__lsm': 'lsm_shadow'}, 'seed coverage incomplete')
        python = inputs['runtime_manifest']['python']
        validate_child(result['seed_child'], run_root / 'seed',
                       [python, '-B', str(RESTORE_HARNESS / 'seed_fixture.py'), '--execute-fixture',
                        '--output', str(run_root / 'seed'), '--inputs', result['inputs']['path']],
                       0, result['started_wall'], result['completed_wall'])
        m.require(seed['pid'] == result['seed_child']['pid'] and seed['python'] == python,
                  'seed process differs from child')
        m.require(m.read_record(run_root / 'seed/seed-execution-guard.json') == {
            'pid': seed['pid'], 'run_id': run_root.name, 'scope': 'synthetic_cpu_fixture_only',
            'automatic_retry_permitted': False}, 'seed execution guard differs')
        m.require(result['seed_child']['started_wall'] <= release['expires_wall'], 'seed launched after release expiry')
        m.require(v.state.inventory_tree(run_root / 'seed/state') == seed['inventory'], 'synthetic seed changed')
        previous = result['seed_child']['completed_wall']
        for row in rows:
            case = row['case']; directory = run_root / case
            m.require(row['request']['path'] == str(directory / 'request.json'), 'case request outside retained case')
            request = reference(row['request'])
            child = row['child']
            validate_child(child, directory, [python, '-B', str(RESTORE_HARNESS / 'restore_verifier.py'),
                           '--request', row['request']['path'], '--execute-private-restore'],
                           0 if case == 'complete' else 1, previous, result['completed_wall'])
            m.require(child['started_wall'] <= release['expires_wall'], 'case launched after historical release expiry')
            previous = child['completed_wall']
            m.require(request['context'] == q.context_for(v, inputs, run_root.name, case), 'synthetic context differs')
            expected_runtime = copy.deepcopy(inputs['runtime_manifest'])
            if case == 'package-drift': expected_runtime['packages']['numpy']['version'] = '0.0.0-private-negative-control'
            expected_source = copy.deepcopy(inputs['source_manifest'])
            if case == 'source-drift': expected_source['root'] = str(directory / 'source')
            expected_config = dict(inputs['config'])
            if case == 'config-mismatch': expected_config['n_nodes'] = 64
            m.require(request['runtime_manifest'] == expected_runtime and request['source_manifest'] == expected_source
                      and request['config'] == expected_config
                      and request['work_root'] == str(v.RUNS / ('fixture-' + run_root.name + '-' + case))
                      and request['backup_witness']['evidence']['backup_root'] == str(directory / 'backup'),
                      'case request differs beyond its named private negative control')
            structurally_valid = copy.deepcopy(request)
            if case == 'config-mismatch': structurally_valid['config'] = dict(inputs['config'])
            v.validate_request(structurally_valid)
            validate_case_inputs(v, inputs, request, case, directory)
            classified = q.classify_case(v, case, request, child, directory, seed)
            m.require({k: value for k, value in row.items() if k not in ('child', 'request')} == classified,
                      'retained restore classification differs from raw evidence')
            if row.get('witness') is not None:
                witness = reference(row['witness'])
                evidence = witness['evidence']; work = Path(request['work_root'])
                m.require(witness.get('bindings') == request['bindings'] and evidence['request'] == row['request']
                          and evidence['comparison']['path'] == str(work / 'comparison.json')
                          and evidence['log']['path'] == str(work / 'restore.log'), 'negative witness provenance differs')
                comparison = reference(evidence['comparison'])
                log = witness['evidence']['log']; m.require(file_hash(log['path']) == log['sha256'], 'restore log changed')
                m.require(comparison['pid'] == child['pid'] and comparison['argv'] == child['argv'][2:]
                          and child['started_wall'] <= comparison['started_wall'] <= comparison['completed_wall'] <= child['completed_wall'],
                          'restore comparison is not from exact child')
    return {'case_count': 9, 'inputs': result['inputs']}


def validate_fixture(spec, plan):
    fixture = reference(spec['fixture']); review = reference(spec['fixture_review'])
    m.require(fixture.get('schema') == 'reservoir_maintenance_launchd_fixture_v1'
              and fixture.get('passed') is True and fixture.get('production_qualified') is False
              and fixture.get('host_binding') == plan['host_binding']
              and fixture.get('cleanup', {}).get('both_jobs_absent') is True
              and fixture['cleanup'].get('force_used') is False
              and fixture.get('controller', {}).get('status') == 'verified'
              and fixture['controller'].get('execution_scope') == 'owned_fixture', 'exact owned fixture not accepted')
    hash_files(fixture['python_executables']); hash_files(fixture['artifacts'])
    m.require(review.get('schema') == 'reservoir_maintenance_actual_fixture_review_v1'
              and review.get('accepted') is True and review.get('scope') == 'owned_fixture_only'
              and review.get('production_qualified') is False and review.get('host_binding') == plan['host_binding']
              and review.get('reviewer') and review.get('artifacts', {}).get(spec['fixture']['path']) == spec['fixture']['sha256'],
              'raw owned fixture lacks exact independent review')
    hash_files(review['artifacts'])
    controller_sha = file_hash(HERE / 'reservoir_maintenance.py')
    controllers = {path: sha for path, sha in fixture['artifacts'].items() if Path(path).name == 'reservoir_maintenance.py'}
    m.require(len(controllers) == 1 and set(controllers.values()) == {controller_sha}
              and all(review['artifacts'].get(path) == sha for path, sha in controllers.items()),
              'fixture did not exercise exact integrated controller')
    selected = [row for row in review.get('roots', []) if row.get('root') == fixture.get('root')
                and row.get('transaction_id') == fixture.get('transaction_id')]
    m.require(len(selected) == 1, 'selected fixture root missing or ambiguous in review')
    root = selected[0]; workers, holders, absent = root.get('worker_pids', []), root.get('holder_pids', []), root.get('pids_absent', [])
    m.require(workers and holders and all(type(pid) is int and pid > 1 for pid in workers + holders + absent)
              and set(workers + holders) <= set(absent)
              and {fixture['controller'][role]['pid'] for role in m.ROLES} <= set(workers)
              and root.get('plan_sha256') == fixture['controller']['plan_sha256']
              and root.get('final_state') == fixture['cleanup'].get('final_state'), 'selected fixture cleanup review incomplete')
    m.require(spec['fixture']['sha256'] == plan['input_hashes']['fixture'], 'fixture plan binding differs')


def validate_admission(spec, plan):
    """Revalidate preserved evidence only; Host separately checks changing live identities."""
    plan = m.validate_plan(plan)
    m.require(spec.get('schema') == 'reservoir_production_preparation_v1' and m.validate_plan(spec['plan']) == plan,
              'admission preparation differs from plan')
    review = reference(spec['execution_review'])
    refs = {name: spec[name] for name in ('execution_review','fixture','fixture_review','real_matrix','restore_qualification')}
    m.require(review.get('schema') == 'reservoir_production_execution_review_v1'
              and review.get('approved') is True and bool(review.get('reviewer'))
              and review.get('preparation_sha256') == p.preparation_digest(spec)
              and all(review.get(name) == spec[name] for name in ('fixture','fixture_review','real_matrix','restore_qualification')),
              'exact reviewed preparation/prerequisites missing')
    required = {str(HERE / name) for name in RUNTIME_HELPERS} | {str(p.HOLD_SOURCE)}
    required |= {str(path) for path in SERVICE_HARNESS.glob('*.py')}
    required |= {str(RESTORE_HARNESS / name) for name in ('qualify_restore.py','seed_fixture.py','test_qualification.py',
                                                       'restore_verifier.py','reservoir_maintenance_state.py')}
    artifacts = review.get('artifacts', {})
    m.require(required <= set(artifacts) and all(spec['immutable_files'].get(path) == artifacts[path] for path in required),
              'reviewed integrated helper/proof-reader closure incomplete')
    hash_files(artifacts)
    validate_fixture(spec, plan)
    model = reference(spec['original_provider_manifest']); resolution = reference(spec['model_resolution'])
    snapshot = p.load_hold().option(model['argv'], '--model')
    m.require(resolution.get('schema') == 'original_provider_model_resolution_review_v1'
              and resolution.get('accepted') is True and resolution.get('snapshot') == snapshot
              and resolution.get('original_argv') == plan['roles']['provider']['identity']['argv']
              and resolution.get('reviewer') and resolution.get('restart_policy') and resolution.get('evidence'),
              'original model resolution review differs')
    m.require(spec['provider_restart_argv'] == p.pinned_argv(plan['roles']['provider']['identity']['argv'], snapshot),
              'provider substitution differs from reviewed snapshot')
    m.require(file_hash(spec['provider_restart_plist']['path']) == plan['input_hashes']['provider_restart_settings'],
              'reviewed restart plist changed')
    validate_service_matrix(spec, reference(spec['real_matrix']), artifacts)
    validate_restore_result(spec, reference(spec['restore_qualification']), artifacts)
    # Detect proof/helper replacement during the validation interval.
    for value in refs.values(): reference(value)
    hash_files(artifacts)
    return {'schema': 'reservoir_production_admission_v1', 'validated': True,
            'plan_sha256': m.digest(plan), 'preparation_sha256': p.preparation_digest(spec),
            'references': refs, 'provider_restart_argv': spec['provider_restart_argv'],
            'model_snapshot': snapshot, 'reservoir_candidate_sha256': plan['input_hashes']['reservoir_candidate'],
            'provider_restart_settings_sha256': plan['input_hashes']['provider_restart_settings'],
            'integrated_artifacts': {path: artifacts[path] for path in sorted(required)}}
