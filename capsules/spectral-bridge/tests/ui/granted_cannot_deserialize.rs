use spectral_bridge_server::authority_types::{AuthorityGranted, SemanticMicrodose};

fn main() {
    let _: AuthorityGranted<SemanticMicrodose> = serde_json::from_str("{}").unwrap();
}
