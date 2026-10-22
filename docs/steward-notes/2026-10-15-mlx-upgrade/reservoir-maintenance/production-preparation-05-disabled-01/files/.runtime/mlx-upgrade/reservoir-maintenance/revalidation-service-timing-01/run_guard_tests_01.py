"""Root-released inert guard tests, no actual timing or target imports."""
import datetime, hashlib, importlib.abc, io, json
from pathlib import Path
import sys, time, unittest
ROOT=Path(__file__).resolve().parent
BAD={'numpy','torch','mlx','mlx_lm','cv2','coremltools','Foundation','Quartz','AVFoundation',
     'sounddevice','sklearn','reservoir_service','coupled_astrid_server'}
violations=[]
class ImportGuard(importlib.abc.MetaPathFinder):
    def find_spec(self,fullname,path=None,target=None):
        if fullname.split('.')[0] in BAD:
            violations.append(fullname);raise AssertionError('forbidden package '+fullname)
def audit(event,args):
    if event in {'subprocess.Popen','os.system','os.posix_spawn','os.fork','os.kill','os.killpg',
                 'socket.connect','socket.bind','socket.getaddrinfo'}:
        violations.append(event);raise AssertionError('forbidden external action '+event)
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
review=ROOT/'root-source-review-01.json'; bound=json.loads(review.read_text())['source_files']
bound[str(Path(__file__).resolve())]=sha(__file__)
assert all(sha(p)==h for p,h in bound.items())
sys.dont_write_bytecode=True;sys.path.insert(0,str(ROOT));sys.meta_path.insert(0,ImportGuard());sys.addaudithook(audit)
start=datetime.datetime.now(datetime.timezone.utc).isoformat();clock=time.monotonic();log=io.StringIO()
suite=unittest.defaultTestLoader.loadTestsFromName('test_timing_guard')
result=unittest.TextTestRunner(stream=log,verbosity=2).run(suite)
after={p:sha(p) for p in bound};loaded=sorted(n for n in sys.modules if n.split('.')[0] in BAD)
target=sorted(n for n in sys.modules if n in {'common','fixture','process_identity','result_checks','driver','review_matrix'})
passed=result.wasSuccessful() and not violations and not loaded and not target and bound==after
logpath=ROOT/'guard-tests-01.txt'
with logpath.open('x') as f:f.write(log.getvalue())
receipt={'schema':'reservoir_timing_guard_cpu_tests_v1','passed':passed,'tests':result.testsRun,
 'failures':len(result.failures),'errors':len(result.errors),'skipped':len(result.skipped),
 'started_at':start,'completed_at':datetime.datetime.now(datetime.timezone.utc).isoformat(),
 'elapsed_s':time.monotonic()-clock,'source_hashes_before':bound,'source_hashes_after':after,
 'violations':violations,'forbidden_modules':loaded,'target_modules':target,
 'log':{'path':str(logpath),'sha256':sha(logpath)},'actual_timing':False,'production_qualified':False}
p=ROOT/'guard-tests-01.json'
with p.open('x') as f:json.dump(receipt,f,indent=2,sort_keys=True);f.write('\n')
print(json.dumps({'passed':passed,'tests':result.testsRun,'errors':len(result.errors),'failures':len(result.failures),'receipt':str(p),'sha256':sha(p)}))
raise SystemExit(0 if passed else 1)
