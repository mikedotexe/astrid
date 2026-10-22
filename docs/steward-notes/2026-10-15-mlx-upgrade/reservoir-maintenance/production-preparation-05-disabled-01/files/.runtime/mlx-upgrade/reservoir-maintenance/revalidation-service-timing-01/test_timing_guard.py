"""Inert stdlib guard checks; no target module, payload or OS control execution."""
import importlib.util
import os
from pathlib import Path
import stat
import sys
import tempfile
import time
import types
import unittest
from unittest.mock import patch

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location('inert_timing_wrapper', HERE/'time_service_review.py')
t = importlib.util.module_from_spec(spec)
spec.loader.exec_module(t)


def metadata(path):
    link=path.lstat(); s=path.stat()
    return {'bytes':s.st_size,'canonical_path':str(path.resolve(strict=True)),
            'ctime_ns':s.st_ctime_ns,'device':s.st_dev,'inode':s.st_ino,'links':s.st_nlink,
            'lstat_inode':link.st_ino,'mode':stat.S_IMODE(s.st_mode),'mtime_ns':s.st_mtime_ns,
            'path':str(path),'regular':stat.S_ISREG(s.st_mode),'symlink':stat.S_ISLNK(link.st_mode),'uid':s.st_uid}


class GuardTests(unittest.TestCase):
    def setUp(self):
        self.tmp=tempfile.TemporaryDirectory(); self.addCleanup(self.tmp.cleanup)
        self.path=Path(self.tmp.name).resolve()/'inert.txt'
        self.path.write_bytes(b'tiny inert content\n')
        self.meta=metadata(self.path)

    def guard(self):
        g=t.Guard(time.monotonic()+30)
        g.phase='review'; g.allowed={str(self.path):self.meta}; g.maximum={str(self.path):1}
        return g

    def opened(self,g):
        g.audit('open',(str(self.path),'rb',os.O_RDONLY))

    def test_actual_pathlib_read_bytes_and_text_profile_accounting(self):
        for method in ('read_bytes','read_text'):
            g=self.guard(); self.opened(g)
            old=sys.getprofile()
            try:
                sys.setprofile(g.profile)
                got=getattr(self.path,method)()
            finally: sys.setprofile(old)
            self.assertTrue(got)
            self.assertEqual(g.opens,g.reads)
            self.assertEqual(sum(g.reads.values()),1)
            self.assertEqual(g.logical_bytes,self.meta['bytes'])

    def test_changed_file_metadata_is_refused(self):
        self.path.write_bytes(b'changed and larger content')
        with self.assertRaisesRegex(t.Refusal,'metadata changed'): self.opened(self.guard())

    def test_unknown_path_and_second_open_refused(self):
        g=self.guard()
        with self.assertRaisesRegex(t.Refusal,'unbudgeted'): g.audit('open',(str(self.path)+'x','rb',0))
        g=self.guard(); self.opened(g)
        with self.assertRaisesRegex(t.Refusal,'per-file'): self.opened(g)

    def test_write_descriptor_open_and_control_are_refused(self):
        for args in ((str(self.path),'wb',os.O_WRONLY),(4,'rb',os.O_RDONLY)):
            with self.assertRaises(t.Refusal): self.guard().audit('open',args)
        for event in ('subprocess.Popen','os.kill','socket.connect','ctypes.dlopen','os.rename'):
            with self.assertRaises(t.Refusal): self.guard().audit(event,())

    def test_byte_visit_and_individual_file_budgets_refuse(self):
        for constant in ('BYTE_LIMIT','FILE_LIMIT','VISIT_LIMIT'):
            with patch.object(t,constant,0),self.assertRaisesRegex(t.Refusal,'ceiling'): self.opened(self.guard())

    def test_descriptor_identity_and_more_than_one_read_refused(self):
        g=self.guard(); self.opened(g)
        frame=types.SimpleNamespace(f_code=types.SimpleNamespace(co_filename=__file__))
        with self.path.open('rb') as handle:
            g.profile(frame,'c_call',handle.read)
            with self.assertRaisesRegex(t.Refusal,'more than one'): g.profile(frame,'c_call',handle.read)
        different=self.path.with_name('different'); different.write_bytes(b'other')
        with different.open('rb') as handle,self.assertRaisesRegex(t.Refusal,'descriptor differs'):
            t.same_fd(handle,self.meta)

    def test_posix_descriptor_write_is_refused_without_calling_it(self):
        frame=types.SimpleNamespace(f_code=types.SimpleNamespace(co_filename=__file__))
        with self.assertRaisesRegex(t.Refusal,'descriptor mutation'): self.guard().profile(frame,'c_call',os.write)

    def test_deadline_and_unexpected_import_and_enumeration_refused(self):
        g=self.guard(); g.deadline=0
        with self.assertRaisesRegex(t.Refusal,'deadline'): self.opened(g)
        self.assertTrue(g.deadline_reached)
        with self.assertRaisesRegex(t.Refusal,'unexpected new import'): self.guard().find_spec('numpy')
        with self.assertRaisesRegex(t.Refusal,'enumeration'): self.guard().audit('os.scandir',(str(self.path.parent),))

    def test_finished_stage_cannot_reopen_inputs(self):
        g=self.guard();g.finished=True
        with self.assertRaisesRegex(t.Refusal,'after review'):self.opened(g)


if __name__=='__main__':unittest.main()
