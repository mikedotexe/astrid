use spectral_bridge_server::authority_types::{
    EvidenceOnly, SemanticMicrodose, dispatch_semantic_microdose,
};
use spectral_bridge_server::types::SensoryMsg;
use tokio::sync::mpsc;

fn attempt_dispatch(
    evidence: EvidenceOnly<SemanticMicrodose>,
    tx: &mpsc::Sender<SensoryMsg>,
) {
    let _ = dispatch_semantic_microdose(evidence, tx);
}

fn main() {}
