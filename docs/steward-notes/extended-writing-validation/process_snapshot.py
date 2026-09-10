"""Bounded read-only host observations, writing only this rollout's evidence."""
import argparse
from collections import Counter
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import re
import stat
import subprocess
import time

ROOT = Path('/Users/v/other/astrid')
WORK = ROOT / 'capsules/spectral-bridge/workspace'
OUT = Path(__file__).resolve().parent
SPOOL = WORK / 'provider_observations/20260908-live-01'
LABELS = ['com.astrid.spectral-bridge', 'com.minime.engine',
          'com.minime.autonomous-agent', 'com.minime.division-gateway',
          'com.minime.division-supervisor', 'com.reservoir.coupled-astrid',
          'com.minime.camera-client', 'com.minime.mic-to-sensory',
          'com.minime.host-sensory', 'com.minime.visual-frame-service',
          'com.minime.usb-hotplug-watchdog', 'com.reservoir.collab-feeder']

def sha(data):
    return hashlib.sha256(data).hexdigest()

def stable(path, cap=4*1024*1024):
    a = path.lstat()
    assert stat.S_ISREG(a.st_mode) and a.st_size <= cap, str(path)
    data = path.read_bytes()
    b = path.stat()
    assert (a.st_ino, a.st_size, a.st_mtime_ns) == (b.st_ino, b.st_size, b.st_mtime_ns)
    return data

def write(path, data):
    path.parent.mkdir(parents=True, exist_ok=True, mode=0o700)
    with path.open('xb') as f:
        os.chmod(path, 0o600)
        f.write(data)

def save(path, obj):
    write(path, (json.dumps(obj, indent=2, sort_keys=True)+'\n').encode())

def cmd(args):
    return subprocess.run(args, text=True, capture_output=True, timeout=15).stdout.strip()

def snapshot():
    result = {'captured_at': datetime.now(timezone.utc).isoformat(), 'processes': {}}
    for label in LABELS:
        job = cmd(['/bin/launchctl', 'print', f'gui/{os.getuid()}/{label}'])
        match = re.search(r'^\s*pid = (\d+)$', job, re.M)
        pid = int(match[1]) if match else None
        plist = Path.home() / f'Library/LaunchAgents/{label}.plist'
        result['processes'][label] = {
            'pid': pid,
            'started_at': cmd(['ps', '-p', str(pid), '-o', 'lstart=']) if pid else None,
            'executable': cmd(['ps', '-p', str(pid), '-o', 'comm=']) if pid else None,
            'plist_sha256': sha(stable(plist)) if plist.exists() else None,
        }
    for name, path in {'selection': ROOT/'.runtime/bridge-deployment/active.json',
                       'telemetry': WORK/'telemetry_heartbeat_delta_v1.json'}.items():
        raw = stable(path)
        value = json.loads(raw)
        if name == 'telemetry':
            value = {k:value.get(k) for k in ['latest_arrival_unix_s', 'active_connection_id', 'reconnect_count', 'disconnect_count']}
        result[name] = {'sha256':sha(raw), 'value':value}
    result['observer_overrides'] = {k:cmd(['/bin/launchctl','getenv',k]) for k in
        ['ASTRID_PROVIDER_OBSERVATION', 'ASTRID_PROVIDER_OBSERVATION_DIR']}
    result['log_offsets'] = {str(p):p.stat().st_size for p in [Path('/tmp/bridge.log'),Path('/tmp/bridge.err')] if p.exists()}
    return result

if __name__ == "__main__":
    import sys
    name = sys.argv[1]
    assert re.fullmatch(r"[a-z0-9-]+", name)
    result = snapshot()
    save(OUT / (name + ".json"), result)
    print(json.dumps({"snapshot": name, "processes": {k:v["pid"] for k,v in result["processes"].items()}, "selection": result["selection"]["value"]}, indent=2))
