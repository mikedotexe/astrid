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
import production_restore_admission as restore_admission

HERE = Path(__file__).resolve().parent
SERVICE_HARNESS = HERE.parent.parent / 'reservoir-snapshot-candidate/qualification-harness-v3'
RESTORE_HARNESS = HERE.parent / 'restore-qualification-02'
RUNTIME_HELPERS = ('reservoir_maintenance.py', 'production_admission.py', 'production_restore_admission.py', 'production_fixture_admission.py', 'production_fixture_history.py', 'production_entrypoint.py', 'production_host.py',
                   'production_preflight.py', 'production_recovery.py', 'production_recovery_inspect.py',
                   'production_restore_runner.py', 'production_retained.py', 'transaction_execution.py',
                   'recovery_ledger.py', 'recovery_phases.py', 'production_recovery_driver.py', 'reservoir_maintenance_state.py', 'restore_verifier.py')
SERVICE_MODULES = ('common', 'fixture', 'process_identity', 'result_checks', 'driver', 'review_matrix')
RESTORE_MODULES = ('reservoir_maintenance_state', 'restore_verifier', 'child_admission', 'qualify_restore')
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


def require_restore_reader():
    origin = getattr(restore_admission, '__file__', None)
    m.require(isinstance(origin, str) and Path(origin).resolve() == HERE / 'production_restore_admission.py',
              'restore admission reader loaded from a different source')


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
    history = validate_service_history(spec, review, records, artifacts)
    return {'case_count': 14, 'source_files': source, 'history': history}


def service_monitor_context(root, final, artifacts, loaded):
    """Check run05 against the exact reviewed v2 policy, without sampler execution."""
    admission, q = loaded['child_admission'], loaded['qualify_restore']
    inputs_ref = ref(RESTORE_HARNESS / 'inputs.json')
    restore_admission.bound_file(sys.modules[__name__], inputs_ref, artifacts)
    inputs = reference(inputs_ref)
    policy_inputs = inputs['admission_supervision']
    for value in policy_inputs.values(): restore_admission.bound_file(sys.modules[__name__], value, artifacts)
    review = reference(root['host_source_review'])
    for value in (root['host_source'], root['host_source_review'], review['sampler_source'],
                  review['sampler_binding'], review['future_observer_binding']):
        restore_admission.bound_file(sys.modules[__name__], value, artifacts)
    m.require(review.get('schema') == 'reservoir_matrix_supervision_source_review_v1'
              and review.get('approved_source') is True
              and review.get('status') == 'accepted_new_instance_source_only_not_executed'
              and review.get('reviewer') and review.get('unresolved_source_blockers') == []
              and review.get('source') == root['host_source']
              and review.get('prior_source') == policy_inputs['helper']
              and review.get('prior_source_review') == policy_inputs['review'], 'service supervisor source review differs')
    groups = ('identity_files', 'protected_processes', 'expected_launchd_pids', 'expected_listener_pids', 'coordination')
    maps = [reference(value) for value in (review['future_observer_binding'], review['sampler_binding'], policy_inputs['bindings'])]
    m.require(all(maps[0].get(key) and all(row.get(key) == maps[0][key] for row in maps) for key in groups),
              'service sampler protected binding maps differ')
    host = reference(root['host_admission']); raw = reference(host['raw'])
    host_independent = reference(host['independent_review'])
    m.require(host_independent.get('passed') is True and host_independent.get('raw') == host['raw'],
              'independent service host admission missing')
    binding = {key: final[key] for key in ('token', 'observer_pid', 'started_at', 'identity_start')}
    binding.update(record_path=root['host_final_raw']['path'], source=root['host_source'],
                   source_review=root['host_source_review'], sampler=review['sampler_source'],
                   bindings=review['future_observer_binding'])
    m.require(binding['identity_start'] == raw['production_identity_end']
              and host['input_binding'] == binding['bindings']
              and raw['input_binding_path'] == binding['bindings']['path']
              and raw['input_binding_sha256'] == binding['bindings']['sha256'], 'service initial host identity differs')
    admission.verify_monitor_revision(Path(binding['source']['path']), inputs, binding)
    policy = admission.load_policy(inputs)
    m.require(binding['sampler'] == {'path': str(policy.SOURCE), 'sha256': policy.SOURCE_SHA}, 'service sampler differs')
    restore_admission.completed_monitor(sys.modules[__name__], final, binding, policy, admission)
    return admission, q, binding, policy, host, raw


def validate_service_history(spec, review, records, artifacts):
    require_restore_reader()
    accepted = reference(spec['service_root_acceptance'])
    m.require(accepted.get('schema') == 'reservoir_private_matrix_root_acceptance_v1'
              and accepted.get('accepted_for_private_numpy_only') is True
              and accepted.get('production_qualified') is False and accepted.get('mlx_qualified') is False
              and accepted.get('case_count') == 14 and accepted.get('matrix') == spec['real_matrix'],
              'complete service root acceptance required')
    root = reference(accepted['root_review']); independent = reference(accepted['independent_review'])
    m.require(root.get('schema') == 'reservoir_private_matrix_root_review_v1'
              and independent.get('schema') == 'reservoir_private_matrix_independent_review_v1'
              and all(row.get('passed') is True and row.get('production_qualified') is False
                      and row.get('accepted_for_private_numpy_only') is True and row.get('case_count') == 14
                      and row.get('matrix_review') == spec['real_matrix'] for row in (root, independent))
              and independent.get('root_matrix_review') == accepted['root_review']
              and root.get('host_final_raw') == accepted['final_monitor']
              and independent.get('host_coverage', {}).get('raw') == accepted['final_monitor']
              and root.get('manifest') == independent.get('manifest') == accepted['source'],
              'complete independent service review differs')
    for key in ('source_review', 'cpu_receipt', 'prepared_input_review'): reference(independent[key])
    final = reference(accepted['final_monitor'])
    root_refs, cases = root.get('root_case_reviews', []), independent.get('cases', [])
    m.require(len(root_refs) == len(cases) == len(records) == 14, 'exact service reviewed sequence required')
    with modules(RESTORE_HARNESS, RESTORE_MODULES, artifacts) as loaded:
        admission, q, binding, policy, host, raw = service_monitor_context(root, final, artifacts, loaded)
        prior = None; prior_end = None; previous_coverage = None
        for ordinal, (record, receipt, root_ref, case_review) in enumerate(zip(records, review['receipts'], root_refs, cases), 1):
            case = reference(root_ref); release = reference(case['release'])
            start, end = record['started_wall'], record['completed_wall']
            m.require(case.get('schema') == 'reservoir_private_case_root_review_v1'
                      and case.get('passed') is True and case.get('errors') == []
                      and case.get('production_qualified') is False and case.get('mlx_qualified') is False
                      and case.get('result') == receipt and case.get('actual_start') == start and case.get('actual_end') == end
                      and case.get('manifest') == accepted['source'] and case.get('host_source') == binding['source'],
                      'service root case review differs')
            m.require(finite(start) and finite(end) and start < end and (prior_end is None or prior_end <= start),
                      'service actual intervals overlap')
            output = str(Path(receipt['path']).parent)
            expected = [record['interpreter']['executable'], '-I', '-B', str(SERVICE_HARNESS / 'driver.py'),
                        'run', '--manifest', record['manifest'], '--variant', record['variant'],
                        '--case', record['case'], '--execute-real-cpu', '--output', output]
            m.require(release.get('schema') == 'reservoir_private_case_root_release_v1'
                      and release.get('agent') == 'root' and release.get('production_mutations') == 0
                      and release.get('case_ordinal') == ordinal and release.get('argv') == expected
                      and release.get('variant') == record['variant'] and release.get('case') == record['case']
                      and release.get('output') == output and release.get('manifest_sha256') == record['manifest_sha256']
                      and release.get('driver_pid') == record['client_identity_before']['pid']
                      and release.get('previous_case_review') == prior
                      and release.get('admission_review') == root['host_admission']
                      and release.get('supervisor_source_review') == binding['source_review']
                      and release.get('supervisor_source_sha256') == binding['source']['sha256']
                      and finite(release.get('issued_wall')) and finite(release.get('expires_wall'))
                      and release['issued_wall'] <= start <= release['expires_wall'], 'one-use service root release differs')
            if ordinal == 1:
                q.validate_host_gate(raw, host, host['raw'], now=start, fresh=True)
            launch = reference(release['supervision_snapshot']); coverage = reference(case['host_snapshot'])
            for moment in (release['issued_wall'], start): admission.validate_monitor(launch, binding, policy, now=moment, ready=True)
            admission.validate_monitor(coverage, binding, policy, now=end, ready=False)
            restore_admission.prefix(launch, coverage); restore_admission.prefix(coverage, final)
            if previous_coverage is not None: restore_admission.prefix(previous_coverage, launch)
            m.require(policy.covers_case(coverage, start, end) and policy.covers_case(final, start, end),
                      'service actual interval lacks raw clean coverage')
            m.require(case_review.get('ordinal') == ordinal and case_review.get('result') == receipt
                      and case_review.get('root_review') == root_ref and case_review.get('release') == case['release']
                      and case_review.get('host_snapshot') == case['host_snapshot']
                      and case_review.get('actual_start') == start and case_review.get('actual_end') == end
                      and case_review.get('child_pid') == record['child_pid']
                      and case_review.get('driver_pid') == record['client_identity_before']['pid']
                      and case_review.get('passed_as_expected') is True, 'independent service case differs')
            absent = case.get('owned_processes_absent', {})
            m.require(absent == {str(record['child_pid']): True, str(record['client_identity_before']['pid']): True},
                      'recorded service process closeout differs')
            prior, prior_end, previous_coverage = root_ref, end, coverage
    return {'root_acceptance': spec['service_root_acceptance'], 'final_monitor': accepted['final_monitor']}


def validate_child(child, directory, argv, expected_code, before, after, admission_claim):
    m.require(isinstance(child, dict) and child.get('reaped') is True
              and child.get('returncode') == expected_code and type(child.get('pid')) is int and child['pid'] > 1
              and child.get('argv') == argv and child.get('admission_claim') == admission_claim
              and isinstance(admission_claim, dict) and finite(child.get('started_wall'))
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
                       'scope': 'owned_synthetic_cpu_child', 'signals_permitted': False,
                       'admission_claim': admission_claim}, 'child-start receipt differs')


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
    require_restore_reader()
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
        m.require(release.get('schema') == 'reservoir_restore_qualification_release_v2'
                  and release.get('accepted') is True and bool(release.get('reviewer'))
                  and release.get('scope') == 'synthetic_cpu_fixture_only'
                  and release.get('purpose') == 'release_restore_qualification_cpu'
                  and release.get('run_id') == run_root.name and release.get('inputs') == result['inputs']
                  and release.get('artifacts') == result['artifacts'] and release.get('input_artifacts') == inputs['artifacts']
                  and finite(release.get('expires_wall')) and result['started_wall'] <= release['expires_wall'],
                  'historical restore release differs')
        raw = reference(release['host_gate_raw']); gate = reference(release['host_gate_review'])
        q.validate_host_gate(raw, gate, release['host_gate_raw'], now=result['started_wall'], fresh=True)
        authority = restore_admission.validate_chain(sys.modules[__name__], spec, result, inputs, release,
                                                     run_root, loaded, artifacts)
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
                       0, result['started_wall'], result['completed_wall'], authority['claims']['seed'])
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
                           0 if case == 'complete' else 1, previous, result['completed_wall'], authority['claims'][case])
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
    # The separate exact-final-release reader rejects the legacy fixture schema.
    # Import lazily: missing final fixture implementation remains fail-closed.
    import production_fixture_admission
    origin = getattr(production_fixture_admission, '__file__', None)
    m.require(isinstance(origin, str) and Path(origin).resolve() == HERE / 'production_fixture_admission.py',
              'final fixture reader loaded from a different source')
    return production_fixture_admission.validate_fixture(spec, plan)


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
              and all(review.get(name) == spec[name] for name in ('fixture','fixture_review','real_matrix','restore_qualification',
                                                               'service_root_acceptance','restore_root_acceptance')),
              'exact reviewed preparation/prerequisites missing')
    required = {str(HERE / name) for name in RUNTIME_HELPERS} | {str(p.HOLD_SOURCE)}
    required |= {str(path) for path in SERVICE_HARNESS.glob('*.py')}
    required |= {str(RESTORE_HARNESS / name) for name in ('qualify_restore.py','seed_fixture.py','test_qualification.py',
                                                       'restore_verifier.py','reservoir_maintenance_state.py','child_admission.py','test_child_admission.py')}
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
    for value in list(refs.values()) + [spec['service_root_acceptance'], spec['restore_root_acceptance']]: reference(value)
    hash_files(artifacts)
    return {'schema': 'reservoir_production_admission_v1', 'validated': True,
            'plan_sha256': m.digest(plan), 'preparation_sha256': p.preparation_digest(spec),
            'references': refs, 'provider_restart_argv': spec['provider_restart_argv'],
            'model_snapshot': snapshot, 'reservoir_candidate_sha256': plan['input_hashes']['reservoir_candidate'],
            'provider_restart_settings_sha256': plan['input_hashes']['provider_restart_settings'],
            'integrated_artifacts': {path: artifacts[path] for path in sorted(required)}}
