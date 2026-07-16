use spectral_bridge_server::authority_types::{
    ApprovalPending, ModeReleaseMicrodose, dispatch_mode_release_microdose,
};
use spectral_bridge_server::types::SensoryMsg;
use tokio::sync::mpsc;

fn attempt_dispatch(
    pending: ApprovalPending<ModeReleaseMicrodose>,
    tx: &mpsc::Sender<SensoryMsg>,
) {
    let _ = dispatch_mode_release_microdose(pending, tx);
}

fn main() {}
