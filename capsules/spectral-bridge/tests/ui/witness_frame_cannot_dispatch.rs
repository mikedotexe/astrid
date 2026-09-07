use spectral_bridge_server::authority_types::{SemanticMicrodose, dispatch_semantic_microdose};
use spectral_bridge_server::types::SensoryMsg;
use spectral_bridge_server::witness::WitnessFrameV1;
use tokio::sync::mpsc;

fn attempt_dispatch(frame: WitnessFrameV1, tx: &mpsc::Sender<SensoryMsg>) {
    let _ = dispatch_semantic_microdose(frame, tx);
}

fn type_anchor(_: SemanticMicrodose) {}

fn main() {}
