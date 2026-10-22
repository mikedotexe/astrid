"""Fixed owned-fixture OS boundary. Definitions only; no work on import.

The coordinator supplies this object only after explicit owned-fixture admission.
There is no arbitrary launchctl target or command execution API.
"""
from __future__ import annotations
import os
from pathlib import Path
import plistlib
import re
import shlex
import signal
import subprocess
import sys
import time

from owned_fixture_records import directory, raw, read, require, target

FORBIDDEN_PORTS = {7878,7879,7880,7881,8090,8092,17881,11434}


class OwnedSystem:
    def __init__(self, root, token, executable):
        self.root=directory(root); self.token=token; self.executable=Path(executable); self.uid=os.getuid()
        require(self.uid>0 and os.geteuid()!=0,'never run fixture as root')
        self._owner()

    def _owner(self):
        require(read(self.root/'fixture.owner.json')=={'schema':'reservoir_maintenance_owned_fixture_v1',
                'token':self.token,'root':str(self.root),'uid':self.uid},'fixture owner changed')

    def _run(self, argv, *, terminal=False, timeout=20):
        self._owner()
        return subprocess.run(argv,check=False,timeout=timeout,
                              **({} if terminal else {'capture_output':True,'text':True}))

    def host(self):
        boot=self._run(['/usr/sbin/sysctl','-n','kern.bootsessionuuid'])
        version=self._run(['/usr/bin/sw_vers','-productVersion'])
        require(boot.returncode==version.returncode==0,'host identity query failed')
        return {'uid':self.uid,'boot_uuid':boot.stdout.strip(),'hostname':os.uname().nodename,'os_version':version.stdout.strip()}

    def job(self, role):
        require(role in ('provider','reservoir'),'unknown fixture role')
        result=self._run(['/bin/launchctl','print',target(self.uid,self.token,role)])
        if result.returncode:
            text=result.stdout+result.stderr
            require('Could not find service' in text or 'Could not find specified service' in text,'job absence unproven')
            return False,None
        match=re.search(r'^\s*pid = (\d+)\s*$',result.stdout,re.M)
        return True,int(match[1]) if match else None

    def _start(self,pid):
        result=self._run(['/bin/ps','-p',str(pid),'-o','lstart='])
        if result.returncode:return None
        return result.stdout.strip() or None

    def process(self,pid):
        if pid is None:return None
        require(type(pid) is int and pid>1,'invalid PID')
        before=self._start(pid)
        if before is None:return None
        result=self._run(['/bin/ps','-ww','-p',str(pid),'-o','command='])
        if result.returncode:return None
        after=self._start(pid)
        if after is None:return None
        require(after==before,'PID identity changed during observation')
        return {'pid':pid,'started_at':before,'argv':shlex.split(result.stdout.strip())}

    def listener_ports(self,pid):
        result=self._run(['/usr/sbin/lsof','-nP','-a','-p',str(pid),'-iTCP','-sTCP:LISTEN','-Fpn'])
        require(result.returncode in (0,1) and not result.stderr.strip(),'listener inspection failed')
        ports=[]
        for line in result.stdout.splitlines():
            if line.startswith('p'):require(line=='p'+str(pid),'foreign listener PID')
            if line.startswith('n'):
                match=re.fullmatch(r'n127\.0\.0\.1:(\d+)',line)
                require(match is not None,'nonprivate listener')
                port=int(match[1]);require(port not in FORBIDDEN_PORTS,'protected listener port');ports.append(port)
        return ports

    def port_pids(self,port):
        require(type(port) is int and 1024<port<65536 and port not in FORBIDDEN_PORTS,'invalid private port')
        result=self._run(['/usr/sbin/lsof','-nP',f'-iTCP:{port}','-sTCP:LISTEN','-Fp'])
        require(result.returncode in (0,1) and not result.stderr.strip(),'port inspection failed')
        return sorted({int(line[1:]) for line in result.stdout.splitlines() if line.startswith('p')})

    def bootstrap(self,role,plist):
        require(role in ('provider','reservoir'),'unknown fixture role')
        require(Path(plist)==self.root/(role+'.plist'),'bootstrap is not private fixture plist')
        expected={'Label':target(self.uid,self.token,role).split('/')[-1],
          'ProgramArguments':[str(self.executable),str(self.root/'maintenance_fixture_worker.py'),
            '--root',str(self.root),'--token',self.token,'--role',role,'--drain-seconds','6.0'],
          'WorkingDirectory':str(self.root),'RunAtLoad':True,'KeepAlive':True,'ThrottleInterval':1,'ExitTimeOut':5,
          'StandardOutPath':str(self.root/(role+'.stdout')),'StandardErrorPath':str(self.root/(role+'.stderr'))}
        require(plistlib.loads(raw(plist))==expected,'private bootstrap settings differ')
        return self._run(['/bin/launchctl','bootstrap',f'gui/{self.uid}',str(plist)])

    def debug(self,role,arguments,*,sudo_debug):
        require(role in ('provider','reservoir'),'unknown fixture role')
        require(arguments[:3]==['--program',str(self.executable),'--'],'debug executable/tail changed')
        expected=[str(self.root/'model_environment.py'),'launch','--control-root',str(self.root/('hold-'+role)),'--hold-only']
        require(arguments[3:-1]==expected and arguments[-1] in (self.token,self.token+'-cleanup'),'unowned holder argv')
        argv=['/bin/launchctl','debug',target(self.uid,self.token,role),*arguments]
        if sudo_debug:
            require(sys.stdin.isatty() and os.geteuid()!=0,'interactive ordinary-user Terminal required')
            argv=['/usr/bin/sudo','--',*argv]
        return self._run(argv,terminal=sudo_debug,timeout=180 if sudo_debug else 20)

    def signal(self,role,identity):
        require(role in ('provider','reservoir'),'unknown fixture role')
        loaded,pid=self.job(role)
        require(loaded and pid==identity['pid'] and self.process(pid)==identity,'cooperative signal identity differs')
        os.kill(pid,signal.SIGTERM)

    def bootout(self,role):
        require(role in ('provider','reservoir'),'unknown fixture role')
        return self._run(['/bin/launchctl','bootout',target(self.uid,self.token,role)])

    monotonic=staticmethod(time.monotonic)
    sleep=staticmethod(time.sleep)
