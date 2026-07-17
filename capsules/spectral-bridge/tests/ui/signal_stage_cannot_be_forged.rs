use spectral_bridge_server::signal_spine::{
    CausalSignalStageV1, SignalStageReceiptV1,
};

fn forge(
    receipt: SignalStageReceiptV1,
    value: Vec<f32>,
) -> CausalSignalStageV1<Vec<f32>> {
    CausalSignalStageV1 { receipt, value }
}

fn main() {}
