"""Finite preparatory04 rehearsal schedule; pure data, never executes work."""
from __future__ import annotations

from owned_fixture_cleanup import ACTIONS as CLEANUP_ACTIONS
from owned_fixture_pin import ACTIONS as PIN_ACTIONS

MAIN = ('signal_stop:provider', 'held_verified:provider', 'witness:provider_drain',
        'remove_held:provider', 'job_absent:provider', 'create_hold:reservoir',
        'arm_next_spawn:reservoir', 'signal_stop:reservoir', 'held_verified:reservoir',
        'witness:final_snapshot', 'remove_held:reservoir', 'job_absent:reservoir',
        'witness:backup', 'witness:private_restore', 'install_repair',
        'bootstrap_original:reservoir', 'witness:startup', 'bootstrap_original:provider',
        'witness:provider_ready', 'release_hold:provider', 'release_hold:reservoir', 'completed')
MAIN_INTERRUPTED = {'signal_stop:provider', 'witness:provider_drain', 'remove_held:provider',
                    'signal_stop:reservoir', 'witness:final_snapshot', 'witness:backup',
                    'witness:private_restore', 'install_repair', 'bootstrap_original:reservoir',
                    'bootstrap_original:provider', 'release_hold:provider', 'completed'}
CLEANUP_INTERRUPTED = {'signal_stop:provider', 'cleanup_witness:provider', 'remove_held:provider',
                       'signal_stop:reservoir', 'cleanup_witness:reservoir', 'remove_held:reservoir',
                       'release_hold:provider', 'complete'}


def proposed_schedule():
    points = [{'id': 'fresh-provider-arm', 'stream': 'host', 'event': 'host:debug_result', 'match': {'role': 'provider'}}]
    steps = []
    def step(kind, action, mode='execute', point=None, code=None):
        steps.append({'id': f'{len(steps)+1:02d}-' + kind.replace('_', '-') + '-' + action.replace(':', '-').replace('_', '-')[:20],
                      'kind': kind, 'mode': mode, 'action_id': action, 'injection': point,
                      'expected_exit': (75 if point else 0) if code is None else code})
    step('lock_hold', 'lock', code=0); step('lock_refusal', 'lock', code=76)
    step('fresh', 'arm_next_spawn:provider', point='fresh-provider-arm')
    step('resume_one', 'arm_next_spawn:provider', mode='reconcile')
    for action in MAIN:
        point = None
        if action in MAIN_INTERRUPTED:
            point = 'main-' + action.replace(':', '-').replace('_', '-')
            if action.startswith('bootstrap_original:'):
                points.append({'id': point, 'stream': 'host', 'event': 'host:bootstrap_command_returned',
                               'match': {'role': action.split(':')[1]}})
            else:
                points.append({'id': point, 'stream': 'action_return', 'event': action, 'match': {'action_id': action}})
        # The exact core recognizes the already observed owned holder after a
        # reconciled signal. Adopt that current effect; do not repeat a signal.
        mode = 'reconcile' if action.startswith('held_verified:') else 'execute'
        step('resume_one', action, mode=mode, point=point)
        if point: step('resume_one', action, mode='reconcile')
    for action in CLEANUP_ACTIONS:
        point = None
        if action in CLEANUP_INTERRUPTED:
            point = 'cleanup-' + action.replace(':', '-').replace('_', '-')
            points.append({'id': point, 'stream': 'cleanup_return', 'event': action, 'match': {'action_id': action}})
        step('cleanup', action, point=point)
        if point: step('cleanup', action, mode='reconcile')
    for action in PIN_ACTIONS:
        point = None
        if action in ('replace_plist', 'bootstrap'):
            point = 'pin-' + action
            points.append({'id': point, 'stream': 'pin', 'event': 'plist_replaced' if action == 'replace_plist' else 'bootstrap_returned',
                           'match': {}})
        step('pin', action, point=point)
        if point: step('pin', action, mode='reconcile')
    return {'injections': {'schema': 'owned_fixture_injections_v1', 'scope': 'owned_fixture_only',
                           'production_qualified': False, 'points': points}, 'executor_plan': steps,
            'required_preparations': ['initialize_fixture', 'cleanup.begin after accepted main completion',
                                      'pin.initialize after accepted cleanup completion'],
            'status': 'proposed_source_only_not_released', 'production_pin_schedule_executed': False,
            'ProductionHost_executed': False, 'numeric_restore': False, 'final_release_acceptance': False}
