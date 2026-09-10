import hashlib
from pathlib import Path
import tempfile
import unittest
from model_reload_context import transition_context, SOURCE_NAMES

class ModelContextTests(unittest.TestCase):
    def test_only_source_drift_is_a_planned_transition(self):
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory);artifacts={}
            for name,file in dict(SOURCE_NAMES,python='python',**{'launchd-plist':'config.plist'}).items():
                path=root/file;path.write_text('old')
                artifacts[name]={'path':str(path),'sha256':hashlib.sha256(b'old').hexdigest()}
            original={'artifacts':artifacts}
            (root/'coupled_http_gateway.py').write_text('new')
            result=transition_context(original,root)
            self.assertTrue(result['source_transition']['sources']['gateway']['changed'])
            self.assertEqual(original['artifacts']['gateway']['sha256'],hashlib.sha256(b'old').hexdigest())
            self.assertFalse(result['source_transition']['signal_authority'])
            (root/'python').write_text('replacement')
            with self.assertRaisesRegex(ValueError,'non-source'):transition_context(original,root)

    def test_wrong_source_path_or_missing_prior_manifest_is_rejected(self):
        with self.assertRaisesRegex(ValueError,'incomplete'):transition_context({'artifacts':{}},Path('/tmp'))
