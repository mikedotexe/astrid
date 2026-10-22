"""Model-free tests: explicit retrospective assessment never erases strict failure."""
from copy import deepcopy
import json
from pathlib import Path
import pytest
import model_qualification as q
from astrid_soak_performance import compare_performance
from test_model_qualification import evidence, check, change_reference, write, iso


def actual_pair():
    return {role: q.load_reference(ref) for role, ref in q.PRACTICAL_PAIR.items()}


def test_actual_measurements_have_two_strict_failures_and_pass_caps_without_mutation():
    pair = actual_pair()
    original = q.load_reference(q.PRACTICAL_COMPARISON)
    before = deepcopy(original)
    published = compare_performance(pair['baseline'], pair['candidate'])
    assert published == original['comparison'] and not published['ok']
    assert {(r['workload'], r['metric']) for r in published['regressions']} == {
        ('exact', 'p95_total_turn_s'), ('next_contract', 'p95_first_token_s')}
    alternative = q.practical_assessment(published)
    assert alternative['ok'] and alternative['published_ok'] is False
    assert original == before and published == before['comparison']
    for role, record in pair.items():
        q.verify_memory(record)
        assert record['state'] == 'completed' and record['summary']['automated_ok']
    assert q.load_reference(q.PRACTICAL_COMPARISON) == before


@pytest.mark.parametrize('metric,cap', [('p95_total_turn_s',5.),('p95_first_token_s',2.)])
def test_absolute_boundary_and_relative_boundary_are_distinct(metric, cap):
    pair = actual_pair()
    base, candidate = deepcopy(pair['baseline']), deepcopy(pair['candidate'])
    base['matched_workloads']['exact'][metric] = cap - .1
    candidate['matched_workloads']['exact'][metric] = cap
    assert q.practical_assessment(compare_performance(base,candidate))['ok']
    candidate['matched_workloads']['exact'][metric] = cap + .01
    assert not q.practical_assessment(compare_performance(base,candidate))['ok']
    base['matched_workloads']['exact'][metric] = cap
    candidate['matched_workloads']['exact'][metric] = cap * 1.2
    assert q.practical_assessment(compare_performance(base,candidate))['ok']
    candidate['matched_workloads']['exact'][metric] = cap * 1.201
    assert not q.practical_assessment(compare_performance(base,candidate))['ok']


@pytest.mark.parametrize('change', [
    lambda b,c: c['matched_workloads']['long_context'].update(p95_total_turn_s=1000.),
    lambda b,c: c['matched_workloads']['dialogue'].update(steady_tok_s=.01),
    lambda b,c: c['matched_workloads']['dialogue'].update(peak_memory_bytes=1e12),
    lambda b,c: c['matched_workloads']['dialogue'].update(count=2),
    lambda b,c: c['matched_workloads']['exact'].update(messages_sha256='x'),
    lambda b,c: c['matched_workloads']['exact'].update(completion_tokens=[1,1,1]),
    lambda b,c: c['matched_workloads']['exact'].update(p95_total_turn_s=None),
])
def test_practical_caps_do_not_waive_other_failures(change):
    pair=actual_pair();b,c=deepcopy(pair['baseline']),deepcopy(pair['candidate'])
    change(b,c)
    assert not q.practical_assessment(compare_performance(b,c))['ok']


def add_review(evidence, monkeypatch):
    proof=evidence['proof'];refs=proof['artifacts']
    proof['performance_attempts'][0]['candidate']=evidence['make_benchmark']('candidate',8200,total=2.)
    evidence['save_review']()
    pair=deepcopy(proof['performance_attempts'][0])
    records={role:q.load_reference(ref) for role,ref in pair.items()}
    comparison=compare_performance(records['baseline'],records['candidate'])
    assert not comparison['ok']
    original=write(evidence['root']/'original-comparison.json',{
        'baseline':pair['baseline'],'core':pair['candidate'],'at_utc':iso(8400),
        'decision':'FAILED_PUBLISHED_COMPARATIVE_PERFORMANCE','comparison':comparison})
    inputs={name:refs[name]['sha256'] for name in q.PRACTICAL_INPUTS}
    monkeypatch.setattr(q,'PRACTICAL_PAIR',pair)
    monkeypatch.setattr(q,'PRACTICAL_COMPARISON',original)
    monkeypatch.setattr(q,'PRACTICAL_INPUTS',inputs)
    review={'schema':'mlx_practical_performance_review_v1','accepted':True,'policy':q.PRACTICAL_POLICY,
        'after_the_fact_reinterpretation':True,'published_failure_preserved':True,
        'reviewer_kind':'agent','reviewer':'Synthetic test reviewer','reviewed_at':iso(8500),
        'rationale':'Synthetic only: exact after-measurement policy validation.',
        'limitations':['Three samples; no claim of a statistically stable improvement.'],
        'benchmark_refs':pair,'input_hashes':inputs,'absolute_caps_s':q.PRACTICAL_CAPS,
        'published_comparison':original,'reader_hashes':q.practical_reader_hashes(),
        'assessment':q.practical_assessment(comparison)}
    refs['performance_policy_review']=write(evidence['root']/'practical-review.json',review)
    return refs['performance_policy_review']


def test_full_proof_and_activation_reader_preserve_failed_comparison(evidence,monkeypatch):
    ref=add_review(evidence,monkeypatch)
    result=check(evidence)
    assert result['accepted'] and result['published_performance_passed'] is False
    assert result['performance'][0]['ok'] is False and result['alternative_performance']['ok'] is True
    assert result['reverse_retest_accepted'] is False
    path=evidence['root']/'proof.json';write(path,evidence['proof'])
    assert q.verify_proof(path,evidence['paths']['candidate'],evidence['paths']['baseline'])['accepted']
    del evidence['proof']['artifacts']['performance_policy_review']
    with pytest.raises(ValueError,match='thresholds failed'):check(evidence)


@pytest.mark.parametrize('edit', [
    lambda r:r.update(accepted=False),lambda r:r.update(after_the_fact_reinterpretation=False),
    lambda r:r.update(published_failure_preserved=False),lambda r:r.update(limitations=[]),
    lambda r:r.update(reviewed_at=iso(8000)),lambda r:r.update(policy='generic'),
    lambda r:r['reader_hashes'].update({'model_qualification.py':'a'*64}),
    lambda r:r['absolute_caps_s'].update(p95_total_turn_s=10.),
    lambda r:r['benchmark_refs']['candidate'].update(sha256='f'*64),
    lambda r:r['assessment'].update(ok=False),
])
def test_review_is_explicit_and_exact(evidence,monkeypatch,edit):
    ref=add_review(evidence,monkeypatch);change_reference(ref,edit)
    with pytest.raises(ValueError):check(evidence)


def test_foreign_pair_and_stage2_cannot_use_exact_exception(evidence,monkeypatch):
    ref=add_review(evidence,monkeypatch)
    pair=evidence['proof']['performance_attempts'][0]
    records={role:q.load_reference(r) for role,r in pair.items()}
    args=(q.load_reference(ref), 'full', [{'refs':pair,'records':records}],
          evidence['proof']['artifacts'], [compare_performance(records['baseline'],records['candidate'])])
    with pytest.raises(ValueError,match='exact campaign04'):q.verify_practical_review(*args)
    monkeypatch.setattr(q,'PRACTICAL_PAIR',{'baseline':pair['baseline'],'candidate':{'path':'/other','sha256':'a'*64}})
    with pytest.raises(ValueError,match='exact campaign04'):check(evidence)


def test_output_quality_review_and_failed_receipts_still_block(evidence,monkeypatch):
    add_review(evidence,monkeypatch)
    change_reference(evidence['proof']['artifacts']['output_review'],lambda r:r.update(accepted=False))
    with pytest.raises(ValueError,match='output review'):check(evidence)
    evidence['save_review']()
    change_reference(evidence['proof']['performance_attempts'][0]['candidate'],lambda r:r.update(state='failed'))
    with pytest.raises(ValueError,match='incomplete or failed'):check(evidence)
