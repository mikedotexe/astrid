use astrid_minime_protocol::EigenPacketV1;
use spectral_bridge_server::witness::{
    MinimeObservationV1, ProvenanceRefV1, WireReceiptV1,
};

fn forge(
    packet: EigenPacketV1,
    provenance: ProvenanceRefV1,
    wire_receipt: WireReceiptV1,
) -> MinimeObservationV1 {
    MinimeObservationV1 {
        packet,
        provenance,
        wire_receipt,
    }
}

fn main() {}
