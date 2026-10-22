"""One finite ordinary-user Terminal parent; execution remains disabled.

No background loop, restart, failure cleanup, credential prewarm or arbitrary
command interface. A retained root always refuses a new parent invocation.
"""
from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import shlex
import sys
import time

import owned_fixture_coordinator as c
import owned_fixture_records as r
from owned_fixture_plan import proposed_schedule


def phases(release):
    """Exact pure plan, including the three non-executor publication boundaries."""
    steps = proposed_schedule()['executor_plan']
    r.require(release['executor_plan'] == steps and len(steps) == 71, 'sealed71 executor schedule differs')
    result = [('lock_pair', steps[:2])]
    cleanup_started = pin_started = False
    for step in steps[2:]:
        if step['kind'] == 'cleanup' and not cleanup_started:
            result.append(('cleanup_initialize', None)); cleanup_started = True
        if step['kind'] == 'pin' and not pin_started:
            result.append(('pin_initialize', None)); pin_started = True
        result.append(('executor:' + step['id'], step))
    result.append(('finalize', None))
    return result


def walk(release, perform):
    """Single pass over the sealed data. Exceptions stop immediately; no retry."""
    for action, detail in phases(release):
        perform(action, detail)


def parent_ledger(host, release_ref):
    return r.Ledger(host.root, 'parent', {'release_sha256': release_ref['sha256'],
                    'parent_identity_sha256': r.digest(host.owner['coordinator_identity'])})


def initial_observations(release, release_path):
    r.require(c.FIXTURE_EXECUTION_ENABLED is True, 'owned fixture execution disabled')
    r.require(os.geteuid() != 0 and os.getuid() == release['host_binding']['uid'] > 0
              and sys.stdin.isatty(), 'ordinary owning user in an interactive Terminal required')
    expected = [release['python'], '-B', str(c.HERE / 'owned_fixture_run.py'), '--execute', '--release', str(release_path)]
    queries = []
    def query(argv):
        from owned_fixture_commands import run
        result = run(argv,timeout=20)
        queries.append({'argv': argv, 'returncode': result.returncode, 'stdout': result.stdout,
                        'stderr': result.stderr, 'observed_wall_time_ns': time.time_ns(),
                        'native_command':result.native_command})
        r.require(result.returncode == 0 and not result.stderr.strip(), 'initial identity query failed')
        return result.stdout.strip()
    pid = os.getpid()
    before = query(['/bin/ps', '-p', str(pid), '-o', 'lstart='])
    argv = shlex.split(query(['/bin/ps', '-ww', '-p', str(pid), '-o', 'command=']))
    after = query(['/bin/ps', '-p', str(pid), '-o', 'lstart='])
    r.require(before == after and before and argv[1:] == expected[1:]
              and str(Path(argv[0]).resolve(strict=True)) in release['python_files'], 'exact parent identity/argv differs')
    host = {'uid': os.getuid(), 'boot_uuid': query(['/usr/sbin/sysctl', '-n', 'kern.bootsessionuuid']),
            'hostname': os.uname().nodename, 'os_version': query(['/usr/bin/sw_vers', '-productVersion'])}
    r.require(host == release['host_binding'], 'host boot/user changed before initial setup')
    return {'pid': pid, 'started_at': before, 'argv': argv}, host, queries


class NativeOperations:
    """Fixed private fixture methods only; not a generic service orchestrator."""
    def __init__(self, release_path, release, core):
        self.release_path = release_path; self.release_ref = r.reference(release_path)
        self.release = release; self.core = core; self.root = Path(release['fixture']['root'])
        self.actions = ['setup', *(action for action, _ in phases(release))]

    def host(self):
        from owned_fixture_system import OwnedSystem
        from owned_recovery_host import FixtureHost
        io = OwnedSystem(self.root, self.release['fixture']['token'], self.release['python'])
        host = FixtureHost.attach(self.root, core=self.core, io=io, sudo_debug=True)
        r.require(host.executor_identity() == host.owner['coordinator_identity'], 'original parent identity changed')
        c.owned_release(host, self.release_ref)
        return host

    def expected_heads(self, host, step):
        if step['kind'] == 'resume_one':
            journal = self.core['m'].Journal(host.root / 'controller')
            inspection = host.recovery_inspection(journal)
            ledger = self.core['recovery_ledger'].read_ledger(host.root, host.plan)
            checkpoint = self.core['recovery_phases'].checkpoint_from_inspection(host.spec, inspection, ledger)
            r.require(checkpoint['action_id'] == step['action_id'], 'next sealed main action differs')
            return self.core['production_recovery_driver'].expected_heads(checkpoint)
        if step['kind'] == 'cleanup':
            from owned_fixture_cleanup import attach
            _, ledger = attach(host, self.core)
            return {'cleanup': ledger.head}
        if step['kind'] == 'pin':
            inputs = r.read(host.root / 'pin/inputs.json')
            ledger = r.Ledger(host.root, 'pin', inputs['binding'])
            return {'pin': ledger.head}
        r.require(step['kind'] == 'fresh', 'unrecognized executor phase')
        return {}

    def perform(self, action, detail):
        host = self.host(); ledger = parent_ledger(host, self.release_ref)
        done = list(ledger.completed())
        r.require(done == self.actions[:len(done)] and len(done) < len(self.actions)
                  and self.actions[len(done)] == action and not ledger.attempted(action),
                  'parent phase attempted, out of order or retained; never replay')
        claim = ledger.append('claim', action, executor_identity=host.executor_identity())
        try:
            if action == 'lock_pair':
                result = c.run_lock_pair(host.root, self.release_ref, detail[0]['id'], detail[1]['id'], host)
            elif action == 'cleanup_initialize':
                from owned_fixture_cleanup import begin
                begin(host, self.core)
                result = {'context': r.reference(host.root / 'cleanup/context.json')}
            elif action == 'pin_initialize':
                from owned_fixture_pin import initialize
                initialize(host, self.core)
                result = {'inputs': r.reference(host.root / 'pin/inputs.json')}
            elif action == 'finalize':
                from owned_fixture_result import collect_final
                with self.core['transaction_execution'].ExecutionSession(host.root, host.plan,
                                                                          host.executor_identity(), 'resume_one'):
                    observation = collect_final(host, self.core, self.release)
                result = {'final_observations': observation, 'parent_absence_claimed': False}
            else:
                r.require(action == 'executor:' + detail['id'], 'parent executor phase differs')
                from owned_fixture_readiness import selected,prepare
                readiness=None
                if selected(detail):
                    claim_ref=r.reference(host.root/'parent'/f'event-{claim["sequence"]:05d}.json')
                    directory,readiness,phase_deadline=prepare(self,host,detail,claim_ref)
                    returned=c.launch_executor(directory,host,phase_deadline=phase_deadline)
                else:
                    heads = self.expected_heads(host, detail)
                    directory = c.prepare_executor(host.root, self.release_ref, detail['id'], host, expected_heads=heads)
                    returned = c.launch_executor(directory, host)
                r.require(returned['returncode'] == detail['expected_exit'], 'executor was not accepted as planned')
                result = {'request': r.reference(directory / 'request.json'), 'exit': r.reference(directory / 'exit.json')}
                if readiness is not None:result['readiness']=readiness
            ledger.append('result', action, claim_sha256=r.digest(claim), result=result)
        except BaseException as error:
            ledger.append('failed', action, claim_sha256=r.digest(claim), error_type=type(error).__name__,
                          error=str(error)[:1000], retry_permitted=False, force_used=False)
            raise


def execute(release_path):
    r.require(c.FIXTURE_EXECUTION_ENABLED is True, 'owned fixture execution disabled')
    r.require(os.geteuid() != 0 and sys.stdin.isatty(), 'ordinary-user interactive Terminal required')
    release_path = Path(release_path)
    release = c.validate_release(release_path)
    root = Path(release['fixture']['root'])
    r.require(not root.exists() and not root.is_symlink(), 'retained fixture root requires inspection, never restart')
    r.require(not Path(release['result_path']).exists(), 'result path already used')
    started = time.time_ns()
    from owned_fixture_commands import configure
    if not c.RUNS.exists():c.RUNS.mkdir(mode=0o700)
    configure(release,r.reference(release_path),'parent')
    identity, host, queries = initial_observations(release, release_path)
    # This original owner/start record precedes the very first service effect.
    c.initialize_fixture(release_path, initial_identity=identity, initial_host_binding=host,
                         started_wall_time_ns=started, initial_raw_queries=queries)
    from owned_recovery_core import load_core
    core = load_core(release)
    operations = NativeOperations(release_path, release, core)
    walk(release, operations.perform)
    from owned_fixture_result import produce
    result = produce(operations.host(), core, operations.release_ref, operations.actions)
    return {'result': result, 'parent_reaped': False, 'post_terminal_closeout_required': True,
            'production_qualified': False}


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--release', type=Path)
    parser.add_argument('--execute', action='store_true')
    args = parser.parse_args(argv)
    if not args.execute:
        # No import of final core, native query, child or service occurs here.
        value = {'status': 'not_executed', 'fixture_execution_enabled': c.FIXTURE_EXECUTION_ENABLED,
                 'production_qualified': False}
        if args.release is not None:
            c.validate_release(args.release)
            value['release'] = r.reference(args.release)
        print(json.dumps(value, sort_keys=True)); return 0
    r.require(args.release is not None, 'exact release required')
    print(json.dumps(execute(args.release), sort_keys=True))
    return 0


if __name__ == '__main__': raise SystemExit(main())
