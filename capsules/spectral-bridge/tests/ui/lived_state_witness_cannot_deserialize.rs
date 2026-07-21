use spectral_bridge_server::lived_state_witness::TemporalLivedStateWitnessV1;

fn main() {
    let _: TemporalLivedStateWitnessV1 = serde_json::from_str("{}").unwrap();
}
