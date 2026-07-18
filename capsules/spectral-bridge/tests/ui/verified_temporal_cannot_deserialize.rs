use spectral_bridge_server::authority_temporal::VerifiedAuthorityContextV1;

fn main() {
    let _: VerifiedAuthorityContextV1 = serde_json::from_str("{}").unwrap();
}
