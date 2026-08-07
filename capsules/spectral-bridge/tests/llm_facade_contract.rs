#[test]
fn llm_facade_reexports_items_without_exposing_implementation_module() {
    let cases = trybuild::TestCases::new();
    cases.pass("tests/ui/llm_facade_reexports_exchange.rs");
    cases.compile_fail("tests/ui/llm_provider_module_is_private.rs");
}
