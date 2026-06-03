use super::should_start_run_loop;
use crate::manifest::CapsuleManifest;

fn manifest_for_tests(extra: &str) -> CapsuleManifest {
    let source = format!(
        r#"
        [package]
        name = "test"
        version = "0.1.0"

        {extra}
        "#
    );
    toml::from_str(&source).expect("test manifest must parse")
}

#[test]
fn run_export_stub_does_not_start_loop_for_tool_capsule() {
    let manifest = manifest_for_tests("");
    assert!(
        !should_start_run_loop(&manifest, "executable", true),
        "required run export stubs must not make normal capsules long-lived"
    );
}

#[test]
fn uplink_capability_starts_run_loop_when_export_exists() {
    let manifest = manifest_for_tests(
        r#"
        [capabilities]
        uplink = true
        "#,
    );
    assert!(should_start_run_loop(&manifest, "executable", true));
}

#[test]
fn daemon_component_type_starts_run_loop_when_export_exists() {
    let manifest = manifest_for_tests("");
    assert!(should_start_run_loop(&manifest, "daemon", true));
}

#[test]
fn explicit_daemon_without_run_export_does_not_start_missing_loop() {
    let manifest = manifest_for_tests(
        r#"
        [capabilities]
        uplink = true
        "#,
    );
    assert!(!should_start_run_loop(&manifest, "daemon", false));
}
