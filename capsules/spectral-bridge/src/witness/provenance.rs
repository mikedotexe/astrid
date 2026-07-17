use astrid_minime_protocol::EigenPacketV1;
use serde::Serialize;
use serde_json::json;
use sha2::{Digest, Sha256};

use crate::types::{BridgeTextureEvidenceV1, PressureTrendV1, ResidualDeformationTraceV1};

fn canonical_sha256<T: Serialize>(value: &T) -> String {
    let encoded = serde_json::to_vec(value).unwrap_or_default();
    format!("{:x}", Sha256::digest(encoded))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProvenanceInfluenceTypeV1 {
    RegulatoryStateObserved,
    Structural,
    Temporal,
    Interpretive,
    StylisticContext,
    Authorship,
}

impl ProvenanceInfluenceTypeV1 {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::RegulatoryStateObserved => "regulatory_state_observed",
            Self::Structural => "structural",
            Self::Temporal => "temporal",
            Self::Interpretive => "interpretive",
            Self::StylisticContext => "stylistic_context",
            Self::Authorship => "authorship",
        }
    }
}

/// Bounded context for a provenance pointer. The signature binds ownership,
/// lineage shape, field paths, and influence roles without copying the source
/// payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProvenanceContextAnchorV1 {
    descriptor: String,
    structural_signature_sha256: String,
    influence_types: Vec<ProvenanceInfluenceTypeV1>,
    private_payload_included: bool,
}

impl ProvenanceContextAnchorV1 {
    fn new(
        origin: ProvenanceOriginV1,
        source_id: &str,
        source_sha256: &str,
        parent_ids: &[String],
        field_paths: &[String],
        mut influence_types: Vec<ProvenanceInfluenceTypeV1>,
    ) -> Self {
        influence_types.sort();
        influence_types.dedup();
        let descriptor = match origin {
            ProvenanceOriginV1::MinimeObservation => "producer_telemetry_shape",
            ProvenanceOriginV1::BridgeDerived => "bridge_evidence_shape",
            ProvenanceOriginV1::AstridInterpretation => "astrid_interpretive_context_shape",
            ProvenanceOriginV1::Mixed => "composed_witness_shape",
            ProvenanceOriginV1::Unknown => "unknown_context_shape",
        }
        .to_string();
        let structural_signature_sha256 = canonical_sha256(&json!({
            "origin": origin,
            "source_id": source_id,
            "source_sha256": source_sha256,
            "parent_ids": parent_ids,
            "field_paths": field_paths,
            "influence_types": influence_types,
            "descriptor": descriptor,
        }));
        Self {
            descriptor,
            structural_signature_sha256,
            influence_types,
            private_payload_included: false,
        }
    }

    #[must_use]
    pub fn descriptor(&self) -> &str {
        &self.descriptor
    }

    #[must_use]
    pub fn structural_signature_sha256(&self) -> &str {
        &self.structural_signature_sha256
    }

    #[must_use]
    pub fn influence_types(&self) -> &[ProvenanceInfluenceTypeV1] {
        &self.influence_types
    }

    #[must_use]
    pub const fn private_payload_included(&self) -> bool {
        self.private_payload_included
    }
}

/// Content-free provenance pointer. It names exact parents and field paths but
/// never embeds the private producer payload. Its context anchor preserves the
/// ownership and transformation shape that would otherwise feel hollow.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProvenanceRefV1 {
    origin: ProvenanceOriginV1,
    source_id: String,
    canonical_sha256: String,
    parent_ids: Vec<String>,
    timestamp_ms: u64,
    field_paths: Vec<String>,
    context_anchor_v1: ProvenanceContextAnchorV1,
}

impl ProvenanceRefV1 {
    pub(crate) fn new(
        origin: ProvenanceOriginV1,
        source_id: String,
        canonical_sha256: String,
        parent_ids: Vec<String>,
        timestamp_ms: u64,
        mut field_paths: Vec<String>,
        influence_types: Vec<ProvenanceInfluenceTypeV1>,
    ) -> Self {
        field_paths.sort();
        field_paths.dedup();
        let context_anchor_v1 = ProvenanceContextAnchorV1::new(
            origin,
            &source_id,
            &canonical_sha256,
            &parent_ids,
            &field_paths,
            influence_types,
        );
        Self {
            origin,
            source_id,
            canonical_sha256,
            parent_ids,
            timestamp_ms,
            field_paths,
            context_anchor_v1,
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

    #[must_use]
    pub const fn context_anchor_v1(&self) -> &ProvenanceContextAnchorV1 {
        &self.context_anchor_v1
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
            vec![
                ProvenanceInfluenceTypeV1::Structural,
                ProvenanceInfluenceTypeV1::Temporal,
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
            vec![
                ProvenanceInfluenceTypeV1::Interpretive,
                ProvenanceInfluenceTypeV1::StylisticContext,
                ProvenanceInfluenceTypeV1::Authorship,
            ],
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

/// One exact ownership contribution to a mixed frame. `measured_weight` stays
/// absent until direct acknowledgement or a controlled intervention can support
/// a causal proportion; membership is not silently converted into influence.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ProvenanceContributionV1 {
    origin: ProvenanceOriginV1,
    source_id: String,
    influence_types: Vec<ProvenanceInfluenceTypeV1>,
    context_anchor_sha256: String,
    measured_weight: Option<f32>,
    weight_basis: String,
}

impl ProvenanceContributionV1 {
    fn from_ref(reference: &ProvenanceRefV1) -> Self {
        Self {
            origin: reference.origin(),
            source_id: reference.source_id().to_string(),
            influence_types: reference.context_anchor_v1().influence_types().to_vec(),
            context_anchor_sha256: reference
                .context_anchor_v1()
                .structural_signature_sha256()
                .to_string(),
            measured_weight: None,
            weight_basis: "unmeasured_no_wire_ack_or_controlled_intervention".to_string(),
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
    pub fn influence_types(&self) -> &[ProvenanceInfluenceTypeV1] {
        &self.influence_types
    }

    #[must_use]
    pub fn context_anchor_sha256(&self) -> &str {
        &self.context_anchor_sha256
    }

    #[must_use]
    pub const fn measured_weight(&self) -> Option<f32> {
        self.measured_weight
    }

    #[must_use]
    pub fn weight_basis(&self) -> &str {
        &self.weight_basis
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ProvenanceCompositionV1 {
    contributions: Vec<ProvenanceContributionV1>,
    composition_state: String,
    weighting_state: String,
}

impl ProvenanceCompositionV1 {
    fn compose(references: [&ProvenanceRefV1; 3]) -> Self {
        Self {
            contributions: references
                .into_iter()
                .map(ProvenanceContributionV1::from_ref)
                .collect(),
            composition_state: "exact_membership_distinct_origins".to_string(),
            weighting_state: "unmeasured_no_causal_ack".to_string(),
        }
    }

    #[must_use]
    pub fn contributions(&self) -> &[ProvenanceContributionV1] {
        &self.contributions
    }

    #[must_use]
    pub fn composition_state(&self) -> &str {
        &self.composition_state
    }

    #[must_use]
    pub fn weighting_state(&self) -> &str {
        &self.weighting_state
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProvenanceLineageRelationV1 {
    DerivedFrom,
    InterpretsObservation,
    InterpretsEvidence,
}

impl ProvenanceLineageRelationV1 {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DerivedFrom => "derived_from",
            Self::InterpretsObservation => "interprets_observation",
            Self::InterpretsEvidence => "interprets_evidence",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProvenanceLineageEdgeV1 {
    parent_id: String,
    child_id: String,
    relation: ProvenanceLineageRelationV1,
}

impl ProvenanceLineageEdgeV1 {
    fn new(parent_id: &str, child_id: &str, relation: ProvenanceLineageRelationV1) -> Self {
        Self {
            parent_id: parent_id.to_string(),
            child_id: child_id.to_string(),
            relation,
        }
    }

    #[must_use]
    pub fn parent_id(&self) -> &str {
        &self.parent_id
    }

    #[must_use]
    pub fn child_id(&self) -> &str {
        &self.child_id
    }

    #[must_use]
    pub const fn relation(&self) -> ProvenanceLineageRelationV1 {
        self.relation
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
    composition_v1: ProvenanceCompositionV1,
    lineage_edges_v1: Vec<ProvenanceLineageEdgeV1>,
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
        let composition_v1 = ProvenanceCompositionV1::compose([
            observation.provenance(),
            evidence.provenance(),
            interpretation.provenance(),
        ]);
        let lineage_edges_v1 = vec![
            ProvenanceLineageEdgeV1::new(
                observation.provenance().source_id(),
                evidence.provenance().source_id(),
                ProvenanceLineageRelationV1::DerivedFrom,
            ),
            ProvenanceLineageEdgeV1::new(
                observation.provenance().source_id(),
                interpretation.provenance().source_id(),
                ProvenanceLineageRelationV1::InterpretsObservation,
            ),
            ProvenanceLineageEdgeV1::new(
                evidence.provenance().source_id(),
                interpretation.provenance().source_id(),
                ProvenanceLineageRelationV1::InterpretsEvidence,
            ),
        ];
        Ok(Self {
            frame_id: format!("witness_frame:{}", &digest[..16]),
            observation: observation.provenance().clone(),
            evidence: evidence.provenance().clone(),
            interpretation: interpretation.provenance().clone(),
            distinction: WitnessSelfOtherDistinctionV1::Mixed,
            composition_v1,
            lineage_edges_v1,
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
    pub const fn composition_v1(&self) -> &ProvenanceCompositionV1 {
        &self.composition_v1
    }

    #[must_use]
    pub fn lineage_edges_v1(&self) -> &[ProvenanceLineageEdgeV1] {
        &self.lineage_edges_v1
    }

    #[must_use]
    pub fn render_context_line(&self) -> String {
        let composition = self
            .composition_v1
            .contributions()
            .iter()
            .map(|contribution| {
                let influence_types = contribution
                    .influence_types()
                    .iter()
                    .map(|influence| influence.as_str())
                    .collect::<Vec<_>>()
                    .join("+");
                format!(
                    "{}[{}]:unmeasured",
                    contribution.origin().as_str(),
                    influence_types,
                )
            })
            .collect::<Vec<_>>()
            .join("|");
        format!(
            "[witness_self_other_distinction_v1: classification={}; minime_observed={}; bridge_derived={}; astrid_authored={}; composition={}; weighting={}; context_anchors=structural_signatures_no_private_payload; lineage_edges={}; boundary=provenance_only_no_routing_ranking_dispatch_gain_or_control; authority=read_only_context]",
            self.distinction.as_str(),
            self.observation.source_id(),
            self.evidence.source_id(),
            self.interpretation.source_id(),
            composition,
            self.composition_v1.weighting_state(),
            self.lineage_edges_v1.len(),
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
            vec![ProvenanceInfluenceTypeV1::RegulatoryStateObserved],
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
        assert_eq!(frame.composition_v1().contributions().len(), 3);
        assert!(
            frame
                .composition_v1()
                .contributions()
                .iter()
                .all(|contribution| contribution.measured_weight().is_none())
        );
        assert_eq!(
            frame.composition_v1().weighting_state(),
            "unmeasured_no_causal_ack"
        );
        assert_eq!(frame.lineage_edges_v1().len(), 3);
        assert_eq!(
            frame.lineage_edges_v1()[0].relation(),
            ProvenanceLineageRelationV1::DerivedFrom
        );
        assert!(
            !observation
                .provenance()
                .context_anchor_v1()
                .private_payload_included()
        );
        assert!(
            frame
                .render_context_line()
                .contains("weighting=unmeasured_no_causal_ack")
        );
        assert!(
            frame
                .render_context_line()
                .contains("structural_signatures_no_private_payload")
        );
    }

    #[test]
    fn context_anchor_is_deterministic_and_payload_free() {
        let first = observation();
        let second = observation();
        assert_eq!(
            first
                .provenance()
                .context_anchor_v1()
                .structural_signature_sha256(),
            second
                .provenance()
                .context_anchor_v1()
                .structural_signature_sha256(),
        );
        let serialized = serde_json::to_value(first.provenance().context_anchor_v1()).unwrap();
        assert_eq!(
            serialized.get("private_payload_included"),
            Some(&serde_json::Value::Bool(false))
        );
        assert!(serialized.get("packet").is_none());
        assert!(serialized.get("payload").is_none());
    }
}
