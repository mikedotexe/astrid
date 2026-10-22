"""One-step cleanup ledger under the unchanged main transaction lock.

Never calls Controller.run or rewrites its completed history. No automatic retry,
forced termination, or worker bootout. The Host owns exact OS-effect validation.
"""
from __future__ import annotations

from pathlib import Path

from owned_fixture_records import Ledger, check_reference, digest, identity, read, reference, require, write

OPERATIONS = ('create_hold', 'arm_next_spawn', 'signal_stop', 'held_verified',
              'cleanup_witness', 'remove_held', 'job_absent')
ACTIONS = tuple(f'{operation}:{role}' for role in ('provider', 'reservoir') for operation in OPERATIONS) + (
    'final_state', 'release_hold:provider', 'release_hold:reservoir', 'complete')


def main_checkpoint(host, core):
    journal = core['m'].Journal(host.root / 'controller')
    inspection = host.recovery_inspection(journal)
    ledger = core['recovery_ledger'].read_ledger(host.root, host.plan)
    checkpoint = core['recovery_phases'].checkpoint_from_inspection(host.spec, inspection, ledger)
    require(checkpoint.get('completed') is True and checkpoint.get('execution_scope') == 'owned_fixture'
            and not inspection.get('unresolved') and inspection.get('evidence_valid') is True,
            'main fixture not fully accepted')
    return checkpoint, inspection


def begin(host, core):
    """Create separate cleanup authority only after current complete main proof."""
    with core['transaction_execution'].ExecutionSession(host.root, host.plan,
                                                       host.executor_identity(), 'resume_one'):
        checkpoint, inspection = main_checkpoint(host, core)
        retained = host.root / 'cleanup-main-inspection.json'
        if retained.exists():
            saved = read(retained)
            require(saved == inspection, 'cleanup initialization inspection changed')
        else: write(retained, inspection)
        context = {'schema': 'owned_fixture_cleanup_context_v1', 'root': str(host.root),
                   'token': host.token, 'main_plan_sha256': digest(host.plan),
                   'main_heads': {k: checkpoint[k] for k in ('controller_head_sha256', 'recovery_head_sha256')},
                   'checkpoint_sha256': digest(checkpoint), 'main_identities': checkpoint['replacements'],
                   'main_inspection': reference(retained),
                   'main_backup': checkpoint['receipts']['backup'],
                   'guard': {'schema': 'model_environment_hold_v1', 'transaction_id': host.token + '-cleanup', 'actor': 'owned-fixture-cleanup'},
                   'expected_ticks': {'fixture_astrid': {'ticks': 22}, 'fixture_minime': {'ticks': 7}},
                   'scope': 'owned_fixture_only', 'production_qualified': False}
        for role in ('provider', 'reservoir'): identity(context['main_identities'][role])
        intent = host.root / 'cleanup-context-intent.json'
        if intent.exists(): require(read(intent) == context, 'partial cleanup intent differs')
        else: write(intent, context)
        path = host.root / 'cleanup'
        if not path.exists():
            ledger = Ledger(host.root, 'cleanup', {'context_sha256': digest(context)}, create=True)
        else:
            if not (path / 'header.json').exists():
                require(not list(path.iterdir()), 'partial cleanup directory has unknown files')
                write(path / 'header.json', {'schema': 'owned_fixture_ledger_v1', 'name': 'cleanup',
                                             'binding': {'context_sha256': digest(context)}})
            ledger = Ledger(host.root, 'cleanup', {'context_sha256': digest(context)})
            require(not ledger.rows, 'cleanup already attempted; attach only')
        if (path / 'context.json').exists(): require(read(path / 'context.json') == context, 'cleanup context differs')
        else: write(path / 'context.json', context)
        return context


def attach(host, core):
    """Read one exact cleanup chain; never start the whole cleanup sequence again."""
    context = read(host.root / 'cleanup/context.json')
    require(read(host.root / 'cleanup-context-intent.json') == context, 'cleanup intent differs')
    require(context.get('schema') == 'owned_fixture_cleanup_context_v1'
            and context.get('root') == str(host.root) and context.get('token') == host.token
            and context.get('main_plan_sha256') == digest(host.plan)
            and context.get('guard') == {'schema': 'model_environment_hold_v1', 'transaction_id': host.token + '-cleanup', 'actor': 'owned-fixture-cleanup'}
            and context.get('production_qualified') is False, 'cleanup context differs')
    ledger = Ledger(host.root, 'cleanup', {'context_sha256': digest(context)})
    done = ledger.completed()
    require(list(done) == list(ACTIONS[:len(done)]), 'cleanup completions are out of order')
    pending = {row['action'] for row in ledger.rows} - set(done)
    require(pending <= ({ACTIONS[len(done)]} if len(done) < len(ACTIONS) else set()), 'unexpected cleanup claim')
    journal = core['m'].Journal(host.root / 'controller')
    recovery = core['recovery_ledger'].read_ledger(host.root, host.plan)
    require(journal.previous == context['main_heads']['controller_head_sha256']
            and recovery['head_sha256'] == context['main_heads']['recovery_head_sha256'],
            'cleanup cannot rewrite or continue main journal')
    inspection = check_reference(context['main_inspection'])
    require(inspection['controller']['events'] == journal.events
            and inspection['controller']['last_sha256'] == journal.previous
            and inspection['recovery_ledger'] == recovery, 'retained main evidence differs')
    for artifact in inspection.get('artifacts', {}).values():
        require(reference(artifact['path'])['sha256'] == artifact['sha256'], 'accepted main artifact changed')
    checkpoint = core['recovery_phases'].checkpoint_from_inspection(host.spec, inspection, recovery)
    require(checkpoint.get('completed') is True and digest(checkpoint) == context['checkpoint_sha256']
            and checkpoint['replacements'] == context['main_identities']
            and checkpoint['receipts']['backup'] == context['main_backup'], 'cleanup main acceptance was not derived')
    return context, ledger


def validate_result(action, result, context, done):
    require(isinstance(result, dict), 'cleanup result missing')
    operation, _, role = action.partition(':')
    if operation == 'create_hold': require(result.get('guard') == context['guard'], 'cleanup guard differs')
    elif operation in ('arm_next_spawn', 'signal_stop') and result.get('effect') == 'owned_holder_observed':
        identity(result['identity'])
        require(result.get('original_identity') == context['main_identities'][role]
                and result.get('guard') == context['guard'], 'wrong cleanup holder effect')
    elif operation == 'arm_next_spawn': require(result.get('armed') is True, 'cleanup arm unproved')
    elif operation == 'signal_stop':
        require(result.get('identity') == context['main_identities'][role] and result.get('signal') == 'SIGTERM'
                and result.get('force_used') is False, 'cleanup signal differs')
    elif operation == 'held_verified':
        identity(result['identity']); require(result['identity'] != context['main_identities'][role], 'worker is not holder')
    elif operation == 'cleanup_witness': require(result.get('passed') is True and result.get('role') == role,
                                                'cleanup drain witness incomplete')
    elif operation == 'remove_held':
        require(result.get('removed_identity') == done['held_verified:' + role]['identity'], 'cleanup removed wrong holder')
    elif operation == 'job_absent': require(result.get('absent') is True, 'cleanup absence unproved')
    elif operation == 'release_hold': require(result.get('released') is True, 'cleanup guard not released')
    elif operation in ('final_state', 'complete'):
        require(result.get('passed') is True and result.get('both_jobs_absent') is True
                and result.get('all_owned_pids_absent') is True and result.get('all_ports_unowned') is True
                and result.get('final_state') == context['expected_ticks']
                and result.get('main_backup_unchanged') is True and result.get('unique_applied_checkins') == 2
                and result.get('force_used') is False, 'cleanup final evidence incomplete')
    else: raise ValueError('unknown cleanup action')


def perform(host, action, context, done):
    operation, _, role = action.partition(':'); guard = context['guard']
    if operation == 'create_hold': return host.create_hold(role, guard)
    if operation == 'arm_next_spawn': return host.arm_next_spawn(role, guard, context['main_identities'][role])
    if operation == 'signal_stop': return host.signal_stop(role, context['main_identities'][role])
    if operation == 'held_verified': return host.cleanup_held(role, context)
    if operation == 'cleanup_witness': return host.cleanup_witness(role, context)
    if operation == 'remove_held': return host.remove_held(role, done['held_verified:' + role]['identity'], guard)
    if operation == 'job_absent':
        view = host.inspect_role(role)
        require(not view['job_loaded'] and view['process'] is None and not view['listener_pids'], 'cleanup job survives')
        return {'absent': True}
    if operation == 'release_hold': return host.release_hold(role, guard)
    if operation in ('final_state', 'complete'): return host.cleanup_final(context, require_guards_absent=operation == 'complete')
    raise ValueError('unsupported cleanup action')


def step(host, core, *, expected_head, action, mode):
    require(mode in ('execute', 'reconcile') and action in ACTIONS, 'explicit cleanup action/mode required')
    with core['transaction_execution'].ExecutionSession(host.root, host.plan,
                                                       host.executor_identity(), 'resume_one'):
        context, ledger = attach(host, core)
        require(ledger.head == expected_head, 'stale cleanup request')
        done = ledger.completed(); require(len(done) < len(ACTIONS) and ACTIONS[len(done)] == action, 'wrong cleanup phase')
        original = next((row for row in ledger.rows if row['action'] == action and row['type'] == 'claim'), None)
        if mode == 'execute': require(original is None, 'cleanup action attempted; reconcile only')
        else: require(original is not None, 'no cleanup attempt to reconcile')
        operation, _, role = action.partition(':')
        claim = ledger.append('claim' if mode == 'execute' else 'reconcile_claim', action,
                              executor_identity=host.executor_identity(), context_sha256=digest(context),
                              operation=operation, role=role or None)

        def authorize(wanted_operation, wanted_role):
            core['transaction_execution'].require_active(host.root, host.plan)
            ledger.reload()
            require(mode == 'execute' and ledger.rows[-1] == claim
                    and (wanted_operation, wanted_role) == (operation, role or None),
                    'cleanup action lacks exact current claim')
            return claim

        host.enable_cleanup(context, authorize)
        try:
            if mode == 'execute':
                result = perform(host, action, context, done)
                validate_result(action, result, context, done)
                ledger.append('result', action, claim_sha256=digest(claim), result=result)
            else:
                proof = host.reconcile_cleanup(action, context, original)
                require(proof.get('passed') is True and proof.get('action_id') == action
                        and proof.get('original_claim_sha256') == digest(original), 'cleanup reconciliation differs')
                result = proof['result']; validate_result(action, result, context, done)
                ledger.append('reconciled', action, claim_sha256=digest(claim),
                              resolves_claims=ledger.outstanding(action), proof=proof, result=result)
        except BaseException as error:
            ledger.append('failed', action, claim_sha256=digest(claim), error_type=type(error).__name__,
                          error=str(error)[:1000], retry_permitted=False)
            raise
        finally:
            host.disable_cleanup()
        return {'action': action, 'head_sha256': ledger.head, 'result': result,
                'complete': action == 'complete', 'automatic_next_action': False, 'production_qualified': False}
