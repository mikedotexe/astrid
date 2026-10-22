"""Exclusive initial fixture setup and positive adoption; no implicit OS actions."""
from __future__ import annotations

import fcntl
import os
from pathlib import Path
import plistlib
import stat

from owned_fixture_records import (Ledger, check_reference, digest, directory, identity,
                                   raw, read, reference, require, sync, target, write)

_ACTIVE = {}


def require_setup_active(root):
    """Require this process's still-owned, exact pre-plan lock descriptor."""
    root = directory(root)
    session = _ACTIVE.get(str(root))
    require(session is not None and session.pid == os.getpid() and session.fd is not None,
            'active setup session required')
    info = os.fstat(session.fd); path_info = (root / 'setup.lock').lstat()
    require(stat.S_ISREG(path_info.st_mode) and info.st_nlink == path_info.st_nlink == 1
            and info.st_uid == path_info.st_uid == os.getuid()
            and (info.st_dev, info.st_ino) == session.inode == (path_info.st_dev, path_info.st_ino),
            'setup lock identity changed')
    return session


def copy_exclusive(source, target_path):
    data = raw(source)
    fd = os.open(target_path, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
    try:
        with os.fdopen(fd, 'wb', closefd=False) as stream:
            stream.write(data); stream.flush(); os.fsync(fd)
    finally: os.close(fd)
    sync(Path(target_path).parent)
    require(raw(target_path) == data, 'private copy differs')


class SetupSession:
    """Separate pre-plan lock. No fabricated plan or core executor session."""
    def __init__(self, root): self.root = Path(root); self.fd = None; self.pid = None; self.inode = None

    def __enter__(self):
        directory(self.root)
        require(str(self.root) not in _ACTIVE, 'setup already executing in this process')
        self.fd = os.open(self.root / 'setup.lock', os.O_RDWR | os.O_CREAT | os.O_NOFOLLOW, 0o600)
        try:
            info = os.fstat(self.fd)
            require(stat.S_ISREG(info.st_mode) and info.st_nlink == 1 and info.st_uid == os.getuid(),
                    'foreign setup lock')
            fcntl.flock(self.fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
            self.pid = os.getpid(); self.inode = (info.st_dev, info.st_ino)
            _ACTIVE[str(self.root)] = self
            require_setup_active(self.root)
            return self
        except BaseException:
            if _ACTIVE.get(str(self.root)) is self: del _ACTIVE[str(self.root)]
            os.close(self.fd); self.fd = None; raise

    def __exit__(self, *_):
        if _ACTIVE.get(str(self.root)) is self: del _ACTIVE[str(self.root)]
        os.close(self.fd); self.fd = None


def create_layout(root, token, release_ref, executor, host_binding, python,
                  worker_source, hold_source):
    """Create evidence/files only; root existence always refuses fresh setup."""
    root = Path(root); directory(root.parent)
    require(root.name == 'owned-recovery-' + token and root.is_absolute(), 'wrong owned root name')
    require(os.geteuid() != 0 and host_binding['uid'] == os.getuid() > 0, 'ordinary user required')
    identity(executor); require(executor['pid'] == os.getpid(), 'wrong setup executor')
    release = check_reference(release_ref)
    require(release.get('schema') == 'owned_recovery_fixture_release_v2'
            and release.get('approved') is True and release.get('scope') == 'owned_fixture_only'
            and release.get('host_binding') == host_binding
            and release.get('production_qualified') is False
            and release.get('final_release_acceptance') is False, 'reviewed preparatory release required')
    targets = {role: target(host_binding['uid'], token, role) for role in ('provider', 'reservoir', 'pin')}
    require(release.get('fixture') == {'root': str(root), 'token': token, 'targets': targets}
            and release.get('setup_plan') == ['bootstrap:reservoir', 'bootstrap:provider'],
            'finite reviewed setup root/targets/order differ')
    root.mkdir(mode=0o700)
    owner = {'schema': 'owned_fixture_setup_owner_v1', 'token': token, 'root': str(root),
             'uid': os.getuid(), 'host_binding': host_binding, 'coordinator_identity': executor,
             'release_ref': release_ref, 'targets': targets, 'python': str(python)}
    write(root / 'setup-owner.json', owner)
    write(root / 'fixture.owner.json', {'schema': 'reservoir_maintenance_owned_fixture_v1',
          'token': token, 'root': str(root), 'uid': os.getuid()})
    for source in (Path(worker_source), Path(hold_source)):
        require(release['artifacts'].get(str(source)) == reference(source)['sha256'], 'unreviewed copied source')
        copy_exclusive(source, root / source.name)
    for filename, data in (('reservoir-source.marker', b'original fixture marker\n'),
                           ('reservoir-repair.marker', b'repaired fixture marker\n'),
                           ('model-never-opened.sentinel', b'never open as a model\n')):
        fd = os.open(root / filename, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
        try: os.write(fd, data); os.fsync(fd)
        finally: os.close(fd)
    sync(root)
    ledger = Ledger(root, 'setup', {'owner_sha256': digest(owner), 'release_sha256': release_ref['sha256']}, create=True)
    return owner, ledger


def retain_plists(root, host):
    """Publish both exact originals before the first bootstrap claim."""
    root = Path(root)
    for role in ('provider', 'reservoir'):
        path = root / (role + '.plist')
        settings = host.plist_settings(role)
        require(settings['Label'] == host.target(role).split('/')[-1]
                and settings['ProgramArguments'] == host.argv(role)
                and settings['WorkingDirectory'] == str(root), 'unexpected initial plist')
        data = plistlib.dumps(settings)
        if path.exists(): require(raw(path) == data, 'existing initial plist differs')
        else:
            fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
            try: os.write(fd, data); os.fsync(fd)
            finally: os.close(fd)
            sync(root)


def setup_ledger(root):
    owner = read(Path(root) / 'setup-owner.json')
    check_reference(owner['release_ref'])
    return owner, Ledger(root, 'setup', {'owner_sha256': digest(owner),
                         'release_sha256': owner['release_ref']['sha256']})


def authorize_successor(root, host, *, expected_head, next_action):
    """One explicitly requested successor for one never-attempted setup launch."""
    root = Path(root)
    with SetupSession(root):
        require(not (root / 'setup-complete.json').exists() and not (root / 'setup-successor.json').exists(),
                'setup complete or successor already assigned')
        owner, ledger = setup_ledger(root); release = check_reference(owner['release_ref'])
        require(release.get('setup_successor_policy') == {'max_successors': 1,
                'allowed_actions': ['bootstrap:reservoir', 'bootstrap:provider'], 'no_attempt_replay': True},
                'reviewed finite successor policy missing')
        require(ledger.head == expected_head and next_action in release['setup_plan']
                and not ledger.attempted(next_action), 'stale or already attempted successor action')
        done = ledger.completed(); attempted = {row['action'] for row in ledger.rows}
        require(attempted == set(done) and list(done) == release['setup_plan'][:len(done)]
                and len(done) < 2 and release['setup_plan'][len(done)] == next_action,
                'unresolved setup attempt or wrong successor phase')
        executor = identity(host.executor_identity()); prior = owner['coordinator_identity']
        require(executor['pid'] == os.getpid() and executor != prior
                and host.io.process(prior['pid']) is None, 'prior exact coordinator not absent')
        host.validate_setup_owner(owner, require_executor=False)
        views = {role: host.setup_observe(role) for role in ('provider', 'reservoir')}
        for role, view in views.items():
            accepted = done.get('bootstrap:' + role)
            if accepted:
                require(view['process'] == accepted['identity'] and view.get('ready') is True,
                        'accepted setup worker changed')
            else:
                require(not view['job_loaded'] and view['process'] is None and not view['listener_pids'],
                        'never-attempted setup role is not absent')
        require(host.io.process(prior['pid']) is None and host.executor_identity() == executor,
                'setup ownership changed during successor inspection')
        receipt = {'schema': 'owned_fixture_setup_successor_v1', 'owner_sha256': digest(owner),
                   'release_ref': owner['release_ref'], 'expected_setup_head': expected_head,
                   'permitted_action': next_action, 'prior_executor': prior, 'executor_identity': executor,
                   'host_binding': owner['host_binding'], 'observed_roles': views,
                   'plist_refs': {r: reference(root / (r + '.plist')) for r in ('provider', 'reservoir')},
                   'prior_exact_executor_absent': True, 'attempt_replay_permitted': False}
        write(root / 'setup-successor.json', receipt)
        return receipt


def bootstrap_one(root, host, role, *, mode, executor):
    """One initial launch or positive retained adoption; never replay a claim."""
    require(role in ('reservoir', 'provider') and mode in ('execute', 'reconcile'), 'unknown setup action')
    root = Path(root); identity(executor)
    with SetupSession(root):
        require(executor['pid'] == os.getpid() and host.executor_identity() == executor,
                'setup claim executor is not this actual process')
        require(not (root / 'setup-complete.json').exists(), 'setup is already complete')
        owner, ledger = setup_ledger(root)
        host.validate_setup_owner(owner, require_executor=mode == 'execute')
        require(host.host_binding == owner['host_binding'] and host.target(role) == owner['targets'][role],
                'setup host/target differs')
        action = 'bootstrap:' + role; completed = ledger.completed()
        require(action not in completed, 'initial bootstrap already accepted')
        if role == 'provider': require('bootstrap:reservoir' in completed, 'reservoir must be ready first')
        binding = {'target': host.target(role), 'plist_ref': reference(root / (role + '.plist')),
                   'expected_argv': host.argv(role), 'executor_identity': executor}
        if mode == 'execute':
            require(not ledger.attempted(action), 'initial bootstrap attempted; inspect or reconcile only')
            view = host.setup_observe(role)
            require(not view['job_loaded'] and view['process'] is None and not view['listener_pids'],
                    'initial job/listener not absent')
            claim = ledger.append('claim', action, **binding, absent_observation=view)
            try:
                result = host.setup_bootstrap(role, claim)
                identity(result['identity'])
                require(result.get('ready') is True and host.env.command_matches(result['identity']['argv'], host.argv(role)),
                        'initial same-process readiness incomplete')
                ledger.append('result', action, claim_sha256=digest(claim), result=result)
            except BaseException as error:
                ledger.append('failed', action, claim_sha256=digest(claim), error_type=type(error).__name__,
                              error=str(error)[:1000], retry_permitted=False)
                raise
        else:
            require(ledger.attempted(action), 'no initial bootstrap attempt to reconcile')
            original = next(row for row in ledger.rows if row['action'] == action and row['type'] == 'claim')
            for key in ('target', 'plist_ref', 'expected_argv'):
                require(original[key] == binding[key], 'setup inputs changed since attempt')
            claim = ledger.append('reconcile_claim', action, **binding)
            proof = host.setup_reconcile(role, original)
            require(proof.get('passed') is True and proof.get('action') == action
                    and proof.get('original_claim_sha256') == digest(original), 'setup adoption proof differs')
            result = proof['result']; identity(result['identity'])
            require(result.get('ready') is True and host.env.command_matches(result['identity']['argv'], host.argv(role)),
                    'setup adoption not ready')
            ledger.append('reconciled', action, claim_sha256=digest(claim),
                          resolves_claims=ledger.outstanding(action), proof=proof, result=result)
        return result


def publish_complete(root, host, plan, spec):
    """Publish only exact original identities; interrupted publication adopts identical bytes."""
    root = Path(root)
    with SetupSession(root):
        owner, ledger = setup_ledger(root); done = ledger.completed()
        require(set(done) == {'bootstrap:reservoir', 'bootstrap:provider'}, 'setup launches incomplete')
        identities = {role: done['bootstrap:' + role]['identity'] for role in ('provider', 'reservoir')}
        for role, ident in identities.items():
            view = host.setup_observe(role)
            require(view['process'] == ident and view.get('ready') is True and view['listener_pids'] == [ident['pid']],
                    'original identity/readiness changed before plan publication')
            require(plan['roles'][role]['identity'] == ident, 'plan identity was invented')
        for name, value in (('plan.json', plan), ('preparation.json', spec)):
            path = root / name
            if path.exists(): require(read(path) == value, 'partial setup publication differs')
            else: write(path, value)
        result = {'schema': 'owned_fixture_setup_complete_v1', 'owner_sha256': digest(owner),
                  'setup_head_sha256': ledger.head, 'plan_ref': reference(root / 'plan.json'),
                  'spec_ref': reference(root / 'preparation.json'), 'identities': identities}
        path = root / 'setup-complete.json'
        if path.exists(): require(read(path) == result, 'setup completion differs')
        else: write(path, result)
        host.bind_plan(plan, spec)
        return result
