/// A spectral bridge event published on the legacy `consciousness.v1.event` topic.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpectralBridgeEvent {
    /// Event type: "`phase_transition`", "distress", "recovery", "`safety_change`".
    pub event_type: String,
    /// Human-readable description.
    pub description: String,
    /// Spectral context at the time of the event.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spectral_context: Option<SpectralContext>,
}

/// Snapshot of spectral state at a point in time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpectralContext {
    pub fill_pct: f32,
    pub lambda1: f32,
    pub phase: String,
    pub safety_level: SafetyLevel,
}

// ---------------------------------------------------------------------------
// Attractor autonomy ledger (IPC topics and SQLite payloads)
// ---------------------------------------------------------------------------

/// Dynamical substrate an attractor intent addresses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttractorSubstrate {
    /// Minime's live ESN / covariance phase space.
    MinimeEsn,
    /// Astrid's semantic codec and prompt/gesture loop.
    AstridCodec,
    /// The persistent named-handle triple reservoir service.
    TripleReservoir,
    /// A coupled Astrid/Minime move across more than one substrate.
    CrossBeing,
}

impl AttractorSubstrate {
    /// Stable string for DB indexing and IPC logs.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::MinimeEsn => "minime_esn",
            Self::AstridCodec => "astrid_codec",
            Self::TripleReservoir => "triple_reservoir",
            Self::CrossBeing => "cross_being",
        }
    }
}

/// High-level command carried by an attractor intent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttractorCommandKind {
    /// Create or seed a new basin.
    Create,
    /// Promote older proto-attractor evidence into a seed.
    Promote,
    /// Re-enter a known seed/basin.
    Summon,
    /// Compare a live basin with a baseline or peer seed.
    Compare,
    /// Let an attractor cool without replay.
    Release,
    /// Name an emergent basin as an authored seed.
    Claim,
    /// Combine two or more parent seeds into a child seed.
    Blend,
    /// Refresh a seed snapshot without live sensory/control writes.
    RefreshSnapshot,
    /// Revert to the last stable seed/control posture.
    Rollback,
}

impl AttractorCommandKind {
    /// Stable string for IPC notes and deterministic schedules.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Create => "create",
            Self::Promote => "promote",
            Self::Summon => "summon",
            Self::Compare => "compare",
            Self::Release => "release",
            Self::Claim => "claim",
            Self::Blend => "blend",
            Self::RefreshSnapshot => "refresh_snapshot",
            Self::Rollback => "rollback",
        }
    }
}

/// Status for a reversible natural-language attractor suggestion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttractorSuggestionStatus {
    Pending,
    Accepted,
    Revised,
    RevisionNeeded,
    Rejected,
    Expired,
    ExecutedDowngraded,
    ExecutedWithoutPending,
    Executed,
}

impl AttractorSuggestionStatus {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Accepted => "accepted",
            Self::Revised => "revised",
            Self::RevisionNeeded => "revision_needed",
            Self::Rejected => "rejected",
            Self::Expired => "expired",
            Self::ExecutedDowngraded => "executed_downgraded",
            Self::ExecutedWithoutPending => "executed_without_pending",
            Self::Executed => "executed",
        }
    }
}

/// A non-authoritative alias/mapping learned from accepted or revised suggestions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttractorNamingLessonV1 {
    pub author: String,
    pub raw_label: String,
    pub resolved_label: String,
    pub suggested_action: String,
    pub status: AttractorSuggestionStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decision_reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at_unix_s: Option<f64>,
}

/// A reversible draft produced from natural attractor-adjacent language.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttractorSuggestionV1 {
    pub policy: String,
    pub schema_version: u8,
    pub suggestion_id: String,
    pub author: String,
    pub raw_action: String,
    pub raw_label: String,
    pub nearest_label: String,
    pub confidence: f32,
    pub suggested_action: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub alternatives: Vec<String>,
    pub status: AttractorSuggestionStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_kind: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub safety_context: BTreeMap<String, serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decision_reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repeat_count: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at_unix_s: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at_unix_s: Option<f64>,
}

/// Outcome classification for authored/emergent attractor behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttractorClassification {
    /// Recurrent basin appeared without explicit authorship.
    Emergent,
    /// Recurrent basin followed an explicit being-authored intent.
    Authored,
    /// Authored basin did not recur above baseline.
    Failed,
    /// Basin recurred with unsafe pressure or lock-in.
    Pathological,
}

impl AttractorClassification {
    /// Stable string for DB indexing and status surfaces.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Emergent => "emergent",
            Self::Authored => "authored",
            Self::Failed => "failed",
            Self::Pathological => "pathological",
        }
    }

    /// Conservative first-pass classification from recurrence, authorship, and safety.
    #[must_use]
    pub fn from_scores(
        recurrence_score: f32,
        authorship_score: f32,
        safety_level: SafetyLevel,
    ) -> Self {
        if safety_level.is_emergency() {
            return Self::Pathological;
        }
        let recurrence = recurrence_score.clamp(0.0, 1.0);
        let authorship = authorship_score.clamp(0.0, 1.0);
        if recurrence >= 0.60 && authorship >= 0.60 {
            Self::Authored
        } else if recurrence >= 0.45 {
            Self::Emergent
        } else {
            Self::Failed
        }
    }
}

/// Safety bounds attached to an attractor intent before any live writes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttractorSafetyBounds {
    /// Maximum fill percentage before the intent should stop or roll back.
    pub max_fill_pct: f32,
    /// Maximum lambda1 positive-energy share before dominance is considered unsafe.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_lambda1_share: Option<f32>,
    /// Minimum normalized spectral entropy before the basin is considered too collapsed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_spectral_entropy: Option<f32>,
    /// Whether this intent may send live Minime control messages.
    pub allow_live_control: bool,
    /// Whether red safety automatically means rollback to a previous seed.
    pub rollback_on_red: bool,
}

impl Default for AttractorSafetyBounds {
    fn default() -> Self {
        Self {
            max_fill_pct: 92.0,
            max_lambda1_share: Some(0.55),
            min_spectral_entropy: Some(0.60),
            allow_live_control: false,
            rollback_on_red: true,
        }
    }
}

/// Bounded control envelope an attractor intent may request.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AttractorControlEnvelope {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub synth_gain: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keep_bias: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exploration_noise: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fill_target: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub regulation_strength: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub geom_curiosity: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub geom_drive: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_lambda_bias: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pi_kp: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pi_ki: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pi_max_step: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pi_integrator_leak: Option<f32>,
}

/// Human/being-authored intervention plan for an attractor intent.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AttractorInterventionPlan {
    /// Human-readable plan mode, e.g. `semantic_seed`, `control_schedule`, `garden_clone`.
    pub mode: String,
    /// Optional deterministic vector schedule for offline or semantic seeding.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub vector_schedule: Vec<Vec<f32>>,
    /// Optional bounded live-control envelope.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub control: Option<AttractorControlEnvelope>,
    /// Optional triple-reservoir rehearsal mode such as `hold`, `rehearse`, or `quiet`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rehearsal_mode: Option<String>,
    /// Freeform notes from the author.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
}

/// Minimal being-local state captured when an attractor seed is authored.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttractorSeedSnapshotV1 {
    pub policy: String,
    pub schema_version: u8,
    pub fill_pct: f32,
    pub lambda1: f32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub eigenvalues: Vec<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spectral_fingerprint_summary: Option<Vec<f32>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub h_state_fingerprint_16: Option<Vec<f32>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub h_state_rms: Option<f32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub lexical_motifs: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub captured_at_unix_s: Option<f64>,
}

/// Optional provenance for a seed, especially promoted proto-attractors.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttractorSeedOriginV1 {
    /// Origin kind, e.g. `manual_current`, `astrid_journal_motif`, `ledger_seed`.
    pub kind: String,
    /// Optional source path, event id, or ledger row id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    /// Label or phrase that matched the promotion request.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub matched_label: Option<String>,
    /// Motifs that made the proto-attractor legible.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub motifs: Vec<String>,
    /// Capture/promotion time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub captured_at_unix_s: Option<f64>,
}

/// A being/steward intent to create, summon, compare, release, or roll back an attractor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttractorIntentV1 {
    pub policy: String,
    pub schema_version: u8,
    pub intent_id: String,
    pub author: String,
    pub substrate: AttractorSubstrate,
    pub command: AttractorCommandKind,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub goal: Option<String>,
    pub intervention_plan: AttractorInterventionPlan,
    pub safety_bounds: AttractorSafetyBounds,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_seed_id: Option<String>,
    /// Parent seed ids for derived/blended attractors.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parent_seed_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub facet_label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub facet_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub facet_kind: Option<String>,
    /// Stable id of a derived atlas entry, when this intent came from an atlas/card view.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub atlas_entry_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin: Option<AttractorSeedOriginV1>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed_snapshot: Option<AttractorSeedSnapshotV1>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at_unix_s: Option<f64>,
}

/// A measured attractor outcome after observation or replay.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttractorObservationV1 {
    pub policy: String,
    pub schema_version: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub intent_id: Option<String>,
    pub substrate: AttractorSubstrate,
    pub label: String,
    pub recurrence_score: f32,
    pub authorship_score: f32,
    pub classification: AttractorClassification,
    pub safety_level: SafetyLevel,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fill_pct: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lambda1: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lambda1_share: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spectral_entropy: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub basin_shift_score: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub facet_label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub facet_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub facet_kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub release_baseline: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub release_effect: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub garden_proof: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observed_at_unix_s: Option<f64>,
}

/// Command payload that records bolder control as attractor-scoped, not casual control.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttractorCommandV1 {
    pub policy: String,
    pub schema_version: u8,
    pub intent_id: String,
    pub author: String,
    pub substrate: AttractorSubstrate,
    pub command: AttractorCommandKind,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub control: Option<AttractorControlEnvelope>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub issued_at_unix_s: Option<f64>,
}

/// A derived, non-authoritative atlas entry built from attractor ledgers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttractorAtlasEntryV1 {
    pub policy: String,
    pub schema_version: u8,
    pub entry_id: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    pub substrate: AttractorSubstrate,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed_intent_id: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub source_intent_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parent_seed_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub facet_label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub facet_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub facet_kind: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub lifecycle_counts: BTreeMap<String, u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lifecycle_status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_reviewed_at_unix_s: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_recurrence_score: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub best_recurrence_score: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_authorship_score: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub best_authorship_score: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_classification: Option<AttractorClassification>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_safety_level: Option<SafetyLevel>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub control_eligible: Option<bool>,
    #[serde(default)]
    pub released: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub release_effect_summary: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub garden_proof: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin_kind: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub motifs: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spectral_summary: Option<AttractorSeedSnapshotV1>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub suggested_next: Vec<String>,
    /// Non-authoritative naming lessons learned from accepted/revised suggestions.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub naming_lessons: Vec<AttractorNamingLessonV1>,
}

/// A complete derived attractor atlas snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttractorAtlasV1 {
    pub policy: String,
    pub schema_version: u8,
    pub generated_at_unix_s: f64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub entries: Vec<AttractorAtlasEntryV1>,
}
