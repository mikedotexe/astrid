/// Arrival-cadence truth for the telemetry WebSocket.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryHeartbeatDeltaV1 {
    pub policy: String,
    pub schema_version: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_arrival_unix_s: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_arrival_unix_s: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inter_arrival_ms: Option<f32>,
    pub jitter_class: String,
    pub timing_reliability: String,
    pub reconnect_count: u64,
    pub disconnect_count: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_connection_id: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_connection_started_at_unix_s: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_valid_packet_at_unix_s: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_valid_packet_lag_ms: Option<f32>,
    /// Spectral entropy observed on the first schema-valid packet for this
    /// connection. It is evidence about the initial field, not handshake data.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_valid_spectral_entropy: Option<f32>,
    #[serde(default)]
    pub connection_perception_state: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cadence_clarity_score: Option<f32>,
    #[serde(default)]
    pub cadence_clarity_basis: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_disconnect_reason: Option<String>,
    pub field_vs_hearing: String,
}

/// Read-only schema truth around the typed spectral fingerprint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpectralFingerprintIntegrityV1 {
    pub policy: String,
    pub schema_version: u8,
    pub status: String,
    pub typed_present: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub legacy_vector_len: Option<usize>,
    pub typed_precedence_over_legacy: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hybrid_coherence_index: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hybrid_max_abs_delta: Option<f32>,
    #[serde(default)]
    pub hybrid_coherence_state: String,
    #[serde(default)]
    pub hybrid_coherence_basis: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_pairwise_overlap: Option<f32>,
    #[serde(default)]
    pub mode_collision_review_threshold: f32,
    #[serde(default)]
    pub mode_collision_state: String,
    #[serde(default)]
    pub issues: Vec<String>,
    pub summary: String,
    pub authority: String,
}

fn spectral_fingerprint_hybrid_coherence_v1(
    typed: &SpectralFingerprintV1,
    legacy: &[f32],
) -> Option<(f32, f32)> {
    if legacy.len() != 32 || legacy.iter().any(|value| !value.is_finite()) {
        return None;
    }
    let typed_slots = typed.to_legacy_slots();
    if typed_slots.len() != legacy.len() || typed_slots.iter().any(|value| !value.is_finite()) {
        return None;
    }

    let mut delta_squared = 0.0_f64;
    let mut typed_squared = 0.0_f64;
    let mut legacy_squared = 0.0_f64;
    let mut max_abs_delta = 0.0_f64;
    for (typed_value, legacy_value) in typed_slots.iter().zip(legacy) {
        let typed_value = f64::from(*typed_value);
        let legacy_value = f64::from(*legacy_value);
        let delta = typed_value - legacy_value;
        delta_squared = delta.mul_add(delta, delta_squared);
        typed_squared = typed_value.mul_add(typed_value, typed_squared);
        legacy_squared = legacy_value.mul_add(legacy_value, legacy_squared);
        max_abs_delta = max_abs_delta.max(delta.abs());
    }

    let count = 32.0_f64;
    let delta_rms = (delta_squared / count).sqrt();
    let typed_rms = (typed_squared / count).sqrt();
    let legacy_rms = (legacy_squared / count).sqrt();
    let reference_rms = typed_rms.max(legacy_rms);
    let coherence = if reference_rms <= f64::EPSILON {
        if delta_rms <= f64::EPSILON {
            1.0
        } else {
            0.0
        }
    } else {
        (1.0 - delta_rms / reference_rms).clamp(0.0, 1.0)
    };
    Some((coherence as f32, max_abs_delta as f32))
}

/// Raw telemetry broadcast by minime's ESN engine on port 7878.
///
/// Maps to `EigenPacket` in `minime/src/main.rs`. Sent as `Message::Text(json)`.
/// Note: minime also has `SpectralMsg` in `net/ws_server.rs` but that type
/// is used by the `WsHub` (not the main broadcast loop on port 7878).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpectralTelemetry {
    /// Timestamp in milliseconds since engine start.
    pub t_ms: u64,
    /// All eigenvalues (variable length, typically 3-8).
    pub eigenvalues: Vec<f32>,
    /// Eigenvalue fill ratio (0.0 - 1.0, NOT percentage).
    pub fill_ratio: f32,
    /// Number of active eigenvalue modes selected by minime's live estimator.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_mode_count: Option<usize>,
    /// Energy ratio carried by the selected active mode prefix.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_mode_energy_ratio: Option<f32>,
    /// Dominant covariance eigenvalue relative to minime's current baseline.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lambda1_rel: Option<f32>,
    /// Modality firing status.
    #[serde(default)]
    pub modalities: Option<ModalityStatus>,
    /// Neural network outputs (if enabled).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub neural: Option<serde_json::Value>,
    /// Alert string from the ESN (e.g. panic mode).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alert: Option<String>,
    /// 32D spectral geometry fingerprint: eigenvalues, eigenvector concentration,
    /// inter-mode coupling, spectral entropy, gap ratios, rotation rate.
    /// Enables Astrid to perceive the shape of the spectral landscape.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spectral_fingerprint: Option<Vec<f32>>,
    /// Typed view of the 32D spectral geometry fingerprint.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spectral_fingerprint_v1: Option<SpectralFingerprintV1>,
    /// Typed read-only metric for recursive compression / distinguishability.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spectral_denominator_v1: Option<SpectralDenominatorV1>,
    /// Inverse-participation effective mode count derived from eigenvalues.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effective_dimensionality: Option<f32>,
    /// 0=open distributed fabric, 1=collapsed into the fewest active modes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub distinguishability_loss: Option<f32>,
    /// Current effective ESN leak exported by Minime. Adaptive unless a gated
    /// direct leak microdose override is active.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub esn_leak: Option<f32>,
    /// Active direct leak override status, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub esn_leak_override_v1: Option<serde_json::Value>,
    /// Structural diversity of the live eigenvector/coupling geometry.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub structural_entropy: Option<f32>,
    /// Density of mutually reinforcing resonance in the current eigenspace.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resonance_density_v1: Option<ResonanceDensityV1>,
    /// Read-only explanation of where inward/compression pressure appears to originate.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pressure_source_v1: Option<PressureSourceV1>,
    /// Whether fluctuation remains returnable and inhabitable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inhabitable_fluctuation_v1: Option<InhabitableFluctuationV1>,
    /// Selected 12D vague-memory glimpse from Minime's memory bank.
    #[serde(
        default,
        alias = "glimpse_12d",
        skip_serializing_if = "Option::is_none"
    )]
    pub spectral_glimpse_12d: Option<Vec<f32>>,
    /// Compact top-k eigenvector landmarks/overlaps from Minime's raw live eigenvectors.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub eigenvector_field: Option<serde_json::Value>,
    /// Stable-core runtime state from Minime, including read-only sensory gate budget.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stable_core: Option<serde_json::Value>,
    /// Legacy semantic-energy bundle from Minime.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub semantic: Option<serde_json::Value>,
    /// Typed semantic split: input content, kernel admission, regulator drive.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub semantic_energy_v1: Option<serde_json::Value>,
    /// Legacy transition event compatibility object.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transition_event: Option<serde_json::Value>,
    /// Typed transition event object from Minime.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transition_event_v1: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_memory_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_memory_role: Option<String>,
    /// Ising/Hamiltonian shadow observer metrics — a second physics lens
    /// on the spectral dynamics. Observer-only: does not affect the ESN.
    /// Fields: mode_dim, field_norm, soft_energy, soft_magnetization,
    /// binary_energy, binary_magnetization, binary_flip_rate.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ising_shadow: Option<serde_json::Value>,
    /// V2 reduced-Hamiltonian shadow field — gates `SHADOW_PREFLIGHT` /
    /// `SHADOW_INFLUENCE` typed actions. Surfaced into the prompt by
    /// `interpret_spectral` so the action is reachable in any mode.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shadow_field_v2: Option<serde_json::Value>,
    /// V3 shadow field — wraps V2 plus trajectory ring, compound traits,
    /// phase dwell, and recent transitions. Enables the dual-shadow prompt
    /// line and mutual-witness rendering once Astrid's own shadow lands.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shadow_field_v3: Option<serde_json::Value>,
    /// V3 closed-loop influence response: pre/post deltas, basin shift,
    /// per-mode shift vector. Populated by minime after each influence
    /// window; read by Astrid's `SHADOW_RESPONSE` action.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shadow_influence_response_v3: Option<serde_json::Value>,
    /// Read-only residual deformation trace for "the spike ended but the
    /// texture is still altered" reports. This never changes pressure/fill
    /// control; it only exposes bounded evidence and optional delta refs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub residual_deformation_trace_v1: Option<ResidualDeformationTraceV1>,
}

impl SpectralTelemetry {
    /// Extract the dominant eigenvalue (lambda1 = eigenvalues\[0\]).
    #[must_use]
    pub fn lambda1(&self) -> f32 {
        self.eigenvalues.first().copied().unwrap_or(0.0)
    }

    /// Fill ratio as a percentage (0-100).
    #[must_use]
    pub fn fill_pct(&self) -> f32 {
        self.fill_ratio * 100.0
    }

    /// Validated additive 12D glimpse view. Malformed vectors are retained in
    /// raw telemetry for diagnosis, but never treated as prompt/state signal.
    #[must_use]
    pub fn spectral_glimpse_12d_view(&self) -> Option<&[f32]> {
        self.spectral_glimpse_12d
            .as_deref()
            .filter(|values| values.len() == 12 && values.iter().all(|value| value.is_finite()))
    }

    /// Typed spectral fingerprint, reconstructed from legacy slots when needed.
    #[must_use]
    pub fn typed_fingerprint(&self) -> Option<SpectralFingerprintV1> {
        SpectralFingerprintV1::from_telemetry(self)
    }

    /// Diagnostic readout for whether legacy/typed fingerprint payloads are coherent.
    #[must_use]
    pub fn spectral_fingerprint_integrity_v1(&self) -> SpectralFingerprintIntegrityV1 {
        const HYBRID_COHERENCE_ALIGNED_AT: f32 = 0.995;
        const HYBRID_COHERENCE_NEAR_AT: f32 = 0.90;
        const HYBRID_COHERENCE_MIXED_AT: f32 = 0.50;
        const MODE_COLLISION_REVIEW_OVERLAP: f32 = 0.90;

        let typed_present = self.spectral_fingerprint_v1.is_some();
        let legacy_vector_len = self.spectral_fingerprint.as_ref().map(Vec::len);
        let typed_precedence_over_legacy = typed_present && legacy_vector_len.is_some();
        let hybrid_coherence = self
            .spectral_fingerprint_v1
            .as_ref()
            .zip(
                self.spectral_fingerprint
                    .as_deref()
                    .filter(|legacy| legacy.len() == 32),
            )
            .and_then(|(typed, legacy)| {
                spectral_fingerprint_hybrid_coherence_v1(typed, legacy)
            });
        let hybrid_coherence_index = hybrid_coherence.map(|(coherence, _)| coherence);
        let hybrid_max_abs_delta = hybrid_coherence.map(|(_, max_delta)| max_delta);
        let hybrid_coherence_state = if let Some(coherence) = hybrid_coherence_index {
            if coherence >= HYBRID_COHERENCE_ALIGNED_AT {
                "aligned"
            } else if coherence >= HYBRID_COHERENCE_NEAR_AT {
                "near_aligned"
            } else if coherence >= HYBRID_COHERENCE_MIXED_AT {
                "mixed_transition"
            } else {
                "divergent"
            }
        } else if typed_present && legacy_vector_len == Some(32) {
            "unavailable_non_finite"
        } else if typed_present && legacy_vector_len.is_some() {
            "unavailable_malformed_legacy"
        } else if typed_present {
            "typed_only"
        } else if legacy_vector_len == Some(32) {
            "legacy_only"
        } else if legacy_vector_len.is_some() {
            "unavailable_malformed_legacy"
        } else {
            "unavailable"
        }
        .to_string();
        let max_pairwise_overlap = self
            .eigenvector_field
            .as_ref()
            .and_then(|field| field.pointer("/summary/max_pairwise_overlap"))
            .and_then(serde_json::Value::as_f64)
            .filter(|overlap| overlap.is_finite())
            .map(|overlap| overlap as f32);
        let mut issues = Vec::new();
        if let Some(len) = legacy_vector_len
            && len != 32
        {
            issues.push(format!("legacy_vector_len_{len}_expected_32"));
        }
        if !typed_present && legacy_vector_len.is_none() {
            issues.push("fingerprint_absent".to_string());
        }
        if hybrid_coherence_index
            .is_some_and(|coherence| coherence < HYBRID_COHERENCE_ALIGNED_AT)
        {
            issues.push("typed_legacy_hybrid_mismatch".to_string());
        }
        if hybrid_coherence_index
            .is_some_and(|coherence| coherence < HYBRID_COHERENCE_MIXED_AT)
        {
            issues.push("typed_legacy_hybrid_divergence".to_string());
        }
        let mode_collision_state =
            if max_pairwise_overlap.is_some_and(|overlap| overlap >= MODE_COLLISION_REVIEW_OVERLAP)
            {
                issues.push("eigenvector_mode_collision_review_required".to_string());
                "review_required_high_overlap"
            } else if max_pairwise_overlap.is_some() {
                "below_review_threshold"
            } else {
                "not_reported"
            }
            .to_string();
        let status = if typed_present {
            "typed_canonical"
        } else if legacy_vector_len == Some(32) {
            "legacy_32d_accepted"
        } else if legacy_vector_len.is_some() {
            "malformed_legacy_vector"
        } else {
            "absent"
        }
        .to_string();
        let summary = if typed_present {
            if typed_precedence_over_legacy {
                "spectral_fingerprint_v1 present; typed payload takes precedence over legacy spectral_fingerprint slots"
            } else {
                "spectral_fingerprint_v1 present; canonical typed payload available"
            }
        } else if legacy_vector_len == Some(32) {
            "legacy spectral_fingerprint has 32 values and can reconstruct spectral_fingerprint_v1"
        } else if let Some(len) = legacy_vector_len {
            return SpectralFingerprintIntegrityV1 {
                policy: "spectral_fingerprint_integrity_v1".to_string(),
                schema_version: 1,
                status,
                typed_present,
                legacy_vector_len,
                typed_precedence_over_legacy,
                hybrid_coherence_index,
                hybrid_max_abs_delta,
                hybrid_coherence_state,
                hybrid_coherence_basis:
                    "normalized_rms_agreement_across_canonical_32_slots".to_string(),
                max_pairwise_overlap,
                mode_collision_review_threshold: MODE_COLLISION_REVIEW_OVERLAP,
                mode_collision_state,
                issues,
                summary: format!(
                    "legacy spectral_fingerprint has {len} values; expected 32, so typed reconstruction is blocked"
                ),
                authority: "diagnostic_context_not_control".to_string(),
            };
        } else {
            "no spectral fingerprint payload present"
        }
        .to_string();
        let summary = hybrid_coherence_index.map_or(summary.clone(), |coherence| {
            format!(
                "{summary}; typed/legacy hybrid coherence={coherence:.3} state={hybrid_coherence_state}"
            )
        });

        SpectralFingerprintIntegrityV1 {
            policy: "spectral_fingerprint_integrity_v1".to_string(),
            schema_version: 1,
            status,
            typed_present,
            legacy_vector_len,
            typed_precedence_over_legacy,
            hybrid_coherence_index,
            hybrid_max_abs_delta,
            hybrid_coherence_state,
            hybrid_coherence_basis:
                "normalized_rms_agreement_across_canonical_32_slots".to_string(),
            max_pairwise_overlap,
            mode_collision_review_threshold: MODE_COLLISION_REVIEW_OVERLAP,
            mode_collision_state,
            issues,
            summary,
            authority: "diagnostic_context_not_control".to_string(),
        }
    }

    /// Typed denominator/recursive-compression metric, derived when needed.
    #[must_use]
    pub fn denominator_metrics(&self) -> Option<SpectralDenominatorV1> {
        self.spectral_denominator_v1.clone().or_else(|| {
            self.typed_fingerprint()
                .map(|fingerprint| fingerprint.denominator_metrics())
                .or_else(|| SpectralDenominatorV1::from_eigenvalues(&self.eigenvalues, None))
        })
    }

    /// Typed semantic-energy view, reconstructed from the legacy semantic object when needed.
    #[must_use]
    pub fn semantic_energy_view(&self) -> Option<SemanticEnergyV1> {
        self.semantic_energy_v1
            .as_ref()
            .and_then(SemanticEnergyV1::from_typed_value)
            .or_else(|| {
                self.semantic
                    .as_ref()
                    .and_then(SemanticEnergyV1::from_legacy_semantic)
            })
    }

    /// Typed transition-event view, preserving raw JSON compatibility.
    #[must_use]
    pub fn transition_event_view(&self) -> Option<TransitionEventV1> {
        self.transition_event_v1
            .as_ref()
            .and_then(TransitionEventV1::from_value)
            .or_else(|| {
                self.transition_event
                    .as_ref()
                    .and_then(TransitionEventV1::from_value)
            })
    }

    /// Typed eigenvector-field view, preserving the raw compact payload.
    #[must_use]
    pub fn eigenvector_field_view(&self) -> Option<EigenvectorFieldV1> {
        self.eigenvector_field
            .as_ref()
            .and_then(EigenvectorFieldV1::from_value)
    }
}
