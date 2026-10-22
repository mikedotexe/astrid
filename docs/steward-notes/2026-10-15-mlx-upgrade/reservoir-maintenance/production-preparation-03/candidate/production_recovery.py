"""Pure, nonexecuting recovery proposals for the fixed maintenance transaction.

Consumes a retained inspector report, never a path/Host. No CLI, file reads,
process observations, numeric imports or execution methods exist here. A proposal
is not authorization; a later separately reviewed executor would need current
ownership, witness semantics and a durable single-use claim before any action.
"""
from __future__ import annotations

import copy
import hashlib
import json


class Refusal(ValueError):
    pass


def require(condition, message):
    if not condition:
        raise Refusal(message)


def digest(value):
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(',', ':'),
                                    allow_nan=False).encode()).hexdigest()


ROLES = ('provider', 'reservoir')
HOST_PAIRS = {'hold_intent': 'hold_created', 'debug_intent': 'debug_result',
              'signal_intent': 'signal_sent', 'held_bootout_intent': 'held_job_absent',
              'install_intent': 'install_completed',
              'provider_snapshot_plist_intent': 'provider_snapshot_plist_installed',
              'bootstrap_intent': 'bootstrap_ready'}


def schedule():
    """Exact ordered controller03 durable steps; no recovery-only mutation."""
    steps = [('preflight', None, None)]
    for role, witness in (('provider', 'provider_drain'), ('reservoir', 'final_snapshot')):
        for op in ('create_hold', 'arm_next_spawn', 'signal_stop'):
            steps += [('mutation_intent', op, role), ('mutation_completed', op, role)]
        steps += [('held_verified', None, role), ('witness', witness, None),
                  ('mutation_intent', 'remove_held', role),
                  ('mutation_completed', 'remove_held', role), ('job_absent', None, role)]
    steps += [('witness', 'backup', None), ('witness', 'private_restore', None)]
    for op, role, after in (
            ('install_repair', None, None), ('bootstrap_original', 'reservoir', 'startup'),
            ('apply_provider_restart', 'provider', None),
            ('bootstrap_original', 'provider', 'provider_ready'),
            ('release_hold', 'provider', None), ('release_hold', 'reservoir', None)):
        steps += [('mutation_intent', op, role), ('mutation_completed', op, role)]
        if after:
            steps.append(('witness', after, None))
    return steps + [('completed', None, None)]


def controller_state(plan, record):
    events = record['events']; expected = schedule(); previous = digest(plan)
    completed = {}; receipts = {}; holders = {}; pending = None; failures = []
    index = 0
    for sequence, event in enumerate(events, 1):
        require(event.get('sequence') == sequence and event.get('previous_sha256') == previous
                and event.get('plan_sha256') == digest(plan), 'controller chain differs')
        previous = digest(event)
        kind = event.get('type')
        if kind == 'failed':
            require(sequence == len(events), 'events after terminal failure are unsupported')
            failures.append(copy.deepcopy(event)); continue
        require(index < len(expected), 'events after completed transaction')
        actual = (kind, event.get('operation') if kind.startswith('mutation_')
                  else event.get('kind') if kind == 'witness' else None,
                  event.get('role') if kind != 'witness' else None)
        require(actual == expected[index], 'controller event is not the exact finite run prefix')
        index += 1
        if kind == 'mutation_intent':
            pending = (event['operation'], event.get('role'))
        elif kind == 'mutation_completed':
            require(pending == (event['operation'], event.get('role')), 'completion without intent')
            require(isinstance(event.get('result'), dict), 'completion result missing')
            completed[pending] = copy.deepcopy(event['result']); pending = None
        elif kind == 'witness':
            receipt = event['receipt']; name = event['kind']
            require(receipt.get('passed') is True and receipt.get('kind') == name
                    and receipt.get('transaction_id') == plan['transaction_id'], 'invalid retained witness')
            bindings = receipt.get('bindings', {})
            required = {'plan_sha256': digest(plan), **{
                r + '_old_identity_sha256': digest(plan['roles'][r]['identity']) for r in ROLES}}
            if name == 'backup': required['final_snapshot_sha256'] = digest(receipts['final_snapshot'])
            if name == 'private_restore':
                required.update(backup_sha256=digest(receipts['backup']),
                                repair_source_sha256=plan['input_hashes']['reservoir_candidate'],
                                interpreter_sha256=plan['input_hashes']['interpreter'],
                                configuration_sha256=plan['input_hashes']['reservoir_settings'])
            if name == 'startup':
                required.update(private_restore_sha256=digest(receipts['private_restore']),
                                reservoir_new_identity_sha256=digest(completed[('bootstrap_original', 'reservoir')]['identity']))
            if name == 'provider_ready':
                required.update(startup_sha256=digest(receipts['startup']),
                                provider_new_identity_sha256=digest(completed[('bootstrap_original', 'provider')]['identity']))
            require(all(bindings.get(k) == v for k, v in required.items()), 'retained witness chain differs')
            receipts[name] = copy.deepcopy(receipt)
        elif kind == 'held_verified':
            holders[event['role']] = copy.deepcopy(event['identity'])
    require(record.get('last_sha256') == previous, 'controller head differs')
    require(record.get('pending') in (pending, list(pending) if pending else None), 'controller pending summary differs')
    parked = {r for r in ROLES if ('remove_held', r) in completed and ('bootstrap_original', r) not in completed}
    return {'completed': completed, 'receipts': receipts, 'holders': holders, 'pending': pending,
            'failures': failures, 'prefix_length': index, 'parked': parked,
            'repaired': ('install_repair', None) in completed,
            'provider_pinned': ('apply_provider_restart', 'provider') in completed,
            'replacements': {r: completed[('bootstrap_original', r)]['identity'] for r in ROLES
                             if ('bootstrap_original', r) in completed}}


def host_state(record):
    pairs = {}; pending = None; releases = set()
    for row in record['events']:
        op = row['operation']; role = row.get('role')
        if op.startswith('provider_snapshot_plist_'): role = 'provider'
        if op in HOST_PAIRS:
            key = (op, role)
            require(pending is None and key not in pairs, 'overlapping/repeated host intent')
            pairs[key] = {'intent': copy.deepcopy(row), 'result': None}; pending = key
        elif op in HOST_PAIRS.values():
            require(pending is not None and HOST_PAIRS[pending[0]] == op and pending[1] == role,
                    'host result without exact intent')
            pairs[pending]['result'] = copy.deepcopy(row); pending = None
        elif op == 'hold_released':
            require(role in ROLES and role not in releases, 'duplicate/unknown hold release')
            releases.add(role)
        else:
            raise Refusal('unsupported host operation')
    require(record.get('pending') in (pending, list(pending) if pending else None), 'host pending summary differs')
    return pairs, pending


def reconstruct(preparation, inspection):
    """Return JSON-only reconstruction. Invalid evidence raises; ambiguity blocks.

    The inspector remains the file/ownership observer. This function independently
    checks the exact run prefix and retains its original offsets without reading
    current files. It cannot establish that the observation is still current.
    """
    spec = copy.deepcopy(preparation); report = copy.deepcopy(inspection); plan = spec['plan']
    projection = {k: v for k, v in spec.items() if k != 'execution_review'}
    require(report.get('schema') == 'reservoir_production_recovery_inspection_v1'
            and report.get('evidence_valid') is True and report.get('errors') == [], 'inspection is invalid')
    require(report.get('transaction_id') == plan['transaction_id']
            and report.get('preparation_sha256') == digest(projection), 'preparation binding differs')
    require(report.get('owner') == {'schema': 'reservoir_production_owner_v1',
            'transaction_id': plan['transaction_id'], 'actor': plan['actor'],
            'host_binding': plan['host_binding'], 'preparation_sha256': digest(projection)}, 'owner differs')
    before = report['observed_before']; after = report['observed_after']
    reasons = list(report['unresolved'])
    if before != after: reasons.append('inspection changed; no current phase inferred')
    if before['host'] != plan['host_binding'] or before['protected'] != plan['protected']:
        reasons.append('host/protected identities differ')
    if before['stewardship'] != {'paused': True, 'pause_generation': plan['pause_generation'],
                                'pause_actor': plan['actor'], 'lease': None}:
        reasons.append('stewardship differs')
    control = controller_state(plan, report['controller']); pairs, pending = host_state(report['host_operations'])
    if control['pending']: reasons.append('controller command outcome unresolved')
    if pending: reasons.append('host command outcome unresolved')
    guard = {'schema': 'model_environment_hold_v1', 'transaction_id': plan['transaction_id'], 'actor': plan['actor']}
    completed = control['completed']; anchors = report['capture_anchors']
    require(set(anchors) == set(ROLES), 'original capture anchor missing')
    for role in ROLES:
        require(anchors[role]['identity'] == plan['roles'][role]['identity'], 'anchor identity differs')
        for key in ('log', 'qos') if role == 'provider' else ('log',):
            cur = anchors[role][key]
            require(type(cur.get('start_offset')) is int and cur['start_offset'] >= 0
                    and all(k in cur for k in ('path', 'device', 'inode', 'prefix_sha256')), 'original cursor incomplete')
        if role == 'provider':
            require(anchors[role].get('start_worker_idle') is True and anchors[role].get('start_queue_empty') is True,
                    'positive idle anchor absent')
        else:
            require(anchors[role].get('inventory_matches_start_boundary') is True, 'inventory boundary absent')
        row = report['roles'][role]; view = row['observed']; old = plan['roles'][role]['identity']
        require(view == before['roles'][role], 'role observation summary differs')
        classification = row['classification']; process = view.get('process')
        if classification == 'original_identity_observed':
            require(process == old and view['listener_pids'] == [old['pid']] and view['job_loaded'], 'original identity differs')
        elif classification == 'owned_holder_observed':
            held = row.get('held_receipt', {})
            require(process == control['holders'].get(role), 'holder lacks durable controller verification')
            require(row.get('guard') == guard and held.get('hold') == guard
                    and held.get('pid') == process['pid'] and held.get('started_at') == process['started_at']
                    and held.get('hold_only') is True and held.get('model_imported') is False
                    and held.get('listener_opened') is False and view['job_loaded'] is True
                    and view['listener_pids'] == [] and row.get('all_tcp_listener_pids') == [], 'owned holder differs')
            require(row['old_pid_observation'] is None, 'old process survives or PID was reused')
        elif classification == 'absent_observed':
            require(role in control['parked'] and process is None and view['job_loaded'] is False
                    and view['listener_pids'] == [] and row['old_pid_observation'] is None, 'absence is not a confirmed parked state')
        elif classification != 'recorded_replacement_observed':
            reasons.append(role + ': unsupported or foreign process phase')
        created = ('create_hold', role) in completed; released = ('release_hold', role) in completed
        require(not created or released or row.get('guard') == guard, 'created guard disappeared')
    for (op, role), pair in pairs.items():
        result = pair['result']
        if op == 'debug_intent' and result is not None and result.get('returncode') != 0:
            reasons.append('nonzero debug is not proven pre-command auth refusal; no re-arm')
        if op == 'signal_intent':
            boundary = pair['intent'].get('wall_time_ns')
            require(type(boundary) is int and boundary > 0
                    and report['signal_wall_time_ns'].get(role) == boundary, 'signal boundary cannot be reconstructed')
    variants = report['installed_variants']
    for role in ROLES:
        expected_source = 'candidate' if role == 'reservoir' and control['repaired'] else 'original'
        expected_plist = 'pinned' if role == 'provider' and control['provider_pinned'] else 'original'
        if (variants[role]['source'], variants[role]['plist']) != (expected_source, expected_plist):
            reasons.append(role + ': disk phase differs from durable completed phase')
    restore = report.get('private_restore_execution')
    if restore is None:
        reasons.append('private-restore launch absence was not inspected')
    elif restore.get('status') != 'not_started':
        # Even a clean exit needs the outer witness and live semantic validation.
        reasons.append('private restore has started; this finite planner cannot resume/relaunch it')
    if 'private_restore' in control['receipts']:
        reasons.append('private restore witness present; later phases are outside this planner')
    classifications = {r: report['roles'][r]['classification'] for r in ROLES}
    phase = 'unsupported'; action = None
    if classifications == {'provider': 'owned_holder_observed', 'reservoir': 'original_identity_observed'}:
        phase = 'provider_held'
        action = 'revalidate_provider_drain' if 'provider_drain' not in control['receipts'] else 'revalidate_then_remove_verified_provider_holder'
    elif classifications == {'provider': 'absent_observed', 'reservoir': 'owned_holder_observed'}:
        phase = 'reservoir_held'
        action = 'revalidate_final_snapshot' if 'final_snapshot' not in control['receipts'] else 'revalidate_then_remove_verified_reservoir_holder'
    elif classifications == {'provider': 'absent_observed', 'reservoir': 'absent_observed'} and not control['repaired']:
        phase = 'both_parked'; action = 'revalidate_stopped_inventory_and_backup'
    else:
        reasons.append('phase outside supported held/parked checkpoints; no command proposed')
    if control['repaired'] or control['provider_pinned'] or control['replacements']:
        reasons.append('install/plist/bootstrap recovery is intentionally unsupported')
    host = {'counter': len(report['host_operations']['events']), 'guards': {r: report['roles'][r].get('guard') for r in ROLES},
            'anchors': anchors, 'receipts': control['receipts'], 'signal_times': report['signal_wall_time_ns'],
            'arm_attempts': sorted(r for k, r in pairs if k == 'debug_intent'),
            'armed_identities': {r: pair['intent']['identity'] for (k, r), pair in pairs.items()
                                 if k == 'debug_intent' and pair['result'] and pair['result']['returncode'] == 0},
            'signalled_identities': {r: pair['intent']['identity'] for (k, r), pair in pairs.items()
                                    if k == 'signal_intent' and pair['result']},
            'startups': {r: pair['intent']['log_anchor'] for (k, r), pair in pairs.items() if k == 'bootstrap_intent'},
            'repaired': control['repaired'], 'provider_pinned': control['provider_pinned']}
    return {'schema': 'reservoir_production_recovery_state_v1', 'transaction_id': plan['transaction_id'],
            'preparation_sha256': digest(projection), 'plan_sha256': digest(plan), 'inspection_sha256': digest(report),
            'controller_head_sha256': report['controller']['last_sha256'],
            'host_events_sha256': digest(report['host_operations']['events']),
            'capture_anchors_sha256': digest(anchors), 'artifacts_sha256': digest(report['artifacts']),
            'phase': phase, 'next_operation': action, 'blocked_reasons': sorted(set(reasons)),
            'host_state': host, 'controller_state': {
                'event_count': len(report['controller']['events']), 'prefix_length': control['prefix_length'],
                'receipts': control['receipts'], 'parked': sorted(control['parked']),
                'replacements': control['replacements'], 'repaired': control['repaired'],
                'provider_pinned': control['provider_pinned'], 'failures': control['failures']},
            'execution_available': False, 'production_recovery_qualified': False,
            'authority': 'Retained evidence reconstruction only; not a Host instance or a resume authorization.'}


def plan_resume(state, *, consumed_tokens=()):
    """One-step proposal with a single-use identity, never an executable action.

    Caller-supplied consumed tokens prevent repeated proposal acceptance in fake
    tests. They are not a durable execution ledger. No token may authorize replay.
    """
    require(state.get('schema') == 'reservoir_production_recovery_state_v1'
            and state.get('execution_available') is False, 'invalid reconstruction')
    require(not state['blocked_reasons'] and state['next_operation'], 'recovery planning blocked: ' + '; '.join(state['blocked_reasons']))
    token = digest({'state_sha256': digest(state), 'operation': state['next_operation']})
    require(token not in consumed_tokens, 'proposal already consumed; no replay')
    return {'schema': 'reservoir_production_recovery_proposal_v1', 'transaction_id': state['transaction_id'],
            'state_sha256': digest(state), 'single_use_token': token, 'operation': state['next_operation'],
            'phase': state['phase'], 'execution_available': False, 'retry_authorized': False,
            'required_before_execution': [
                'Separate reviewed recovery executor and explicit admission; Controller.run remains fresh-only.',
                'Fresh exact inspector result matching this retained evidence and source/helper/runtime closure.',
                'Revalidate original log prefixes, final check-in/snapshot semantics and complete stopped inventory as applicable.',
                'Revalidate exact owned holder, original guard, absent old PID and every listener immediately before any removal.',
                'Durably claim this token before one action; never infer completion or replay from an intent alone.'],
            'prohibited': ['rearm', 'resignal', 'rebootstrap', 'state_rollback', 'force', 'foreign_job_control', 'automatic_resume']}


def validate_proposal(proposal, preparation, inspection, *, consumed_tokens=()):
    """Pure stale/replay check; returns no runnable callable or authority."""
    expected = plan_resume(reconstruct(preparation, inspection), consumed_tokens=consumed_tokens)
    require(proposal == expected, 'proposal is stale or altered')
