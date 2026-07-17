use spectral_bridge_server::signal_spine::SignalStageReceiptV1;

fn deserialize_untrusted_receipt(value: &str) -> SignalStageReceiptV1 {
    serde_json::from_str(value).unwrap()
}

fn main() {}
