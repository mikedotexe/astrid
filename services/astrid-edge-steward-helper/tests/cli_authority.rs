use std::process::Command;

#[test]
fn installed_cli_rejects_operator_supplied_schedule_or_question() {
    for option in ["--due-nonce", "--question"] {
        let output = Command::new(env!("CARGO_BIN_EXE_astrid-edge-steward-helper"))
            .args(["--config", "/does/not/exist", option, "operator-value"])
            .output()
            .unwrap();
        assert!(!output.status.success());
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(stderr.contains("unsupported argument"));
        assert!(stderr.contains(option));
    }
}
