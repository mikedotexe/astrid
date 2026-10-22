"""One bounded read-only final03/06 release check; never a fixture launcher.

Requires a separately supplied immutable final-input hash. Definitions are inert.
Run only after independent review, using the selected Python with -I -B.
"""
from __future__ import annotations
import argparse
import datetime
import hashlib
import importlib
import importlib.abc
import importlib.machinery
import importlib.util
import json
import os
from pathlib import Path
import re
import stat
import sys
import time

ROOT=Path('/Users/v/other/astrid/.runtime/mlx-upgrade/reservoir-maintenance')
EVIDENCE=ROOT/'evidence'
FIXTURE=ROOT/'owned-recovery-fixture-03'
CORE=ROOT/'production-candidate-06'
INPUTS=EVIDENCE/'owned-fixture03-final-source-inputs-01.json'
RELEASE=EVIDENCE/'owned-fixture03-final-release-01.json'
OUTPUT=EVIDENCE/'owned-fixture03-final-release-validation-02.json'
WORKER=ROOT/'candidate-v2/maintenance_fixture_worker.py'
HOLD=Path('/Users/v/other/astrid/scripts/model_environment.py')


def require(value,message):
    if not value:raise ValueError(message)


def raw(path):
    path=Path(path);require(path.is_absolute() and path==path.resolve(strict=True),'input alias refused')
    before=path.stat();require(stat.S_ISREG(before.st_mode) and before.st_nlink==1 and before.st_size<=8*1024*1024,'oversized/shared input')
    data=path.read_bytes();after=path.stat()
    stamp=lambda s:(s.st_dev,s.st_ino,s.st_size,s.st_mtime_ns,s.st_ctime_ns,s.st_mode,s.st_uid,s.st_nlink)
    require(stamp(before)==stamp(after) and len(data)==before.st_size,'input changed during read')
    return data


def sha(path):return hashlib.sha256(raw(path)).hexdigest()
def reference(path):return {'path':str(path),'sha256':sha(path)}


def manifest(expected):
    require(re.fullmatch('[0-9a-f]{64}',expected or '') is not None,'explicit lowercase64-character input hash required')
    data=raw(INPUTS);require(hashlib.sha256(data).hexdigest()==expected,'final input manifest hash differs')
    value=json.loads(data)
    require(value.get('schema')=='owned_fixture03_final_source_review_inputs_v1'
            and value.get('scope')=='owned_fixture_only' and value.get('release_stage')=='final_enabled'
            and value.get('core_root')==str(CORE) and value.get('production_qualified') is False
            and value.get('actual_os_fixture_executed') is False,'final input manifest scope differs')
    wanted_fixture={str(p) for p in FIXTURE.glob('*.py')}|{str(FIXTURE/'README.md'),str(WORKER),str(HOLD)}
    wanted_core={str(p) for p in CORE.glob('*.py')}
    require(set(value['fixture_artifacts'])==wanted_fixture and set(value['core_artifacts'])==wanted_core
            and wanted_core and len(wanted_fixture|wanted_core)<=150,'complete exact03/06 input closure required')
    bindings={**value['fixture_artifacts'],**value['core_artifacts']}
    require(all(re.fullmatch('[0-9a-f]{64}',h or '') and sha(p)==h for p,h in bindings.items()),'bound final source changed')
    require(value.get('core_flags')=={'PRODUCTION_CONTROLLER_ENABLED':True,'PRODUCTION_EXECUTION_ENABLED':True},'final enabled flags required')
    require(sys.flags.isolated==1 and sys.dont_write_bytecode is True,'selected Python -I -B required')
    family=value['python_files'];require(isinstance(family,dict) and len(family)==2,'exact selected Python family required')
    require(str(Path(value['python']).resolve(strict=True)) in family and str(Path(sys.executable).resolve(strict=True)) in family,
            'selected/current Python family differs')
    require(all(re.fullmatch('[0-9a-f]{64}',h or '') and sha(p)==h for p,h in family.items()),'selected Python bytes changed')
    return value,bindings


class PinnedFixtureSourceLoader(importlib.machinery.SourceFileLoader):
    """Load only the manifest-bound source; never consult or publish bytecode."""
    def __init__(self,fullname,filename,wanted):
        super().__init__(fullname,filename)
        self.wanted=wanted

    def get_code(self,fullname):
        require(fullname==self.name,'source loader module identity differs')
        data=raw(Path(self.path))
        require(hashlib.sha256(data).hexdigest()==self.wanted,'pinned fixture source changed before compile')
        return compile(data,self.path,'exec',dont_inherit=True)

    def set_data(self,path,data,*,_mode=0o666):
        raise RuntimeError('source-only fixture loader cannot publish bytecode')


class PinnedFixtureSourceFinder(importlib.abc.MetaPathFinder):
    """The exact25 direct fixture names; no directory/path fallback is added."""
    def __init__(self,bindings):
        self.sources={Path(path).stem:(path,wanted) for path,wanted in bindings.items()
                      if Path(path).parent==FIXTURE and Path(path).suffix=='.py'}
        require(len(self.sources)==25 and all('.' not in name for name in self.sources),
                'exact25 pinned fixture modules required')

    def find_spec(self,fullname,path=None,target=None):
        if fullname not in self.sources:return None
        require(path is None and target is None,'fixture package/reload import refused')
        filename,wanted=self.sources[fullname]
        loader=PinnedFixtureSourceLoader(fullname,filename,wanted)
        return importlib.util.spec_from_file_location(fullname,filename,loader=loader)


def main(argv=None):
    started=time.monotonic()
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--inputs-sha256',required=True)
    args=parser.parse_args(argv)
    inputs,bindings=manifest(args.inputs_sha256)
    require(not any(name.startswith(('owned_fixture_','owned_recovery_','owned_pin_')) for name in sys.modules),'fresh validation process required')
    initial_release=reference(RELEASE)
    violations=[];executed={}
    forbidden_modules={'numpy','scipy','mlx','mlx_lm','torch','cv2','CoreML','coremltools','AVFoundation','Quartz','ctypes'}
    forbidden_events={'subprocess.Popen','os.system','os.fork','os.forkpty','os.exec','os.posix_spawn','os.kill','os.killpg',
        'os.remove','os.rmdir','os.rename','os.mkdir','os.link','os.symlink','os.truncate','os.chmod','os.chown','os.utime','os.chdir'}
    receipt={'schema':'owned_fixture03_read_only_release_validation_v1','started_at':datetime.datetime.now(datetime.timezone.utc).isoformat(),
        'runner':reference(Path(__file__).resolve()),'inputs':{'path':str(INPUTS),'sha256':args.inputs_sha256},'release':initial_release,
        'actual_os_fixture_executed':False,'production_qualified':False,'native_entrypoints_called':False}
    fd=os.open(OUTPUT,os.O_WRONLY|os.O_CREAT|os.O_EXCL|os.O_NOFOLLOW,0o600)
    with os.fdopen(fd,'w') as output:
        def refuse(event):violations.append(event);raise RuntimeError('read-only guard refused '+event)
        def audit(event,args):
            if time.monotonic()-started>30:refuse('cooperative_30_second_deadline')
            if event in forbidden_events or event.startswith(('socket.','ctypes.')):refuse(event)
            if event=='import' and args[0].split('.')[0] in forbidden_modules:refuse('import:'+args[0])
            if event=='open' and isinstance(args[2],int) and args[2]&(os.O_WRONLY|os.O_RDWR|os.O_CREAT|os.O_TRUNC|os.O_APPEND):refuse('write_open')
            if event=='open' and isinstance(args[0],str) and args[0].startswith('/Users/v/other/') and args[0].endswith('.pyc'):
                refuse('unbound_local_bytecode')
            if event=='exec':
                filename=args[0].co_filename
                if filename.startswith('/Users/v/other/'):
                    if bindings.get(filename)!=sha(filename):refuse('unbound_local_source:'+filename)
                    executed[filename]=bindings[filename]
        sys.addaudithook(audit)
        try:
            sys.meta_path.insert(0,PinnedFixtureSourceFinder(bindings))
            coordinator=importlib.import_module('owned_fixture_coordinator')
            require(Path(coordinator.__file__)==FIXTURE/'owned_fixture_coordinator.py','validator imported alternate coordinator')
            release=coordinator.validate_release(RELEASE)
            require(coordinator.FIXTURE_EXECUTION_ENABLED is True,'fixture final flag is not enabled')
            require(release['artifacts']==inputs['fixture_artifacts'] and release['core']['artifacts']==inputs['core_artifacts']
                    and release['core']['root']==str(CORE) and release['core']['flags']==inputs['core_flags'],'release/source inputs differ')
            require(all(release[key]==inputs[key] for key in ('python','python_files','coordination','host_binding','executor_plan','injections')),
                    'release/runtime/schedule input bindings differ')
            require(reference(RELEASE)==initial_release and sha(INPUTS)==args.inputs_sha256,'release/input changed across validation')
            require(all(sha(path)==wanted for path,wanted in bindings.items()),'source changed across validation')
            require(all(sha(path)==wanted for path,wanted in inputs['python_files'].items()),'Python changed across validation')
            root=Path(release['fixture']['root']);native=FIXTURE/'runs'/('owned-native-'+release['fixture']['token']);result=Path(release['result_path'])
            for path in (root,native,result):require(not path.exists() and not path.is_symlink(),'retained fixture/native/result destination: '+str(path))
            elapsed=time.monotonic()-started;require(elapsed<=30,'cooperative whole-validation acceptance deadline exceeded')
            receipt.update(passed=True,fixture_root_unused=True,native_root_unused=True,result_path_unused=True,
                fixture_root=str(root),native_root=str(native),result_path=str(result),final_source_files=len(bindings),
                executor_steps=len(release['executor_plan']),acceptance_elapsed_s=elapsed)
        except BaseException as error:receipt.update(passed=False,error_type=type(error).__name__,error=str(error))
        receipt.update(completed_at=datetime.datetime.now(datetime.timezone.utc).isoformat(),elapsed_s=time.monotonic()-started,
                       guard_violations=violations,executed_local_sources=executed)
        json.dump(receipt,output,indent=2,sort_keys=True);output.write('\n');output.flush();os.fsync(output.fileno())
    print(json.dumps({'passed':receipt['passed'],'output':str(OUTPUT)}))
    return 0 if receipt['passed'] and not violations else 1


if __name__=='__main__':raise SystemExit(main())
