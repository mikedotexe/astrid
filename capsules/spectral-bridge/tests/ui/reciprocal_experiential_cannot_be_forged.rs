use spectral_bridge_server::reciprocal_experiential::{
    ReciprocalPresenceKindV1, ReciprocalPresenceReceiptV1,
};

fn main() {
    let _forged = ReciprocalPresenceReceiptV1::new(
        "presence_1".into(),
        ReciprocalPresenceKindV1::Offered,
        "astrid".into(),
        "minime".into(),
        "thread_1".into(),
        "event_1".into(),
        "a".repeat(64),
        1,
    );
}
