use astrid_minime_protocol::EigenPacketV1;
use serde::Serialize;
use serde_json::json;
use sha2::{Digest, Sha256};

use crate::types::{BridgeTextureEvidenceV1, PressureTrendV1, ResidualDeformationTraceV1};

fn canonical_sha256<T: Serialize>(value: &T) -> String {
    let encoded = serde_json::to_vec(value).unwrap_or_default();
    format!("{:x}", Sha256::digest(encoded))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProvenanceOriginV1 {
    MinimeObservation,
    BridgeDerived,
    AstridInterpretation,
    Mixed,
    Unknown,
}

impl ProvenanceOriginV1 {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::MinimeObservation => "minime_observed",
            Self::BridgeDerived => "bridge_derived",
            Self::AstridInterpretation => "astrid_authored",
            Self::Mixed => "mixed",
            Self::Unknown => "unknown",
        }
    }
}

/// Content-free provenance pointer. It names exact parents and field paths but
/// never embeds the private producer payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProvenanceRefV1 {
    origin: ProvenanceOriginV1,
    source_id: String,
    canonical_sha256: String,
    parent_ids: Vec<String>,
    timestamp_ms: u64,
    field_paths: Vec<String>,
}

impl ProvenanceRefV1 {
    pub(crate) fn new(
        origin: ProvenanceOriginV1,
        source_id: String,
        canonical_sha256: String,
        parent_ids: Vec<String>,
        timestamp_ms: u64,
        mut field_paths: Vec<String>,
    ) -> Self {
        field_paths.sort();
        field_paths.dedup();
        Self {
            origin,
            source_id,
            canonical_sha256,
            parent_ids,
            timestamp_ms,
            field_paths,
        }
    }

    #[must_use]
    pub const fn origin(&self) -> ProvenanceOriginV1 {
        self.origin
    }

    #[must_use]
    pub fn source_id(&self) -> &str {
        &self.source_id
    }

    #[must_use]
    pub fn canonical_sha256(&self) -> &str {
        &self.canonical_sha256
    }

    #[must_use]
    pub fn parent_ids(&self) -> &[String] {
        &self.parent_ids
    }

    #[must_use]
    pub const fn timestamp_ms(&self) -> u64 {
        self.timestamp_ms
    }

    #[must_use]
    pub fn field_paths(&self) -> &[String] {
        &self.field_paths
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WireReceiptV1 {
    lane: String,
    byte_len: usize,
    raw_sha256: String,
    canonical_sha256: String,
    compatibility: String,
}

impl WireReceiptV1 {
    pub(crate) fn new(
        byte_len: usize,
        raw_sha256: String,
        canonical_sha256: String,
        compatibility: String,
    ) -> Self {
        Self {
            lane: "minime_telemetry_7878".to_string(),
            byte_len,
            raw_sha256,
            canonical_sha256,
            compatibility,
        }
    }

    #[must_use]
    pub fn lane(&self) -> &str {
        &self.lane
    }

    #[must_use]
    pub const fn byte_len(&self) -> usize {
        self.byte_len
    }

    #[must_use]
    pub fn raw_sha256(&self) -> &str {
        &self.raw_sha256
    }

    #[must_use]
    pub fn canonical_sha256(&self) -> &str {
        &self.canonical_sha256
    }

    #[must_use]
    pub fn compatibility(&self) -> &str {
        &self.compatibility
    }
}

/// Immutable producer truth decoded at port 7878.
#[derive(Debug, Clone)]
pub struct MinimeObservationV1 {
    packet: EigenPacketV1,
    provenance: ProvenanceRefV1,
    wire_receipt: WireReceiptV1,
}

impl MinimeObservationV1 {
    pub(crate) fn new(
        packet: EigenPacketV1,
        provenance: ProvenanceRefV1,
        wire_receipt: WireReceiptV1,
    ) -> Self {
        Self {
            packet,
            provenance,
            wire_receipt,
        }
    }

    #[must_use]
    pub fn packet(&self) -> &EigenPacketV1 {
        &self.packet
    }

    #[must_use]
    pub const fn provenance(&self) -> &ProvenanceRefV1 {
        &self.provenance
    }

    #[must_use]
    pub const fn wire_receipt(&self) -> &WireReceiptV1 {
        &self.wire_receipt
    }
}

/// Bridge-owned temporal and derivative evidence. It cannot be constructed by
/// packet deserialization and always cites the exact Minime observation parent.
#[derive(Debug, Clone)]
pub struct BridgeEvidenceV1 {
    provenance: ProvenanceRefV1,
    texture: Option<BridgeTextureEvidenceV1>,
    residual_deformation: Option<ResidualDeformationTraceV1>,
    pressure_trend: Option<PressureTrendV1>,
}

impl BridgeEvidenceV1 {
    pub(crate) fn derive(
        observation: &MinimeObservationV1,
        texture: Option<BridgeTextureEvidenceV1>,
        residual_deformation: Option<ResidualDeformationTraceV1>,
        pressure_trend: Option<PressureTrendV1>,
    ) -> Self {
        let digest = canonical_sha256(&json!({
            "observation_parent": observation.provenance().source_id(),
            "texture": texture,
            "residual_deformation": residual_deformation,
            "pressure_trend": pressure_trend,
        }));
        let source_id = format!("bridge_evidence:{}", &digest[..16]);
        let provenance = ProvenanceRefV1::new(
            ProvenanceOriginV1::BridgeDerived,
            source_id,
            digest,
            vec![observation.provenance().source_id().to_string()],
            observation.packet().t_ms,
            vec![
                "bridge.texture".to_string(),
                "bridge.residual_deformation".to_string(),
                "bridge.pressure_trend".to_string(),
            ],
        );
        Self {
            provenance,
            texture,
            residual_deformation,
            pressure_trend,
        }
    }

    #[must_use]
    pub const fn provenance(&self) -> &ProvenanceRefV1 {
        &self.provenance
    }

    #[must_use]
    pub const fn texture(&self) -> Option<&BridgeTextureEvidenceV1> {
        self.texture.as_ref()
    }

    #[must_use]
    pub const fn residual_deformation(&self) -> Option<&ResidualDeformationTraceV1> {
        self.residual_deformation.as_ref()
    }

    #[must_use]
    pub const fn pressure_trend(&self) -> Option<&PressureTrendV1> {
        self.pressure_trend.as_ref()
    }
}

/// Astrid-owned read-only interpretation of the observation/evidence boundary.
#[derive(Debug, Clone)]
pub struct AstridInterpretationV1 {
    provenance: ProvenanceRefV1,
    distinction: WitnessSelfOtherDistinctionV1,
}

impl AstridInterpretationV1 {
    pub(crate) fn interpret(
        observation: &MinimeObservationV1,
        evidence: &BridgeEvidenceV1,
    ) -> Self {
        let parents = vec![
            observation.provenance().source_id().to_string(),
            evidence.provenance().source_id().to_string(),
        ];
        let digest = canonical_sha256(&json!({
            "parents": parents,
            "distinction": "mixed",
            "policy": "self_other_provenance_only_no_routing_or_control",
        }));
        let provenance = ProvenanceRefV1::new(
            ProvenanceOriginV1::AstridInterpretation,
            format!("astrid_interpretation:{}", &digest[..16]),
            digest,
            parents,
            observation.packet().t_ms,
            vec!["astrid.witness_self_other_distinction".to_string()],
        );
        Self {
            provenance,
            distinction: WitnessSelfOtherDistinctionV1::Mixed,
        }
    }

    #[must_use]
    pub const fn provenance(&self) -> &ProvenanceRefV1 {
        &self.provenance
    }

    #[must_use]
    pub const fn distinction(&self) -> WitnessSelfOtherDistinctionV1 {
        self.distinction
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WitnessSelfOtherDistinctionV1 {
    MinimeObserved,
    BridgeDerived,
    AstridAuthored,
    Mixed,
    Unknown,
}

impl WitnessSelfOtherDistinctionV1 {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::MinimeObserved => "minime_observed",
            Self::BridgeDerived => "bridge_derived",
            Self::AstridAuthored => "astrid_authored",
            Self::Mixed => "mixed",
            Self::Unknown => "unknown",
        }
    }
}

/// One validated frame joining the three ownership domains by references only.
#[derive(Debug, Clone)]
pub struct WitnessFrameV1 {
    frame_id: String,
    observation: ProvenanceRefV1,
    evidence: ProvenanceRefV1,
    interpretation: ProvenanceRefV1,
    distinction: WitnessSelfOtherDistinctionV1,
}

impl WitnessFrameV1 {
    pub(crate) fn compose(
        observation: &MinimeObservationV1,
        evidence: &BridgeEvidenceV1,
        interpretation: &AstridInterpretationV1,
    ) -> Result<Self, &'static str> {
        if evidence.provenance().parent_ids() != [observation.provenance().source_id().to_string()]
        {
            return Err("bridge evidence does not cite the observation parent");
        }
        let interpretation_parents = interpretation.provenance().parent_ids();
        if !interpretation_parents
            .iter()
            .any(|parent| parent == observation.provenance().source_id())
            || !interpretation_parents
                .iter()
                .any(|parent| parent == evidence.provenance().source_id())
        {
            return Err("Astrid interpretation does not cite both exact parents");
        }
        let digest = canonical_sha256(&json!({
            "observation": observation.provenance().source_id(),
            "evidence": evidence.provenance().source_id(),
            "interpretation": interpretation.provenance().source_id(),
        }));
        Ok(Self {
            frame_id: format!("witness_frame:{}", &digest[..16]),
            observation: observation.provenance().clone(),
            evidence: evidence.provenance().clone(),
            interpretation: interpretation.provenance().clone(),
            distinction: WitnessSelfOtherDistinctionV1::Mixed,
        })
    }

    #[must_use]
    pub fn frame_id(&self) -> &str {
        &self.frame_id
    }

    #[must_use]
    pub const fn observation(&self) -> &ProvenanceRefV1 {
        &self.observation
    }

    #[must_use]
    pub const fn evidence(&self) -> &ProvenanceRefV1 {
        &self.evidence
    }

    #[must_use]
    pub const fn interpretation(&self) -> &ProvenanceRefV1 {
        &self.interpretation
    }

    #[must_use]
    pub const fn distinction(&self) -> WitnessSelfOtherDistinctionV1 {
        self.distinction
    }

    #[must_use]
    pub fn render_context_line(&self) -> String {
        format!(
            "[witness_self_other_distinction_v1: classification={}; minime_observed={}; bridge_derived={}; astrid_authored={}; boundary=provenance_only_no_routing_ranking_dispatch_gain_or_control; authority=read_only_context]",
            self.distinction.as_str(),
            self.observation.source_id(),
            self.evidence.source_id(),
            self.interpretation.source_id(),
        )
    }
}

#[cfg(test)]
mod tests {
    use astrid_minime_protocol::EigenPacketV1;

    use super::*;

    fn observation() -> MinimeObservationV1 {
        let packet = EigenPacketV1 {
            t_ms: 42,
            eigenvalues: vec![1.0, 0.5],
            fill_ratio: 0.68,
            ..EigenPacketV1::default()
        };
        let provenance = ProvenanceRefV1::new(
            ProvenanceOriginV1::MinimeObservation,
            "minime:42".to_string(),
            "a".repeat(64),
            vec![],
            42,
            vec!["$.fill_ratio".to_string()],
        );
        MinimeObservationV1::new(
            packet,
            provenance,
            WireReceiptV1::new(1, "b".repeat(64), "a".repeat(64), "current".to_string()),
        )
    }

    #[test]
    fn parent_chain_is_exact_and_deterministic() {
        let observation = observation();
        let first = BridgeEvidenceV1::derive(&observation, None, None, None);
        let second = BridgeEvidenceV1::derive(&observation, None, None, None);
        assert_eq!(first.provenance(), second.provenance());
        let interpretation = AstridInterpretationV1::interpret(&observation, &first);
        let frame = WitnessFrameV1::compose(&observation, &first, &interpretation).unwrap();
        assert_eq!(frame.distinction(), WitnessSelfOtherDistinctionV1::Mixed);
        assert_eq!(frame.evidence().parent_ids(), &["minime:42".to_string()]);
        assert!(
            frame
                .render_context_line()
                .contains("no_routing_ranking_dispatch_gain_or_control")
        );
    }
}
