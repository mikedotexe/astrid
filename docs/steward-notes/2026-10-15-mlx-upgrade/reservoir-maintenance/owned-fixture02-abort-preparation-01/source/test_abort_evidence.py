"""Pure retained-evidence failure cases with private synthetic files only."""
from __future__ import annotations
import copy
import hashlib
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import abort_evidence as e


class EvidenceTests(unittest.TestCase):
    def setUp(self):
        self.tmp=tempfile.TemporaryDirectory();self.addCleanup(self.tmp.cleanup)
        self.root=Path(self.tmp.name).resolve()
        self.t={'run_root':str(self.root),'request_id':'one',
                'workers':{'provider':{'pid':9267,'started_at':'old','argv':['python','worker','provider']},
                           'reservoir':{'pid':9217,'started_at':'old','argv':['python','worker','reservoir']}}}
        self.provider=self.rows('provider',['started','idle','generation_admitted','stop_received','checkin_sent','checkin_acknowledged','drained'],[0,1,2,3,9,10,11])
        self.provider[0].update(argv=['worker','provider'],cwd=str(self.root))
        for row in self.provider[2:]:row['request_id']='one'
        self.provider[2]['ticks']=self.provider[5]['ticks']=11
        self.reservoir=self.rows('reservoir',['started','restored','checkin_applied'],[0,1,9.5])
        self.reservoir[0].update(argv=['worker','reservoir'],cwd=str(self.root))
        self.reservoir[1]['state']={'fixture_astrid':{'ticks':0},'fixture_minime':{'ticks':7}}
        self.reservoir[2].update(state=e.STATE,request_id='one')
        self.write_json('request.one.json',{'token':e.TOKEN,'request_id':'one','handle':'fixture_astrid','ticks':11,'provider_pid':9267})
        self.write_json('ack.one.json',{'token':e.TOKEN,'request_id':'one','ticks':11,'ok':True})
        self.logs()
    def rows(self,role,phases,times):
        return [{'phase':phase,'role':role,'token':e.TOKEN,'pid':self.t['workers'][role]['pid'],'monotonic':stamp}
                for phase,stamp in zip(phases,times,strict=True)]
    def write_json(self,name,value):(self.root/name).write_text(json.dumps(value)+'\n')
    def logs(self):
        for role,rows in [('provider',self.provider),('reservoir',self.reservoir)]:
            (self.root/(role+'.events.jsonl')).write_text(''.join(json.dumps(row)+'\n' for row in rows))
    def saved(self):
        self.reservoir+=self.rows('reservoir',['stop_received','final_snapshot_started','handle_saved','handle_saved','final_snapshot_complete','drained'],[20,26,27,28,29,30])
        self.reservoir[4]['handles']=sorted(e.STATE)
        for row,(name,value) in zip(self.reservoir[5:7],sorted(e.STATE.items()),strict=True):row.update(name=name,value=value)
        self.reservoir[7]['state']=e.STATE;self.logs();self.write_json('state.json',e.STATE)
    def test_one_checkin_preserved_and_complete_save(self):
        self.assertTrue(e.checkin_proof(self.t)['one_unique_checkin']);self.saved()
        self.assertTrue(e.checkin_proof(self.t,saved=True)['saved'])
    def test_second_checkin_is_refused(self):
        self.write_json('request.second.json',{})
        with self.assertRaisesRegex(ValueError,'extra request'):e.checkin_proof(self.t)
    def test_ack_does_not_substitute_for_applied_state(self):
        self.reservoir[2]['state']={'fixture_astrid':{'ticks':0},'fixture_minime':{'ticks':7}};self.logs()
        with self.assertRaisesRegex(ValueError,'applied check-in'):e.checkin_proof(self.t)
    def test_infinite_timestamp_from_json_exponent_is_refused(self):
        path=self.root/'provider.events.jsonl'
        path.write_text(path.read_text().replace('"monotonic": 11','"monotonic": 1e999'))
        with self.assertRaisesRegex(ValueError,'timestamps'):e.checkin_proof(self.t)
    def test_short_final_save_delay_is_refused(self):
        self.saved();self.reservoir[4]['monotonic']=25;self.logs()
        with self.assertRaisesRegex(ValueError,'durable11/7'):e.checkin_proof(self.t,saved=True)
    def test_missing_or_reordered_handle_cannot_pass_complete_save(self):
        self.saved();self.reservoir[5],self.reservoir[6]=self.reservoir[6],self.reservoir[5]
        self.reservoir[5]['monotonic']=27;self.reservoir[6]['monotonic']=28;self.logs()
        with self.assertRaisesRegex(ValueError,'durable11/7'):e.checkin_proof(self.t,saved=True)
    def test_wrong_final_disk_state_is_refused(self):
        self.saved();self.write_json('state.json',{'fixture_astrid':{'ticks':22},'fixture_minime':{'ticks':7}})
        with self.assertRaisesRegex(ValueError,'durable11/7'):e.checkin_proof(self.t,saved=True)
    def test_partial_jsonl_tail_is_refused(self):
        path=self.root/'reservoir.events.jsonl';path.write_bytes(path.read_bytes().rstrip(b'\n'))
        with self.assertRaisesRegex(ValueError,'partial'):e.checkin_proof(self.t)
    def test_duplicate_keys_are_refused(self):
        with self.assertRaisesRegex(ValueError,'duplicate'):e.decode(b'{"ok":true,"ok":false}')
    def test_retargeted_selected_python_alias_is_refused_before_hashing(self):
        def family(name):
            root=self.root/name/'Python.framework/Versions/3.14'
            wrapper=root/'bin/python3.14';runtime=root/'Resources/Python.app/Contents/MacOS/Python'
            wrapper.parent.mkdir(parents=True);runtime.parent.mkdir(parents=True)
            wrapper.write_bytes(b'wrapper');runtime.write_bytes(b'runtime');return wrapper,runtime
        first=family('original');second=family('other');alias=self.root/'python';alias.symlink_to(second[0])
        t={'python':str(alias),'python_files':{str(p):'digest' for p in first}}
        with patch.object(e,'sha') as hashed,self.assertRaisesRegex(ValueError,'retargeted'):
            e.verify_sources(t,{})
        hashed.assert_not_called()
    def test_snapshot_does_not_allow_unlisted_old_root_writes(self):
        archive=self.root/'archive';archive.mkdir(mode=0o700);(archive/'files').mkdir(mode=0o700)
        run=self.root/'run';run.mkdir(mode=0o700);entries=[]
        for index in range(141):
            name=f'{index}.json';data=b'{}\n';(run/name).write_bytes(data);(archive/'files'/name).write_bytes(data)
            entries.append({'source':str(run/name),'snapshot':'files/'+name,'sha256':hashlib.sha256(data).hexdigest(),
                            'bytes':len(data),'stable_during_read':True})
        t={'run_root':str(run),'snapshot':{'path':str(archive/'manifest.json')}}
        (run/'foreign.json').write_text('{}')
        with self.assertRaisesRegex(ValueError,'record set changed'):e.verify_original(t,{'entries':entries})


if __name__=='__main__':unittest.main()
