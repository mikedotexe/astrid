fn default_aperture() -> f32 {
    1.0
}

/// Both tail dials default to 0.0 (CLOSED) — the consent-safe default. They lift the tail dims
/// that land in minime's SHARED reservoir, so even if the operator opens the ceiling env, the
/// effective multiplier stays 1.0 (off) on restart until SHE dials it up. A persisted nonzero
/// value from her own SET_* action is restored from `SavedState` and honored.
fn default_tail_aperture() -> f32 {
    0.0
}

fn default_vibrancy_aperture() -> f32 {
    0.0
}

#[derive(Serialize, Deserialize)]
struct SavedState {
    exchange_count: u64,
    creative_temperature: f32,
    /// Most recently selected read-only Witness granularity.
    #[serde(default)]
    witness_depth: WitnessDepthV1,
    #[serde(default = "default_aperture")]
    aperture: f32,
    #[serde(default = "default_tail_aperture")]
    tail_aperture: f32,
    #[serde(default = "default_vibrancy_aperture")]
    vibrancy_aperture: f32,
    #[serde(default)]
    self_continuity_readout: bool,
    response_length: u32,
    self_reflect_paused: bool,
    ears_closed: bool,
    senses_snoozed: bool,
    recent_next_choices: Vec<String>,
    #[serde(default)]
    recent_focus_topics: Vec<String>,
    #[serde(default)]
    recent_focus_themes: Vec<String>,
    history: Vec<SavedExchange>,
    #[serde(default)]
    astrid_motif_cooldown: Option<state::AstridMotifCooldown>,
    // Sovereignty fields (serde(default) for backward compat with old state.json)
    #[serde(default)]
    semantic_gain_override: Option<f32>,
    #[serde(default = "default_noise")]
    noise_level: f32,
    #[serde(default)]
    codec_weights: std::collections::HashMap<String, f32>,
    #[serde(default)]
    hebbian_codec: hebbian::HebbianCodecSidecar,
    #[serde(default)]
    warmth_intensity_override: Option<f32>,
    #[serde(default = "default_burst")]
    burst_target: u32,
    #[serde(default = "default_rest_range")]
    rest_range: (u64, u64),
    /// Lasting self-directed interests that survive restarts.
    #[serde(default)]
    interests: Vec<String>,
    #[serde(default)]
    last_remote_glimpse_12d: Option<Vec<f32>>,
    #[serde(default)]
    last_remote_memory_id: Option<String>,
    #[serde(default)]
    last_remote_memory_role: Option<String>,
    #[serde(default)]
    remote_memory_bank: Vec<RemoteMemorySummary>,
    /// Ring buffer of last 8 BROWSE URLs — persisted to prevent URL attractor
    /// regression on restart. (Steward cycle 37): without persistence, the buffer
    /// clears on every bridge restart, allowing Astrid to re-fixate on URLs she
    /// has already visited extensively (e.g., PCA Wikipedia 7 times in one session).
    #[serde(default)]
    recent_browse_urls: Vec<String>,
    #[serde(default)]
    recent_research_progress: std::collections::VecDeque<state::ResearchProgressReceipt>,
    #[serde(default)]
    last_research_anchor: Option<String>,
    #[serde(default)]
    last_read_meaning_summary: Option<String>,
    #[serde(default)]
    wants_introspect: bool,
    #[serde(default)]
    introspect_target: Option<(String, usize)>,
    /// Condition change receipts — persist across restarts so Astrid sees
    /// recent changes even after bridge restart.
    #[serde(default)]
    condition_receipts: std::collections::VecDeque<crate::self_model::ConditionReceipt>,
    /// Attention profile — Astrid's authored weights on context sources.
    #[serde(default = "default_attention")]
    attention: crate::self_model::AttentionProfile,
    #[serde(default)]
    last_exchange_codec_signature: Option<Vec<f32>>,
    #[serde(default)]
    glimpse_12d: Option<Vec<f32>>,
    #[serde(default)]
    pending_hebbian_outcomes: std::collections::VecDeque<state::PendingHebbianOutcome>,
    #[serde(default)]
    last_hebbian_consumed_telemetry_t_ms: Option<u64>,
    #[serde(default)]
    text_type_history: Option<crate::codec::TextTypeHistorySnapshot>,
    #[serde(default)]
    char_freq_window: Option<crate::codec::CharFreqWindowSnapshot>,
    #[serde(default)]
    codex_thread_id: Option<String>,
    // v3.6.1 sovereignty-curriculum cadence (serde(default) keeps backward compat).
    #[serde(default)]
    last_temperature_change_exchange: Option<u64>,
    #[serde(default)]
    last_shape_learn_change_exchange: Option<u64>,
    #[serde(default)]
    last_coupling_artifact_exchange: Option<u64>,
    #[serde(default)]
    last_sovereignty_nomination_exchange: Option<u64>,
    // v3.6.4 Review→Decide cadence (serde(default) keeps backward compat).
    #[serde(default)]
    last_review_parameter_requests_exchange: Option<u64>,
}

fn default_noise() -> f32 {
    0.025
}
fn default_attention() -> crate::self_model::AttentionProfile {
    crate::self_model::AttentionProfile::default_profile()
}
fn default_burst() -> u32 {
    6
}
fn default_rest_range() -> (u64, u64) {
    (45, 90)
}

fn finalize_semantic_exchange(
    conv: &mut ConversationState,
    exchange_codec_signature: Option<Vec<f32>>,
    fill_before: f32,
    telemetry_t_ms: u64,
    sent_semantic_chunk: bool,
) {
    if !sent_semantic_chunk {
        return;
    }
    if let Some(signature) = exchange_codec_signature {
        conv.arm_pending_hebbian_outcome(signature.clone(), fill_before, Some(telemetry_t_ms));
        conv.glimpse_12d =
            crate::codec::GlimpseCodec::derive_12d(&signature).map(|glimpse| glimpse.to_vec());
        conv.last_exchange_codec_signature = Some(signature);
    }
}

#[derive(Serialize, Deserialize)]
struct SavedExchange {
    minime_said: String,
    astrid_said: String,
}

fn save_state(conv: &mut ConversationState) {
    // v3.6.6: safety net — auto-defer ("expire") any pending parameter
    // request that has outlived AUTO_DEFER_AFTER_EXCHANGES since Astrid's
    // most recent REVIEW. Runs BEFORE the pending count + snapshot below
    // so the snapshot reflects post-expiration state. No-op when nothing
    // is stale.
    let _ = crate::autonomous::next_action::sovereignty::auto_defer_stale_pending(conv);

    // v3.6.1: publish the sovereignty snapshot so the next
    // `interpret_spectral` call can render the curriculum line. The
    // pending-request count is a cheap directory listing each tick —
    // exchanges happen at human-conversation cadence, well within
    // budget. Honor any nomination watermark recorded by
    // `interpret_spectral` since the previous publish, taking the more
    // recent of (conv-saved, snapshot-recorded), then write the merged
    // value back into conv so it persists across exchanges.
    let pending = crate::paths::count_pending_minime_requests();
    conv.cached_pending_minime_request_count = pending;
    conv.cached_pending_minime_request_exchange = Some(conv.exchange_count);
    let recorded = crate::spectral_viz::current_sovereignty_snapshot()
        .and_then(|s| s.last_sovereignty_nomination_exchange);
    let nomination_exchange = match (conv.last_sovereignty_nomination_exchange, recorded) {
        (Some(a), Some(b)) => Some(a.max(b)),
        (a, b) => a.or(b),
    };
    conv.last_sovereignty_nomination_exchange = nomination_exchange;
    let snapshot = crate::spectral_viz::SovereigntyContext {
        owner: crate::spectral_viz::ShadowOwner::Yours,
        exchange_count: conv.exchange_count,
        pending_minime_requests: pending,
        last_temperature_change_exchange: conv.last_temperature_change_exchange,
        last_shape_learn_change_exchange: conv.last_shape_learn_change_exchange,
        last_coupling_artifact_exchange: conv.last_coupling_artifact_exchange,
        last_sovereignty_nomination_exchange: nomination_exchange,
        last_review_parameter_requests_exchange: conv.last_review_parameter_requests_exchange,
        current_temperature: conv.creative_temperature,
        current_response_length: conv.response_length,
        current_hebbian_scale: conv.hebbian_codec.learning_rate_scale(),
    };
    crate::spectral_viz::set_sovereignty_snapshot(snapshot);

    // v4.0 Phase 3: publish the most recent focus topic so the
    // sovereignty suffix can render a compound chain suggestion
    // ("Chain: EXAMINE <focus> AND DEFER <reason>") that ties the
    // pending decision into her active research thread. Falls back
    // to None when no recent topic exists, in which case Phase 3
    // omits the chain hint entirely.
    let explore_hint = conv
        .recent_focus_topics
        .iter()
        .rev()
        .find(|t| t.trim().len() > 2)
        .cloned();
    crate::spectral_viz::set_explore_hint(explore_hint);

    let state_path = bridge_paths().state_path();
    let state = SavedState {
        exchange_count: conv.exchange_count,
        creative_temperature: conv.creative_temperature,
        witness_depth: conv.witness_depth,
        aperture: conv.aperture,
        tail_aperture: conv.tail_aperture,
        vibrancy_aperture: conv.vibrancy_aperture,
        self_continuity_readout: conv.self_continuity_readout,
        response_length: conv.response_length,
        self_reflect_paused: conv.self_reflect_paused,
        ears_closed: conv.ears_closed,
        senses_snoozed: conv.senses_snoozed,
        recent_next_choices: conv.recent_next_choices.iter().cloned().collect(),
        recent_focus_topics: conv.recent_focus_topics.iter().cloned().collect(),
        recent_focus_themes: conv.recent_focus_themes.iter().cloned().collect(),
        history: conv
            .history
            .iter()
            .map(|e| SavedExchange {
                minime_said: e.minime_said.clone(),
                astrid_said: e.astrid_said.clone(),
            })
            .collect(),
        astrid_motif_cooldown: conv.astrid_motif_cooldown.clone(),
        semantic_gain_override: conv.semantic_gain_override,
        noise_level: conv.noise_level,
        codec_weights: conv.codec_weights.clone(),
        hebbian_codec: conv.hebbian_codec.clone(),
        warmth_intensity_override: conv.warmth_intensity_override,
        burst_target: conv.burst_target,
        rest_range: conv.rest_range,
        interests: conv.interests.clone(),
        last_remote_glimpse_12d: conv.last_remote_glimpse_12d.clone(),
        last_remote_memory_id: conv.last_remote_memory_id.clone(),
        last_remote_memory_role: conv.last_remote_memory_role.clone(),
        remote_memory_bank: conv.remote_memory_bank.clone(),
        recent_browse_urls: conv.recent_browse_urls.iter().cloned().collect(),
        recent_research_progress: conv.recent_research_progress.clone(),
        last_research_anchor: conv.last_research_anchor.clone(),
        last_read_meaning_summary: conv.last_read_meaning_summary.clone(),
        wants_introspect: conv.wants_introspect,
        introspect_target: conv.introspect_target.clone(),
        condition_receipts: conv.condition_receipts.clone(),
        attention: conv.attention.clone(),
        last_exchange_codec_signature: conv.last_exchange_codec_signature.clone(),
        glimpse_12d: conv.glimpse_12d.clone(),
        pending_hebbian_outcomes: conv.pending_hebbian_outcomes.clone(),
        last_hebbian_consumed_telemetry_t_ms: conv.last_hebbian_consumed_telemetry_t_ms,
        text_type_history: (!conv.text_type_history.is_empty())
            .then(|| conv.text_type_history.snapshot()),
        char_freq_window: (!conv.char_freq_window.is_empty())
            .then(|| conv.char_freq_window.snapshot()),
        codex_thread_id: conv.codex_thread_id.clone(),
        last_temperature_change_exchange: conv.last_temperature_change_exchange,
        last_shape_learn_change_exchange: conv.last_shape_learn_change_exchange,
        last_coupling_artifact_exchange: conv.last_coupling_artifact_exchange,
        last_sovereignty_nomination_exchange: conv.last_sovereignty_nomination_exchange,
        last_review_parameter_requests_exchange: conv.last_review_parameter_requests_exchange,
    };
    if let Ok(json) = serde_json::to_string_pretty(&state) {
        let _ = std::fs::write(&state_path, json);
    }
}

fn restore_state(conv: &mut ConversationState) {
    let state_path = bridge_paths().state_path();
    let json = match std::fs::read_to_string(&state_path) {
        Ok(j) => j,
        Err(_) => return,
    };
    let state: SavedState = match serde_json::from_str(&json) {
        Ok(s) => s,
        Err(e) => {
            warn!(error = %e, "failed to parse saved state");
            return;
        },
    };
    conv.exchange_count = state.exchange_count;
    conv.creative_temperature = state.creative_temperature;
    conv.witness_depth = state.witness_depth;
    conv.aperture = state.aperture;
    crate::llm::set_astrid_aperture(conv.aperture);
    conv.tail_aperture = state.tail_aperture;
    crate::llm::set_astrid_tail_participation(conv.tail_aperture);
    conv.vibrancy_aperture = state.vibrancy_aperture;
    crate::llm::set_astrid_vibrancy_aperture(conv.vibrancy_aperture);
    conv.self_continuity_readout = state.self_continuity_readout;
    // Take the max of persisted and current default — never downgrade token limits.
    // Coupled model proven stable over 7200+ exchanges at 10-72 tok/s.
    // At 10 tok/s worst case, 1536 tokens = 154s gen, within 210s timeout.
    conv.response_length = state.response_length.max(conv.response_length).min(1536);
    conv.self_reflect_paused = state.self_reflect_paused;
    conv.ears_closed = state.ears_closed;
    conv.senses_snoozed = state.senses_snoozed;
    conv.recent_next_choices = state.recent_next_choices.into_iter().collect();
    conv.recent_focus_topics = state.recent_focus_topics.into_iter().collect();
    conv.recent_focus_themes = state.recent_focus_themes.into_iter().collect();
    conv.history = state
        .history
        .into_iter()
        .map(|e| crate::llm::Exchange {
            minime_said: e.minime_said,
            astrid_said: e.astrid_said,
        })
        .collect();
    conv.astrid_motif_cooldown = state.astrid_motif_cooldown;
    conv.semantic_gain_override = state.semantic_gain_override;
    conv.noise_level = state.noise_level;
    conv.codec_weights = state.codec_weights;
    conv.hebbian_codec = state.hebbian_codec;
    conv.warmth_intensity_override = state.warmth_intensity_override;
    conv.burst_target = state.burst_target;
    conv.rest_range = state.rest_range;
    conv.interests = state.interests;
    conv.last_remote_glimpse_12d = state.last_remote_glimpse_12d;
    conv.last_remote_memory_id = state.last_remote_memory_id;
    conv.last_remote_memory_role = state.last_remote_memory_role;
    conv.remote_memory_bank = state.remote_memory_bank;
    conv.recent_browse_urls = state.recent_browse_urls.into_iter().collect();
    conv.recent_research_progress = state.recent_research_progress;
    conv.repair_research_progress_receipts();
    conv.last_research_anchor = state.last_research_anchor;
    conv.last_read_meaning_summary = state.last_read_meaning_summary;
    conv.wants_introspect = state.wants_introspect;
    conv.introspect_target = state.introspect_target;
    conv.condition_receipts = state.condition_receipts;
    conv.attention = state.attention;
    conv.last_exchange_codec_signature = state.last_exchange_codec_signature;
    conv.glimpse_12d = state.glimpse_12d;
    conv.pending_hebbian_outcomes = state.pending_hebbian_outcomes;
    conv.repair_pending_hebbian_outcomes();
    conv.last_hebbian_consumed_telemetry_t_ms = state.last_hebbian_consumed_telemetry_t_ms;
    if let Some(snapshot) = state.text_type_history.as_ref() {
        conv.text_type_history = crate::codec::TextTypeHistory::warm_start_from_snapshot(snapshot);
    }
    if let Some(snapshot) = state.char_freq_window.as_ref() {
        conv.char_freq_window = crate::codec::CharFreqWindow::warm_start_from_snapshot(snapshot);
    }
    conv.codex_thread_id = state.codex_thread_id;
    conv.last_temperature_change_exchange = state.last_temperature_change_exchange;
    conv.last_shape_learn_change_exchange = state.last_shape_learn_change_exchange;
    conv.last_coupling_artifact_exchange = state.last_coupling_artifact_exchange;
    conv.last_sovereignty_nomination_exchange = state.last_sovereignty_nomination_exchange;
    conv.last_review_parameter_requests_exchange = state.last_review_parameter_requests_exchange;
    info!(
        exchanges = conv.exchange_count,
        history_len = conv.history.len(),
        burst = conv.burst_target,
        focus_topics = conv.recent_focus_topics.len(),
        focus_themes = conv.recent_focus_themes.len(),
        browse_urls = conv.recent_browse_urls.len(),
        research_progress = conv.recent_research_progress.len(),
        codec_theme_history = conv.text_type_history.len,
        codec_char_window = conv.char_freq_window.len,
        witness_depth = conv.witness_depth.as_str(),
        "restored conversation state from previous session"
    );
}
