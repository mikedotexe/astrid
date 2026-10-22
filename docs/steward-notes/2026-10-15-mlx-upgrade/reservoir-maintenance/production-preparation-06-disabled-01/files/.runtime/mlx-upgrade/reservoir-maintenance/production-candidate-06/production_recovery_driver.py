"""Explicit one-step continuation. No CLI or production-enabling path.

Actual effects are available only through the still-disabled reviewed Host and
controller flags. An attempted action is never executed again. Reconciliation
observes and adopts a proven existing effect; it does not replay its producer.
"""
from __future__ import annotations
import copy
import hashlib
from pathlib import Path

import reservoir_maintenance as m
from recovery_ledger import RecoveryLedger, read_ledger
from recovery_phases import apply_result, checkpoint_from_inspection, context
from transaction_execution import controller_session, require_active

HEAD_KEYS = ('controller_head_sha256', 'host_operations_sha256', 'recovery_head_sha256', 'inspection_sha256')


def expected_heads(checkpoint):
    return {key: checkpoint[key] for key in HEAD_KEYS}


def host_history(root):
    rows = []; refs = []
    paths = sorted(Path(root).glob('host-operation-*.json'))
    m.require(len(paths) <= 10000, 'too many Host records')
    for index, path in enumerate(paths, 1):
        m.require(path.name == f'host-operation-{index:05d}.json', 'Host operation sequence incomplete')
        row = m.read_record(path); rows.append(row)
        with path.open('rb') as stream: sha = hashlib.file_digest(stream, 'sha256').hexdigest()
        refs.append({'path': str(path), 'sha256': sha, 'record_sha256': m.digest(row)})
    return {'events': rows, 'refs': refs, 'sha256': m.digest(rows)}


def span(before, after):
    count = len(before['events'])
    m.require(after['events'][:count] == before['events'] and after['refs'][:count] == before['refs'],
              'Host history was rewritten during one step')
    return {'before_count': count, 'after_count': len(after['events']),
            'before_sha256': before['sha256'], 'after_sha256': after['sha256'],
            'appended': after['refs'][count:]}


def hydrate_controller(journal, host, checkpoint):
    controller = m.Controller(journal, host)
    for field in ('receipts', 'repaired', 'provider_pinned', 'admission', 'replacements'):
        setattr(controller, field, copy.deepcopy(checkpoint[field]))
    controller.parked = set(checkpoint['parked'])
    return controller


def validate_phase_observation(plan, checkpoint, report, *, reconcile=False):
    """Validate the other role too; a local action guard is not enough."""
    action = checkpoint['action']; operation = action['operation']; target = action['role']
    exempt_effect = reconcile and operation in ('arm_next_spawn', 'signal_stop', 'held_verified',
                                                'remove_held', 'bootstrap_original')
    for role in m.ROLES:
        variant = report['installed_variants'][role]
        source = 'candidate' if role == 'reservoir' and checkpoint['repaired'] else 'original'
        settings = 'pinned' if role == 'provider' and checkpoint['provider_pinned'] else 'original'
        if reconcile and operation == 'install_repair' and role == 'reservoir': source = 'candidate'
        if reconcile and operation == 'apply_provider_restart' and role == 'provider': settings = 'pinned'
        m.require(variant['source'] == source and variant['plist'] == settings,
                  'unrelated or unproved installed phase changed: ' + role)
        row = report['roles'][role]; view = row['observed']; old = plan['roles'][role]['identity']
        if exempt_effect and target == role:
            m.require(row['classification'] in ('owned_holder_observed', 'absent_observed',
                      'original_identity_observed', 'recorded_replacement_observed',
                      'unrecorded_replacement_candidate'), 'foreign/ambiguous reconciliation target')
            continue  # Exact desired effect is verified by reconcile_recovery.
        if role in checkpoint['replacements']:
            expected = checkpoint['replacements'][role]
            m.require(view.get('process') == expected and view['job_loaded'] is True
                      and view['listener_pids'] == [expected['pid']] and row['old_pid_observation'] is None,
                      'accepted replacement changed: ' + role)
        elif role in checkpoint['parked']:
            m.require(view.get('process') is None and view['job_loaded'] is False
                      and view['listener_pids'] == [] and row['old_pid_observation'] is None,
                      'parked role changed: ' + role)
        elif role in checkpoint['holders'] or role in checkpoint['held_effects']:
            expected = checkpoint['holders'].get(role, checkpoint['held_effects'].get(role))
            m.require(row['classification'] == 'owned_holder_observed' and view.get('process') == expected
                      and view['job_loaded'] is True and view['listener_pids'] == []
                      and row['old_pid_observation'] is None and row['guard'] == checkpoint['guard'],
                      'held role changed: ' + role)
        else:
            m.require(view.get('process') == old and view['job_loaded'] is True
                      and view['listener_pids'] == [old['pid']], 'original role changed: ' + role)


def execute_one(controller, checkpoint, action):
    """Exactly one existing Host operation/observation, with no journal replay."""
    host, plan = controller.host, controller.plan
    op, role = action['operation'], action['role']
    ctx = context(plan, checkpoint, action)
    if action['kind'] == 'preflight': return m.preflight(plan, host)
    if action['kind'] == 'witness': return host.collect_witness(action['witness'], ctx)
    if op == 'create_hold': return host.create_hold(role, controller.guard)
    if op == 'arm_next_spawn': return host.arm_next_spawn(role, controller.guard, plan['roles'][role]['identity'])
    if op == 'signal_stop': return host.signal_stop(role, plan['roles'][role]['identity'])
    if op == 'held_verified': return {'identity': controller.wait_held(role)}
    if op == 'remove_held': return host.remove_held(role, checkpoint['holders'][role], controller.guard)
    if op == 'job_absent': controller.absent(role); return {'absent': True}
    if op == 'install_repair': return host.install_repair(ctx)
    if op == 'apply_provider_restart': return host.apply_provider_restart(ctx)
    if op == 'bootstrap_original': return host.bootstrap_original(role, ctx)
    if op == 'release_hold': return host.release_hold(role, controller.guard)
    if action['kind'] == 'completed':
        for name in m.ROLES: controller.ready_identity(name, checkpoint['replacements'][name])
        controller.invariant()
        return {'schema': 'reservoir_maintenance_result_v1', 'status': 'verified',
                'transaction_id': plan['transaction_id'], 'plan_sha256': m.digest(plan),
                **{r: checkpoint['replacements'][r] for r in m.ROLES},
                'force_used': False, 'runtime_state_rollback': False, 'execution_scope': host.scope,
                'production_qualified': False, 'no_missed_input_claimed': False,
                'witness_hashes': {k: m.digest(v) for k, v in checkpoint['receipts'].items()}}
    raise m.Refusal('unsupported one-step action')


def resume_one(journal, host, *, expected, mode='execute'):
    """Recompute under shared lock; execute or reconcile one explicit phase.

    `expected` is a stale-command guard, never authority. No supplied checkpoint
    is trusted. There is no loop, automatic retry or fall-through to another step.
    """
    m.require(host.scope in ('production', 'owned_fixture'), 'unsupported concrete recovery scope')
    if host.scope == 'production':
        m.require(m.PRODUCTION_CONTROLLER_ENABLED is True, 'production controller admission disabled')
    m.require(mode in ('execute', 'reconcile') and set(expected) == set(HEAD_KEYS), 'explicit mode/head bindings required')
    with controller_session(journal, host, 'resume_one'):
        root = journal.root.parent
        current = m.Journal(journal.root)
        m.require(current.plan == journal.plan and current.plan == host.plan, 'recovery plan changed')
        report = host.recovery_inspection(current)
        ledger_view = read_ledger(root, current.plan)
        checkpoint = checkpoint_from_inspection(host.spec, report, ledger_view)
        m.require(checkpoint['execution_scope'] == host.scope and host.spec['transaction_root'] == str(root),
                  'retained execution scope/root differs')
        m.require(expected_heads(checkpoint) == expected, 'stale recovery request; no claim or action')
        action = checkpoint['action']
        m.require(action is not None and not checkpoint['completed'], 'transaction already completed')
        validate_phase_observation(current.plan, checkpoint, report, reconcile=mode == 'reconcile')
        m.require(host.revalidate_recovery(checkpoint, report) == checkpoint, 'retained Host did not revalidate exact state')
        # Re-read all three evolving evidence heads immediately before the claim.
        m.require(m.Journal(current.root).events == current.events and read_ledger(root, current.plan) == ledger_view,
                  'controller/recovery history changed before claim')
        before = host_history(root)
        m.require(before['events'] == report['host_operations']['events'], 'Host history changed before claim')
        controller = hydrate_controller(current, host, checkpoint)
        if mode == 'execute':
            m.require(not checkpoint['requires_reconciliation'] and not report['unresolved'],
                      'attempted or unresolved phase requires evidence reconciliation; never replay')
            controller.invariant()
        ledger = RecoveryLedger(root, current.plan, checkpoint['preparation_sha256'], checkpoint['controller_head_sha256'])
        details = {'action_id': action['id'], 'operation': action['operation'], 'role': action['role'],
                   'witness': action.get('witness'), 'context': context(current.plan, checkpoint, action),
                   'checkpoint_sha256': m.digest(checkpoint), 'inspection_sha256': m.digest(report),
                   'host_before_count': len(before['events']), 'host_before_sha256': before['sha256']}
        claim = ledger.append('action_claim' if mode == 'execute' else 'reconciliation_claim', **details)
        claim_hash = m.digest(claim)
        try:
            require_active(root, current.plan)
            proof = None
            if mode == 'reconcile':
                proof = host.reconcile_recovery(action['id'], checkpoint, report)
                m.require(proof.get('schema') == 'reservoir_recovery_reconciliation_v1'
                          and proof.get('passed') is True and proof.get('action_id') == action['id']
                          and proof.get('transaction_id') == current.plan['transaction_id']
                          and proof.get('plan_sha256') == m.digest(current.plan)
                          and proof.get('checkpoint_sha256') == m.digest(checkpoint)
                          and proof.get('inspection_sha256') == m.digest(report)
                          and isinstance(proof.get('evidence'), dict), 'observed-effect reconciliation proof differs')
                result = proof['result']
            else:
                result = execute_one(controller, checkpoint, action)
            # Validate the step before durable acceptance. An invalid result
            # leaves the claim uncertain even if an external effect occurred.
            apply_result(host.spec, copy.deepcopy(checkpoint), action, result, reconciled=mode == 'reconcile')
            after = host_history(root); host_span = span(before, after)
            if mode == 'reconcile':
                m.require(before == after, 'reconciliation called a Host producer/control operation')
            record = ledger.append('action_result' if mode == 'execute' else 'reconciliation_result',
                                   **details, claim_sha256=claim_hash, result=result, host_operations=host_span,
                                   **({'reconciliation': proof, 'resolves_claims': checkpoint['outstanding_claims'] + [claim_hash]}
                                      if mode == 'reconcile' else {}))
            return {'schema': 'reservoir_recovery_step_v1', 'status': 'step_completed' if mode == 'execute' else 'effect_reconciled',
                    'action_id': action['id'], 'record_sha256': m.digest(record), 'recovery_head_sha256': ledger.view['head_sha256'],
                    'production_qualified': False, 'automatic_next_action': False, 'result': result}
        except BaseException as error:
            # Retain claim and effects. Failure cannot authorize a second call.
            try:
                current_span = span(before, host_history(root))
            except BaseException:
                current_span = {'unreadable': True}
            ledger.append('action_failed', **details, claim_sha256=claim_hash,
                          error_type=type(error).__name__, error=str(error)[:1500],
                          host_operations=current_span, automatic_retry_permitted=False,
                          effect_absence_inferred=False)
            raise
