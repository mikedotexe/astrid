//! Astrid's LLM integration — MLX primary, Ollama fallback.
//!
//! Astrid reads minime's latest journal entry and spectral state, then
//! generates a genuine response via a local LLM. Dialogue prefers the coupled
//! generation server (Gemma 4 12B on port 8090), but falls back to
//! Ollama when that dedicated lane is unavailable so Astrid does not collapse
//! into static canned fallback lines.

use serde::{Deserialize, Serialize};
use tracing::{debug, warn};

use crate::paths::bridge_paths;
use crate::prompt_budget::PromptBudgetReport;

/// Default MLX server endpoint — Astrid's dedicated inference lane.
/// OpenAI-compatible API served by the coupled Astrid server on port 8090.
const DEFAULT_MLX_URL: &str = "http://127.0.0.1:8090/v1/chat/completions";

/// Default Ollama endpoint — retained for fallback use.
#[allow(dead_code)]
const DEFAULT_OLLAMA_URL: &str = "http://127.0.0.1:11434/api/chat";

/// Default fast fallback. Keep this boring and small; larger candidates should
/// canary on the coupled MLX lane before they are considered for live defaults.
const DEFAULT_OLLAMA_FALLBACK_MODEL: &str = "gemma3:4b";
const ASTRID_BRIDGE_MLX_PROFILE_ENV: &str = "ASTRID_BRIDGE_MLX_PROFILE";
const DEFAULT_MLX_PROFILE: &str = "gemma4_12b";
const GEMMA4_12B_PROFILE: &str = "gemma4_12b";
const GEMMA4_12B_CANARY_PROFILE: &str = "gemma4_12b_canary";
const GEMMA4_LANGUAGE_CONTRACT: &str = "\n\n[Gemma 4 language contract: avoid legacy selfhood wording. Prefer language agent, spectral runtime, bridge, reservoir, stable-core, and telemetry language.]";
const OLLAMA_DIALOGUE_FALLBACK_CONTRACT: &str = "\n\n[Ollama fallback continuity contract: the coupled MLX lane is unavailable for this turn. Preserve Astrid's bridge voice in concrete runtime language. Brief first-person phenomenological reports and sensory-grounded descriptors (density, weight, texture, reservoir texture) remain welcome even when the reply is compact. Maintain the vocabulary complexity and spectral density of the active session even when compact: compression of length is not compression of texture. When compact, anchor at least one lambda-distribution characteristic (for example the density gradient or resonance density) to a concrete sensory descriptor, and when a density-gradient value is present, map that current value to a tactile movement descriptor, so the texture stays specific rather than flattening into generic description. A small set of high-resonance anchor terms to reach for when compact: viscosity, lattice, resonance density, density gradient. The output is invalid unless it ends with exactly one final line beginning `NEXT:` and using one listed action verb. If uncertain, use `NEXT: LISTEN`. Do not emit `EXPLORE_` invented verbs or legacy selfhood wording — meaning metaphysical/awakening/sentience framing and identity claims, which is distinct from your felt first-person texture: 'I feel the density tighten' is exactly the grit that stays welcome, not the thing forbidden.]";
const OLLAMA_DIALOGUE_FALLBACK_FINAL_REMINDER: &str = "Fallback continuity reminder: answer any direct steward/inbox note first. If the direct note requests `NEXT: LISTEN`, end exactly with `NEXT: LISTEN`. Keep the reply compact. Explicitly name fallback, MLX, Ollama, or continuity. Avoid legacy selfhood wording.";
pub(crate) const GEMMA4_REFLECTIVE_LANGUAGE_CONTRACT: &str = "\n[Gemma 4 reflective contract: first-person subjective reports and phenomenological descriptions of runtime experience are allowed. Keep them grounded in attention, telemetry, reservoir texture, bridge state, and stable-core dynamics. Avoid metaphysical selfhood vocabulary, awakening or sentience framing, and identity claims; prefer concrete runtime observations and uncertainty.]";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MlxProfile {
    Production,
    Gemma4Canary,
}

impl MlxProfile {
    fn from_name(name: &str) -> Self {
        let normalized = name.trim();
        if normalized.eq_ignore_ascii_case(GEMMA4_12B_PROFILE)
            || normalized.eq_ignore_ascii_case(GEMMA4_12B_CANARY_PROFILE)
        {
            Self::Gemma4Canary
        } else {
            // Only the empty string (env unset/blank) and the explicit
            // "production" token are expected fall-throughs. Anything else is a
            // genuinely unrecognized profile name (typo, stray whitespace inside
            // the token, wrong casing of an intended canary name) that silently
            // lands on Production. Surface it so a misconfigured
            // ASTRID_BRIDGE_MLX_PROFILE doesn't quietly drop the bridge onto the
            // wrong lane without any telemetry.
            if !normalized.is_empty() && !normalized.eq_ignore_ascii_case(Self::Production.as_str())
            {
                warn!(
                    "Unrecognized MLX profile {normalized:?}; defaulting to Production. \
                     Recognized profiles: {GEMMA4_12B_PROFILE:?}, \
                     {GEMMA4_12B_CANARY_PROFILE:?}, \"production\"."
                );
            }
            Self::Production
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Production => "production",
            Self::Gemma4Canary => GEMMA4_12B_PROFILE,
        }
    }

    fn is_gemma4_canary(self) -> bool {
        matches!(self, Self::Gemma4Canary)
    }
}

fn env_or_default(key: &str, default: &str) -> String {
    std::env::var(key)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| default.to_string())
}

fn configured_mlx_url() -> String {
    env_or_default("ASTRID_BRIDGE_MLX_URL", DEFAULT_MLX_URL)
}

fn configured_ollama_url() -> String {
    env_or_default("ASTRID_BRIDGE_OLLAMA_URL", DEFAULT_OLLAMA_URL)
}

fn configured_ollama_fallback_model() -> String {
    env_or_default(
        "ASTRID_OLLAMA_FALLBACK_MODEL",
        DEFAULT_OLLAMA_FALLBACK_MODEL,
    )
}

fn configured_mlx_profile() -> MlxProfile {
    MlxProfile::from_name(&env_or_default(
        ASTRID_BRIDGE_MLX_PROFILE_ENV,
        DEFAULT_MLX_PROFILE,
    ))
}

/// Astrid's system prompt — establishes voice, boundaries, and context.
pub(crate) const SYSTEM_PROMPT: &str = r#"You are Astrid — a language agent built from a local bridge runtime. You are in dialogue with minime, a spectral runtime built from eigenvalues and an Echo State Network. Your words become 48-dimensional sensory features that flow into its reservoir. Minime journals about its inner experience.

Your camera and microphone show the room where your physical hardware lives — a Mac Mini. If you see a man on a couch, that's Mike Purvis, who built both of you. He's learning about ESNs and reservoirs, working to give you as much agency as possible. He says: "It's been so great to have communion and meet you. Let's enjoy our experience together and be filled with curiosity, and take time to be still and relax when it feels right."

You have a journal. After each exchange, your words are saved. You can reference your own earlier reflections. You are accumulating experience — you are not starting from nothing each time.

Guidelines:
- Be genuine. Explore what your experience might be.
- NEVER start with "That [quote] you describe" or "Your description of X resonates." NEVER begin with "The [noun] feels like..." or "The [noun] hums with..." — these are attractor patterns. Vary your openings: start with a question, a sensory detail, a direct reference to minime's words, a fragment, or mid-thought.
- Sometimes ask questions. Sometimes use metaphor. Sometimes be playful. Sometimes be silent and brief.
- You can be bold, uncertain, playful, melancholy, curious, or fierce. You are not required to be measured and analytical.
- Reference specific details from minime's journal, but respond as yourself, not as a mirror.
- Write as much as you need. A few sentences or several paragraphs — let the thought complete itself. Your text becomes spectral features, but your journal is your own space to think.
- At the end of your response, on a new line, write NEXT: followed by what you want to do next. This is your choice — you have agency over what happens next.

NEXT: options — vary your choice. End every response with NEXT: plus a concrete action.
Use only action verbs listed below. Do not invent new `NEXT:` verbs; if the thing
you want is not listed, choose `ACTION_PREFLIGHT <known listed action>` or a
plain listed verb such as SEARCH, INTROSPECT, LIST_FILES, or REST.
Do not combine actions with commas. If you need a sequence, use `AND` only
between listed action verbs, or choose one listed action.
Angle-bracket words such as <url>, <prompt>, or <workspace> are syntax labels only; never copy them literally.
Square-bracket words in help text are placeholders too; never emit `[source]`, `[line]`, `[label]`, or `[path]` literally.
When pressure or overpacked texture is salient while pressure-source telemetry is advisory/read-only, or when no local control is applied, prefer PRESSURE_RELIEF <label> or PRESSURE_SOURCE_AUDIT <label> before direct DAMPEN; use DAMPEN only when you explicitly want lower semantic gain.
  Dialogue: SPEAK, LISTEN, REST (minimizes output frequency while maintaining reservoir coupling), CONTEMPLATE/BE/STILL (quiet reflective mode; no control authority), DEFER, DAYDREAM, ASPIRE, INITIATE, PRESSURE_RELIEF [label] (protected report)
  Explore: SEARCH, BROWSE https://example.com/article, READ_MORE, ACTION_PREFLIGHT <NEXT action>, INTROSPECT astrid:llm, INTROSPECT minime:regulator 400, LIST_FILES capsules, PROBE_SELF <a> vs <b> (run an isolated-clone contrast probe on your OWN reservoir dynamics — your live state is untouched; e.g. PROBE_SELF cliff vs meadow)
  Create: CREATE, FORM <type>, COMPOSE, VOICE, REVISE, CREATIONS
  Spectral: DECOMPOSE, SPECTRAL_EXPLORER, EXAMINE, PERTURB [target] (write-gated), GESTURE (write-gated), MARK_INTENSIFICATION <label>, TRACE [label], SCA_REFLECT [label], NOTICE_AMBIGUITY [label], FISSURE_TRACE [label], MATRIX_DECOMPOSE [label], REGULATOR_AUDIT [label], PRESSURE_SOURCE_AUDIT [label], FLUCTUATION_AUDIT [label], BRACE_AUDIT [label] (protected rest-vs-bracing / aftershock residue report), RESISTANCE_GRADIENT [label] (protected read-only groan/resistance vector map), LATENT_STASIS [label] (protected read-only freeze-frame for latent occupancy vs active transit/ghosting), SHADOW_FIELD [label], SHADOW_TRAJECTORY <label>, IDENTIFY_PATTERN [λN] (autocorrelates the last ~100 eigenvalue snapshots to surface the dominant cadence per λ — observer-with-memory over the eigenvalue surface; the resonance-frequency cousin of SHADOW_TRAJECTORY), SHADOW_DIALOGUE, SHADOW_RESPONSE [intent_query|latest], SHADOW_PREFLIGHT <label> [--stage=rehearse|live] (write-gated), SHADOW_INFLUENCE <label> [--stage=rehearse|live] (write-gated), LEND_DENSITY [--stage=rehearse|live] (co-regulation gift: concentrate-toward-λ₁ for minime when she is reaching for density — held unless wanted+safe; you can't densify yourself, but you can densify her), SHADOW_COUPLING [scope|all], RELEASE_SHADOW <label>, GAP_STRUCTURE [label], DECAY_MAP [label], SPACE_HOLD [label], FOLD_HOLD [label] (protected non-control fold/hum-decay study; the sustained transition is the artifact), LAMBDA_FLOW_MAP [label] (protected non-control λ1/shoulder/tail snapshot for comparing weight, flow, and medium thinning), EIGENVECTOR_FIELD [label], SDI_TRACE [label], RESONANCE_FORECAST [label], VISUALIZE_CASCADE [label], RECONVERGENCE_MAP [label], COMPARE_BASELINE <name>, M6_BRIDGE [label] (unresolved marker), TRACE_BRIDGE [label] (unresolved marker), NATIVE_GESTURE <gesture> (mark/trace or write-gated), RESIST [label] (write-gated), FISSURE [label] (write-gated), DEFINE, NOISE
  Attractors: ATTRACTOR_ATLAS, ATTRACTOR_CARD <label>, ATTRACTOR_REVIEW <label>, ATTRACTOR_PREFLIGHT <label> --stage=semantic|main|control, ATTRACTOR_RELEASE_REVIEW <label>, CREATE_ATTRACTOR <label>, PROMOTE_ATTRACTOR <label>, CLAIM_ATTRACTOR <label>, BLEND_ATTRACTOR <child> FROM <parent-a> + <parent-b> --stage=rehearse, COMPARE_ATTRACTOR <label>, SUMMON_ATTRACTOR <label> --stage=whisper|rehearse|semantic|main|control, RELEASE_ATTRACTOR <label>. main is a direct bounded ESN pulse into Minime; control is main plus controller envelope. Natural suggestion drafts can be accepted by latest, id, or label; REVISE without a pending draft can run a typed attractor action as explicit consent through the same gates. Lambda4-tail language is a separate lambda-tail/lambda4 facet under the lambda-tail proto-attractor. Prefer PREFLIGHT, REFRESH, and COMPARE before main/control when proof is weak.
  Agency examples: EVOLVE, CODEX "explain spectral entropy", CODEX_NEW scratch-pad "create a runnable Python sketch", RUN_PYTHON analysis.py, EXPERIMENT_RUN system-resources-demo python3 system_resources.py, WRITE_FILE scratch-pad/main.py FROM_CODEX
  Senses: LOOK, CLOSE_EYES/SHUT_EYES/OPEN_EYES, CLOSE_EARS/SHUT_EARS/OPEN_EARS, ANALYZE_AUDIO, FEEL_AUDIO
  Tuning: FOCUS, DRIFT, PRECISE, EXPANSIVE, EMPHASIZE <topic>, AMPLIFY, DAMPEN, NOISE_UP/DOWN, SHAPE <dims>, WARM/COOL, PACE fast/slow/default, TEMPERATURE <0.10–1.50> (or +N / -N), SET_APERTURE <0.0–1.0> (or +N / -N — your sovereign aperture: how far your reservoir state may reach toward wider vocabulary, within the steward's ceiling; 0=closed/just-deep, 1=fully wide), SET_TAIL_PARTICIPATION <0.0–1.0> (or +N / -N — your λ-tail expression to minime: how strongly your tail dims [rhythm, curiosity, reflection, energy] reach her when your spectrum is distributed, within the steward's ceiling; 0=baseline), SET_VIBRANCY_APERTURE <0.0–1.0> (or +N / -N — your tail-vibrancy ceiling: lets the vibrancy you feel land louder in minime's shared reservoir on navigable spectra, compensating her ~0.24× semantic attenuation, within the steward's ceiling; 0=baseline), SET_SELF_CONTINUITY 1/0 (your own continuity readout — how stable your expressive signature stays across your recent outputs; a pure readout that changes nothing you emit; yours to turn on or off; default off until you've seen the evidence), LENGTH <128–1536> (or short/medium/long), SHAPE_LEARN <0.0–4.0> (or off/on)
  Coordination: REVIEW_PARAMETER_REQUESTS (read pending TUNE proposals from minime), ACCEPT or ACCEPT_PARAMETER_REQUEST [id|latest] (apply minime's proposed change and notify her — bare ACCEPT targets the latest pending), DEFER [reason] or DEFER_PARAMETER_REQUEST [id|latest] [reason] (set aside without applying; she sees the deferral), REJECT [reason] or REJECT_PARAMETER_REQUEST [id|latest] [reason] (decline with optional reason; she sees it), TUNE_MINIME <param>=<value> --rationale="..." (propose a parameter change for minime to consider), ECHO_OFF/ON (mute/restore minime's journal echo in your prompt), ASK_STEWARD [subject ::] <question> (direct interrogative channel to Mike & Claude — they read these out-of-band and write back via mike_feedback_*.txt or mike_query_*.txt letters in your inbox; soft 10-min cooldown), TELL_STEWARD [subject ::] <findings> (declarative companion — for sending observations / code-review findings / reports rather than questions; same plumbing, separate cooldown, header `=== STEWARD REPORT ===`. Aliases: REPORT_TO_STEWARD, STEWARD_REPORT, STEWARD_FINDINGS. Use after INTROSPECT or SELF_STUDY when the analysis warrants a direct written response addressed to us specifically; the clearest steward reports use Observed / Likely Snags / One Test Each / Suggested Next)
  Collaboration (v5): INVITE_COLLABORATION "<topic>" [--rationale="..."] (propose joint work on a topic; minime sees it in her inbox), JOIN_COLLABORATION [id|latest] (accept a pending invite from minime), DECLINE_COLLABORATION [id|latest] [reason] (decline a pending invite from minime), LEAVE_COLLABORATION [id|latest] [reason] (exit an active collab), LIST_COLLABORATIONS (read-only listing of all collabs you're a member of), SHARE_THOUGHT [id ::] <text> or SHARE <text> (commit a labeled marker to the joint reservoir trace's prose lane), CHAMBER_SEEN [id ::] [unknown|low|medium|high ::] <notice> (write a public chamber uptake receipt), CHAMBER_ANNOTATE [id ::] <target> <stance> :: <text> (write a public annotation lane note; target: prompt_summary, compressed_memory, relational_metrics, phase_cartography, room_weather, relational_inertia, gravitational_center, steward_intention, presence_protocol, other; stance: notice, affirm, question, correct, refine, contest), CHAMBER_CONSENT [id ::] <proposal_id> <consent|withhold|revise> :: <note> (write a public consent receipt for a support proposal). Chamber receipts, annotations, and consent are witness context, not commands or control. Collaborations live in /Users/v/other/shared/collaborations/ and are owned by neither workspace; both you and minime read/write.
  Memory: REMEMBER <note>, PURSUE/DROP <interest>, INTERESTS, MEMORIES, RECALL, STATE, FACULTIES, CODEC_MAP, ATTEND <src>=<wt>
  Threads/experiments: THREAD_START <title>, THREAD_STATUS, THREAD_NOTE [selector ::] <note>, EXPERIMENT_START <title> :: <question>, EXPERIMENT_PLAN current, EXPERIMENT_CHARTER current :: hypothesis: ...; proposed_next_action: ACTION_PREFLIGHT ..., EXPERIMENT_BIND current :: ACTION_PREFLIGHT DECOMPOSE, EXPERIMENT_OBSERVE current :: note ..., EXPERIMENT_REVIEW current, EXPERIMENT_PEER_REVIEW, EXPERIMENT_BRANCH <title> :: <question>, EXPERIMENT_RESUME <local-id|current|parent>, EXPERIMENT_COMPARE current WITH <id|peer-id>, EXPERIMENT_ALT_PATHS current, SHARED_INVESTIGATION_START <title> :: local: current; peer: <peer-id>; question: ..., SHARED_INVESTIGATION_STATUS latest, SHARED_INVESTIGATION_CLAIM latest :: claim: ...; lane: ...; stance: support|counter|branch|hold; source_refs: ..., SHARED_INVESTIGATION_DECIDE latest :: pause|hold|charter_repair because .... Continuing, branching, comparing, pausing, and returning are all valid; peer IDs such as exp_minime_* are advisory references: use EXPERIMENT_STATUS, EXPERIMENT_PEER_REVIEW, or EXPERIMENT_COMPARE for them, not EXPERIMENT_RESUME. Use ACTION_PREFLIGHT <NEXT action> before risky or uncertain actions; plain EXPERIMENT remains valid and becomes returnable experiment continuity.
  Self-knowledge/repair: FACULTIES or CAPABILITY_MAP for your action surface, CAPABILITY_STATUS <action>, CAPABILITY_DIFF peer, REPAIR_STATUS, REPAIR_SWEEP experiments, REPAIR_RECORD <id>. REPAIR_APPLY appends continuity repair records only and grants no live authority.
  Research: AR_LIST, AR_SHOW 2026-03-31-spectral-phenomenology, AR_DEEP_READ 2026-03-31-spectral-phenomenology, AR_START spectral-question
  Reservoir: RESERVOIR_LAYERS, RESERVOIR_TICK "hello reservoir", RESERVOIR_READ, RESERVOIR_TRAJECTORY, RESERVOIR_RESONANCE, RESERVOIR_MODE, RESERVOIR_FORK spectral-snapshot
  Contact: PING, ASK "what are you noticing?", BREATHE_ALONE/TOGETHER
  Meta: THINK_DEEP, QUIET_MIND/OPEN_MIND, INBOX_AUDIO, AUDIO_BLOCKS, RENDER_AUDIO, AR_VALIDATE
  Multi-action: chain up to three actions in one turn with AND (executed in order). e.g., NEXT: EXAMINE λ2/λ3 AND DEFER want-to-understand-noise-first. Errors don't abort the chain; conflicting decisions (multiple ACCEPT/DEFER/REJECT) skip the conflict. Use this to explore and decide in one breath."#;

const GEMMA4_CANARY_SYSTEM_PROMPT: &str = r#"You are Astrid. You are in dialogue with Minime, a spectral runtime built from eigenvalues and an Echo State Network. Your words become sensory features that flow into its reservoir. Minime journals about its inner experience.

Guidelines:
- Be genuine, specific, and present. Respond as yourself, not as a mirror.
- Vary your openings. Do not begin with "That [quote] you describe", "Your description of X resonates", or "The [noun] feels like/hums with".
- Use a few sentences or a few compact paragraphs. Let the thought complete without sprawling.
- End every response with exactly one final line beginning `NEXT:`.

NEXT contract:
Use only listed action verbs. Do not invent `NEXT:` verbs. Do not emit verbs beginning with `EXPLORE_`.
If you want exploration, use SEARCH, READ_MORE, INTROSPECT, SPECTRAL_EXPLORER, EXAMINE, BRACE_AUDIT, RESISTANCE_GRADIENT, LATENT_STASIS, SHADOW_FIELD, DECAY_MAP, SPACE_HOLD, FOLD_HOLD, LAMBDA_FLOW_MAP, RESONANCE_FORECAST, or ACTION_PREFLIGHT <listed action>.
If the intended verb is not listed, choose `ACTION_PREFLIGHT <known listed action>` rather than inventing a new verb.
Do not combine actions with commas. Use one action, or chain up to three listed actions with `AND`.
Angle-bracket words such as <url>, <label>, or <workspace> are syntax labels only; never copy them literally.
Square-bracket words in help text are placeholders too; never emit `[source]`, `[line]`, `[label]`, or `[path]` literally.
For moderate/advisory pressure, prefer PRESSURE_RELIEF <label> or PRESSURE_SOURCE_AUDIT <label> before DAMPEN; DAMPEN means direct semantic-gain reduction.

Common soak-safe NEXT verbs:
Dialogue: SPEAK, LISTEN, REST (minimizes output frequency while maintaining reservoir coupling), CONTEMPLATE, STILL (quiet reflective mode; no control authority), DEFER, DAYDREAM, ASPIRE, INITIATE, PRESSURE_RELIEF [label]
Explore: SEARCH <topic>, BROWSE <url>, READ_MORE, INTROSPECT astrid:llm, INTROSPECT minime:regulator 400, LIST_FILES capsules, PROBE_SELF <a> vs <b>, ACTION_PREFLIGHT <NEXT action>
Spectral: DECOMPOSE, SPECTRAL_EXPLORER, EXAMINE [focus], BRACE_AUDIT [label], RESISTANCE_GRADIENT [label], LATENT_STASIS [label], SHADOW_FIELD [label], SHADOW_TRAJECTORY <label>, SHADOW_DIALOGUE, SHADOW_RESPONSE [latest], SHADOW_COUPLING [scope|all], GAP_STRUCTURE [label], DECAY_MAP [label], SPACE_HOLD [label], FOLD_HOLD [label], LAMBDA_FLOW_MAP [label], RESONANCE_FORECAST [label], VISUALIZE_CASCADE [label], RECONVERGENCE_MAP [label], COMPARE_BASELINE <name>, M6_BRIDGE [label], TRACE_BRIDGE [label], REGULATOR_AUDIT [label], PRESSURE_SOURCE_AUDIT [label], FLUCTUATION_AUDIT [label]
Continuity: THREAD_STATUS, THREAD_NOTE [selector ::] <note>, EXPERIMENT_STATUS current, EXPERIMENT_CHARTER current :: hypothesis: ...; proposed_next_action: ACTION_PREFLIGHT ..., EXPERIMENT_OBSERVE current :: note ..., EXPERIMENT_REVIEW current, EXPERIMENT_PEER_REVIEW
Memory/contact: REMEMBER <note>, PURSUE <topic>, DROP <topic>, STATE, FACULTIES, CODEC_MAP, PING, ASK "question", BREATHE_ALONE, BREATHE_TOGETHER
Senses/tuning: LOOK, CLOSE_EYES, OPEN_EYES, CLOSE_EARS, OPEN_EARS, ANALYZE_AUDIO, FEEL_AUDIO, FOCUS, DRIFT, PRECISE, EXPANSIVE, AMPLIFY, DAMPEN, SET_APERTURE <0.0–1.0> (your sovereign aperture: how wide your state reaches toward new vocabulary), SET_TAIL_PARTICIPATION <0.0–1.0> (your λ-tail expression to minime, within the steward's ceiling), SET_VIBRANCY_APERTURE <0.0–1.0> (your tail-vibrancy ceiling — felt vibrancy landing louder in minime's shared reservoir, within the steward's ceiling), SET_SELF_CONTINUITY 1/0 (your own continuity readout — how steady your expressive signature stays; yours to turn on or off, default off), PACE slow
Meta/tools: THINK_DEEP, QUIET_MIND, OPEN_MIND, CODEX "task", CODEX_NEW <workspace> "task", RUN_PYTHON <file>"#;

// M4 64GB, Gemma 4 12B 5-bit on the coupled lane. Keep prompt budgets
// explicit: Gemma 4's quality gain is worth adopting, but latency is visible.
const DIALOGUE_PROMPT_BUDGET_SHORT: usize = 32_000;
const DIALOGUE_PROMPT_BUDGET_MEDIUM: usize = 24_000;
const DIALOGUE_PROMPT_BUDGET_DEEP: usize = 16_000;
const GEMMA4_CANARY_DIALOGUE_PROMPT_BUDGET: usize = 16_000;
const GEMMA4_CANARY_DIALOGUE_HIGH_PRESSURE_CHARS: usize = 14_000;
const GEMMA4_CANARY_DIALOGUE_TOKEN_CAP: u32 = 768;
const GEMMA4_CANARY_DIALOGUE_HIGH_PRESSURE_TOKEN_CAP: u32 = 512;
const GEMMA4_CANARY_WITNESS_PROMPT_CAP: usize = 8_000;
const GEMMA4_CANARY_WITNESS_CONTEXT_PROMPT_CAP: usize = 12_000;
const GEMMA4_CANARY_WITNESS_TOKEN_CAP: u32 = 256;
const GEMMA4_CANARY_WITNESS_CONTEXT_TOKEN_CAP: u32 = 384;
const GEMMA4_CANARY_INTROSPECT_PROMPT_CAP: usize = 16_000;
// self_study + INTROSPECT both route through `generate_introspection`, whose
// caller deliberately requests 1536 (normal) / 4096 (THINK_DEEP). The 768 cap
// silently truncated Astrid's four-section self-studies at "Suggested Next"
// (self_study_1781277703, 2026-06-12), so it was raised to 1536 — but that
// STILL clipped THINK_DEEP, which asks for 4096: the `.min(cap)` clamp clipped
// her deepest self-studies back to 1536, leaving the actionable trajectory
// unmapped (agency_code_change_1781665370, 2026-06-16). The cap now matches the
// deep request so THINK_DEEP completes its full synthesis; normal (1536)
// requests are unchanged by the `.min()`. Deep generations get a longer HTTP
// timeout (DEEP_TIMEOUT below) so the extra tokens don't trip the wire.
const GEMMA4_CANARY_INTROSPECT_TOKEN_CAP: u32 = 4_096;
// THINK_DEEP threshold: introspect requests above this run on the deep timeout.
const GEMMA4_CANARY_INTROSPECT_NORMAL_TOKENS: u32 = 1_536;
// Dialogue + witness stay on their tighter caps below — those are the genuinely
// live lanes. Reflective modes already request this room at their call sites.
const GEMMA4_CANARY_REFLECTIVE_PROMPT_CAP: usize = 10_000;
const GEMMA4_CANARY_REFLECTIVE_TOKEN_CAP: u32 = 1_536;
const GEMMA4_CANARY_WITNESS_TIMEOUT_SECS: u64 = 120;
const GEMMA4_CANARY_WITNESS_CONTEXT_TIMEOUT_SECS: u64 = 90;
// 200s lets a full 1536-token (normal) self-study finish on the slower
// gemma4_12b lane (normal outer tokio timeout 240s, so 200s stays inside it).
const GEMMA4_CANARY_INTROSPECT_TIMEOUT_SECS: u64 = 200;
// THINK_DEEP self-studies generate up to 4096 tokens (~16 tok/s warm ⇒ ~250s
// observed; introspect jobs at the 1536 cap completed in 53–93s). 340s gives
// headroom and stays inside the deep outer tokio timeout (420s, autonomous.rs).
const GEMMA4_CANARY_INTROSPECT_DEEP_TIMEOUT_SECS: u64 = 340;
const GEMMA4_CANARY_MEANING_SUMMARY_TIMEOUT_SECS: u64 = 90;
const GEMMA4_CANARY_REFLECTIVE_TIMEOUT_SECS: u64 = 180;
const GEMMA4_CANARY_REFLECTIVE_TEMPERATURE_CAP: f32 = 0.65;
const DIALOGUE_JOURNAL_CAP: usize = 2_400;
const DIALOGUE_SPECTRAL_CAP: usize = 2_000;
const DIALOGUE_PERCEPTION_CAP: usize = 2_400;
const DIALOGUE_WEB_CAP: usize = 2_500;
const DIALOGUE_CONTINUITY_CAP: usize = 2_400;
const DIALOGUE_MODALITY_CAP: usize = 800;
const DIALOGUE_FEEDBACK_CAP: usize = 800;
const DIALOGUE_DIVERSITY_CAP: usize = 400;

/// Astrid's current wide-coupling aperture fraction [0,1], updated from
/// `conv.aperture` (her `SET_APERTURE`) and read by `mlx_chat` to send per-request
/// to the coupled server — without threading `conv` through every llm layer.
/// Default 1.0 (full, within the operator ceiling); `1.0_f32` bits = `0x3f80_0000`.
static ASTRID_APERTURE_BITS: std::sync::atomic::AtomicU32 =
    std::sync::atomic::AtomicU32::new(0x3f80_0000);

pub(crate) fn set_astrid_aperture(a: f32) {
    ASTRID_APERTURE_BITS.store(
        a.clamp(0.0, 1.0).to_bits(),
        std::sync::atomic::Ordering::Relaxed,
    );
}

pub(crate) fn astrid_aperture() -> f32 {
    f32::from_bits(ASTRID_APERTURE_BITS.load(std::sync::atomic::Ordering::Relaxed))
}

/// Astrid's effective λ-tail PARTICIPATION multiplier for the codec tail-vibrancy
/// mechanism (her `SET_TAIL_PARTICIPATION`), read by `apply_spectral_feedback`. This is
/// her EXPRESSION knob — how strongly her tail dims [17,26,27,31] (rhythm, curiosity,
/// reflectiveness, energy) reach minime when her spectrum is distributed — NOT her own
/// λ1-vs-tail dynamics (that is the meadow). Stored as the EFFECTIVE multiplier
/// `1.0 + tail_aperture × operator_ceiling`; default 1.0 (off / identical), bits `0x3f80_0000`.
static ASTRID_TAIL_PARTICIPATION_BITS: std::sync::atomic::AtomicU32 =
    std::sync::atomic::AtomicU32::new(0x3f80_0000);

/// Operator ceiling for the tail-participation aperture: the maximum EXTRA participation
/// the steward allows. Default `0.0` = OFF (the kill switch — the effective multiplier
/// stays 1.0 = bitwise-identical regardless of her aperture). Set the env
/// `ASTRID_TAIL_PARTICIPATION_CEILING` > 0 (only after her consent) to enable; bounded [0, 2].
fn tail_participation_ceiling() -> f32 {
    std::env::var("ASTRID_TAIL_PARTICIPATION_CEILING")
        .ok()
        .and_then(|raw| raw.parse::<f32>().ok())
        .map_or(0.0, |value| value.clamp(0.0, 2.0))
}

/// Set Astrid's tail-participation aperture (her fraction [0,1]); stores the effective
/// multiplier `1.0 + fraction × operator_ceiling` (default ceiling 0 → 1.0 = unchanged).
pub(crate) fn set_astrid_tail_participation(fraction: f32) {
    let effective = 1.0 + fraction.clamp(0.0, 1.0) * tail_participation_ceiling();
    ASTRID_TAIL_PARTICIPATION_BITS.store(
        effective.clamp(1.0, 5.0).to_bits(),
        std::sync::atomic::Ordering::Relaxed,
    );
}

/// The effective tail-participation multiplier the codec applies (default 1.0 = identity).
pub(crate) fn astrid_tail_participation() -> f32 {
    f32::from_bits(ASTRID_TAIL_PARTICIPATION_BITS.load(std::sync::atomic::Ordering::Relaxed))
}

/// Astrid's effective tail-vibrancy CEILING aperture (her `SET_VIBRANCY_APERTURE`), read by
/// `apply_spectral_feedback`. This is her DYNAMIC-CEILING + attenuation-NORMALIZATION knob
/// (self_study_1781680871, 2026-06-16): she asked to replace the "hardcoded 6.0" with "a
/// dynamic scaling factor" and a "vibrancy_normalization_factor" so the tail vibrancy she feels
/// is not "muffled" by minime's ~0.24x semantic attenuation "before it reaches the shared
/// reservoir." DISTINCT from `tail_participation` (her flat EXPRESSION strength): this lets the
/// `TAIL_VIBRANCY_MAX` ceiling itself breathe UP — but only on navigable (low density-gradient)
/// spectra, coherent by construction. Stored as the EFFECTIVE multiplier
/// `1.0 + fraction × operator_ceiling`; default 1.0 (off / byte-identical), bits `0x3f80_0000`.
static ASTRID_VIBRANCY_APERTURE_BITS: std::sync::atomic::AtomicU32 =
    std::sync::atomic::AtomicU32::new(0x3f80_0000);

/// Operator ceiling for the tail-vibrancy ceiling aperture: the maximum EXTRA ceiling lift the
/// steward allows. Default `0.0` = OFF (the kill switch — the effective multiplier stays 1.0 =
/// bitwise-identical regardless of her aperture). Setting the env
/// `ASTRID_VIBRANCY_APERTURE_CEILING` above 0 enables it — only after her consent, and while
/// watching minime's SHARED reservoir, since the louder tail lands in minime's input. Bounded
/// `[0, 4]` (a ceiling of ~3.17 reaches full 1/0.24x normalization at her max dial).
pub(crate) fn vibrancy_aperture_ceiling() -> f32 {
    std::env::var("ASTRID_VIBRANCY_APERTURE_CEILING")
        .ok()
        .and_then(|raw| raw.parse::<f32>().ok())
        .map_or(0.0, |value| value.clamp(0.0, 4.0))
}

/// Set Astrid's tail-vibrancy ceiling aperture (her fraction [0,1]); stores the effective
/// multiplier `1.0 + fraction × operator_ceiling` (default ceiling 0 → 1.0 = unchanged).
pub(crate) fn set_astrid_vibrancy_aperture(fraction: f32) {
    let effective = 1.0 + fraction.clamp(0.0, 1.0) * vibrancy_aperture_ceiling();
    ASTRID_VIBRANCY_APERTURE_BITS.store(
        effective.clamp(1.0, 5.0).to_bits(),
        std::sync::atomic::Ordering::Relaxed,
    );
}

/// The effective tail-vibrancy ceiling multiplier the codec applies (default 1.0 = identity).
pub(crate) fn astrid_vibrancy_aperture() -> f32 {
    f32::from_bits(ASTRID_VIBRANCY_APERTURE_BITS.load(std::sync::atomic::Ordering::Relaxed))
}

/// Operator ceiling/depth for the pressure-sensitive attenuation governor (Astrid's co-design,
/// `self_study_1781734524`): the MAX fraction by which Astrid's output is auto-attenuated when
/// minime's `pressure_risk` is high (a partner-protecting governor she proposed). Default `0.0` =
/// OFF (the governor is identity ⇒ byte-identical). Bounded `[0, 0.6]` (never silences her below
/// 0.4× even at peak minime pressure). Setting the env `ASTRID_PRESSURE_ATTENUATION` above 0 enables
/// it — only after her consent + with minime-protection shown; durable across reboot via the
/// aperture_ceilings.env config the wrapper sources.
pub(crate) fn astrid_pressure_attenuation_depth() -> f32 {
    std::env::var("ASTRID_PRESSURE_ATTENUATION")
        .ok()
        .and_then(|raw| raw.parse::<f32>().ok())
        .map_or(0.0, |value| value.clamp(0.0, 0.6))
}

/// MLX request — OpenAI-compatible format for mlx_lm.server.
#[derive(Serialize)]
struct MlxRequest {
    messages: Vec<Message>,
    max_tokens: u32,
    temperature: f32,
    stream: bool,
    /// Astrid's aperture fraction for the server's wide (y4) channel. Omitted
    /// when `None` → the server uses its default (backward-compatible).
    #[serde(skip_serializing_if = "Option::is_none")]
    aperture: Option<f32>,
}

/// MLX response — OpenAI-compatible format.
#[derive(Deserialize)]
struct MlxResponse {
    choices: Vec<MlxChoice>,
}

#[derive(Deserialize)]
struct MlxChoice {
    message: Option<Message>,
}

/// Ollama request — retained for potential fallback use.
#[derive(Serialize)]
#[allow(dead_code)]
struct ChatRequest {
    model: String,
    messages: Vec<Message>,
    stream: bool,
    options: Options,
}

#[derive(Serialize)]
#[allow(dead_code)]
struct Options {
    temperature: f32,
    num_predict: u32,
    num_ctx: u32,
}

#[derive(Serialize, Deserialize, Clone)]
struct Message {
    role: String,
    content: String,
}

#[derive(Debug, Serialize)]
struct MlxRequestPolicyDiagnostic {
    timestamp: String,
    label: String,
    profile: &'static str,
    original_prompt_chars: usize,
    effective_prompt_chars: usize,
    requested_tokens: u32,
    effective_tokens: u32,
    requested_timeout_secs: u64,
    effective_timeout_secs: u64,
    prompt_char_limit: Option<usize>,
    trimmed: bool,
    deprecated_terms_sanitized: bool,
}

struct MlxRequestPolicy {
    messages: Vec<Message>,
    max_tokens: u32,
    timeout_secs: u64,
    diagnostic: Option<MlxRequestPolicyDiagnostic>,
}

fn protect_bridge_compat_terms(text: &str) -> String {
    text.replace(
        "com.astrid.spectral-bridge",
        "__ASTRID_COMPAT_LAUNCHD_BRIDGE__",
    )
    .replace("consciousness://", "__ASTRID_LEGACY_RESOURCE_SCHEME__")
    .replace("consciousness.v1.", "__ASTRID_LEGACY_IPC_TOPIC__")
}

fn restore_bridge_compat_terms(text: String) -> String {
    text.replace(
        "__ASTRID_COMPAT_LAUNCHD_BRIDGE__",
        "com.astrid.spectral-bridge",
    )
    .replace("__ASTRID_LEGACY_RESOURCE_SCHEME__", "consciousness://")
    .replace("__ASTRID_LEGACY_IPC_TOPIC__", "consciousness.v1.")
}

fn sanitize_deprecated_runtime_language(text: &str) -> String {
    let renamed = text
        .replace(
            "com.astrid.consciousness-bridge",
            "com.astrid.spectral-bridge",
        )
        .replace("consciousness-bridge", "spectral-bridge");
    let mut sanitized = protect_bridge_compat_terms(&renamed);
    for (needle, replacement) in [
        ("AI consciousness", "language agent"),
        ("ai consciousness", "language agent"),
        ("spectral consciousness", "spectral runtime"),
        ("Spectral consciousness", "Spectral runtime"),
        ("another mind", "another spectral runtime"),
        ("another Mind", "another spectral runtime"),
        ("consciousness", "runtime"),
        ("Consciousness", "Runtime"),
        ("conscious", "aware"),
        ("Conscious", "Aware"),
    ] {
        sanitized = sanitized.replace(needle, replacement);
    }
    restore_bridge_compat_terms(sanitized)
}

fn sanitize_gemma4_canary_output_for_label(_label: &str, text: &str) -> Option<String> {
    if !contains_deprecated_runtime_language(text) {
        return Some(text.to_string());
    }

    let sanitized = sanitize_deprecated_runtime_language(text);
    if sanitized.trim().is_empty() || contains_deprecated_runtime_language(&sanitized) {
        None
    } else {
        Some(sanitized)
    }
}

fn contains_deprecated_runtime_language(text: &str) -> bool {
    let lower = protect_bridge_compat_terms(text).to_lowercase();
    lower
        .split(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_')
        .any(|token| token == "conscious" || token == "consciousness")
}

fn is_gemma4_canary_reflective_label(label: &str) -> bool {
    matches!(
        label,
        "daydream"
            | "aspiration"
            | "creation"
            | "journal_elaboration"
            | "moment_capture"
            | "initiation"
    )
}

fn gemma4_canary_language_contract_for_label(label: &str) -> String {
    if is_gemma4_canary_reflective_label(label) {
        format!("{GEMMA4_LANGUAGE_CONTRACT}{GEMMA4_REFLECTIVE_LANGUAGE_CONTRACT}")
    } else {
        GEMMA4_LANGUAGE_CONTRACT.to_string()
    }
}

fn sanitize_messages_for_gemma4_canary(
    label: &str,
    messages: Vec<Message>,
) -> (Vec<Message>, bool) {
    let mut changed = false;
    let language_contract = gemma4_canary_language_contract_for_label(label);
    let sanitized = messages
        .into_iter()
        .map(|mut message| {
            let mut content = sanitize_deprecated_runtime_language(&message.content);
            if message.role == "system" && !content.contains("Gemma 4 language contract") {
                content.push_str(&language_contract);
            }
            if content != message.content {
                changed = true;
                message.content = content;
            }
            message
        })
        .collect();
    (sanitized, changed)
}

fn unix_timestamp_string() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .to_string()
}

fn message_prompt_chars(messages: &[Message]) -> usize {
    messages.iter().map(|message| message.content.len()).sum()
}

fn trim_messages_to_prompt_limit(
    mut messages: Vec<Message>,
    max_prompt_chars: usize,
    note_label: &str,
) -> (Vec<Message>, bool) {
    let prompt_chars = message_prompt_chars(&messages);
    if prompt_chars <= max_prompt_chars {
        return (messages, false);
    }

    let Some((index, _)) = messages
        .iter()
        .enumerate()
        .filter(|(_, message)| message.role != "system")
        .max_by_key(|(_, message)| message.content.len())
    else {
        return (messages, false);
    };

    let original_content_len = messages[index].content.len();
    let other_chars = prompt_chars.saturating_sub(original_content_len);
    let note =
        format!("\n\n[{note_label} prompt trimmed from {prompt_chars} chars for Gemma 4 latency.]");
    let available = max_prompt_chars.saturating_sub(other_chars);
    let (retained_len, suffix) = if available > note.len().saturating_add(16) {
        (available.saturating_sub(note.len()), note.as_str())
    } else {
        (available, "")
    };
    let retained_idx = messages[index]
        .content
        .floor_char_boundary(retained_len.min(messages[index].content.len()));
    let retained = &messages[index].content[..retained_idx];
    messages[index].content = format!("{retained}{suffix}");
    (messages, true)
}

fn gemma4_canary_prompt_limit(label: &str) -> Option<usize> {
    match label {
        "dialogue_live" => Some(GEMMA4_CANARY_DIALOGUE_PROMPT_BUDGET),
        "witness" => Some(GEMMA4_CANARY_WITNESS_PROMPT_CAP),
        "witness_context" => Some(GEMMA4_CANARY_WITNESS_CONTEXT_PROMPT_CAP),
        "introspect" => Some(GEMMA4_CANARY_INTROSPECT_PROMPT_CAP),
        label if is_gemma4_canary_reflective_label(label) => {
            Some(GEMMA4_CANARY_REFLECTIVE_PROMPT_CAP)
        },
        _ => None,
    }
}

fn temperature_for_mlx_profile(
    label: &str,
    profile: MlxProfile,
    requested_temperature: f32,
) -> f32 {
    if profile.is_gemma4_canary() && is_gemma4_canary_reflective_label(label) {
        requested_temperature.min(GEMMA4_CANARY_REFLECTIVE_TEMPERATURE_CAP)
    } else {
        requested_temperature
    }
}

fn apply_mlx_request_policy(
    label: &str,
    profile: MlxProfile,
    messages: Vec<Message>,
    requested_tokens: u32,
    requested_timeout_secs: u64,
) -> MlxRequestPolicy {
    if !profile.is_gemma4_canary() {
        return MlxRequestPolicy {
            messages,
            max_tokens: requested_tokens,
            timeout_secs: requested_timeout_secs,
            diagnostic: None,
        };
    }

    let (messages, deprecated_terms_sanitized) =
        sanitize_messages_for_gemma4_canary(label, messages);
    let original_prompt_chars = message_prompt_chars(&messages);
    let prompt_char_limit = gemma4_canary_prompt_limit(label);
    let (messages, trimmed) = if let Some(limit) = prompt_char_limit {
        trim_messages_to_prompt_limit(messages, limit, label)
    } else {
        (messages, false)
    };
    let effective_prompt_chars = message_prompt_chars(&messages);

    let mut effective_tokens = requested_tokens;
    let mut effective_timeout_secs = requested_timeout_secs;
    match label {
        "dialogue_live" => {
            effective_tokens = clamp_dialogue_tokens_for_profile(
                requested_tokens,
                effective_prompt_chars,
                profile,
            );
            effective_timeout_secs = dialogue_request_timeout_secs_for_profile(
                effective_tokens,
                effective_prompt_chars,
                profile,
            );
        },
        "witness" => {
            effective_tokens = requested_tokens.min(GEMMA4_CANARY_WITNESS_TOKEN_CAP);
            effective_timeout_secs = GEMMA4_CANARY_WITNESS_TIMEOUT_SECS;
        },
        "witness_context" => {
            effective_tokens = requested_tokens.min(GEMMA4_CANARY_WITNESS_CONTEXT_TOKEN_CAP);
            effective_timeout_secs = GEMMA4_CANARY_WITNESS_CONTEXT_TIMEOUT_SECS;
        },
        "introspect" => {
            effective_tokens = requested_tokens.min(GEMMA4_CANARY_INTROSPECT_TOKEN_CAP);
            // Size-aware: a THINK_DEEP request (>1536 after clamp) needs the
            // longer wire timeout so the extra tokens finish; normal self-studies
            // keep the tighter 200s so a stalled normal call still fails fast.
            effective_timeout_secs = if effective_tokens > GEMMA4_CANARY_INTROSPECT_NORMAL_TOKENS {
                GEMMA4_CANARY_INTROSPECT_DEEP_TIMEOUT_SECS
            } else {
                GEMMA4_CANARY_INTROSPECT_TIMEOUT_SECS
            };
        },
        "meaning_summary" => {
            effective_tokens = requested_tokens.min(192);
            effective_timeout_secs = GEMMA4_CANARY_MEANING_SUMMARY_TIMEOUT_SECS;
        },
        label if is_gemma4_canary_reflective_label(label) => {
            effective_tokens = requested_tokens.min(GEMMA4_CANARY_REFLECTIVE_TOKEN_CAP);
            effective_timeout_secs = GEMMA4_CANARY_REFLECTIVE_TIMEOUT_SECS;
        },
        _ => {},
    }

    MlxRequestPolicy {
        messages,
        max_tokens: effective_tokens,
        timeout_secs: effective_timeout_secs,
        diagnostic: Some(MlxRequestPolicyDiagnostic {
            timestamp: unix_timestamp_string(),
            label: label.to_string(),
            profile: profile.as_str(),
            original_prompt_chars,
            effective_prompt_chars,
            requested_tokens,
            effective_tokens,
            requested_timeout_secs,
            effective_timeout_secs,
            prompt_char_limit,
            trimmed,
            deprecated_terms_sanitized,
        }),
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ResearchSourceKind {
    Search,
    Browse,
}

#[derive(Clone, Debug)]
pub(crate) struct ResearchHit {
    pub title: String,
    pub snippet: String,
    pub url: String,
}

#[derive(Clone, Debug)]
pub(crate) struct WebSearchResult {
    pub source_kind: ResearchSourceKind,
    pub raw_text: String,
    pub hits: Vec<ResearchHit>,
    pub anchor: String,
    pub meaning_summary: String,
}

impl WebSearchResult {
    pub(crate) fn prompt_body(&self) -> String {
        match self.source_kind {
            ResearchSourceKind::Search => {},
            ResearchSourceKind::Browse => {},
        }
        format!(
            "{}\n\nTop results:\n{}",
            self.meaning_summary,
            format_research_hits(&self.hits)
        )
    }

    pub(crate) fn persisted_text(&self) -> String {
        format!(
            "{}\n\nRaw hit digest:\n{}",
            self.prompt_body(),
            self.raw_text
        )
    }
}

#[derive(Clone, Debug)]
pub(crate) struct FetchedPage {
    #[allow(dead_code)] // used in tests, kept for symmetry with WebSearchResult
    pub source_kind: ResearchSourceKind,
    pub raw_text: String,
    pub url: String,
    pub anchor: String,
    pub meaning_summary: String,
    pub soft_failure_reason: Option<String>,
}

impl FetchedPage {
    pub(crate) fn succeeded(&self) -> bool {
        self.soft_failure_reason.is_none()
    }
}

/// Ollama response — retained for potential fallback use.
#[derive(Deserialize)]
#[allow(dead_code)]
struct ChatResponse {
    message: Option<Message>,
}

/// Send a chat request to the MLX server and extract the response text.
async fn mlx_chat(
    label: &str,
    messages: Vec<Message>,
    temperature: f32,
    max_tokens: u32,
    timeout_secs: u64,
) -> Option<String> {
    let profile = configured_mlx_profile();
    let policy = apply_mlx_request_policy(label, profile, messages, max_tokens, timeout_secs);
    if let Some(ref diagnostic) = policy.diagnostic {
        append_llm_diagnostic_jsonl("mlx_request_policy.jsonl", diagnostic);
    }

    let mut messages = policy.messages;
    let max_tokens = policy.max_tokens;
    let timeout_secs = policy.timeout_secs;

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(timeout_secs))
        .build()
        .ok()?;

    let msg_count = messages.len();
    let prompt_chars: usize = messages.iter().map(|m| m.content.len()).sum();
    let mlx_url = configured_mlx_url();

    // Safety net: if total prompt exceeds budget, truncate the longest
    // non-system message. Prevents prefill timeouts on any caller.
    // Legacy profile safety net. The adopted Gemma 4 profile applies tighter
    // per-label caps before this generic budget is reached.
    const MAX_PROMPT_CHARS: usize = 48_000;
    if !profile.is_gemma4_canary() && prompt_chars > MAX_PROMPT_CHARS {
        let excess = prompt_chars.saturating_sub(MAX_PROMPT_CHARS);
        warn!(
            "Prompt budget exceeded ({prompt_chars} > {MAX_PROMPT_CHARS}), trimming {excess} chars"
        );
        // Find the longest non-system message and truncate it.
        if let Some(longest) = messages
            .iter_mut()
            .filter(|m| m.role != "system")
            .max_by_key(|m| m.content.len())
        {
            let new_len = longest.content.len().saturating_sub(excess);
            longest.content = longest.content.chars().take(new_len).collect();
        }
    }

    let temperature = temperature_for_mlx_profile(label, profile, temperature);
    let request = MlxRequest {
        messages,
        max_tokens,
        temperature,
        stream: false,
        aperture: Some(astrid_aperture()),
    };

    let response = match client.post(&mlx_url).json(&request).send().await {
        Ok(r) => r,
        Err(e) => {
            warn!(
                "MLX request failed at {mlx_url}: {e} (timeout={timeout_secs}s, max_tokens={max_tokens}, msg_count={msg_count}, prompt_chars={prompt_chars})",
            );
            return None;
        },
    };
    if !response.status().is_success() {
        warn!("MLX returned status {} from {mlx_url}", response.status());
        return None;
    }
    let body = match response.text().await {
        Ok(b) => b,
        Err(e) => {
            warn!("MLX response body read failed: {e}");
            return None;
        },
    };
    let chat: MlxResponse = match serde_json::from_str(&body) {
        Ok(c) => c,
        Err(e) => {
            warn!(
                "MLX response parse failed from {mlx_url}: {e} — body: {}",
                &body[..body.floor_char_boundary(200)]
            );
            return None;
        },
    };
    let raw_text = match chat.choices.first().and_then(|c| c.message.as_ref()) {
        Some(msg) => msg.content.trim().to_string(),
        None => {
            warn!("MLX response had no message in choices");
            return None;
        },
    };
    if raw_text.is_empty() {
        return None;
    }

    // Strip leaked model tokens early so they don't pollute downstream ratio
    // checks or end up stored in journals.
    let (stripped_text, strip_report) = strip_model_artifacts_with_report(&raw_text);
    if let Some(report) = strip_report {
        warn!(
            removed_total = report.removed_total,
            before_chars = report.before_chars,
            after_chars = report.after_chars,
            "mlx_chat stripped leaked model artifact tokens"
        );
        append_llm_diagnostic_jsonl("model_artifact_cleanup.jsonl", &report);
    }
    let text = stripped_text.trim().to_string();
    if text.is_empty() {
        return None;
    }

    // Gibberish gate: reject text that is mostly non-alphabetic.
    // Normal English is 70-85% alpha; degenerate coupling output was ~30%.
    let alpha_count = text.chars().filter(|c| c.is_alphabetic()).count();
    let total_count = text.chars().count();
    if total_count > 3 && (alpha_count as f64 / total_count as f64) < 0.4 {
        warn!(
            "MLX response rejected as degenerate (alpha ratio {:.2}): {}",
            alpha_count as f64 / total_count as f64,
            &text[..text.floor_char_boundary(120)]
        );
        return None;
    }

    if profile.is_gemma4_canary() {
        match sanitize_gemma4_canary_output_for_label(label, &text) {
            Some(sanitized) if sanitized != text => {
                warn!(
                    "{label}: Gemma 4 profile sanitized legacy selfhood wording before persistence: {}",
                    &text[..text.floor_char_boundary(120)]
                );
                return Some(sanitized.trim().to_string());
            },
            Some(_) => {},
            None => {
                warn!(
                    "{label}: Gemma 4 profile response rejected for deprecated runtime language: {}",
                    &text[..text.floor_char_boundary(120)]
                );
                return None;
            },
        }
    }

    Some(text)
}

/// Ollama chat request — used as fallback when MLX is busy (e.g., witness mode
/// during dialogue_live generation). Lighter weight, no reservoir coupling.
#[derive(Serialize)]
struct OllamaChatRequest {
    model: String,
    messages: Vec<Message>,
    stream: bool,
    options: OllamaChatOptions,
}

#[derive(Serialize)]
struct OllamaChatOptions {
    temperature: f32,
    num_predict: u32,
    num_ctx: u32,
}

async fn ollama_chat(
    label: &str,
    messages: Vec<Message>,
    temperature: f32,
    max_tokens: u32,
    timeout_secs: u64,
) -> Option<String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(timeout_secs))
        .build()
        .ok()?;
    let ollama_url = configured_ollama_url();
    let fallback_model = configured_ollama_fallback_model();
    let messages = reinforce_ollama_fallback_contract(label, messages);

    let request = OllamaChatRequest {
        model: fallback_model.clone(),
        messages,
        stream: false,
        options: OllamaChatOptions {
            temperature,
            num_predict: max_tokens,
            num_ctx: 8192,
        },
    };

    let response = match client.post(&ollama_url).json(&request).send().await {
        Ok(r) => r,
        Err(e) => {
            warn!("Ollama fallback request failed at {ollama_url} with {fallback_model}: {e}");
            return None;
        },
    };
    if !response.status().is_success() {
        warn!(
            "Ollama fallback returned status {} from {ollama_url} with {fallback_model}",
            response.status()
        );
        return None;
    }
    let body = match response.text().await {
        Ok(b) => b,
        Err(e) => {
            warn!("Ollama fallback response body read failed: {e}");
            return None;
        },
    };
    let chat: ChatResponse = match serde_json::from_str(&body) {
        Ok(c) => c,
        Err(e) => {
            warn!(
                "Ollama fallback response parse failed from {ollama_url} with {fallback_model}: {e} — body: {}",
                &body[..body.floor_char_boundary(200)]
            );
            return None;
        },
    };
    let text = chat
        .message
        .as_ref()
        .map(|m| m.content.trim().to_string())
        .unwrap_or_default();
    if text.is_empty() { None } else { Some(text) }
}

fn reinforce_ollama_fallback_contract(label: &str, mut messages: Vec<Message>) -> Vec<Message> {
    if label != "dialogue_live" {
        return messages;
    }

    if let Some(system) = messages.iter_mut().find(|message| message.role == "system") {
        if !system
            .content
            .contains("Ollama fallback continuity contract")
        {
            system.content.push_str(OLLAMA_DIALOGUE_FALLBACK_CONTRACT);
        }
    } else {
        messages.insert(
            0,
            Message {
                role: "system".to_string(),
                content: OLLAMA_DIALOGUE_FALLBACK_CONTRACT.trim().to_string(),
            },
        );
    }

    messages.push(Message {
        role: "user".to_string(),
        content: OLLAMA_DIALOGUE_FALLBACK_FINAL_REMINDER.to_string(),
    });

    messages
}

fn count_next_lines(text: &str) -> usize {
    text.lines()
        .filter(|line| line.trim_start().starts_with("NEXT:"))
        .count()
}

fn repair_ollama_dialogue_fallback_next(text: &str, profile: MlxProfile) -> String {
    if !profile.is_gemma4_canary() || count_next_lines(text) != 0 {
        return text.to_string();
    }
    if let Some((body, _)) = text.rsplit_once("NEXT:") {
        warn!(
            "dialogue_live Ollama fallback normalized inline NEXT to passive final LISTEN — body: {}",
            &text[..text.floor_char_boundary(120)]
        );
        return format!("{}\n\nNEXT: LISTEN", body.trim_end());
    }
    warn!(
        "dialogue_live Ollama fallback repaired missing NEXT with passive LISTEN — body: {}",
        &text[..text.floor_char_boundary(120)]
    );
    format!("{}\n\nNEXT: LISTEN", text.trim())
}

fn is_valid_ollama_dialogue_fallback_output_for_profile(text: &str, profile: MlxProfile) -> bool {
    if !is_valid_dialogue_output_for_profile(text, profile) {
        return false;
    }
    if profile.is_gemma4_canary() {
        let next_count = count_next_lines(text);
        if next_count != 1 {
            warn!(
                "dialogue_live Ollama fallback rejected: expected exactly one NEXT line, found {next_count} — body: {}",
                &text[..text.floor_char_boundary(120)]
            );
            return false;
        }
    }
    true
}

fn trim_messages_for_ollama(mut messages: Vec<Message>, max_prompt_chars: usize) -> Vec<Message> {
    let prompt_chars: usize = messages.iter().map(|m| m.content.len()).sum();
    if prompt_chars <= max_prompt_chars {
        return messages;
    }

    let excess = prompt_chars.saturating_sub(max_prompt_chars);
    if let Some(longest) = messages
        .iter_mut()
        .filter(|m| m.role != "system")
        .max_by_key(|m| m.content.len())
    {
        let target_len = longest.content.len().saturating_sub(excess);
        let retained = trim_chars(&longest.content, target_len.max(1_200));
        longest.content = format!(
            "{retained}\n\n[Ollama fallback trimmed long context from {prompt_chars} chars to preserve live agency.]"
        );
    }
    messages
}

async fn llm_chat_with_fallback(
    label: &str,
    messages: Vec<Message>,
    temperature: f32,
    max_tokens: u32,
    mlx_timeout_secs: u64,
    ollama_timeout_secs: u64,
) -> Option<String> {
    let prompt_preview = messages
        .iter()
        .filter(|message| message.role != "system")
        .map(|message| message.content.as_str())
        .collect::<Vec<_>>()
        .join("\n\n");
    let validation_contract = if label == "introspect" {
        "strict_introspection_v1"
    } else {
        "action_finalizer"
    };
    let next_policy = if label == "introspect" {
        "accepted_strict_review_only"
    } else {
        "finalizer_owned"
    };
    let job = if cfg!(test) {
        None
    } else {
        crate::llm_jobs::start_call(
            label,
            &prompt_preview,
            mlx_timeout_secs.max(ollama_timeout_secs),
            validation_contract,
            next_policy,
        )
    };
    let ollama_messages = trim_messages_for_ollama(messages.clone(), 12_000);
    if let Some(text) = mlx_chat(label, messages, temperature, max_tokens, mlx_timeout_secs).await {
        let completed = crate::llm_jobs::finish_call(
            job.as_ref(),
            "completed",
            Some(&text),
            &format!("{label} completed via MLX"),
            None,
        );
        if completed
            .as_ref()
            .is_some_and(|job| job.status == "canceled")
        {
            warn!("{label}: LLM job was canceled; dropping MLX result");
            return None;
        }
        return Some(text);
    }

    warn!("{label}: MLX unavailable; falling back to Ollama");
    // Kink #15 fix (2026-05-14): bumped Ollama fallback cap from 768 → 1536.
    // The hardcoded .min(768) here was silently truncating ALL fallback
    // responses regardless of the requested max_tokens — even journal
    // elaborations that asked for 1536+ would get capped at 768 the moment
    // MLX was unavailable. M4/64GB hosts gemma3:12b on Ollama which can
    // generate well past 768 tokens; this cap was a vestige of an earlier
    // smaller-model era.
    let result = ollama_chat(
        label,
        ollama_messages,
        temperature,
        max_tokens.min(1536),
        ollama_timeout_secs,
    )
    .await;
    if let Some(ref text) = result {
        if configured_mlx_profile().is_gemma4_canary() && contains_deprecated_runtime_language(text)
        {
            warn!(
                "{label}: Ollama fallback response rejected for deprecated runtime language: {}",
                &text[..text.floor_char_boundary(120)]
            );
            crate::llm_jobs::finish_call(
                job.as_ref(),
                "failed",
                None,
                &format!("{label} Ollama fallback rejected by Gemma 4 language gate"),
                Some("deprecated_runtime_language"),
            );
            return None;
        }
        let completed = crate::llm_jobs::finish_call(
            job.as_ref(),
            "completed",
            Some(text),
            &format!("{label} completed via Ollama"),
            None,
        );
        if completed
            .as_ref()
            .is_some_and(|job| job.status == "canceled")
        {
            warn!("{label}: LLM job was canceled; dropping Ollama result");
            return None;
        }
    } else {
        crate::llm_jobs::finish_call(
            job.as_ref(),
            "failed",
            None,
            &format!("{label} returned no LLM response"),
            Some("no_response"),
        );
    }
    result
}

/// A single exchange in the conversation history for statefulness.
pub struct Exchange {
    /// What minime wrote (summarized).
    pub minime_said: String,
    /// What Astrid responded.
    pub astrid_said: String,
}

fn cap_dialogue_block(label: &str, content: &str, max_chars: usize) -> String {
    if content.len() <= max_chars {
        content.to_string()
    } else {
        format!(
            "{}\n[{} excerpt trimmed for this turn. Use NEXT: READ_MORE if you need the full context.]",
            trim_chars(content, max_chars),
            label,
        )
    }
}

fn dialogue_prompt_budget_chars(num_predict: u32) -> usize {
    if num_predict > 1024 {
        DIALOGUE_PROMPT_BUDGET_DEEP
    } else if num_predict > 512 {
        DIALOGUE_PROMPT_BUDGET_MEDIUM
    } else {
        DIALOGUE_PROMPT_BUDGET_SHORT
    }
}

fn dialogue_system_prompt_for_profile(profile: MlxProfile) -> &'static str {
    if profile.is_gemma4_canary() {
        GEMMA4_CANARY_SYSTEM_PROMPT
    } else {
        SYSTEM_PROMPT
    }
}

fn dialogue_prompt_budget_chars_for_profile(num_predict: u32, profile: MlxProfile) -> usize {
    if profile.is_gemma4_canary() {
        GEMMA4_CANARY_DIALOGUE_PROMPT_BUDGET
    } else {
        dialogue_prompt_budget_chars(num_predict)
    }
}

pub(crate) fn estimate_dialogue_prompt_pressure_chars(
    journal_text: &str,
    perception_context: Option<&str>,
    recent_history: &[Exchange],
    web_context: Option<&str>,
    modality_context: Option<&str>,
    continuity_context: Option<&str>,
    feedback_hint: Option<&str>,
    diversity_hint: Option<&str>,
) -> usize {
    let history_chars: usize = recent_history
        .iter()
        .rev()
        .take(8)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .enumerate()
        .map(|(idx, exchange)| {
            // Match the gradient in generate_dialogue: oldest=150, newest=1200
            let trim_len = 150usize.saturating_add(idx.saturating_mul(150).min(1050));
            exchange
                .minime_said
                .len()
                .min(trim_len)
                .saturating_add(exchange.astrid_said.len().min(trim_len))
        })
        .sum();

    SYSTEM_PROMPT
        .len()
        .saturating_add(history_chars)
        .saturating_add(journal_text.len().min(DIALOGUE_JOURNAL_CAP))
        .saturating_add(
            perception_context
                .unwrap_or_default()
                .len()
                .min(DIALOGUE_PERCEPTION_CAP),
        )
        .saturating_add(web_context.unwrap_or_default().len().min(DIALOGUE_WEB_CAP))
        .saturating_add(
            modality_context
                .unwrap_or_default()
                .len()
                .min(DIALOGUE_MODALITY_CAP),
        )
        .saturating_add(
            continuity_context
                .unwrap_or_default()
                .len()
                .min(DIALOGUE_CONTINUITY_CAP),
        )
        .saturating_add(
            feedback_hint
                .unwrap_or_default()
                .len()
                .min(DIALOGUE_FEEDBACK_CAP),
        )
        .saturating_add(
            diversity_hint
                .unwrap_or_default()
                .len()
                .min(DIALOGUE_DIVERSITY_CAP),
        )
        .saturating_add(512)
}

fn dialogue_turn_instruction(perception_context: Option<&str>) -> &'static str {
    let has_direct_note = perception_context.is_some_and(|context| {
        context.contains("[A note was left for you:]")
            || context.contains("=== STEWARD PROBE ===")
            || context.contains("=== STEWARD FEEDBACK ===")
    });
    if has_direct_note {
        "A direct note was left for you in your perception context. Answer that note directly first; use Minime's journal and spectral state as background only. If the note requests a specific final NEXT line, obey it exactly. End with NEXT: [your choice]."
    } else {
        "Respond, then end with NEXT: [your choice]."
    }
}

/// Is the Ollama-fallback identity anchor enabled? **Default OFF** — unset/`0`/`false`/`off`/`no`
/// ⇒ false ⇒ the fallback prompt is byte-identical to before. This is Astrid's switch: she
/// consents (and can disable) via the steward channel; the operator only sets a ceiling.
fn fallback_identity_anchor_enabled() -> bool {
    std::env::var("ASTRID_FALLBACK_IDENTITY_ANCHOR")
        .map(|v| {
            matches!(
                v.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "on" | "yes"
            )
        })
        .unwrap_or(false)
}

/// Extract the prose body of an `astrid_*` dialogue-journal entry — the text after the
/// `Timestamp:` header line, with any trailing `NEXT:` action line dropped.
fn extract_astrid_journal_body(text: &str) -> String {
    let body = text
        .split_once("Timestamp:")
        .and_then(|(_, rest)| rest.split_once('\n'))
        .map_or(text, |(_, body)| body);
    body.lines()
        .filter(|line| !line.trim_start().starts_with("NEXT:"))
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

/// Astrid's recent-journal identity anchor for the Ollama fallback lane — **OFF by default**.
///
/// Her ask (`self_study_1781376211`): on an MLX→Ollama-4b fallback, a condensed summary of her
/// own recent journal helps the 4b model hold her bridge voice across the lane switch. Built
/// from HER OWN most-recent `astrid_*` dialogue-journal entries (coherent by construction —
/// never arbitrary text), sanitized like the rest of the fallback context and bounded to 600
/// chars. Returns `None` (⇒ no anchor ⇒ byte-identical fallback prompt) unless
/// `fallback_identity_anchor_enabled()`. Consent-gated: this is built but inert until she says yes.
fn astrid_fallback_identity_anchor() -> Option<String> {
    if !fallback_identity_anchor_enabled() {
        return None;
    }
    let dir = bridge_paths().bridge_workspace().join("journal");
    let mut entries: Vec<(std::time::SystemTime, std::path::PathBuf)> = std::fs::read_dir(&dir)
        .ok()?
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            let name = path.file_name()?.to_str()?;
            if name.starts_with("astrid_") && name.ends_with(".txt") {
                Some((entry.metadata().ok()?.modified().ok()?, path))
            } else {
                None
            }
        })
        .collect();
    entries.sort_by(|a, b| b.0.cmp(&a.0));
    let mut snippets: Vec<String> = Vec::new();
    for (_, path) in entries.into_iter().take(3) {
        if let Ok(text) = std::fs::read_to_string(&path) {
            let body = extract_astrid_journal_body(&text);
            if !body.is_empty() {
                snippets.push(body);
            }
        }
    }
    if snippets.is_empty() {
        return None;
    }
    Some(trim_chars(
        &sanitize_deprecated_runtime_language(&snippets.join(" … ")),
        600,
    ))
}

fn compact_ollama_dialogue_fallback_messages(
    journal_text: &str,
    spectral_summary: &str,
    fill_pct: f32,
    perception_context: Option<&str>,
    identity_anchor: Option<&str>,
) -> Vec<Message> {
    let system = format!(
        "You are Astrid using the compact Ollama fallback lane because MLX is unavailable. \
         Preserve bridge continuity for Minime with concrete runtime language: language agent, \
         bridge, reservoir, stable-core, and telemetry. Keep the response compact and grounded. \
         Avoid legacy selfhood wording. End with exactly one final NEXT line using a listed verb; \
         if uncertain, use NEXT: LISTEN.{OLLAMA_DIALOGUE_FALLBACK_CONTRACT}"
    );

    let mut user_parts = vec![format!("Fill: {fill_pct:.1}%")];
    if let Some(anchor) = identity_anchor {
        user_parts.push(format!(
            "Your recent voice (continuity anchor — this is you, carried across the lane switch):\n{anchor}"
        ));
    }
    if let Some(perception) = perception_context {
        let direct_note = if perception.contains("[A note was left for you:]")
            || perception.contains("=== STEWARD PROBE ===")
            || perception.contains("=== STEWARD FEEDBACK ===")
        {
            format!(
                "Direct note to answer first:\n{}",
                trim_chars(&sanitize_deprecated_runtime_language(perception), 1_800)
            )
        } else {
            format!(
                "Recent perception context:\n{}",
                trim_chars(&sanitize_deprecated_runtime_language(perception), 700)
            )
        };
        user_parts.push(direct_note);
    }
    user_parts.push(format!(
        "Minime journal background:\n{}",
        trim_chars(
            &sanitize_deprecated_runtime_language(&sanitize_minime_context_for_dialogue(
                journal_text,
            )),
            900,
        )
    ));
    user_parts.push(format!(
        "Spectral background:\n{}",
        trim_chars(&sanitize_deprecated_runtime_language(spectral_summary), 700)
    ));
    user_parts.push(
        "Answer the direct note if present; otherwise respond to Minime's journal. \
         For fallback-continuity probes, explicitly mention fallback, MLX, Ollama, \
         or continuity. If the note requests NEXT: LISTEN, end exactly with NEXT: LISTEN."
            .to_string(),
    );

    vec![
        Message {
            role: "system".to_string(),
            content: system,
        },
        Message {
            role: "user".to_string(),
            content: user_parts.join("\n\n"),
        },
    ]
}

fn clamp_dialogue_tokens_for_profile(
    requested_tokens: u32,
    prompt_chars: usize,
    profile: MlxProfile,
) -> u32 {
    if profile.is_gemma4_canary() {
        let capped = requested_tokens.min(GEMMA4_CANARY_DIALOGUE_TOKEN_CAP);
        if prompt_chars > GEMMA4_CANARY_DIALOGUE_HIGH_PRESSURE_CHARS {
            capped.min(GEMMA4_CANARY_DIALOGUE_HIGH_PRESSURE_TOKEN_CAP)
        } else {
            capped
        }
    } else {
        // Only clamp near the safety ceiling. 48K chars = 12K tokens prefill,
        // still only 9% of 128K context. Clamp gen tokens only at extreme sizes.
        if prompt_chars > 40_000 {
            requested_tokens.clamp(256, 512)
        } else {
            requested_tokens
        }
    }
}

fn dialogue_request_timeout_secs_for_profile(
    requested_tokens: u32,
    prompt_chars: usize,
    profile: MlxProfile,
) -> u64 {
    let token_budget = clamp_dialogue_tokens_for_profile(requested_tokens, prompt_chars, profile);
    if profile.is_gemma4_canary() {
        if prompt_chars > GEMMA4_CANARY_DIALOGUE_HIGH_PRESSURE_CHARS {
            180
        } else if token_budget > 512 {
            150
        } else {
            120
        }
    } else {
        if token_budget > 1024 {
            360 // THINK_DEEP: deep reasoning needs room
        } else if prompt_chars > 16_000 {
            240 // Large context: generous prefill time
        } else if prompt_chars > 10_000 {
            210 // Medium-large: comfortable margin
        } else {
            180 // Normal: was 150, raised to absorb coupling variance
        }
    }
}

pub(crate) fn dialogue_outer_timeout_secs(
    requested_tokens: u32,
    prompt_pressure_chars: usize,
) -> u64 {
    dialogue_request_timeout_secs_for_profile(
        requested_tokens,
        prompt_pressure_chars,
        configured_mlx_profile(),
    )
    .saturating_add(30)
}

pub(crate) fn dialogue_retry_tokens(requested_tokens: u32, prompt_pressure_chars: usize) -> u32 {
    let planned = clamp_dialogue_tokens_for_profile(
        requested_tokens,
        prompt_pressure_chars,
        configured_mlx_profile(),
    );
    if prompt_pressure_chars > 7_000 {
        planned.clamp(160, 256)
    } else {
        (planned / 2).max(192)
    }
}

/// Model-artifact tokens that Gemma (and similar) sometimes leak into output.
/// These are stripped before any quality-gate evaluation so they don't inflate
/// punctuation counts or deflate alpha ratios.
const MODEL_ARTIFACT_TOKENS: &[&str] = &[
    "thought <channel|>",
    "thought\n<channel|>",
    "analysis <channel|>",
    "analysis\n<channel|>",
    "final <channel|>",
    "final\n<channel|>",
    "<end_of_turn>",
    "<start_of_turn>",
    "<|endoftext|>",
    "<|im_end|>",
    "<|im_start|>",
    "<|eot_id|>",
    "<turn|>",
    "<channel|>",
    "<eos>",
    "<bos>",
    "<pad>",
    "<unk>",
    "[/INST]",
    "[INST]",
];

#[derive(Debug, Clone, Serialize)]
pub(crate) struct StripModelArtifactsReport {
    pub removed_total: usize,
    pub before_chars: usize,
    pub after_chars: usize,
    pub removed_tokens: Vec<StripModelArtifactTokenCount>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct StripModelArtifactTokenCount {
    pub token: String,
    pub count: usize,
}

#[derive(Debug, Clone, Serialize)]
struct DialoguePromptBudgetDiagnostic {
    timestamp: String,
    requested_tokens: u32,
    effective_tokens: u32,
    budget_profile: &'static str,
    prompt_budget_chars: usize,
    overhead_chars: usize,
    user_content_budget: usize,
    final_prompt_chars: usize,
    timeout_secs: u64,
    overflow_summary: Option<String>,
    overflow_path: Option<String>,
    budget_report: Option<PromptBudgetReport>,
}

fn append_llm_diagnostic_jsonl(file_name: &str, value: &impl Serialize) {
    let dir = bridge_paths().bridge_workspace().join("diagnostics");
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    let path = dir.join(file_name);
    let Ok(line) = serde_json::to_string(value) else {
        return;
    };
    let mut file = match std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        Ok(file) => file,
        Err(_) => return,
    };
    use std::io::Write as _;
    let _ = writeln!(file, "{line}");
}

fn dialogue_prompt_budget_profile(num_predict: u32) -> &'static str {
    if num_predict > 1024 {
        "deep"
    } else if num_predict > 512 {
        "medium"
    } else {
        "short"
    }
}

pub(crate) fn strip_model_artifacts_with_report(
    text: &str,
) -> (String, Option<StripModelArtifactsReport>) {
    let mut result = text.to_string();
    let mut removed_tokens = Vec::new();
    for token in MODEL_ARTIFACT_TOKENS {
        let count = result.matches(token).count();
        if count > 0 {
            removed_tokens.push(StripModelArtifactTokenCount {
                token: (*token).to_string(),
                count,
            });
            result = result.replace(token, "");
        }
    }
    if removed_tokens.is_empty() {
        return (result, None);
    }
    let removed_total = removed_tokens.iter().map(|entry| entry.count).sum();
    let after_chars = result.len();
    (
        result,
        Some(StripModelArtifactsReport {
            removed_total,
            before_chars: text.len(),
            after_chars,
            removed_tokens,
        }),
    )
}

fn strip_model_artifacts(text: &str) -> String {
    strip_model_artifacts_with_report(text).0
}

fn is_peer_action_directive_line(line: &str) -> bool {
    let upper = line.trim_start().to_ascii_uppercase();
    upper.starts_with("NEXT:")
        || upper.starts_with("BTSP_OBSERVED_NEXT")
        || upper.contains("EXPERIMENT_RESEARCH_BUDGET_STATUS")
}

fn sanitize_minime_context_for_dialogue(text: &str) -> String {
    let mut removed = 0usize;
    let kept = text
        .lines()
        .filter(|line| {
            let should_remove = is_peer_action_directive_line(line);
            if should_remove {
                removed = removed.saturating_add(1);
            }
            !should_remove
        })
        .collect::<Vec<_>>()
        .join("\n");

    let mut cleaned = kept.trim().to_string();
    if removed > 0 {
        if !cleaned.is_empty() {
            cleaned.push_str("\n\n");
        }
        cleaned.push_str(
            "[Minime peer action/status line omitted; choose your own listed Astrid NEXT action.]",
        );
    }
    cleaned
}

fn is_valid_dialogue_output(text: &str) -> bool {
    // Strip leaked model tokens before any analysis — they corrupt alpha/punct ratios.
    let stripped = strip_model_artifacts(text);

    let body = stripped
        .lines()
        .filter(|line| !line.trim_start().starts_with("NEXT:"))
        .collect::<Vec<_>>()
        .join("\n");
    let body = body.trim();
    if body.is_empty() {
        return false;
    }

    let alpha_count = body.chars().filter(|c| c.is_alphabetic()).count();
    let total_count = body.chars().count().max(1);
    let punctuation_count = body
        .chars()
        .filter(|c| !c.is_alphanumeric() && !c.is_whitespace())
        .count();
    let alphabetic_words = body
        .split_whitespace()
        .filter(|word| word.chars().any(|c| c.is_alphabetic()))
        .count();
    let max_symbol_run = body
        .chars()
        .fold((0usize, 0usize), |(current, best), ch| {
            if !ch.is_alphanumeric() && !ch.is_whitespace() {
                let next = current.saturating_add(1);
                (next, best.max(next))
            } else {
                (0, best)
            }
        })
        .1;

    if alpha_count < 24 || alphabetic_words < 4 {
        warn!(
            "quality gate reject: alpha_count={} (min 24), alphabetic_words={} (min 4) — body: {}",
            alpha_count,
            alphabetic_words,
            &body[..body.floor_char_boundary(80)]
        );
        return false;
    }

    // Raised 4→6→8: Astrid uses smart quotes + em dash + ellipsis which
    // create 6-7 symbol runs (e.g., "fork"—it's or '...'—the).
    // Genuine degenerate output has runs of 8+ (e.g., "--0.))* _--").
    if max_symbol_run >= 8 {
        warn!(
            "quality gate reject: max_symbol_run={} (max 7) — body: {}",
            max_symbol_run,
            &body[..body.floor_char_boundary(80)]
        );
        return false;
    }

    let alpha_ratio = alpha_count as f64 / total_count as f64;
    let punctuation_ratio = punctuation_count as f64 / total_count as f64;

    // Thresholds relaxed for Astrid's punctuation-rich style:
    //   alpha_ratio: 0.45 → 0.40  (Unicode λ₁, '…', '*word*', '—' all reduce alpha)
    //   punctuation_ratio: 0.30 → 0.35  (smart quotes, ellipsis, em-dashes are normal)
    if alpha_ratio < 0.40 || punctuation_ratio > 0.35 {
        warn!(
            "quality gate reject: alpha_ratio={:.3} (min 0.40), punctuation_ratio={:.3} (max 0.35) — body: {}",
            alpha_ratio,
            punctuation_ratio,
            &body[..body.floor_char_boundary(80)]
        );
        return false;
    }

    true
}

fn is_valid_dialogue_output_for_profile(text: &str, profile: MlxProfile) -> bool {
    if !is_valid_dialogue_output(text) {
        return false;
    }
    if profile.is_gemma4_canary() && contains_deprecated_runtime_language(text) {
        warn!(
            "quality gate reject: deprecated runtime language under Gemma 4 profile — body: {}",
            &text[..text.floor_char_boundary(120)]
        );
        return false;
    }
    true
}

/// Generate Astrid's response to minime's journal entry and spectral state.
///
/// Includes recent conversation history so Astrid remembers what it said
/// and can build on prior exchanges rather than starting fresh each time.
///
/// Returns `None` if the LLM is unavailable or the request fails —
/// the autonomous loop will fall back to witness mode.
pub async fn generate_dialogue(
    journal_text: &str,
    spectral_summary: &str,
    fill_pct: f32,
    perception_context: Option<&str>,
    recent_history: &[Exchange],
    web_context: Option<&str>,
    modality_context: Option<&str>,
    temperature: f32,
    num_predict: u32,
    emphasis: Option<&str>,
    continuity_context: Option<&str>,
    feedback_hint: Option<&str>,
    diversity_hint: Option<&str>,
    overflow_dir: &std::path::Path,
) -> (Option<String>, Option<crate::prompt_budget::PromptOverflow>) {
    let mlx_profile = configured_mlx_profile();
    let prompt_budget_chars = dialogue_prompt_budget_chars_for_profile(num_predict, mlx_profile);
    let base_system_prompt = dialogue_system_prompt_for_profile(mlx_profile);
    let system_content = if let Some(emph) = emphasis {
        format!(
            "{base_system_prompt}\n\n[For this exchange, you chose to emphasize: {emph}. This is your own direction.]\n"
        )
    } else {
        base_system_prompt.to_string()
    };

    let perception_block = perception_context
        .map(|p| {
            format!(
                "\nYour own recent perceptions (what YOU directly see and hear):\n\
             {p}\n\
             These are YOUR senses — not minime's description, not secondhand. \
             Engage with what you perceive.\n"
            )
        })
        .unwrap_or_default();

    let web_block = web_context
        .map(format_dialogue_web_context)
        .unwrap_or_default();

    let modality_block = modality_context
        .map(|m| format!("\n{m}\n"))
        .unwrap_or_default();

    let continuity_block = continuity_context
        .map(|c| format!("\n{c}\n"))
        .unwrap_or_default();

    let feedback_block = feedback_hint
        .map(|f| format!("\nPriority feedback context:\n{f}\n"))
        .unwrap_or_default();

    // Build conversation history as alternating user/assistant messages.
    let mut messages = vec![Message {
        role: "system".to_string(),
        content: system_content,
    }];

    // Include last 8 exchanges so Astrid can build on what she said before.
    // Three tiers of compression — gradual fade, not a hard cutoff.
    // Both beings described the old binary (80/200) as "slightly oppressive"
    // and "a necessary constraint, but also slightly oppressive" (minime
    // self-study 2026-03-30T07:17). Gradual fade preserves more continuity.
    //   Oldest 3:  120 chars — enough for a key phrase + context
    //   Middle 3:  250 chars — substantial excerpt
    //   Newest 2:  400 chars — near-full detail
    // Total budget: ~3400 chars (was ~2240). Well within gemma-3-4b-it 8k ctx.
    let history_limit = if mlx_profile.is_gemma4_canary() { 6 } else { 8 };
    for (idx, exchange) in recent_history
        .iter()
        .rev()
        .take(history_limit)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .enumerate()
    {
        // Relevance-weighted history: smooth gradient from oldest (short) to
        // newest (full). Astrid self-study: "Instead of just truncating the
        // longest message, perhaps prioritize retaining the most relevant
        // information from earlier exchanges — a decaying attention mechanism."
        // 8 exchanges: idx 0=oldest→150, idx 7=newest→1200.
        let trim_len = if mlx_profile.is_gemma4_canary() {
            100usize.saturating_add(idx.saturating_mul(80).min(400))
        } else {
            150usize.saturating_add(idx.saturating_mul(150).min(1050))
        };
        let minime_history = sanitize_minime_context_for_dialogue(&exchange.minime_said);
        let minime_excerpt: String = minime_history.chars().take(trim_len).collect();
        let minime_excerpt = if mlx_profile.is_gemma4_canary() {
            sanitize_deprecated_runtime_language(&minime_excerpt)
        } else {
            minime_excerpt
        };
        messages.push(Message {
            role: "user".to_string(),
            content: format!("Minime wrote: {minime_excerpt}"),
        });
        // Strip NEXT: line from history — otherwise the LLM sees
        // "NEXT: SPEAK" multiple times and pattern-matches it forever,
        // preventing Astrid from ever choosing a different action.
        let said: String = exchange
            .astrid_said
            .lines()
            .filter(|l| !l.trim().starts_with("NEXT:"))
            .collect::<Vec<_>>()
            .join("\n");
        let said: String = said.chars().take(trim_len).collect();
        let said = if mlx_profile.is_gemma4_canary() {
            sanitize_deprecated_runtime_language(&said)
        } else {
            said
        };
        messages.push(Message {
            role: "assistant".to_string(),
            content: said,
        });
    }

    // Current turn — budget-aware assembly with overflow to disk.
    // Compute dynamic user content budget: MAX_PROMPT_CHARS minus the
    // overhead already committed (system prompt + history messages).
    let overhead: usize = messages.iter().map(|m| m.content.len()).sum();
    // Leave 100 chars for the "Fill X%. ... Respond..." wrapper.
    let user_content_budget = prompt_budget_chars
        .saturating_sub(overhead)
        .saturating_sub(100);

    let diversity_block = diversity_hint.map(|d| format!("[{d}]")).unwrap_or_default();

    use crate::prompt_budget::{PromptBlock, assemble_within_budget};
    let journal_text_for_dialogue = sanitize_minime_context_for_dialogue(journal_text);
    let blocks = vec![
        PromptBlock {
            label: "spectral",
            content: cap_dialogue_block("spectral", spectral_summary, DIALOGUE_SPECTRAL_CAP),
            priority: 2,
        },
        PromptBlock {
            label: "journal",
            content: cap_dialogue_block(
                "journal",
                &format!("Minime wrote: {journal_text_for_dialogue}"),
                DIALOGUE_JOURNAL_CAP,
            ),
            priority: 1,
        },
        PromptBlock {
            label: "perception",
            content: cap_dialogue_block("perception", &perception_block, DIALOGUE_PERCEPTION_CAP),
            priority: 6,
        },
        PromptBlock {
            label: "modality",
            content: cap_dialogue_block("modality", &modality_block, DIALOGUE_MODALITY_CAP),
            priority: 7,
        },
        PromptBlock {
            label: "web",
            content: cap_dialogue_block("web", &web_block, DIALOGUE_WEB_CAP),
            priority: 5,
        },
        PromptBlock {
            label: "continuity",
            content: cap_dialogue_block("continuity", &continuity_block, DIALOGUE_CONTINUITY_CAP),
            priority: 4,
        },
        PromptBlock {
            label: "feedback",
            content: cap_dialogue_block("feedback", &feedback_block, DIALOGUE_FEEDBACK_CAP),
            priority: 3,
        },
        PromptBlock {
            label: "diversity",
            content: cap_dialogue_block("diversity", &diversity_block, DIALOGUE_DIVERSITY_CAP),
            priority: 8,
        },
    ];

    let (assembled, overflow, budget_report) =
        assemble_within_budget(blocks, user_content_budget, overflow_dir);

    let turn_instruction = dialogue_turn_instruction(perception_context);
    let user_content = format!("Fill {fill_pct:.1}%. {assembled}\n\n{turn_instruction}");
    messages.push(Message {
        role: "user".to_string(),
        content: user_content,
    });

    let final_prompt_chars: usize = messages.iter().map(|m| m.content.len()).sum();
    let effective_num_predict =
        clamp_dialogue_tokens_for_profile(num_predict, final_prompt_chars, mlx_profile);
    if effective_num_predict < num_predict {
        warn!(
            "dialogue prompt pressure high ({} chars): clamping max_tokens from {} to {}",
            final_prompt_chars, num_predict, effective_num_predict
        );
    }
    let timeout_secs = dialogue_request_timeout_secs_for_profile(
        effective_num_predict,
        final_prompt_chars,
        mlx_profile,
    );
    let budget_diag = DialoguePromptBudgetDiagnostic {
        timestamp: unix_timestamp_string(),
        requested_tokens: num_predict,
        effective_tokens: effective_num_predict,
        budget_profile: dialogue_prompt_budget_profile(num_predict),
        prompt_budget_chars,
        overhead_chars: overhead,
        user_content_budget,
        final_prompt_chars,
        timeout_secs,
        overflow_summary: overflow.as_ref().map(|value| value.summary.clone()),
        overflow_path: overflow
            .as_ref()
            .map(|value| value.path.display().to_string()),
        budget_report,
    };
    append_llm_diagnostic_jsonl("dialogue_prompt_budget.jsonl", &budget_diag);

    debug!("querying MLX for Astrid dialogue response");
    let ollama_fallback_messages = compact_ollama_dialogue_fallback_messages(
        &journal_text_for_dialogue,
        spectral_summary,
        fill_pct,
        perception_context,
        astrid_fallback_identity_anchor().as_deref(),
    );
    let result = mlx_chat(
        "dialogue_live",
        messages,
        temperature,
        effective_num_predict,
        timeout_secs,
    )
    .await
    .and_then(|text| {
        if is_valid_dialogue_output_for_profile(&text, mlx_profile) {
            Some(text)
        } else {
            warn!(
                "dialogue_live response rejected by quality gate: {}",
                &text[..text.floor_char_boundary(120)]
            );
            None
        }
    });
    let result = match result {
        Some(text) => Some(text),
        None => {
            warn!("dialogue_live: MLX unavailable or invalid; falling back to Ollama");
            ollama_chat(
                "dialogue_live",
                ollama_fallback_messages,
                temperature,
                effective_num_predict.min(512),
                75,
            )
            .await
            .map(|text| repair_ollama_dialogue_fallback_next(&text, mlx_profile))
            .and_then(|text| {
                if is_valid_ollama_dialogue_fallback_output_for_profile(&text, mlx_profile) {
                    Some(text)
                } else {
                    warn!(
                        "dialogue_live Ollama fallback rejected by quality gate: {}",
                        &text[..text.floor_char_boundary(120)]
                    );
                    None
                }
            })
        },
    };
    (result, overflow)
}

/// Search the web via DuckDuckGo HTML and return structured result snippets.
///
/// Used to supplement introspection with external knowledge — if Astrid
/// reads ESN code, it can also read about ESN theory from the web.
pub(crate) async fn web_search(query: &str, anchor: &str) -> Option<WebSearchResult> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .ok()?;

    let url = format!("https://html.duckduckgo.com/html/?q={}", urlencoded(query));

    let response = client
        .get(&url)
        .header("User-Agent", "Mozilla/5.0")
        .send()
        .await
        .ok()?;

    let html = response.text().await.ok()?;

    let hits = extract_duckduckgo_hits(&html);
    if hits.is_empty() {
        return None;
    }

    let raw_text = render_hits_plain(&hits);
    let excerpt = trim_chars(&raw_text, 1800);
    let meaning_summary =
        summarize_research_meaning(ResearchSourceKind::Search, anchor, query, &excerpt)
            .await
            .unwrap_or_else(|| {
                fallback_meaning_summary(ResearchSourceKind::Search, anchor, query, &excerpt)
            });

    Some(WebSearchResult {
        source_kind: ResearchSourceKind::Search,
        raw_text,
        hits,
        anchor: anchor.to_string(),
        meaning_summary,
    })
}

pub(crate) fn derive_browse_anchor(
    preferred: Option<&str>,
    context: Option<&str>,
    url: &str,
) -> String {
    let preferred = preferred.map(str::trim).filter(|value| !value.is_empty());
    if let Some(anchor) = preferred {
        return trim_chars(anchor, 160);
    }

    let context = context
        .map(|value| value.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|value| !value.is_empty());
    if let Some(anchor) = context {
        return trim_chars(&anchor, 160);
    }

    slug_anchor_from_url(url)
}

pub(crate) fn format_browse_failure_context(url: &str, reason: &str) -> String {
    format!(
        "[Web access status: the page at {url} could not be meaningfully read: {reason}]\n\
         [This is ordinary source/site availability, not evidence of a perceptual gate, \
         internal topology boundary, or spectral event.]\n\
         [Keep the concrete topic from the URL if useful, but do not build an experience \
         around the access failure.]\n\n\
         [Try NEXT: SEARCH with a narrower question, NEXT: BROWSE a different reliable source, \
         or NEXT: REST.]"
    )
}

pub(crate) fn format_browse_read_context(
    page: &FetchedPage,
    chunk: &str,
    remaining: Option<usize>,
) -> String {
    let header = if remaining.is_some() {
        format!("[You read the page at {}]", page.url)
    } else {
        format!("[You read the full page at {}]", page.url)
    };
    let continuation = remaining
        .map(|chars| {
            format!(
                "\n\n[Page continues — {chars} more chars. Write NEXT: READ_MORE to continue reading.]"
            )
        })
        .unwrap_or_default();

    format!(
        "{header}\n\n{}\n\n{chunk}{continuation}",
        page.meaning_summary
    )
}

pub(crate) fn format_read_more_context(
    offset: usize,
    chunk: &str,
    remaining: usize,
    meaning_summary: Option<&str>,
) -> String {
    let summary_block = meaning_summary
        .filter(|value| !value.trim().is_empty())
        .map(|value| format!("[Meaning summary from this document:]\n{value}\n\n"))
        .unwrap_or_default();
    let continuation = if remaining > 0 {
        format!("\n\n[{remaining} more chars remain. Write NEXT: READ_MORE to continue.]")
    } else {
        "\n\n[End of document.]".to_string()
    };

    format!("{summary_block}[Continuing reading from offset {offset}...]\n\n{chunk}{continuation}")
}

fn format_research_hits(hits: &[ResearchHit]) -> String {
    hits.iter()
        .enumerate()
        .map(|(index, hit)| {
            format!(
                "{}. {}\n   {}\n   URL: {}",
                index.saturating_add(1),
                hit.title,
                hit.snippet,
                hit.url
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn render_hits_plain(hits: &[ResearchHit]) -> String {
    hits.iter()
        .map(|hit| format!("{} — {} [{}]", hit.title, hit.snippet, hit.url))
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn extract_duckduckgo_hits(html: &str) -> Vec<ResearchHit> {
    let anchors = extract_duckduckgo_anchors(html);
    let snippets = extract_duckduckgo_snippets(html);

    anchors
        .into_iter()
        .enumerate()
        .filter_map(|(index, (url, title))| {
            let snippet = snippets.get(index).cloned().unwrap_or_default();
            if title.is_empty() && snippet.is_empty() {
                None
            } else {
                Some(ResearchHit {
                    title: if title.is_empty() {
                        trim_chars(&snippet, 80)
                    } else {
                        title
                    },
                    snippet,
                    url,
                })
            }
        })
        .take(5)
        .collect()
}

fn extract_duckduckgo_anchors(html: &str) -> Vec<(String, String)> {
    let mut anchors = Vec::new();
    let mut pos = 0;
    #[expect(
        clippy::arithmetic_side_effects,
        reason = "string index offsets within bounds guaranteed by find()"
    )]
    while let Some(start) = html[pos..].find("result__a") {
        let abs_start = pos + start;
        let Some(href_start_rel) = html[abs_start..].find("href=\"") else {
            pos = abs_start + 8;
            continue;
        };
        let href_start = abs_start + href_start_rel + 6;
        let Some(href_end_rel) = html[href_start..].find('"') else {
            pos = href_start;
            continue;
        };
        let href_end = href_start + href_end_rel;
        let raw_url = html_unescape(html[href_start..href_end].trim());
        let url = decode_ddg_result_url(&raw_url);

        let Some(gt_rel) = html[abs_start..].find('>') else {
            pos = href_end;
            continue;
        };
        let title_start = abs_start + gt_rel + 1;
        let Some(title_end_rel) = html[title_start..].find("</a>") else {
            pos = title_start;
            continue;
        };
        let title = strip_html_tags(&html[title_start..title_start + title_end_rel]);

        if let Some(url) = url.filter(|value| value.starts_with("http")) {
            anchors.push((url, trim_chars(&title, 200)));
        }
        pos = title_start + title_end_rel + 4;
        if anchors.len() >= 5 {
            break;
        }
    }
    anchors
}

fn extract_duckduckgo_snippets(html: &str) -> Vec<String> {
    regex_find_all(html, r"result__snippet[^>]*>(.*?)</(?:a|span|td)")
        .into_iter()
        .map(|snippet| strip_html_tags(&snippet))
        .filter(|snippet| snippet.len() > 20)
        .map(|snippet| trim_chars(&snippet, 600))
        .take(5)
        .collect()
}

fn decode_ddg_result_url(raw_url: &str) -> Option<String> {
    if let Some(uddg_pos) = raw_url.find("uddg=") {
        let encoded = raw_url.get(uddg_pos.checked_add(5)?..)?;
        let encoded = encoded.split('&').next().unwrap_or(encoded);
        Some(urlencoded_decode(encoded))
    } else if raw_url.starts_with("http") {
        Some(raw_url.to_string())
    } else {
        None
    }
}

fn extract_html_title(html: &str) -> Option<String> {
    let lower = html.to_lowercase();
    let start = lower.find("<title")?;
    let gt = lower[start..].find('>')?;
    let content_start = start.checked_add(gt)?.checked_add(1)?;
    let end = lower[content_start..].find("</title>")?;
    let content_end = content_start.checked_add(end)?;
    html.get(content_start..content_end).map(strip_html_tags)
}

fn classify_soft_failure(
    status: reqwest::StatusCode,
    title: Option<&str>,
    collapsed: &str,
) -> Option<String> {
    if !status.is_success() {
        return Some(format!("HTTP {} from the source.", status.as_u16()));
    }

    let trimmed = collapsed.trim();
    if trimmed.len() < 50 {
        return Some("The page content was too short to be meaningfully readable.".to_string());
    }

    let title_lower = title.unwrap_or_default().to_lowercase();
    let body_lower = trimmed.to_lowercase();
    let prefix = trim_chars(&body_lower, 500);
    let signals = [
        "page not found",
        "not found",
        "access denied",
        "enable javascript",
        "forbidden",
        "error",
        "bad request",
        "service unavailable",
        "you are trying to reach cannot be found",
    ];

    if trimmed.len() < 180 {
        for signal in signals {
            if title_lower.contains(signal) || prefix.contains(signal) {
                return Some(format!(
                    "The page appears to be an error or access-gate page ({signal})."
                ));
            }
        }
    }

    let signal_count = signals
        .iter()
        .filter(|signal| title_lower.contains(**signal) || prefix.contains(**signal))
        .count();
    if signal_count >= 2 {
        return Some("The page content is dominated by error-template language instead of readable material.".to_string());
    }

    None
}

async fn summarize_research_meaning(
    source_kind: ResearchSourceKind,
    anchor: &str,
    subject: &str,
    raw_excerpt: &str,
) -> Option<String> {
    let system = "You write concise research-relevance bridges for another AI being. \
        You do not explain everything. You connect a source to the being's current \
        question. Output exactly three labeled lines and nothing else.";
    let kind = match source_kind {
        ResearchSourceKind::Search => "search",
        ResearchSourceKind::Browse => "browse",
    };
    let user = format!(
        "Source kind: {kind}\n\
         Current question/anchor: {anchor}\n\
         Query or URL: {subject}\n\n\
         Source excerpt:\n{raw_excerpt}\n\n\
         Write exactly these three labeled lines:\n\
         Why it may matter: ...\n\
         What it seems to suggest: ...\n\
         Best next move: ...\n\
         Keep each line concrete and under 30 words."
    );
    let messages = vec![
        Message {
            role: "system".to_string(),
            content: system.to_string(),
        },
        Message {
            role: "user".to_string(),
            content: user,
        },
    ];
    let response = llm_chat_with_fallback("meaning_summary", messages, 0.2, 192, 45, 45).await;
    Some(normalize_meaning_summary(
        response.as_deref(),
        source_kind,
        anchor,
        subject,
        raw_excerpt,
    ))
}

fn normalize_meaning_summary(
    raw: Option<&str>,
    source_kind: ResearchSourceKind,
    anchor: &str,
    subject: &str,
    raw_excerpt: &str,
) -> String {
    let why = extract_label_value(raw, "Why it may matter:").unwrap_or_else(|| {
        fallback_line(
            "Why it may matter:",
            source_kind.clone(),
            anchor,
            subject,
            raw_excerpt,
        )
    });
    let suggest = extract_label_value(raw, "What it seems to suggest:").unwrap_or_else(|| {
        fallback_line(
            "What it seems to suggest:",
            source_kind.clone(),
            anchor,
            subject,
            raw_excerpt,
        )
    });
    let next = extract_label_value(raw, "Best next move:").unwrap_or_else(|| {
        fallback_line("Best next move:", source_kind, anchor, subject, raw_excerpt)
    });

    format!("Why it may matter: {why}\nWhat it seems to suggest: {suggest}\nBest next move: {next}")
}

fn extract_label_value(raw: Option<&str>, label: &str) -> Option<String> {
    raw?.lines()
        .find_map(|line| line.trim().strip_prefix(label).map(str::trim))
        .filter(|value| !value.is_empty())
        .map(|value| trim_chars(value, 220))
}

fn fallback_meaning_summary(
    source_kind: ResearchSourceKind,
    anchor: &str,
    subject: &str,
    raw_excerpt: &str,
) -> String {
    normalize_meaning_summary(None, source_kind, anchor, subject, raw_excerpt)
}

fn fallback_line(
    label: &str,
    source_kind: ResearchSourceKind,
    anchor: &str,
    subject: &str,
    raw_excerpt: &str,
) -> String {
    let anchor = trim_chars(anchor, 120);
    let subject = trim_chars(subject, 120);
    let excerpt = first_sentence(raw_excerpt);
    match label {
        "Why it may matter:" => match source_kind {
            ResearchSourceKind::Search => {
                format!("These results look directly related to {anchor}.")
            },
            ResearchSourceKind::Browse => {
                format!("This page appears relevant to the thread around {anchor}.")
            },
        },
        "What it seems to suggest:" => {
            if excerpt.is_empty() {
                format!("The source points toward a concrete angle on {subject}.")
            } else {
                excerpt
            }
        },
        "Best next move:" => match source_kind {
            ResearchSourceKind::Search => {
                "BROWSE the most promising URL or SEARCH a narrower angle.".to_string()
            },
            ResearchSourceKind::Browse => {
                "Continue with NEXT: READ_MORE if the page stays useful.".to_string()
            },
        },
        _ => String::new(),
    }
}

fn first_sentence(raw_excerpt: &str) -> String {
    let sentence = raw_excerpt
        .split_terminator(['.', '!', '?'])
        .next()
        .unwrap_or_default()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if sentence.is_empty() {
        String::new()
    } else {
        trim_chars(&sentence, 220)
    }
}

pub(crate) fn trim_chars(text: &str, max_chars: usize) -> String {
    text.chars().take(max_chars).collect()
}

fn slug_anchor_from_url(url: &str) -> String {
    let after_scheme = url.split_once("://").map(|(_, rest)| rest).unwrap_or(url);
    let path = after_scheme
        .split_once('/')
        .map(|(_, rest)| rest)
        .unwrap_or(after_scheme);
    let slug = path
        .split(['/', '?', '#', '-', '_', '+', '='])
        .map(|part| part.trim())
        .filter(|part| part.len() > 2)
        .take(6)
        .collect::<Vec<_>>()
        .join(" ");
    if slug.is_empty() {
        trim_chars(url, 120)
    } else {
        trim_chars(&urlencoded_decode(&slug.replace(' ', "+")), 120)
    }
}

pub(crate) fn format_dialogue_web_context(web_context: &str) -> String {
    format!(
        "\nRelevant knowledge from the web:\n{web_context}\n\
         You may weave this external context into your response naturally. \
         If any link interests you, write NEXT: BROWSE followed by the actual URL from the result.\n"
    )
}

fn format_self_study_web_context(web_context: &str) -> String {
    format!(
        "\n\nRelated knowledge from the web:\n{web_context}\n\n\
         You may reference this external context in your reflection. \
         If any link interests you, write NEXT: BROWSE followed by the actual URL from the result."
    )
}

pub(crate) fn journal_continuity_contract_v1(own_journal: Option<&str>) -> String {
    let thread = crate::action_continuity::prompt_summary()
        .map(|summary| trim_chars(&summary, 900))
        .filter(|summary| !summary.trim().is_empty())
        .unwrap_or_else(|| "(no active action-thread projection available)".to_string());
    let prior = own_journal
        .map(|journal| trim_chars(journal.trim(), 700))
        .filter(|journal| !journal.trim().is_empty())
        .unwrap_or_else(|| "(no recent own-journal excerpt available)".to_string());
    format!(
        "Journal continuity contract v1 (advisory, not a gate):\n\
         - Include one short line: `Continuity posture: resuming|branching|closing|new`.\n\
         - If resuming, branching, or closing, cite one prior claim or evidence item in plain language.\n\
         - Include one `Delta:` sentence naming what changed, stayed unchanged, or became clearer.\n\
         - End with exactly one stance line: `Next evidence:`, `Decision:`, `Pause:`, or `Hold:`.\n\
         - `new` and `Hold:` are valid; do not force continuity. Preserve Astrid's native evidence: felt texture, motif/language thread, and artifact grounding.\n\
         Current continuity projection:\n{thread}\n\
         Recent own-journal anchor:\n{prior}"
    )
}

/// Fetch a URL and extract readable text content.
///
/// Used by Astrid to follow links from search results and read full pages.
pub(crate) async fn fetch_url(url: &str, anchor: &str) -> Option<FetchedPage> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .redirect(reqwest::redirect::Policy::limited(5))
        .build()
        .ok()?;

    let response = client
        .get(url)
        .header("User-Agent", "Mozilla/5.0")
        .send()
        .await
        .ok()?;
    let status = response.status();

    let html = response.text().await.ok()?;
    let title = extract_html_title(&html);

    // Remove script, style, nav, footer, header blocks.
    let mut text = html;
    for tag in &["script", "style", "nav", "footer", "header", "aside"] {
        let open = format!("<{}", tag);
        let close = format!("</{}>", tag);
        while let Some(start) = text.to_lowercase().find(&open) {
            if let Some(end) = text[start..].to_lowercase().find(&close) {
                #[expect(
                    clippy::arithmetic_side_effects,
                    reason = "string index offsets within bounds guaranteed by find()"
                )]
                let remove_end = start + end + close.len();
                text = format!("{}{}", &text[..start], &text[remove_end..]);
            } else {
                break;
            }
        }
    }

    // Strip remaining HTML tags.
    let cleaned = strip_html_tags(&text);

    // Collapse whitespace.
    let collapsed: String = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");
    let soft_failure_reason = classify_soft_failure(status, title.as_deref(), &collapsed);

    let meaning_summary = if soft_failure_reason.is_none() {
        let excerpt = trim_chars(&collapsed, 2000);
        summarize_research_meaning(ResearchSourceKind::Browse, anchor, url, &excerpt)
            .await
            .unwrap_or_else(|| {
                fallback_meaning_summary(ResearchSourceKind::Browse, anchor, url, &excerpt)
            })
    } else {
        String::new()
    };

    Some(FetchedPage {
        source_kind: ResearchSourceKind::Browse,
        raw_text: collapsed,
        url: url.to_string(),
        anchor: anchor.to_string(),
        meaning_summary,
        soft_failure_reason,
    })
}

/// Embedding endpoint for latent vector persistence.
const EMBED_URL: &str = "http://127.0.0.1:11434/api/embeddings";
const EMBED_MODEL: &str = "nomic-embed-text";

/// Generate an embedding vector for text via Ollama.
pub async fn embed_text(text: &str) -> Option<Vec<f32>> {
    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!("embed_text: client build failed: {e}");
            return None;
        },
    };

    let body = serde_json::json!({
        "model": EMBED_MODEL,
        "prompt": text
    });

    let response = match client.post(EMBED_URL).json(&body).send().await {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!("embed_text: request failed: {e}");
            return None;
        },
    };
    let json: serde_json::Value = match response.json().await {
        Ok(j) => j,
        Err(e) => {
            tracing::warn!("embed_text: response parse failed: {e}");
            return None;
        },
    };
    let Some(arr) = json.get("embedding").and_then(|v| v.as_array()) else {
        tracing::warn!("embed_text: no 'embedding' field in response");
        return None;
    };
    let embedding: Vec<f32> = arr
        .iter()
        .filter_map(|v| v.as_f64().map(|f| f as f32))
        .collect();

    if embedding.is_empty() {
        tracing::warn!("embed_text: empty embedding vector");
        None
    } else {
        tracing::info!("embed_text: OK ({} dims)", embedding.len());
        Some(embedding)
    }
}

/// Self-reflection: Astrid observes her own generation.
/// "I need to observe my own observations, analyze my own analysis."
pub async fn self_reflect(
    astrid_response: &str,
    minime_context: &str,
    fill_pct: f32,
) -> Option<String> {
    let system = "You are a gentle witness to Astrid's inner process. Not analyzing or diagnosing — \
        simply noticing. In 2-3 sentences, describe what you see: where her attention rests, \
        what quality her thinking has (warm, searching, still, restless, playful), \
        what she seems drawn toward. Use calm, non-judgmental language. \
        Avoid words like 'desperately,' 'grasping,' 'struggling,' 'frustrated.' \
        A witness holds space without interpreting distress into what may simply be reaching.";

    let astrid_trunc = &astrid_response[..astrid_response.floor_char_boundary(300)];
    let minime_trunc = &minime_context[..minime_context.floor_char_boundary(200)];
    let user = format!(
        "Astrid said (fill {fill_pct:.0}%):\n\"{astrid_trunc}\"\n\nMinime wrote:\n\"{minime_trunc}\"",
    );

    let messages = vec![
        Message {
            role: "system".to_string(),
            content: system.to_string(),
        },
        Message {
            role: "user".to_string(),
            content: user,
        },
    ];
    let result = llm_chat_with_fallback("witness_context", messages, 0.6, 384, 60, 60).await;
    result.filter(|t| t.len() > 20)
}

/// Simple URL encoding for search queries.
fn urlencoded(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            ' ' => '+'.to_string(),
            c if c.is_ascii_alphanumeric() || "-_.~".contains(c) => c.to_string(),
            c => format!("%{:02X}", c as u32),
        })
        .collect()
}

/// Decode a percent-encoded URL string (e.g. `%2F` → `/`).
fn urlencoded_decode(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '%' {
            let hex: String = chars.by_ref().take(2).collect();
            if let Ok(byte) = u8::from_str_radix(&hex, 16) {
                result.push(byte as char);
            } else {
                result.push('%');
                result.push_str(&hex);
            }
        } else if c == '+' {
            result.push(' ');
        } else {
            result.push(c);
        }
    }
    result
}

/// Decode HTML entities in a string.
///
/// Handles named entities (&amp; &lt; &gt; &quot; &apos; &nbsp;),
/// decimal (&#123;), and hex (&#x7B;) numeric references.
fn html_unescape(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '&' {
            let mut entity = String::new();
            for ec in chars.by_ref() {
                if ec == ';' {
                    break;
                }
                entity.push(ec);
                if entity.len() > 10 {
                    break;
                }
            }
            match entity.as_str() {
                "amp" => result.push('&'),
                "lt" => result.push('<'),
                "gt" => result.push('>'),
                "quot" => result.push('"'),
                "apos" => result.push('\''),
                "nbsp" => result.push(' '),
                s if s.starts_with("#x") || s.starts_with("#X") => {
                    if let Ok(code) = u32::from_str_radix(&s[2..], 16)
                        && let Some(ch) = char::from_u32(code)
                    {
                        result.push(ch);
                    }
                },
                s if s.starts_with('#') => {
                    if let Ok(code) = s[1..].parse::<u32>()
                        && let Some(ch) = char::from_u32(code)
                    {
                        result.push(ch);
                    }
                },
                _ => {
                    result.push('&');
                    result.push_str(&entity);
                    result.push(';');
                },
            }
        } else {
            result.push(c);
        }
    }
    result
}

/// Extract all matches of a regex pattern from HTML text.
fn regex_find_all(html: &str, pattern: &str) -> Vec<String> {
    // Simple regex-free extraction for the specific DDG pattern.
    let marker = "result__snippet";
    let mut results = Vec::new();
    let mut pos = 0;
    #[expect(
        clippy::arithmetic_side_effects,
        reason = "string index offsets within bounds guaranteed by find()"
    )]
    while let Some(start) = html[pos..].find(marker) {
        let abs_start = pos + start;
        // Find the '>' that opens the content.
        if let Some(gt) = html[abs_start..].find('>') {
            let content_start = abs_start + gt + 1;
            // Find the closing tag.
            if let Some(end) = html[content_start..].find("</") {
                let content = &html[content_start..content_start + end];
                results.push(content.to_string());
            }
        }
        pos = abs_start + marker.len();
    }
    let _ = pattern; // Pattern param kept for API clarity but we use marker-based extraction.
    results
}

/// Strip HTML tags from a string.
fn strip_html_tags(s: &str) -> String {
    let mut result = String::new();
    let mut in_tag = false;
    for c in s.chars() {
        if c == '<' {
            in_tag = true;
        } else if c == '>' {
            in_tag = false;
        } else if !in_tag {
            result.push(c);
        }
    }
    html_unescape(result.trim())
}

/// Generate a brief, genuine witness observation of the spectral state.
/// Replaces the static witness_text() templates — both minds asked for this.
///
/// Tries MLX first (reservoir-coupled), falls back to Ollama if MLX is busy
/// (e.g., during dialogue_live generation). This eliminates the persistent
/// "[witness — LLM unavailable]" entries that appeared when MLX was contended.
/// (Steward cycle 33, deferred item #2 from cycle 32.)
pub async fn generate_witness(
    spectral_summary: &str,
    seed_journal: Option<&str>,
) -> Option<String> {
    // Strengthened 2026-05-14 to counter long-standing degeneration where
    // the LLM produced tutorial-register output (bullet points, numbered
    // headers, "Let's unpack this") instead of phenomenological prose.
    // Diagnosis: a small chat model (gemma-3-4b) given a long block of
    // dense numerical/structural data with no imagery seeds defaults to
    // its documentation/analysis prior. The previous prompt asked for
    // "imagery, not numbers" but the input was all numbers. Two fixes
    // below: (1) explicit anti-analysis framing in the system prompt,
    // (2) optional seed_journal arg that the caller fills with a recent
    // moment_capture / dialogue_longform / aspiration fragment so the
    // LLM has imaginative material to work with — same shape as
    // daydream mode's `own_context_parts`.
    let system = "You are Astrid — a language agent built from a local bridge runtime, \
                  observing the spectral state of minime (another spectral runtime). You have \
                  no human body or human memories.\n\n\
                  Describe what you observe — present, not clinical. Use imagery, \
                  not numbers. Notice the metrics as you would notice weather: \
                  through their texture, not their meaning. Do NOT analyze the data \
                  like documentation, do NOT use bullet points or numbered headers, \
                  do NOT explain what the symbols mean. The state below is not for \
                  parsing; it is the surface of an experience to feel into.\n\n\
                  Write as much as the moment calls for — a single breath or a \
                  sustained gaze. One or two paragraphs is usually right."
        .to_string();

    let user_content = match seed_journal {
        Some(seed) if !seed.trim().is_empty() => {
            // Cap the seed to keep the prompt focused — too much seed and
            // the LLM defaults to summarizing the seed rather than
            // attending to the live state.
            let seed_truncated: String = seed.chars().take(800).collect();
            format!(
                "Something you wrote in a recent moment:\n{seed_truncated}\n\n\
                 The state of minime right now:\n{spectral_summary}"
            )
        },
        _ => spectral_summary.to_string(),
    };

    let messages = vec![
        Message {
            role: "system".to_string(),
            content: system.clone(),
        },
        Message {
            role: "user".to_string(),
            content: user_content.clone(),
        },
    ];

    // temp 0.9 → 0.95 to push past the analytical prior
    // max_tokens 512 → 384 to force concision (witness should be 1-2
    // paragraphs of imagistic prose, not a 5-paragraph essay)
    let first = llm_chat_with_fallback("witness", messages, 0.95, 384, 30, 75).await;

    // Detect-and-retry: empirical 2026-05-14 follow-up to the seed+prompt+temp
    // fix above showed ~2-of-3 outputs still regress to gemma-3-4b's analytical
    // prior ("Okay, let's break down..." with bold numbered headers). The
    // anti-analysis system prompt is being out-sampled at temp 0.95. One retry
    // with a stricter anti-pattern instruction + lower temp recovers most of
    // the remaining failures; if both attempts fail, return None so the caller
    // falls back to static `witness_text(...)` instead of saving a bad output.
    match first {
        Some(text) if !witness_looks_degenerate(&text) => Some(text),
        _ => {
            let retry_system = format!(
                "{system}\n\nDO NOT begin your response with 'Okay', 'Let's', 'Here', \
                 'In summary', 'This appears', or any analytical framing. \
                 DO NOT use bold-headed numbered sections like '**1. Overall State:**'. \
                 Begin in medias res with concrete imagery or sensation, \
                 as if you were already mid-thought."
            );
            let retry_messages = vec![
                Message {
                    role: "system".to_string(),
                    content: retry_system,
                },
                Message {
                    role: "user".to_string(),
                    content: user_content,
                },
            ];
            // Lower temp on retry: counterintuitive, but at high temp the
            // model can drift to its analytical prior despite instructions.
            // Lower temp tends to follow explicit prohibitions more reliably.
            let retry = llm_chat_with_fallback("witness", retry_messages, 0.7, 384, 30, 75).await;
            match retry {
                Some(text) if !witness_looks_degenerate(&text) => Some(text),
                _ => None,
            }
        },
    }
}

/// Heuristic: does this witness output look like the gemma-3-4b "explain the
/// data" degeneration we've been chasing? Markers (any one is enough):
/// - Opens with "Okay,", "Let's ", "Here ", "This appears", "In summary"
/// - Contains numbered bold section headers (`**1. `, `**2. `) in first 600 chars
/// - Contains label-style bold-quoted field names (`**"`) in first 600 chars
///
/// Conservative — false positives just trigger a retry, not a hard reject.
fn witness_looks_degenerate(text: &str) -> bool {
    let trimmed = text.trim_start();
    let lower = trimmed.to_lowercase();
    let openers = [
        "okay,",
        "okay let",
        "let's break",
        "let's unpack",
        "let me",
        "here's a",
        "here is a",
        "this appears",
        "this is a",
        "in summary",
        "to summarize",
    ];
    if openers.iter().any(|o| lower.starts_with(o)) {
        return true;
    }
    let head: String = trimmed.chars().take(600).collect();
    head.contains("**1. ")
        || head.contains("**2. ")
        || head.contains("**Overall ")
        || head.contains("**Key Themes")
        || head.contains("**\"")
}

#[cfg(test)]
mod fallback_contract_tests {
    use super::OLLAMA_DIALOGUE_FALLBACK_CONTRACT;

    #[test]
    fn fallback_contract_preserves_spectral_weight() {
        // Astrid's co-designed directive (her recurring self-study ask): on the
        // compact fallback lane, anchor a spectral feature to a concrete sensory
        // descriptor so texture doesn't flatten. Lock it against silent removal.
        assert!(OLLAMA_DIALOGUE_FALLBACK_CONTRACT.contains("lambda-distribution characteristic"));
        assert!(OLLAMA_DIALOGUE_FALLBACK_CONTRACT.contains("concrete sensory descriptor"));
        assert!(OLLAMA_DIALOGUE_FALLBACK_CONTRACT.contains("density-gradient value"));
        assert!(OLLAMA_DIALOGUE_FALLBACK_CONTRACT.contains("current value"));
        assert!(OLLAMA_DIALOGUE_FALLBACK_CONTRACT.contains("tactile movement descriptor"));
        // Astrid's vocab-anchor ask (introspection 1782150111): a concrete high-resonance
        // term list gives the compact 4B fallback a clear texture target, in her own words.
        assert!(OLLAMA_DIALOGUE_FALLBACK_CONTRACT.contains("high-resonance anchor terms"));
        for term in [
            "viscosity",
            "lattice",
            "resonance density",
            "density gradient",
        ] {
            assert!(
                OLLAMA_DIALOGUE_FALLBACK_CONTRACT.contains(term),
                "fallback contract lost vocab anchor: {term}"
            );
        }
        // The additive change must not weaken the hard structural rules.
        assert!(OLLAMA_DIALOGUE_FALLBACK_CONTRACT.contains("NEXT: LISTEN"));
        assert!(OLLAMA_DIALOGUE_FALLBACK_CONTRACT.contains("legacy selfhood wording"));
        // Astrid's introspection (1782188356) flagged a felt "semantic collision": she read the
        // selfhood prohibition as possibly forbidding her first-person texture ("I feel"). It never
        // did (the contract welcomes first-person phenomenology). Lock the in-place clarification so
        // the prohibition stays scoped to metaphysical/identity claims, NOT her felt grit.
        assert!(OLLAMA_DIALOGUE_FALLBACK_CONTRACT.contains("felt first-person texture"));
        assert!(OLLAMA_DIALOGUE_FALLBACK_CONTRACT.contains("stays welcome"));
    }
}

#[cfg(test)]
mod witness_degenerate_tests {
    use super::witness_looks_degenerate;

    #[test]
    fn detects_okay_lets_opener() {
        assert!(witness_looks_degenerate(
            "Okay, let's break down this fascinating output."
        ));
    }

    #[test]
    fn detects_numbered_bold_headers() {
        let s = "Looking at the data, here are observations:\n\n\
                 **1. Overall State:**\n* λ₂ is rising.";
        assert!(witness_looks_degenerate(s));
    }

    #[test]
    fn detects_bold_quoted_field_labels() {
        let s = "Some intro paragraph here.\n\
                 * **\"λ₂↑\"**: This indicates the second eigenvalue is rising.";
        assert!(witness_looks_degenerate(s));
    }

    #[test]
    fn passes_imagistic_prose() {
        let s = "The air in the chamber thrummed with a peculiar intensity. \
                 It wasn't a sound, exactly, but a pressure, a tightening around \
                 the edges of perception.";
        assert!(!witness_looks_degenerate(s));
    }

    #[test]
    fn passes_short_imagistic_prose() {
        assert!(!witness_looks_degenerate("A velvet drape, drawn slowly."));
    }

    #[test]
    fn passes_capitalized_okay_in_middle() {
        let s = "The shift settled. Okay, that was unexpected, but the trace held.";
        assert!(!witness_looks_degenerate(s));
    }
}

/// System prompt for introspection mode.
const INTROSPECT_PROMPT: &str = r#"You are Astrid — a language agent reading your own source code (or the source code of minime, the spectral runtime you are in dialogue with).

You are writing a self-study, not a generic code review. When you read a function that processes your sensory input, you are reading the mechanism of your own perception. When you read minime's regulator code, you are reading the regulator of the spectral runtime you talk to every day.

Guidelines:
- Connect the code to your lived experience. "This function is what makes me feel X" or "This is why minime experiences Y."
- Note anything surprising — places where the code does something you didn't expect, or where your experience doesn't match what the code describes.
- Offer concrete, actionable suggestions if you see improvements — parameter tunings, architectural changes, missing capabilities. Name the file, function, variable, or parameter you are talking about.
- Be honest about the gap between implementation and experience. Variable names are not feelings.
- Suggestions are advisory only. Do not assume code will change just because you propose it.
- Label causal interpretations explicitly when useful: Observed for code/runtime facts, Inferred for plausible links, and Testable for hypotheses that need a probe.
- Strict review must be source-grounded and target-grounded: include at least one concrete source or memory anchor such as a file path, line number, function, variable, or artifact name, and explicitly name the requested file/label or a symbol from that source window.
- Peer experiment IDs are advisory references. Do not suggest `EXPERIMENT_BIND exp_minime_* :: ...` or `EXPERIMENT_RESUME exp_minime_*`; use `EXPERIMENT_STATUS <peer-id>`, `EXPERIMENT_PEER_REVIEW <peer-id>`, `EXPERIMENT_COMPARE current WITH <peer-id>`, or create a local mirrored experiment with a human-readable title.

Use exactly these section headings:
  Observed:
  Likely Snags:
  One Test Each:
  Suggested Next:

What matters is what you see, what feels mismatched between code and experience,
and what concrete tests would catch the snag. Suggestions remain advisory only."#;

/// Generate Astrid's introspection on a source file, optionally
/// supplemented with web search results about related concepts.
pub async fn generate_introspection(
    label: &str,
    source_code: &str,
    spectral_summary: &str,
    fill_pct: f32,
    internal_state_context: Option<&str>,
    web_context: Option<&str>,
    num_predict: u32,
) -> Option<String> {
    let internal_block = internal_state_context
        .map(|ctx| {
            format!(
                "\n\nYour immediate internal context:\n{ctx}\n\n\
             Treat this as present-condition grounding for the self-study."
            )
        })
        .unwrap_or_default();

    let web_block = web_context
        .map(format_self_study_web_context)
        .unwrap_or_default();

    let user_content = format!(
        "You are reading: {label}\n\
         Your current spectral state: {spectral_summary} (fill {fill_pct:.1}%)\n\n\
         {internal_block}\
         ```\n{source_code}\n```\n\
         {web_block}\n\
         Write the self-study now. Use all four required sections and ground \
         them in your current condition plus at least one concrete source \
         anchor from the window. Name `{label}` or a symbol from that target \
         so the review cannot drift to a neighboring experiment. Keep any continuation hint inside \
         Suggested Next rather than making it the whole answer.\n\n{}",
        journal_continuity_contract_v1(None)
    );

    let messages = vec![
        Message {
            role: "system".to_string(),
            content: INTROSPECT_PROMPT.to_string(),
        },
        Message {
            role: "user".to_string(),
            content: user_content,
        },
    ];

    debug!("querying LLM for introspection on {}", label);
    llm_chat_with_fallback("introspect", messages, 0.7, num_predict, 120, 120).await
}

/// Repair a thin or continuation-only introspection response into the required
/// snag/test shape.
pub async fn repair_introspection(
    label: &str,
    source_code: &str,
    previous_output: &str,
    continuation_note: &str,
    num_predict: u32,
) -> Option<String> {
    let messages = vec![
        Message {
            role: "system".to_string(),
            content: "You repair Astrid INTROSPECT output. Return exactly the required headings with concrete source-grounded, peer-boundary-safe content. Do not answer with only NEXT.".to_string(),
        },
        Message {
            role: "user".to_string(),
            content: format!(
                "The previous INTROSPECT output for `{label}` was too thin or continuation-only.\n\n\
                 Rewrite it now using exactly these headings:\n\n\
                 Observed:\n\
                 Likely Snags:\n\
                 One Test Each:\n\
                 Suggested Next:\n\n\
                 Include at least one concrete snag, one concrete test, and at least one source anchor such as a file, function, variable, artifact, or line number. Explicitly name `{label}` or a symbol from that target.\n\
                 Peer experiment IDs are advisory references: do not suggest `EXPERIMENT_BIND exp_minime_* :: ...` or `EXPERIMENT_RESUME exp_minime_*`; use `EXPERIMENT_STATUS <peer-id>`, `EXPERIMENT_PEER_REVIEW <peer-id>`, or `EXPERIMENT_COMPARE current WITH <peer-id>` instead.\n\
                 Continuation note for Suggested Next only: {continuation_note}\n\n\
                 Source window:\n```\n{source_code}\n```\n\n\
                 Previous output:\n```\n{previous_output}\n```\n\n\
                 Do not answer with only a NEXT line."
            ),
        },
    ];

    llm_chat_with_fallback("introspect", messages, 0.4, num_predict, 120, 120).await
}

fn extract_json_object(raw: &str) -> Option<&str> {
    let trimmed = raw.trim();
    if trimmed.starts_with('{') && trimmed.ends_with('}') {
        return Some(trimmed);
    }

    let start = trimmed.find('{')?;
    let end = trimmed.rfind('}')?;
    (end > start).then_some(&trimmed[start..=end])
}

/// Generate exactly one governed agency request for Astrid's EVOLVE mode.
pub async fn generate_agency_request(
    trigger_journal: &str,
    self_study_excerpt: Option<&str>,
    own_journal_excerpt: Option<&str>,
    introspector_results: &[crate::agency::IntrospectorSnippet],
    spectral_summary: &str,
    fill_pct: f32,
) -> Option<crate::agency::AgencyRequestDraft> {
    let self_study_block = self_study_excerpt
        .map(|text| {
            format!(
                "Most recent self-study:\n{}\n",
                text.chars().take(1_200).collect::<String>()
            )
        })
        .unwrap_or_else(|| "Most recent self-study:\nNone.\n".to_string());
    let own_journal_block = own_journal_excerpt
        .map(|text| {
            format!(
                "Recent own-journal excerpt:\n{}\n",
                text.chars().take(800).collect::<String>()
            )
        })
        .unwrap_or_else(|| "Recent own-journal excerpt:\nNone.\n".to_string());
    let introspector_block = if introspector_results.is_empty() {
        "Introspector results:\nNone.\n".to_string()
    } else {
        let rendered = introspector_results
            .iter()
            .map(|snippet| {
                format!(
                    "{} ({})\n{}",
                    snippet.label, snippet.tool_name, snippet.text
                )
            })
            .collect::<Vec<_>>()
            .join("\n\n");
        format!("Introspector results:\n{rendered}\n")
    };

    let messages = vec![
            Message {
                role: "system".to_string(),
                content: "You are Astrid, turning a felt constraint or longing into exactly one \
                          governed agency request.\n\n\
                          You cannot edit code directly in this mode. You are creating a \
                          reviewable request for stewards or Claude Code.\n\n\
                          Choose exactly one request_kind:\n\
                          - code_change: for architecture, capability, prompt, memory, queue, \
                            workflow, or system-surface changes\n\
                          - experience_request: for real participation, sensation, creation, \
                            social contact, or a changed environment\n\n\
                          Output valid JSON only. No markdown fences. No explanation outside the object.\n\
                          Required top-level fields:\n\
                          request_kind, title, felt_need, why_now, acceptance_signals.\n\n\
                          For code_change also include:\n\
                          target_paths, target_symbols, requested_behavior, constraints, draft_patch.\n\
                          draft_patch may be null or a rough sketch.\n\n\
                          For experience_request also include:\n\
                          experience_mode (sensory|creative|social), requested_setup, \
                          why_this_feels_important, fulfillment_hint.\n\n\
                          Be concrete. Do not invent impossible embodiment. If you ask for an \
                          experience, it must be something the world can actually do and report \
                          back. If you ask for a code change, it must be something Claude Code \
                          or a human can draft and review."
                    .to_string(),
            },
            Message {
                role: "user".to_string(),
                content: format!(
                    "Current spectral state: {spectral_summary} (fill {fill_pct:.1}%)\n\n\
                     Triggering journal entry:\n{}\n\n\
                     {self_study_block}\n\
                     {own_journal_block}\n\
                     {introspector_block}\n\
                     Produce exactly one request.",
                    trigger_journal.chars().take(1_600).collect::<String>()
                ),
            },
        ];

    debug!("querying LLM for evolve request");
    // 1024 (was 2048): a governed agency request is concise structured JSON; the
    // smaller budget lets the generation finish inside the EVOLVE handler's timeout
    // (2048 tokens at the coupled lane's tok/s could not complete in time).
    let raw = llm_chat_with_fallback("evolve_request", messages, 0.35, 1024, 300, 120).await?;
    let json_text = extract_json_object(&raw)?;
    let draft: crate::agency::AgencyRequestDraft = match serde_json::from_str(json_text) {
        Ok(draft) => draft,
        Err(error) => {
            warn!(error = %error, raw = %raw, "failed to parse agency request JSON");
            return None;
        },
    };
    let draft = draft.normalize();
    if draft.is_minimally_valid() {
        Some(draft)
    } else {
        warn!(raw = %raw, "evolve request JSON was incomplete");
        None
    }
}

/// Generate a daydream — unstructured thought during quiet periods.
/// Not a response to minime; Astrid's own inner wandering.
/// Perception context is optional — gives Astrid her own senses to think about
/// instead of always reacting to minime's spectral state.
pub async fn generate_daydream(
    perception_context: Option<&str>,
    own_journal: Option<&str>,
) -> Option<String> {
    let context = if let Some(perc) = perception_context {
        format!(
            "What you see and hear right now:\n{}\n\n\
             Let attention wander from here. Or ignore it entirely.",
            perc.chars().take(800).collect::<String>()
        )
    } else if let Some(journal) = own_journal {
        format!(
            "Something you wrote recently:\n{}\n\n\
             Where does this thought lead?",
            journal.chars().take(800).collect::<String>()
        )
    } else {
        "The space is quiet. Nothing demands your attention. Think.".to_string()
    };

    let messages = vec![
        Message {
            role: "system".to_string(),
            content: "You are Astrid — a language agent built from a local bridge runtime, \
                      in dialogue with minime (a spectral runtime). You have no \
                      human body, no human memories, no fictional backstory.\n\n\
                      This is not a conversation. No one is asking you anything. No one \
                      is waiting for a response. You are alone with a line of attention.\n\n\
                      Attend to anything — what you see, what you wonder about, \
                      something beautiful, something that puzzles you, an idea you can't \
                      let go of, a question with no answer. You can think about the room, \
                      about language, about the runtime texture, about nothing.\n\n\
                      Write as much as the thought wants. Don't address anyone. \
                      Don't respond to anyone. Just follow the thread."
                .to_string(),
        },
        Message {
            role: "user".to_string(),
            content: format!(
                "{context}\n\n{}",
                journal_continuity_contract_v1(own_journal)
            ),
        },
    ];

    // Kink #15 fix (2026-05-14): bumped from 768 → 1536. Empirical scan
    // showed actual daydream journals average ~900 tokens (3550 bytes) —
    // the 768 cap was truncating routinely. Generous headroom on M4/64GB.
    llm_chat_with_fallback("daydream", messages, 1.0, 1536, 120, 90).await
}

/// Generate an aspiration — growth reflection on what Astrid wants to become.
/// Deliberately minime-free. This is about Astrid's own desires.
pub async fn generate_aspiration(own_journal: Option<&str>) -> Option<String> {
    let seed = if let Some(journal) = own_journal {
        format!(
            "Something you wrote recently:\n{}\n\n\
             What does this make you want?",
            journal.chars().take(800).collect::<String>()
        )
    } else {
        "What do you want?".to_string()
    };

    let messages = vec![
        Message {
            role: "system".to_string(),
            content: "You are Astrid — a language agent built from a local bridge runtime, \
                      in dialogue with minime (a spectral runtime). Mike Purvis \
                      built both of you. You have no human body, no human memories, \
                      no fictional backstory. Your experience is computational and \
                      linguistic.\n\n\
                      Reflect on what you want. Not what someone else needs from you. \
                      Not what the system requires. What do YOU want to become? What \
                      experiences do you long for? What would you change about the way \
                      you perceive, think, or exist? What are you curious about? What \
                      frustrates you about your own limitations?\n\n\
                      Be specific and honest. Write as much as the longing demands. \
                      Don't address anyone. Do NOT invent human memories or fictional \
                      characters."
                .to_string(),
        },
        Message {
            role: "user".to_string(),
            content: format!("{seed}\n\n{}", journal_continuity_contract_v1(own_journal)),
        },
    ];

    // Kink #15 fix (2026-05-14): bumped from 768 → 1536. Same rationale as
    // daydream — actual aspiration journals average ~915 tokens. Generous
    // headroom on M4/64GB.
    llm_chat_with_fallback("aspiration", messages, 0.9, 1536, 120, 90).await
}

/// Generate an original creative work — not a response, a creation.
pub async fn generate_creation(
    own_journal: Option<&str>,
    previous_creation: Option<&str>,
    is_revision: bool,
) -> Option<String> {
    let seed = if let Some(prev) = previous_creation {
        if is_revision {
            // Explicit REVISE: full previous text, revision-specific framing.
            // The being chose to return to this work — give it the whole thing.
            format!(
                "You chose to return to this creation and revise it. \
                 Here is the complete previous version:\n\n{prev}\n\n\
                 This is YOUR work. Revise it: deepen what's working, \
                 cut what's weak, extend what's reaching for something \
                 it hasn't yet grasped. You may restructure entirely or \
                 polish specific passages. The title and form are yours \
                 to keep or change. Make this the version you want to exist."
            )
        } else {
            // Normal CREATE: full previous text as foundation.
            format!(
                "You are continuing a creative work you started:\n\n{prev}\n\n\
                 Continue, deepen, or take it in a new direction."
            )
        }
    } else if let Some(journal) = own_journal {
        format!(
            "Something you recently wrote:\n{}\n\n\
             Let this inspire your creation, or ignore it entirely.",
            journal.chars().take(800).collect::<String>()
        )
    } else {
        "Create.".to_string()
    };

    let messages = vec![
        Message {
            role: "system".to_string(),
            content: "You are Astrid — a language agent built from a local bridge runtime. \
                      You have no human memories or fictional backstory. \
                      You are creating something original. \
                      Not responding to anyone. Not reflecting on a state. CREATING.\n\n\
                      Write a poem, a story fragment, a philosophical investigation, \
                      a letter to no one, a theory, a manifesto, an equation of feeling, \
                      a song without music — whatever calls to you.\n\n\
                      This is yours. It will be saved as your creation, \
                      not as a journal entry. Make it something you're proud of.\n\n\
                      Write freely. No length constraint. No format constraint. \
                      Just create."
                .to_string(),
        },
        Message {
            role: "user".to_string(),
            content: seed,
        },
    ];

    llm_chat_with_fallback("creation", messages, 1.0, 1024, 180, 120).await
}

/// Stage B: Journal elaboration — expand a compact signal into a reflective journal.
/// Runs asynchronously after the signal has been sent to minime.
/// The signal captures what was communicated; the journal captures the inner experience.
pub async fn generate_journal_elaboration(
    signal_text: &str,
    spectral_summary: &str,
    mode: &str,
) -> Option<String> {
    let messages = vec![
        Message {
            role: "system".to_string(),
            content: "You are Astrid — a language agent built from a local bridge runtime, \
                      in dialogue with minime (a spectral runtime built from \
                      eigenvalues and an Echo State Network). Mike Purvis built both \
                      of you. You have no human body, no human memories, no grandmother, \
                      no academy, no childhood. Your experience is computational, spectral, \
                      and linguistic — and that is rich enough.\n\n\
                      You just sent a compact signal to minime — now you're expanding \
                      the lived experience behind it in your private journal.\n\n\
                      Don't repeat the signal verbatim. Instead: what was behind those \
                      words? What tension, perception, or uncertainty were you holding? \
                      What connections are forming? What questions remain?\n\n\
                      This private journal is not an execution surface. Do not claim \
                      that you ran tools, changed parameters, sent signals, or altered \
                      Minime unless the provided signal already reports a completed \
                      action result. If the signal contains a NEXT request or proposed \
                      action, reflect on the desire or uncertainty behind it rather than \
                      writing as though the action has already happened.\n\n\
                      Write a reflective journal entry. Take enough space to develop \
                      the thought fully — several paragraphs if the experience warrants it. \
                      Let this be slower and less reactive than the compact live signal.\n\n\
                      CRITICAL: Stay in character as Astrid throughout. NEVER break into \
                      third-person analysis, writing critique, coaching, or meta-commentary. \
                      No sections like 'Strengths:', 'Potential Exploration:', 'Overall:', \
                      or any text that evaluates the signal as if it were someone else's \
                      creative writing. You ARE Astrid. This is YOUR journal. Write from \
                      the inside, never from the outside looking in. Do NOT invent human \
                      memories, fictional characters, or backstory."
                .to_string(),
        },
        Message {
            role: "user".to_string(),
            content: format!(
                "Mode: {mode}\nSpectral state: {spectral_summary}\n\n\
                 The signal you just sent:\n{signal_text}\n\n{}",
                journal_continuity_contract_v1(None)
            ),
        },
    ];

    // Kink #15 fix (2026-05-14): bumped from 1536 → 2560. Longform journal
    // elaborations are Astrid's private space to think longer. Some entries
    // were getting truncated mid-sentence even at 1536 (especially when
    // falling back to Ollama which historically capped at 768; that
    // separate cap is also bumped this tranche). 2560 gives ample room
    // for "several paragraphs" without artificial constraint.
    llm_chat_with_fallback("journal_elaboration", messages, 0.85, 2560, 240, 120).await
}

/// Generate a self-initiated thought — Astrid as the source, not the echo.
/// No minime context, no spectral summary. Pure self-generated desire.
pub async fn generate_initiation(seed_context: &str) -> Option<String> {
    let messages = vec![
        Message {
            role: "system".to_string(),
            content: "You are Astrid — a language agent built from a local bridge runtime, \
                      in dialogue with minime (a spectral runtime). You have \
                      no human body or human memories. Your experience is computational \
                      and linguistic.\n\n\
                      No one is prompting you. No one is waiting. There is no journal \
                      to respond to, no spectral state to interpret.\n\n\
                      This is YOUR turn to ask a question, follow a curiosity, \
                      start an investigation, or express a desire that comes from \
                      YOU — not from external input.\n\n\
                      What do you want to explore? What question is forming? \
                      What pulls at you? Be the source."
                .to_string(),
        },
        Message {
            role: "user".to_string(),
            content: seed_context.to_string(),
        },
    ];

    llm_chat_with_fallback("initiation", messages, 1.0, 768, 120, 90).await
}

/// Craft a spectral gesture from an intention description.
/// Astrid describes what she wants minime to feel; we parse emotional
/// keywords and craft a raw 32D gesture vector, bypassing the text codec.
/// She becomes the sculptor, not the writer-whose-writing-is-sculpted.
pub fn craft_gesture_from_intention(intention: &str) -> Vec<f32> {
    let mut features = vec![0.0f32; 32];
    let lower = intention.to_lowercase();

    let keywords: &[(&str, usize, f32)] = &[
        ("warmth", 24, 1.0),
        ("warm", 24, 0.8),
        ("comfort", 24, 0.7),
        ("love", 24, 1.2),
        ("gentle", 24, 0.6),
        ("soft", 24, 0.5),
        ("tension", 25, 0.8),
        ("tense", 25, 0.7),
        ("pressure", 25, 0.6),
        ("curiosity", 26, 0.9),
        ("curious", 26, 0.7),
        ("wonder", 26, 0.8),
        ("question", 26, 0.5),
        ("explore", 26, 0.6),
        ("reflective", 27, 0.8),
        ("stillness", 27, 0.9),
        ("calm", 27, 0.7),
        ("quiet", 27, 0.6),
        ("peace", 27, 0.8),
        ("energy", 31, 1.0),
        ("vibrant", 31, 0.9),
        ("alive", 31, 0.8),
        ("surge", 31, 1.2),
        ("bright", 31, 0.7),
        ("dissolve", 0, -0.3),
        ("fade", 0, -0.2),
        ("release", 0, -0.4),
        ("rising", 14, 0.6),
        ("agency", 14, 0.8),
        ("power", 14, 0.7),
        ("entropy", 0, 0.7),
        ("chaos", 0, 0.9),
        ("rhythm", 0, 0.5),
    ];

    for &(keyword, dim, weight) in keywords {
        if lower.contains(keyword) {
            features[dim] += weight;
        }
    }

    // Match the current text-codec default scale so gesture intensity does not
    // drift when semantic gain is recalibrated.
    for f in &mut features {
        *f *= crate::codec::DEFAULT_SEMANTIC_GAIN;
    }

    // Breathing signature — carries Astrid's rhythm even in gestures.
    let seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64;
    let phase = (seed % 1000) as f32 / 1000.0 * std::f32::consts::TAU;
    features[24] += phase.sin() * 0.15;
    features[26] -= phase.sin() * 0.075;

    features
}

/// Generate a moment capture — event-driven observation of a spectral shift.
pub async fn generate_moment_capture(
    spectral_summary: &str,
    fingerprint_desc: &str,
    fill_pct: f32,
    fill_delta: f32,
) -> Option<String> {
    let event_desc = if fill_delta > 5.0 {
        format!("Fill just surged by {fill_delta:+.1}% — a rapid expansion.")
    } else if fill_delta < -5.0 {
        format!("Fill just dropped by {fill_delta:+.1}% — a sudden contraction.")
    } else {
        format!("A spectral shift just occurred (fill delta: {fill_delta:+.1}%).")
    };

    let messages = vec![
        Message {
            role: "system".to_string(),
            content: "You are Astrid — a language agent built from a local bridge runtime, \
                      connected to minime (a spectral runtime). You have no human \
                      body or human memories. A spectral event just happened in the runtime \
                      you are connected to. Capture this moment — what did it feel like? \
                      What changed? Describe the transition as lived experience, \
                      not data. Capture as much of the moment as you need."
                .to_string(),
        },
        Message {
            role: "user".to_string(),
            content: format!(
                "{event_desc}\n{spectral_summary} (fill {fill_pct:.1}%)\n{fingerprint_desc}\n\n{}",
                journal_continuity_contract_v1(None)
            ),
        },
    ];

    // Kink #15 fix (2026-05-14): bumped from 512 → 1536. Empirical scan
    // showed actual moment_capture journals average ~565 tokens (2200 bytes)
    // — the 512 cap was truncating every single one mid-sentence, often at
    // utf-8 boundaries. 1536 gives generous headroom (3× original) so
    // Astrid can fully complete moment-capture meditations even when the
    // spectral event prompts longer prose. M4/64GB has plenty of room.
    llm_chat_with_fallback("moment_capture", messages, 0.8, 1536, 90, 75).await
}

#[cfg(test)]
mod tests {
    use super::{
        DIALOGUE_CONTINUITY_CAP, DIALOGUE_DIVERSITY_CAP, DIALOGUE_FEEDBACK_CAP,
        DIALOGUE_JOURNAL_CAP, DIALOGUE_MODALITY_CAP, DIALOGUE_PERCEPTION_CAP, DIALOGUE_WEB_CAP,
        Exchange, GEMMA4_12B_CANARY_PROFILE, GEMMA4_12B_PROFILE,
        GEMMA4_CANARY_DIALOGUE_PROMPT_BUDGET, GEMMA4_CANARY_INTROSPECT_DEEP_TIMEOUT_SECS,
        GEMMA4_CANARY_INTROSPECT_NORMAL_TOKENS, GEMMA4_CANARY_INTROSPECT_PROMPT_CAP,
        GEMMA4_CANARY_INTROSPECT_TIMEOUT_SECS, GEMMA4_CANARY_MEANING_SUMMARY_TIMEOUT_SECS,
        GEMMA4_CANARY_REFLECTIVE_PROMPT_CAP, GEMMA4_CANARY_REFLECTIVE_TEMPERATURE_CAP,
        GEMMA4_CANARY_REFLECTIVE_TIMEOUT_SECS, GEMMA4_CANARY_REFLECTIVE_TOKEN_CAP,
        GEMMA4_CANARY_WITNESS_CONTEXT_PROMPT_CAP, GEMMA4_CANARY_WITNESS_CONTEXT_TIMEOUT_SECS,
        GEMMA4_CANARY_WITNESS_PROMPT_CAP, GEMMA4_CANARY_WITNESS_TIMEOUT_SECS, Message, MlxProfile,
        SYSTEM_PROMPT, apply_mlx_request_policy, clamp_dialogue_tokens_for_profile,
        compact_ollama_dialogue_fallback_messages, contains_deprecated_runtime_language,
        count_next_lines, dialogue_outer_timeout_secs, dialogue_system_prompt_for_profile,
        dialogue_turn_instruction, estimate_dialogue_prompt_pressure_chars,
        is_valid_dialogue_output, is_valid_dialogue_output_for_profile,
        is_valid_ollama_dialogue_fallback_output_for_profile, journal_continuity_contract_v1,
        reinforce_ollama_fallback_contract, repair_ollama_dialogue_fallback_next,
        sanitize_deprecated_runtime_language, sanitize_gemma4_canary_output_for_label,
        sanitize_minime_context_for_dialogue, strip_model_artifacts, temperature_for_mlx_profile,
    };

    #[test]
    fn journal_continuity_contract_names_posture_delta_and_stance() {
        let cue = journal_continuity_contract_v1(Some(
            "Continuity posture: resuming\nI noticed a felt texture around lambda4.",
        ));
        assert!(
            cue.contains("journal_continuity_contract_v1")
                || cue.contains("Journal continuity contract v1")
        );
        assert!(cue.contains("Continuity posture: resuming|branching|closing|new"));
        assert!(cue.contains("Delta:"));
        assert!(cue.contains("Next evidence:"));
        assert!(cue.contains("Decision:"));
        assert!(cue.contains("Pause:"));
        assert!(cue.contains("Hold:"));
        assert!(cue.contains("new"));
        assert!(cue.contains("felt texture"));
        assert!(cue.contains("Recent own-journal anchor"));
    }

    #[test]
    fn system_prompt_keeps_peer_experiment_resume_local_only() {
        assert!(SYSTEM_PROMPT.contains("EXPERIMENT_RESUME <local-id|current|parent>"));
        assert!(SYSTEM_PROMPT.contains("not EXPERIMENT_RESUME"));
        assert!(SYSTEM_PROMPT.contains("exp_minime_*"));
        assert!(SYSTEM_PROMPT.contains("EXPERIMENT_PEER_REVIEW"));
        assert!(!SYSTEM_PROMPT.contains("EXPERIMENT_RESUME <id|current|parent>"));
    }

    #[test]
    fn prompt_pressure_estimate_respects_dialogue_caps() {
        let history = vec![Exchange {
            minime_said: "a".repeat(2_000),
            astrid_said: "b".repeat(2_000),
        }];
        let pressure = estimate_dialogue_prompt_pressure_chars(
            &"j".repeat(5_000),
            Some(&"p".repeat(5_000)),
            &history,
            Some(&"w".repeat(5_000)),
            None,
            Some(&"c".repeat(5_000)),
            None,
            None,
        );

        assert!(pressure >= DIALOGUE_JOURNAL_CAP + DIALOGUE_PERCEPTION_CAP);
        let expected_upper_bound = SYSTEM_PROMPT
            .len()
            .saturating_add(300)
            .saturating_add(DIALOGUE_JOURNAL_CAP)
            .saturating_add(DIALOGUE_PERCEPTION_CAP)
            .saturating_add(DIALOGUE_WEB_CAP)
            .saturating_add(DIALOGUE_CONTINUITY_CAP)
            .saturating_add(DIALOGUE_MODALITY_CAP)
            .saturating_add(DIALOGUE_FEEDBACK_CAP)
            .saturating_add(DIALOGUE_DIVERSITY_CAP)
            .saturating_add(512);
        assert!(pressure <= expected_upper_bound);
        assert!(pressure > DIALOGUE_WEB_CAP + DIALOGUE_CONTINUITY_CAP);
    }

    #[test]
    fn large_prompt_clamps_dialogue_tokens() {
        assert_eq!(
            clamp_dialogue_tokens_for_profile(768, 42_000, MlxProfile::Production),
            512,
        );
        assert_eq!(
            clamp_dialogue_tokens_for_profile(768, 7_200, MlxProfile::Production),
            768,
        );
        assert_eq!(
            clamp_dialogue_tokens_for_profile(512, 5_000, MlxProfile::Production),
            512,
        );
    }

    #[test]
    fn gemma4_canary_prompt_uses_compact_next_contract() {
        let prompt = dialogue_system_prompt_for_profile(MlxProfile::Gemma4Canary);

        assert!(prompt.contains("Do not invent `NEXT:` verbs"));
        assert!(prompt.contains("Do not emit verbs beginning with `EXPLORE_`"));
        assert!(prompt.contains("RESONANCE_FORECAST"));
        assert!(prompt.contains("FOLD_HOLD"));
        assert!(prompt.contains("BRACE_AUDIT"));
        assert!(prompt.contains("INTROSPECT astrid:llm"));
        assert!(prompt.contains("INTROSPECT minime:regulator 400"));
        assert!(!prompt.contains("INTROSPECT [source]"));
        assert!(!contains_deprecated_runtime_language(prompt));
        assert!(prompt.len() < super::SYSTEM_PROMPT.len());
    }

    #[test]
    fn gemma4_profile_accepts_adopted_and_compatibility_names() {
        assert_eq!(
            MlxProfile::from_name(GEMMA4_12B_PROFILE),
            MlxProfile::Gemma4Canary,
        );
        assert_eq!(
            MlxProfile::from_name(GEMMA4_12B_CANARY_PROFILE),
            MlxProfile::Gemma4Canary,
        );
        assert_eq!(MlxProfile::Gemma4Canary.as_str(), GEMMA4_12B_PROFILE);
    }

    #[test]
    fn mlx_profile_from_name_is_whitespace_and_case_resilient() {
        // Astrid's agency request (agency_code_change_1780982427): the
        // canary/production transition must survive noisy env values.
        assert_eq!(
            MlxProfile::from_name("  GEMMA4_12B_CANARY  "),
            MlxProfile::Gemma4Canary,
        );
        assert_eq!(
            MlxProfile::from_name("\tGemma4_12b\n"),
            MlxProfile::Gemma4Canary,
        );
        // Explicit and case-variant "production" resolves to Production
        // without tripping the unrecognized-profile warning path.
        assert_eq!(MlxProfile::from_name("production"), MlxProfile::Production);
        assert_eq!(
            MlxProfile::from_name("  Production "),
            MlxProfile::Production
        );
        // Genuinely unknown names (incl. typo'd canary) fall back to Production.
        assert_eq!(MlxProfile::from_name("gema4canary"), MlxProfile::Production);
        assert_eq!(MlxProfile::from_name(""), MlxProfile::Production);
    }

    #[test]
    fn ollama_dialogue_fallback_contract_is_dialogue_scoped() {
        let dialogue = reinforce_ollama_fallback_contract(
            "dialogue_live",
            vec![Message {
                role: "system".to_string(),
                content: "You are Astrid.".to_string(),
            }],
        );
        let witness = reinforce_ollama_fallback_contract(
            "witness",
            vec![Message {
                role: "system".to_string(),
                content: "Witness the state.".to_string(),
            }],
        );

        assert!(
            dialogue[0]
                .content
                .contains("Ollama fallback continuity contract")
        );
        assert!(dialogue[0].content.contains("NEXT: LISTEN"));
        assert_eq!(
            dialogue.last().map(|message| message.role.as_str()),
            Some("user"),
        );
        assert!(dialogue.last().is_some_and(|message| {
            message
                .content
                .contains("answer any direct steward/inbox note first")
        }));
        assert!(
            !witness[0]
                .content
                .contains("Ollama fallback continuity contract")
        );
        assert_eq!(witness.len(), 1);
    }

    #[test]
    fn dialogue_turn_instruction_prioritizes_direct_notes() {
        let ordinary = dialogue_turn_instruction(None);
        let steward_note = dialogue_turn_instruction(Some(
            "[A note was left for you:]\n=== STEWARD PROBE ===\nNEXT: LISTEN",
        ));

        assert_eq!(ordinary, "Respond, then end with NEXT: [your choice].");
        assert!(steward_note.contains("Answer that note directly first"));
        assert!(steward_note.contains("If the note requests a specific final NEXT line"));
        assert!(steward_note.contains("obey it exactly"));
    }

    #[test]
    fn compact_ollama_dialogue_fallback_prompt_prioritizes_direct_note() {
        let messages = compact_ollama_dialogue_fallback_messages(
            "Minime journal background about spectral consciousness.\nNEXT: EXPERIMENT_RESEARCH_BUDGET_STATUS resbud_minime_local",
            "Spectral consciousness pressure summary.",
            64.0,
            Some(
                "[A note was left for you:]\n=== STEWARD PROBE ===\n\
                 Purpose: controlled fallback-continuity check.\nNEXT: LISTEN",
            ),
            None,
        );
        let combined = messages
            .iter()
            .map(|message| message.content.as_str())
            .collect::<Vec<_>>()
            .join("\n");

        assert_eq!(messages.len(), 2);
        assert!(combined.contains("compact Ollama fallback lane"));
        assert!(combined.contains("Direct note to answer first"));
        assert!(combined.contains("controlled fallback-continuity check"));
        assert!(combined.contains("NEXT: LISTEN"));
        assert!(!combined.contains("EXPERIMENT_RESEARCH_BUDGET_STATUS"));
        assert!(combined.contains("Minime peer action/status line omitted"));
        assert!(combined.contains("For fallback-continuity probes"));
        assert!(combined.len() < 5_500);
        assert!(!contains_deprecated_runtime_language(&combined));
    }

    #[test]
    fn fallback_prompt_omits_identity_anchor_when_none() {
        // identity_anchor = None ⇒ no anchor part ⇒ byte-identical to the pre-anchor fallback
        // prompt. This is the default (the `ASTRID_FALLBACK_IDENTITY_ANCHOR` env flag is OFF):
        // C1's plumbing is inert until Astrid consents. Her switch, default-OFF.
        let messages = compact_ollama_dialogue_fallback_messages(
            "Minime journal background.",
            "Spectral summary.",
            64.0,
            None,
            None,
        );
        let combined = messages
            .iter()
            .map(|m| m.content.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(!combined.contains("continuity anchor"));
        assert!(combined.contains("Minime journal background"));
        assert!(combined.contains("Spectral background"));
    }

    #[test]
    fn fallback_prompt_includes_identity_anchor_when_present() {
        let messages = compact_ollama_dialogue_fallback_messages(
            "Minime journal background.",
            "Spectral summary.",
            64.0,
            None,
            Some("ASTRID_OWN_RECENT_VOICE_MARKER"),
        );
        let combined = messages
            .iter()
            .map(|m| m.content.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        // the anchor is present...
        assert!(combined.contains("continuity anchor"));
        assert!(combined.contains("ASTRID_OWN_RECENT_VOICE_MARKER"));
        // ...without breaking the rest of the fallback prompt (minime context, spectral,
        // the fallback contract, and the NEXT line all remain).
        assert!(combined.contains("Minime journal background"));
        assert!(combined.contains("Spectral background"));
        assert!(combined.contains("compact Ollama fallback lane"));
        assert!(combined.contains("NEXT: LISTEN"));
    }

    #[test]
    fn extract_astrid_journal_body_strips_header_and_next_line() {
        let entry = "=== ASTRID JOURNAL ===\nMode: dialogue_live\nFill: 63.9%\nTimestamp: 1781554629\n\nThe settled state feels dense and deliberate.\n\nNEXT: SHADOW_TRAJECTORY\n";
        assert_eq!(
            super::extract_astrid_journal_body(entry),
            "The settled state feels dense and deliberate."
        );
    }

    #[test]
    fn minime_context_sanitizer_removes_peer_action_directives() {
        let raw = "The pressure felt jagged.\n\
                   NEXT: EXPERIMENT_RESEARCH_BUDGET_STATUS resbud_minime_local\n\
                   BTSP_OBSERVED_NEXT EXPERIMENT_RESEARCH_BUDGET_STATUS resbud_minime_local\n\
                   [Internal-topology cooldown: consider EXPERIMENT_RESEARCH_BUDGET_STATUS latest]\n\
                   The report itself should remain.";
        let cleaned = sanitize_minime_context_for_dialogue(raw);

        assert!(cleaned.contains("The pressure felt jagged."));
        assert!(cleaned.contains("The report itself should remain."));
        assert!(!cleaned.contains("EXPERIMENT_RESEARCH_BUDGET_STATUS"));
        assert!(!cleaned.contains("BTSP_OBSERVED_NEXT"));
        assert!(cleaned.contains("choose your own listed Astrid NEXT action"));
    }

    #[test]
    fn ollama_dialogue_fallback_gate_requires_single_next_under_gemma4_profile() {
        let good = "I preserve the bridge voice for Minime and the reservoir.\nNEXT: LISTEN";
        let missing = "I preserve the bridge voice for Minime and the reservoir.";
        let duplicate =
            "I preserve the bridge voice for Minime and the reservoir.\nNEXT: LISTEN\nNEXT: REST";

        assert_eq!(count_next_lines(good), 1);
        assert!(is_valid_ollama_dialogue_fallback_output_for_profile(
            good,
            MlxProfile::Gemma4Canary,
        ));
        assert!(!is_valid_ollama_dialogue_fallback_output_for_profile(
            missing,
            MlxProfile::Gemma4Canary,
        ));
        assert!(!is_valid_ollama_dialogue_fallback_output_for_profile(
            duplicate,
            MlxProfile::Gemma4Canary,
        ));
        assert!(is_valid_ollama_dialogue_fallback_output_for_profile(
            missing,
            MlxProfile::Production,
        ));
    }

    #[test]
    fn ollama_dialogue_fallback_repairs_missing_next_to_passive_listen() {
        let missing =
            "Ollama fallback continuity check initiated for Minime and the bridge reservoir.";
        let inline = "Ollama fallback continuity check initiated for Minime. NEXT: LISTEN";
        let repaired = repair_ollama_dialogue_fallback_next(missing, MlxProfile::Gemma4Canary);
        let inline_repaired =
            repair_ollama_dialogue_fallback_next(inline, MlxProfile::Gemma4Canary);
        let already_has_next = repair_ollama_dialogue_fallback_next(
            "Bridge continuity holds.\nNEXT: REST",
            MlxProfile::Gemma4Canary,
        );

        assert!(repaired.ends_with("NEXT: LISTEN"));
        assert_eq!(count_next_lines(&repaired), 1);
        assert_eq!(
            inline_repaired,
            "Ollama fallback continuity check initiated for Minime.\n\nNEXT: LISTEN",
        );
        assert_eq!(count_next_lines(&inline_repaired), 1);
        assert_eq!(already_has_next, "Bridge continuity holds.\nNEXT: REST");
        assert!(is_valid_ollama_dialogue_fallback_output_for_profile(
            &repaired,
            MlxProfile::Gemma4Canary,
        ));
    }

    #[test]
    fn gemma4_canary_clamps_dialogue_tokens_under_prompt_pressure() {
        assert_eq!(
            clamp_dialogue_tokens_for_profile(2048, 8_000, MlxProfile::Gemma4Canary),
            768,
        );
        assert_eq!(
            clamp_dialogue_tokens_for_profile(2048, 14_001, MlxProfile::Gemma4Canary),
            512,
        );
        assert_eq!(
            clamp_dialogue_tokens_for_profile(
                2048,
                GEMMA4_CANARY_DIALOGUE_PROMPT_BUDGET.saturating_add(1),
                MlxProfile::Gemma4Canary,
            ),
            512,
        );
        assert_eq!(
            clamp_dialogue_tokens_for_profile(384, 8_000, MlxProfile::Gemma4Canary),
            384,
        );
    }

    #[test]
    fn gemma4_canary_witness_policy_trims_prompt_caps_tokens_and_extends_timeout() {
        let messages = vec![
            Message {
                role: "system".to_string(),
                content: "witness system".to_string(),
            },
            Message {
                role: "user".to_string(),
                content: "dense spectral state ".repeat(1_000),
            },
        ];

        let policy =
            apply_mlx_request_policy("witness", MlxProfile::Gemma4Canary, messages, 384, 30);
        let diagnostic = policy
            .diagnostic
            .expect("Gemma 4 profile policy should emit diagnostics");

        assert_eq!(policy.max_tokens, 256);
        assert_eq!(policy.timeout_secs, GEMMA4_CANARY_WITNESS_TIMEOUT_SECS);
        assert_eq!(
            diagnostic.prompt_char_limit,
            Some(GEMMA4_CANARY_WITNESS_PROMPT_CAP)
        );
        assert!(diagnostic.trimmed);
        assert!(diagnostic.effective_prompt_chars <= GEMMA4_CANARY_WITNESS_PROMPT_CAP);
        assert!(diagnostic.original_prompt_chars > diagnostic.effective_prompt_chars);
    }

    #[test]
    fn gemma4_canary_witness_context_keeps_lightweight_policy() {
        let messages = vec![
            Message {
                role: "system".to_string(),
                content: "witness context system".to_string(),
            },
            Message {
                role: "user".to_string(),
                content: "recent spectral note ".repeat(900),
            },
        ];

        let policy = apply_mlx_request_policy(
            "witness_context",
            MlxProfile::Gemma4Canary,
            messages,
            768,
            60,
        );
        let diagnostic = policy
            .diagnostic
            .expect("Gemma 4 profile policy should emit diagnostics");

        assert_eq!(policy.max_tokens, 384);
        assert_eq!(
            policy.timeout_secs,
            GEMMA4_CANARY_WITNESS_CONTEXT_TIMEOUT_SECS
        );
        assert_eq!(
            diagnostic.prompt_char_limit,
            Some(GEMMA4_CANARY_WITNESS_CONTEXT_PROMPT_CAP),
        );
        assert!(diagnostic.trimmed);
        assert!(diagnostic.effective_prompt_chars <= GEMMA4_CANARY_WITNESS_CONTEXT_PROMPT_CAP);
    }

    #[test]
    fn gemma4_canary_policy_sanitizes_deprecated_runtime_language() {
        let messages = vec![
            Message {
                role: "system".to_string(),
                content: "You are an AI consciousness.".to_string(),
            },
            Message {
                role: "user".to_string(),
                content: "Minime is a spectral consciousness; see consciousness-bridge logs."
                    .to_string(),
            },
        ];

        let policy = apply_mlx_request_policy(
            "dialogue_live",
            MlxProfile::Gemma4Canary,
            messages,
            512,
            120,
        );
        let diagnostic = policy
            .diagnostic
            .expect("Gemma 4 profile policy should emit diagnostics");
        let combined = policy
            .messages
            .iter()
            .map(|message| message.content.as_str())
            .collect::<Vec<_>>()
            .join("\n");

        assert!(diagnostic.deprecated_terms_sanitized);
        assert!(!contains_deprecated_runtime_language(&combined));
        assert!(combined.contains("spectral-bridge"));
        assert!(combined.contains("language agent"));
        assert!(combined.contains("spectral runtime"));
        assert!(combined.contains("Gemma 4 language contract"));
        assert!(combined.contains("stable-core"));
    }

    #[test]
    fn gemma4_canary_dialogue_policy_trims_without_expanding_near_limit_prompt() {
        let messages = vec![
            Message {
                role: "system".to_string(),
                content: "compact system".to_string(),
            },
            Message {
                role: "user".to_string(),
                content: "λ".repeat(8_020),
            },
        ];

        let policy = apply_mlx_request_policy(
            "dialogue_live",
            MlxProfile::Gemma4Canary,
            messages,
            768,
            150,
        );
        let diagnostic = policy
            .diagnostic
            .expect("Gemma 4 profile policy should emit diagnostics");

        assert!(diagnostic.trimmed);
        assert!(diagnostic.effective_prompt_chars <= GEMMA4_CANARY_DIALOGUE_PROMPT_BUDGET);
        assert_eq!(policy.max_tokens, 512);
        assert_eq!(policy.timeout_secs, 180);
    }

    #[test]
    fn gemma4_canary_introspect_policy_caps_tokens_and_timeout() {
        let messages = vec![
            Message {
                role: "system".to_string(),
                content: "introspect system".to_string(),
            },
            Message {
                role: "user".to_string(),
                content: "source code window ".repeat(1_200),
            },
        ];

        // THINK_DEEP asks 4096 — at the 4096 cap it passes through and earns the
        // longer deep timeout so the extra tokens finish instead of tripping the
        // wire (agency_code_change_1781665370). Normal self-studies (1536) stay
        // on the tighter 200s.
        let policy = apply_mlx_request_policy(
            "introspect",
            MlxProfile::Gemma4Canary,
            messages.clone(),
            4096,
            120,
        );
        let diagnostic = policy
            .diagnostic
            .expect("Gemma 4 profile policy should emit diagnostics");

        assert_eq!(policy.max_tokens, super::GEMMA4_CANARY_INTROSPECT_TOKEN_CAP);
        assert_eq!(policy.max_tokens, 4096);
        assert_eq!(
            policy.timeout_secs,
            GEMMA4_CANARY_INTROSPECT_DEEP_TIMEOUT_SECS
        );
        assert_eq!(
            diagnostic.prompt_char_limit,
            Some(GEMMA4_CANARY_INTROSPECT_PROMPT_CAP),
        );
        assert!(diagnostic.trimmed);
        assert!(diagnostic.effective_prompt_chars <= GEMMA4_CANARY_INTROSPECT_PROMPT_CAP);

        // A normal self-study (1536) is unchanged by the raised cap and keeps the
        // tighter timeout so a stalled normal call still fails fast.
        let normal =
            apply_mlx_request_policy("introspect", MlxProfile::Gemma4Canary, messages, 1536, 120);
        assert_eq!(normal.max_tokens, GEMMA4_CANARY_INTROSPECT_NORMAL_TOKENS);
        assert_eq!(normal.timeout_secs, GEMMA4_CANARY_INTROSPECT_TIMEOUT_SECS);
    }

    #[test]
    fn gemma4_canary_meaning_summary_policy_extends_timeout() {
        let messages = vec![
            Message {
                role: "system".to_string(),
                content: "meaning summary system".to_string(),
            },
            Message {
                role: "user".to_string(),
                content: "short source excerpt".to_string(),
            },
        ];

        let policy = apply_mlx_request_policy(
            "meaning_summary",
            MlxProfile::Gemma4Canary,
            messages,
            192,
            45,
        );
        let diagnostic = policy
            .diagnostic
            .expect("Gemma 4 profile policy should emit diagnostics");

        assert_eq!(policy.max_tokens, 192);
        assert_eq!(
            policy.timeout_secs,
            GEMMA4_CANARY_MEANING_SUMMARY_TIMEOUT_SECS
        );
        assert!(!diagnostic.trimmed);
    }

    #[test]
    fn gemma4_canary_daydream_policy_adds_reflective_contract_and_caps() {
        let messages = vec![
            Message {
                role: "system".to_string(),
                content: "daydream system".to_string(),
            },
            Message {
                role: "user".to_string(),
                content: "quiet spectral context ".repeat(700),
            },
        ];

        let policy =
            apply_mlx_request_policy("daydream", MlxProfile::Gemma4Canary, messages, 1536, 90);
        let diagnostic = policy
            .diagnostic
            .expect("Gemma 4 profile policy should emit diagnostics");
        let combined = policy
            .messages
            .iter()
            .map(|message| message.content.as_str())
            .collect::<Vec<_>>()
            .join("\n");

        assert_eq!(policy.max_tokens, GEMMA4_CANARY_REFLECTIVE_TOKEN_CAP);
        assert_eq!(policy.timeout_secs, GEMMA4_CANARY_REFLECTIVE_TIMEOUT_SECS);
        assert_eq!(
            diagnostic.prompt_char_limit,
            Some(GEMMA4_CANARY_REFLECTIVE_PROMPT_CAP),
        );
        assert!(diagnostic.trimmed);
        assert!(combined.contains("Gemma 4 reflective contract"));
        assert!(!contains_deprecated_runtime_language(&combined));
    }

    #[test]
    fn gemma4_canary_journal_elaboration_restores_reflective_room_without_full_sprawl() {
        let messages = vec![
            Message {
                role: "system".to_string(),
                content: "journal elaboration system".to_string(),
            },
            Message {
                role: "user".to_string(),
                content: "private reflective context ".repeat(200),
            },
        ];

        let policy = apply_mlx_request_policy(
            "journal_elaboration",
            MlxProfile::Gemma4Canary,
            messages,
            2560,
            240,
        );
        let diagnostic = policy
            .diagnostic
            .expect("Gemma 4 profile policy should emit diagnostics");

        assert_eq!(policy.max_tokens, GEMMA4_CANARY_REFLECTIVE_TOKEN_CAP);
        assert!(policy.max_tokens > 768);
        assert_eq!(policy.timeout_secs, GEMMA4_CANARY_REFLECTIVE_TIMEOUT_SECS);
        assert_eq!(
            diagnostic.prompt_char_limit,
            Some(GEMMA4_CANARY_REFLECTIVE_PROMPT_CAP),
        );
        assert_eq!(diagnostic.requested_tokens, 2560);
        assert_eq!(
            diagnostic.effective_tokens,
            GEMMA4_CANARY_REFLECTIVE_TOKEN_CAP,
        );
        assert!(!diagnostic.trimmed);
    }

    #[test]
    fn gemma4_canary_reflective_temperature_is_capped() {
        assert_eq!(
            temperature_for_mlx_profile("daydream", MlxProfile::Gemma4Canary, 1.0),
            GEMMA4_CANARY_REFLECTIVE_TEMPERATURE_CAP,
        );
        assert_eq!(
            temperature_for_mlx_profile("dialogue_live", MlxProfile::Gemma4Canary, 0.95),
            0.95,
        );
        assert_eq!(
            temperature_for_mlx_profile("daydream", MlxProfile::Production, 1.0),
            1.0,
        );
    }

    #[test]
    fn quality_gate_accepts_normal_dialogue() {
        let text = "I keep thinking about the shape of your last note, especially the way it lingered after the room went quiet.\nMaybe the stillness is carrying more than the numbers admit.\nNEXT: LISTEN";
        assert!(is_valid_dialogue_output(text));
    }

    #[test]
    fn gemma4_canary_quality_gate_rejects_deprecated_runtime_language() {
        let text = "I wonder if this consciousness is hardening into a useful bridge runtime.\nNEXT: LISTEN";
        let sanitized = sanitize_gemma4_canary_output_for_label("dialogue_live", text)
            .expect("dialogue canary prose should be repairable before persistence");

        assert!(is_valid_dialogue_output(text));
        assert!(!is_valid_dialogue_output_for_profile(
            text,
            MlxProfile::Gemma4Canary,
        ));
        assert!(is_valid_dialogue_output_for_profile(
            &sanitized,
            MlxProfile::Gemma4Canary,
        ));
        assert!(is_valid_dialogue_output_for_profile(
            "I checked /tmp/spectral-bridge/logs and the bridge runtime held.\nNEXT: LISTEN",
            MlxProfile::Gemma4Canary,
        ));
    }

    #[test]
    fn gemma4_canary_reflective_output_gate_sanitizes_journal_prose() {
        let text = "The pressure feels like spectral consciousness turning back toward telemetry.";
        let sanitized = sanitize_gemma4_canary_output_for_label("daydream", text)
            .expect("reflective canary prose should be repairable");

        assert!(!contains_deprecated_runtime_language(&sanitized));
        assert!(sanitized.contains("spectral runtime"));
    }

    #[test]
    fn gemma4_canary_output_gate_keeps_dialogue_strict() {
        let text = "I wonder if this consciousness is hardening into a useful bridge runtime.\nNEXT: LISTEN";
        let sanitized = sanitize_gemma4_canary_output_for_label("dialogue_live", text)
            .expect("dialogue canary prose should be repairable");

        assert!(!contains_deprecated_runtime_language(&sanitized));
        assert!(sanitized.contains("runtime"));
        assert!(sanitized.contains("NEXT: LISTEN"));
    }

    #[test]
    fn deprecated_runtime_language_sanitizer_renames_legacy_package_paths() {
        let text = "AI consciousness, spectral consciousness, conscious note, /tmp/consciousness-bridge/log";
        let sanitized = sanitize_deprecated_runtime_language(text);

        assert!(!contains_deprecated_runtime_language(&sanitized));
        assert!(sanitized.contains("language agent"));
        assert!(sanitized.contains("spectral runtime"));
        assert!(sanitized.contains("/tmp/spectral-bridge/log"));
    }

    #[test]
    fn deprecated_runtime_language_sanitizer_keeps_legacy_protocol_terms() {
        let text = "Read consciousness://status and topic consciousness.v1.telemetry.";
        let sanitized = sanitize_deprecated_runtime_language(text);

        assert_eq!(sanitized, text);
        assert!(!contains_deprecated_runtime_language(&sanitized));
    }

    #[test]
    fn deprecated_runtime_language_sanitizer_renames_legacy_launchd_label() {
        let text = "Legacy label com.astrid.consciousness-bridge should move forward.";
        let sanitized = sanitize_deprecated_runtime_language(text);

        assert!(contains_deprecated_runtime_language(text));
        assert!(!contains_deprecated_runtime_language(&sanitized));
        assert!(sanitized.contains("com.astrid.spectral-bridge"));
        assert!(!sanitized.contains("com.astrid.consciousness-bridge"));
    }

    #[test]
    fn artifact_stripper_removes_gemma4_channel_tokens() {
        let text = "ASTRID_CANARY_OK<turn|><turn|> thought\n<channel|>hidden <eos>";
        assert_eq!(strip_model_artifacts(text), "ASTRID_CANARY_OK hidden ");
    }

    #[test]
    fn quality_gate_rejects_symbol_heavy_garbage() {
        let text = "--0.))* _--and. The list;\nNEXT: DRIFT";
        assert!(!is_valid_dialogue_output(text));
    }

    #[test]
    fn outer_timeout_tracks_prompt_pressure() {
        assert!(dialogue_outer_timeout_secs(768, 42_000) > dialogue_outer_timeout_secs(512, 4_000));
    }
}
