use spectral_bridge_server::evidence_study_capture::TelemetryStudySampleV1;

fn main() {
    let _: TelemetryStudySampleV1 = serde_json::from_str("{}").unwrap();
}
