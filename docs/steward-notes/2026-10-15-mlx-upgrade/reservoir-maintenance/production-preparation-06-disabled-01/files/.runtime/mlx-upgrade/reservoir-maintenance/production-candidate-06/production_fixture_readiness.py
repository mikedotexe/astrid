"""Pure fixture03 ready-before-seal proof; no controller/Host imports."""
from __future__ import annotations
import math
import re
import shlex
from pathlib import Path

from production_fixture_admission import require,digest,finite,identity,schedule
from production_fixture_native import resolve_query


def selected(step):
    return step['kind']=='resume_one' and step['mode']=='reconcile' and step['action_id'] in ('held_verified:provider','held_verified:reservoir')


def check_readiness(files,fixture,release,executors,parent,sessions,native):
    root=Path(fixture['root']);steps=schedule()[0];wanted=[s for s in steps if selected(s)]
    section=fixture['readiness'];require(set(section)=={'records','inspections'},'readiness sections differ')
    records=[root/'readiness'/(s['id']+'.json') for s in wanted]
    inspections=[root/'readiness'/(s['id']+'-inspection.json') for s in wanted]
    refs=lambda paths:[{'path':str(p),'sha256':fixture['artifacts'][str(p)]} for p in paths]
    require(section['records']==refs(records) and section['inspections']==refs(inspections)
            and set((root/'readiness').iterdir())==set(records+inspections),'readiness inventory differs')
    plan=files.ref(fixture['plan']);guard={'schema':'model_environment_hold_v1','transaction_id':plan['transaction_id'],'actor':plan['actor']}
    for step in steps:
        executor=executors[step['id']];request=executor['request'];action='executor:'+step['id']
        parent_result=parent['done'].get(action,{})
        if not selected(step):
            require('readiness' not in request and 'readiness' not in parent_result,'unplanned readiness phase');continue
        ref={'path':str(root/'readiness'/(step['id']+'.json')),'sha256':fixture['artifacts'][str(root/'readiness'/(step['id']+'.json'))]}
        require(request.get('readiness')==parent_result.get('readiness')==ref,'selected readiness reference differs')
        value=files.ref(ref);role=step['action_id'].split(':')[1];old=plan['roles'][role]['identity']
        claim=next(r for r in parent['rows'] if r['type']=='claim' and r['action']==action)
        claim_ref=value['parent_claim'];require(files.ref(claim_ref)==claim,'readiness parent action differs')
        end=next(r for r in parent['rows'] if r['type']=='result' and r['action']==action)
        require(value['schema']=='owned_fixture_holder_readiness_v1' and value['step']==step
                and value['release']==fixture['release'] and value['root']==str(root)
                and value['parent_identity']==parent['identity'] and value['guard']==guard
                and value['source_artifacts_sha256']==digest(release['artifacts']) and value['coordination']==release['coordination'],
                'readiness ownership/source differs')
        session=value['executor_session'];require(sessions.get(digest(session))==session and session['executor_identity']==parent['identity'],'readiness session differs')
        ended=files.json(root/'execution-sessions'/(session['session_id']+'-end.json'))
        require(claim['wall_time_ns']<=session['started_wall_time_ns']<=value['started_wall_time_ns']
                <=value['completed_wall_time_ns']<=ended['completed_wall_time_ns']<=executor['launch']['started_wall_time_ns']
                <=executor['exit']['completed_wall_time_ns']<=end['wall_time_ns'],'ready/seal/session/child order differs')
        prior=executors[steps[steps.index(step)-1]['id']]
        require(prior['step']['action_id']=='signal_stop:'+role and prior['step']['mode']=='reconcile'
                and value['previous_exit']==prior['entry']['exit'] and prior['exit']['completed_wall_time_ns']<=claim['wall_time_ns'], 'readiness predecessor differs')
        for name,operation in (('prior_debug','debug'),('prior_signal','signal')):
            pair=value[name];first=files.ref(pair['intent']);last=files.ref(pair['result'])
            require(first['operation']==operation+'_intent' and last['operation']==('debug_result' if operation=='debug' else 'signal_sent')
                    and first['role']==last['role']==role and first['identity']==old
                    and last['wall_time_ns']<=value['started_wall_time_ns'],'readiness prior effect differs')
            if operation=='debug':require(last['returncode']==0,'readiness debug failed')
            else:require(first['signal']==last['signal']=='SIGTERM' and last['identity']==old,'readiness signal differs')
        phase=value['phase_started_monotonic'];deadline=value['readiness_deadline_monotonic'];phase_end=value['phase_deadline_monotonic']
        require(all(finite(v) for v in (phase,deadline,phase_end)) and phase_end==phase+300 and deadline==phase+90,'readiness budget changed')
        inspection=files.ref(value['final_inspection']);heads=value['stable_heads'];expected=value['expected_heads']
        require(set(heads)=={'controller_head_sha256','host_operations_sha256','recovery_head_sha256'}
                and expected=={**heads,'inspection_sha256':digest(inspection)} and request['expected_heads']==expected,
                'readiness authoritative sealed heads differ')
        samples=value['samples'];require(isinstance(samples,list) and 0<len(samples)<=901,'readiness samples missing/oversized')
        previous=phase;previous_wall=value['started_wall_time_ns'];pending=None
        for sample in samples:
            begin=sample['started_monotonic'];finish=sample['completed_monotonic'];when=sample['observed_wall_time_ns']
            require(finite(begin) and finite(finish) and previous<=begin<=finish<deadline
                    and type(when) is int and previous_wall<=when<=value['completed_wall_time_ns']
                    and sample['guard']==guard,'readiness sample timing/guard differs')
            target=sample['target'];other=sample['other_role'];observed=sample['old_pid_observation']
            require(target['target']==release['fixture']['targets'][role] and target['job_loaded'] is True
                    and observed in (None,old),'readiness target/original observation differs')
            if role=='provider':
                other_old=plan['roles']['reservoir']['identity']
                require(other['target']==release['fixture']['targets']['reservoir'] and other['job_loaded'] is True
                        and other['process']==other_old and other['listener_pids']==[other_old['pid']] and other['held'] is None,'readiness other original changed')
            else:require(other['target']==release['fixture']['targets']['provider'] and other['job_loaded'] is False
                         and other['process'] is None and not other['listener_pids'] and other['held'] is None,'readiness other parked role differs')
            process=target['process']
            if process==old:
                require(target['held'] is None and target['listener_pids'] in ([],[old['pid']]),'draining worker state differs')
            elif process is None:require(not target['listener_pids'] and target['held'] is None,'publication gap not empty')
            else:
                ident=identity(process);key=(ident['pid'],ident['started_at'])
                require(pending in (None,key) and not target['listener_pids'],'pending holder drift/listener')
                pending=key
                expected_tail=[str(root/'model_environment.py'),'launch','--control-root',str(root/('hold-'+role)),'--hold-only',plan['transaction_id']]
                require(ident['argv'][1:]==expected_tail and str(Path(ident['argv'][0]).resolve(strict=True)) in release['python_files'],'pending holder command differs')
            raw_sample(files,fixture,release,plan,role,sample,native,previous_wall)
            previous=finish;previous_wall=when
        final=samples[-1]['target'];ready=identity(value['ready_identity'])
        receipt_path=root/('hold-'+role)/'held'/(str(ready['pid'])+'.json')
        require(str(receipt_path) in fixture['artifacts'],'readiness held receipt unbound')
        held_receipt=files.json(receipt_path)
        require(held_receipt.get('schema')=='model_environment_held_v1' and held_receipt.get('pid')==ready['pid']
                and held_receipt.get('started_at')==ready['started_at'] and held_receipt.get('hold')==guard
                and held_receipt.get('hold_only') is True and held_receipt.get('model_imported') is False
                and held_receipt.get('listener_opened') is False
                and held_receipt.get('launcher_sha256')==fixture['artifacts'][str(root/'model_environment.py')],
                'readiness held receipt identity/guard/launcher differs')
        require(final['process']==ready and final['held'] is not None and samples[-1]['old_pid_observation'] is None
                and final['held']['identity']==ready and final['held']['guard']==guard and final['held']['hold_only'] is True
                and not final['listener_pids'] and inspection['roles'][role]['observed']==final,
                'final readiness is not an exact accepted holder')
        launch=executor['launch'];started=launch.get('started_monotonic');timeout=launch.get('timeout_s')
        require(finite(started) and previous<=started<phase_end and type(timeout) is int and timeout>=1
                and timeout==math.floor(phase_end-started) and launch.get('phase_deadline_monotonic')==phase_end,
                'readiness child renewed or exceeded original phase budget')
        created=executor['created'];budget=created.get('budget_observed_monotonic')
        require(finite(budget) and started<=budget<phase_end and created.get('budget_timeout_s')==math.floor(phase_end-budget),
                'readiness creation budget differs')
        end_record=executor['exit'];wait_start=end_record.get('wait_started_monotonic');wait_timeout=end_record.get('wait_timeout_s');completed=end_record.get('completed_monotonic')
        require(finite(wait_start) and finite(completed) and budget<=wait_start<=completed<phase_end
                and type(wait_timeout) is int and wait_timeout>=1 and wait_timeout==math.floor(phase_end-wait_start),
                'readiness child wait renewed phase budget')


def raw_sample(files,fixture,release,plan,role,sample,native,earliest):
    """Reconstruct observed roles from ordered actual query results, not booleans.

    Repeated process reads may prove a publication transition or natural exit;
    every positive start/argv/start triple must be internally stable. Every
    exported query is both caller-bound and within this exact monotonic sample.
    """
    rows=sample['queries'];require(isinstance(rows,list) and rows,'readiness raw queries missing')
    for row in rows:
        command=resolve_query(row,native,caller='parent',earliest=earliest,latest=sample['observed_wall_time_ns'])
        require(sample['started_monotonic']<=command['intent']['monotonic']
                <=command['exit']['monotonic']<=sample['completed_monotonic'],'native query outside monotonic readiness sample')
    old=plan['roles'][role]['identity'];other='reservoir' if role=='provider' else 'provider'
    other_old=plan['roles'][other]['identity'];target=sample['target'];other_view=sample['other_role']
    allowed=[old,other_old]+([target['process']] if target['process'] not in (None,old) else [])
    processes={};jobs={};pid_ports={};port_owners={};index=0
    while index<len(rows):
        row=rows[index];argv=row['argv'];index+=1
        if argv[:2]==['/bin/launchctl','print']:
            name=next((r for r in ('provider','reservoir') if release['fixture']['targets'][r]==argv[2]),None)
            require(name is not None,'readiness unrelated launchd target')
            if row['returncode']==0:
                require(not row['stderr'].strip(),'readiness launchd query warning')
                matches=re.findall(r'^\s*pid = (\d+)\s*$',row['stdout'],re.M)
                require(len(matches)<=1,'readiness duplicate launchd PID')
                jobs.setdefault(name,[]).append((True,int(matches[0]) if matches else None))
            else:jobs.setdefault(name,[]).append((False,None))
        elif argv[:2]==['/bin/ps','-p']:
            pid=int(argv[2]);require(argv[3:]==['-o','lstart='],'readiness process start grammar')
            if row['returncode']==1:
                processes.setdefault(pid,[]).append(None);continue
            require(row['returncode']==0 and row['stdout'].strip() and not row['stderr'].strip(),'readiness process start absent')
            require(index<len(rows),'readiness command query missing');command=rows[index];index+=1
            require(command['argv']==['/bin/ps','-ww','-p',str(pid),'-o','command='],'readiness start/command order differs')
            require(index<len(rows),'readiness final start missing');last=rows[index];index+=1
            require(last['argv']==argv,'readiness final start order differs')
            if command['returncode']==1 or last['returncode']==1:
                require(last['returncode']==1 and not last['stdout'].strip() and not last['stderr'].strip(), 'readiness disappeared PID not proven')
                processes.setdefault(pid,[]).append(None);continue
            require(command['returncode']==last['returncode']==0 and not command['stderr'].strip() and not last['stderr'].strip()
                    and row['stdout'].strip()==last['stdout'].strip(),'readiness PID start changed')
            observed={'pid':pid,'started_at':row['stdout'].strip(),'argv':shlex.split(command['stdout'].strip())}
            require(observed in allowed,'readiness raw foreign PID/argv identity')
            processes.setdefault(pid,[]).append(observed)
        elif argv[:4]==['/usr/sbin/lsof','-nP','-a','-p']:
            require(row['returncode'] in (0,1) and not row['stderr'].strip(),'readiness listener inspection failed')
            pid=int(argv[4]);ports=[]
            for line in row['stdout'].splitlines():
                if line.startswith('p'):require(line=='p'+str(pid),'readiness foreign listener PID')
                elif line.startswith('n'):
                    found=re.fullmatch(r'n127\.0\.0\.1:(\d+)',line);require(found is not None,'readiness nonprivate listener')
                    ports.append(int(found[1]))
            pid_ports.setdefault(pid,[]).append(ports)
        elif argv[:2]==['/usr/sbin/lsof','-nP'] and argv[2].startswith('-iTCP:'):
            require(row['returncode'] in (0,1) and not row['stderr'].strip(),'readiness port inspection failed')
            port=int(argv[2][6:]);owners=[]
            for line in row['stdout'].splitlines():
                require(re.fullmatch(r'p[0-9]+',line) is not None,'readiness malformed port owners')
                owners.append(int(line[1:]))
            port_owners.setdefault(port,[]).append(sorted(set(owners)))
        else:raise ValueError('readiness unrelated native query')
    require(role in jobs and other in jobs and jobs[role][0][0] is True,'readiness role queries missing')
    known_ports={r:{files.json(Path(fixture['root'])/'preparation.json')['anchors'][r]['sentinel_port']} for r in ('provider','reservoir')}
    for name,view in ((role,target),(other,other_view)):
        loaded,published=jobs[name][-1];process=view['process'];old_id=plan['roles'][name]['identity']
        require(loaded is view['job_loaded'],'readiness parsed loaded state differs')
        if process is None:
            require(published is None or name==role and published==old_id['pid'] and processes.get(published,[])[-1:]==[None], 'readiness empty PID publication unproved')
        else:
            observations=processes.get(published,[])
            # The two consecutive original-worker reads can straddle natural
            # exit. This exact source-produced transition is the sole earlier
            # positive allowed; a holder/other role must match its latest read.
            draining=(name==role and process==old_id and sample['old_pid_observation'] is None
                      and observations==[old_id,None])
            require(published==process['pid'] and (observations[-1:]==[process] or draining),
                    'readiness raw process identity differs')
        owners=set()
        for port in known_ports[name]:
            require(port in port_owners,'readiness known sentinel query missing')
            owners.update(port_owners[port][-1])
        require(sorted(owners)==view['listener_pids'],'readiness parsed listener owners differ')
        if process is not None:
            require(process['pid'] in pid_ports,'readiness process listener query missing')
            found=pid_ports[process['pid']][-1]
            require(found in ([],list(known_ports[name])) if process==old_id else found==[], 'readiness extra process listeners')
    require(old['pid'] in processes and processes[old['pid']][-1]==sample['old_pid_observation'],'readiness old PID observation unproved')
    if role=='reservoir':require(processes.get(other_old['pid'],[])[-1:]==[None],'readiness parked provider absence unproved')
