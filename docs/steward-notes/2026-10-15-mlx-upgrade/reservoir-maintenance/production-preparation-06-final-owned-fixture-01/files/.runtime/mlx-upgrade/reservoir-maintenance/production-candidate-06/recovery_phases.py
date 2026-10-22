"""Pure finite phase reconstruction; no Host calls, file reads or execution."""
from __future__ import annotations
import copy

import reservoir_maintenance as m
from recovery_ledger import session_shape


def actions(*, production=True):
    rows = [{'id': 'preflight', 'kind': 'preflight', 'operation': None, 'role': None}]
    def add(kind, operation=None, role=None, witness=None):
        name = 'witness:' + witness if witness else operation + (':' + role if role else '')
        rows.append({'id': name, 'kind': kind, 'operation': operation, 'role': role, 'witness': witness})
    for role, witness in (('provider', 'provider_drain'), ('reservoir', 'final_snapshot')):
        for op in ('create_hold', 'arm_next_spawn', 'signal_stop'): add('mutation', op, role)
        add('observation', 'held_verified', role); add('witness', witness=witness)
        add('mutation', 'remove_held', role); add('observation', 'job_absent', role)
    add('witness', witness='backup'); add('witness', witness='private_restore')
    add('mutation', 'install_repair'); add('mutation', 'bootstrap_original', 'reservoir')
    add('witness', witness='startup')
    if production: add('mutation', 'apply_provider_restart', 'provider')
    add('mutation', 'bootstrap_original', 'provider'); add('witness', witness='provider_ready')
    for role in m.ROLES: add('mutation', 'release_hold', role)
    add('completed', 'completed')
    return rows


def context(plan, state, action):
    bindings = {'plan_sha256': m.digest(plan), **{
        role + '_old_identity_sha256': m.digest(plan['roles'][role]['identity']) for role in m.ROLES}}
    name = action.get('witness'); receipts = state['receipts']
    if name == 'backup': bindings['final_snapshot_sha256'] = m.digest(receipts['final_snapshot'])
    if name == 'private_restore':
        bindings.update(backup_sha256=m.digest(receipts['backup']),
                        repair_source_sha256=plan['input_hashes']['reservoir_candidate'],
                        interpreter_sha256=plan['input_hashes']['interpreter'],
                        configuration_sha256=plan['input_hashes']['reservoir_settings'])
    if name == 'startup':
        bindings.update(private_restore_sha256=m.digest(receipts['private_restore']),
                        reservoir_new_identity_sha256=m.digest(state['replacements']['reservoir']))
    if name == 'provider_ready':
        bindings.update(startup_sha256=m.digest(receipts['startup']),
                        provider_new_identity_sha256=m.digest(state['replacements']['provider']))
    return {'transaction_id': plan['transaction_id'], 'plan': copy.deepcopy(plan), 'bindings': bindings,
            'execution_scope': state['execution_scope'], 'production_qualified': False,
            'receipts': copy.deepcopy(receipts)}


def apply_result(spec, state, action, result, *, reconciled=False):
    """Validate objective result semantics before advancing this one phase."""
    plan = spec['plan']; role = action['role']; op = action['operation']
    m.require(isinstance(result, dict), 'action result missing')
    old = plan['roles'][role]['identity'] if role else None
    if action['kind'] == 'preflight':
        m.require(result.get('passed') is True, 'preflight is not accepted')
        state['admission'] = copy.deepcopy(result.get('production_admission'))
        if state['execution_scope'] == 'production':
            m.validate_production_admission(plan, state['admission'])
    elif action['kind'] == 'witness':
        name = action['witness']; expected = context(plan, state, action)
        m.require(result.get('schema') == 'reservoir_maintenance_witness_v1'
                  and result.get('kind') == name and result.get('transaction_id') == plan['transaction_id']
                  and result.get('passed') is True and isinstance(result.get('evidence'), dict)
                  and all(result.get('bindings', {}).get(k) == v for k, v in expected['bindings'].items()),
                  'retained witness binding/acceptance differs: ' + name)
        state['receipts'][name] = copy.deepcopy(result)
    elif op == 'create_hold':
        m.require(result.get('guard') == state['guard'], 'created guard differs')
    elif op in ('arm_next_spawn', 'signal_stop') and result.get('effect') == 'owned_holder_observed':
        m.require(reconciled and result.get('original_identity') == old and result.get('guard') == state['guard'],
                  'owned holder adoption lacks exact original/guard')
        held = m.identity(result.get('identity')); m.require(held != old, 'holder is original worker')
        state['held_effects'][role] = copy.deepcopy(held)
        state['satisfied_by_effect'][action['id']] = copy.deepcopy(result)
        # This proves the safe current effect, not an unrecorded signal delivery.
    elif op == 'arm_next_spawn':
        m.require(result.get('armed') is True, 'arm not positively confirmed')
        state['armed_identities'][role] = copy.deepcopy(old)
    elif op == 'signal_stop':
        m.require(result.get('signal') == 'SIGTERM' and result.get('identity') == old
                  and result.get('force_used') is False, 'signal result differs')
        state['signalled_identities'][role] = copy.deepcopy(old)
    elif op == 'held_verified':
        ident = m.identity(result.get('identity')); m.require(ident != old, 'original is not a holder')
        if role in state['held_effects']: m.require(ident == state['held_effects'][role], 'adopted holder changed')
        state['holders'][role] = copy.deepcopy(ident)
    elif op == 'remove_held':
        m.require(result.get('removed_identity') == state['holders'].get(role), 'removed holder differs')
        if role not in state['parked']: state['parked'].append(role)
    elif op == 'job_absent':
        m.require(result.get('absent') is True and role in state['parked'], 'absence lacks confirmed removal')
    elif op == 'install_repair':
        m.require(result.get('source_sha256') == plan['input_hashes']['reservoir_candidate'], 'installed source differs')
        state['repaired'] = True
    elif op == 'apply_provider_restart':
        m.require(result.get('provider_settings_sha256') == plan['input_hashes']['provider_restart_settings']
                  and result.get('provider_restart_argv') == spec['provider_restart_argv'], 'applied pin differs')
        state['provider_pinned'] = True
    elif op == 'bootstrap_original':
        ident = m.identity(result.get('identity'))
        expected = spec['provider_restart_argv'] if role == 'provider' and state['execution_scope'] == 'production' else old['argv']
        m.require(ident != old and ident['argv'] == expected and role in state['parked'], 'replacement identity/phase differs')
        state['replacements'][role] = copy.deepcopy(ident); state['parked'].remove(role)
    elif op == 'release_hold':
        m.require(result.get('released') is True and 'provider_ready' in state['receipts'], 'guard release not accepted')
        state['released'].append(role)
    elif action['kind'] == 'completed':
        m.require(result.get('status') == 'verified' and result.get('transaction_id') == plan['transaction_id']
                  and result.get('plan_sha256') == m.digest(plan)
                  and result.get('witness_hashes') == {k: m.digest(v) for k, v in state['receipts'].items()},
                  'completion result differs')
        state['completed'] = True
    else:
        raise m.Refusal('unsupported finite recovery action')


def original_prefix(spec, report, state, sequence):
    plan = spec['plan']; previous = m.digest(plan); index = 0; pending = None; attempted = set()
    events = report['controller']['events']
    for number, row in enumerate(events, 1):
        m.require(row.get('sequence') == number and row.get('previous_sha256') == previous
                  and row.get('plan_sha256') == m.digest(plan), 'original controller chain differs')
        previous = m.digest(row); kind = row.get('type')
        if kind == 'failed':
            m.require(number == len(events), 'original events after failure are unsupported')
            continue
        m.require(index < len(sequence), 'original event after completion')
        action = sequence[index]
        if kind == 'mutation_intent':
            m.require(action['kind'] == 'mutation' and pending is None
                      and row.get('operation') == action['operation'] and row.get('role') == action['role'], 'unexpected original intent')
            pending = action['id']; attempted.add(pending); continue
        if kind == 'mutation_completed':
            m.require(pending == action['id'] and row.get('operation') == action['operation']
                      and row.get('role') == action['role'], 'original completion differs')
            result = row['result']; pending = None
        elif kind == 'preflight':
            m.require(action['id'] == 'preflight', 'preflight out of phase'); result = row['receipt']
        elif kind == 'witness':
            m.require(action['kind'] == 'witness' and row.get('kind') == action['witness'], 'witness out of phase')
            result = row['receipt']
        elif kind in ('held_verified', 'job_absent'):
            m.require(action['operation'] == kind and row.get('role') == action['role'], 'observation out of phase')
            result = {'identity': row['identity']} if kind == 'held_verified' else {'absent': True}
        elif kind == 'completed':
            m.require(action['kind'] == 'completed', 'completion out of phase'); result = row['receipt']
        else:
            raise m.Refusal('unknown original controller record')
        apply_result(spec, state, action, result); index += 1
    m.require(previous == report['controller']['last_sha256'], 'original controller head differs')
    return index, attempted


def ledger_prefix(spec, report, ledger, state, sequence, index, attempted):
    plan = spec['plan']; header = ledger.get('header'); rows = ledger.get('records', [])
    if header is None:
        m.require(rows == [] and ledger.get('head_sha256') == m.digest(plan), 'invalid empty recovery ledger')
        return index, attempted, []
    m.require(header.get('schema') == 'reservoir_recovery_ledger_v1'
              and header.get('transaction_id') == plan['transaction_id']
              and header.get('plan_sha256') == m.digest(plan)
              and header.get('preparation_sha256') == state['preparation_sha256']
              and header.get('original_controller_head_sha256') == state['controller_head_sha256']
              and header.get('root') == spec['transaction_root'], 'recovery ledger anchor differs')
    previous = m.digest(header); claims = {}; outstanding = []; failed_claims = set()
    host_events = report['host_operations']['events']
    for number, row in enumerate(rows, 1):
        m.require(row.get('schema') == 'reservoir_recovery_event_v1' and row.get('sequence') == number
                  and row.get('previous_sha256') == previous and row.get('transaction_id') == plan['transaction_id']
                  and row.get('plan_sha256') == m.digest(plan), 'recovery ledger chain differs')
        previous = m.digest(row)
        session_shape(row.get('executor_session'), spec['transaction_root'], plan)
        m.require(index < len(sequence), 'recovery after completed transaction')
        action = sequence[index]
        m.require(row.get('action_id') == action['id'] and row.get('operation') == action['operation']
                  and row.get('role') == action['role'], 'recovery action is out of phase')
        kind = row.get('type')
        if kind in ('action_claim', 'reconciliation_claim'):
            if kind == 'action_claim':
                m.require(action['id'] not in attempted and not outstanding, 'attempted action was reclaimed for execution')
            # Another explicit read-only reconciliation may supersede an
            # interrupted reconciliation claim. It never calls the producer;
            # its successful result must name every outstanding claim below.
            m.require(row.get('context') == context(plan, state, action), 'claim context differs from accepted prior state')
            for field in ('checkpoint_sha256', 'inspection_sha256'):
                m.require(m.hash_value(row.get(field)), 'claim binding missing')
            count = row.get('host_before_count')
            m.require(type(count) is int and 0 <= count <= len(host_events)
                      and row.get('host_before_sha256') == m.digest(host_events[:count]), 'claim Host prefix differs')
            claims[previous] = row; outstanding.append(previous); attempted.add(action['id'])
        elif kind == 'action_failed':
            m.require(row.get('claim_sha256') in outstanding, 'failure lacks current claim')
            validate_span(spec, host_events, claims[row['claim_sha256']], row)
            failed_claims.add(row['claim_sha256'])
            # Failure says nothing about whether an external effect happened.
        elif kind in ('action_result', 'reconciliation_result'):
            claim = claims.get(row.get('claim_sha256'))
            expected_kind = 'action_claim' if kind == 'action_result' else 'reconciliation_claim'
            m.require(claim is not None and claim['type'] == expected_kind
                      and row['claim_sha256'] in outstanding, 'result lacks exact current claim')
            validate_span(spec, host_events, claim, row)
            reconciled = kind == 'reconciliation_result'
            if reconciled:
                proof = row.get('reconciliation', {})
                m.require(proof.get('schema') == 'reservoir_recovery_reconciliation_v1' and proof.get('passed') is True
                          and proof.get('action_id') == action['id'] and proof.get('transaction_id') == plan['transaction_id']
                          and proof.get('plan_sha256') == m.digest(plan)
                          and proof.get('checkpoint_sha256') == claim['checkpoint_sha256']
                          and proof.get('inspection_sha256') == claim['inspection_sha256']
                          and proof.get('result') == row.get('result') and isinstance(proof.get('evidence'), dict),
                          'reconciliation proof binding differs')
                m.require(row.get('resolves_claims') == outstanding, 'reconciliation did not explicitly resolve current claims')
            else:
                m.require(outstanding == [row['claim_sha256']], 'ordinary result cannot resolve another uncertain claim')
                m.require(row['claim_sha256'] not in failed_claims, 'failed action requires reconciliation, not a later ordinary result')
            apply_result(spec, state, action, row['result'], reconciled=reconciled)
            outstanding = []; index += 1
        else:
            raise m.Refusal('unknown recovery event')
    m.require(previous == ledger.get('head_sha256'), 'recovery head differs')
    return index, attempted, outstanding


def validate_span(spec, events, claim, row):
    m.require(row['executor_session'] == claim['executor_session'], 'result executor differs from claim')
    value = row.get('host_operations', {})
    if value.get('unreadable') is True:
        m.require(row['type'] == 'action_failed', 'success has unreadable Host span')
        return
    first, last = value.get('before_count'), value.get('after_count')
    m.require(type(first) is int and type(last) is int and first == claim['host_before_count']
              and 0 <= first <= last <= len(events)
              and value.get('before_sha256') == m.digest(events[:first])
              and value.get('after_sha256') == m.digest(events[:last]), 'result Host prefix/span differs')
    refs = value.get('appended')
    m.require(isinstance(refs, list) and len(refs) == last - first, 'result Host appended set differs')
    for number, ref in enumerate(refs, first + 1):
        expected = spec['transaction_root'] + f'/host-operation-{number:05d}.json'
        m.require(ref.get('path') == expected and ref.get('record_sha256') == m.digest(events[number - 1])
                  and m.hash_value(ref.get('sha256')), 'result Host record binding differs')
    if row['type'] == 'reconciliation_result':
        m.require(first == last, 'reconciliation replayed a Host operation')


def checkpoint_from_inspection(spec, inspection, ledger):
    """Authoritative inputs must be freshly read by the driver under the lock."""
    plan = spec['plan']; report = copy.deepcopy(inspection)
    projection = {k: v for k, v in spec.items() if k != 'execution_review'}
    m.require(report.get('evidence_valid') is True and report.get('errors') == []
              and report.get('transaction_id') == plan['transaction_id']
              and report.get('preparation_sha256') == m.digest(projection), 'invalid retained inspection binding')
    m.require(report.get('observed_before') == report.get('observed_after'), 'unstable inspection')
    observed = report['observed_before']
    m.require(observed.get('host') == plan['host_binding'] and observed.get('protected') == plan['protected']
              and observed.get('stewardship') == {'paused': True, 'pause_generation': plan['pause_generation'],
                  'pause_actor': plan['actor'], 'lease': None}, 'current ownership/protected identity differs')
    if ledger['header'] is not None:
        m.require(report.get('recovery_ledger') == ledger, 'ledger not bound to fresh inspector evidence')
    root_guard = {'schema': 'model_environment_hold_v1', 'transaction_id': plan['transaction_id'], 'actor': plan['actor']}
    anchors = report['capture_anchors']
    for role in m.ROLES:
        m.require(anchors[role]['identity'] == plan['roles'][role]['identity'], 'original capture identity changed')
        m.require(report['roles'][role].get('guard') in (None, root_guard), 'foreign guard')
    scope = spec.get('execution_scope', 'production')
    m.require(scope in ('production', 'owned_fixture'), 'unsupported recovery execution scope')
    state = {'schema': 'reservoir_recovery_checkpoint_v1', 'transaction_id': plan['transaction_id'],
             'plan_sha256': m.digest(plan), 'preparation_sha256': m.digest(projection),
             'controller_head_sha256': report['controller']['last_sha256'], 'recovery_head_sha256': ledger['head_sha256'],
             'inspection_sha256': m.digest(report), 'execution_scope': scope, 'guard': root_guard,
             'host_counter': len(report['host_operations']['events']), 'host_operations_sha256': m.digest(report['host_operations']['events']),
             'guards': {r: report['roles'][r].get('guard') for r in m.ROLES}, 'anchors': copy.deepcopy(anchors),
             'receipts': {}, 'signal_times': copy.deepcopy(report['signal_wall_time_ns']),
             'armed_attempts': [], 'signal_attempts': [], 'armed_identities': {}, 'signalled_identities': {},
             'holders': {}, 'held_effects': {}, 'satisfied_by_effect': {}, 'startups': {}, 'parked': [],
             'replacements': {}, 'repaired': False, 'provider_pinned': False, 'released': [],
             'admission': None, 'completed': False, 'original_unresolved': copy.deepcopy(report['unresolved'])}
    sequence = actions(production=scope == 'production')
    index, attempted = original_prefix(spec, report, state, sequence)
    index, attempted, outstanding = ledger_prefix(spec, report, ledger, state, sequence, index, attempted)
    for action_id in attempted:
        if action_id.startswith('arm_next_spawn:'): state['armed_attempts'].append(action_id.split(':')[1])
        if action_id.startswith('signal_stop:'): state['signal_attempts'].append(action_id.split(':')[1])
    for row in report['host_operations']['events']:
        if row.get('operation') == 'bootstrap_intent':
            role = row['role']
            state['startups'][role] = {'log_anchor': copy.deepcopy(row['log_anchor'])}
            if role in state['replacements']: state['startups'][role]['identity'] = copy.deepcopy(state['replacements'][role])
    action = sequence[index] if index < len(sequence) else None
    outputs = report.get('recovery_output_presence', {})
    presence = bool(action and action['kind'] == 'witness' and outputs.get(action['witness']))
    holder_effect = bool(action and action['operation'] in ('arm_next_spawn', 'signal_stop', 'held_verified')
                         and report['roles'][action['role']]['classification'] == 'owned_holder_observed')
    state.update(action_index=index, action=copy.deepcopy(action), action_id=action['id'] if action else None,
                 attempted_actions=sorted(attempted), outstanding_claims=outstanding,
                 attempted=bool(action and action['id'] in attempted),
                 requires_reconciliation=bool(outstanding or presence or holder_effect or action and action['id'] in attempted),
                 inspection_unresolved=copy.deepcopy(report['unresolved']))
    state['armed_attempts'].sort(); state['signal_attempts'].sort(); state['parked'].sort()
    return state
