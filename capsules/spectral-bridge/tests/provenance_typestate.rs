#[test]
fn provenance_domains_cannot_be_forged_or_dispatched() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/ui/raw_packet_cannot_construct_observation.rs");
    cases.compile_fail("tests/ui/producer_cannot_construct_bridge_evidence.rs");
    cases.compile_fail("tests/ui/observation_cannot_deserialize.rs");
    cases.compile_fail("tests/ui/interpretation_cannot_dispatch.rs");
    cases.compile_fail("tests/ui/lived_state_witness_cannot_be_forged.rs");
    cases.compile_fail("tests/ui/lived_state_witness_cannot_deserialize.rs");
    cases.compile_fail("tests/ui/reciprocal_experiential_cannot_be_forged.rs");
    cases.compile_fail("tests/ui/reciprocal_experiential_cannot_deserialize.rs");
    cases.compile_fail("tests/ui/reciprocal_resonance_cannot_be_forged.rs");
    cases.compile_fail("tests/ui/reciprocal_resonance_cannot_deserialize.rs");
}
