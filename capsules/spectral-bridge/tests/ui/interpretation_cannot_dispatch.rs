use spectral_bridge_server::authority_types::{
    SemanticMicrodose, dispatch_semantic_microdose,
};
use spectral_bridge_server::types::SensoryMsg;
use spectral_bridge_server::witness::AstridInterpretationV1;
use tokio::sync::mpsc;

fn attempt_dispatch(
    interpretation: AstridInterpretationV1,
    tx: &mpsc::Sender<SensoryMsg>,
) {
    let _ = dispatch_semantic_microdose(interpretation, tx);
}

fn type_anchor(_: SemanticMicrodose) {}

fn main() {}
