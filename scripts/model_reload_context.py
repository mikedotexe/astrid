#!/usr/bin/env python3
"""Witness a planned Python source transition without rewriting loaded provenance.

Only reviewed model-source paths may differ. Interpreter and launch configuration
must still match the prior manifest. The resulting context is candidate-on-disk
inventory, not a claim that the current PID loaded it or permission to signal it.
"""
from __future__ import annotations
import argparse
import copy
import hashlib
import json
from pathlib import Path

SOURCE_NAMES = {'server': 'coupled_astrid_server.py', 'gateway': 'coupled_http_gateway.py',
                'logit-processor': 'mlx_reservoir.py', 'generation-controls': 'generation_controls.py'}


def transition_context(manifest, model_root):
    result = copy.deepcopy(manifest)
    artifacts = result.get('artifacts', {})
    if not {'server','gateway','logit-processor','python','launchd-plist'} <= set(artifacts):
        raise ValueError('prior coupled manifest is incomplete')
    transitions = {}
    for name, artifact in artifacts.items():
        path = Path(artifact['path'])
        actual = hashlib.sha256(path.read_bytes()).hexdigest()
        expected = artifact.get('sha256')
        if name in SOURCE_NAMES:
            if path.resolve() != (model_root / SOURCE_NAMES[name]).resolve():
                raise ValueError(f'unexpected source path for {name}')
            transitions[name] = dict(path=str(path), prior_manifest_sha256=expected,
                                     candidate_disk_sha256=actual, changed=expected != actual)
            artifact['sha256'] = actual
        elif actual != expected:
            raise ValueError(f'non-source artifact changed: {name}')
    result['source_transition'] = dict(schema='model_source_transition_v1', sources=transitions,
        relation='candidate_disk_inventory_not_loaded_source_confirmation',
        prior_manifest_sha256=hashlib.sha256(json.dumps(manifest,sort_keys=True).encode()).hexdigest(),
        prior_manifest_preserved=True, signal_authority=False)
    result['authority'] = 'candidate_inventory_witness_not_loaded_identity_or_deploy_authority'
    return result


def main():
    parser=argparse.ArgumentParser();parser.add_argument('--manifest',type=Path,required=True)
    parser.add_argument('--model-root',type=Path,required=True);parser.add_argument('--output',type=Path,required=True)
    args=parser.parse_args()
    source=json.loads(args.manifest.read_text())
    result=transition_context(source,args.model_root)
    with args.output.open('x') as out:
        out.write(json.dumps(result,indent=2)+'\n')

if __name__=='__main__':main()
