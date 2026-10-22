"""Read-only historical restore02 admission evidence; no execution or present-time waiver.

The numerical oracles remain in the frozen qualifier/verifier. This reader binds
all ten children to their original one-use permits, immutable launch snapshots,
raw continuous coverage and completed independent acceptance. Imports do no work.
"""
from __future__ import annotations

from pathlib import Path

import reservoir_maintenance as m


def bound_file(api, value, artifacts):
    m.require(isinstance(value, dict) and set(value) == {'path', 'sha256'}
              and artifacts.get(value['path']) == value['sha256']
              and api.file_hash(value['path']) == value['sha256'], 'unbound numerical reader dependency')


def prefix(earlier, later):
    keys = ('schema', 'swap_policy', 'token', 'observer_pid', 'started_at', 'identity_start',
            'self_sha256', 'source_sha256', 'initial_swap_bytes')
    m.require(all(earlier.get(key) == later.get(key) for key in keys)
              and isinstance(earlier.get('samples'), list) and len(earlier['samples']) >= 2
              and later.get('samples', [])[:len(earlier['samples'])] == earlier['samples'],
              'supervisor immutable history is not an exact final prefix')


def completed_monitor(api, record, binding, policy, admission):
    admission.validate_monitor(record, binding, policy, now=0, ready=False)
    m.require(record.get('state') == 'completed' and record.get('completion_reason') == 'owned_finish_request'
              and record.get('identity_end') == binding['identity_start']
              and record.get('invalidated') is False and len(record.get('samples', [])) >= 2,
              'completed clean supervisor required')
    rows = record['samples']
    m.require(policy.covers_case(record, rows[0]['t_s'], rows[-1]['t_s']),
              'completed raw supervisor policy failed')


def validate_reviews(api, spec, result):
    accepted = api.reference(spec['restore_root_acceptance'])
    m.require(accepted.get('schema') == 'reservoir_restore_root_numeric_acceptance_v1'
              and accepted.get('accepted') is True and accepted.get('reviewer') == 'root'
              and accepted.get('scope') == 'synthetic_cpu_fixture_only'
              and accepted.get('production_qualified') is False
              and accepted.get('live_persistence_qualified') is False
              and accepted.get('mlx_upgrade_qualified') is False
              and accepted.get('seed_plus_nine_cases_accepted') is True
              and accepted.get('all_children_absent') is True
              and accepted.get('qualification') == spec['restore_qualification'],
              'complete root restore acceptance required')
    root = api.reference(accepted['root_review']); independent = api.reference(accepted['independent_review'])
    m.require(root.get('schema') == 'reservoir_restore_numeric_root_review_v1'
              and root.get('passed') is True and root.get('production_qualified') is False
              and root.get('qualification') == spec['restore_qualification']
              and root.get('release') == result['release']
              and root.get('final_supervision') == accepted['final_supervision'], 'root restore review differs')
    m.require(independent.get('schema') == 'reservoir_restore_qualification_independent_numerical_review_v1'
              and independent.get('state') == 'accepted_completed_synthetic_cpu_matrix'
              and independent.get('passed') is True and independent.get('production_qualified') is False
              and independent.get('mlx_qualified') is False and independent.get('reviewer')
              and independent.get('qualification') == spec['restore_qualification']
              and independent.get('release') == result['release']
              and independent.get('case_count') == 9 and independent.get('seed_plus_child_count') == 10
              and independent.get('continuous_host', {}).get('record') == accepted['final_supervision'],
              'complete independent restore review required')
    # Reviews attest raw numerical inspection. The reader below separately checks
    # raw authority/coverage and the caller separately reruns every frozen oracle.
    for key in ('source_review', 'partial_positive_review', 'root_preclose', 'root_launch'):
        api.reference(independent[key])
    api.reference(accepted['host_admission'])
    m.require(root.get('host_admission') == accepted['host_admission'], 'restore host admission review differs')
    return accepted, root, independent


def validate_chain(api, spec, result, inputs, release, run_root, loaded, artifacts):
    """Validate retained history at recorded times; never read mutable monitor_path."""
    admission = loaded['child_admission']
    for value in inputs['admission_supervision'].values():
        bound_file(api, value, artifacts)
    binding = release['supervision']
    for key in ('source', 'source_review', 'sampler', 'bindings'):
        bound_file(api, binding[key], artifacts)
    source_review = api.reference(binding['source_review'])
    bound_file(api, source_review['sampler_binding'], artifacts)
    admission.validate_release_supervision(release, inputs)
    policy = admission.load_policy(inputs)  # Pure policy functions only, never sampler/observe/main.
    accepted, root_review, independent = validate_reviews(api, spec, result)
    final = api.reference(accepted['final_supervision'])
    completed_monitor(api, final, binding, policy, admission)
    m.require(accepted['final_supervision']['path'] == binding['record_path']
              and result['final_supervision']['path'] == str(run_root / 'final-supervision.json'),
              'restore final monitor paths differ')
    producer_final = api.reference(result['final_supervision'])
    admission.validate_monitor(producer_final, binding, policy, now=result['completed_wall'], ready=True)
    prefix(producer_final, final)
    m.require(independent['continuous_host'].get('source') == binding['source']
              and independent['continuous_host'].get('source_review') == binding['source_review']
              and independent['continuous_host'].get('bindings') == binding['bindings']
              and independent['continuous_host'].get('sampler') == binding['sampler'],
              'independent restore supervisor binding differs')
    cases = ('seed',) + api.RESTORE_CASES
    refs = result.get('child_acceptances', [])
    reviews = independent.get('cases', [])
    root_refs = root_review.get('root_child_reviews', [])
    m.require(len(refs) == len(reviews) == len(root_refs) == 10
              and len({row['path'] for row in refs}) == 10
              and [row.get('case') for row in reviews] == list(cases), 'exact ten ordered admission records required')
    expected_files = {f'{index:02d}-{case}-{suffix}.json' for index, case in enumerate(cases)
                      for suffix in ('request','permit','launch-claim','coverage','accepted','root-review','root-supervisor')}
    m.require({path.name for path in (run_root / 'admissions').iterdir()} == expected_files,
              'unknown or missing per-child admission artifact')
    prior_ref = None; prior_coverage = None; prior_end = result['started_wall']; claims = {}
    for ordinal, (case, acceptance_ref, reviewed, root_ref) in enumerate(zip(cases, refs, reviews, root_refs)):
        directory = run_root / case; stem = f'{ordinal:02d}-{case}'
        path = run_root / 'admissions' / (stem + '-accepted.json')
        m.require(acceptance_ref['path'] == str(path), 'admission ordinal/path differs')
        acceptance, child = admission.validate_acceptance(acceptance_ref)
        m.require(acceptance.get('schema') == 'reservoir_restore_child_acceptance_v1'
                  and acceptance.get('production_qualified') is False
                  and acceptance.get('run_release') == result['release'], 'child acceptance envelope differs')
        expected_child = result['seed_child'] if ordinal == 0 else result['cases'][ordinal - 1]['child']
        expected_result = ({'case': 'seed', 'passed': True, 'seed': result['seed'], 'child': child}
                           if ordinal == 0 else result['cases'][ordinal - 1])
        m.require(child == expected_child and acceptance['child_exit']['path'] == str(directory / 'child-exit.json')
                  and acceptance['result']['path'] == str(directory / 'expected-result.json')
                  and api.reference(acceptance['result']) == expected_result, 'accepted raw child result differs')
        for key, suffix in (('request', 'request'), ('permit', 'permit'), ('claim', 'launch-claim'), ('coverage', 'coverage')):
            m.require(acceptance[key]['path'] == str(run_root / 'admissions' / (stem + '-' + suffix + '.json')),
                      'admission component path differs')
        waiting = api.reference(acceptance['request']); permit = api.reference(acceptance['permit'])
        claim = api.reference(acceptance['claim']); coverage = api.reference(acceptance['coverage'])
        private_request = None if ordinal == 0 else result['cases'][ordinal - 1]['request']
        m.require(waiting == {'schema': 'reservoir_restore_child_request_v1', 'run_id': run_root.name,
                    'ordinal': ordinal, 'case': case, 'argv': child['argv'], 'directory': str(directory),
                    'request': private_request, 'inputs': result['inputs'], 'run_release': result['release'],
                    'artifacts': result['artifacts'], 'input_artifacts': inputs['artifacts'],
                    'supervision': binding, 'previous_acceptance': prior_ref, 'production_qualified': False},
                  'waiting request/previous acceptance/source/argv differs')
        m.require(permit.get('schema') == 'reservoir_restore_child_permit_v1'
                  and permit.get('accepted') is True and permit.get('reviewer') == 'root'
                  and permit.get('request') == acceptance['request'] and permit.get('run_release') == result['release']
                  and permit.get('previous_acceptance') == prior_ref, 'one-use root permit chain differs')
        m.require(claim.get('schema') == 'reservoir_restore_child_launch_claim_v1'
                  and claim.get('request') == acceptance['request'] and claim.get('permit') == acceptance['permit']
                  and claim.get('run_release') == result['release'] and claim.get('production_qualified') is False
                  and claim.get('automatic_retry_permitted') is False
                  and child.get('admission_claim') == acceptance['claim'], 'launch claim/child binding differs')
        stamp, start, end, expires = claim.get('claimed_wall'), child.get('started_wall'), child.get('completed_wall'), permit.get('expires_wall')
        m.require(all(api.finite(value) for value in (stamp, start, end, expires, release.get('expires_wall')))
                  and prior_end <= stamp <= start < end <= result['completed_wall']
                  and start <= expires <= release['expires_wall'], 'historical child timing/permit expiry differs')
        # The mutable path is only a recorded name. Resolve its digest to exact
        # retained bytes; absent correspondence is an error, never invented history.
        snapshot_ref = permit['supervisor_snapshot']
        m.require(snapshot_ref['path'] == str(run_root / 'admissions' / (stem + '-root-supervisor.json')),
                  'launch snapshot is not the retained ordinal snapshot')
        snapshot = api.reference(snapshot_ref)
        m.require(claim.get('supervisor_observed') == {'path': binding['record_path'], 'sha256': snapshot_ref['sha256']},
                  'claim monitor digest has no exact retained immutable snapshot')
        for moment in (stamp, start):
            admission.validate_monitor(snapshot, binding, policy, now=moment, ready=True)
        prefix(snapshot, coverage); prefix(coverage, producer_final)
        admission.validate_monitor(coverage, binding, policy, now=end, ready=False)
        m.require(policy.covers_case(coverage, start, end) and policy.covers_case(final, start, end),
                  'actual child interval lacks clean continuous coverage')
        if prior_coverage is not None:
            prefix(prior_coverage, snapshot)
            m.require(policy.covers_case(snapshot, previous_child['started_wall'], previous_child['completed_wall']),
                      'next child lacked accepted previous interval')
        root_case = api.reference(root_ref)
        m.require(reviewed.get('ordinal') == ordinal and reviewed.get('acceptance') == acceptance_ref
                  and reviewed.get('launch_claim') == acceptance['claim'] and reviewed.get('permit') == acceptance['permit']
                  and reviewed.get('coverage') == acceptance['coverage'] and reviewed.get('result') == acceptance['result']
                  and reviewed.get('root_review') == root_ref and reviewed.get('passed') is True
                  and reviewed.get('pid') == child['pid'] and reviewed.get('started_wall') == start
                  and reviewed.get('completed_wall') == end and reviewed.get('returncode') == child['returncode'],
                  'independent child review differs from raw chain')
        m.require(root_case.get('schema') == 'reservoir_restore_child_root_review_v1'
                  and root_case.get('passed') is True and root_case.get('production_qualified') is False
                  and root_case.get('case') == case and root_case.get('ordinal') == ordinal
                  and root_case.get('child_absent') is True and root_case.get('child_exit') == acceptance['child_exit']
                  and root_case.get('coverage') == acceptance['coverage'] and root_case.get('result') == acceptance['result']
                  and root_case.get('run_release') == result['release'] and root_case.get('acceptance') == acceptance_ref,
                  'root child review differs from accepted chain')
        claims[case] = acceptance['claim']
        prior_ref, prior_coverage, prior_end, previous_child = acceptance_ref, coverage, end, child
    return {'claims': claims, 'final_supervision': accepted['final_supervision'],
            'root_acceptance': spec['restore_root_acceptance'], 'child_count': 10}
