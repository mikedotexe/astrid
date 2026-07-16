#[test]
fn untrusted_authority_states_cannot_reach_dispatch() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/ui/evidence_cannot_dispatch.rs");
    cases.compile_fail("tests/ui/pending_cannot_dispatch.rs");
    cases.compile_fail("tests/ui/granted_cannot_deserialize.rs");
}
