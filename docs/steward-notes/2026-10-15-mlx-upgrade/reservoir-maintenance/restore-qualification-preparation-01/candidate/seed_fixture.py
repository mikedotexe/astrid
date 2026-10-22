"""Private synthetic writer child; no numerical import before explicit opt-in."""
from __future__ import annotations

import argparse
from collections import deque
import hashlib
import importlib.abc
import json
import os
from pathlib import Path
import sys
import time

import qualify_restore as q


class CpuOnly(importlib.abc.MetaPathFinder):
    def find_spec(self, fullname, path=None, target=None):
        if fullname.split('.')[0] in {'mlx', 'mlx_lm', 'mlx_reservoir', 'coremltools'}:
            raise ImportError('GPU/model import forbidden in fixture: ' + fullname)


def deny_external(event, args):
    if event in {'socket.bind', 'socket.connect', 'socket.sendto', 'subprocess.Popen'}:
        raise RuntimeError('external activity forbidden in synthetic seed: ' + event)


def execute(output, inputs_path):
    output = Path(output)
    q.require(inputs_path == q.INPUTS and output == output.resolve() and output.name == 'seed'
              and output.parent.parent == q.HERE / 'runs', 'fixed private seed output required')
    guard = q.read(q.HERE / 'launch-guard.json')
    q.require(guard['run_id'] == output.parent.name, 'seed lacks owned launch guard')
    q.check_ref(guard['release'])
    inputs = q.check(); q.validate_release(Path(guard['release']['path']), inputs, guard['run_id'])
    q.require(sys.executable == q.PYTHON and all(os.environ.get(key) == '1' for key in q.THREADS),
              'exact original interpreter and single CPU thread environment required')
    q.write(output / 'seed-execution-guard.json', {'pid': os.getpid(), 'run_id': guard['run_id'],
            'scope': 'synthetic_cpu_fixture_only', 'automatic_retry_permitted': False})
    v = q.load_verifier(); v.check_runtime({'runtime_manifest': inputs['runtime_manifest']})
    q.require(not any(name.split('.')[0] in {'numpy', 'torch', 'reservoir_service', 'mlx', 'coremltools'}
                      for name in sys.modules), 'fresh numeric child required')
    record = {'schema': 'reservoir_restore_seed_v1', 'passed': False, 'production_qualified': False,
              'scope': 'synthetic_cpu_fixture_only', 'pid': os.getpid(), 'python': sys.executable,
              'started_wall': time.time(), 'inputs': q.ref(inputs_path), 'errors': []}
    try:
        sys.meta_path.insert(0, CpuOnly()); sys.addaudithook(deny_external)
        sys.path.insert(0, inputs['source_manifest']['root'])
        sys.pycache_prefix = str(output / 'unused-bytecode')
        import numpy as np
        import torch
        torch.set_num_threads(1); torch.set_num_interop_threads(1); torch.set_default_device('cpu')
        import reservoir_service as module
        q.require(not any(name.split('.')[0] in {'mlx', 'mlx_lm', 'mlx_reservoir', 'coremltools'}
                          for name in sys.modules), 'GPU/model module imported')
        service = module.ReservoirService(state_dir=output / 'state', **inputs['config'])
        for filename in inputs['source_manifest']['files']:
            loaded = sys.modules.get(filename[:-3])
            q.require(loaded is not None and Path(loaded.__file__).resolve() == Path(inputs['source_manifest']['root']) / filename,
                      'source import escaped pinned private closure')
        expected_startup, sequences, thermostats, fields = {}, {}, {}, {}
        for index, (name, backend) in enumerate((('qualification_numpy', 'numpy'), ('qualification_shadow__lsm', 'lsm_shadow'))):
            info = service.create_handle(name, entity='synthetic-' + backend, backend=backend)
            layers = [(np.arange(192, dtype=np.float32).reshape(1, 192) / (2000 + 100 * layer)
                       + np.float32(.01 * (index + 1))) for layer in range(3)]
            service.bridge.set_state(name, *layers)
            info.tick_count = 123 + index; info.event_seq = 10 + index
            info.last_tick_time = time.monotonic() - (90 + index)
            info.last_live_meta = {'event_seq': 14 + index, 'source': 'synthetic-fixture',
                                   'nested': {'valid': [True, None, .125]}}
            info.last_lane_meta = {'feeder': dict(info.last_live_meta), 'synthetic': {'event_seq': 11}}
            info.provenance_ring = deque([{'event_seq': n + index, 'source': 'synthetic'} for n in range(11, 20)], maxlen=12)
            # Deliberately longer stored rings exercise the loader's documented tails.
            info.output_ring = deque([n / 128 for n in range(70)], maxlen=80)
            info.h_norm_ring = deque([[n / 128, n / 256, n / 512] for n in range(70)], maxlen=80)
            rs = service.rehearsal.get_state(name)
            rs.last_live_input = np.arange(32, dtype=np.float32) / 64
            rs.last_live_time = info.last_tick_time
            rs.mode = ('rehearse', 'hold')[index]; rs.decay_profile = ('slow', 'fast')[index]
            rs.decay_weight = .375 + index / 8; rs.hint_policy = 'guarded'; rs.mode_authority = 'explicit'
            for layer, thermostat in enumerate(info.thermostats):
                thermostat.entropy_target = .01 if layer < 2 else None
                thermostat.rho = .97; thermostat.tick = 1001 + layer
                thermostat.last_entropy = .45; thermostat.last_saturation = .02
                thermostat._warmup_entropies = [n / 256 for n in range(53)]
                thermostat.buffer = deque([np.full(192, (row + layer + 1) / 1000, dtype=np.float32)
                                           for row in range(53)], maxlen=200)
            service.snapshot_handle(name)  # Unmodified real public persistence writer.
            expected_startup[name] = {'entity': info.entity, 'backend': backend,
                                      'tick_count': info.tick_count, 'mode': rs.mode}
            sequences[name] = 19 + index
            stored_thermostats = q.read(output / 'state' / (name + '_thermostats.json'))
            for layer, thermostat in enumerate(stored_thermostats):
                if thermostat['entropy_target'] is not None:
                    thermostat['entropy_target'] = max(thermostat['entropy_target'], module.LAYER_H_TARGET_FLOORS[layer])
            thermostats[name] = v.state.canonical_hash(stored_thermostats)
            with np.load(output / 'state' / (name + '.npz'), allow_pickle=False) as archive:
                raw = {key: archive[key] for key in archive.files}
            q.require(set(raw) == v.NPZ_KEYS, 'actual writer field coverage changed')
            fields[name] = {key: {'dtype': str(array.dtype), 'shape': list(array.shape),
                                  'sha256': hashlib.sha256(array.tobytes()).hexdigest()}
                            for key, array in raw.items()}
            if index == 0:
                raw['snapshot_version'] = np.array(module.SNAPSHOT_SCHEMA_VERSION + 1, dtype=np.int64)
                np.savez(output / 'schema-mismatch.npz', **raw)
        (output / 'state' / 'quarantine').mkdir()
        (output / 'state' / 'quarantine' / 'synthetic-retained.txt').write_text('Historical synthetic artifact; keep exact bytes.\n')
        record.update(expected_startup=expected_startup, event_seq=sequences, source_fields=fields,
                      thermostat_semantics_sha256=thermostats,
                      source_files={name: str(Path(inputs['source_manifest']['root']) / name)
                                    for name in inputs['source_manifest']['files']},
                      inventory=v.state.inventory_tree(output / 'state'),
                      torch_device=str(torch.get_default_device()), torch_threads=torch.get_num_threads(),
                      tasks_started=False, listener_opened=False, passed=True)
        q.check()
    except BaseException as error:
        record['passed'] = False
        record['errors'].append(type(error).__name__ + ': ' + str(error))
    finally:
        record['completed_wall'] = time.time()
        q.write(output / 'seed-receipt.json', record)
    return record


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--execute-fixture', action='store_true')
    parser.add_argument('--inputs', type=Path, default=q.INPUTS)
    parser.add_argument('--output', type=Path)
    args = parser.parse_args(argv)
    if not args.execute_fixture:
        print(json.dumps({'passed': False, 'status': 'not_executed', 'production_qualified': False}))
        return 0
    q.require(args.output is not None, 'private output required')
    result = execute(args.output, args.inputs)
    return 0 if result['passed'] else 1


if __name__ == '__main__':
    raise SystemExit(main())
