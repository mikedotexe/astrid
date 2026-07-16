use std::collections::BTreeSet;

use astrid_minime_protocol::{CompatibilityStatus, EigenPacketV1};
use serde_json::Value;
use sha2::{Digest, Sha256};

use super::{MinimeObservationV1, ProvenanceOriginV1, ProvenanceRefV1, WireReceiptV1};
use crate::types::SpectralTelemetry;

pub(crate) struct DecodedTelemetryV1 {
    pub(crate) observation: MinimeObservationV1,
    pub(crate) compatibility_projection: SpectralTelemetry,
}

fn compatibility_label(status: CompatibilityStatus) -> &'static str {
    match status {
        CompatibilityStatus::Current => "current",
        CompatibilityStatus::CompatibleMinor => "compatible_minor",
        CompatibilityStatus::LegacyUnversioned => "legacy_unversioned",
        CompatibilityStatus::UnsupportedName => "unsupported_name",
        CompatibilityStatus::UnsupportedMajor => "unsupported_major",
    }
}

fn collect_field_paths(value: &Value, path: &str, fields: &mut BTreeSet<String>) {
    match value {
        Value::Object(map) => {
            for (key, child) in map {
                let child_path = format!("{path}.{key}");
                fields.insert(child_path.clone());
                collect_field_paths(child, &child_path, fields);
            }
        },
        Value::Array(items) => {
            let item_path = format!("{path}[]");
            fields.insert(item_path.clone());
            for child in items {
                if child.is_object() || child.is_array() {
                    collect_field_paths(child, &item_path, fields);
                }
            }
        },
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {},
    }
}

pub(crate) fn decode_telemetry_v1(data: &[u8]) -> Result<DecodedTelemetryV1, serde_json::Error> {
    // Parse bytes exactly once. Both typed forms are conversions from this one
    // parsed tree; the tree is dropped at the port boundary.
    let wire_value: Value = serde_json::from_slice(data)?;
    let canonical_bytes = serde_json::to_vec(&wire_value)?;
    let canonical_sha256 = format!("{:x}", Sha256::digest(&canonical_bytes));
    let raw_sha256 = format!("{:x}", Sha256::digest(data));
    let mut field_paths = BTreeSet::new();
    collect_field_paths(&wire_value, "$", &mut field_paths);

    let packet: EigenPacketV1 = serde_json::from_value(wire_value.clone())?;
    let compatibility = packet.compatibility();
    let compatibility_projection: SpectralTelemetry = serde_json::from_value(wire_value)?;
    let source_id = format!(
        "minime_observation:{}:{}",
        packet.t_ms,
        &canonical_sha256[..16]
    );
    let provenance = ProvenanceRefV1::new(
        ProvenanceOriginV1::MinimeObservation,
        source_id,
        canonical_sha256.clone(),
        vec![],
        packet.t_ms,
        field_paths.into_iter().collect(),
    );
    let wire_receipt = WireReceiptV1::new(
        data.len(),
        raw_sha256,
        canonical_sha256,
        compatibility_label(compatibility).to_string(),
    );
    Ok(DecodedTelemetryV1 {
        observation: MinimeObservationV1::new(packet, provenance, wire_receipt),
        compatibility_projection,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_decode_preserves_legacy_absence_and_records_presence() {
        let data = br#"{"t_ms":7,"eigenvalues":[1.0],"fill_ratio":0.5}"#;
        let decoded = decode_telemetry_v1(data).unwrap();
        assert_eq!(decoded.observation.packet().t_ms, 7);
        assert_eq!(decoded.compatibility_projection.active_mode_count, None);
        assert!(
            decoded
                .observation
                .provenance()
                .field_paths()
                .contains(&"$.fill_ratio".to_string())
        );
        assert!(
            !decoded
                .observation
                .provenance()
                .field_paths()
                .contains(&"$.active_mode_count".to_string())
        );
    }

    #[test]
    fn canonical_hash_ignores_object_key_order() {
        let left =
            decode_telemetry_v1(br#"{"t_ms":7,"eigenvalues":[1.0],"fill_ratio":0.5}"#).unwrap();
        let right =
            decode_telemetry_v1(br#"{"fill_ratio":0.5,"eigenvalues":[1.0],"t_ms":7}"#).unwrap();
        assert_eq!(
            left.observation.provenance().canonical_sha256(),
            right.observation.provenance().canonical_sha256()
        );
        assert_ne!(
            left.observation.wire_receipt().raw_sha256(),
            right.observation.wire_receipt().raw_sha256()
        );
    }
}
