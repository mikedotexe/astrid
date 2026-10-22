"""Pure evidence tamper/failure tests; all records originate in private inert fakes."""
import copy
import unittest
from unittest.mock import patch
import reservoir_maintenance as m
import recovery_phases as phases
from recovery_ledger import read_ledger, RecoveryLedger
import test_production_recovery_driver as driver_tests


class LedgerTests(driver_tests.DriverTests):
    # Only local tests are collected below; reuse setup/helpers without inheriting
    # the expensive full matrix a second time.
    def prepared(self):
        self.step(); self.step()
        report=self.host.recovery_inspection(m.Journal(self.journal.root))
        return report,read_ledger(self.root,self.plan)

    def changed(self,report,ledger,change):
        value=copy.deepcopy(ledger);change(value['records'])
        previous=m.digest(value['header'])
        for number,row in enumerate(value['records'],1):
            row['sequence']=number;row['previous_sha256']=previous;previous=m.digest(row)
        value['head_sha256']=previous
        report=copy.deepcopy(report);report['recovery_ledger']=value
        return phases.checkpoint_from_inspection(self.host.spec,report,value)

    def test_executor_and_host_span_mutations_refused_after_rechaining(self):
        report,ledger=self.prepared()
        changes=[lambda rows:rows[0]['executor_session'].update(mode='fresh'),
                 lambda rows:rows[0]['executor_session'].update(root='/foreign'),
                 lambda rows:rows[0].update(host_before_sha256='f'*64),
                 lambda rows:rows[-1]['host_operations'].update(after_count=0),
                 lambda rows:rows[-1]['host_operations']['appended'][0].update(record_sha256='f'*64),
                 lambda rows:rows[-1]['host_operations']['appended'][0].update(path='/other'),
                 lambda rows:rows[-1]['executor_session']['executor_identity'].update(started_at='other-start')]
        for change in changes:
            with self.subTest(change=change),self.assertRaises(m.Refusal):self.changed(report,ledger,change)

    def test_claim_context_and_double_claim_refused(self):
        report,ledger=self.prepared()
        with self.assertRaises(m.Refusal):self.changed(report,ledger,lambda rows:rows[0]['context']['bindings'].update(plan_sha256='f'*64))
        with self.assertRaises(m.Refusal):self.changed(report,ledger,lambda rows:rows.insert(1,copy.deepcopy(rows[0])))

    def test_failed_claim_cannot_be_converted_to_ordinary_success(self):
        report,ledger=self.prepared()
        def change(rows):
            claim=rows[-2];result=rows[-1]
            failed=copy.deepcopy(result);failed.update(type='action_failed',error='failure',
                automatic_retry_permitted=False,effect_absence_inferred=False)
            # This is a deliberately reconstructed chain with matching references,
            # so the refusal tests phase semantics rather than hash corruption.
            rows.insert(len(rows)-1,failed)
        with self.assertRaisesRegex(m.Refusal,'failed action'):self.changed(report,ledger,change)

    def test_hard_interrupt_with_no_failure_record_still_cannot_replay(self):
        self.advance('create_hold:provider');append=RecoveryLedger.append
        def fail(log,kind,**details):
            if kind in ('action_result','action_failed'):raise OSError('storage interrupted')
            return append(log,kind,**details)
        with patch.object(RecoveryLedger,'append',fail),self.assertRaises(OSError):self.step()
        self.assertEqual(read_ledger(self.root,self.plan)['records'][-1]['type'],'action_claim')
        with self.assertRaisesRegex(m.Refusal,'never replay'):self.step('execute')
        self.assertEqual(self.step('reconcile')['action_id'],'create_hold:provider')
        self.assertEqual(self.host.calls.count(('create_hold','provider')),1)

    def test_result_write_is_durable_even_when_session_end_write_fails(self):
        write=m.write_new
        def fail(path,value):
            if str(path).endswith('-end.json'):raise OSError('end write interrupted')
            return write(path,value)
        with patch.object(m,'write_new',side_effect=fail),self.assertRaises(OSError):self.step()
        self.assertEqual(self.checkpoint()['action_id'],'create_hold:provider')
        self.assertEqual(self.step()['action_id'],'create_hold:provider')


# Reuse setup/helpers, but do not collect inherited DriverTests cases again.
for _name in list(driver_tests.DriverTests.__dict__):
    if _name.startswith('test_') and _name not in LedgerTests.__dict__:
        setattr(LedgerTests,_name,None)

if __name__=='__main__':unittest.main(verbosity=2)
