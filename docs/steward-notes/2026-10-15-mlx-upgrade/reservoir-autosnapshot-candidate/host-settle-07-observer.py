import os,sys,time,json,re,subprocess,tempfile,urllib.request,hashlib
from pathlib import Path
from datetime import datetime,timezone
# This finite read-only window is for the private CPU fixture, not MLX acceptance.
for completed_input in (".runtime/mlx-upgrade/observations-post-perception-01/20261016T012057Z/soak_result.json", ".runtime/perception-memory-investigation/passive-rusage-02.json"):
 if json.loads(Path(completed_input).read_text()).get("state") != "completed":
  raise RuntimeError("Both retained passive observations must be completed first")
output=Path(".runtime/mlx-upgrade/reservoir-snapshot-candidate/host-settle-07.json")
fd=os.open(output,os.O_CREAT|os.O_EXCL|os.O_WRONLY,0o600);os.close(fd)
def run(args,timeout=5):
 r=subprocess.run(args,capture_output=True,text=True,timeout=timeout,check=False)
 if r.returncode: raise RuntimeError(f"{args[0]} exited {r.returncode}: {r.stderr.strip()[:200]}")
 return r.stdout
def process_rows():
 rows=[]
 for line in run(["ps","-axo","pid=,ppid=,pcpu=,rss=,lstart=,comm="]).splitlines():
  values=line.split(None,9)
  if len(values)!=10:continue
  try:rows.append({"pid":int(values[0]),"ppid":int(values[1]),"cpu_percent":float(values[2]),"rss_bytes":int(values[3])*1024,"started_local":" ".join(values[4:9]),"executable":values[9]})
  except ValueError:continue
 return rows
def identities():
 rows={r["pid"]:r for r in process_rows()}
 bindings_path=Path('/Users/v/other/astrid/.runtime/mlx-upgrade/reservoir-snapshot-candidate/host-settle-07-bindings.json')
 if hashlib.sha256(bindings_path.read_bytes()).hexdigest()!='beae54c2362ea251c6b5a1fc9dbdf78532fa65b498796b97c7515c2c7cd5f629':raise RuntimeError("host-window input binding changed")
 bindings=json.loads(bindings_path.read_text())
 expected=bindings["protected_processes"]
 for path,digest in bindings["identity_files"].items():
  if hashlib.sha256(Path(path).read_bytes()).hexdigest()!=digest:raise RuntimeError("protected input changed: "+path)
 exact={pid:run(["/bin/ps","-ww","-p",pid,"-o","pid=,ppid=,lstart=,command="]).strip() for pid in expected}
 if exact!=expected:raise RuntimeError("protected exact PID/start/argv identity changed")
 selected={"protected_exact":exact,"identity_files":bindings["identity_files"]}
 job=run(["launchctl","print",f"gui/{os.getuid()}/com.minime.engine"])
 m=re.search(r"(?m)^\s*pid = (\d+)\s*$",job)
 if not m or int(m.group(1))!=35303:raise RuntimeError("native Minime engine launchd PID changed")
 selected["minime_engine"]={k:rows.get(35303,{}).get(k) for k in ("pid","started_local","executable")}
 for name,port in (("astrid_provider",8090),("astrid_reservoir",7881),("ollama",11434)):
  found=subprocess.run(["lsof","-nP",f"-iTCP:{port}","-sTCP:LISTEN","-t"],capture_output=True,text=True,timeout=5)
  if found.returncode != 0 or found.stderr.strip():raise RuntimeError("listener inspection failed: "+name)
  pids=sorted(set(int(v) for v in found.stdout.split() if v.isdigit()))
  if pids != [bindings["expected_listener_pids"][str(port)]]:raise RuntimeError("protected listener changed: "+name)
  selected[name]={"port":port,"pids":pids,"processes":[{k:rows[p].get(k) for k in ("pid","started_local","executable")} for p in pids if p in rows]}
 job=run(["launchctl","print",f"gui/{os.getuid()}/com.minime.autonomous-agent"])
 m=re.search(r"(?m)^\s*pid = (\d+)\s*$",job)
 if not m or int(m.group(1))!=bindings["expected_launchd_pids"]["com.minime.autonomous-agent"]:raise RuntimeError("Minime agent launchd PID changed")
 selected["minime_agent"]={k:rows.get(int(m.group(1)),{}).get(k) for k in ("pid","started_local","executable")}
 job=run(["launchctl","print",f"gui/{os.getuid()}/com.astrid.spectral-bridge"])
 m=re.search(r"(?m)^\s*pid = (\d+)\s*$",job)
 if m:
  p=int(m.group(1))
  if p!=bindings["expected_launchd_pids"]["com.astrid.spectral-bridge"]:raise RuntimeError("bridge launchd PID changed")
  selected["spectral_bridge"]={k:rows.get(p,{}).get(k) for k in ("pid","started_local","executable")}
 else:raise RuntimeError("bridge launchd PID missing")
 job=run(["launchctl","print",f"gui/{os.getuid()}/com.astrid.perception-host-ascii"])
 m=re.search(r"(?m)^\s*pid = (\d+)\s*$",job)
 if not m or int(m.group(1))!=22354:raise RuntimeError("perception PID differs from verified restart22354")
 selected["astrid_perception"]={k:rows.get(22354,{}).get(k) for k in ("pid","started_local","executable")}
 return selected
def host_sample():
 row={"t_s":time.time(),"time_utc":datetime.now(timezone.utc).isoformat()}
 try:
  raw=run(["sysctl","kern.memorystatus_vm_pressure_level","vm.swapusage"])
  pressure=re.search(r"memorystatus_vm_pressure_level:\s*(\d+)",raw)
  swap=re.search(r"used\s*=\s*([\d.]+)([KMGT])",raw)
  if not pressure or not swap:raise ValueError("unparseable sysctl output")
  row["pressure_level"]=int(pressure.group(1));row["swap_bytes"]=float(swap.group(1))*1024**("KMGT".index(swap.group(2))+1)
 except Exception as e:row["memory_error"]=str(e)
 try:
  raw=run(["vm_stat"]);row["vm_stat_raw"]=raw
  page=int(re.search(r"page size of (\d+) bytes",raw).group(1))
  names={"free_bytes":"Pages free","inactive_bytes":"Pages inactive","speculative_bytes":"Pages speculative","wired_bytes":"Pages wired down","compressor_bytes":"Pages occupied by compressor","compressed_logical_bytes":"Pages stored in compressor"}
  row["vm_stat"]={"page_size_bytes":page}
  for key,label in names.items():row["vm_stat"][key]=int(re.search(re.escape(label)+r":\s*(\d+)",raw).group(1))*page
 except Exception as e:row["vm_stat_error"]=str(e)
 try:
  cpu=run(["/usr/bin/top","-l","2","-s","1","-n","0"],timeout=6)
  lines=[line for line in cpu.splitlines() if line.startswith("CPU usage:")]
  row["system_cpu_raw"]=lines[-1]
  m=re.search(r"CPU usage:\s*([\d.]+)% user,\s*([\d.]+)% sys,\s*([\d.]+)% idle",lines[-1])
  if not m:raise ValueError("unparseable top CPU")
  row["system_cpu"]={key:float(value) for key,value in zip(("user_percent","system_percent","idle_percent"),m.groups())}
 except Exception as e:row["system_cpu_error"]=str(e)
 try:
  current_processes=process_rows()
  row["perception_process"]=[p for p in current_processes if p["pid"]==22354]
  row["screen_sharing_processes"]=[r for r in current_processes if any(part in Path(r["executable"]).name.lower() for part in ("screensharing","screen sharing","remotedesktop","remote desktop","ardagent"))]
  row["build_test_processes"]=[r for r in current_processes if Path(r["executable"]).name in ("cargo","rustc") or re.search(r"/target/(?:[^/]+/)?(?:debug|release)/deps/[^/]+$",r["executable"])]
 except Exception as e:row["process_error"]=str(e)
 try:
  with urllib.request.urlopen("http://127.0.0.1:8090/readyz",timeout=3) as response:
   row["readyz"]={"http_status":response.status,"body":json.load(response)}
 except Exception as e:row["readyz"]={"error":str(e)}
 return row
record={"schema":"reservoir_cpu_fixture_host_window_v1","input_binding_path":'/Users/v/other/astrid/.runtime/mlx-upgrade/reservoir-snapshot-candidate/host-settle-07-bindings.json',"input_binding_sha256":'beae54c2362ea251c6b5a1fc9dbdf78532fa65b498796b97c7515c2c7cd5f629',"protected_file_count":237,"steward_pause_generation":528,"purpose":"Private real-service CPU fixture settling; not MLX upgrade qualification","accepted_mlx_baseline":False,"observer_pid":os.getpid(),"expected_perception_pid":22354,"perception_restart_receipt":{"path":"/Users/v/other/astrid/.runtime/mlx-upgrade/maintenance/perception-recover-01.json","sha256":"5fdc6bdccfbaf5acf37e1bdf8e25f95e307a225f284428712bbcaf2a2e8bc6a6"},"previous_observation":"/Users/v/other/astrid/.runtime/mlx-upgrade/reservoir-snapshot-candidate/host-settle-05.json","observer_source_sha256":hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),"final_cpu_receipt_sha256":hashlib.sha256(Path("docs/steward-notes/2026-10-15-mlx-upgrade/focused-tests-final-07.json").read_bytes()).hexdigest(),"screen_sharing_scope":"Matching process identity, CPU and RSS only; process presence or absence does not conclusively establish whether a remote session is active.","started_at":datetime.now(timezone.utc).isoformat(),"duration_requested_s":610,"interval_s":5,"state":"running","production_identity_start":identities(),"samples":[],"window_rule":">=300 consecutive observed seconds of pressure1, swap not above window's first sample, no cargo/rustc/target test process; sample gaps >15s invalidate","cpu_method":"second sample of top -l2 -s1 -n0; percentages across host; process ps %cpu is OS lifetime estimate","production_mutations":0,"gpu_workloads_started":0}
def save():
 descriptor,name=tempfile.mkstemp(prefix=".host-window-",dir=output.parent)
 try:
  with os.fdopen(descriptor,"w") as f:
   json.dump(record,f,indent=2,sort_keys=True);f.write("\n");f.flush();os.fsync(f.fileno())
  os.replace(name,output)
 finally:
  if os.path.exists(name):os.unlink(name)
started=time.monotonic();next_due=started;window_start=None;window_swap=None;max_window=0.;last_t=None;max_window_indices=None;first_index=None
try:
 while True:
  remaining=next_due-time.monotonic()
  if remaining>0:time.sleep(remaining)
  row=host_sample()
  try:
   observed_identity=identities()
   row["production_identity_unchanged"]=observed_identity==record["production_identity_start"]
   if not row["production_identity_unchanged"]:row["identity_error"]="protected runtime identity changed"
  except Exception as e:row["production_identity_unchanged"]=False;row["identity_error"]=str(e)
  row["elapsed_s"]=time.monotonic()-started
  record["samples"].append(row);idx=len(record["samples"])-1
  clean=row.get("pressure_level")==1 and row.get("build_test_processes")==[] and "memory_error" not in row and "process_error" not in row
  ready=row.get("readyz",{})
  clean=clean and row.get("production_identity_unchanged") is True and ready.get("http_status")==200 and ready.get("body",{}).get("ready") is True and ready.get("body",{}).get("reservoir",{}).get("status")=="connected"
  gap=last_t is not None and row["t_s"]-last_t>15
  if not clean or gap:
   window_start=window_swap=first_index=None
  elif window_start is None or row["swap_bytes"]>window_swap:
   window_start,window_swap,first_index=row["t_s"],row["swap_bytes"],idx
  current=0 if window_start is None else row["t_s"]-window_start
  if current>max_window:max_window=current;max_window_indices=[first_index,idx]
  record["current_eligible_window_s"]=current;record["longest_eligible_window_s"]=max_window
  record["longest_eligible_window_sample_indices"]=max_window_indices
  record["five_minute_eligible_window_observed"]=max_window>=300
  last_t=row["t_s"];save()
  if max_window>=300 and not record.get("eligible_window_notified"):
   record["eligible_window_notified"]=True;record["first_eligible_window_observed_at"]=datetime.now(timezone.utc).isoformat();save();print(json.dumps({"event":"five_minute_eligible_window","observed_at":record["first_eligible_window_observed_at"],"duration_s":max_window,"path":str(output)}),flush=True)
  if current>=300 or time.monotonic()-started>=610:break
  next_due+=5
 record["production_identity_end"]=identities()
 record["production_identity_unchanged"]=record["production_identity_start"]==record["production_identity_end"]
 record["state"]="completed";record["duration_s"]=time.monotonic()-started;record["completed_at"]=datetime.now(timezone.utc).isoformat()
 save()
 print(json.dumps({"state":record["state"],"duration_s":record["duration_s"],"samples":len(record["samples"]),"longest_eligible_window_s":max_window,"five_minute_eligible_window_observed":max_window>=300,"production_identity_unchanged":record["production_identity_unchanged"],"path":str(output)}),flush=True)
except BaseException as e:
 record["state"]="failed";record["error"]=str(e);record["completed_at"]=datetime.now(timezone.utc).isoformat();save();raise

