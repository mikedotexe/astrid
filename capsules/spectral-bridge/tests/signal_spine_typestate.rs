#[test]
fn persisted_signal_receipts_cannot_forge_trusted_stages() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/ui/signal_stage_cannot_be_forged.rs");
    cases.compile_fail("tests/ui/signal_receipt_cannot_deserialize.rs");
}
