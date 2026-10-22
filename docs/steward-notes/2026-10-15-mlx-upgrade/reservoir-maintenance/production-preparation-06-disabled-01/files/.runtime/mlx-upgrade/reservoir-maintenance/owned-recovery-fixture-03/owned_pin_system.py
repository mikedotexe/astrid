"""Narrow generated .pin job OS adapter. No work occurs on import."""
from __future__ import annotations

from pathlib import Path
import plistlib
import re
import time

from owned_fixture_records import raw, require, target
from owned_fixture_pin import settings


class PinSystem:
    def __init__(self, host):
        require(host.scope == 'owned_fixture', 'only an owned pin fixture is supported')
        self.host = host

    def _target(self, supplied):
        self.host.check_owned()
        wanted = target(self.host.uid, self.host.token, 'pin')
        require(supplied == wanted, 'foreign pin target')
        return wanted

    @staticmethod
    def _run(argv):
        from owned_fixture_coordinator import FIXTURE_EXECUTION_ENABLED
        require(FIXTURE_EXECUTION_ENABLED is True, 'owned pin native IO disabled')
        from owned_fixture_commands import run
        value = run(argv,timeout=20)
        return {'returncode': value.returncode, 'stdout': value.stdout, 'stderr': value.stderr,
                'argv': argv, 'observed_wall_time_ns': time.time_ns(),'native_command':value.native_command}

    def pin_observe(self, supplied, recorded_pid):
        own = self._target(supplied)
        outcome = self._run(['/bin/launchctl', 'print', own]); text = outcome['stdout'] + outcome['stderr']
        loaded = outcome['returncode'] == 0
        if not loaded:
            require('Could not find service' in text or 'Could not find specified service' in text,
                    'pin absence is not proven')
        def number(pattern):
            found = re.search(pattern, outcome['stdout'], re.M)
            return int(found[1]) if found else None
        pid = number(r'^\s*pid = (\d+)\s*$') if loaded else None
        process = self.host.io.process(pid)
        require(pid is None or process is not None, 'pin PID vanished during observation; inspect again explicitly')
        recorded = self.host.io.process(recorded_pid) if recorded_pid is not None else None
        listeners = self.host.io.listener_ports(recorded_pid) if recorded is not None else []
        return {'target': own, 'job_loaded': loaded, 'process': process,
                'runs': number(r'^\s*runs = (\d+)\s*$'),
                'last_exit_code': number(r'^\s*last exit code = (-?\d+)\s*$'),
                'recorded_pid_absent': recorded is None, 'listener_pids': [recorded_pid] if listeners else [],
                'raw': outcome}

    def pin_bootstrap(self, plist, supplied):
        own = self._target(supplied); path = Path(plist)
        require(path == self.host.root / 'pin/active.plist'
                and plistlib.loads(raw(path)) == settings(self.host, self.host.root / 'pin/model-b.sentinel'),
                'pin bootstrap settings differ')
        observation = self.pin_observe(own, None)
        require(not observation['job_loaded'] and observation['process'] is None, 'pin job already present')
        return self._run(['/bin/launchctl', 'bootstrap', f'gui/{self.host.uid}', str(path)])

    def pin_remove_absent(self, supplied, recorded_pid):
        own = self._target(supplied); observation = self.pin_observe(own, recorded_pid)
        require(observation['job_loaded'] and observation['process'] is None
                and observation['recorded_pid_absent'] is True and observation['runs'] == 1
                and observation['last_exit_code'] == 0 and not observation['listener_pids'],
                'only the naturally exited stateless pin job can be removed')
        return self._run(['/bin/launchctl', 'bootout', own])
