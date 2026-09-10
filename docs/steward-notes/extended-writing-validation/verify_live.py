"""Read-only verification of the selected release and completed Minime reload."""
from pathlib import Path
from datetime import datetime, timezone
import hashlib,json,subprocess,sys
BASE=Path(__file__).resolve().parent
ASTRID=Path('/Users/v/other/astrid')
MINIME=Path('/Users/v/other/minime')
def sha(path):
 with path.open('rb') as f:return hashlib.file_digest(f,'sha256').hexdigest()
def read(path):return json.loads(path.read_text())
selection=read(ASTRID/'.runtime/bridge-deployment/active.json')
stage=BASE/'bridge-stage-01'
assert selection['stage']==str(stage)
manifest=read(stage/'manifest.json')
assert sha(stage/'manifest.json')==selection['manifest_sha256']
assert manifest['repository']['head']=='6f8aac6384232bc010d2776f13454eb8795bf3ae'
failure_path=Path(selection['transaction'])/'receipt.json'
failure=read(failure_path)
assert failure['status']=='failed_requires_review' and failure['activation_performed'] is False
recoveries=list((Path(selection['transaction'])/'stopped-transition-recoveries').glob('*/receipt.json'))
assert len(recoveries)==1
activation_path=recoveries[0]
activation=read(activation_path)
assert activation['status']=='transition_recovered'
assert activation['force_used'] is False and activation['signal_sent'] is False
assert activation['original_failure_sha256']==sha(failure_path)
assert not (ASTRID/'.runtime/bridge-deployment/hold.json').exists()
process=activation['new_process'];pid=process['pid']
assert process['new_saved_exchange_observed'] is True
assert subprocess.check_output(['ps','-p',str(pid),'-o','comm='],text=True).strip()==str(stage/'spectral-bridge-server')
assert subprocess.check_output(['ps','-p',str(pid),'-o','lstart='],text=True).strip()==process['started_at']
assert process['startup']['self_control']['state_targets_this_binary'] is True
artifacts={k:sha(Path(v['path'])) for k,v in manifest['artifacts'].items()}
assert all(artifacts[k]==v['sha256'] for k,v in manifest['artifacts'].items())
inputs=read(stage/'source-inputs.json')
assert sha(stage/'source-inputs.json')==manifest['source_inputs']['sha256']
assert all(sha(Path(row['path']))==row['sha256'] for row in inputs['files'])
release_root=BASE/'astrid'
main_matches=[]
recovery_tool_differences=[]
for row in inputs['files']:
 p=Path(row['path'])
 if p.is_relative_to(release_root):
  counterpart=ASTRID/p.relative_to(release_root)
  if sha(counterpart)!=row['sha256']:
   assert str(p.relative_to(release_root))=='scripts/bridge_stopped_recovery.py',str(counterpart)
   recovery_tool_differences.append({'path':str(counterpart),'staged_sha256':row['sha256'],'main_sha256':sha(counterpart),'reason':'Committed V3 stopped-recovery schema compatibility; frozen staged inputs unchanged.'})
  else: main_matches.append(str(counterpart))
assert subprocess.check_output(['git','rev-parse','--short=7','HEAD'],cwd=MINIME,text=True).strip()=='33c324d'
reload_path=BASE/'minime-reload.jsonl'
reload=[json.loads(line) for line in reload_path.read_text().splitlines()][-1]
assert reload['outcome']=='success' and reload['forced_termination'] is False
minime_pid=reload['new_pid'];status=read(MINIME/'workspace/runtime/autonomous_agent_source_status.json')
assert status['pid']==minime_pid and status['reload_required'] is False
assert status['source_inputs_at_start']['minime_autonomy/writing.py']==sha(MINIME/'minime_autonomy/writing.py')
assert subprocess.check_output(['ps','-p',str(minime_pid),'-o','lstart='],text=True).strip()==reload['new_started_at']
sys.path.insert(0,str(MINIME))
from minime_autonomy.source_study import selected_reader
assert selected_reader(ASTRID)==stage/'helpers/astrid-source-study'
assert reload['pre_signal']['continuity']['session_id']==reload['post_ready_continuity']['session_id']
pending=read(BASE/'minime-pending-next-continuity.json')
assert pending['verified'] is True
assert pending['pre_signal_sha256']==reload['pre_signal']['continuity']['pending_next_sha256']
assert sha(BASE/pending['log']['retained'])==pending['log']['sha256']
if pending.get('job'): assert pending['job']['worker_pid']==minime_pid
assert reload['post_ready_continuity']['cycle_count']>=reload['pre_signal']['continuity']['cycle_count']
before=read(BASE/'before-rollout.json');after=read(BASE/'after-reloads.json')
protected=[k for k in before['processes'] if k not in ['com.astrid.spectral-bridge','com.minime.autonomous-agent']]
assert all(before['processes'][k]==after['processes'][k] for k in protected)
assert before['observer_overrides']==after['observer_overrides']
result={'schema':'extended_writing_live_verification_v1','verified_at_utc':datetime.now(timezone.utc).isoformat(),'verified':True,
 'astrid_commit':manifest['repository']['head'],'minime_commit':subprocess.check_output(['git','rev-parse','HEAD'],cwd=MINIME,text=True).strip(),
 'manifest_sha256':selection['manifest_sha256'],'artifacts':artifacts,'source_count':len(inputs['files']),'main_matching_source_inputs':len(main_matches),'recovery_tool_differences':recovery_tool_differences,'main_at_verification':subprocess.check_output(['git','rev-parse','HEAD'],cwd=ASTRID,text=True).strip(),
 'bridge':{'old_pid':failure['old_pid'],'new_pid':pid,'started_at':process['started_at'],'checkpoint':process['startup']['checkpoint'],'new_saved_exchange_observed':True,'state_targets_this_binary':True,'activation_receipt':str(activation_path),'activation_receipt_sha256':sha(activation_path),'original_failure_receipt':str(failure_path),'original_failure_sha256':sha(failure_path)},
 'minime':{'old_pid':reload['old_pid'],'new_pid':minime_pid,'started_at':reload['new_started_at'],'receipt':str(reload_path),'receipt_sha256':sha(reload_path),'source_inputs':reload['source_inputs'],'continuity_before':reload['pre_signal']['continuity'],'continuity_after':reload['post_ready_continuity'],'selected_reader':str(selected_reader(ASTRID)),'pending_next_outcome':pending['outcome'],'pending_next_evidence':str(BASE/'minime-pending-next-continuity.json'),'pending_next_evidence_sha256':sha(BASE/'minime-pending-next-continuity.json')},
 'protected_services_unchanged':protected,'scope':'Verified release and process continuity; no induced study, no comprehension or felt-benefit inference.'}
(BASE/'live-verification.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps({k:v for k,v in result.items() if k not in ['minime','bridge','artifacts']},indent=2))
print('bridge',failure['old_pid'],'->',pid,'minime',reload['old_pid'],'->',minime_pid)
