use spectral_bridge_server::witness::MinimeObservationV1;

fn main() {
    let _: MinimeObservationV1 = serde_json::from_str("{}").unwrap();
}
