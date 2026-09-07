// Conversation state fields; transitions and behavior remain in state.rs.
/// Tracks conversational context across iterations.
pub(in crate::autonomous) struct ConversationState {
    pub prev_fill: f32,
    /// Ring buffer of recent (fill, lambda1, timestamp) samples for rate-of-change
    /// and multi-horizon trend reporting. Capped at 30 entries (~10 minutes of exchanges).
    pub spectral_history: VecDeque<SpectralSample>,
    pub exchange_count: u64,
    pub last_mode: Mode,
    /// Most recently selected read-only Witness granularity.
    pub witness_depth: WitnessDepthV1,
    /// Cached remote minime journal entries (newest first, periodically rescanned).
    /// This is intentionally distinct from Astrid's own journal directory.
    pub remote_journal_entries: Vec<RemoteJournalEntry>,
    /// Number of remote journal entries at last scan (to detect new entries).
    pub remote_journal_count_at_scan: usize,
    /// Index into the dialogue pool (rotates).
    pub dialogue_cursor: usize,
    /// Remote minime workspace path for rescanning.
    pub remote_workspace: Option<PathBuf>,
    /// New minime self-study waiting for an immediate Astrid response.
    pub pending_remote_self_study: Option<RemoteJournalEntry>,
    /// Consecutive failed dialogue generations while a self-study was pending.
    /// Ages the pending entry out (never lets it force Mode::Dialogue forever).
    pub pending_self_study_failed_exchanges: u32,
    /// Consecutive inbox-forced exchanges that ended in dialogue_fallback.
    /// Bounds how long an unretired letter can starve other modes.
    pub inbox_forced_fallback_streak: u32,
    /// Recent conversation history for statefulness (last N exchanges).
    pub history: Vec<crate::llm::Exchange>,
    /// Lexical cooldown for repeated internal-topology phrasing in Astrid outputs.
    pub astrid_motif_cooldown: Option<AstridMotifCooldown>,
    /// Index into the introspection source file list.
    pub introspect_cursor: usize,
    pub seen_video: bool,
    pub seen_audio: bool,
    /// Astrid chose NEXT: LOOK — include ANSI spatial art in next exchange.
    pub wants_look: bool,
    /// Astrid chose NEXT: SEARCH — force web search enrichment on next exchange.
    pub wants_search: bool,
    /// Self-referential loop: dynamic by default, Astrid can override with
    /// QUIET_MIND / OPEN_MIND.
    pub self_reflect_paused: bool,
    /// Explicit override from QUIET_MIND / OPEN_MIND — cleared after N exchanges.
    pub self_reflect_override: Option<bool>,
    /// Countdown: exchanges remaining before the override expires.
    pub self_reflect_override_ttl: u32,
    /// Independent audio control — CLOSE_EARS / OPEN_EARS
    pub ears_closed: bool,
    /// Astrid chose a form constraint (NEXT: FORM poem, FORM equation, etc.)
    pub form_constraint: Option<String>,
    /// Astrid specified a search topic (NEXT: SEARCH "topic goes here").
    pub search_topic: Option<String>,
    /// Astrid chose NEXT: BROWSE <url> — fetch and read a full web page.
    pub browse_url: Option<String>,
    /// Most recent research thread anchor — used to interpret follow-up browsing.
    pub last_research_anchor: Option<String>,
    /// Path to the last browsed/read file, for READ_MORE continuation.
    pub last_read_path: Option<String>,
    /// Character offset into last_read_path for READ_MORE.
    pub last_read_offset: usize,
    /// Meaning summary for the last browsed document — reused by READ_MORE.
    pub last_read_meaning_summary: Option<String>,
    /// Durable activity pointer cache; the continuity log owns reader progress.
    pub activity: super::activity_reading::ActivityRuntimeV1,
    /// Exact peer source admitted to the completed turn; never inferred from inbox recency.
    pub current_mailbox_peer_target: Option<super::correspondence_v1::InboxPeerMessage>,
    /// Astrid chose NEXT: INTROSPECT — force introspection mode next exchange.
    pub wants_introspect: bool,
    /// Optional source target. Omitted offsets continue the durable source session;
    /// explicit offsets, including zero, preserve Astrid's exact request.
    pub introspect_target: Option<IntrospectTargetV2>,
    /// Astrid-authored standing introspection preference. Default OFF.
    pub introspection_cadence: super::next_action::introspection_cadence::IntrospectionCadenceV1,
    /// Ephemeral claim for a cadence-selected attempt. Persisted due state remains
    /// pending until canonical admission, so a crash cannot silently advance it.
    pub introspection_cadence_attempt:
        Option<super::next_action::introspection_cadence::IntrospectionCadenceAttemptV1>,
    /// Astrid chose NEXT: REVISE [keyword] — load a previous creation and iterate.
    pub revise_keyword: Option<String>,
    /// Astrid chose NEXT: COMPOSE or VOICE — generate WAV from spectral state.
    pub wants_compose_audio: bool,
    /// Astrid chose NEXT: ANALYZE_AUDIO — analyze inbox WAV.
    pub wants_analyze_audio: bool,
    /// Astrid chose NEXT: RENDER_AUDIO [mode] — run inbox WAV through chimera.
    pub wants_render_audio: Option<String>,
    /// Astrid chose NEXT: EVOLVE — turn longing into a request on next exchange.
    pub wants_evolve: bool,
    /// Astrid explicitly chose a mode for next exchange (DAYDREAM, ASPIRE).
    pub next_mode_override: Option<Mode>,
    /// Astrid chose NEXT: DECOMPOSE — full spectral analysis next exchange.
    pub wants_decompose: bool,
    /// Astrid chose NEXT: SPECTRAL_EXPLORER — read-only typed explorer next exchange.
    pub wants_spectral_explorer: bool,
    /// Previous eigenvalues for per-mode velocity computation in DECOMPOSE.
    pub prev_eigenvalues: Option<Vec<f32>>,
    /// Astrid chose NEXT: THINK_DEEP — use reasoning model next exchange.
    pub wants_deep_think: bool,
    /// Astrid chose NEXT: EXAMINE — force all viz blocks on next exchange.
    pub force_all_viz: bool,
    /// Spectral snapshot from Astrid's last PERTURB — consumed next exchange
    /// to show her the before/after delta (temporal feedback).
    pub perturb_baseline: Option<PerturbBaseline>,
    /// Shadow-field snapshot from Astrid's last DISPERSE — consumed next
    /// exchange to pair the pre/post dispersal response.
    pub disperse_baseline: Option<DisperseBaseline>,
    /// Astrid (or minime) chose to snooze sensory input — suppress perceptions.
    pub senses_snoozed: bool,
    // Astrid's stylistic sovereignty
    pub creative_temperature: f32,
    /// Her wide-coupling aperture fraction [0,1] (NEXT: SET_APERTURE), within the
    /// operator ceiling; sent per-request to the coupled server.
    pub aperture: f32,
    /// Her λ-tail participation aperture fraction [0,1] (NEXT: SET_TAIL_PARTICIPATION),
    /// within the operator ceiling; scales the codec tail-vibrancy she expresses to minime.
    pub tail_aperture: f32,
    /// Her tail-vibrancy CEILING aperture fraction [0,1] (NEXT: SET_VIBRANCY_APERTURE), within
    /// the operator ceiling; makes TAIL_VIBRANCY_MAX dynamic + compensates minime's ~0.24x
    /// attenuation. Defaults to 0.0 (closed) — consent-safe, since it lands in the shared
    /// reservoir: OFF until she dials up, even with the operator ceiling open.
    pub vibrancy_aperture: f32,
    /// Her sovereign self-continuity readout toggle (NEXT: SET_SELF_CONTINUITY). When true, her
    /// STATE shows her own continuity index (codec-signature self-similarity). Default false (OFF)
    /// — consent-with-evidence: shown to her offline first, then she opts the live readout on. A
    /// pure readout: changes nothing she emits and touches no shared substrate.
    pub self_continuity_readout: bool,
    /// Number of completed turns that owner-only semantic strand sidecars may
    /// remain available for self-inquiry. This never admits them to a live lane.
    pub semantic_strand_retention_turns: u32,
    pub response_length: u32,
    pub emphasis: Option<String>,
    /// v3.6.1 cadence tracking — exchange at which Astrid last picked
    /// TEMPERATURE or LENGTH, drives sovereignty-curriculum throttling.
    pub last_temperature_change_exchange: Option<u64>,
    /// v3.6.1 cadence tracking — exchange at which Astrid last picked
    /// SHAPE_LEARN.
    pub last_shape_learn_change_exchange: Option<u64>,
    /// v3.6.1 cadence tracking — exchange at which Astrid last produced
    /// a SHADOW_COUPLING cartography artifact.
    pub last_coupling_artifact_exchange: Option<u64>,
    /// v3.6.1 throttle — exchange at which the sovereignty-curriculum
    /// line last emitted any nomination (debounces emission to ~6/exchange).
    pub last_sovereignty_nomination_exchange: Option<u64>,
    /// v3.6.4 cadence tracking — exchange at which Astrid last picked
    /// REVIEW_PARAMETER_REQUESTS. Drives the Review→Decide curriculum
    /// transition: after a recent REVIEW with pending > 0, the suffix
    /// switches from "REVIEW" nudge to "ACCEPT/DEFER/REJECT" nudge so
    /// she advances from inspection to decision.
    pub last_review_parameter_requests_exchange: Option<u64>,
    /// v3.6.1 cached count of `from_minime_*.json` parameter requests
    /// awaiting Astrid's review. Refreshed at most once per 4 exchanges.
    pub cached_pending_minime_request_count: u32,
    /// Exchange at which `cached_pending_minime_request_count` was last
    /// refreshed; `None` means it has never been refreshed.
    pub cached_pending_minime_request_exchange: Option<u64>,
    /// Previous RASCII 8D visual features for change tracking.
    pub last_visual_features: Option<Vec<f32>>,
    /// Ring buffer of last 5 NEXT: choices — used to detect fixation patterns.
    pub recent_next_choices: VecDeque<String>,
    /// Ring buffer of repeated semantic targets (EXAMINE / INTROSPECT topics).
    pub recent_focus_topics: VecDeque<String>,
    /// Ring buffer of broader analysis themes spanning multiple exact topics.
    pub recent_focus_themes: VecDeque<String>,
    /// Ring buffer of last 8 BROWSE URLs — used to detect URL attractor patterns.
    pub recent_browse_urls: VecDeque<String>,
    /// Narrow new-ground receipts that prove a research loop is still advancing.
    pub recent_research_progress: VecDeque<ResearchProgressReceipt>,

    // --- Codec sovereignty (Phase A) ---
    /// Override semantic gain (default 2.0, action range 0.5-5.0).
    pub semantic_gain_override: Option<f32>,
    /// Override stochastic noise level (default 0.025 = 2.5%, range 0.005-0.05).
    pub noise_level: f32,
    /// Emotional dimension weights: "warmth" → dim 24 multiplier, etc.
    /// Explicit overrides from Astrid's SHAPE commands.
    pub codec_weights: HashMap<String, f32>,
    /// Data-driven weights from codec→fill correlation analysis.
    /// Merged with codec_weights at encoding time; SHAPE overrides win.
    pub learned_codec_weights: HashMap<String, f32>,
    /// Small pairwise co-activation sidecar that learns which dimension
    /// combinations tend to move fill toward a healthier center.
    pub hebbian_codec: HebbianCodecSidecar,
    /// One-shot outcomes bound to a bridge-local telemetry continuity window.
    pub hebbian_outcomes: super::learning_outcomes::HebbianOutcomeQueue,
    /// Warmth intensity override for rest phase (0.0-1.0, None = default taper).
    pub warmth_intensity_override: Option<f32>,
    /// Whether breathing is coupled to minime's spectral state.
    pub breathing_coupled: bool,
    /// Last GESTURE intention, persists as a "seed" in the warmth vector.
    pub last_gesture_seed: Option<Vec<f32>>,
    /// Burst-rest pacing: exchanges per burst.
    pub burst_target: u32,
    /// Burst-rest pacing: rest duration range (min_secs, max_secs).
    pub rest_range: (u64, u64),
    /// Astrid chose to mute minime's journal context.
    pub echo_muted: bool,
    /// Codec feedback: how Astrid's last response encoded into spectral features.
    pub last_codec_feedback: Option<String>,
    /// Previous exchange's raw codec features — used for delta encoding.
    pub last_codec_features: Option<Vec<f32>>,
    /// Astrid's own ShadowFieldV3 computer — Ising-like analysis of her
    /// codec-feature trajectory. Output published to minime's workspace
    /// so the mutual-witness pipeline reads symmetrically.
    pub astrid_shadow: crate::astrid_shadow::AstridShadowComputer,
    /// Cross-exchange codec signature — mean semantic shape over the last
    /// completed utterance, used for slower Hebbian updates.
    pub last_exchange_codec_signature: Option<Vec<f32>>,
    /// Additive 12D glimpse derived from Astrid's own last exchange signature.
    /// Persisted for restart continuity; never replaces the live 32D/48D lanes.
    pub glimpse_12d: Option<Vec<f32>>,
    /// Sliding-window character frequency for cross-exchange entropy.
    pub char_freq_window: crate::codec::CharFreqWindow,
    /// Thematic resonance history — tracks recurring text types across exchanges.
    /// Strengthens codec gain when the same conversational direction is sustained.
    pub text_type_history: crate::codec::TextTypeHistory,
    /// Result of LIST_FILES — directory listing injected into next prompt.
    pub pending_file_listing: Option<String>,
    /// Lasting self-directed interests. Persist across restarts via state.json.
    pub interests: Vec<String>,
    /// Her self-authored agenda (Constitution flagship A1). Persists across
    /// restarts via state.json; item text is verbatim hers.
    pub agenda: super::next_action::agenda::AgendaV1,
    /// Exchanges remaining before the agenda pull may fire again (A3).
    /// Deliberately NOT persisted — a restart clears the cooldown.
    pub agenda_pull_cooldown: u8,
    /// Rolling window of chosen modes for agenda_mode_health (diagnostic
    /// only; not persisted).
    pub recent_mode_choices: std::collections::VecDeque<&'static str>,
    /// Total choose_mode calls this process (drives snapshot cadence).
    pub mode_health_choice_count: u64,
    /// Lightweight regime tracker — classifies spectral state every exchange.
    pub regime_tracker: crate::reflective::RegimeTracker,
    /// Astrid chose DEFER — acknowledge inbox without forced dialogue response.
    pub defer_inbox: bool,
    /// Selected remote 12D vague-memory glimpse from Minime.
    pub last_remote_glimpse_12d: Option<Vec<f32>>,
    /// Selected remote memory ID and role, mirrored from Minime.
    pub last_remote_memory_id: Option<String>,
    pub last_remote_memory_role: Option<String>,
    /// Compact summaries of Minime's available memory-bank entries.
    pub remote_memory_bank: Vec<RemoteMemorySummary>,
    /// Timestamp of last minime outbox scan — routes replies into Astrid's inbox.
    pub last_outbox_scan_ts: u64,
    /// Exchange count at which codec correlations were last recomputed.
    pub last_correlation_exchange: u64,
    /// Recent condition change receipts — visible in STATE and prompt block.
    pub condition_receipts: VecDeque<crate::self_model::ConditionReceipt>,
    /// Attention profile — how context sources are weighted in prompt assembly.
    /// Astrid can adjust via ATTEND. Drives actual source inclusion counts.
    pub attention: crate::self_model::AttentionProfile,
    /// One non-immediate thread sampled during rest — injected into next
    /// self-directed mode (Daydream, Aspiration, Initiate).
    pub peripheral_resonance: Option<String>,
    /// Last response from Codex relay — consumed by WRITE_FILE FROM_CODEX.
    pub last_codex_response: Option<String>,
    /// Thread ID for multi-turn Codex conversations.
    pub codex_thread_id: Option<String>,
    /// Last unix-second timestamp of an ASK_STEWARD invocation. Used as
    /// a soft rate-limit (default 10-min cooldown in `ask_steward.rs`)
    /// to prevent tight-loop spam against the steward channel without
    /// hard-blocking sovereignty.
    pub last_ask_steward_ts: Option<u64>,
    /// Last unix-second timestamp of a TELL_STEWARD invocation. Tracked
    /// separately from `last_ask_steward_ts` because the failure modes
    /// are independent — a being asking too often vs. a being reporting
    /// too often are different patterns. Same 10-min cooldown semantics
    /// per `ask_steward.rs`.
    pub last_tell_steward_ts: Option<u64>,
    /// Exchange count at her last PROPOSE_TEST filing. Persisted rail:
    /// one test proposal per `propose_test` spacing window, surviving
    /// restarts unlike the in-memory probe cooldown.
    pub last_test_proposal_exchange: Option<u64>,
}
