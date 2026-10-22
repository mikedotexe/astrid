"""Explicit one-shot fixture argv recorder. Never opens its inert --model path."""
from __future__ import annotations

import argparse
import os
from pathlib import Path
import sys
import time

# The copied record helper is source-bound; do not leave a private bytecode cache.
sys.dont_write_bytecode = True
from owned_fixture_records import read, require, write


def record(root, token, model, output):
    root = Path(root); output = Path(output); model = Path(model)
    owner = read(root / 'fixture.owner.json')
    require(owner == {'schema': 'reservoir_maintenance_owned_fixture_v1', 'token': token,
                     'root': str(root), 'uid': os.getuid()}, 'pin owner differs')
    require(model == root / 'pin/model-b.sentinel' and output == root / 'pin/sentinel-exit-intent.json'
            and root == root.resolve(strict=True), 'pin target is outside the exact private fixture')
    write(output, {'schema': 'owned_fixture_pin_argv_v1', 'pid': os.getpid(), 'token': token,
                   'argv': [sys.executable, *sys.argv], 'model_argument': str(model),
                   'recorded_wall_time_ns': time.time_ns(), 'model_opened': False,
                   'numeric_imported': False, 'listener_opened': False,
                   'scope': 'private_pin_primitive_only', 'production_qualified': False})
    return 0


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--root', type=Path, required=True)
    parser.add_argument('--token', required=True)
    parser.add_argument('--model', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    return record(args.root, args.token, args.model, args.output)


if __name__ == '__main__': raise SystemExit(main())
