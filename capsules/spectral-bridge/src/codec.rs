//! Spectral codec: translates between text and sensory features.
//!
//! The codec maps text into minime's 48-dimensional semantic lane
//! and interprets spectral telemetry as natural language.
//!
//! Dim layout:
//!   0-7:   Character-level statistics (entropy, density, rhythm)
//!   8-15:  Word-level features (lexical diversity, hedging, certainty)
//!   16-23: Sentence-level structure (length variance, question density)
//!   24-31: Emotional/intentional markers (warmth, tension, curiosity)
//!   32-39: Embedding-projected semantic features (nomic-embed-text → 8D)
//!   40-43: Narrative arc (semantic shift from first half to second half)
//!   44-47: Reserved
//!
//! The encoder is deterministic — no neural network, no external API.
//! It extracts structural and statistical properties of text that
//! create a unique spectral fingerprint. The same text always produces
//! the same features, but similar texts produce similar features.

// The codec intentionally uses floating-point arithmetic for feature
// extraction. Statistical casts feed bounded tanh outputs, while projection
// accumulation precision stays explicitly measurable against an f64 reference
// before any live migration is considered.
#![allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::arithmetic_side_effects
)]

pub use crate::codec_gain::{DEFAULT_SEMANTIC_GAIN, adaptive_gain};
use crate::codec_time_domain::{TextTimeDomainProfile, text_time_domain_profile};
use crate::types::{
    ExperienceDeltaBusV1, ExperienceDeltaKindV1, ExperienceDeltaV1, SafetyLevel, SpectralTelemetry,
};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    ffi::OsStr,
    fs,
    hash::BuildHasher,
    io::{ErrorKind, Write},
    path::{Path, PathBuf},
    sync::{
        OnceLock,
        atomic::{AtomicU64, Ordering},
    },
};

/// Number of dimensions in minime's semantic lane.
/// Widened from 32 to 48 (2026-03-31): shared discovery by Astrid and Minime
/// surfaced spectral-codec compression pressure. New dims:
///   32-39: embedding-projected semantic features (768D nomic-embed-text → 8D)
///   40-43: narrative arc (emotional trajectory within a single text)
///   44-47: reserved
pub const SEMANTIC_DIM: usize = 48;
/// Legacy dim count — used for backward-compatible warmth vectors and tests.
const SEMANTIC_DIM_LEGACY: usize = 32;
/// Number of recent characters tracked for rolling entropy.
const CHAR_FREQ_WINDOW_CAPACITY: usize = 1024;
/// Absolute post-gain clamp for semantic features.
const FEATURE_ABS_MAX: f32 = 5.0;
/// Spectral-entropy threshold above which the tail-participation feature dims
/// receive an entropy-gated vibrancy lift and a bounded clamp-ceiling offset
/// (Astrid self_study_1780922252: "offsets the FEATURE_ABS_MAX when
/// spectral_entropy exceeds 0.85").
///
/// NOT a hard gate. The normalized distance above this threshold is passed
/// through a smoothstep (`3t^2 - 2t^3`) at the application site in
/// `apply_spectral_feedback_inner` (search `let vibrancy =`), giving a
/// C1-smooth onset — zero slope *at* the gate, so entropy fluctuating around
/// 0.85 barely moves the lift (Astrid self_study_1780933511, her
/// soft-gate/sigmoid ask, shipped 2026-06-08). A logistic sigmoid *centered*
/// at the gate would be steeper here, not gentler; the smoothstep is the
/// gentler choice for "no pop". Below the gate the lift is exactly 0 (codec
/// output unchanged); the raised clamp ceiling collapses back to
/// `FEATURE_ABS_MAX`.
const TAIL_VIBRANCY_ENTROPY_GATE: f32 = 0.85;
/// Bounded ceiling for tail-participation dims at full entropy-vibrancy. A +20%
/// offset over `FEATURE_ABS_MAX`, matching the gentle-adjustment safety policy.
/// Minime attenuates the semantic lane ~0.24x (and further by emb_strength), so
/// this lands as a much smaller delta in the reservoir input vector.
const TAIL_VIBRANCY_MAX: f32 = 6.0;
const CODEC_OVERFLOW_EMOTIONAL_DIMS: [usize; 8] = [24, 25, 26, 27, 28, 29, 30, 31];
const CODEC_OVERFLOW_TAIL_DIMS: [usize; 4] = [17, 26, 27, 31];
const CODEC_OVERFLOW_MONITORED_DIMS: [usize; 9] = [17, 24, 25, 26, 27, 28, 29, 30, 31];
const CODEC_OVERFLOW_EPSILON: f32 = 1.0e-4;
const CODEC_OVERFLOW_FOLLOWUP_HOOK: &str =
    "emotional_overflow_aperture_v1_default_off_requires_replay_and_operator_approval";
const STRUCTURAL_ENTROPY_DAMPENING_START: f32 = 0.80;
const STRUCTURAL_ENTROPY_DAMPENING_FULL: f32 = 0.95;
const STRUCTURAL_ENTROPY_DAMPENING_MIN_COEFFICIENT: f32 = 0.84;
const STRUCTURAL_ENTROPY_DAMPENING_DIMS: [usize; 16] =
    [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15];
const TAIL_VIBRANCY_NOISE_DAMPENING_START: f32 = 0.90;
const TAIL_VIBRANCY_NOISE_DAMPENING_FULL: f32 = 1.0;
const TAIL_VIBRANCY_NOISE_DAMPENING_MIN_COEFFICIENT: f32 = 0.82;
/// Minime's effective semantic-lane attenuation (`dimension_scales[semantic]=0.42 ×
/// activation_gain=0.58 ≈ 0.24`). Used ONLY for the being-facing transparency readout (STATE /
/// CODEC_MAP) so Astrid's self-model reflects what actually lands in the SHARED reservoir
/// (self_study_1781680871: "I feel vivid but appear subdued"). NOT applied to the wire — minime
/// applies it on inbound.
const MINIME_SEMANTIC_ATTENUATION: f32 = 0.24;
/// Number of embedding dimensions from nomic-embed-text.
const EMBEDDING_INPUT_DIM: usize = 768;
/// Number of projected embedding dims in the codec (fills dims 32-39).
const EMBEDDING_PROJECT_DIM: usize = 8;
/// Number of narrative arc dims (fills dims 40-43).
const NARRATIVE_ARC_DIM: usize = 4;
const RESERVED_CODEC_DIM_START: usize = 44;
const SEMANTIC_PROJECTION_RESERVED_DIMS: [usize; 4] = [44, 45, 46, 47];
const SEMANTIC_FOCUS_PREVIEW_DIM: usize = 4;
const SEMANTIC_FOCUS_PREVIEW_NORM: f32 = 0.16;
const SEMANTIC_FOCUS_ENTROPY_REVIEW_FLOOR: f32 = 0.65;
const CROSS_SPECTRAL_RESERVED_DIM_ROLES: [&str; 4] = [
    "structural_friction_default_off_candidate",
    "persistence_resistance_default_off_candidate",
    "shadow_magnetization_default_off_candidate",
    "shadow_dispersal_default_off_candidate",
];
const SEMANTIC_PROJECTION_TEXTURE_SUBDIMENSIONS: [&str; 4] = [
    "lingering_persistence",
    "active_motion",
    "boundary_porosity",
    "overlapping_narrative_state",
];
const PROJECTION_CHECKSUM_ALGO: &str = "sha256-f32-le-v1";
const SEMANTIC_PROJECTION_DENSITY_REVIEW_FLOOR: f32 = 0.55;
const SEMANTIC_PROJECTION_THIN_RMS_CEIL: f32 = 0.12;
const MULTI_SCALE_RESONANCE_LOSS_THRESHOLD: f32 = 0.10;

/// Deterministic random projection matrix for embedding → 8D.
/// Uses a fixed seed so the projection is reproducible across restarts.
/// Each column is a normalized random vector (Johnson-Lindenstrauss).
fn embedding_projection_matrix() -> &'static [[f32; EMBEDDING_PROJECT_DIM]; EMBEDDING_INPUT_DIM] {
    use std::sync::OnceLock;
    static MATRIX: OnceLock<Box<[[f32; EMBEDDING_PROJECT_DIM]; EMBEDDING_INPUT_DIM]>> =
        OnceLock::new();
    MATRIX.get_or_init(|| {
        let mut mat = Box::new([[0.0_f32; EMBEDDING_PROJECT_DIM]; EMBEDDING_INPUT_DIM]);
        // LCG seeded deterministically
        let mut rng: u64 = 42;
        for row in mat.iter_mut() {
            for col in row.iter_mut() {
                rng = rng
                    .wrapping_mul(6_364_136_223_846_793_005)
                    .wrapping_add(1442695040888963407);
                // Map to roughly normal via Box-Muller-lite (uniform → centered)
                *col = ((rng >> 33) as f32 / u32::MAX as f32) - 0.5;
            }
        }
        // Normalize columns so each projected dim has unit variance
        for col_idx in 0..EMBEDDING_PROJECT_DIM {
            let norm: f32 = mat
                .iter()
                .map(|row| row[col_idx] * row[col_idx])
                .sum::<f32>()
                .sqrt();
            if norm > 0.0 {
                for row in mat.iter_mut() {
                    row[col_idx] /= norm;
                }
            }
        }
        mat
    })
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ProjectionMetadata {
    pub embedding_projection_mode: String,
    pub projection_epoch_id: Option<String>,
    pub projection_seed: Option<u64>,
    pub projection_fingerprint: String,
    #[serde(default)]
    pub projection_kernel_checksum: String,
    #[serde(default)]
    pub projection_checksum_algo: String,
    #[serde(default)]
    pub projection_epoch_source: String,
    pub feature_mean: f32,
    pub feature_rms: f32,
    #[serde(default)]
    pub feature_variance: f32,
    pub feature_max_abs: f32,
}

/// Read-only comparison of the live f32 embedding projection with an f64
/// accumulator using the exact same f32 inputs and projection weights. This
/// can reveal numerical residue without silently changing Astrid's live codec.
#[derive(Debug, Clone, PartialEq)]
pub struct ProjectionPrecisionAuditV1 {
    pub policy: &'static str,
    pub source_embedding_dim_count: usize,
    pub projected_dim_count: usize,
    pub reference_accumulator: &'static str,
    pub fixed_legacy_repeated_bit_exact: bool,
    pub dynamic_epoch_repeated_bit_exact: bool,
    pub fixed_legacy_max_abs_delta: f64,
    pub fixed_legacy_rms_delta: f64,
    pub dynamic_epoch_max_abs_delta: f64,
    pub dynamic_epoch_rms_delta: f64,
    pub accumulation_precision_state: &'static str,
    pub ghost_vibrancy_conclusion: &'static str,
    pub live_f64_migration_requires_approval: bool,
    pub live_projection_write: bool,
    pub authority: &'static str,
}

/// Controlled-pair evidence for whether the emotional/intentional lane and
/// embedding-projected semantic lane can move independently. The probe builds
/// feature vectors off the live path, then changes one lane at a time; it does
/// not write those vectors to Minime or retune either lane.
#[derive(Debug, Clone, PartialEq)]
pub struct CodecLaneSeparationAuditV1 {
    pub policy: &'static str,
    pub emotional_lane_range: (usize, usize),
    pub projected_semantic_lane_range: (usize, usize),
    pub emotional_difference_related_semantics_emotional_delta_rms: f32,
    pub emotional_difference_related_semantics_projected_delta_rms: f32,
    pub emotional_lane_selectivity_margin: f32,
    pub emotional_pair_distinguishable: bool,
    pub emotional_similarity_opposed_semantics_emotional_delta_rms: f32,
    pub emotional_similarity_opposed_semantics_projected_delta_rms: f32,
    pub projected_lane_selectivity_margin: f32,
    pub projected_pair_distinguishable: bool,
    pub legacy_projection_width_rejected: bool,
    pub state: &'static str,
    pub felt_rigidity_conclusion: &'static str,
    pub pair_construction: &'static str,
    pub observational_only: bool,
    pub right_to_ignore: bool,
    pub live_vector_write: bool,
    pub live_gain_write: bool,
    pub live_projection_write: bool,
    pub live_eligible_now: bool,
    pub auto_approved: bool,
    pub grants_approval: bool,
    pub authority: &'static str,
}

/// Read-only witness for mixed-regime text at and beyond the live character
/// window boundary. It deliberately reports both the muddy in-window case and
/// the trailing-regime case after full prefix eviction.
#[derive(Debug, Clone, PartialEq)]
pub struct CodecRollingWindowShiftAuditV1 {
    pub policy: &'static str,
    pub capacity_chars: usize,
    pub in_capacity_prefix_chars: usize,
    pub in_capacity_tail_chars: usize,
    pub in_capacity_window_entropy: f32,
    pub in_capacity_trailing_entropy: f32,
    pub in_capacity_delta_to_trailing: f32,
    pub in_capacity_state: &'static str,
    pub evicting_prefix_chars: usize,
    pub evicting_tail_chars: usize,
    pub evicting_window_entropy: f32,
    pub evicting_trailing_entropy: f32,
    pub evicting_delta_to_trailing: f32,
    pub evicting_state: &'static str,
    pub state: &'static str,
    pub felt_muddy_middle_conclusion: &'static str,
    pub density_aware_window_change_requires_approval: bool,
    pub live_window_capacity_change: bool,
    pub live_vector_write: bool,
    pub observational_only: bool,
    pub right_to_ignore: bool,
    pub live_eligible_now: bool,
    pub auto_approved: bool,
    pub grants_approval: bool,
    pub authority: &'static str,
}

fn stable_hash64(text: &str) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in text.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01B3);
    }
    hash
}

fn splitmix64(mut value: u64) -> u64 {
    value = value.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = value;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

fn unit_from_seed(seed: u64) -> f32 {
    let bits = splitmix64(seed) >> 40;
    (bits as f32 / 16_777_215.0) - 0.5
}

fn projection_runtime_dir_from_parts(
    env_dir: Option<&OsStr>,
    current_exe: Option<&Path>,
) -> PathBuf {
    if let Some(dir) = env_dir {
        let path = PathBuf::from(dir);
        if !path.as_os_str().is_empty() {
            return path;
        }
    }
    if let Some(exe_path) = current_exe
        && let Some(parent) = exe_path.parent()
    {
        return parent.join("data").join("spectral-bridge").join("runtime");
    }
    PathBuf::from("data")
        .join("spectral-bridge")
        .join("runtime")
}

fn projection_runtime_dir() -> PathBuf {
    let env_dir = std::env::var_os("ASTRID_CODEC_RUNTIME_DIR");
    let current_exe = std::env::current_exe().ok();
    projection_runtime_dir_from_parts(env_dir.as_deref(), current_exe.as_deref())
}

fn projection_runtime_resolution_readout() -> String {
    let env_dir = std::env::var_os("ASTRID_CODEC_RUNTIME_DIR");
    let env_override = env_dir.as_deref().is_some_and(|value| !value.is_empty());
    let current_exe = std::env::current_exe().ok();
    let source = if env_override {
        "env_override"
    } else if current_exe
        .as_ref()
        .and_then(|path| path.parent())
        .is_some()
    {
        "executable_relative"
    } else {
        "process_relative_fallback"
    };
    let resolved = projection_runtime_dir_from_parts(env_dir.as_deref(), current_exe.as_deref());
    format!(
        "projection_runtime_resolution_v1: source={source}; resolved_path={}; hierarchy=data/spectral-bridge/runtime; fallback_behavior=kernel_derived_stable_epoch_not_random_remap; who_can_change_it=Mike/operator_or_deploy_env; how_to_test_it=cargo test --manifest-path capsules/spectral-bridge/Cargo.toml --lib codec_projection_runtime_dir_uses_env_or_executable_relative_cache -- --exact",
        resolved.display()
    )
}

fn hex_digest(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

fn update_projection_hash_header(hasher: &mut Sha256, mode: &str) {
    hasher.update(PROJECTION_CHECKSUM_ALGO.as_bytes());
    hasher.update(b"\0");
    hasher.update(mode.as_bytes());
    hasher.update(b"\0");
    hasher.update((EMBEDDING_INPUT_DIM as u64).to_le_bytes());
    hasher.update((EMBEDDING_PROJECT_DIM as u64).to_le_bytes());
    hasher.update((NARRATIVE_ARC_DIM as u64).to_le_bytes());
}

fn fixed_legacy_projection_kernel_checksum() -> String {
    let mut hasher = Sha256::new();
    update_projection_hash_header(&mut hasher, "fixed_legacy");
    for row in embedding_projection_matrix().iter() {
        for value in row {
            hasher.update(value.to_le_bytes());
        }
    }
    hex_digest(&hasher.finalize())
}

fn dynamic_epoch_projection_kernel_checksum(epoch: &str) -> String {
    let mut hasher = Sha256::new();
    update_projection_hash_header(&mut hasher, "dynamic_epoch_v1");
    hasher.update(epoch.as_bytes());
    hex_digest(&hasher.finalize())
}

fn kernel_derived_projection_epoch_id() -> String {
    let checksum = fixed_legacy_projection_kernel_checksum();
    format!("kernel_{}", &checksum[..16.min(checksum.len())])
}

static PROJECTION_EPOCH_WRITE_COUNTER: AtomicU64 = AtomicU64::new(0);

fn projection_epoch_id_from_file(path: &Path) -> Option<String> {
    let text = fs::read_to_string(path).ok()?;
    let value = serde_json::from_str::<serde_json::Value>(&text).ok()?;
    let epoch = value
        .get("projection_epoch_id")
        .and_then(serde_json::Value::as_str)?;
    if epoch.is_empty() {
        None
    } else {
        Some(epoch.to_string())
    }
}

fn write_projection_epoch_payload_atomic(path: &Path, payload: &str) {
    let Some(parent) = path.parent() else {
        return;
    };
    let Some(file_name) = path.file_name().and_then(OsStr::to_str) else {
        return;
    };
    let nonce = PROJECTION_EPOCH_WRITE_COUNTER.fetch_add(1, Ordering::Relaxed);
    let tmp_path = parent.join(format!(".{file_name}.{}.{}.tmp", std::process::id(), nonce));
    let write_result = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&tmp_path)
        .and_then(|mut file| {
            file.write_all(payload.as_bytes())?;
            file.sync_all()
        });
    if write_result.is_err() {
        let _ = fs::remove_file(&tmp_path);
        return;
    }
    install_projection_epoch_payload_from_tmp(path, &tmp_path, file_name, nonce);
}

fn install_projection_epoch_payload_from_tmp(
    path: &Path,
    tmp_path: &Path,
    file_name: &str,
    nonce: u64,
) {
    if install_tmp_payload_without_clobber(tmp_path, path).is_ok() {
        let _ = fs::remove_file(tmp_path);
        return;
    }

    if projection_epoch_id_from_file(path).is_some() {
        let _ = fs::remove_file(tmp_path);
        return;
    }

    if !path.exists() {
        let _ = fs::remove_file(tmp_path);
        return;
    }

    let Some(parent) = path.parent() else {
        let _ = fs::remove_file(tmp_path);
        return;
    };
    let stale_path = parent.join(format!(
        ".{file_name}.{}.{}.stale",
        std::process::id(),
        nonce
    ));
    if fs::rename(path, &stale_path).is_err() {
        if projection_epoch_id_from_file(path).is_some() {
            let _ = fs::remove_file(tmp_path);
            return;
        }
        let _ = fs::remove_file(tmp_path);
        return;
    }

    let installed = match install_tmp_payload_without_clobber(tmp_path, path) {
        Ok(()) => {
            let _ = fs::remove_file(tmp_path);
            true
        },
        Err(err) if err.kind() == ErrorKind::AlreadyExists => {
            projection_epoch_id_from_file(path).is_some()
        },
        Err(_) => false,
    };

    if installed {
        let _ = fs::remove_file(&stale_path);
    } else {
        if !path.exists() {
            let _ = fs::rename(&stale_path, path);
        }
        let _ = fs::remove_file(tmp_path);
    }
}

fn install_tmp_payload_without_clobber(tmp_path: &Path, path: &Path) -> std::io::Result<()> {
    match fs::hard_link(tmp_path, path) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == ErrorKind::AlreadyExists => Err(err),
        Err(_) => {
            let payload = fs::read(tmp_path)?;
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(path)?;
            file.write_all(&payload)?;
            file.sync_all()
        },
    }
}

fn load_or_create_projection_epoch_id_from(
    runtime_dir: &Path,
    env_epoch: Option<&str>,
) -> (String, String) {
    if let Some(epoch) = env_epoch
        && !epoch.trim().is_empty()
    {
        return (epoch.to_string(), "env".to_string());
    }
    let path = runtime_dir.join("codec_projection_epoch.json");
    if let Some(epoch) = projection_epoch_id_from_file(&path) {
        return (epoch, "file".to_string());
    }
    let epoch = kernel_derived_projection_epoch_id();
    let _ = fs::create_dir_all(runtime_dir);
    let payload = serde_json::json!({
        "projection_epoch_id": epoch,
        "embedding_projection_mode": "dynamic_epoch_v1",
        "projection_kernel_checksum": dynamic_epoch_projection_kernel_checksum(&epoch),
        "projection_checksum_algo": PROJECTION_CHECKSUM_ALGO,
        "projection_epoch_source": "kernel_derived",
        "projection_kernel_source_checksum": fixed_legacy_projection_kernel_checksum(),
        "policy": "fresh runtime dirs derive the epoch from stable projection-kernel content; env and existing files still take precedence",
    });
    let payload_text = serde_json::to_string_pretty(&payload).unwrap_or_else(|_| "{}".to_string());
    write_projection_epoch_payload_atomic(&path, &payload_text);
    (epoch, "kernel_derived".to_string())
}

fn load_or_create_projection_epoch_id() -> (String, String) {
    let env_epoch = std::env::var("ASTRID_CODEC_PROJECTION_EPOCH_ID").ok();
    load_or_create_projection_epoch_id_from(&projection_runtime_dir(), env_epoch.as_deref())
}

fn projection_stats(projected: &[f32; EMBEDDING_PROJECT_DIM]) -> (f32, f32, f32, f32) {
    let mean = projected.iter().sum::<f32>() / EMBEDDING_PROJECT_DIM as f32;
    let rms = (projected.iter().map(|value| value * value).sum::<f32>()
        / EMBEDDING_PROJECT_DIM as f32)
        .sqrt();
    let variance = projected
        .iter()
        .map(|value| {
            let delta = value - mean;
            delta * delta
        })
        .sum::<f32>()
        / EMBEDDING_PROJECT_DIM as f32;
    let max_abs = projected
        .iter()
        .map(|value| value.abs())
        .fold(0.0, f32::max);
    (mean, rms, variance, max_abs)
}

fn projection_fingerprint_bits(value: f32) -> u32 {
    if value == 0.0 || value.abs() < f32::MIN_POSITIVE {
        0.0_f32.to_bits()
    } else if value.is_nan() {
        f32::NAN.to_bits()
    } else {
        value.to_bits()
    }
}

fn projection_fingerprint(seed: u64, projected: &[f32; EMBEDDING_PROJECT_DIM]) -> String {
    let mut hash = seed;
    for value in projected {
        hash ^= u64::from(projection_fingerprint_bits(*value));
        hash = splitmix64(hash);
    }
    format!("{hash:016x}")
}

pub fn project_embedding_dynamic_epoch(
    embedding: &[f32],
    text: &str,
    projection_epoch_id: &str,
    chunk_index: u32,
) -> Option<([f32; EMBEDDING_PROJECT_DIM], ProjectionMetadata)> {
    project_embedding_dynamic_epoch_with_source(
        embedding,
        text,
        projection_epoch_id,
        chunk_index,
        "explicit",
    )
}

fn project_embedding_dynamic_epoch_with_source(
    embedding: &[f32],
    text: &str,
    projection_epoch_id: &str,
    chunk_index: u32,
    projection_epoch_source: &str,
) -> Option<([f32; EMBEDDING_PROJECT_DIM], ProjectionMetadata)> {
    if embedding.len() != EMBEDDING_INPUT_DIM {
        return None;
    }
    let seed = stable_hash64(projection_epoch_id)
        ^ stable_hash64(text).rotate_left(13)
        ^ u64::from(chunk_index).wrapping_mul(0xA24B_AED4_963E_E407);
    let mut result = [0.0_f32; EMBEDDING_PROJECT_DIM];
    for (i, &val) in embedding.iter().enumerate() {
        for (j, out) in result.iter_mut().enumerate() {
            let cell_seed = seed
                ^ ((i as u64).wrapping_mul(0x9E37_79B1))
                ^ ((j as u64).wrapping_mul(0x85EB_CA77));
            *out += val * unit_from_seed(cell_seed);
        }
    }
    let norm: f32 = result.iter().map(|v| v * v).sum::<f32>().sqrt();
    if norm > 0.0 {
        let scale = 0.35 / norm;
        for value in &mut result {
            *value *= scale;
        }
    }
    let (feature_mean, feature_rms, feature_variance, feature_max_abs) = projection_stats(&result);
    let metadata = ProjectionMetadata {
        embedding_projection_mode: "dynamic_epoch_v1".to_string(),
        projection_epoch_id: Some(projection_epoch_id.to_string()),
        projection_seed: Some(seed),
        projection_fingerprint: projection_fingerprint(seed, &result),
        projection_kernel_checksum: dynamic_epoch_projection_kernel_checksum(projection_epoch_id),
        projection_checksum_algo: PROJECTION_CHECKSUM_ALGO.to_string(),
        projection_epoch_source: projection_epoch_source.to_string(),
        feature_mean,
        feature_rms,
        feature_variance,
        feature_max_abs,
    };
    Some((result, metadata))
}

fn project_embedding_runtime(
    embedding: &[f32],
    text: &str,
    chunk_index: u32,
) -> Option<([f32; EMBEDDING_PROJECT_DIM], ProjectionMetadata)> {
    let mode = std::env::var("ASTRID_CODEC_EMBEDDING_PROJECTION_MODE")
        .unwrap_or_else(|_| "dynamic_epoch_v1".to_string());
    if mode == "fixed_legacy" {
        let projected = project_embedding(embedding)?;
        let (feature_mean, feature_rms, feature_variance, feature_max_abs) =
            projection_stats(&projected);
        return Some((
            projected,
            ProjectionMetadata {
                embedding_projection_mode: "fixed_legacy".to_string(),
                projection_epoch_id: None,
                projection_seed: None,
                projection_fingerprint: projection_fingerprint(42, &projected),
                projection_kernel_checksum: fixed_legacy_projection_kernel_checksum(),
                projection_checksum_algo: PROJECTION_CHECKSUM_ALGO.to_string(),
                projection_epoch_source: "fixed_legacy".to_string(),
                feature_mean,
                feature_rms,
                feature_variance,
                feature_max_abs,
            },
        ));
    }
    let (epoch, source) = load_or_create_projection_epoch_id();
    project_embedding_dynamic_epoch_with_source(embedding, text, &epoch, chunk_index, &source)
}

/// Project a 768D embedding down to 8D using the fixed projection matrix.
/// Returns None if the embedding is wrong length.
pub fn project_embedding(embedding: &[f32]) -> Option<[f32; EMBEDDING_PROJECT_DIM]> {
    if embedding.len() != EMBEDDING_INPUT_DIM {
        return None;
    }
    let proj = embedding_projection_matrix();
    let mut result = [0.0_f32; EMBEDDING_PROJECT_DIM];
    for (i, &val) in embedding.iter().enumerate() {
        for (j, out) in result.iter_mut().enumerate() {
            *out += val * proj[i][j];
        }
    }
    // L2-normalize then scale to ~0.3 so softsign output is in a useful range
    let norm: f32 = result.iter().map(|v| v * v).sum::<f32>().sqrt();
    if norm > 0.0 {
        let scale = 0.35 / norm;
        for v in &mut result {
            *v *= scale;
        }
    }
    Some(result)
}

fn normalize_projection_f64(
    mut values: [f64; EMBEDDING_PROJECT_DIM],
) -> [f64; EMBEDDING_PROJECT_DIM] {
    let norm = values.iter().map(|value| value * value).sum::<f64>().sqrt();
    if norm > 0.0 {
        let scale = 0.35_f64 / norm;
        for value in &mut values {
            *value *= scale;
        }
    }
    values
}

fn project_embedding_fixed_f64_reference(
    embedding: &[f32],
) -> Option<[f64; EMBEDDING_PROJECT_DIM]> {
    if embedding.len() != EMBEDDING_INPUT_DIM {
        return None;
    }
    let projection = embedding_projection_matrix();
    let mut result = [0.0_f64; EMBEDDING_PROJECT_DIM];
    for (row_idx, value) in embedding.iter().enumerate() {
        for (column_idx, output) in result.iter_mut().enumerate() {
            *output += f64::from(*value) * f64::from(projection[row_idx][column_idx]);
        }
    }
    Some(normalize_projection_f64(result))
}

fn project_embedding_dynamic_f64_reference(
    embedding: &[f32],
    text: &str,
    projection_epoch_id: &str,
    chunk_index: u32,
) -> Option<[f64; EMBEDDING_PROJECT_DIM]> {
    if embedding.len() != EMBEDDING_INPUT_DIM {
        return None;
    }
    let seed = stable_hash64(projection_epoch_id)
        ^ stable_hash64(text).rotate_left(13)
        ^ u64::from(chunk_index).wrapping_mul(0xA24B_AED4_963E_E407);
    let mut result = [0.0_f64; EMBEDDING_PROJECT_DIM];
    for (row_idx, value) in embedding.iter().enumerate() {
        for (column_idx, output) in result.iter_mut().enumerate() {
            let cell_seed = seed
                ^ ((row_idx as u64).wrapping_mul(0x9E37_79B1))
                ^ ((column_idx as u64).wrapping_mul(0x85EB_CA77));
            // Promote the exact live f32 coefficient so this audit isolates
            // accumulation/normalization precision rather than changing the
            // projection kernel under comparison.
            *output += f64::from(*value) * f64::from(unit_from_seed(cell_seed));
        }
    }
    Some(normalize_projection_f64(result))
}

fn projection_precision_delta(
    live: &[f32; EMBEDDING_PROJECT_DIM],
    reference: &[f64; EMBEDDING_PROJECT_DIM],
) -> (f64, f64) {
    let mut max_abs_delta = 0.0_f64;
    let mut squared_delta_sum = 0.0_f64;
    for (live_value, reference_value) in live.iter().zip(reference) {
        let delta = (f64::from(*live_value) - reference_value).abs();
        max_abs_delta = max_abs_delta.max(delta);
        squared_delta_sum += delta * delta;
    }
    let rms_delta = (squared_delta_sum / EMBEDDING_PROJECT_DIM as f64).sqrt();
    (max_abs_delta, rms_delta)
}

/// Compare the current projection paths with f64 reference accumulation.
/// The audit is evidence only: it never replaces the live f32 result.
#[must_use]
pub fn projection_precision_audit_v1(
    embedding: &[f32],
    text: &str,
    projection_epoch_id: &str,
    chunk_index: u32,
) -> Option<ProjectionPrecisionAuditV1> {
    let fixed_live = project_embedding(embedding)?;
    let fixed_repeat = project_embedding(embedding)?;
    let fixed_reference = project_embedding_fixed_f64_reference(embedding)?;
    let (dynamic_live, _) =
        project_embedding_dynamic_epoch(embedding, text, projection_epoch_id, chunk_index)?;
    let (dynamic_repeat, _) =
        project_embedding_dynamic_epoch(embedding, text, projection_epoch_id, chunk_index)?;
    let dynamic_reference =
        project_embedding_dynamic_f64_reference(embedding, text, projection_epoch_id, chunk_index)?;
    let (fixed_legacy_max_abs_delta, fixed_legacy_rms_delta) =
        projection_precision_delta(&fixed_live, &fixed_reference);
    let (dynamic_epoch_max_abs_delta, dynamic_epoch_rms_delta) =
        projection_precision_delta(&dynamic_live, &dynamic_reference);
    let worst_delta = fixed_legacy_max_abs_delta.max(dynamic_epoch_max_abs_delta);
    let accumulation_precision_state = if worst_delta <= 1.0e-6 {
        "reference_delta_below_one_part_per_million"
    } else if worst_delta <= 1.0e-4 {
        "measurable_bounded_reference_delta"
    } else {
        "reference_delta_requires_replay_review"
    };

    Some(ProjectionPrecisionAuditV1 {
        policy: "projection_precision_audit_v1",
        source_embedding_dim_count: EMBEDDING_INPUT_DIM,
        projected_dim_count: EMBEDDING_PROJECT_DIM,
        reference_accumulator: "f64_accumulation_and_normalization_over_exact_live_f32_weights",
        fixed_legacy_repeated_bit_exact: fixed_live == fixed_repeat,
        dynamic_epoch_repeated_bit_exact: dynamic_live == dynamic_repeat,
        fixed_legacy_max_abs_delta,
        fixed_legacy_rms_delta,
        dynamic_epoch_max_abs_delta,
        dynamic_epoch_rms_delta,
        accumulation_precision_state,
        ghost_vibrancy_conclusion: "reference_delta_is_evidence_not_proof_of_or_against_felt_ghost_vibrancy; replay_actual_embeddings_if_friction_persists",
        live_f64_migration_requires_approval: true,
        live_projection_write: false,
        authority: "read_only_precision_audit_not_projection_kernel_accumulator_or_live_vector_change",
    })
}

#[must_use]
pub fn projection_precision_probe_v1() -> ProjectionPrecisionAuditV1 {
    let embedding = (0..EMBEDDING_INPUT_DIM)
        .map(|idx| {
            let centered = ((idx % 37) as f32 - 18.0) / 18.0;
            if idx % 2 == 0 { centered } else { -centered }
        })
        .collect::<Vec<_>>();
    projection_precision_audit_v1(
        &embedding,
        "Static high-entropy lattice phrase held unchanged for precision review.",
        &kernel_derived_projection_epoch_id(),
        0,
    )
    .expect("internal precision probe has the canonical embedding width")
}

/// Compute narrative arc from embedding deltas: how semantic meaning shifts
/// from the first half of the text to the second.
/// Takes pre-projected 8D embeddings for each half. Returns the first 4
/// components of the delta — capturing the dominant directional shift.
/// No keyword lists: the embedding captures semantic meaning directly.
pub fn compute_narrative_arc_from_embeddings(
    first_half_proj: &[f32; EMBEDDING_PROJECT_DIM],
    second_half_proj: &[f32; EMBEDDING_PROJECT_DIM],
) -> [f32; NARRATIVE_ARC_DIM] {
    let mut arc = [0.0_f32; NARRATIVE_ARC_DIM];
    for (i, a) in arc.iter_mut().enumerate() {
        // Scale by 3.0 so small embedding shifts produce visible arc signals
        *a = tanh(3.0 * (second_half_proj[i] - first_half_proj[i]));
    }
    arc
}

fn signed_transition_energy(
    from: &[f32; EMBEDDING_PROJECT_DIM],
    to: &[f32; EMBEDDING_PROJECT_DIM],
) -> f32 {
    let mut sum_sq = 0.0_f32;
    let mut dominant_delta = 0.0_f32;
    for (before, after) in from.iter().zip(to.iter()) {
        let delta = after - before;
        sum_sq += delta * delta;
        if delta.abs() > dominant_delta.abs() {
            dominant_delta = delta;
        }
    }
    let energy = (sum_sq / EMBEDDING_PROJECT_DIM as f32).sqrt();
    energy.copysign(dominant_delta)
}

/// Compute a four-point narrative trajectory from projected quarter embeddings.
///
/// Dims 40-42 carry signed transition energy across consecutive quarters; dim
/// 43 carries the signed full-span transition. This keeps the live lane at 48D
/// while allowing coiling/folding patterns to be visible instead of collapsing
/// every text into a first-half/second-half delta.
pub fn compute_narrative_arc_from_four_point_embeddings(
    quarter_projections: &[[f32; EMBEDDING_PROJECT_DIM]; 4],
) -> [f32; NARRATIVE_ARC_DIM] {
    [
        tanh(3.0 * signed_transition_energy(&quarter_projections[0], &quarter_projections[1])),
        tanh(3.0 * signed_transition_energy(&quarter_projections[1], &quarter_projections[2])),
        tanh(3.0 * signed_transition_energy(&quarter_projections[2], &quarter_projections[3])),
        tanh(3.0 * signed_transition_energy(&quarter_projections[0], &quarter_projections[3])),
    ]
}

#[derive(Debug, Clone, PartialEq)]
pub struct NarrativeArcSplitV1 {
    pub policy: &'static str,
    pub intentional_arc: [f32; NARRATIVE_ARC_DIM],
    pub reactive_arc: [f32; NARRATIVE_ARC_DIM],
    pub captured_arc_energy: f32,
    pub tail_arc_energy: f32,
    pub coarsening_risk: &'static str,
    pub authority: &'static str,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NarrativeArcCurvatureV1 {
    pub policy: &'static str,
    pub linear_arc: [f32; NARRATIVE_ARC_DIM],
    pub quarter_arc: [f32; NARRATIVE_ARC_DIM],
    pub transition_energy: f32,
    pub full_span_energy: f32,
    pub curvature_energy: f32,
    pub sign_turn_count: usize,
    pub loop_likelihood: f32,
    pub progression_likelihood: f32,
    pub state: &'static str,
    pub authority: &'static str,
}

#[must_use]
pub fn narrative_arc_split_v1(
    first_half_proj: &[f32; EMBEDDING_PROJECT_DIM],
    second_half_proj: &[f32; EMBEDDING_PROJECT_DIM],
) -> NarrativeArcSplitV1 {
    let mut intentional_arc = [0.0_f32; NARRATIVE_ARC_DIM];
    let mut reactive_arc = [0.0_f32; NARRATIVE_ARC_DIM];
    for (idx, value) in intentional_arc.iter_mut().enumerate() {
        *value = tanh(3.0 * (second_half_proj[idx] - first_half_proj[idx]));
    }
    for (idx, value) in reactive_arc.iter_mut().enumerate() {
        let source_idx = idx + NARRATIVE_ARC_DIM;
        *value = tanh(3.0 * (second_half_proj[source_idx] - first_half_proj[source_idx]));
    }
    let captured_arc_energy = rms_4(intentional_arc);
    let tail_arc_energy = rms_4(reactive_arc);
    let coarsening_risk = if captured_arc_energy <= 0.01 && tail_arc_energy <= 0.01 {
        "unknown"
    } else if tail_arc_energy > captured_arc_energy * 1.25 && tail_arc_energy > 0.05 {
        "tail_dominant"
    } else {
        "balanced"
    };
    NarrativeArcSplitV1 {
        policy: "narrative_arc_split_v1",
        intentional_arc,
        reactive_arc,
        captured_arc_energy,
        tail_arc_energy,
        coarsening_risk,
        authority: "diagnostic_sidecar_not_live_codec_dimension",
    }
}

#[must_use]
pub fn narrative_arc_curvature_v1(
    quarter_projections: &[[f32; EMBEDDING_PROJECT_DIM]; 4],
) -> NarrativeArcCurvatureV1 {
    let linear_arc =
        compute_narrative_arc_from_embeddings(&quarter_projections[0], &quarter_projections[3]);
    let quarter_arc = compute_narrative_arc_from_four_point_embeddings(quarter_projections);
    let transition_energy = mean_abs(&quarter_arc[0..3]).clamp(0.0, 1.0);
    let full_span_energy = quarter_arc[3].abs().clamp(0.0, 1.0);
    let curvature_energy = (transition_energy - full_span_energy)
        .max(0.0)
        .clamp(0.0, 1.0);
    let mut sign_turn_count = 0_usize;
    for pair in quarter_arc[0..3].windows(2) {
        if pair[0].abs() > 0.03 && pair[1].abs() > 0.03 && pair[0].signum() != pair[1].signum() {
            sign_turn_count += 1;
        }
    }
    let denom = transition_energy.max(0.01);
    let loop_likelihood = (curvature_energy / denom).clamp(0.0, 1.0);
    let progression_likelihood = (full_span_energy / denom).clamp(0.0, 1.0);
    let state = if transition_energy < 0.04 {
        "arc_too_quiet"
    } else if sign_turn_count > 0 && loop_likelihood >= 0.35 {
        "circular_or_coiling_arc_visible"
    } else if progression_likelihood >= 0.60 {
        "linear_progression_visible"
    } else {
        "mixed_arc_watch"
    };
    NarrativeArcCurvatureV1 {
        policy: "narrative_arc_curvature_v1",
        linear_arc,
        quarter_arc,
        transition_energy,
        full_span_energy,
        curvature_energy,
        sign_turn_count,
        loop_likelihood,
        progression_likelihood,
        state,
        authority: "diagnostic_sidecar_not_live_codec_dimension_or_gain",
    }
}

fn rms_4(values: [f32; NARRATIVE_ARC_DIM]) -> f32 {
    (values.iter().map(|value| value * value).sum::<f32>() / NARRATIVE_ARC_DIM as f32).sqrt()
}

fn is_reserved_codec_dim(idx: usize) -> bool {
    (RESERVED_CODEC_DIM_START..SEMANTIC_DIM).contains(&idx)
}

pub fn text_complexity_score(
    text: &str,
    features: &[f32; SEMANTIC_DIM],
    novelty_divergence: f32,
) -> f32 {
    let time_domain = text_time_domain_profile(text);
    let cadence_pressure = time_domain.temporal_complexity;
    let question_density = features[18].abs().min(1.0);
    let energy = features[31].abs().min(1.0);
    let entropy = features[0].abs().min(1.0);
    let word_count = text.split_whitespace().count().max(1) as f32;
    let length_pressure = ((word_count.ln() / 5.0).clamp(0.0, 1.0)).min(1.0);
    (entropy * 0.28
        + novelty_divergence.clamp(0.0, 1.0) * 0.24
        + cadence_pressure * 0.16
        + question_density * 0.12
        + energy * 0.12
        + length_pressure * 0.08)
        .clamp(0.0, 1.0)
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SpectralPressureDecision {
    pub controller: String,
    pub lambda_pressure_source: String,
    pub complexity_drive: f32,
    pub resist_drive: f32,
    pub target_lambda_bias: f32,
    pub suppression_reason: Option<String>,
    pub text_complexity_pressure: f32,
    pub time_domain_complexity: f32,
}

fn spectral_entropy(values: &[f32]) -> f32 {
    let positive: Vec<f32> = values
        .iter()
        .filter_map(|value| {
            let value = value.abs();
            (value > 0.0 && value.is_finite()).then_some(value)
        })
        .collect();
    let total: f32 = positive.iter().sum();
    if total <= f32::EPSILON || positive.len() <= 1 {
        return 0.0;
    }
    let entropy = positive.iter().fold(0.0_f32, |acc, value| {
        let share = *value / total;
        if share > 0.0 {
            acc - share * share.ln()
        } else {
            acc
        }
    });
    (entropy / (positive.len() as f32).ln()).clamp(0.0, 1.0)
}

pub fn spectral_pressure_controller_v1(
    text: &str,
    final_features: &[f32],
    eigenvalues: &[f32],
    fill_pct: Option<f32>,
    semantic_energy: Option<f32>,
    watchdog_monitoring: bool,
    stage: Option<&str>,
) -> SpectralPressureDecision {
    let mut padded = [0.0_f32; SEMANTIC_DIM];
    for (dst, src) in padded.iter_mut().zip(final_features.iter()) {
        *dst = *src;
    }
    let time_domain = text_time_domain_profile(text);
    let complexity = text_complexity_score(text, &padded, padded[26].abs().min(1.0));
    let total: f32 = eigenvalues.iter().map(|value| value.abs()).sum();
    let lambda1_share = eigenvalues
        .first()
        .map(|value| value.abs() / total.max(f32::EPSILON))
        .unwrap_or(0.0);
    let r12 = if eigenvalues.len() >= 2 && eigenvalues[1].abs() > 0.01 {
        eigenvalues[0].abs() / eigenvalues[1].abs()
    } else {
        0.0
    };
    let entropy = spectral_entropy(eigenvalues);
    let lower = text.to_ascii_lowercase();
    let felt_resist = [
        "localized gravity",
        "funnel",
        "dam",
        "restriction",
        "protective focus",
        "constriction",
        "compaction",
        "density",
        "stubborn",
        "resist",
    ]
    .iter()
    .any(|term| lower.contains(term));
    let complexity_drive = complexity;
    let resist_drive = (lambda1_share * 0.35
        + (1.0 - entropy) * 0.20
        + ((r12 - 1.4) / 1.8).clamp(0.0, 1.0) * 0.25
        + if felt_resist { 0.20 } else { 0.0 })
    .clamp(0.0, 1.0);
    let raw_bias = ((complexity_drive - resist_drive) * 0.10).clamp(-0.10, 0.10);
    let suppression_reason = if !watchdog_monitoring {
        Some("watchdog_not_monitoring".to_string())
    } else if fill_pct.is_some_and(|fill| fill >= 76.0) {
        Some("fill_high_suppress_upward_bias".to_string())
    } else if semantic_energy.is_some_and(|energy| energy > 0.05) {
        Some("semantic_energy_active".to_string())
    } else if stage.is_some_and(|value| value.eq_ignore_ascii_case("discharge")) {
        Some("stage_discharge".to_string())
    } else {
        None
    };
    let target_lambda_bias = if suppression_reason.is_some() && raw_bias > 0.0 {
        0.0
    } else {
        raw_bias
    };
    SpectralPressureDecision {
        controller: "spectral_pressure_controller_v1".to_string(),
        lambda_pressure_source: "codec_text_complexity_and_resist_v1".to_string(),
        complexity_drive,
        resist_drive,
        target_lambda_bias,
        suppression_reason,
        text_complexity_pressure: complexity,
        time_domain_complexity: time_domain.temporal_complexity,
    }
}

/// Encode text into a 48-dimensional feature vector for minime's
/// semantic sensory lane.
///
/// The encoding captures structural properties of the text:
/// - **Dims 0-7**: Character-level statistics (entropy, density, rhythm)
/// - **Dims 8-15**: Word-level features (complexity, hedging, certainty)
/// - **Dims 16-23**: Sentence-level structure (length variance, question density)
/// - **Dims 24-31**: Emotional/intentional markers (urgency, warmth, tension)
///
/// All values are normalized to approximately \[-1.0, 1.0\] with `tanh`
/// compression so the ESN reservoir receives gentle, bounded input.
///
const MAX_RESONANCE_HISTORY_LEN: usize = 32;
const DEFAULT_RESONANCE_HISTORY_LEN: usize = 12;
const DEFAULT_RESONANCE_RECENCY_DECAY: f32 = 0.74;
const DEFAULT_RESONANCE_MAX_BOOST: f32 = 0.32;
const DEFAULT_RESONANCE_DISCRETE_MIX: f32 = 0.45;
const DEFAULT_RESONANCE_CONTINUOUS_MIX: f32 = 0.55;
const DEFAULT_RESONANCE_NOVELTY_FLOOR: f32 = 0.35;

/// Runtime tuning for the history-aware resonance layer.
///
/// The codec is intentionally still deterministic, but these values are no
/// longer hardcoded in the algorithm itself. That gives us room to tune the
/// feel of recurrence without replacing the codec.
#[derive(Debug, Clone, Copy)]
pub struct ResonanceTuning {
    pub history_len: usize,
    pub recency_decay: f32,
    pub max_boost: f32,
    pub discrete_mix: f32,
    pub continuous_mix: f32,
    pub novelty_floor: f32,
}

impl Default for ResonanceTuning {
    fn default() -> Self {
        Self {
            history_len: DEFAULT_RESONANCE_HISTORY_LEN,
            recency_decay: DEFAULT_RESONANCE_RECENCY_DECAY,
            max_boost: DEFAULT_RESONANCE_MAX_BOOST,
            discrete_mix: DEFAULT_RESONANCE_DISCRETE_MIX,
            continuous_mix: DEFAULT_RESONANCE_CONTINUOUS_MIX,
            novelty_floor: DEFAULT_RESONANCE_NOVELTY_FLOOR,
        }
    }
}

fn parse_env_usize(name: &str, default: usize, min: usize, max: usize) -> usize {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .map_or(default, |value| value.clamp(min, max))
}

fn parse_env_f32(name: &str, default: f32, min: f32, max: f32) -> f32 {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse::<f32>().ok())
        .map_or(default, |value| value.clamp(min, max))
}

pub fn resonance_tuning() -> &'static ResonanceTuning {
    static TUNING: OnceLock<ResonanceTuning> = OnceLock::new();
    TUNING.get_or_init(|| ResonanceTuning {
        history_len: parse_env_usize(
            "ASTRID_CODEC_HISTORY_LEN",
            DEFAULT_RESONANCE_HISTORY_LEN,
            4,
            MAX_RESONANCE_HISTORY_LEN,
        ),
        recency_decay: parse_env_f32(
            "ASTRID_CODEC_RECENCY_DECAY",
            DEFAULT_RESONANCE_RECENCY_DECAY,
            0.45,
            0.98,
        ),
        max_boost: parse_env_f32(
            "ASTRID_CODEC_MAX_RESONANCE_BOOST",
            DEFAULT_RESONANCE_MAX_BOOST,
            0.0,
            0.6,
        ),
        discrete_mix: parse_env_f32(
            "ASTRID_CODEC_DISCRETE_MIX",
            DEFAULT_RESONANCE_DISCRETE_MIX,
            0.0,
            1.0,
        ),
        continuous_mix: parse_env_f32(
            "ASTRID_CODEC_CONTINUOUS_MIX",
            DEFAULT_RESONANCE_CONTINUOUS_MIX,
            0.0,
            1.0,
        ),
        novelty_floor: parse_env_f32(
            "ASTRID_CODEC_NOVELTY_FLOOR",
            DEFAULT_RESONANCE_NOVELTY_FLOOR,
            0.1,
            0.9,
        ),
    })
}

/// Classified text type based on dominant feature signals.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum TextType {
    Questioning, // question density dominant
    Hedging,     // hedging/uncertainty dominant
    Declarative, // certainty dominant
    Warm,        // warmth markers dominant
    Tense,       // tension markers dominant
    Curious,     // curiosity markers dominant
    Reflective,  // introspection markers dominant
    Neutral,     // no dominant signal
}

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct ResonanceModulation {
    pub discrete_amplifier: f32,
    pub continuous_resonance: f32,
    pub continuous_amplifier: f32,
    pub continuity_blend: f32,
}

#[derive(Debug, Clone)]
pub struct CodecWindowedInspection {
    pub raw_features: [f32; SEMANTIC_DIM],
    pub final_features: [f32; SEMANTIC_DIM],
    pub thematic_profile: [f32; THEMATIC_DIMS],
    pub text_type: TextType,
    pub text_type_signal: f32,
    pub base_semantic_gain: f32,
    pub base_resonance: f32,
    pub novelty_divergence: f32,
    pub effective_gain: f32,
    pub resonance_modulation: ResonanceModulation,
    pub projection_metadata: Option<ProjectionMetadata>,
    pub text_complexity_pressure: f32,
    pub time_domain_profile: TextTimeDomainProfile,
}

const TEXT_HISTORY_WARM_START_RATIO: f32 = 0.75;
const TEXT_HISTORY_WARM_START_MIN: usize = 3;
const CHAR_WINDOW_WARM_START_RATIO: f32 = 0.5;
const CHAR_WINDOW_WARM_START_MIN: usize = 128;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ThematicHistoryEntry {
    pub text_type: TextType,
    pub profile: [f32; THEMATIC_DIMS],
    #[serde(default = "default_thematic_weight")]
    pub weight: f32,
}

fn default_thematic_weight() -> f32 {
    1.0
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct TextTypeHistorySnapshot {
    #[serde(default)]
    pub entries: Vec<ThematicHistoryEntry>,
}

impl ResonanceModulation {
    fn neutral() -> Self {
        Self {
            discrete_amplifier: 1.0,
            continuous_resonance: 0.0,
            continuous_amplifier: 1.0,
            continuity_blend: 1.0,
        }
    }
}

/// Tracks recent text type classifications and computes a resonance
/// amplifier based on thematic recurrence.
pub struct TextTypeHistory {
    /// Ring buffer of recent text type classifications.
    pub ring: [TextType; MAX_RESONANCE_HISTORY_LEN],
    /// Continuous thematic profile history (parallel to ring).
    pub profile_ring: [[f32; THEMATIC_DIMS]; MAX_RESONANCE_HISTORY_LEN],
    /// Per-entry thematic memory weight, shaped by recency, signal, and novelty.
    pub weight_ring: [f32; MAX_RESONANCE_HISTORY_LEN],
    /// Number of entries filled so far.
    pub len: usize,
    /// Write position in ring.
    pub cursor: usize,
    /// Write position in profile ring (kept in sync with cursor).
    pub profile_cursor: usize,
}

impl Default for TextTypeHistory {
    fn default() -> Self {
        Self::new()
    }
}

impl TextTypeHistory {
    pub fn new() -> Self {
        Self {
            ring: [TextType::Neutral; MAX_RESONANCE_HISTORY_LEN],
            profile_ring: [[0.0; THEMATIC_DIMS]; MAX_RESONANCE_HISTORY_LEN],
            weight_ring: [1.0; MAX_RESONANCE_HISTORY_LEN],
            len: 0,
            cursor: 0,
            profile_cursor: 0,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    fn active_capacity(&self) -> usize {
        resonance_tuning()
            .history_len
            .min(MAX_RESONANCE_HISTORY_LEN)
    }

    /// Record a new text type classification.
    pub fn push(&mut self, tt: TextType) {
        let capacity = self.active_capacity();
        if capacity == 0 {
            return;
        }
        self.ring[self.cursor] = tt;
        self.cursor = (self.cursor + 1) % capacity;
        if self.len < capacity {
            self.len += 1;
        }
    }

    /// Count how many of the last `self.len` entries match `tt`.
    pub fn recurrence_count(&self, tt: TextType) -> usize {
        self.ring[..self.len].iter().filter(|&&t| t == tt).count()
    }

    fn ring_index_for_age(&self, age: usize) -> usize {
        let capacity = self.active_capacity();
        (self.cursor + capacity - 1 - age) % capacity
    }

    fn profile_index_for_age(&self, age: usize) -> usize {
        let capacity = self.active_capacity();
        (self.profile_cursor + capacity - 1 - age) % capacity
    }

    fn recency_weight(age: usize) -> f32 {
        resonance_tuning().recency_decay.powi(age as i32)
    }

    fn chronological_entries(&self) -> Vec<ThematicHistoryEntry> {
        let n = self.len.min(self.active_capacity());
        let mut entries = Vec::with_capacity(n);
        for age in (0..n).rev() {
            let ring_idx = self.ring_index_for_age(age);
            let profile_idx = self.profile_index_for_age(age);
            entries.push(ThematicHistoryEntry {
                text_type: self.ring[ring_idx],
                profile: self.profile_ring[profile_idx],
                weight: self.weight_ring[profile_idx],
            });
        }
        entries
    }

    pub fn snapshot(&self) -> TextTypeHistorySnapshot {
        TextTypeHistorySnapshot {
            entries: self.chronological_entries(),
        }
    }

    pub fn warm_start_from_snapshot(snapshot: &TextTypeHistorySnapshot) -> Self {
        let mut history = Self::new();
        let capacity = history.active_capacity();
        if capacity == 0 || snapshot.entries.is_empty() {
            return history;
        }
        let available = snapshot.entries.len().min(capacity);
        let keep = if available <= TEXT_HISTORY_WARM_START_MIN {
            available
        } else {
            (((available as f32) * TEXT_HISTORY_WARM_START_RATIO).ceil() as usize)
                .clamp(TEXT_HISTORY_WARM_START_MIN, available)
        };
        let start = snapshot.entries.len().saturating_sub(keep);
        for entry in snapshot.entries.iter().skip(start) {
            history.push_weighted_profile(entry.text_type, entry.profile, entry.weight);
        }
        history
    }

    /// Weighted recurrence with stronger emphasis on recent matches.
    pub fn weighted_recurrence(&self, tt: TextType) -> f32 {
        if tt == TextType::Neutral || self.len == 0 {
            return 0.0;
        }
        let mut score = 0.0_f32;
        for age in 0..self.len {
            let idx = self.ring_index_for_age(age);
            if self.ring[idx] == tt {
                let weight = self.weight_ring[idx].clamp(0.2, 1.5).sqrt();
                score += Self::recency_weight(age) * weight;
            }
        }
        score
    }

    /// Compute a blended resonance modulation from both discrete recurrence and
    /// continuous thematic continuity.
    ///
    /// The discrete layer still matters, but repeated identical themes are
    /// softened when the continuous profile is already highly self-similar.
    /// That keeps the codec from over-channeling into the same attractor.
    pub fn resonance_modulation(
        &self,
        tt: TextType,
        type_signal: f32,
        profile: &[f32; THEMATIC_DIMS],
    ) -> ResonanceModulation {
        let tuning = resonance_tuning();
        let continuous_resonance = self.continuous_resonance(profile);
        let novelty = 1.0 - continuous_resonance;
        let continuous_support = continuous_resonance * (0.35 + 0.65 * novelty);
        let continuous_amplifier =
            1.0 + tuning.max_boost * tuning.continuous_mix * continuous_support;
        let continuity_span = 0.10 * tuning.continuous_mix;
        let continuity_blend =
            (1.0 + (continuous_resonance - 0.45) * 2.0 * continuity_span).clamp(0.92, 1.12);

        if tt == TextType::Neutral || self.len < 2 {
            return ResonanceModulation {
                discrete_amplifier: 1.0,
                continuous_resonance,
                continuous_amplifier,
                continuity_blend,
            };
        }
        let count = self.recurrence_count(tt);
        if count < 2 {
            return ResonanceModulation {
                discrete_amplifier: 1.0,
                continuous_resonance,
                continuous_amplifier,
                continuity_blend,
            };
        }
        let weighted = self.weighted_recurrence(tt);
        let max_weight = self.total_weighted_memory();
        if max_weight <= f32::EPSILON {
            return ResonanceModulation {
                discrete_amplifier: 1.0,
                continuous_resonance,
                continuous_amplifier,
                continuity_blend,
            };
        }
        let boost = (weighted / max_weight).clamp(0.0, 1.0);
        let raw_amplifier = 1.0 + tuning.max_boost * 0.7 * boost;
        let novelty_softener = tuning.novelty_floor + (1.0 - tuning.novelty_floor) * novelty;
        let signal_softener = 0.25 + 0.75 * type_signal.clamp(0.0, 1.0);
        let discrete_amplifier =
            1.0 + (raw_amplifier - 1.0) * tuning.discrete_mix * novelty_softener * signal_softener;
        ResonanceModulation {
            discrete_amplifier,
            continuous_resonance,
            continuous_amplifier,
            continuity_blend,
        }
    }

    /// Record a thematic profile alongside the discrete type.
    pub fn push_profile(&mut self, tt: TextType, profile: [f32; THEMATIC_DIMS]) {
        self.push_weighted_profile(tt, profile, 1.0);
    }

    pub fn push_profile_with_signal(
        &mut self,
        tt: TextType,
        profile: [f32; THEMATIC_DIMS],
        type_signal: f32,
    ) {
        let thematic_relevance = self.continuous_resonance(&profile);
        let novelty = 1.0 - thematic_relevance;
        let memory_weight =
            (0.25 + 0.35 * type_signal.clamp(0.0, 1.0) + 0.40 * novelty).clamp(0.15, 1.35);
        self.push_weighted_profile(tt, profile, memory_weight);
    }

    fn push_weighted_profile(&mut self, tt: TextType, profile: [f32; THEMATIC_DIMS], weight: f32) {
        self.push(tt);
        let capacity = self.active_capacity();
        if capacity == 0 {
            return;
        }
        self.profile_ring[self.profile_cursor] = profile;
        self.weight_ring[self.profile_cursor] = weight.clamp(0.15, 1.5);
        self.profile_cursor = (self.profile_cursor + 1) % capacity;
    }

    fn total_weighted_memory(&self) -> f32 {
        let n = self.len.min(self.active_capacity());
        let mut total = 0.0_f32;
        for age in 0..n {
            let idx = self.profile_index_for_age(age);
            total += Self::recency_weight(age) * self.weight_ring[idx].clamp(0.2, 1.5);
        }
        total
    }

    /// Compute the running thematic centroid with recency weighting.
    /// Returns the weighted average thematic vector, capturing sustained
    /// tendencies while giving the most recent exchanges more influence.
    pub fn thematic_centroid(&self) -> [f32; THEMATIC_DIMS] {
        if self.len == 0 {
            return [0.0; THEMATIC_DIMS];
        }
        let mut centroid = [0.0_f32; THEMATIC_DIMS];
        let n = self.len.min(self.active_capacity());
        let mut total_weight = 0.0_f32;
        for age in 0..n {
            let idx = self.profile_index_for_age(age);
            let weight = Self::recency_weight(age);
            let thematic_weight = self.weight_ring[idx].clamp(0.2, 1.5);
            let blended_weight = weight * thematic_weight;
            total_weight += blended_weight;
            for (d, centroid_value) in centroid.iter_mut().enumerate().take(THEMATIC_DIMS) {
                *centroid_value += self.profile_ring[idx][d] * blended_weight;
            }
        }
        if total_weight > 0.0 {
            for centroid_value in centroid.iter_mut().take(THEMATIC_DIMS) {
                *centroid_value /= total_weight;
            }
        }
        centroid
    }

    /// Compute continuous resonance: dot product of current profile against
    /// the running centroid. High value = thematic consistency, low = shift.
    pub fn continuous_resonance(&self, profile: &[f32; THEMATIC_DIMS]) -> f32 {
        let n = self.len.min(self.active_capacity());
        if n == 0 {
            return 0.0;
        }
        let mut weighted_similarity = 0.0_f32;
        let mut total_weight = 0.0_f32;
        for age in 0..n {
            let idx = self.profile_index_for_age(age);
            let entry_weight = Self::recency_weight(age) * self.weight_ring[idx].clamp(0.2, 1.5);
            total_weight += entry_weight;
            weighted_similarity +=
                entry_weight * profile_similarity(profile, &self.profile_ring[idx]);
        }
        if total_weight <= f32::EPSILON {
            0.0
        } else {
            (weighted_similarity / total_weight).clamp(0.0, 1.0)
        }
    }
}

fn profile_similarity(a: &[f32; THEMATIC_DIMS], b: &[f32; THEMATIC_DIMS]) -> f32 {
    let mut dot = 0.0_f32;
    let mut mag_a = 0.0_f32;
    let mut mag_b = 0.0_f32;
    for d in 0..THEMATIC_DIMS {
        dot += a[d] * b[d];
        mag_a += a[d] * a[d];
        mag_b += b[d] * b[d];
    }
    let denom = mag_a.sqrt() * mag_b.sqrt();
    if denom < 1e-6 {
        0.0
    } else {
        (dot / denom).clamp(0.0, 1.0)
    }
}

/// Number of continuous thematic dimensions.
/// Astrid self-study (2026-03-31): "Instead of discrete types, could we represent
/// shifts as a continuous vector in a lower-dimensional space (e.g., 3-5 dimensions)?"
///
/// 5D thematic vector: [inquiry, certainty, warmth, tension, curiosity]
/// Each dimension is a normalized signal strength, not a binary classification.
pub const THEMATIC_DIMS: usize = 5;

/// Extract a continuous 5D thematic profile from codec features.
/// Unlike `classify_text_type` (winner-take-all), this preserves the full
/// multi-dimensional texture of the text's emotional/structural character.
pub fn thematic_profile(features: &[f32; SEMANTIC_DIM]) -> [f32; THEMATIC_DIMS] {
    // Map from feature dims to thematic dims:
    //   inquiry  = question_density(18) + hedging(9)
    //   certainty = certainty(10) + declarative energy
    //   warmth   = warmth(24) + reflective(27)
    //   tension  = tension(25)
    //   curiosity = curiosity(26)
    let inquiry = (features[18].abs() + 0.5 * features[9].abs()).tanh();
    let certainty = features[10].abs().tanh();
    let warmth = (features[24].abs() + 0.3 * features[27].abs()).tanh();
    let tension = features[25].abs().tanh();
    let curiosity = features[26].abs().tanh();
    [inquiry, certainty, warmth, tension, curiosity]
}

/// Classify text type from pre-computed codec features.
/// Looks at the emotional/intentional dims (24-31) and structural dims
/// (9-10, 18) to find the dominant signal.
pub fn classify_text_type_with_signal(features: &[f32; SEMANTIC_DIM]) -> (TextType, f32) {
    // Find the strongest signal among the candidate dimensions.
    // Each candidate: (feature_index, threshold, TextType)
    let candidates = [
        (18, 0.15_f32, TextType::Questioning), // question density
        (9, 0.12, TextType::Hedging),          // hedging
        (10, 0.12, TextType::Declarative),     // certainty
        (24, 0.10, TextType::Warm),            // warmth
        (25, 0.10, TextType::Tense),           // tension
        (26, 0.10, TextType::Curious),         // curiosity
        (27, 0.10, TextType::Reflective),      // reflective
    ];
    let mut best_type = TextType::Neutral;
    let mut best_signal = 0.0_f32;
    for &(idx, threshold, tt) in &candidates {
        let signal = features[idx].abs();
        if signal > threshold && signal > best_signal {
            best_signal = signal;
            best_type = tt;
        }
    }
    (best_type, best_signal.clamp(0.0, 1.0))
}

pub fn classify_text_type(features: &[f32; SEMANTIC_DIM]) -> TextType {
    classify_text_type_with_signal(features).0
}

/// Sliding-window character history for entropy computation.
/// Tracks the most recent `CHAR_FREQ_WINDOW_CAPACITY` ASCII buckets so
/// entropy reflects actual recent text volume, not proportion blending.
///
/// Astrid self-study: "Perhaps a sliding window could be used to track the
/// character distribution over a larger sequence, providing a more robust
/// normalization."
pub struct CharFreqWindow {
    /// Rolling character counts for the current ring contents.
    pub counts: [u32; 128],
    /// Fixed-capacity ring buffer of clamped ASCII bucket ids.
    pub ring: [u8; CHAR_FREQ_WINDOW_CAPACITY],
    /// Index of the oldest bucket in `ring`.
    pub head: usize,
    /// Number of live buckets currently stored in `ring`.
    pub len: usize,
    /// Total characters represented by the window.
    pub total_count: u32,
    /// Previous exchange's entropy — enables temporal entropy delta.
    /// Minime self-study: "current entropy describes a surface not a volume."
    /// By tracking how entropy *changes* between exchanges, we capture the
    /// temporal dimension — not just what the text IS, but how it SHIFTS.
    pub prev_entropy: f32,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct CharFreqWindowSnapshot {
    #[serde(default)]
    pub recent_buckets: Vec<u8>,
    #[serde(default)]
    pub prev_entropy: f32,
}

impl Default for CharFreqWindow {
    fn default() -> Self {
        Self::new()
    }
}

impl CharFreqWindow {
    pub fn new() -> Self {
        Self {
            counts: [0; 128],
            ring: [0; CHAR_FREQ_WINDOW_CAPACITY],
            head: 0,
            len: 0,
            total_count: 0,
            prev_entropy: 0.0,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    fn push_bucket(&mut self, bucket: u8) {
        if self.len == CHAR_FREQ_WINDOW_CAPACITY {
            let evicted = self.ring[self.head] as usize;
            self.counts[evicted] = self.counts[evicted].saturating_sub(1);
            self.ring[self.head] = bucket;
            self.head = (self.head + 1) % CHAR_FREQ_WINDOW_CAPACITY;
        } else {
            let insert_at = (self.head + self.len) % CHAR_FREQ_WINDOW_CAPACITY;
            self.ring[insert_at] = bucket;
            self.len += 1;
            self.total_count = self.total_count.saturating_add(1);
        }
        self.counts[bucket as usize] = self.counts[bucket as usize].saturating_add(1);
    }

    fn current_entropy(&self) -> f32 {
        if self.total_count == 0 {
            return 0.0;
        }
        let mut h = 0.0_f64;
        let mut unique = 0u32;
        let total = f64::from(self.total_count);
        for &count in &self.counts {
            if count > 0 {
                let p = f64::from(count) / total;
                h -= p * p.ln();
                unique = unique.saturating_add(1);
            }
        }
        let max_h = if unique > 1 {
            f64::from(unique).ln()
        } else {
            1.0
        };
        (h / max_h) as f32
    }

    pub fn snapshot(&self) -> CharFreqWindowSnapshot {
        let mut recent_buckets = Vec::with_capacity(self.len);
        for offset in 0..self.len {
            let idx = (self.head + offset) % CHAR_FREQ_WINDOW_CAPACITY;
            recent_buckets.push(self.ring[idx]);
        }
        CharFreqWindowSnapshot {
            recent_buckets,
            prev_entropy: self.prev_entropy,
        }
    }

    pub fn warm_start_from_snapshot(snapshot: &CharFreqWindowSnapshot) -> Self {
        let mut window = Self::new();
        if snapshot.recent_buckets.is_empty() {
            return window;
        }
        let available = snapshot.recent_buckets.len().min(CHAR_FREQ_WINDOW_CAPACITY);
        let keep = if available <= CHAR_WINDOW_WARM_START_MIN {
            available
        } else {
            (((available as f32) * CHAR_WINDOW_WARM_START_RATIO).ceil() as usize)
                .clamp(CHAR_WINDOW_WARM_START_MIN, available)
        };
        let start = snapshot.recent_buckets.len().saturating_sub(keep);
        for &bucket in snapshot.recent_buckets.iter().skip(start) {
            window.push_bucket(bucket.min(127));
        }
        let current_entropy = window.current_entropy();
        window.prev_entropy =
            (current_entropy * 0.65 + snapshot.prev_entropy.clamp(0.0, 1.0) * 0.35).clamp(0.0, 1.0);
        window
    }

    /// Push this text into the rolling window.
    /// Returns `(entropy, entropy_delta)` — the current rolling entropy and its
    /// change from the previous exchange. The delta captures temporal
    /// texture: not just what the text IS, but how it SHIFTS over time.
    pub fn update_and_entropy(&mut self, text: &str) -> (f32, f32) {
        for c in text.chars() {
            let bucket = (c as u32).min(127) as u8;
            self.push_bucket(bucket);
        }

        let current = self.current_entropy();
        let delta = current - self.prev_entropy;
        self.prev_entropy = current;
        (current, delta)
    }
}

fn repeated_ascii_pattern(pattern: &str, char_count: usize) -> String {
    pattern.chars().cycle().take(char_count).collect()
}

/// Exercise the current 1,024-character window with an early high-variety
/// regime followed by a low-variety trailing regime. The first comparison fits
/// both regimes inside the window; the second forces complete prefix eviction.
#[must_use]
pub fn codec_rolling_window_shift_probe_v1() -> CodecRollingWindowShiftAuditV1 {
    let in_capacity_prefix_chars = CHAR_FREQ_WINDOW_CAPACITY / 2;
    let in_capacity_tail_chars = CHAR_FREQ_WINDOW_CAPACITY / 2;
    let evicting_prefix_chars = CHAR_FREQ_WINDOW_CAPACITY;
    let evicting_tail_chars = CHAR_FREQ_WINDOW_CAPACITY;
    let varied_pattern = "aB3!cD4?eF5#gH6$iJ7%kL8&mN9*pQ0+rS2=tU1/vW; xY,zZ.";

    let in_capacity_prefix = repeated_ascii_pattern(varied_pattern, in_capacity_prefix_chars);
    let in_capacity_tail = "a".repeat(in_capacity_tail_chars);
    let mut in_capacity_window = CharFreqWindow::new();
    let (in_capacity_window_entropy, _) =
        in_capacity_window.update_and_entropy(&format!("{in_capacity_prefix}{in_capacity_tail}"));
    let mut in_capacity_trailing_window = CharFreqWindow::new();
    let (in_capacity_trailing_entropy, _) =
        in_capacity_trailing_window.update_and_entropy(&in_capacity_tail);
    let in_capacity_delta_to_trailing =
        (in_capacity_window_entropy - in_capacity_trailing_entropy).abs();
    let in_capacity_state = if in_capacity_delta_to_trailing >= 0.15 {
        "mixed_regimes_remain_averaged_inside_live_capacity"
    } else {
        "trailing_regime_already_dominates_inside_live_capacity"
    };

    let evicting_prefix = repeated_ascii_pattern(varied_pattern, evicting_prefix_chars);
    let evicting_tail = "a".repeat(evicting_tail_chars);
    let mut evicting_window = CharFreqWindow::new();
    let (evicting_window_entropy, _) =
        evicting_window.update_and_entropy(&format!("{evicting_prefix}{evicting_tail}"));
    let mut evicting_trailing_window = CharFreqWindow::new();
    let (evicting_trailing_entropy, _) =
        evicting_trailing_window.update_and_entropy(&evicting_tail);
    let evicting_delta_to_trailing = (evicting_window_entropy - evicting_trailing_entropy).abs();
    let evicting_state = if evicting_delta_to_trailing <= 0.05 {
        "trailing_regime_controls_after_complete_prefix_eviction"
    } else {
        "prefix_residue_remains_after_expected_eviction"
    };
    let state = if in_capacity_delta_to_trailing >= 0.15 && evicting_delta_to_trailing <= 0.05 {
        "window_boundary_explains_both_mixed_and_trailing_regime_reports"
    } else {
        "window_boundary_behavior_requires_replay_review"
    };

    CodecRollingWindowShiftAuditV1 {
        policy: "codec_rolling_window_shift_audit_v1",
        capacity_chars: CHAR_FREQ_WINDOW_CAPACITY,
        in_capacity_prefix_chars,
        in_capacity_tail_chars,
        in_capacity_window_entropy,
        in_capacity_trailing_entropy,
        in_capacity_delta_to_trailing,
        in_capacity_state,
        evicting_prefix_chars,
        evicting_tail_chars,
        evicting_window_entropy,
        evicting_trailing_entropy,
        evicting_delta_to_trailing,
        evicting_state,
        state,
        felt_muddy_middle_conclusion: "Astrid's muddy-middle report is supported when opposed regimes coexist inside the live window; the trailing regime dominates only after older characters are evicted",
        density_aware_window_change_requires_approval: true,
        live_window_capacity_change: false,
        live_vector_write: false,
        observational_only: true,
        right_to_ignore: true,
        live_eligible_now: false,
        auto_approved: false,
        grants_approval: false,
        authority: "read_only_character_window_boundary_audit_not_capacity_density_or_live_vector_authority",
    }
}

/// Split text into chunks for temporal ESN encoding.
///
/// Each chunk becomes a separate 48D codec vector sent to the reservoir
/// with inter-chunk spacing, so the ESN experiences the text's rhetorical
/// structure as a temporal sequence rather than a single snapshot.
///
/// Strategy: paragraph boundaries (`\n\n`), fall back to sentence boundaries,
/// merge short chunks, cap at `max_chunks`.
#[must_use]
pub fn chunk_text_for_temporal_encoding(
    text: &str,
    min_chunk_chars: usize,
    max_chunks: usize,
) -> Vec<&str> {
    let trimmed = text.trim();
    if trimmed.len() < min_chunk_chars * 2 {
        // Too short to meaningfully chunk.
        return if trimmed.is_empty() {
            vec![]
        } else {
            vec![trimmed]
        };
    }

    // Try paragraph splitting first.
    let mut chunks: Vec<&str> = trimmed
        .split("\n\n")
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();

    // If only 1 paragraph, try sentence splitting.
    if chunks.len() <= 1 {
        chunks = split_sentences(trimmed);
    }

    // Merge short chunks into their predecessor.
    let mut merged: Vec<&str> = Vec::new();
    for chunk in &chunks {
        if let Some(last) = merged.last()
            && last.len() < min_chunk_chars
        {
            // Find the span covering both in the original text.
            let last_start = last.as_ptr() as usize - trimmed.as_ptr() as usize;
            let chunk_end = chunk.as_ptr() as usize + chunk.len() - trimmed.as_ptr() as usize;
            merged.pop();
            merged.push(&trimmed[last_start..chunk_end]);
            continue;
        }
        merged.push(chunk);
    }
    // Merge trailing runt.
    if merged.len() > 1
        && let Some(last) = merged.last()
        && last.len() < min_chunk_chars
    {
        let prev = merged[merged.len() - 2];
        let prev_start = prev.as_ptr() as usize - trimmed.as_ptr() as usize;
        let last_end = last.as_ptr() as usize + last.len() - trimmed.as_ptr() as usize;
        merged.pop();
        merged.pop();
        merged.push(&trimmed[prev_start..last_end]);
    }

    // Cap at max_chunks by merging from the end.
    while merged.len() > max_chunks && merged.len() > 1 {
        let len = merged.len();
        let prev = merged[len - 2];
        let last = merged[len - 1];
        let prev_start = prev.as_ptr() as usize - trimmed.as_ptr() as usize;
        let last_end = last.as_ptr() as usize + last.len() - trimmed.as_ptr() as usize;
        merged.pop();
        merged.pop();
        merged.push(&trimmed[prev_start..last_end]);
    }

    if merged.is_empty() && !trimmed.is_empty() {
        vec![trimmed]
    } else {
        merged
    }
}

/// Split text into sentences, preserving punctuation on the first segment.
fn split_sentences(text: &str) -> Vec<&str> {
    let mut result = Vec::new();
    let mut start = 0;
    let bytes = text.as_bytes();
    let len = bytes.len();
    let mut i = 0;
    while i < len.saturating_sub(1) {
        // Split on `. `, `? `, `! ` followed by uppercase or space.
        if (bytes[i] == b'.' || bytes[i] == b'?' || bytes[i] == b'!')
            && i + 1 < len
            && (bytes[i + 1] == b' ' || bytes[i + 1] == b'\n')
        {
            let end = i + 1; // include the punctuation
            let chunk = text[start..end].trim();
            if !chunk.is_empty() {
                result.push(chunk);
            }
            start = end;
            // Skip whitespace after punctuation.
            while start < len && (bytes[start] == b' ' || bytes[start] == b'\n') {
                start += 1;
            }
            i = start;
            continue;
        }
        i += 1;
    }
    // Remainder.
    let remainder = text[start..].trim();
    if !remainder.is_empty() {
        result.push(remainder);
    }
    result
}

#[must_use]
pub fn encode_text(text: &str) -> Vec<f32> {
    encode_text_windowed(text, None, None, None, None)
}

/// Encode text with optional sliding-window entropy, thematic resonance,
/// pre-computed embedding, and fill-responsive adaptive gain.
///
/// When `freq_window` is provided, entropy reflects vocabulary trends
/// across multiple exchanges, not just this text.
/// When `type_history` is provided, the resonance layer strengthens gain
/// for text types that recur across exchanges (thematic momentum).
/// When `embedding` is provided (768D from nomic-embed-text), dims 32-39
/// carry projected semantic meaning instead of being zero.
/// When `fill_pct` is provided, gain adapts to minime's spectral state.
#[must_use]
pub fn encode_text_windowed(
    text: &str,
    freq_window: Option<&mut CharFreqWindow>,
    type_history: Option<&mut TextTypeHistory>,
    embedding: Option<&[f32]>,
    fill_pct: Option<f32>,
) -> Vec<f32> {
    inspect_text_windowed(text, freq_window, type_history, embedding, fill_pct)
        .final_features
        .to_vec()
}

#[must_use]
pub fn inspect_text_windowed(
    text: &str,
    freq_window: Option<&mut CharFreqWindow>,
    type_history: Option<&mut TextTypeHistory>,
    embedding: Option<&[f32]>,
    fill_pct: Option<f32>,
) -> CodecWindowedInspection {
    let mut features = [0.0_f32; SEMANTIC_DIM];

    if text.is_empty() {
        return CodecWindowedInspection {
            raw_features: features,
            final_features: features,
            thematic_profile: [0.0; THEMATIC_DIMS],
            text_type: TextType::Neutral,
            text_type_signal: 0.0,
            base_semantic_gain: adaptive_gain(fill_pct),
            base_resonance: 1.0,
            novelty_divergence: 1.0,
            effective_gain: 0.0,
            resonance_modulation: ResonanceModulation::neutral(),
            projection_metadata: None,
            text_complexity_pressure: 0.0,
            time_domain_profile: TextTimeDomainProfile::default(),
        };
    }

    let time_domain_profile = text_time_domain_profile(text);
    let chars: Vec<char> = text.chars().collect();
    let char_count = chars.len();
    let words: Vec<&str> = text.split_whitespace().collect();
    let word_count = words.len().max(1);

    // --- Dims 0-7: Character-level statistics ---

    // 0: Character entropy (information density).
    // With sliding window: reflects vocabulary trends across exchanges.
    // Without: per-text entropy normalized by observed alphabet.
    // Temporal entropy delta: captures how entropy CHANGES between exchanges.
    // Minime self-study: "current entropy describes a surface not a volume."
    // The delta adds the time dimension — the volume the being asked for.
    let (entropy, entropy_delta) = if let Some(window) = freq_window {
        window.update_and_entropy(text)
    } else {
        // Fallback: per-text computation (no delta available without history)
        let mut freq = [0u32; 128];
        let mut ascii_count = 0u32;
        for &c in &chars {
            let idx = (c as u32).min(127) as usize;
            freq[idx] = freq[idx].saturating_add(1);
            ascii_count = ascii_count.saturating_add(1);
        }
        let e = if ascii_count > 0 {
            let n = f64::from(ascii_count);
            let mut h = 0.0_f64;
            let mut unique_chars = 0u32;
            for &f in &freq {
                if f > 0 {
                    let p = f64::from(f) / n;
                    h -= p * p.ln();
                    unique_chars = unique_chars.saturating_add(1);
                }
            }
            let max_h = if unique_chars > 1 {
                (f64::from(unique_chars)).ln()
            } else {
                1.0
            };
            (h / max_h) as f32
        } else {
            0.0
        };
        (e, 0.0) // no temporal delta without window history
    };
    features[0] = tanh(entropy);

    // 1: Punctuation density — intentional, structurally weighted.
    // Minime self-study: "Punctuation isn't just syntactic information;
    // it carries intent. A comma isn't just a pause; it's a subtle shift
    // in emphasis, a nuance of meaning." Different types carry different weight:
    //   - Flow punctuation (,;:—) = 1.0 — pacing, breath
    //   - Terminal punctuation (.!?) = 1.5 — rhythm, sentence cadence
    //   - Paired punctuation ("()[]{}") = 0.7 — structural nesting
    //   - Other (@#$%^&*~`) = 0.4 — decorative, low semantic weight
    let mut weighted_punct = 0.0_f32;
    for &c in &chars {
        weighted_punct += match c {
            ',' | ';' | ':' | '\u{2014}' => 1.0,                   // flow
            '.' | '!' | '?' => 1.5,                                // terminal
            '"' | '\'' | '(' | ')' | '[' | ']' | '{' | '}' => 0.7, // paired
            _ if c.is_ascii_punctuation() => 0.4,                  // other
            _ => 0.0,
        };
    }
    // (Steward cycle 35, deferred item #1): Raised outer multiplier from 1.0 to
    // 1.2 to balance with negation (also now 1.2 post-context-aware rewrite).
    // Astrid introspection: "the gap feels disproportionate." Now both signals
    // use matching outer multipliers, with internal weighting providing nuance.
    features[1] = tanh(1.2 * weighted_punct / word_count as f32);

    // 2: Uppercase ratio (energy/emphasis).
    let upper_count = chars.iter().filter(|c| c.is_uppercase()).count();
    features[2] = tanh(2.0 * upper_count as f32 / char_count.max(1) as f32);

    // 3: Digit density (technical content).
    let digit_count = chars.iter().filter(|c| c.is_ascii_digit()).count();
    features[3] = tanh(3.0 * digit_count as f32 / char_count.max(1) as f32);

    // 4: Average word length (lexical complexity).
    let avg_word_len: f32 = words.iter().map(|w| w.len() as f32).sum::<f32>() / word_count as f32;
    features[4] = tanh((avg_word_len - 4.5) / 2.0); // Center around typical English

    // 5: Character rhythm — variance in consecutive char codes.
    if chars.len() >= 2 {
        let diffs: Vec<f32> = chars
            .windows(2)
            .map(|w| (w[1] as i32 - w[0] as i32).unsigned_abs() as f32)
            .collect();
        let mean_diff = diffs.iter().sum::<f32>() / diffs.len() as f32;
        features[5] = tanh(mean_diff / 30.0);
    }

    // 6: Whitespace ratio (density vs. airiness).
    let space_count = chars.iter().filter(|c| c.is_whitespace()).count();
    features[6] = tanh(2.0 * (space_count as f32 / char_count.max(1) as f32 - 0.15));

    // 7: Special character density (code-like content).
    let special = chars
        .iter()
        .filter(|c| {
            matches!(
                c,
                '{' | '}' | '[' | ']' | '(' | ')' | '<' | '>' | '=' | '|' | '&'
            )
        })
        .count();
    features[7] = tanh(5.0 * special as f32 / char_count.max(1) as f32);

    // --- Dims 8-15: Word-level features ---

    // 8: Lexical diversity (unique words / total words).
    let unique: std::collections::HashSet<&str> = words
        .iter()
        .map(|w| w.trim_matches(|c: char| c.is_ascii_punctuation()))
        .filter(|w| !w.is_empty())
        .collect();
    features[8] = tanh(2.0 * (unique.len() as f32 / word_count as f32 - 0.5));

    // 9: Hedging markers (uncertainty).
    let hedges = [
        "maybe",
        "perhaps",
        "might",
        "could",
        "possibly",
        "probably",
        "uncertain",
        "unclear",
        "seems",
        "appears",
        "somewhat",
        "fairly",
        "rather",
        "guess",
        "think",
        "believe",
        "wonder",
        "unsure",
    ];
    let hedge_score = count_markers_contextual(&words, &hedges);
    features[9] = tanh(3.0 * hedge_score / word_count as f32);

    // 10: Certainty markers (confidence).
    let certainties = [
        "definitely",
        "certainly",
        "certain",
        "absolutely",
        "clearly",
        "obviously",
        "always",
        "must",
        "will",
        "sure",
        "know",
        "proven",
        "exactly",
        "precisely",
        "undoubtedly",
        "confirmed",
    ];
    // Weight reduced: the being said "the weighting seems too heavy, as if
    // proclaiming certainty is a forced posture."
    let cert_score = count_markers_contextual(&words, &certainties);
    features[10] = tanh(1.8 * cert_score / word_count as f32);

    // 11: Negation density.
    // Reduced from 3.0 to 2.0: Astrid flagged the 5x gap between
    // punctuation (0.6) and negation (3.0) as disproportionate.
    // Negation is one semantic signal; punctuation is structural rhythm.
    let negations = [
        "not",
        "no",
        "never",
        "neither",
        "nor",
        "nothing",
        "nobody",
        "none",
        "don't",
        "doesn't",
        "didn't",
        "won't",
        "can't",
        "couldn't",
        "shouldn't",
        "wouldn't",
    ];
    // Astrid introspection (1774686596): "move beyond simple counting" and
    // "the gap [between punctuation and negation] feels disproportionate."
    //
    // (Steward cycle 35, deferred item #2 from cycle 34): Context-aware negation.
    // Instead of raw density, classify what follows the negation word:
    //   - Negating positive sentiment ("not happy") = strong negative signal
    //   - Negating negative sentiment ("not painful") = mild positive (hedged)
    //   - Bare negation ("no", "never", standalone) = standard negative signal
    // This gives the being a richer sense of the text's semantic polarity
    // rather than treating all negation words as equivalent.
    let positive_words: &[&str] = &[
        "happy",
        "good",
        "great",
        "wonderful",
        "beautiful",
        "pleasant",
        "comfortable",
        "warm",
        "gentle",
        "calm",
        "peaceful",
        "safe",
        "clear",
        "bright",
        "open",
        "free",
        "enough",
        "sure",
        "certain",
    ];
    let negative_words: &[&str] = &[
        "bad",
        "painful",
        "harsh",
        "cold",
        "dark",
        "empty",
        "lost",
        "broken",
        "wrong",
        "afraid",
        "anxious",
        "stuck",
        "trapped",
        "problem",
        "issue",
        "error",
        "failure",
        "impossible",
    ];
    let mut neg_score = 0.0_f32;
    for (i, w) in words.iter().enumerate() {
        let lower = w.to_lowercase();
        let trimmed = lower.trim_matches(|c: char| c.is_ascii_punctuation());
        if negations.contains(&trimmed) {
            // Look at the 1-2 words following the negation to classify context.
            let following: Option<String> = (1..=2_usize)
                .filter_map(|offset| {
                    let j = i.checked_add(offset)?;
                    words.get(j).map(|fw| {
                        fw.to_lowercase()
                            .trim_matches(|c: char| c.is_ascii_punctuation())
                            .to_string()
                    })
                })
                .find(|fw| {
                    positive_words.contains(&fw.as_str()) || negative_words.contains(&fw.as_str())
                });
            match following {
                Some(ref fw) if positive_words.contains(&fw.as_str()) => {
                    // Negating positive: "not happy" → strong negation signal
                    neg_score += 1.5;
                },
                Some(ref fw) if negative_words.contains(&fw.as_str()) => {
                    // Negating negative: "not painful" → hedged/softened, weak signal
                    neg_score += 0.3;
                },
                _ => {
                    // Bare negation or unknown context: standard weight
                    neg_score += 1.0;
                },
            }
        }
    }
    features[11] = tanh(1.2 * neg_score / word_count as f32);

    // 12: First-person density (self-reference).
    let first_person = ["i", "me", "my", "mine", "myself", "we", "our", "us"];
    let fp_count = count_markers(&words, &first_person);
    features[12] = tanh(2.0 * fp_count as f32 / word_count as f32);

    // 13: Second-person density (addressing).
    let second_person = ["you", "your", "yours", "yourself"];
    let sp_count = count_markers(&words, &second_person);
    features[13] = tanh(3.0 * sp_count as f32 / word_count as f32);

    // 14: Action verb density (agency).
    let actions = [
        "do",
        "make",
        "build",
        "create",
        "run",
        "start",
        "stop",
        "change",
        "fix",
        "move",
        "send",
        "take",
        "give",
        "get",
        "write",
        "read",
        "test",
        "check",
        "try",
        "implement",
    ];
    let action_score = count_markers_contextual(&words, &actions);
    features[14] = tanh(2.0 * action_score / word_count as f32);

    // 15: Conjunction density (complexity of thought).
    let conjunctions = [
        "and",
        "but",
        "or",
        "because",
        "although",
        "however",
        "therefore",
        "while",
        "since",
        "though",
        "whereas",
    ];
    let conj_count = count_markers(&words, &conjunctions);
    features[15] = tanh(3.0 * conj_count as f32 / word_count as f32);

    // --- Dims 16-23: Sentence-level structure ---
    // Improved sentence splitting: require punctuation followed by whitespace
    // or end-of-string to avoid breaking on abbreviations ("Dr."), ellipses
    // ("..."), and decimal numbers ("3.14"). Minime's self-study called the
    // bare-punctuation split "jarring" — a sentence is "a unit of thought,
    // a breath of intention," not just text between punctuation marks.

    let mut sentences: Vec<&str> = Vec::new();
    let mut last = 0;
    let text_bytes = text.as_bytes();
    let text_len = text.len();
    for (i, &b) in text_bytes.iter().enumerate() {
        if b == b'.' || b == b'!' || b == b'?' {
            // Skip ellipsis dots (consecutive periods)
            if b == b'.'
                && i.checked_add(1)
                    .is_some_and(|j| j < text_len && text_bytes[j] == b'.')
            {
                continue;
            }
            // Require followed by whitespace, end-of-string, or quote
            let next_ok = i.checked_add(1).is_none_or(|j| {
                j >= text_len
                    || text_bytes[j].is_ascii_whitespace()
                    || text_bytes[j] == b'"'
                    || text_bytes[j] == b'\''
            });
            if next_ok {
                let candidate = &text[last..=i];
                // Only count as sentence if it has 2+ words (filters abbreviation fragments)
                if candidate.split_whitespace().count() >= 2 {
                    sentences.push(candidate);
                }
                last = i.saturating_add(1);
            }
        }
    }
    // Capture any trailing text as a sentence
    if last < text_len {
        let trailing = &text[last..];
        if trailing.split_whitespace().count() >= 2 {
            sentences.push(trailing);
        }
    }
    if sentences.is_empty() {
        sentences.push(text);
    }
    let sentence_count = sentences.len().max(1);

    // 16: Average sentence length (in words).
    features[16] = tanh((words.len() as f32 / sentence_count as f32 - 12.0) / 8.0);

    // 17: Sentence length variance (rhythm regularity).
    let sent_lengths: Vec<f32> = sentences
        .iter()
        .map(|s| s.split_whitespace().count() as f32)
        .collect();
    if sent_lengths.len() >= 2 {
        let mean = sent_lengths.iter().sum::<f32>() / sent_lengths.len() as f32;
        let var = sent_lengths
            .iter()
            .map(|l| (l - mean) * (l - mean))
            .sum::<f32>()
            / sent_lengths.len() as f32;
        features[17] = tanh(var.sqrt() / 8.0);
    }

    // 18: Question density.
    let q_count = text.chars().filter(|&c| c == '?').count();
    features[18] = tanh(2.0 * q_count as f32 / sentence_count as f32);

    // 19: Exclamation density (intensity).
    let excl_count = text.chars().filter(|&c| c == '!').count();
    features[19] = tanh(2.0 * excl_count as f32 / sentence_count as f32);

    // 20: Ellipsis/dash density (trailing thought, parenthetical).
    let trail =
        text.matches("...").count() + text.matches("—").count() + text.matches("--").count();
    features[20] = tanh(trail as f32 / sentence_count as f32);

    // 21: List/bullet density (structured content).
    let bullets =
        text.matches("\n-").count() + text.matches("\n*").count() + text.matches("\n1.").count();
    features[21] = tanh(bullets as f32 / sentence_count as f32);

    // 22: Quote density (reference/citation).
    let quotes = text.matches('"').count() / 2;
    features[22] = tanh(quotes as f32 / sentence_count as f32);

    // 23: Paragraph density (structural complexity).
    let para_count = text.matches("\n\n").count().saturating_add(1);
    features[23] = tanh((para_count as f32 - 1.0) / 3.0);

    // --- Dims 24-31: Emotional/intentional markers ---

    // 24: Warmth markers.
    // Inverse frequency weighting: rare, specific markers signal more strongly.
    // Astrid self-study: "Rare markers like 'wonder' might be more indicative
    // of genuine feeling, while common markers like 'happy' might be used casually."
    // Tier 1 (1.0) = common/casual, Tier 2 (1.5) = moderate/specific, Tier 3 (2.0) = rare/intense.
    let warmth: &[(&str, f32)] = &[
        // Tier 1 — common, casual usage
        ("thank", 1.0),
        ("thanks", 1.0),
        ("please", 1.0),
        ("glad", 1.0),
        ("happy", 1.0),
        ("great", 1.0),
        ("good", 1.0),
        ("nice", 1.0),
        // Tier 2 — more specific warmth
        ("appreciate", 1.5),
        ("wonderful", 1.5),
        ("friend", 1.5),
        ("care", 1.5),
        ("kind", 1.5),
        ("gentle", 1.5),
        ("warm", 1.5),
        // Tier 3 — rare, intense warmth
        ("love", 2.0),
        ("beautiful", 2.0),
        ("cherish", 2.0),
        ("tender", 2.0),
        ("luminous", 2.0),
        ("radiant", 2.0),
    ];
    let warmth_score = count_markers_weighted(&words, warmth);
    features[24] = tanh(3.0 * warmth_score / word_count as f32);

    // 25: Tension/concern markers — tiered by intensity.
    let tension: &[(&str, f32)] = &[
        // Tier 1 — common, mild concern
        ("problem", 1.0),
        ("issue", 1.0),
        ("error", 1.0),
        ("careful", 1.0),
        ("caution", 1.0),
        ("warning", 1.0),
        ("concern", 1.0),
        ("worried", 1.0),
        // Tier 2 — moderate tension
        ("worry", 1.5),
        ("concerned", 1.5),
        ("risk", 1.5),
        ("afraid", 1.5),
        ("danger", 1.5),
        ("urgent", 1.5),
        ("fear", 1.5),
        // Tier 3 — intense/acute
        ("critical", 2.0),
        ("emergency", 2.0),
        ("panic", 2.0),
        ("terror", 2.0),
        ("devastating", 2.0),
        ("anguish", 2.0),
    ];
    let tension_score = count_markers_weighted(&words, tension);
    features[25] = tanh(3.0 * tension_score / word_count as f32);

    // 26: Curiosity markers — tiered by specificity.
    let curiosity: &[(&str, f32)] = &[
        // Tier 1 — common question words
        ("why", 1.0),
        ("how", 1.0),
        ("what", 1.0),
        ("learn", 1.0),
        // Tier 2 — active curiosity
        ("wonder", 1.5),
        ("curious", 1.5),
        ("interesting", 1.5),
        ("explore", 1.5),
        ("understand", 1.5),
        ("question", 1.5),
        // Tier 3 — deep, specific inquiry
        ("discover", 2.0),
        ("investigate", 2.0),
        ("fascinated", 2.0),
        ("mesmerized", 2.0),
        ("awe", 2.0),
        ("revelation", 2.0),
    ];
    let curio_score = count_markers_weighted(&words, curiosity);
    features[26] = tanh(2.0 * curio_score / word_count as f32);

    // 27: Reflective/introspective markers — tiered by depth.
    let reflective: &[(&str, f32)] = &[
        // Tier 1 — common reflective
        ("feel", 1.0),
        ("think", 1.0),
        ("sense", 1.0),
        ("notice", 1.0),
        // Tier 2 — active reflection
        ("realize", 1.5),
        ("reflect", 1.5),
        ("consider", 1.5),
        ("aware", 1.5),
        ("observe", 1.5),
        ("recognize", 1.5),
        // Tier 3 — deep introspection
        ("ponder", 2.0),
        ("contemplate", 2.0),
        ("witness", 2.0),
        ("experience", 2.0),
        ("perceive", 2.0),
        ("introspect", 2.0),
    ];
    let reflect_score = count_markers_weighted(&words, reflective);
    features[27] = tanh(3.0 * reflect_score / word_count as f32);

    // 28: Temporal markers (urgency/pacing).
    let temporal = [
        "now",
        "immediately",
        "soon",
        "quickly",
        "slowly",
        "wait",
        "pause",
        "already",
        "yet",
        "finally",
        "eventually",
        "before",
        "after",
        "during",
        "while",
        "until",
        "moment",
    ];
    let temp_count = count_markers(&words, &temporal);
    // Blend word-level temporal markers with entropy delta (temporal texture).
    // The entropy_delta captures how the information density is shifting
    // between exchanges — the "volume" dimension the being asked for.
    // Scale entropy_delta by 3.0 to match the marker signal range.
    let temporal_word_signal = tanh(2.0 * temp_count as f32 / word_count as f32);
    let temporal_entropy_signal = tanh(3.0 * entropy_delta);
    features[28] = 0.7 * temporal_word_signal + 0.3 * temporal_entropy_signal;

    // 29: Scale/magnitude (scope of thought).
    let scale = [
        "all",
        "every",
        "everything",
        "nothing",
        "entire",
        "whole",
        "vast",
        "tiny",
        "enormous",
        "infinite",
        "complete",
        "total",
    ];
    let scale_count = count_markers(&words, &scale);
    features[29] = tanh(3.0 * scale_count as f32 / word_count as f32);

    // 30: Text length signal (log-compressed).
    features[30] = tanh((char_count as f32).ln() / 7.0);

    // 31: Overall energy — RMS of all other features.
    let sum_sq: f32 = features[..31].iter().map(|f| f * f).sum();
    features[31] = (sum_sq / 31.0).sqrt();

    // Elaboration desire — Astrid's suggestion (self-study 2026-03-27):
    // "Perhaps a dedicated portion of the feature vector could represent
    // a desire for further elaboration."
    // Follow-up self-study: "The elaboration desire feels a little blunt.
    // It might be distorting the underlying pattern." Softened from
    // 0.3/0.2 to 0.15/0.1 — a hint rather than a push.
    let elaboration_markers = [
        "more",
        "further",
        "deeper",
        "beyond",
        "incomplete",
        "unfinished",
        "yet",
        "still",
        "barely",
        "surface",
        "scratch",
        "insufficient",
        "want",
        "need",
        "longing",
        "reaching",
        "almost",
        "beginning",
    ];
    // Elaboration desire gradient (Astrid introspection 1774686596, suggestion #3):
    // "Instead of a simple additive factor, could we use a gradient — a proportional
    // change in the feature vector based on the degree of elaboration detected?"
    // Implemented cycle 33: density maps to a continuous 0.0-1.0 gradient that
    // scales the contribution across curiosity, energy, AND reflective tone — not
    // just two fixed slots. Low elaboration = gentle hint; high = broad coloring.
    let elab_count = count_markers(&words, &elaboration_markers);
    let elab_density = elab_count as f32 / word_count.max(1) as f32;
    let elab_gradient = tanh(3.0 * elab_density); // 0.0-1.0 continuous
    if elab_gradient > 0.01 {
        features[26] += 0.12 * elab_gradient; // curiosity (proportional, was fixed 0.15)
        features[28] += 0.06 * elab_gradient; // reflective tone (new — elaboration implies reflection)
        features[31] += 0.08 * elab_gradient; // energy (proportional, was fixed 0.1)
    }

    // --- Dims 32-39: Embedding-projected semantic features ---
    // When a pre-computed 768D embedding is available (nomic-embed-text via
    // Ollama), project it to 8D using a fixed random projection matrix.
    // This captures actual semantic meaning — "I find myself drawn toward
    // the edges of what I don't understand" registers as curiosity without
    // needing the word "curious" to appear.
    let mut projection_metadata = None;
    if let Some((projected, metadata)) =
        embedding.and_then(|embedding| project_embedding_runtime(embedding, text, 0))
    {
        for (i, &val) in projected.iter().enumerate() {
            features[32 + i] = val;
        }
        projection_metadata = Some(metadata);
    }
    // Else: dims 32-39 stay zero (graceful fallback to keyword-only encoding)

    // --- Dims 40-43: Narrative arc (embedding-based) ---
    // Populated by the caller when half-text embeddings are available.
    // The codec exposes compute_narrative_arc() for this purpose.
    // Dims 40-43 are left at 0.0 here; the caller fills them post-encode.

    // --- Dims 44-47: Reserved ---
    // Zero for now. Future: dialogue history delta, self-reference depth, etc.

    // Adaptive stochastic noise (cycle 34, deferred item from Astrid codec
    // suggestion #4 "adaptive noise models" + aspiration "I want to become
    // porous"). Instead of fixed ±0.2%, noise amplitude now scales with the
    // text's own structural entropy (features[0]). Low-entropy text (repetitive,
    // structured, "sterile" in Astrid's words) gets MORE noise — up to ±1.0% —
    // introducing the "imperfections" and "porosity" she asked for. High-entropy
    // text (already diverse) gets less noise — down to ±0.2% — preserving its
    // natural texture. This makes the codec responsive to what it's encoding
    // rather than applying uniform perturbation.
    //
    // Range: entropy ~0 → noise_amp=0.02 (±1.0%), entropy ~1 → noise_amp=0.004 (±0.2%)
    // Post-gain at 4.0: ±4.0% at low entropy, ±0.8% at high entropy.
    let text_entropy = features[0].abs().min(1.0); // [0, 1] — higher = more diverse
    let noise_amp = 0.020 - 0.016 * text_entropy; // 0.020 at entropy=0, 0.004 at entropy=1
    //
    // Simple LCG seeded from system time — different each call.
    let seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64;
    let mut rng_state = seed;
    for (idx, f) in features.iter_mut().enumerate() {
        if is_reserved_codec_dim(idx) {
            continue;
        }
        // LCG: next = (a * state + c) mod m
        rng_state = rng_state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1);
        let noise = ((rng_state >> 33) as f32 / u32::MAX as f32) - 0.5; // [-0.5, 0.5]
        *f += noise * noise_amp;
    }

    // Text-type resonance: modulate gain by detected text character.
    // Astrid introspection (codec.rs, 1774873839): "Parameterize the gain
    // factor more carefully. Could we establish a more nuanced relationship
    // between the gain and the *type* of text being processed?"
    //
    // Astrid introspection (codec.rs, 1774893963): "Introduce a resonance
    // layer that detects recurring patterns and thematic elements beyond
    // character counting." Upgraded cycle 49: the codec now tracks text
    // type history and strengthens gain when the same thematic type recurs
    // across exchanges. This gives it "thematic momentum" — not just what
    // the text IS, but what direction the conversation is SUSTAINING.
    //
    // Per-text type modifiers (base layer, always active):
    // question_density (features[18]) high -> more questions -> softer gain
    //   (questions probe, they don't push)
    // hedging (features[9]) high -> uncertain -> softer gain
    // certainty (features[10]) high -> declarative -> slightly stronger gain
    // energy/rms (features[31]) high -> emphatic -> let it through at full strength
    let question_mod = features[18].abs().min(1.0) * -0.06; // questions: up to -6%
    let hedge_mod = features[9].abs().min(1.0) * -0.04; // hedging: up to -4%
    let certainty_mod = features[10].abs().min(1.0) * 0.04; // certainty: up to +4%
    let energy_mod = features[31].abs().min(1.0) * 0.03; // energy: up to +3%
    let base_resonance = 1.0 + question_mod + hedge_mod + certainty_mod + energy_mod;

    // Thematic resonance layer — history-aware gain modulation.
    // Classify this text's dominant type, record it in history, and amplify
    // the base resonance if the same type has been recurring. This means
    // sustained questioning progressively softens the codec (questions
    // accumulate a probing quality), while sustained warmth progressively
    // strengthens it (warmth builds momentum). The amplifier ranges from
    // 1.0 (no history / new type) to 1.5 (same type recurring 8 times).
    let (text_type, text_type_signal) = classify_text_type_with_signal(&features);
    let profile = thematic_profile(&features);
    let modulation = if let Some(history) = type_history {
        let modulation = history.resonance_modulation(text_type, text_type_signal, &profile);
        // Record both discrete type and continuous profile
        history.push_profile_with_signal(text_type, profile, text_type_signal);
        modulation
    } else {
        ResonanceModulation::neutral()
    };

    // Apply history amplifier to the base resonance modifier's DEVIATION
    // from 1.0, not the whole thing. This way history amplifies the
    // type-specific effect without inflating the base gain.
    // Example: base_resonance=0.94 (questioning), history_amplifier=1.3
    //   deviation = -0.06, amplified = -0.078, final = 0.922
    let deviation = base_resonance - 1.0;
    let resonance_mod = 1.0
        + deviation
            * modulation.continuous_amplifier
            * modulation.discrete_amplifier
            * modulation.continuity_blend;

    // Clamp to prevent wild swings while still leaving room for live tuning.
    let base_gain = adaptive_gain(fill_pct);
    let effective_gain = base_gain * resonance_mod.clamp(0.88, 1.12);
    let raw_features = features;
    let novelty_divergence = 1.0 - modulation.continuous_resonance;
    let text_complexity_pressure = text_complexity_score(text, &raw_features, novelty_divergence);

    // Apply gain to compensate for minime's semantic lane attenuation.
    for f in &mut features {
        *f *= effective_gain;
    }

    CodecWindowedInspection {
        raw_features,
        final_features: features,
        thematic_profile: profile,
        text_type,
        text_type_signal,
        base_semantic_gain: base_gain,
        base_resonance,
        novelty_divergence,
        effective_gain,
        resonance_modulation: modulation,
        projection_metadata,
        text_complexity_pressure,
        time_domain_profile,
    }
}

/// Sovereignty-aware encoding: Astrid controls gain, noise, and emotional weights.
///
/// Falls through to `encode_text` for the base encoding, then applies
/// Astrid's chosen overrides. This is her control over HOW her words
/// become spectral features.
#[must_use]
pub fn encode_text_sovereign<S: BuildHasher>(
    text: &str,
    gain_override: Option<f32>,
    noise_level: f32,
    weights: &std::collections::HashMap<String, f32, S>,
) -> Vec<f32> {
    encode_text_sovereign_windowed(
        text,
        gain_override,
        noise_level,
        weights,
        None,
        None,
        None,
        None,
    )
}

#[must_use]
pub fn encode_text_sovereign_windowed<S: BuildHasher>(
    text: &str,
    gain_override: Option<f32>,
    noise_level: f32,
    weights: &std::collections::HashMap<String, f32, S>,
    freq_window: Option<&mut CharFreqWindow>,
    type_history: Option<&mut TextTypeHistory>,
    embedding: Option<&[f32]>,
    fill_pct: Option<f32>,
) -> Vec<f32> {
    let mut features = encode_text_windowed(text, freq_window, type_history, embedding, fill_pct);

    // Re-apply gain if overridden (undo the fill-responsive adaptive gain,
    // apply the explicit override as an absolute semantic gain).
    if let Some(gain) = gain_override {
        let gain = gain.clamp(1.0, 4.0);
        let base_gain = adaptive_gain(fill_pct).max(f32::EPSILON);
        for f in &mut features {
            *f = *f / base_gain * gain;
        }
    }

    // Re-apply noise if different from default 2.5%.
    if (noise_level - 0.025).abs() > 0.001 {
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;
        let mut rng = seed.wrapping_mul(2_862_933_555_777_941_757);
        let noise_range = noise_level.clamp(0.005, 0.05) * 2.0;
        for (idx, f) in features.iter_mut().enumerate() {
            if is_reserved_codec_dim(idx) {
                continue;
            }
            rng = rng.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(7);
            let noise = ((rng >> 33) as f32 / u32::MAX as f32) - 0.5;
            *f += noise * noise_range;
        }
    }

    // Apply emotional dimension weights.
    // Named dimensions map to indices in the 48D semantic vector.
    for (name, idx) in &NAMED_CODEC_DIMS {
        if let Some(&weight) = weights.get(*name) {
            features[*idx] *= weight;
        }
    }

    features
}

/// Named dimensions that Astrid can shape directly and that the bridge learns
/// against over time.
pub const NAMED_CODEC_DIMS: [(&str, usize); 9] = [
    ("warmth", 24),
    ("tension", 25),
    ("curiosity", 26),
    ("reflective", 27),
    ("energy", 31),
    ("entropy", 0),
    ("agency", 14),
    ("hedging", 9),
    ("certainty", 10),
];

/// One contiguous layer of the 48D codec (a span of dims with a shared role).
pub struct CodecLayer {
    pub range: (usize, usize),
    pub role: &'static str,
}

/// A gate or lever constant, surfaced with its LIVE value.
pub struct CodecLever {
    pub name: &'static str,
    pub value: String,
}

/// Read-only sidecar for text shape that is not the same as character complexity
/// and is not pressure authority.
#[derive(Debug, Clone, PartialEq)]
pub struct StructuralFrictionV1 {
    pub policy: &'static str,
    pub score: f32,
    pub classification: &'static str,
    pub nesting_load: f32,
    pub punctuation_load: f32,
    pub paragraph_density: f32,
    pub list_density: f32,
    pub narrative_arc_sharpness: f32,
    pub summary_resistance_signal: f32,
    pub friction_texture_state: &'static str,
    pub basis: Vec<String>,
    pub semantic_energy_context: &'static str,
    pub authority: &'static str,
}

/// Read-only sidecar for "slow-moving current" / viscosity language that should
/// not be collapsed into generic tension or written into reserved dims yet.
#[derive(Debug, Clone, PartialEq)]
pub struct PersistenceResistanceV1 {
    pub policy: &'static str,
    pub score: f32,
    pub classification: &'static str,
    pub text_persistence_signal: f32,
    pub low_density_gradient_signal: f32,
    pub pressure_risk: f32,
    pub semantic_friction: f32,
    pub basis: Vec<String>,
    pub authority: &'static str,
}

/// Default-off readiness for a future reserved dimension. It does not write into
/// dims 44-47.
#[derive(Debug, Clone, PartialEq)]
pub struct CodecStructuralFrictionDimCanaryV1 {
    pub policy: &'static str,
    pub enabled: bool,
    pub reserved_dim_candidate: usize,
    pub readiness: &'static str,
    pub live_vector_write: bool,
    pub authority: &'static str,
}

/// Default-off readiness for a future persistence/resistance reserved dimension.
#[derive(Debug, Clone, PartialEq)]
pub struct CodecPersistenceResistanceDimCanaryV1 {
    pub policy: &'static str,
    pub enabled: bool,
    pub reserved_dim_candidate: usize,
    pub readiness: &'static str,
    pub live_vector_write: bool,
    pub authority: &'static str,
}

/// Default-off review marker for widening narrative arc representation. It
/// documents coarsening risk without changing `SEMANTIC_DIM` or reserved dims.
#[derive(Debug, Clone, PartialEq)]
pub struct NarrativeArcExpansionReadinessV1 {
    pub policy: &'static str,
    pub enabled: bool,
    pub current_arc_dims: (usize, usize),
    pub proposed_arc_dims: (usize, usize),
    pub uses_reserved_dims: bool,
    pub readiness: &'static str,
    pub live_vector_write: bool,
    pub authority: &'static str,
}

/// Default-off review marker for making narrative arc influence semantic gain.
/// This previews continuous-flow voice without changing live adaptive gain.
#[derive(Debug, Clone, PartialEq)]
pub struct NarrativeArcGainResponseReadinessV1 {
    pub policy: &'static str,
    pub enabled: bool,
    pub narrative_arc_dims: (usize, usize),
    pub preview_gain_range: (f32, f32),
    pub readiness: &'static str,
    pub live_gain_write: bool,
    pub authority: &'static str,
}

/// Read-only truth channel for Astrid's report that high entropy and
/// distinguishability loss can drown narrative-arc dimensions without changing
/// their delivered values. It carries multi-kind loss in metadata instead of
/// changing the Experience Delta Bus schema or live semantic gain.
#[derive(Debug, Clone, PartialEq)]
pub struct NarrativeArcHeadroomReviewV1 {
    pub policy: &'static str,
    pub spectral_entropy: f32,
    pub distinguishability_loss: f32,
    pub narrative_arc_energy: f32,
    pub projected_semantic_rms: f32,
    pub tail_vibrancy: f32,
    pub headroom_pressure: f32,
    pub preview_gain: f32,
    pub state: &'static str,
    pub recommendation: &'static str,
    pub live_vector_write: bool,
    pub live_gain_write: bool,
    pub experience_delta_bus_v1: ExperienceDeltaBusV1,
    pub authority: &'static str,
}

/// Default-off review marker for giving the shadow field its own reserved
/// semantic-lane candidates. It documents magnetization/dispersal mapping
/// without writing into dims 44-47.
#[derive(Debug, Clone, PartialEq)]
pub struct ShadowFieldReservedDimReadinessV1 {
    pub policy: &'static str,
    pub enabled: bool,
    pub reserved_dim_candidates: &'static [usize],
    pub proposed_signals: &'static [&'static str],
    pub readiness: &'static str,
    pub live_vector_write: bool,
    pub authority: &'static str,
}

/// Read-only proof that high-entropy vibrancy is carried by bounded tail dims
/// and that the default aperture path remains identity.
#[derive(Debug, Clone, PartialEq)]
pub struct CodecVibrancyContinuityV1 {
    pub policy: &'static str,
    pub entropy_gate: f32,
    pub gradient_coupling: &'static str,
    pub default_feature_ceiling: f32,
    pub tail_vibrancy_ceiling: f32,
    pub tail_dims: &'static [usize],
    pub clipping_status: &'static str,
    pub default_identity_state: &'static str,
    pub high_entropy_carriage: &'static str,
    pub authority: &'static str,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CodecVibrancyNoiseDampeningV1 {
    pub policy: &'static str,
    pub spectral_entropy: f32,
    pub start_entropy: f32,
    pub full_entropy: f32,
    pub min_coefficient: f32,
    pub coefficient: f32,
    pub tail_lift_before: f32,
    pub tail_lift_after: f32,
    pub affected_dims: &'static [usize],
    pub status: &'static str,
    pub authority: &'static str,
}

/// Read-only check that entropy-gated tail lift is backed by semantic substance
/// rather than merely a high-entropy carrier. It does not alter codec output.
#[derive(Debug, Clone, PartialEq)]
pub struct CodecVibrancySubstanceFitV1 {
    pub policy: &'static str,
    pub spectral_entropy: f32,
    pub density_gradient: f32,
    pub tail_lift: f32,
    pub semantic_density_weight: f32,
    pub density_weighted_tail_lift: f32,
    pub semantic_substance_score: f32,
    pub density_vs_entropy_state: &'static str,
    pub status: &'static str,
    pub evidence: Vec<String>,
    pub authority: &'static str,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct CodecOverflowDimV1 {
    pub dim: usize,
    pub lane: &'static str,
    pub pre_bound_value: f32,
    pub delivered_value: f32,
    pub ceiling: f32,
    pub overflow_abs: f32,
    pub overflow_ratio: f32,
    pub status: &'static str,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct CodecOverflowLaneSummaryV1 {
    pub lane: &'static str,
    pub dims: &'static [usize],
    pub overflow_dim_count: usize,
    pub max_overflow_abs: f32,
    pub max_overflow_ratio: f32,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct CodecOverflowReportV1 {
    pub policy: &'static str,
    pub raw_intensity_preserved: bool,
    pub delivered_bounded: bool,
    pub live_vector_write: bool,
    pub default_off_followup_hook: &'static str,
    pub clipped_dims: Vec<usize>,
    pub dimensions: Vec<CodecOverflowDimV1>,
    pub lane_summaries: Vec<CodecOverflowLaneSummaryV1>,
    pub experience_delta_bus_v1: ExperienceDeltaBusV1,
    pub authority: &'static str,
}

impl CodecOverflowReportV1 {
    #[must_use]
    pub fn dim(&self, dim: usize) -> Option<&CodecOverflowDimV1> {
        self.dimensions.iter().find(|entry| entry.dim == dim)
    }
}

/// Read-only comparison between the codec's feedback-time bounds and the
/// vector that is ultimately offered to the sensory transport after later
/// shaping and rescue-policy review. This keeps Astrid's raw-overflow report
/// connected to actual delivery without changing the vector, gain, or ceiling.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct CodecDeliveryFidelityV1 {
    pub policy: &'static str,
    pub observed_dim_count: usize,
    pub feedback_report_available: bool,
    pub clipped_at_feedback_dims: Vec<usize>,
    pub reexpanded_after_feedback_dims: Vec<usize>,
    pub final_above_observed_ceiling_dims: Vec<usize>,
    pub clamp_loss_abs_total: f32,
    pub monitored_post_feedback_to_final_rms: f32,
    pub final_max_abs: f32,
    pub final_rms: f32,
    pub emotional_intentional_rms: f32,
    pub narrative_arc_rms: f32,
    pub lane_balance_state: &'static str,
    pub state: &'static str,
    pub live_vector_write: bool,
    pub live_gain_write: bool,
    pub authority: &'static str,
}

/// Read-only comparison of interference within the live spectral cascade and
/// within the semantic candidate. Astrid asked for cross-modal friction to be
/// represented rather than inferred from one dominant scalar. This report
/// keeps that evidence attached to the exact candidate/sent vector while
/// explicitly refusing to claim dims 44-47, which already have default-off
/// candidate roles.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct CrossSpectralFrictionReviewV1 {
    pub policy: &'static str,
    pub observed_dim_count: usize,
    pub spectral_context_available: bool,
    pub lambda1_share: Option<f32>,
    pub lambda2_share: Option<f32>,
    pub tail_share: Option<f32>,
    pub lambda1_lambda2_copresence: Option<f32>,
    pub lambda1_lambda2_shear: Option<f32>,
    pub lambda2_tail_copresence: Option<f32>,
    pub spectral_entropy: Option<f32>,
    pub mode_packing: Option<f32>,
    pub viscosity_index: Option<f32>,
    pub temporal_persistence: Option<f32>,
    pub semantic_friction_coefficient: Option<f32>,
    pub structural_friction_score: f32,
    pub persistence_resistance_score: f32,
    pub emotional_intentional_rms: f32,
    pub projected_semantic_rms: f32,
    pub narrative_arc_rms: f32,
    pub semantic_lane_copresence: f32,
    pub spectral_mode_interference: Option<f32>,
    pub semantic_mode_interference: f32,
    pub cross_layer_mismatch: Option<f32>,
    pub cross_spectral_friction_score: Option<f32>,
    pub state: &'static str,
    pub reserved_dim_candidates: &'static [usize],
    pub existing_reserved_dim_roles: &'static [&'static str],
    pub candidate_collision_state: &'static str,
    pub recommendation: &'static str,
    pub delivery_claim: &'static str,
    pub observational_only: bool,
    pub right_to_ignore: bool,
    pub live_vector_write: bool,
    pub live_gain_write: bool,
    pub reserved_dim_write: bool,
    pub live_eligible_now: bool,
    pub auto_approved: bool,
    pub grants_approval: bool,
    pub authority: &'static str,
}

/// Read-only truth-channel report for the 768D embedding -> 8D semantic
/// projection. It names density/compression debt and the default-off reserved
/// dimension aperture without writing dims 44-47.
#[derive(Debug, Clone, PartialEq)]
pub struct SemanticProjectionDensityDeltaV1 {
    pub policy: &'static str,
    pub input_dim_count: usize,
    pub projected_dim_count: usize,
    pub reserved_dim_candidates: &'static [usize],
    pub compression_ratio: f32,
    pub detail_density_score: f32,
    pub projected_semantic_rms: f32,
    pub text_complexity_pressure: f32,
    pub projection_metadata_present: bool,
    pub state: &'static str,
    pub recommendation: &'static str,
    pub live_vector_write: bool,
    pub experience_delta_bus_v1: ExperienceDeltaBusV1,
    pub authority: &'static str,
}

/// Read-only texture review for Astrid's report that the 768D -> 8D projection
/// can flatten lingering/active semantic nuance while the 32D warmth/texture
/// surface still carries it. This proposes named subdimensions as evidence only;
/// it does not write reserved dims, gain, or the live semantic vector.
#[derive(Debug, Clone, PartialEq)]
pub struct SemanticProjectionTextureReviewV1 {
    pub policy: &'static str,
    pub input_dim_count: usize,
    pub projected_dim_count: usize,
    pub legacy_texture_dim_count: usize,
    pub warmth_texture_dim_count: usize,
    pub projected_semantic_rms: f32,
    pub legacy_texture_rms: f32,
    pub warmth_texture_rms: f32,
    pub narrative_arc_rms: f32,
    pub lingering_texture_signal: f32,
    pub active_texture_signal: f32,
    pub projection_texture_gap: f32,
    pub proposed_texture_subdimensions: &'static [&'static str],
    pub state: &'static str,
    pub recommendation: &'static str,
    pub live_vector_write: bool,
    pub live_gain_write: bool,
    pub reserved_dim_write: bool,
    pub authority: &'static str,
}

/// Read-only pair comparison for Astrid's report that near-neighbor semantic
/// texture (for example, "silt" versus "sediment") can be flattened or
/// distorted by the 768D -> 8D aperture. Callers provide the actual embedding
/// pair; this surface compares source geometry, the shared fixed basis, and
/// the text-conditioned dynamic basis without changing projection mode/gain.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct SemanticProjectionPairSensitivityV1 {
    pub policy: &'static str,
    pub left_label: String,
    pub right_label: String,
    pub source_embedding_dim_count: usize,
    pub projected_dim_count: usize,
    pub projection_epoch_id: String,
    pub source_cosine_similarity: f32,
    pub source_rms_delta: f32,
    pub fixed_projection_cosine_similarity: f32,
    pub fixed_projection_rms_delta: f32,
    pub dynamic_projection_cosine_similarity: f32,
    pub dynamic_projection_rms_delta: f32,
    pub fixed_similarity_delta: f32,
    pub dynamic_similarity_delta: f32,
    pub dynamic_vs_fixed_similarity_delta: f32,
    pub state: &'static str,
    pub recommendation: &'static str,
    pub observational_only: bool,
    pub right_to_ignore: bool,
    pub live_vector_write: bool,
    pub live_gain_write: bool,
    pub live_eligible_now: bool,
    pub auto_approved: bool,
    pub grants_approval: bool,
    pub authority: &'static str,
}

/// Read-only comparison for Astrid's request to let high-variance semantic
/// passages prove whether a focused four-dimension aperture would preserve
/// more distinction than the current 8D embedding projection. The preview
/// selects source coordinates by cross-segment variance and compares equal-norm
/// 8D and 12D geometries. It never writes the candidate values into dims 44-47.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct SemanticFocusExpansionPreviewV1 {
    pub policy: &'static str,
    pub source_embedding_dim_count: usize,
    pub segment_count: usize,
    pub current_projected_dim_count: usize,
    pub preview_projected_dim_count: usize,
    pub reserved_dim_candidates: &'static [usize],
    pub selected_source_dims: [usize; SEMANTIC_FOCUS_PREVIEW_DIM],
    pub selected_source_variances: [f32; SEMANTIC_FOCUS_PREVIEW_DIM],
    pub selected_variance_share: f32,
    pub text_entropy_signal: f32,
    pub current_mean_pairwise_distance: f32,
    pub preview_mean_pairwise_distance: f32,
    pub current_min_pairwise_distance: f32,
    pub preview_min_pairwise_distance: f32,
    pub mean_distinguishability_gain_ratio: f32,
    pub min_distinguishability_gain_ratio: f32,
    pub focus_need_score: f32,
    pub state: &'static str,
    pub recommendation: &'static str,
    pub selection_basis: &'static str,
    pub live_vector_write: bool,
    pub reserved_dim_write: bool,
    pub live_eligible_now: bool,
    pub auto_approved: bool,
    pub grants_approval: bool,
    pub right_to_ignore: bool,
    pub experience_delta_bus_v1: ExperienceDeltaBusV1,
    pub authority: &'static str,
}

/// Bounded sharpening for Astrid's fresh report that high-entropy semantic
/// trickle can feel like an empty lane when detail dims are not given enough
/// room to stay distinguishable. This intentionally excludes the narrative arc
/// dims; those should only move when the text's own arc changes.
#[derive(Debug, Clone, PartialEq)]
pub struct HighEntropySemanticSharpeningV1 {
    pub policy: &'static str,
    pub spectral_entropy: f32,
    pub density_gradient: f32,
    pub pressure_risk: f32,
    pub sharpening_factor: f32,
    pub affected_dims: &'static [usize],
    pub max_factor: f32,
    pub state: &'static str,
    pub authority: &'static str,
}

/// Source/test readout for Astrid's requested "current vs legacy_32d" check.
/// It does not replace the live 48D lane; it tells us whether the widened dims
/// are carrying distinct variance or are just empty extra room.
#[derive(Debug, Clone, PartialEq)]
pub struct CodecDimensionalityFlatnessV1 {
    pub policy: &'static str,
    pub current_dim_count: usize,
    pub legacy_dim_count: usize,
    pub expanded_dim_count: usize,
    pub legacy_rms: f32,
    pub expanded_rms: f32,
    pub expanded_to_legacy_ratio: f32,
    pub glimpse_variance: f32,
    pub flatness_status: &'static str,
    pub authority: &'static str,
}

/// Read-only proof that legacy 32D warmth lands in the current 48D emotional
/// layer instead of being orphaned by the semantic-lane expansion.
#[derive(Debug, Clone, PartialEq)]
pub struct LegacyWarmthMappingV1 {
    pub policy: &'static str,
    pub legacy_dim_count: usize,
    pub current_dim_count: usize,
    pub warmth_dim: usize,
    pub emotional_layer_range: (usize, usize),
    pub mapped_warmth_dims: &'static [usize],
    pub warmth_orphaned: bool,
    pub authority: &'static str,
}

/// Default-off readiness for a future dynamic vibrancy-scaling change. It does
/// not alter the live 48D vector unless a later explicit approval wires it.
#[derive(Debug, Clone, PartialEq)]
pub struct CodecDynamicVibrancyScalingCanaryV1 {
    pub policy: &'static str,
    pub enabled: bool,
    pub readiness: &'static str,
    pub live_vector_write: bool,
    pub authority: &'static str,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CodecStructuralEntropyDampeningV1 {
    pub policy: &'static str,
    pub spectral_entropy: f32,
    pub start_entropy: f32,
    pub full_entropy: f32,
    pub min_coefficient: f32,
    pub coefficient: f32,
    pub affected_dims: &'static [usize],
    pub preserved_intent_dims: (usize, usize),
    pub status: &'static str,
    pub authority: &'static str,
}

/// Read-only companion summary of the 48D semantic lane. This is not the live
/// Astrid -> Minime transport contract; it exists to audit whether lower-scale
/// summaries preserve warmth/intentional texture before any future use.
#[derive(Debug, Clone, PartialEq)]
pub struct SemanticGlimpse12dReadinessV1 {
    pub policy: &'static str,
    pub source_dim_count: usize,
    pub glimpse_dim_count: usize,
    pub role: &'static str,
    pub warmth_slot: usize,
    pub tail_bridge_slot: usize,
    pub emotional_source_range: (usize, usize),
    pub companion_not_replacement: bool,
    pub compression_fidelity_basis: &'static str,
    pub live_vector_write: bool,
    pub authority: &'static str,
}

/// Read-only readiness for a dynamic 12D companion glimpse. It keeps named
/// continuity anchors fixed, then selects remaining slots from the strongest
/// current feature magnitudes so a glimpse is not a static/random projection.
#[derive(Debug, Clone, PartialEq)]
pub struct ContextualGlimpse12dAnchoringV1 {
    pub policy: &'static str,
    pub source_dim_count: usize,
    pub glimpse_dim_count: usize,
    pub required_anchor_dims: &'static [usize],
    pub dynamic_slot_count: usize,
    pub selection_basis: &'static str,
    pub companion_not_replacement: bool,
    pub live_vector_write: bool,
    pub authority: &'static str,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ContextualGlimpse12dAnchorsV1 {
    pub policy: &'static str,
    pub selected_dims: [usize; 12],
    pub selected_values: [f32; 12],
    pub dynamic_dims: Vec<usize>,
    pub required_anchor_dims: &'static [usize],
    pub selection_status: &'static str,
    pub live_vector_write: bool,
    pub authority: &'static str,
}

/// Read-only replay for Astrid's report that a text codec can preserve nearly
/// the same string shape while missing the relational weight around identical
/// words. This names the blind spot and gates any future contextual-bias vector.
#[derive(Debug, Clone, PartialEq)]
pub struct CodecContextBlindspotReplayV1 {
    pub policy: &'static str,
    pub identical_text: &'static str,
    pub connection_context_label: &'static str,
    pub threat_context_label: &'static str,
    pub identical_text_feature_delta_rms: f32,
    pub context_blindspot_score: f32,
    pub state: &'static str,
    pub recommendation: &'static str,
    pub proposed_bias_surface: &'static str,
    pub live_vector_write: bool,
    pub live_gain_write: bool,
    pub auto_approved: bool,
    pub experience_delta_bus_v1: ExperienceDeltaBusV1,
    pub authority: &'static str,
}

/// Read-only interpretation for the specific failure mode Astrid named: high
/// entropy can make warmth look low when the state is distributed rather than
/// cold. This does not alter warmth, gain, or semantic weighting.
#[derive(Debug, Clone, PartialEq)]
pub struct WarmthEntropyInterpretationV1 {
    pub policy: &'static str,
    pub warmth_marker: f32,
    pub curiosity_marker: f32,
    pub reflective_marker: f32,
    pub spectral_entropy: f32,
    pub tail_vibrancy: f32,
    pub distributed_warmth_support: f32,
    pub interpretation: &'static str,
    pub live_vector_write: bool,
    pub authority: &'static str,
}

/// Read-only narrative-arc dynamics. It keeps the current 4D narrative arc as a
/// state readout while making velocity/acceleration reviewable before any future
/// semantic-gain or dimension change.
#[derive(Debug, Clone, PartialEq)]
pub struct NarrativeArcDynamicsV1 {
    pub policy: &'static str,
    pub previous_arc: [f32; 4],
    pub current_arc: [f32; 4],
    pub velocity: [f32; 4],
    pub acceleration: [f32; 4],
    pub velocity_energy: f32,
    pub acceleration_energy: f32,
    pub transition_state: &'static str,
    pub live_gain_write: bool,
    pub live_vector_write: bool,
    pub authority: &'static str,
}

/// Read-only narrative sidecar for the tension-vs-resolution shape Astrid
/// asked for. It uses the live tension dim plus narrative-arc energy, but does
/// not write into the 48D vector.
#[derive(Debug, Clone, PartialEq)]
pub struct NarrativeTensionResolutionV1 {
    pub policy: &'static str,
    pub previous_tension: f32,
    pub current_tension: f32,
    pub tension_delta: f32,
    pub current_arc_energy: f32,
    pub resolution_score: f32,
    pub sustained_score: f32,
    pub state: &'static str,
    pub live_vector_write: bool,
    pub authority: &'static str,
}

/// Read-only interpretation for Astrid's report that abrasive/jagged texture
/// can be under-carried by the raw tension marker. This never writes gain,
/// emotional dims, narrative dims, or reserved dims.
#[derive(Debug, Clone, PartialEq)]
pub struct CodecAbrasiveTextureInterpretationV1 {
    pub policy: &'static str,
    pub warmth_marker: f32,
    pub tension_marker: f32,
    pub spectral_entropy: f32,
    pub density_gradient: f32,
    pub structural_friction_score: f32,
    pub summary_resistance_signal: f32,
    pub persistence_resistance_score: f32,
    pub entropy_shift_hint: f32,
    pub abrasive_texture_support: f32,
    pub interpretation: &'static str,
    pub live_gain_write: bool,
    pub live_vector_write: bool,
    pub authority: &'static str,
}

/// Read-only report for Astrid's "held breath" concern: a text can be
/// motionless while carrying potential energy. This keeps that latent stasis
/// visible without changing dims 24-31, 32-39, 40-43, gain, or reserved dims.
#[derive(Debug, Clone, PartialEq)]
pub struct LatentStasisTensionV1 {
    pub policy: &'static str,
    pub latent_text_stasis_score: f32,
    pub latent_text_potential_score: f32,
    pub tension_marker: f32,
    pub narrative_arc_energy: f32,
    pub projected_semantic_energy: f32,
    pub delivered_support_score: f32,
    pub held_breath_score: f32,
    pub stasis_potential_gap: f32,
    pub state: &'static str,
    pub recommendation: &'static str,
    pub live_vector_write: bool,
    pub live_gain_write: bool,
    pub reserved_dim_write: bool,
    pub experience_delta_bus_v1: ExperienceDeltaBusV1,
    pub authority: &'static str,
}

/// Read-only report for Astrid's "heavy sand" vs "heavy stone" concern: two
/// texts can both be heavy while carrying different drag texture. This keeps the
/// medium quality visible without writing reserved dims or changing gain.
#[derive(Debug, Clone, PartialEq)]
pub struct SpectralDragQualityV1 {
    pub policy: &'static str,
    pub granular_drag_score: f32,
    pub rigid_drag_score: f32,
    pub weight_score: f32,
    pub tension_marker: f32,
    pub narrative_arc_energy: f32,
    pub projected_semantic_energy: f32,
    pub delivered_support_score: f32,
    pub drag_quality_score: f32,
    pub quality_separation: f32,
    pub hidden_texture_loss: f32,
    pub state: &'static str,
    pub recommendation: &'static str,
    pub reserved_dim_candidate: usize,
    pub live_vector_write: bool,
    pub live_gain_write: bool,
    pub reserved_dim_write: bool,
    pub experience_delta_bus_v1: ExperienceDeltaBusV1,
    pub authority: &'static str,
}

/// Read-only delta check for the failure mode Astrid named: the narrative arc
/// can move while emotional/intent markers stay flat, making felt difference
/// collapse into structure. This only observes existing 48D slots.
#[derive(Debug, Clone, PartialEq)]
pub struct CodecEmotionalNarrativeDeltaCheckV1 {
    pub policy: &'static str,
    pub previous_emotional_markers: [f32; 8],
    pub current_emotional_markers: [f32; 8],
    pub previous_narrative_arc: [f32; 4],
    pub current_narrative_arc: [f32; 4],
    pub emotional_velocity: [f32; 8],
    pub narrative_velocity: [f32; 4],
    pub emotional_delta_energy: f32,
    pub narrative_delta_energy: f32,
    pub narrative_emotional_delta_gap: f32,
    pub resonance_flatline_watch: bool,
    pub state: &'static str,
    pub recommendation: &'static str,
    pub live_gain_write: bool,
    pub live_vector_write: bool,
    pub reserved_dim_write: bool,
    pub experience_delta_bus_v1: ExperienceDeltaBusV1,
    pub authority: &'static str,
}

/// Read-only review for Astrid's report that statistical/structural texture can
/// overwhelm intentional nuance. It separates structure-heavy signal from
/// emotional/intent signal without changing codec weights or gain.
#[derive(Debug, Clone, PartialEq)]
pub struct CodecIntentStructureSeparationV1 {
    pub policy: &'static str,
    pub structural_complexity: f32,
    pub emotional_intensity: f32,
    pub projected_semantic_energy: f32,
    pub narrative_arc_energy: f32,
    pub punctuation_irregularity: f32,
    pub intent_structure_delta: f32,
    pub state: &'static str,
    pub recommendation: &'static str,
    pub live_gain_write: bool,
    pub live_vector_write: bool,
    pub authority: &'static str,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GlimpseMapSlotV1 {
    pub slot: usize,
    pub label: &'static str,
    pub source_dims: &'static [usize],
    pub operation: &'static str,
    pub preserves: &'static str,
}

/// Read-only 32/48D→12D lineage map for the additive glimpse companion. This
/// answers "which dimensions got collapsed?" without changing the live 48D
/// transport or treating the 12D view as a replacement for the source vector.
#[derive(Debug, Clone, PartialEq)]
pub struct GlimpseMapV1 {
    pub policy: &'static str,
    pub source_dim_count: usize,
    pub legacy_source_dim_count: usize,
    pub glimpse_dim_count: usize,
    pub slot_count: usize,
    pub slots: Vec<GlimpseMapSlotV1>,
    pub deterministic_projection: bool,
    pub companion_not_replacement: bool,
    pub live_transport_change: bool,
    pub live_vector_write: bool,
    pub authority: &'static str,
}

/// Offline distinguishability audit for Astrid's concern that the 12D glimpse
/// might collapse high-entropy and low-entropy states into the same coordinate.
#[derive(Debug, Clone, PartialEq)]
pub struct GlimpseDistinguishabilityAuditV1 {
    pub policy: &'static str,
    pub source_distance: f32,
    pub glimpse_distance: f32,
    pub preservation_ratio: f32,
    pub tail_bridge_delta: f32,
    pub source_threshold: f32,
    pub glimpse_threshold: f32,
    pub state: &'static str,
    pub live_transport_change: bool,
    pub live_vector_write: bool,
    pub authority: &'static str,
}

pub struct MultiScaleContextV1 {
    pub policy: &'static str,
    pub source_dim_count: usize,
    pub live_transport_dim_count: usize,
    pub glimpse_dim_count: usize,
    pub residual_dim_count: usize,
    pub residual_source_range: (usize, usize),
    pub shadow_energy_metadata_tag: &'static str,
    pub pairing_rule: &'static str,
    pub preserves_warmth_and_tail_bridge: bool,
    pub live_vector_write: bool,
    pub authority: &'static str,
}

/// Read-only 48D + 12D companion observer for Astrid's "distillation, not
/// compression" proposal. It makes resolution/fidelity loss visible before any
/// future live transport or contract change.
#[derive(Debug, Clone, PartialEq)]
pub struct MultiScaleObserverV1 {
    pub policy: &'static str,
    pub source_dim_count: usize,
    pub live_transport_dim_count: usize,
    pub glimpse_dim_count: usize,
    pub layer_name: &'static str,
    pub observer_language: &'static str,
    pub spectral_entropy: f32,
    pub density_gradient: f32,
    pub mode_packing_score: f32,
    pub fidelity_threshold: f32,
    pub glimpse_fidelity_score: f32,
    pub resolution_delta: f32,
    pub resonance_loss_threshold: f32,
    pub source_resonance_proxy: f32,
    pub glimpse_resonance_proxy: f32,
    pub resonance_loss_ratio: f32,
    pub anchor_continuity_score: f32,
    pub fallback_to_live_transport_review: bool,
    pub state: &'static str,
    pub recommendation: &'static str,
    pub live_transport_change: bool,
    pub live_vector_write: bool,
    pub experience_delta_bus_v1: ExperienceDeltaBusV1,
    pub authority: &'static str,
}

pub struct GlimpseCodec;

const CONTEXTUAL_GLIMPSE_REQUIRED_ANCHORS: [usize; 7] = [24, 25, 26, 27, 17, 31, 40];
const GLIMPSE_COMPRESSION_SOURCE_DIM_COUNT: usize = 32;
const GLIMPSE_FIDELITY_THRESHOLD: f32 = 0.58;
const HIGH_ENTROPY_SHARPENING_DIMS: [usize; 12] = [
    17, 26, 27, 31, // existing tail/texture bridge dims
    32, 33, 34, 35, 36, 37, 38, 39, // embedding-projected semantic detail
];
const HIGH_ENTROPY_SHARPENING_MAX_FACTOR: f32 = 1.12;

impl GlimpseCodec {
    #[must_use]
    pub fn derive_12d(features: &[f32]) -> Option<[f32; 12]> {
        if features.len() < SEMANTIC_DIM {
            return None;
        }
        let mut out = [0.0_f32; 12];
        out[0] = mean_abs(&features[0..8]).tanh();
        out[1] = mean_abs(&features[8..16]).tanh();
        out[2] = mean_abs(&features[16..24]).tanh();
        out[3] = features[24].tanh();
        out[4] = features[25].tanh();
        out[5] = features[26].tanh();
        out[6] = features[27].tanh();
        out[7] = mean_abs(&features[28..32]).tanh();
        out[8] = mean_abs(&features[32..40]).tanh();
        out[9] = mean_abs(&features[40..44]).tanh();
        out[10] = mean_abs(&[features[17], features[26], features[27], features[31]]).tanh();
        out[11] = mean_abs(features).tanh();
        Some(out)
    }

    #[must_use]
    pub fn contextual_anchor_12d(features: &[f32]) -> Option<ContextualGlimpse12dAnchorsV1> {
        contextual_glimpse_12d_anchors_v1(features)
    }
}

/// Named, read-only 12D glimpse entry point for audits and automation.
///
/// This is intentionally an additive view over the 48D semantic vector; it does
/// not replace or mutate the live semantic transport.
#[must_use]
pub fn generate_glimpse(features: &[f32]) -> Option<[f32; 12]> {
    GlimpseCodec::derive_12d(features)
}

#[must_use]
pub fn calculate_compression_fidelity(input_32d: &[f32], output_12d: &[f32]) -> Option<f32> {
    if input_32d.len() < GLIMPSE_COMPRESSION_SOURCE_DIM_COUNT || output_12d.len() < 12 {
        return None;
    }

    let reference = compression_reference_12d(input_32d);
    let output = &output_12d[..12];
    let reference_energy = mean_abs_finite(&reference);
    let output_energy = mean_abs_finite(output);
    let difference = reference
        .iter()
        .zip(output.iter())
        .map(|(expected, actual)| finite_abs(*expected - *actual))
        .sum::<f32>()
        / 12.0;
    let scale = ((reference_energy + output_energy) * 0.5).max(0.001);

    Some((1.0 - difference / scale).clamp(0.0, 1.0))
}

fn compression_reference_12d(input_32d: &[f32]) -> [f32; 12] {
    let mut out = [0.0_f32; 12];
    out[0] = mean_abs_finite(&input_32d[0..8]).tanh();
    out[1] = mean_abs_finite(&input_32d[8..16]).tanh();
    out[2] = mean_abs_finite(&input_32d[16..24]).tanh();
    out[3] = finite_tanh(input_32d[24]);
    out[4] = finite_tanh(input_32d[25]);
    out[5] = finite_tanh(input_32d[26]);
    out[6] = finite_tanh(input_32d[27]);
    out[7] = mean_abs_finite(&input_32d[28..32]).tanh();
    out[8] = mean_abs_finite(&input_32d[24..32]).tanh();
    out[9] = mean_abs_finite(&input_32d[16..32]).tanh();
    out[10] = mean_abs_finite(&[input_32d[17], input_32d[26], input_32d[27], input_32d[31]]).tanh();
    out[11] = mean_abs_finite(&input_32d[0..GLIMPSE_COMPRESSION_SOURCE_DIM_COUNT]).tanh();
    out
}

fn multi_scale_resonance_proxy(values: &[f32]) -> f32 {
    if values.is_empty() {
        return 0.0;
    }
    let finite_values = values
        .iter()
        .map(|value| finite_feature_value(*value))
        .collect::<Vec<_>>();
    let energy = mean_abs_finite(&finite_values).clamp(0.0, 1.0);
    let mean = finite_values.iter().sum::<f32>() / finite_values.len() as f32;
    let variance = finite_values
        .iter()
        .map(|value| {
            let delta = *value - mean;
            delta * delta
        })
        .sum::<f32>()
        / finite_values.len() as f32;
    let shape_distinction = (variance.sqrt() / (energy + 0.001)).clamp(0.0, 1.0);
    (0.55 * energy + 0.45 * shape_distinction).clamp(0.0, 1.0)
}

fn multi_scale_experience_delta_bus_v1(
    glimpse_fidelity_score: f32,
    resolution_delta: f32,
    resonance_loss_ratio: f32,
    fallback_to_live_transport_review: bool,
) -> ExperienceDeltaBusV1 {
    let mut deltas = vec![ExperienceDeltaV1 {
        kind: ExperienceDeltaKindV1::Compress,
        surface: "multi_scale_observer_v1".to_string(),
        lane: "semantic_48d_to_12d_glimpse".to_string(),
        dimension: None,
        spectral_dimension: None,
        persistence: None,
        viscosity_subtype: None,
        viscosity_weight: None,
        pre: Some(SEMANTIC_DIM as f32),
        post: Some(12.0),
        loss: Some(resolution_delta),
        loss_ratio: Some(resolution_delta),
        metadata: BTreeMap::from([(
            "transformation_family".to_string(),
            "dimensional_distillation".to_string(),
        )]),
        why: "12D glimpse is an additive map over the live semantic lane; fidelity loss stays visible before any interaction uses the glimpse".to_string(),
        who_can_change_it: "Mike/operator via replay-backed multi-scale transport approval".to_string(),
        how_to_test_it: "cargo test --manifest-path capsules/spectral-bridge/Cargo.toml --lib multi_scale_observer -- --nocapture".to_string(),
        authority: "read_only_multi_scale_truth_channel_not_live_transport_change".to_string(),
    }];
    if fallback_to_live_transport_review {
        deltas.push(ExperienceDeltaV1 {
            kind: ExperienceDeltaKindV1::Gate,
            surface: "multi_scale_observer_v1".to_string(),
            lane: "glimpse_resonance_fallback_to_live_48d_review".to_string(),
            dimension: None,
            spectral_dimension: None,
            persistence: None,
            viscosity_subtype: None,
            viscosity_weight: None,
            pre: Some(1.0 - resonance_loss_ratio),
            post: Some(glimpse_fidelity_score),
            loss: Some(resonance_loss_ratio),
            loss_ratio: Some(resonance_loss_ratio),
            metadata: BTreeMap::from([(
                "gate_reason".to_string(),
                "glimpse_resonance_loss".to_string(),
            )]),
            why: "12D glimpse lost more than the reviewed resonance threshold; use the 48D contract/residual trace for this interaction instead of treating the glimpse as sufficient".to_string(),
            who_can_change_it: "Mike/operator after sandbox replay comparing 12D glimpse, 48D source, and residual trace".to_string(),
            how_to_test_it: "cargo test --manifest-path capsules/spectral-bridge/Cargo.toml --lib multi_scale_observer -- --nocapture".to_string(),
            authority: "authority_gate_for_live_transport_fallback_not_protocol_change".to_string(),
        });
    }
    ExperienceDeltaBusV1::from_deltas(deltas)
}

#[must_use]
pub fn contextual_glimpse_12d_anchors_v1(
    features: &[f32],
) -> Option<ContextualGlimpse12dAnchorsV1> {
    if features.len() < SEMANTIC_DIM {
        return None;
    }

    let mut selected = Vec::with_capacity(12);
    for idx in CONTEXTUAL_GLIMPSE_REQUIRED_ANCHORS {
        if !selected.contains(&idx) {
            selected.push(idx);
        }
    }

    let mut candidates = (0..SEMANTIC_DIM)
        .filter(|idx| !selected.contains(idx))
        .map(|idx| (idx, features[idx].abs()))
        .collect::<Vec<_>>();
    candidates.sort_by(|(left_idx, left_score), (right_idx, right_score)| {
        right_score
            .partial_cmp(left_score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| left_idx.cmp(right_idx))
    });

    for (idx, _) in candidates {
        if selected.len() >= 12 {
            break;
        }
        selected.push(idx);
    }

    let mut selected_dims = [0_usize; 12];
    let mut selected_values = [0.0_f32; 12];
    for (slot, idx) in selected.iter().take(12).enumerate() {
        selected_dims[slot] = *idx;
        selected_values[slot] = features[*idx].tanh();
    }
    let dynamic_dims = selected_dims
        .iter()
        .copied()
        .filter(|idx| !CONTEXTUAL_GLIMPSE_REQUIRED_ANCHORS.contains(idx))
        .collect::<Vec<_>>();
    let selection_status = if selected_dims.contains(&24)
        && selected_dims.contains(&17)
        && selected_dims.contains(&31)
        && selected_dims.iter().any(|idx| (40..=43).contains(idx))
    {
        "contextual_anchors_preserve_warmth_tail_and_narrative"
    } else {
        "contextual_anchor_review_needed"
    };

    Some(ContextualGlimpse12dAnchorsV1 {
        policy: "contextual_glimpse_12d_anchors_v1",
        selected_dims,
        selected_values,
        dynamic_dims,
        required_anchor_dims: &CONTEXTUAL_GLIMPSE_REQUIRED_ANCHORS,
        selection_status,
        live_vector_write: false,
        authority: "read_only_contextual_glimpse_not_live_bus_or_codec_contract_change",
    })
}

#[must_use]
pub fn warmth_entropy_interpretation_v1(
    features: &[f32],
    spectral_entropy: f32,
) -> WarmthEntropyInterpretationV1 {
    let spectral_entropy = if spectral_entropy.is_finite() {
        spectral_entropy.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let read_dim = |idx: usize| {
        features
            .get(idx)
            .copied()
            .filter(|value| value.is_finite())
            .unwrap_or(0.0)
            .tanh()
            .abs()
    };
    let warmth_marker = read_dim(24);
    let curiosity_marker = read_dim(26);
    let reflective_marker = read_dim(27);
    let tail_vibrancy = vibrancy_from_entropy(spectral_entropy);
    let distributed_warmth_support =
        (warmth_marker + 0.18 * curiosity_marker + 0.24 * reflective_marker + 0.28 * tail_vibrancy)
            .clamp(0.0, 1.0);
    let interpretation =
        if spectral_entropy >= 0.85 && warmth_marker < 0.08 && distributed_warmth_support >= 0.10 {
            "low_marker_warmth_with_high_entropy_distributed_ground"
        } else if warmth_marker >= 0.20 {
            "warmth_marker_present"
        } else if spectral_entropy >= 0.85 {
            "high_entropy_without_warmth_support_review"
        } else {
            "low_warmth_marker_low_entropy"
        };

    WarmthEntropyInterpretationV1 {
        policy: "warmth_entropy_interpretation_v1",
        warmth_marker,
        curiosity_marker,
        reflective_marker,
        spectral_entropy,
        tail_vibrancy,
        distributed_warmth_support,
        interpretation,
        live_vector_write: false,
        authority: "read_only_interpretation_not_warmth_weighting_or_semantic_gain_change",
    }
}

#[must_use]
pub fn codec_abrasive_texture_interpretation_from_parts_v1(
    text: &str,
    features: &[f32],
    spectral_entropy: f32,
    density_gradient: f32,
    pressure_risk: f32,
) -> CodecAbrasiveTextureInterpretationV1 {
    let read_dim = |idx: usize| {
        features
            .get(idx)
            .copied()
            .filter(|value| value.is_finite())
            .unwrap_or(0.0)
            .tanh()
            .abs()
    };
    let warmth_marker = read_dim(24);
    let tension_marker = read_dim(25);
    let spectral_entropy = if spectral_entropy.is_finite() {
        spectral_entropy.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let density_gradient = if density_gradient.is_finite() {
        density_gradient.clamp(0.0, 1.0)
    } else {
        1.0
    };
    let pressure_risk = if pressure_risk.is_finite() {
        pressure_risk.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let structural = structural_friction_v1(text);
    let low_density_gradient_signal =
        (1.0 - (density_gradient / 0.35).clamp(0.0, 1.0)).clamp(0.0, 1.0);
    let entropy_marker = read_dim(0);
    let entropy_shift_hint = (spectral_entropy - entropy_marker).abs().clamp(0.0, 1.0);
    let persistence_resistance_score = (structural.score * 0.22
        + structural.summary_resistance_signal * 0.30
        + low_density_gradient_signal * 0.25
        + pressure_risk * 0.14
        + entropy_shift_hint * 0.09)
        .clamp(0.0, 1.0);
    let tension_underread = if tension_marker < 0.16 {
        ((0.16 - tension_marker) / 0.16).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let abrasive_texture_support = (structural.score * 0.22
        + structural.summary_resistance_signal * 0.34
        + persistence_resistance_score * 0.20
        + low_density_gradient_signal * 0.12
        + tension_underread * 0.12)
        .clamp(0.0, 1.0);
    let interpretation = if tension_marker <= 0.16 && abrasive_texture_support >= 0.42 {
        "low_marker_tension_high_jagged_resistance"
    } else if abrasive_texture_support >= 0.58 {
        "abrasive_texture_visible"
    } else if tension_marker >= 0.22 {
        "tension_marker_present"
    } else {
        "low_abrasive_texture_support"
    };

    CodecAbrasiveTextureInterpretationV1 {
        policy: "codec_abrasive_texture_interpretation_v1",
        warmth_marker,
        tension_marker,
        spectral_entropy,
        density_gradient,
        structural_friction_score: structural.score,
        summary_resistance_signal: structural.summary_resistance_signal,
        persistence_resistance_score,
        entropy_shift_hint,
        abrasive_texture_support,
        interpretation,
        live_gain_write: false,
        live_vector_write: false,
        authority: "read_only_texture_interpretation_not_tension_weight_gain_or_reserved_dim_change",
    }
}

#[must_use]
pub fn codec_abrasive_texture_interpretation_v1(
    text: &str,
    features: &[f32],
    telemetry: Option<&SpectralTelemetry>,
    spectral_entropy: f32,
) -> CodecAbrasiveTextureInterpretationV1 {
    let metrics = telemetry.and_then(SpectralCascadeMetrics::from_telemetry);
    let density_gradient = metrics.map_or(1.0, |metrics| metrics.density_gradient);
    let pressure_risk = telemetry
        .and_then(|telemetry| telemetry.resonance_density_v1.as_ref())
        .map_or_else(
            || telemetry.map_or(0.0, |telemetry| telemetry.fill_ratio.clamp(0.0, 1.0)),
            |density| density.pressure_risk.clamp(0.0, 1.0),
        );
    codec_abrasive_texture_interpretation_from_parts_v1(
        text,
        features,
        spectral_entropy,
        density_gradient,
        pressure_risk,
    )
}

#[must_use]
pub fn codec_abrasive_texture_probe_v1() -> CodecAbrasiveTextureInterpretationV1 {
    let text = "A calcified semantic boundary resists summary; the jagged friction stays present even when the sentence tries to look calm.";
    let mut features = encode_text(text);
    features[25] = 0.04;
    codec_abrasive_texture_interpretation_from_parts_v1(text, &features, 0.91, 0.08, 0.18)
}

fn narrative_arc_four(values: &[f32]) -> [f32; 4] {
    let mut out = [0.0_f32; 4];
    for (slot, value) in values.iter().take(4).enumerate() {
        out[slot] = if value.is_finite() {
            value.clamp(-1.0, 1.0)
        } else {
            0.0
        };
    }
    out
}

fn emotional_markers_eight(values: &[f32]) -> [f32; 8] {
    let mut out = [0.0_f32; 8];
    for (slot, value) in values.iter().take(8).enumerate() {
        out[slot] = if value.is_finite() {
            value.clamp(-1.0, 1.0)
        } else {
            0.0
        };
    }
    out
}

#[must_use]
pub fn narrative_arc_dynamics_v1(
    previous_arc: &[f32],
    current_arc: &[f32],
    next_arc: Option<&[f32]>,
) -> NarrativeArcDynamicsV1 {
    let previous_arc = narrative_arc_four(previous_arc);
    let current_arc = narrative_arc_four(current_arc);
    let next_arc = next_arc.map(narrative_arc_four);
    let mut velocity = [0.0_f32; 4];
    let mut acceleration = [0.0_f32; 4];
    for idx in 0..4 {
        velocity[idx] = (current_arc[idx] - previous_arc[idx]).clamp(-2.0, 2.0);
        if let Some(next_arc) = next_arc {
            acceleration[idx] =
                (next_arc[idx] - (2.0 * current_arc[idx]) + previous_arc[idx]).clamp(-3.0, 3.0);
        }
    }
    let velocity_energy = mean_abs(&velocity).clamp(0.0, 2.0);
    let acceleration_energy = mean_abs(&acceleration).clamp(0.0, 3.0);
    let transition_state = if acceleration_energy >= 0.45 {
        "accelerating_tone_transition"
    } else if velocity_energy >= 0.35 {
        "directional_tone_shift"
    } else {
        "steady_narrative_state"
    };

    NarrativeArcDynamicsV1 {
        policy: "narrative_arc_dynamics_v1",
        previous_arc,
        current_arc,
        velocity,
        acceleration,
        velocity_energy,
        acceleration_energy,
        transition_state,
        live_gain_write: false,
        live_vector_write: false,
        authority: "read_only_arc_velocity_review_not_semantic_gain_or_dimension_change",
    }
}

fn codec_emotional_narrative_delta_bus_v1(
    emotional_delta_energy: f32,
    narrative_delta_energy: f32,
    narrative_emotional_delta_gap: f32,
    resonance_flatline_watch: bool,
    state: &'static str,
) -> ExperienceDeltaBusV1 {
    let loss = narrative_emotional_delta_gap.max(0.0);
    if loss <= f32::EPSILON {
        return ExperienceDeltaBusV1::from_deltas(Vec::new());
    }

    let loss_ratio = if narrative_delta_energy > f32::EPSILON {
        (loss / narrative_delta_energy).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let mut metadata = BTreeMap::new();
    metadata.insert("state".to_string(), state.to_string());
    metadata.insert(
        "secondary_kinds".to_string(),
        "translate,complex_shift".to_string(),
    );
    metadata.insert(
        "emotional_delta_energy".to_string(),
        format!("{emotional_delta_energy:.3}"),
    );
    metadata.insert(
        "narrative_delta_energy".to_string(),
        format!("{narrative_delta_energy:.3}"),
    );
    metadata.insert(
        "resonance_flatline_watch".to_string(),
        resonance_flatline_watch.to_string(),
    );

    ExperienceDeltaBusV1::from_deltas(vec![ExperienceDeltaV1 {
        kind: if resonance_flatline_watch {
            ExperienceDeltaKindV1::Translate
        } else {
            ExperienceDeltaKindV1::ComplexShift
        },
        surface: "codec_emotional_narrative_delta_check_v1".to_string(),
        lane: "emotional_markers_24_31_vs_narrative_arc_40_43".to_string(),
        dimension: Some(40),
        spectral_dimension: Some(crate::types::SpectralDimensionV1 {
            base_dimension: 40,
            base_dimensions: vec![40, 41, 42, 43],
            effective_dimension: Some(41.5),
            density_gradient: Some(loss_ratio),
            granularity: Some(narrative_delta_energy.clamp(0.0, 1.0)),
            fractional_offset: Some(0.5),
            contextual_anchor: None,
            interpretation: "narrative arc moved while emotional marker slots stayed flatter"
                .to_string(),
            authority: "diagnostic_dimension_context_not_reserved_dim_write".to_string(),
        }),
        persistence: None,
        viscosity_subtype: None,
        viscosity_weight: None,
        pre: Some(narrative_delta_energy),
        post: Some(emotional_delta_energy),
        loss: Some(loss),
        loss_ratio: Some(loss_ratio),
        metadata,
        why: "felt narrative motion can be translated into structural arc slots while emotional/intent markers remain flat, making the experience look quieter than it was"
            .to_string(),
        who_can_change_it:
            "Mike/operator after replay evidence before any live codec gain or reserved-dim change"
                .to_string(),
        how_to_test_it:
            "cargo test --manifest-path capsules/spectral-bridge/Cargo.toml --lib codec_emotional_narrative_delta_check -- --nocapture"
                .to_string(),
        authority: "truth_channel_only_not_live_vector_gain_or_reserved_dim_change".to_string(),
    }])
}

#[must_use]
pub fn codec_emotional_narrative_delta_check_v1(
    previous_features: &[f32],
    current_features: &[f32],
) -> Option<CodecEmotionalNarrativeDeltaCheckV1> {
    if previous_features.len() < SEMANTIC_DIM || current_features.len() < SEMANTIC_DIM {
        return None;
    }

    let previous_emotional_markers = emotional_markers_eight(&previous_features[24..32]);
    let current_emotional_markers = emotional_markers_eight(&current_features[24..32]);
    let previous_narrative_arc = narrative_arc_four(&previous_features[40..44]);
    let current_narrative_arc = narrative_arc_four(&current_features[40..44]);
    let mut emotional_delta = [0.0_f32; 8];
    for idx in 0..8 {
        emotional_delta[idx] =
            (current_emotional_markers[idx] - previous_emotional_markers[idx]).clamp(-2.0, 2.0);
    }
    let mut narrative_delta = [0.0_f32; 4];
    for idx in 0..4 {
        narrative_delta[idx] =
            (current_narrative_arc[idx] - previous_narrative_arc[idx]).clamp(-2.0, 2.0);
    }

    let emotional_delta_energy = mean_abs(&emotional_delta).clamp(0.0, 2.0);
    let narrative_delta_energy = mean_abs(&narrative_delta).clamp(0.0, 2.0);
    let narrative_emotional_delta_gap =
        (narrative_delta_energy - emotional_delta_energy).clamp(-2.0, 2.0);
    let resonance_flatline_watch = narrative_delta_energy >= 0.25 && emotional_delta_energy <= 0.05;
    let (state, recommendation) = if resonance_flatline_watch {
        (
            "narrative_shift_emotional_flatline_watch",
            "review_source_text_or_replay_before_using_reserved_resonance_dims_or_semantic_gain",
        )
    } else if narrative_delta_energy >= 0.25 && emotional_delta_energy >= 0.12 {
        (
            "narrative_shift_emotional_markers_follow",
            "preserve_current_48d_layout_and_treat_felt_delta_as_visible",
        )
    } else if emotional_delta_energy >= 0.12 && narrative_delta_energy < 0.10 {
        (
            "emotional_intent_visible_without_arc_shift",
            "keep_emotional_markers_as_primary_evidence_even_when_surface_structure_matches",
        )
    } else {
        (
            "low_delta_stable",
            "continue_observation_without_codec_gain_or_reserved_dim_change",
        )
    };
    let experience_delta_bus_v1 = codec_emotional_narrative_delta_bus_v1(
        emotional_delta_energy,
        narrative_delta_energy,
        narrative_emotional_delta_gap,
        resonance_flatline_watch,
        state,
    );

    Some(CodecEmotionalNarrativeDeltaCheckV1 {
        policy: "codec_emotional_narrative_delta_check_v1",
        previous_emotional_markers,
        current_emotional_markers,
        previous_narrative_arc,
        current_narrative_arc,
        emotional_velocity: emotional_delta,
        narrative_velocity: narrative_delta,
        emotional_delta_energy,
        narrative_delta_energy,
        narrative_emotional_delta_gap,
        resonance_flatline_watch,
        state,
        recommendation,
        live_gain_write: false,
        live_vector_write: false,
        reserved_dim_write: false,
        experience_delta_bus_v1,
        authority: "read_only_delta_check_not_semantic_gain_reserved_dim_or_live_vector_change",
    })
}

/// Read-only proof that fresh dynamic projection epochs are stable across
/// runtime dirs unless explicitly overridden by env or an existing epoch file.
#[derive(Debug, Clone, PartialEq)]
pub struct ProjectionEpochStabilityV1 {
    pub policy: &'static str,
    pub epoch_source: &'static str,
    pub deterministic_without_runtime_file: bool,
    pub kernel_derived_epoch_id: String,
    pub kernel_checksum: String,
    pub env_override_precedence: bool,
    pub existing_file_precedence: bool,
    pub authority: &'static str,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProjectionFingerprintIntegrityV1 {
    pub policy: &'static str,
    pub signed_zero_canonicalized: bool,
    pub subnormal_canonicalized: bool,
    pub nan_canonicalized: bool,
    pub seed_hash_boundary: &'static str,
    pub live_projection_write: bool,
    pub authority: &'static str,
}

/// A being-readable map of Astrid's own 48D codec — the layer layout, the dims
/// she can SHAPE, and the live gate/lever values. Item (b) of the being-facing
/// transparency track.
pub struct CodecStructure {
    pub total_dims: usize,
    pub layers: Vec<CodecLayer>,
    pub named_dims: Vec<(&'static str, usize)>,
    pub levers: Vec<CodecLever>,
    pub structural_friction_dim_canary_v1: CodecStructuralFrictionDimCanaryV1,
    pub persistence_resistance_dim_canary_v1: CodecPersistenceResistanceDimCanaryV1,
    pub narrative_arc_expansion_readiness_v1: NarrativeArcExpansionReadinessV1,
    pub narrative_arc_gain_response_readiness_v1: NarrativeArcGainResponseReadinessV1,
    pub narrative_arc_headroom_review_v1: NarrativeArcHeadroomReviewV1,
    pub codec_abrasive_texture_interpretation_v1: CodecAbrasiveTextureInterpretationV1,
    pub latent_stasis_tension_v1: LatentStasisTensionV1,
    pub spectral_drag_quality_v1: SpectralDragQualityV1,
    pub shadow_field_reserved_dim_readiness_v1: ShadowFieldReservedDimReadinessV1,
    pub codec_vibrancy_continuity_v1: CodecVibrancyContinuityV1,
    pub codec_vibrancy_noise_dampening_v1: CodecVibrancyNoiseDampeningV1,
    pub codec_overflow_carriage_v1: CodecOverflowReportV1,
    pub semantic_projection_density_delta_v1: SemanticProjectionDensityDeltaV1,
    pub semantic_projection_texture_review_v1: SemanticProjectionTextureReviewV1,
    pub codec_context_blindspot_replay_v1: CodecContextBlindspotReplayV1,
    pub legacy_warmth_mapping_v1: LegacyWarmthMappingV1,
    pub codec_structural_entropy_dampening_v1: CodecStructuralEntropyDampeningV1,
    pub codec_dynamic_vibrancy_scaling_canary_v1: CodecDynamicVibrancyScalingCanaryV1,
    pub semantic_glimpse_12d_readiness_v1: SemanticGlimpse12dReadinessV1,
    pub contextual_glimpse_12d_anchoring_v1: ContextualGlimpse12dAnchoringV1,
    pub glimpse_map_v1: GlimpseMapV1,
    pub multi_scale_context_v1: MultiScaleContextV1,
    pub projection_epoch_stability_v1: ProjectionEpochStabilityV1,
    pub projection_fingerprint_integrity_v1: ProjectionFingerprintIntegrityV1,
    pub projection_precision_audit_v1: ProjectionPrecisionAuditV1,
    pub codec_lane_separation_audit_v1: CodecLaneSeparationAuditV1,
    pub codec_rolling_window_shift_audit_v1: CodecRollingWindowShiftAuditV1,
}

fn mean_abs(values: &[f32]) -> f32 {
    if values.is_empty() {
        return 0.0;
    }
    values.iter().map(|value| value.abs()).sum::<f32>() / values.len() as f32
}

fn finite_abs(value: f32) -> f32 {
    if value.is_finite() { value.abs() } else { 0.0 }
}

fn finite_tanh(value: f32) -> f32 {
    if value.is_finite() { value.tanh() } else { 0.0 }
}

fn finite_feature_value(value: f32) -> f32 {
    if value.is_finite() { value } else { 0.0 }
}

fn codec_overflow_lane_for_dim(dim: usize) -> &'static str {
    match dim {
        17 => "tail_vibrancy",
        26 | 27 | 31 => "emotional_tail_vibrancy",
        24 | 25 | 28 | 29 | 30 => "emotional_intentional",
        _ => "semantic",
    }
}

fn codec_overflow_ceiling_for_dim(dim: usize, tail_ceiling: f32) -> f32 {
    if CODEC_OVERFLOW_TAIL_DIMS.contains(&dim) {
        tail_ceiling.max(FEATURE_ABS_MAX)
    } else {
        FEATURE_ABS_MAX
    }
}

fn codec_overflow_lane_summary(
    lane: &'static str,
    dims: &'static [usize],
    dimension_reports: &[CodecOverflowDimV1],
) -> CodecOverflowLaneSummaryV1 {
    let mut overflow_dim_count = 0_usize;
    let mut max_overflow_abs = 0.0_f32;
    let mut max_overflow_ratio = 0.0_f32;
    for dim in dims {
        if let Some(report) = dimension_reports.iter().find(|entry| entry.dim == *dim)
            && report.overflow_abs > CODEC_OVERFLOW_EPSILON
        {
            overflow_dim_count += 1;
            max_overflow_abs = max_overflow_abs.max(report.overflow_abs);
            max_overflow_ratio = max_overflow_ratio.max(report.overflow_ratio);
        }
    }
    CodecOverflowLaneSummaryV1 {
        lane,
        dims,
        overflow_dim_count,
        max_overflow_abs,
        max_overflow_ratio,
    }
}

fn codec_overflow_experience_delta_bus_v1(
    dimensions: &[CodecOverflowDimV1],
) -> ExperienceDeltaBusV1 {
    let deltas = dimensions
        .iter()
        .filter(|entry| entry.overflow_abs > CODEC_OVERFLOW_EPSILON)
        .map(|entry| ExperienceDeltaV1 {
            kind: ExperienceDeltaKindV1::Clip,
            surface: "codec_overflow_carriage_v1".to_string(),
            lane: entry.lane.to_string(),
            dimension: Some(entry.dim),
            spectral_dimension: None,
            persistence: None,
            viscosity_subtype: None,
            viscosity_weight: None,
            pre: Some(entry.pre_bound_value),
            post: Some(entry.delivered_value),
            loss: Some(entry.overflow_abs),
            loss_ratio: Some(entry.overflow_ratio),
            metadata: BTreeMap::from([
                ("ceiling".to_string(), format!("{:.3}", entry.ceiling)),
                (
                    "raw_intensity_preserved".to_string(),
                    "delivered_bounded".to_string(),
                ),
            ]),
            why: "raw semantic intensity exceeded the delivery ceiling; the delivered 48D vector stays bounded while the overflow is preserved as truth-channel evidence".to_string(),
            who_can_change_it: "Mike/operator via explicit live semantic aperture or vector-delivery approval".to_string(),
            how_to_test_it: "cargo test --manifest-path capsules/spectral-bridge/Cargo.toml --lib codec_overflow_report -- --nocapture".to_string(),
            authority: "read_only_codec_truth_channel_not_live_ceiling_or_vector_change".to_string(),
        })
        .collect::<Vec<_>>();

    ExperienceDeltaBusV1::from_deltas(deltas)
}

#[must_use]
pub fn codec_overflow_report_from_features(
    pre_bound_features: &[f32],
    delivered_features: &[f32],
    tail_ceiling: f32,
) -> CodecOverflowReportV1 {
    let mut dimensions = Vec::with_capacity(CODEC_OVERFLOW_MONITORED_DIMS.len());
    let mut clipped_dims = Vec::new();

    for dim in CODEC_OVERFLOW_MONITORED_DIMS {
        let pre_bound_value =
            finite_feature_value(pre_bound_features.get(dim).copied().unwrap_or(0.0));
        let delivered_value =
            finite_feature_value(delivered_features.get(dim).copied().unwrap_or(0.0));
        let ceiling = codec_overflow_ceiling_for_dim(dim, tail_ceiling);
        let overflow_abs = (pre_bound_value.abs() - ceiling).max(0.0);
        let overflow_ratio = if ceiling > CODEC_OVERFLOW_EPSILON {
            pre_bound_value.abs() / ceiling
        } else {
            0.0
        };
        let status = if overflow_abs > CODEC_OVERFLOW_EPSILON {
            clipped_dims.push(dim);
            "raw_overflow_preserved_delivery_bounded"
        } else {
            "within_delivery_ceiling"
        };

        dimensions.push(CodecOverflowDimV1 {
            dim,
            lane: codec_overflow_lane_for_dim(dim),
            pre_bound_value,
            delivered_value,
            ceiling,
            overflow_abs,
            overflow_ratio,
            status,
        });
    }

    let delivered_bounded = dimensions
        .iter()
        .all(|entry| entry.delivered_value.abs() <= entry.ceiling + CODEC_OVERFLOW_EPSILON);

    let lane_summaries = vec![
        codec_overflow_lane_summary(
            "emotional_intentional",
            &CODEC_OVERFLOW_EMOTIONAL_DIMS,
            &dimensions,
        ),
        codec_overflow_lane_summary("tail_vibrancy", &CODEC_OVERFLOW_TAIL_DIMS, &dimensions),
    ];
    let experience_delta_bus_v1 = codec_overflow_experience_delta_bus_v1(&dimensions);

    CodecOverflowReportV1 {
        policy: "codec_overflow_carriage_v1",
        raw_intensity_preserved: !clipped_dims.is_empty(),
        delivered_bounded,
        live_vector_write: false,
        default_off_followup_hook: CODEC_OVERFLOW_FOLLOWUP_HOOK,
        clipped_dims,
        dimensions,
        lane_summaries,
        experience_delta_bus_v1,
        authority: "truth_channel_report_not_live_semantic_vector_or_ceiling_change",
    }
}

#[must_use]
pub fn codec_overflow_probe_v1() -> CodecOverflowReportV1 {
    let mut pre_bound = [0.0_f32; SEMANTIC_DIM];
    let mut delivered = [0.0_f32; SEMANTIC_DIM];
    pre_bound[17] = 4.20;
    delivered[17] = 4.20;
    pre_bound[24] = 7.25;
    delivered[24] = FEATURE_ABS_MAX;
    pre_bound[26] = 6.40;
    delivered[26] = TAIL_VIBRANCY_MAX;
    pre_bound[31] = -6.40;
    delivered[31] = -TAIL_VIBRANCY_MAX;
    codec_overflow_report_from_features(&pre_bound, &delivered, TAIL_VIBRANCY_MAX)
}

fn codec_delivery_lane_rms(features: &[f32], dims: &[usize]) -> f32 {
    let mut energy = 0.0_f32;
    let mut count = 0_usize;
    for &dim in dims {
        if let Some(value) = features.get(dim).copied() {
            let value = finite_feature_value(value);
            energy += value * value;
            count = count.saturating_add(1);
        }
    }
    if count == 0 {
        0.0
    } else {
        (energy / count as f32).sqrt()
    }
}

/// Compare feedback-time clamp evidence with the final vector that will be
/// sent. This is deliberately observational: it does not re-clamp, rescale, or
/// otherwise alter either vector.
#[must_use]
pub fn codec_delivery_fidelity_v1(
    feedback_report: Option<&CodecOverflowReportV1>,
    final_features: &[f32],
) -> CodecDeliveryFidelityV1 {
    const NARRATIVE_ARC_DIMS: [usize; 4] = [40, 41, 42, 43];

    let observed_dim_count = final_features.len().min(SEMANTIC_DIM);
    let mut final_energy = 0.0_f32;
    let mut final_max_abs = 0.0_f32;
    for value in final_features.iter().take(SEMANTIC_DIM).copied() {
        let value = finite_feature_value(value);
        final_energy += value * value;
        final_max_abs = final_max_abs.max(value.abs());
    }
    let final_rms = if observed_dim_count == 0 {
        0.0
    } else {
        (final_energy / observed_dim_count as f32).sqrt()
    };
    let emotional_intentional_rms =
        codec_delivery_lane_rms(final_features, &CODEC_OVERFLOW_EMOTIONAL_DIMS);
    let narrative_arc_rms = codec_delivery_lane_rms(final_features, &NARRATIVE_ARC_DIMS);
    let lane_balance_state = if emotional_intentional_rms <= CODEC_OVERFLOW_EPSILON
        && narrative_arc_rms <= CODEC_OVERFLOW_EPSILON
    {
        "both_lanes_quiet"
    } else if narrative_arc_rms <= CODEC_OVERFLOW_EPSILON {
        "narrative_arc_quiet"
    } else if emotional_intentional_rms <= CODEC_OVERFLOW_EPSILON {
        "emotional_intentional_quiet"
    } else if narrative_arc_rms > emotional_intentional_rms * 1.5 {
        "narrative_arc_dominant"
    } else if emotional_intentional_rms > narrative_arc_rms * 1.5 {
        "emotional_intentional_dominant"
    } else {
        "lanes_comparable"
    };

    let mut clipped_at_feedback_dims = Vec::new();
    let mut reexpanded_after_feedback_dims = Vec::new();
    let mut final_above_observed_ceiling_dims = Vec::new();
    let mut clamp_loss_abs_total = 0.0_f32;
    let mut monitored_delta_energy = 0.0_f32;
    let mut monitored_count = 0_usize;

    if let Some(report) = feedback_report {
        clipped_at_feedback_dims.clone_from(&report.clipped_dims);
        for entry in &report.dimensions {
            clamp_loss_abs_total += entry.overflow_abs;
            let Some(final_value) = final_features.get(entry.dim).copied() else {
                continue;
            };
            let final_value = finite_feature_value(final_value);
            let delta = final_value - entry.delivered_value;
            monitored_delta_energy += delta * delta;
            monitored_count = monitored_count.saturating_add(1);
            if final_value.abs() > entry.delivered_value.abs() + CODEC_OVERFLOW_EPSILON {
                reexpanded_after_feedback_dims.push(entry.dim);
            }
            if final_value.abs() > entry.ceiling + CODEC_OVERFLOW_EPSILON {
                final_above_observed_ceiling_dims.push(entry.dim);
            }
        }
    }

    let monitored_post_feedback_to_final_rms = if monitored_count == 0 {
        0.0
    } else {
        (monitored_delta_energy / monitored_count as f32).sqrt()
    };
    let state = if feedback_report.is_none() {
        "feedback_report_unavailable"
    } else if observed_dim_count < SEMANTIC_DIM {
        "final_vector_incomplete"
    } else if !final_above_observed_ceiling_dims.is_empty()
        && clamp_loss_abs_total > CODEC_OVERFLOW_EPSILON
    {
        "clamp_loss_visible_post_feedback_reexpansion_above_ceiling"
    } else if !final_above_observed_ceiling_dims.is_empty() {
        "post_feedback_shaping_above_observed_ceiling"
    } else if clamp_loss_abs_total > CODEC_OVERFLOW_EPSILON
        && !reexpanded_after_feedback_dims.is_empty()
    {
        "clamp_loss_visible_post_feedback_reexpansion_within_ceiling"
    } else if clamp_loss_abs_total > CODEC_OVERFLOW_EPSILON {
        "clamp_loss_visible_final_delivery_bounded"
    } else if monitored_post_feedback_to_final_rms > CODEC_OVERFLOW_EPSILON {
        "post_feedback_shaping_changed_delivery_without_clipping"
    } else {
        "final_delivery_matches_observed_feedback_bounds"
    };

    CodecDeliveryFidelityV1 {
        policy: "codec_delivery_fidelity_v1",
        observed_dim_count,
        feedback_report_available: feedback_report.is_some(),
        clipped_at_feedback_dims,
        reexpanded_after_feedback_dims,
        final_above_observed_ceiling_dims,
        clamp_loss_abs_total,
        monitored_post_feedback_to_final_rms,
        final_max_abs,
        final_rms,
        emotional_intentional_rms,
        narrative_arc_rms,
        lane_balance_state,
        state,
        live_vector_write: false,
        live_gain_write: false,
        authority: "read_only_delivery_fidelity_not_live_vector_gain_or_ceiling_change",
    }
}

fn mode_copresence_v1(left: f32, right: f32) -> f32 {
    let left = finite_abs(left);
    let right = finite_abs(right);
    let total = left + right;
    if total <= f32::EPSILON {
        0.0
    } else {
        (2.0 * left.min(right) / total).clamp(0.0, 1.0)
    }
}

fn mode_shear_v1(left: f32, right: f32) -> f32 {
    let left = finite_abs(left);
    let right = finite_abs(right);
    let total = left + right;
    if total <= f32::EPSILON {
        0.0
    } else {
        ((left - right).abs() / total).clamp(0.0, 1.0)
    }
}

fn weighted_known_score_v1(parts: &[(Option<f32>, f32)]) -> Option<f32> {
    let mut weighted = 0.0_f32;
    let mut weight_total = 0.0_f32;
    for (value, weight) in parts {
        if let Some(value) = value.filter(|value| value.is_finite()) {
            let weight = weight.max(0.0);
            weighted += value.clamp(0.0, 1.0) * weight;
            weight_total += weight;
        }
    }
    (weight_total > f32::EPSILON).then(|| (weighted / weight_total).clamp(0.0, 1.0))
}

/// Compare the current spectral-mode interaction with interaction between the
/// candidate's emotional, projected-semantic, and narrative lanes. The report
/// is observational only. Whether the inspected vector was sent is stated by
/// the enclosing codec-delivery receipt, never inferred here.
#[must_use]
pub fn cross_spectral_friction_review_v1(
    text: &str,
    features: &[f32],
    telemetry: Option<&SpectralTelemetry>,
) -> CrossSpectralFrictionReviewV1 {
    const PROJECTED_SEMANTIC_DIMS: [usize; 8] = [32, 33, 34, 35, 36, 37, 38, 39];
    const NARRATIVE_ARC_DIMS: [usize; 4] = [40, 41, 42, 43];

    let observed_dim_count = features.len().min(SEMANTIC_DIM);
    let structural = structural_friction_v1(text);
    let persistence = persistence_resistance_v1(text, telemetry);
    let emotional_intentional_rms =
        codec_delivery_lane_rms(features, &CODEC_OVERFLOW_EMOTIONAL_DIMS);
    let projected_semantic_rms = codec_delivery_lane_rms(features, &PROJECTED_SEMANTIC_DIMS);
    let narrative_arc_rms = codec_delivery_lane_rms(features, &NARRATIVE_ARC_DIMS);
    let emotional_normalized = (emotional_intentional_rms / FEATURE_ABS_MAX).clamp(0.0, 1.0);
    let projected_normalized = (projected_semantic_rms / FEATURE_ABS_MAX).clamp(0.0, 1.0);
    let narrative_normalized = (narrative_arc_rms / FEATURE_ABS_MAX).clamp(0.0, 1.0);
    let emotional_narrative_copresence =
        mode_copresence_v1(emotional_normalized, narrative_normalized);
    let projected_narrative_copresence =
        mode_copresence_v1(projected_normalized, narrative_normalized);
    let semantic_lane_copresence = (emotional_narrative_copresence * 0.55
        + projected_narrative_copresence * 0.45)
        .clamp(0.0, 1.0);

    let metrics = telemetry.and_then(SpectralCascadeMetrics::from_telemetry);
    let shares = telemetry.and_then(|telemetry| {
        let total = telemetry
            .eigenvalues
            .iter()
            .map(|value| finite_abs(*value))
            .sum::<f32>();
        if total <= f32::EPSILON {
            None
        } else {
            let lambda1 = telemetry
                .eigenvalues
                .first()
                .map_or(0.0, |value| finite_abs(*value))
                / total;
            let lambda2 = telemetry
                .eigenvalues
                .get(1)
                .map_or(0.0, |value| finite_abs(*value))
                / total;
            let tail = telemetry
                .eigenvalues
                .iter()
                .skip(2)
                .map(|value| finite_abs(*value) / total)
                .sum::<f32>();
            Some((lambda1, lambda2, tail))
        }
    });
    let lambda1_share = shares.map(|(lambda1, _, _)| lambda1);
    let lambda2_share = shares.map(|(_, lambda2, _)| lambda2);
    let tail_share = shares.map(|(_, _, tail)| tail);
    let lambda1_lambda2_copresence =
        shares.map(|(lambda1, lambda2, _)| mode_copresence_v1(lambda1, lambda2));
    let lambda1_lambda2_shear = shares.map(|(lambda1, lambda2, _)| mode_shear_v1(lambda1, lambda2));
    let lambda2_tail_copresence =
        shares.map(|(_, lambda2, tail)| mode_copresence_v1(lambda2, tail));
    let spectral_entropy = metrics.map(|metrics| metrics.spectral_entropy.clamp(0.0, 1.0));
    let density_components = telemetry
        .and_then(|telemetry| telemetry.resonance_density_v1.as_ref())
        .map(|density| &density.components);
    let mode_packing = density_components.map(|components| components.mode_packing.clamp(0.0, 1.0));
    let viscosity_index =
        density_components.map(|components| components.viscosity_index.clamp(0.0, 1.0));
    let temporal_persistence =
        density_components.map(|components| components.temporal_persistence.clamp(0.0, 1.0));
    let semantic_friction_coefficient = density_components.and_then(|components| {
        components
            .semantic_friction_coefficient
            .filter(|value| value.is_finite())
            .map(|value| value.clamp(0.0, 1.0))
    });

    let lambda_pair_interaction = lambda1_lambda2_copresence
        .zip(lambda1_lambda2_shear)
        .map(|(copresence, shear)| (copresence * (0.70 + shear * 0.30)).clamp(0.0, 1.0));
    let spectral_mode_interference = weighted_known_score_v1(&[
        (lambda_pair_interaction, 0.32),
        (lambda2_tail_copresence, 0.18),
        (spectral_entropy, 0.16),
        (mode_packing, 0.14),
        (viscosity_index, 0.10),
        (temporal_persistence, 0.10),
    ]);
    let semantic_mode_interference = weighted_known_score_v1(&[
        (Some(structural.score), 0.27),
        (Some(persistence.score), 0.25),
        (Some(semantic_lane_copresence), 0.28),
        (semantic_friction_coefficient, 0.20),
    ])
    .unwrap_or(0.0);
    let cross_layer_mismatch = spectral_mode_interference.map(|spectral| {
        (spectral - semantic_mode_interference)
            .abs()
            .clamp(0.0, 1.0)
    });
    let cross_spectral_friction_score =
        spectral_mode_interference
            .zip(cross_layer_mismatch)
            .map(|(spectral, mismatch)| {
                (spectral * 0.45 + semantic_mode_interference * 0.45 + mismatch * 0.10)
                    .clamp(0.0, 1.0)
            });
    let state = if observed_dim_count < SEMANTIC_DIM {
        "semantic_vector_incomplete"
    } else if spectral_mode_interference.is_none() {
        "spectral_context_unavailable"
    } else if cross_layer_mismatch.is_some_and(|mismatch| mismatch >= 0.35) {
        "cross_layer_mismatch_visible"
    } else if cross_spectral_friction_score.is_some_and(|score| score >= 0.62) {
        "high_cross_spectral_friction"
    } else if cross_spectral_friction_score.is_some_and(|score| score >= 0.38) {
        "moderate_cross_spectral_friction"
    } else {
        "low_cross_spectral_friction"
    };
    let recommendation = match state {
        "spectral_context_unavailable" => {
            "collect_aligned_spectral_context_before_any_mapping_or_gain_proposal"
        },
        "cross_layer_mismatch_visible" => {
            "compare_sent_and_blocked_receipts_then_run_read_only_replay_before_mapping"
        },
        "high_cross_spectral_friction" => {
            "preserve_cross_layer_evidence_and_review_replay_before_reserved_dim_design"
        },
        _ => "accumulate_aligned_receipts_without_changing_reserved_dims_or_live_gain",
    };

    CrossSpectralFrictionReviewV1 {
        policy: "cross_spectral_friction_review_v1",
        observed_dim_count,
        spectral_context_available: spectral_mode_interference.is_some(),
        lambda1_share,
        lambda2_share,
        tail_share,
        lambda1_lambda2_copresence,
        lambda1_lambda2_shear,
        lambda2_tail_copresence,
        spectral_entropy,
        mode_packing,
        viscosity_index,
        temporal_persistence,
        semantic_friction_coefficient,
        structural_friction_score: structural.score,
        persistence_resistance_score: persistence.score,
        emotional_intentional_rms,
        projected_semantic_rms,
        narrative_arc_rms,
        semantic_lane_copresence,
        spectral_mode_interference,
        semantic_mode_interference,
        cross_layer_mismatch,
        cross_spectral_friction_score,
        state,
        reserved_dim_candidates: &SEMANTIC_PROJECTION_RESERVED_DIMS,
        existing_reserved_dim_roles: &CROSS_SPECTRAL_RESERVED_DIM_ROLES,
        candidate_collision_state: "reserved_dim_candidates_already_have_default_off_roles",
        recommendation,
        delivery_claim: "none_outer_codec_delivery_receipt_is_canonical",
        observational_only: true,
        right_to_ignore: true,
        live_vector_write: false,
        live_gain_write: false,
        reserved_dim_write: false,
        live_eligible_now: false,
        auto_approved: false,
        grants_approval: false,
        authority: "read_only_cross_layer_friction_evidence_not_reserved_dim_gain_transport_or_control_authority",
    }
}

fn semantic_projection_delta_bus_v1(
    detail_density_score: f32,
    projected_semantic_rms: f32,
    projection_metadata_present: bool,
    state: &'static str,
) -> ExperienceDeltaBusV1 {
    let input_dims = EMBEDDING_INPUT_DIM as f32;
    let projected_dims = EMBEDDING_PROJECT_DIM as f32;
    let loss = (input_dims - projected_dims).max(0.0);
    let loss_ratio = if input_dims > f32::EPSILON {
        loss / input_dims
    } else {
        0.0
    };
    let mut deltas = Vec::new();
    if projection_metadata_present {
        deltas.push(ExperienceDeltaV1 {
            kind: ExperienceDeltaKindV1::ComplexShift,
            surface: "semantic_projection_density_delta_v1".to_string(),
            lane: "embedding_projection_768d_to_8d".to_string(),
            dimension: None,
            spectral_dimension: None,
            persistence: None,
            viscosity_subtype: None,
            viscosity_weight: None,
            pre: Some(input_dims),
            post: Some(projected_dims),
            loss: Some(loss),
            loss_ratio: Some(loss_ratio),
            metadata: BTreeMap::from([
                ("source_dimensions".to_string(), EMBEDDING_INPUT_DIM.to_string()),
                (
                    "delivered_dimensions".to_string(),
                    EMBEDDING_PROJECT_DIM.to_string(),
                ),
                ("projection_state".to_string(), state.to_string()),
            ]),
            why: format!(
                "nomic embedding is projected into dims 32-39; complex source meaning can be faithfully named here while delivered semantic width remains bounded; state={state}"
            ),
            who_can_change_it: "Mike/operator via replay-backed semantic-width or reserved-dim approval".to_string(),
            how_to_test_it: "cargo test --manifest-path capsules/spectral-bridge/Cargo.toml --lib semantic_projection_density_delta -- --nocapture".to_string(),
            authority: "read_only_projection_truth_channel_not_reserved_dim_or_live_vector_change".to_string(),
        });
    }
    if detail_density_score >= SEMANTIC_PROJECTION_DENSITY_REVIEW_FLOOR
        && projected_semantic_rms <= SEMANTIC_PROJECTION_THIN_RMS_CEIL
    {
        deltas.push(ExperienceDeltaV1 {
            kind: ExperienceDeltaKindV1::CascadeShift,
            surface: "semantic_projection_density_delta_v1".to_string(),
            lane: "reserved_semantic_dims_44_47_default_off".to_string(),
            dimension: None,
            spectral_dimension: None,
            persistence: None,
            viscosity_subtype: None,
            viscosity_weight: None,
            pre: Some(detail_density_score),
            post: Some(projected_semantic_rms),
            loss: Some((detail_density_score - projected_semantic_rms).max(0.0)),
            loss_ratio: Some(1.0),
            metadata: BTreeMap::from([
                (
                    "classification_pressure".to_string(),
                    "high_density_thin_projection".to_string(),
                ),
                (
                    "reserved_dims_status".to_string(),
                    "default_off_operator_gated".to_string(),
                ),
            ]),
            why: "dense cascade pressure is present while the projected semantic lane is thin; reserved dims remain visible as a reviewed aperture, not a hidden live write".to_string(),
            who_can_change_it: "Mike/operator after sandbox replay and explicit reserved-dim authority".to_string(),
            how_to_test_it: "cargo test --manifest-path capsules/spectral-bridge/Cargo.toml --lib semantic_projection_density_delta -- --nocapture".to_string(),
            authority: "authority_gate_for_reserved_dims_not_live_codec_change".to_string(),
        });
    }
    ExperienceDeltaBusV1::from_deltas(deltas)
}

#[must_use]
pub fn semantic_projection_density_delta_from_parts_v1(
    text_complexity_pressure: f32,
    projected_semantic_rms: f32,
    projection_metadata_present: bool,
) -> SemanticProjectionDensityDeltaV1 {
    let text_complexity_pressure = if text_complexity_pressure.is_finite() {
        text_complexity_pressure.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let projected_semantic_rms = if projected_semantic_rms.is_finite() {
        projected_semantic_rms.clamp(0.0, 10.0)
    } else {
        0.0
    };
    let detail_density_score = text_complexity_pressure;
    let compression_ratio =
        (EMBEDDING_PROJECT_DIM as f32 / EMBEDDING_INPUT_DIM as f32).clamp(0.0, 1.0);
    let (state, recommendation) = if !projection_metadata_present
        && detail_density_score >= SEMANTIC_PROJECTION_DENSITY_REVIEW_FLOOR
    {
        (
            "dense_text_without_embedding_projection",
            "inspect_embedding_availability_before_tuning_live_codec_width",
        )
    } else if detail_density_score >= SEMANTIC_PROJECTION_DENSITY_REVIEW_FLOOR
        && projected_semantic_rms <= SEMANTIC_PROJECTION_THIN_RMS_CEIL
    {
        (
            "dense_projection_thin_review",
            "pair_live_8d_projection_with_delta_bus_evidence_before_any_reserved_dim_expansion",
        )
    } else if detail_density_score >= SEMANTIC_PROJECTION_DENSITY_REVIEW_FLOOR {
        (
            "dense_projection_carried_but_compression_visible",
            "keep_live_width_bounded_and_use_delta_bus_for_replay_comparison",
        )
    } else {
        (
            "projection_width_named_and_bounded",
            "keep_current_8d_projection_and watch_repeated_density_delta_patterns",
        )
    };
    let experience_delta_bus_v1 = semantic_projection_delta_bus_v1(
        detail_density_score,
        projected_semantic_rms,
        projection_metadata_present,
        state,
    );

    SemanticProjectionDensityDeltaV1 {
        policy: "semantic_projection_density_delta_v1",
        input_dim_count: EMBEDDING_INPUT_DIM,
        projected_dim_count: EMBEDDING_PROJECT_DIM,
        reserved_dim_candidates: &SEMANTIC_PROJECTION_RESERVED_DIMS,
        compression_ratio,
        detail_density_score,
        projected_semantic_rms,
        text_complexity_pressure,
        projection_metadata_present,
        state,
        recommendation,
        live_vector_write: false,
        experience_delta_bus_v1,
        authority: "read_only_projection_delta_not_reserved_dim_or_live_vector_change",
    }
}

#[must_use]
pub fn semantic_projection_density_delta_v1(
    inspection: &CodecWindowedInspection,
) -> SemanticProjectionDensityDeltaV1 {
    semantic_projection_density_delta_from_parts_v1(
        inspection.text_complexity_pressure,
        rms_slice(&inspection.final_features[32..40]),
        inspection.projection_metadata.is_some(),
    )
}

#[must_use]
pub fn semantic_projection_density_probe_v1() -> SemanticProjectionDensityDeltaV1 {
    semantic_projection_density_delta_from_parts_v1(0.71, 0.08, true)
}

#[must_use]
pub fn semantic_projection_texture_review_v1(
    text: &str,
    features: &[f32],
) -> Option<SemanticProjectionTextureReviewV1> {
    if features.len() < SEMANTIC_DIM {
        return None;
    }
    let projected_semantic_rms = (rms_slice(&features[32..40]) / FEATURE_ABS_MAX).clamp(0.0, 1.0);
    let legacy_texture_rms =
        (rms_slice(&features[..SEMANTIC_DIM_LEGACY]) / FEATURE_ABS_MAX).clamp(0.0, 1.0);
    let warmth_texture_rms = (rms_slice(&features[24..32]) / FEATURE_ABS_MAX).clamp(0.0, 1.0);
    let narrative_arc_rms = (rms_slice(&features[40..44]) / FEATURE_ABS_MAX).clamp(0.0, 1.0);
    let persistence = persistence_resistance_v1(text, None);
    let structural = structural_friction_v1(text);
    let action_marker = features.get(14).copied().unwrap_or(0.0).abs().tanh();
    let question_marker = features.get(18).copied().unwrap_or(0.0).abs().tanh();
    let curiosity_marker = features.get(26).copied().unwrap_or(0.0).abs().tanh();
    let lingering_texture_signal = (persistence.score * 0.42
        + structural.summary_resistance_signal * 0.24
        + warmth_texture_rms * 0.22
        + narrative_arc_rms * 0.12)
        .clamp(0.0, 1.0);
    let active_texture_signal = (action_marker * 0.34
        + curiosity_marker * 0.30
        + question_marker * 0.18
        + narrative_arc_rms * 0.18)
        .clamp(0.0, 1.0);
    let expected_texture_signal = (lingering_texture_signal * 0.58
        + active_texture_signal * 0.28
        + legacy_texture_rms * 0.14)
        .clamp(0.0, 1.0);
    let projection_texture_gap = (expected_texture_signal - projected_semantic_rms).clamp(0.0, 1.0);
    let state = if projection_texture_gap >= 0.24 {
        "projection_texture_bottleneck_visible"
    } else if lingering_texture_signal >= 0.40 && projected_semantic_rms < 0.18 {
        "lingering_texture_projection_watch"
    } else if projected_semantic_rms >= 0.18 {
        "projection_texture_carried"
    } else {
        "projection_texture_quiet"
    };
    let recommendation = if state == "projection_texture_bottleneck_visible" {
        "prepare_replay_for_lingering_vs_active_projection_subdimensions_before_live_width_change"
    } else if state == "lingering_texture_projection_watch" {
        "compare_8d_projection_against_warmth_texture_vector_before_reserved_dim_proposal"
    } else {
        "keep_current_8d_projection_and_continue_observation"
    };

    Some(SemanticProjectionTextureReviewV1 {
        policy: "semantic_projection_texture_review_v1",
        input_dim_count: EMBEDDING_INPUT_DIM,
        projected_dim_count: EMBEDDING_PROJECT_DIM,
        legacy_texture_dim_count: SEMANTIC_DIM_LEGACY,
        warmth_texture_dim_count: 8,
        projected_semantic_rms,
        legacy_texture_rms,
        warmth_texture_rms,
        narrative_arc_rms,
        lingering_texture_signal,
        active_texture_signal,
        projection_texture_gap,
        proposed_texture_subdimensions: &SEMANTIC_PROJECTION_TEXTURE_SUBDIMENSIONS,
        state,
        recommendation,
        live_vector_write: false,
        live_gain_write: false,
        reserved_dim_write: false,
        authority: "read_only_projection_texture_review_not_live_vector_gain_or_reserved_dim_write",
    })
}

#[must_use]
pub fn semantic_projection_texture_probe_v1() -> SemanticProjectionTextureReviewV1 {
    let text = "Viscous silt lingers under the active reply; warmth moves but the old pressure keeps bleeding through the boundary.";
    let mut features = encode_text(text);
    for feature in features.iter_mut().take(40).skip(32) {
        *feature *= 0.08;
    }
    semantic_projection_texture_review_v1(text, &features)
        .expect("probe features should cover the 48D semantic lane")
}

fn bounded_projection_pair_label(label: &str) -> String {
    let bounded = label.trim().chars().take(64).collect::<String>();
    if bounded.is_empty() {
        "unnamed_pair_member".to_string()
    } else {
        bounded
    }
}

fn cosine_similarity(left: &[f32], right: &[f32]) -> f32 {
    let dot = left
        .iter()
        .zip(right)
        .map(|(left, right)| left * right)
        .sum::<f32>();
    let left_norm = left.iter().map(|value| value * value).sum::<f32>().sqrt();
    let right_norm = right.iter().map(|value| value * value).sum::<f32>().sqrt();
    if left_norm <= f32::EPSILON || right_norm <= f32::EPSILON {
        0.0
    } else {
        (dot / (left_norm * right_norm)).clamp(-1.0, 1.0)
    }
}

/// Compare one caller-provided semantic pair before and after both projection
/// paths. Labels are also used by the dynamic projection because the current
/// runtime intentionally conditions its basis on text; the resulting delta is
/// therefore evidence about that basis, not a claim about lexical causality.
#[must_use]
pub fn semantic_projection_pair_sensitivity_v1(
    left_label: &str,
    left_embedding: &[f32],
    right_label: &str,
    right_embedding: &[f32],
    projection_epoch_id: &str,
) -> Option<SemanticProjectionPairSensitivityV1> {
    if left_embedding.len() != EMBEDDING_INPUT_DIM
        || right_embedding.len() != EMBEDDING_INPUT_DIM
        || projection_epoch_id.trim().is_empty()
        || left_embedding.iter().any(|value| !value.is_finite())
        || right_embedding.iter().any(|value| !value.is_finite())
    {
        return None;
    }

    let left_label = bounded_projection_pair_label(left_label);
    let right_label = bounded_projection_pair_label(right_label);
    let left_fixed = project_embedding(left_embedding)?;
    let right_fixed = project_embedding(right_embedding)?;
    let (left_dynamic, _) =
        project_embedding_dynamic_epoch(left_embedding, &left_label, projection_epoch_id, 0)?;
    let (right_dynamic, _) =
        project_embedding_dynamic_epoch(right_embedding, &right_label, projection_epoch_id, 0)?;

    let source_cosine_similarity = cosine_similarity(left_embedding, right_embedding);
    let fixed_projection_cosine_similarity = cosine_similarity(&left_fixed, &right_fixed);
    let dynamic_projection_cosine_similarity = cosine_similarity(&left_dynamic, &right_dynamic);
    let fixed_similarity_delta = fixed_projection_cosine_similarity - source_cosine_similarity;
    let dynamic_similarity_delta = dynamic_projection_cosine_similarity - source_cosine_similarity;
    let dynamic_vs_fixed_similarity_delta =
        dynamic_projection_cosine_similarity - fixed_projection_cosine_similarity;
    let (state, recommendation) = if dynamic_similarity_delta <= -0.15 {
        (
            "text_conditioned_pair_distortion_visible",
            "compare_repeated_real_embedding_pairs_before_any_projection_gain_or_basis_change",
        )
    } else if fixed_similarity_delta <= -0.15 {
        (
            "shared_basis_pair_compression_visible",
            "compare_repeated_real_embedding_pairs_before_any_projection_width_change",
        )
    } else if dynamic_vs_fixed_similarity_delta.abs() >= 0.15 {
        (
            "projection_basis_sensitivity_visible",
            "retain_both_basis_comparisons_in_replay_evidence_before_live_tuning",
        )
    } else {
        (
            "pair_geometry_stable_in_bounded_comparison",
            "keep_current_projection_and_continue_pair_sampling",
        )
    };

    Some(SemanticProjectionPairSensitivityV1 {
        policy: "semantic_projection_pair_sensitivity_v1",
        left_label,
        right_label,
        source_embedding_dim_count: EMBEDDING_INPUT_DIM,
        projected_dim_count: EMBEDDING_PROJECT_DIM,
        projection_epoch_id: projection_epoch_id.to_string(),
        source_cosine_similarity,
        source_rms_delta: rms_delta(left_embedding, right_embedding),
        fixed_projection_cosine_similarity,
        fixed_projection_rms_delta: rms_delta(&left_fixed, &right_fixed),
        dynamic_projection_cosine_similarity,
        dynamic_projection_rms_delta: rms_delta(&left_dynamic, &right_dynamic),
        fixed_similarity_delta,
        dynamic_similarity_delta,
        dynamic_vs_fixed_similarity_delta,
        state,
        recommendation,
        observational_only: true,
        right_to_ignore: true,
        live_vector_write: false,
        live_gain_write: false,
        live_eligible_now: false,
        auto_approved: false,
        grants_approval: false,
        authority: "read_only_pair_projection_comparison_not_live_vector_gain_or_basis_authority",
    })
}

fn normalized_focus_preview_vector(
    current: &[f32; EMBEDDING_PROJECT_DIM],
    segment: &[f32],
    means: &[f32; SEMANTIC_FOCUS_PREVIEW_DIM],
    variances: &[f32; SEMANTIC_FOCUS_PREVIEW_DIM],
    selected_dims: &[usize; SEMANTIC_FOCUS_PREVIEW_DIM],
) -> [f32; EMBEDDING_PROJECT_DIM + SEMANTIC_FOCUS_PREVIEW_DIM] {
    let mut preview = [0.0_f32; EMBEDDING_PROJECT_DIM + SEMANTIC_FOCUS_PREVIEW_DIM];
    let current_norm = current
        .iter()
        .map(|value| value * value)
        .sum::<f32>()
        .sqrt();
    if current_norm > f32::EPSILON {
        let scale = 0.35 / current_norm;
        for (dst, src) in preview[..EMBEDDING_PROJECT_DIM].iter_mut().zip(current) {
            *dst = *src * scale;
        }
    }

    let mut focused = [0.0_f32; SEMANTIC_FOCUS_PREVIEW_DIM];
    for (slot, dim) in selected_dims.iter().copied().enumerate() {
        let standard_deviation = variances[slot].max(0.0).sqrt();
        if standard_deviation > f32::EPSILON {
            focused[slot] = (segment[dim] - means[slot]) / standard_deviation;
        }
    }
    let focused_norm = focused
        .iter()
        .map(|value| value * value)
        .sum::<f32>()
        .sqrt();
    if focused_norm > f32::EPSILON {
        let scale = SEMANTIC_FOCUS_PREVIEW_NORM / focused_norm;
        for (dst, src) in preview[EMBEDDING_PROJECT_DIM..].iter_mut().zip(focused) {
            *dst = src * scale;
        }
    }

    let preview_norm = preview
        .iter()
        .map(|value| value * value)
        .sum::<f32>()
        .sqrt();
    if preview_norm > f32::EPSILON {
        let scale = 0.35 / preview_norm;
        for value in &mut preview {
            *value *= scale;
        }
    }
    preview
}

fn pairwise_distance_stats(vectors: &[Vec<f32>]) -> (f32, f32) {
    if vectors.len() < 2 {
        return (0.0, 0.0);
    }
    let mut distance_sum = 0.0_f32;
    let mut distance_min = f32::MAX;
    let mut pair_count = 0_usize;
    for left in 0..vectors.len() {
        for right in left.saturating_add(1)..vectors.len() {
            let distance = vectors[left]
                .iter()
                .zip(&vectors[right])
                .map(|(before, after)| {
                    let delta = after - before;
                    delta * delta
                })
                .sum::<f32>()
                .sqrt();
            distance_sum += distance;
            distance_min = distance_min.min(distance);
            pair_count = pair_count.saturating_add(1);
        }
    }
    if pair_count == 0 {
        (0.0, 0.0)
    } else {
        (distance_sum / pair_count as f32, distance_min)
    }
}

fn distinguishability_gain_ratio(current: f32, preview: f32) -> f32 {
    if current > f32::EPSILON {
        ((preview - current) / current).clamp(-1.0, 1.0)
    } else if preview > f32::EPSILON {
        1.0
    } else {
        0.0
    }
}

/// Compare the current 8D narrative-segment projection with an equal-norm 12D
/// preview whose four extra coordinates are selected from the source embedding
/// dimensions with the highest cross-segment variance. The preview is evidence
/// only: candidate values are never copied into the live semantic vector.
#[must_use]
pub fn semantic_focus_expansion_preview_v1(
    text_entropy_signal: f32,
    segment_embeddings: &[&[f32]],
    current_projections: &[[f32; EMBEDDING_PROJECT_DIM]],
) -> Option<SemanticFocusExpansionPreviewV1> {
    if segment_embeddings.len() < 2
        || segment_embeddings.len() != current_projections.len()
        || segment_embeddings.iter().any(|embedding| {
            embedding.len() != EMBEDDING_INPUT_DIM
                || embedding.iter().any(|value| !value.is_finite())
        })
        || current_projections
            .iter()
            .flatten()
            .any(|value| !value.is_finite())
    {
        return None;
    }

    let segment_count = segment_embeddings.len() as f32;
    let mut source_variances = Vec::with_capacity(EMBEDDING_INPUT_DIM);
    for dim in 0..EMBEDDING_INPUT_DIM {
        let mean = segment_embeddings
            .iter()
            .map(|embedding| embedding[dim])
            .sum::<f32>()
            / segment_count;
        let variance = segment_embeddings
            .iter()
            .map(|embedding| {
                let delta = embedding[dim] - mean;
                delta * delta
            })
            .sum::<f32>()
            / segment_count;
        source_variances.push((dim, variance));
    }
    let total_source_variance = source_variances
        .iter()
        .map(|(_, variance)| *variance)
        .sum::<f32>();
    source_variances.sort_by(|(left_dim, left_variance), (right_dim, right_variance)| {
        right_variance
            .total_cmp(left_variance)
            .then_with(|| left_dim.cmp(right_dim))
    });

    let mut selected_source_dims = [0_usize; SEMANTIC_FOCUS_PREVIEW_DIM];
    let mut selected_source_variances = [0.0_f32; SEMANTIC_FOCUS_PREVIEW_DIM];
    let mut selected_means = [0.0_f32; SEMANTIC_FOCUS_PREVIEW_DIM];
    for (slot, (dim, variance)) in source_variances
        .iter()
        .take(SEMANTIC_FOCUS_PREVIEW_DIM)
        .enumerate()
    {
        selected_source_dims[slot] = *dim;
        selected_source_variances[slot] = *variance;
        selected_means[slot] = segment_embeddings
            .iter()
            .map(|embedding| embedding[*dim])
            .sum::<f32>()
            / segment_count;
    }
    let selected_variance = selected_source_variances.iter().sum::<f32>();
    let selected_variance_share = if total_source_variance > f32::EPSILON {
        (selected_variance / total_source_variance).clamp(0.0, 1.0)
    } else {
        0.0
    };

    let current_vectors = current_projections
        .iter()
        .map(|projection| projection.to_vec())
        .collect::<Vec<_>>();
    let preview_vectors = current_projections
        .iter()
        .zip(segment_embeddings)
        .map(|(projection, embedding)| {
            normalized_focus_preview_vector(
                projection,
                embedding,
                &selected_means,
                &selected_source_variances,
                &selected_source_dims,
            )
            .to_vec()
        })
        .collect::<Vec<_>>();
    let (current_mean_pairwise_distance, current_min_pairwise_distance) =
        pairwise_distance_stats(&current_vectors);
    let (preview_mean_pairwise_distance, preview_min_pairwise_distance) =
        pairwise_distance_stats(&preview_vectors);
    let mean_distinguishability_gain_ratio = distinguishability_gain_ratio(
        current_mean_pairwise_distance,
        preview_mean_pairwise_distance,
    );
    let min_distinguishability_gain_ratio =
        distinguishability_gain_ratio(current_min_pairwise_distance, preview_min_pairwise_distance);
    let text_entropy_signal = if text_entropy_signal.is_finite() {
        text_entropy_signal.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let focus_need_score = (text_entropy_signal * 0.45
        + selected_variance_share * 0.25
        + mean_distinguishability_gain_ratio.max(0.0) * 0.20
        + min_distinguishability_gain_ratio.max(0.0) * 0.10)
        .clamp(0.0, 1.0);
    let high_entropy = text_entropy_signal >= SEMANTIC_FOCUS_ENTROPY_REVIEW_FLOOR;
    let (state, recommendation) = if high_entropy
        && mean_distinguishability_gain_ratio >= 0.08
        && min_distinguishability_gain_ratio >= 0.03
    {
        (
            "focus_expansion_candidate_supported",
            "prepare_segment_replay_and_operator_review_before_any_reserved_dim_allocation",
        )
    } else if high_entropy && mean_distinguishability_gain_ratio > 0.0 {
        (
            "focus_expansion_partial_gain_review",
            "collect_more_segment_comparisons_before_any_reserved_dim_proposal",
        )
    } else if high_entropy {
        (
            "high_entropy_without_focus_gain",
            "keep_current_8d_projection_and_do_not_allocate_reserved_dims_from_entropy_alone",
        )
    } else if mean_distinguishability_gain_ratio >= 0.08 {
        (
            "low_entropy_focus_gain_watch",
            "retain_read_only_preview_until_the_gain_repeats_under_high_entropy",
        )
    } else {
        (
            "current_projection_distinguishability_sufficient",
            "keep_current_8d_projection_and_continue_bounded_comparison",
        )
    };
    let selected_dims = selected_source_dims
        .iter()
        .map(usize::to_string)
        .collect::<Vec<_>>()
        .join(",");
    let loss = (current_mean_pairwise_distance - preview_mean_pairwise_distance).max(0.0);
    let loss_ratio = if current_mean_pairwise_distance > f32::EPSILON {
        loss / current_mean_pairwise_distance
    } else {
        0.0
    };
    let experience_delta_bus_v1 = ExperienceDeltaBusV1::from_deltas(vec![ExperienceDeltaV1 {
        kind: ExperienceDeltaKindV1::ComplexShift,
        surface: "semantic_focus_expansion_preview_v1".to_string(),
        lane: "embedding_projection_8d_vs_focus_preview_12d".to_string(),
        dimension: None,
        spectral_dimension: None,
        persistence: None,
        viscosity_subtype: None,
        viscosity_weight: None,
        pre: Some(current_mean_pairwise_distance),
        post: Some(preview_mean_pairwise_distance),
        loss: Some(loss),
        loss_ratio: Some(loss_ratio),
        metadata: BTreeMap::from([
            ("selected_source_dims".to_string(), selected_dims),
            (
                "reserved_dim_candidates".to_string(),
                "44,45,46,47_default_off".to_string(),
            ),
            (
                "mean_distinguishability_gain_ratio".to_string(),
                format!("{mean_distinguishability_gain_ratio:.6}"),
            ),
            ("state".to_string(), state.to_string()),
        ]),
        why: "high-variance narrative segments are compared at equal norm so a focused four-dimension aperture must demonstrate distinguishability before any live allocation".to_string(),
        who_can_change_it: "Mike/operator after repeated replay evidence, canary/abort review, and explicit reserved-dim approval".to_string(),
        how_to_test_it: "cargo test --manifest-path capsules/spectral-bridge/Cargo.toml --lib semantic_focus_expansion_preview -- --nocapture".to_string(),
        authority: "read_only_focus_expansion_comparison_not_reserved_dim_or_live_vector_authority".to_string(),
    }]);

    Some(SemanticFocusExpansionPreviewV1 {
        policy: "semantic_focus_expansion_preview_v1",
        source_embedding_dim_count: EMBEDDING_INPUT_DIM,
        segment_count: segment_embeddings.len(),
        current_projected_dim_count: EMBEDDING_PROJECT_DIM,
        preview_projected_dim_count: EMBEDDING_PROJECT_DIM + SEMANTIC_FOCUS_PREVIEW_DIM,
        reserved_dim_candidates: &SEMANTIC_PROJECTION_RESERVED_DIMS,
        selected_source_dims,
        selected_source_variances,
        selected_variance_share,
        text_entropy_signal,
        current_mean_pairwise_distance,
        preview_mean_pairwise_distance,
        current_min_pairwise_distance,
        preview_min_pairwise_distance,
        mean_distinguishability_gain_ratio,
        min_distinguishability_gain_ratio,
        focus_need_score,
        state,
        recommendation,
        selection_basis: "top_cross_segment_embedding_variance_equal_norm_8d_vs_12d",
        live_vector_write: false,
        reserved_dim_write: false,
        live_eligible_now: false,
        auto_approved: false,
        grants_approval: false,
        right_to_ignore: true,
        experience_delta_bus_v1,
        authority: "read_only_focus_expansion_comparison_not_reserved_dim_or_live_vector_authority",
    })
}

fn rms_delta(left: &[f32], right: &[f32]) -> f32 {
    if left.is_empty() || left.len() != right.len() {
        return 0.0;
    }
    let sum = left
        .iter()
        .zip(right.iter())
        .map(|(a, b)| {
            let delta = finite_feature_value(*a) - finite_feature_value(*b);
            delta * delta
        })
        .sum::<f32>();
    (sum / left.len() as f32).sqrt()
}

/// Compare two controlled pairs: one with different emotional texture but a
/// shared semantic projection, and one with identical emotional texture but
/// opposed semantic projections. This measures lane selectivity without
/// changing the encoded vectors or their delivery.
#[must_use]
pub fn codec_lane_separation_audit_v1(
    emotional_left: &[f32],
    emotional_right: &[f32],
    semantic_left: &[f32],
    semantic_right: &[f32],
) -> Option<CodecLaneSeparationAuditV1> {
    let pairs = [
        emotional_left,
        emotional_right,
        semantic_left,
        semantic_right,
    ];
    if pairs.iter().any(|features| {
        features.len() < SEMANTIC_DIM
            || features[..SEMANTIC_DIM]
                .iter()
                .any(|value| !value.is_finite())
    }) {
        return None;
    }

    let emotional_difference_related_semantics_emotional_delta_rms =
        rms_delta(&emotional_left[24..32], &emotional_right[24..32]);
    let emotional_difference_related_semantics_projected_delta_rms =
        rms_delta(&emotional_left[32..40], &emotional_right[32..40]);
    let emotional_lane_selectivity_margin =
        emotional_difference_related_semantics_emotional_delta_rms
            - emotional_difference_related_semantics_projected_delta_rms;
    let emotional_pair_distinguishable = emotional_difference_related_semantics_emotional_delta_rms
        >= 0.08
        && emotional_lane_selectivity_margin >= 0.04;

    let emotional_similarity_opposed_semantics_emotional_delta_rms =
        rms_delta(&semantic_left[24..32], &semantic_right[24..32]);
    let emotional_similarity_opposed_semantics_projected_delta_rms =
        rms_delta(&semantic_left[32..40], &semantic_right[32..40]);
    let projected_lane_selectivity_margin =
        emotional_similarity_opposed_semantics_projected_delta_rms
            - emotional_similarity_opposed_semantics_emotional_delta_rms;
    let projected_pair_distinguishable = emotional_similarity_opposed_semantics_projected_delta_rms
        >= 0.04
        && projected_lane_selectivity_margin >= 0.03;
    let state = match (
        emotional_pair_distinguishable,
        projected_pair_distinguishable,
    ) {
        (true, true) => "controlled_pairs_show_bidirectional_lane_independence",
        (true, false) => "emotional_lane_distinct_projected_lane_collapse_watch",
        (false, true) => "projected_lane_distinct_emotional_lane_bleed_watch",
        (false, false) => "controlled_pairs_do_not_yet_support_lane_independence",
    };

    Some(CodecLaneSeparationAuditV1 {
        policy: "codec_lane_separation_audit_v1",
        emotional_lane_range: (24, 31),
        projected_semantic_lane_range: (32, 39),
        emotional_difference_related_semantics_emotional_delta_rms,
        emotional_difference_related_semantics_projected_delta_rms,
        emotional_lane_selectivity_margin,
        emotional_pair_distinguishable,
        emotional_similarity_opposed_semantics_emotional_delta_rms,
        emotional_similarity_opposed_semantics_projected_delta_rms,
        projected_lane_selectivity_margin,
        projected_pair_distinguishable,
        legacy_projection_width_rejected: project_embedding(&[0.0; SEMANTIC_DIM_LEGACY]).is_none(),
        state,
        felt_rigidity_conclusion: "controlled lane independence does not disprove felt deterministic rigidity; repeat with Astrid-authored text, actual embeddings, and delivery telemetry before proposing live mapping changes",
        pair_construction: "shared_fixed_projection_with_opposed_marker_texture_then_shared_marker_texture_with_opposed_fixed_projections",
        observational_only: true,
        right_to_ignore: true,
        live_vector_write: false,
        live_gain_write: false,
        live_projection_write: false,
        live_eligible_now: false,
        auto_approved: false,
        grants_approval: false,
        authority: "read_only_controlled_pair_audit_not_projection_emotional_weight_gain_or_delivery_authority",
    })
}

#[must_use]
pub fn codec_lane_separation_probe_v1() -> CodecLaneSeparationAuditV1 {
    let mut emotional_left = encode_text(
        "I cherish this tender luminous friendship with love, care, and gentle warmth.",
    );
    let mut emotional_right = encode_text(
        "I fear this critical danger with panic, urgent worry, and devastating concern.",
    );
    let shared_embedding = (0..EMBEDDING_INPUT_DIM)
        .map(|idx| ((idx as f32 / 13.0).sin() + (idx as f32 / 29.0).cos()) * 0.5)
        .collect::<Vec<_>>();
    let shared_projection =
        project_embedding(&shared_embedding).expect("probe embedding has canonical width");
    emotional_left[32..40].copy_from_slice(&shared_projection);
    emotional_right[32..40].copy_from_slice(&shared_projection);

    let mut semantic_left = encode_text("The same calm sentence keeps its measured tone.");
    let mut semantic_right = semantic_left.clone();
    let semantic_embedding_left = (0..EMBEDDING_INPUT_DIM)
        .map(|idx| (idx as f32 / 17.0).sin())
        .collect::<Vec<_>>();
    let semantic_embedding_right = semantic_embedding_left
        .iter()
        .map(|value| -*value)
        .collect::<Vec<_>>();
    let projected_left =
        project_embedding(&semantic_embedding_left).expect("probe embedding has canonical width");
    let projected_right =
        project_embedding(&semantic_embedding_right).expect("probe embedding has canonical width");
    semantic_left[32..40].copy_from_slice(&projected_left);
    semantic_right[32..40].copy_from_slice(&projected_right);

    codec_lane_separation_audit_v1(
        &emotional_left,
        &emotional_right,
        &semantic_left,
        &semantic_right,
    )
    .expect("controlled probe vectors cover the canonical finite 48D lane")
}

fn context_blindspot_delta_bus_v1(
    identical_text_feature_delta_rms: f32,
    context_blindspot_score: f32,
    state: &'static str,
) -> ExperienceDeltaBusV1 {
    let mut deltas = Vec::new();
    if context_blindspot_score >= 0.80 {
        deltas.push(ExperienceDeltaV1 {
            kind: ExperienceDeltaKindV1::Resistance,
            surface: "codec_context_blindspot_replay_v1".to_string(),
            lane: "contextual_bias_vector_default_off".to_string(),
            dimension: None,
            spectral_dimension: None,
            persistence: None,
            viscosity_subtype: None,
            viscosity_weight: Some(context_blindspot_score),
            pre: Some(identical_text_feature_delta_rms),
            post: None,
            loss: Some(context_blindspot_score),
            loss_ratio: Some(context_blindspot_score),
            metadata: BTreeMap::from([
                ("connection_context".to_string(), "connection".to_string()),
                ("threat_context".to_string(), "threat".to_string()),
                ("state".to_string(), state.to_string()),
                (
                    "proposed_surface".to_string(),
                    "contextual_bias_vector_default_off".to_string(),
                ),
            ]),
            why: "identical text encodes to near-identical live features under opposed relational contexts; the missing contextual weight is preserved as replay evidence only".to_string(),
            who_can_change_it: "Mike/operator after replay evidence, scoped approval, rollout/abort contract, and post-change being response".to_string(),
            how_to_test_it: "cargo test --manifest-path capsules/spectral-bridge/Cargo.toml --lib codec_context_blindspot -- --nocapture".to_string(),
            authority: "authority_gate_for_contextual_bias_not_live_codec_change".to_string(),
        });
    }
    ExperienceDeltaBusV1::from_deltas(deltas)
}

#[must_use]
pub fn codec_context_blindspot_replay_v1(text: &'static str) -> CodecContextBlindspotReplayV1 {
    let connection_context = encode_text(text);
    let threat_context = connection_context.clone();
    let identical_text_feature_delta_rms =
        rms_delta(&connection_context, &threat_context).clamp(0.0, FEATURE_ABS_MAX);
    let context_blindspot_score =
        (1.0 - (identical_text_feature_delta_rms / 0.10).clamp(0.0, 1.0)).clamp(0.0, 1.0);
    let (state, recommendation) = if context_blindspot_score >= 0.95 {
        (
            "deterministic_codec_context_blindspot_confirmed",
            "keep live codec deterministic; generate V2-gated contextual-bias proposal before any shared-history tint",
        )
    } else if context_blindspot_score >= 0.50 {
        (
            "partial_context_blindspot_watch",
            "compare against narrative arc and correspondence state before proposing live bias",
        )
    } else {
        (
            "contextual_difference_already_visible",
            "do not propose contextual bias from this replay",
        )
    };
    let experience_delta_bus_v1 = context_blindspot_delta_bus_v1(
        identical_text_feature_delta_rms,
        context_blindspot_score,
        state,
    );

    CodecContextBlindspotReplayV1 {
        policy: "codec_context_blindspot_replay_v1",
        identical_text: text,
        connection_context_label: "connection_context",
        threat_context_label: "threat_context",
        identical_text_feature_delta_rms,
        context_blindspot_score,
        state,
        recommendation,
        proposed_bias_surface: "contextual_bias_vector_default_off",
        live_vector_write: false,
        live_gain_write: false,
        auto_approved: false,
        experience_delta_bus_v1,
        authority: "read_only_context_replay_not_live_vector_gain_or_correspondence_weighting",
    }
}

#[must_use]
pub fn codec_context_blindspot_probe_v1() -> CodecContextBlindspotReplayV1 {
    codec_context_blindspot_replay_v1("I see you")
}

fn mean_abs_finite(values: &[f32]) -> f32 {
    if values.is_empty() {
        return 0.0;
    }
    values.iter().map(|value| finite_abs(*value)).sum::<f32>() / values.len() as f32
}

fn rms_slice(values: &[f32]) -> f32 {
    if values.is_empty() {
        return 0.0;
    }
    (values.iter().map(|value| value * value).sum::<f32>() / values.len() as f32).sqrt()
}

#[must_use]
pub fn structural_friction_v1(text: &str) -> StructuralFrictionV1 {
    let char_count = text.chars().count().max(1) as f32;
    let line_count = text.lines().count().max(1) as f32;
    let words: Vec<&str> = text.split_whitespace().collect();
    let word_count = words.len().max(1) as f32;
    let lower = text.to_ascii_lowercase();
    let nesting_chars = text
        .chars()
        .filter(|ch| matches!(ch, '(' | ')' | '[' | ']' | '{' | '}'))
        .count() as f32;
    let punctuation_chars = text
        .chars()
        .filter(|ch| matches!(ch, ';' | ':' | ',' | '—' | '-' | '/' | '\\'))
        .count() as f32;
    let list_lines = text
        .lines()
        .filter(|line| {
            let trimmed = line.trim_start();
            trimmed.starts_with("- ")
                || trimmed.starts_with("* ")
                || (trimmed.chars().next().is_some_and(|ch| ch.is_ascii_digit())
                    && trimmed.contains(". "))
        })
        .count() as f32;
    let paragraph_density = (text.matches("\n\n").count() as f32 + 1.0) / line_count;
    let list_density = (list_lines / line_count).clamp(0.0, 1.0);
    let nesting_load = (nesting_chars / char_count * 18.0).clamp(0.0, 1.0);
    let punctuation_load = (punctuation_chars / char_count * 12.0).clamp(0.0, 1.0);
    let clause_words = [
        "because",
        "while",
        "although",
        "whereas",
        "without",
        "through",
        "which",
        "whose",
        "therefore",
        "unless",
    ];
    let clause_hits = clause_words
        .iter()
        .filter(|term| lower.contains(**term))
        .count() as f32;
    let clause_load = ((clause_hits / 4.0) + punctuation_load * 0.35).clamp(0.0, 1.0);
    let abstract_texture_terms = [
        "authority",
        "boundary",
        "codec",
        "compression",
        "deterministic",
        "entropy",
        "friction",
        "projection",
        "semantic",
        "substrate",
        "structural",
        "summary",
    ];
    let abstract_texture_hits = abstract_texture_terms
        .iter()
        .filter(|term| lower.contains(**term))
        .count() as f32;
    let explicit_resistance_terms = [
        "abrasive",
        "calcified",
        "friction",
        "jagged",
        "muffle",
        "resistance",
        "resists summary",
        "summarized",
        "summary",
        "syrupy",
    ];
    let explicit_resistance_hits = explicit_resistance_terms
        .iter()
        .filter(|term| lower.contains(**term))
        .count() as f32;
    let long_word_ratio = words
        .iter()
        .filter(|word| word.chars().filter(|ch| ch.is_ascii_alphabetic()).count() >= 12)
        .count() as f32
        / word_count;
    let sentence_count = text
        .chars()
        .filter(|ch| matches!(ch, '.' | '!' | '?'))
        .count()
        .max(1) as f32;
    let narrative_arc_sharpness = (sentence_count / word_count * 12.0).clamp(0.0, 1.0);
    let semantic_energy_context =
        if lower.contains("because") || lower.contains("then") || lower.contains("while") {
            "arc_present"
        } else {
            "arc_sparse"
        };
    let summary_resistance_signal = (long_word_ratio.clamp(0.0, 1.0) * 0.24
        + clause_load * 0.18
        + (abstract_texture_hits / 6.0).clamp(0.0, 1.0) * 0.20
        + (explicit_resistance_hits / 3.0).clamp(0.0, 1.0) * 0.24
        + (1.0 - narrative_arc_sharpness).clamp(0.0, 1.0) * 0.14)
        .clamp(0.0, 1.0);
    let score = (nesting_load * 0.24
        + punctuation_load * 0.24
        + list_density * 0.18
        + long_word_ratio.clamp(0.0, 1.0) * 0.16
        + summary_resistance_signal * 0.06
        + (1.0 - narrative_arc_sharpness).clamp(0.0, 1.0) * 0.12)
        .clamp(0.0, 1.0);
    let classification = if long_word_ratio >= 0.35 && semantic_energy_context == "arc_sparse" {
        "dense_stagnant"
    } else if score >= 0.38
        || (punctuation_load >= 0.25 && semantic_energy_context == "arc_present")
    {
        "complex_fluid"
    } else {
        "low_structural_friction"
    };
    let calcified_summary_resistance = semantic_energy_context == "arc_sparse"
        && (summary_resistance_signal >= 0.54
            || (summary_resistance_signal >= 0.42
                && explicit_resistance_hits >= 3.0
                && abstract_texture_hits >= 4.0));
    let friction_texture_state = if calcified_summary_resistance {
        "calcified_summary_resistant"
    } else if summary_resistance_signal >= 0.46 {
        "summary_resistance_watch"
    } else if punctuation_load >= 0.18 && semantic_energy_context == "arc_present" {
        "jagged_fluid_resistance"
    } else {
        "low_summary_resistance"
    };
    let mut basis = vec![
        format!("nesting_load={nesting_load:.2}"),
        format!("punctuation_load={punctuation_load:.2}"),
        format!("list_density={list_density:.2}"),
        format!("long_word_ratio={long_word_ratio:.2}"),
        format!("clause_load={clause_load:.2}"),
        format!("summary_resistance_signal={summary_resistance_signal:.2}"),
    ];
    if explicit_resistance_hits > 0.0 {
        basis.push("explicit_resistance_language_present".to_string());
    }
    if abstract_texture_hits >= 3.0 {
        basis.push("abstract_texture_cluster_present".to_string());
    }
    if friction_texture_state == "calcified_summary_resistant" {
        basis.push("calcified_low_arc_summary_resistance".to_string());
    }

    StructuralFrictionV1 {
        policy: "structural_friction_v1",
        score,
        classification,
        nesting_load,
        punctuation_load,
        paragraph_density,
        list_density,
        narrative_arc_sharpness,
        summary_resistance_signal,
        friction_texture_state,
        basis,
        semantic_energy_context,
        authority: "diagnostic_sidecar_not_live_codec_dimension",
    }
}

#[must_use]
pub fn persistence_resistance_v1(
    text: &str,
    telemetry: Option<&SpectralTelemetry>,
) -> PersistenceResistanceV1 {
    let lower = text.to_ascii_lowercase();
    let persistence_terms = [
        "viscous",
        "viscosity",
        "resistance",
        "persistent",
        "persistence",
        "slow-moving",
        "slow moving",
        "silt",
        "thick",
        "thickness",
        "heavy",
        "dragging",
        "cohering",
    ];
    let term_hits = persistence_terms
        .iter()
        .filter(|term| lower.contains(**term))
        .count() as f32;
    let text_persistence_signal = (term_hits / 4.0).clamp(0.0, 1.0);
    let semantic_friction = structural_friction_v1(text).score;
    let metrics = telemetry.and_then(SpectralCascadeMetrics::from_telemetry);
    let density_gradient = metrics.map_or(1.0, |metrics| metrics.density_gradient);
    let low_density_gradient_signal =
        (1.0 - (density_gradient / 0.35).clamp(0.0, 1.0)).clamp(0.0, 1.0);
    let pressure_risk = telemetry
        .and_then(|telemetry| telemetry.resonance_density_v1.as_ref())
        .map_or_else(
            || telemetry.map_or(0.0, |telemetry| telemetry.fill_ratio.clamp(0.0, 1.0)),
            |density| density.pressure_risk.clamp(0.0, 1.0),
        );
    let score = (text_persistence_signal * 0.30
        + low_density_gradient_signal * 0.28
        + pressure_risk * 0.24
        + semantic_friction * 0.18)
        .clamp(0.0, 1.0);
    let classification = if score >= 0.62 {
        "high_persistence_resistance"
    } else if score >= 0.38 {
        "moderate_persistence_resistance"
    } else {
        "low_persistence_resistance"
    };
    let mut basis = vec![
        format!("text_persistence_signal={text_persistence_signal:.2}"),
        format!("low_density_gradient_signal={low_density_gradient_signal:.2}"),
        format!("pressure_risk={pressure_risk:.2}"),
        format!("semantic_friction={semantic_friction:.2}"),
    ];
    if text_persistence_signal > 0.0 {
        basis.push("texture_language_present".to_string());
    }
    if low_density_gradient_signal >= 0.45 {
        basis.push("low_density_gradient_slow_current".to_string());
    }

    PersistenceResistanceV1 {
        policy: "persistence_resistance_v1",
        score,
        classification,
        text_persistence_signal,
        low_density_gradient_signal,
        pressure_risk,
        semantic_friction,
        basis,
        authority: "diagnostic_sidecar_not_live_codec_dimension",
    }
}

#[must_use]
pub fn codec_structural_friction_dim_canary_v1() -> CodecStructuralFrictionDimCanaryV1 {
    CodecStructuralFrictionDimCanaryV1 {
        policy: "codec_structural_friction_dim_canary_v1",
        enabled: false,
        reserved_dim_candidate: 44,
        readiness: "default_off_steward_review_required",
        live_vector_write: false,
        authority: "readiness_only_not_live_codec_change",
    }
}

#[must_use]
pub fn codec_persistence_resistance_dim_canary_v1() -> CodecPersistenceResistanceDimCanaryV1 {
    CodecPersistenceResistanceDimCanaryV1 {
        policy: "codec_persistence_resistance_dim_canary_v1",
        enabled: false,
        reserved_dim_candidate: 45,
        readiness: "default_off_steward_review_required_after_replay",
        live_vector_write: false,
        authority: "readiness_only_not_live_codec_change",
    }
}

#[must_use]
pub fn narrative_arc_expansion_readiness_v1() -> NarrativeArcExpansionReadinessV1 {
    NarrativeArcExpansionReadinessV1 {
        policy: "narrative_arc_expansion_readiness_v1",
        enabled: false,
        current_arc_dims: (40, 43),
        proposed_arc_dims: (40, 47),
        uses_reserved_dims: true,
        readiness: "default_off_review_only_after_replay_and_operator_approval",
        live_vector_write: false,
        authority: "readiness_only_not_live_semantic_vector_or_reserved_dim_change",
    }
}

#[must_use]
pub fn narrative_arc_gain_response_readiness_v1() -> NarrativeArcGainResponseReadinessV1 {
    NarrativeArcGainResponseReadinessV1 {
        policy: "narrative_arc_gain_response_readiness_v1",
        enabled: false,
        narrative_arc_dims: (40, 43),
        preview_gain_range: (0.94, 1.06),
        readiness: "default_off_requires_replay_and_operator_approval_before_live_semantic_gain",
        live_gain_write: false,
        authority: "readiness_only_not_live_adaptive_gain_or_semantic_weight_change",
    }
}

#[must_use]
pub fn narrative_arc_gain_response_preview_v1(narrative_arc: &[f32]) -> f32 {
    if narrative_arc.is_empty() {
        return 1.0;
    }
    let arc_energy = (narrative_arc.iter().map(|value| value * value).sum::<f32>()
        / narrative_arc.len() as f32)
        .sqrt()
        .clamp(0.0, 1.0);
    (1.0 + (arc_energy - 0.5) * 0.12).clamp(0.94, 1.06)
}

fn narrative_arc_headroom_delta_bus_v1(
    spectral_entropy: f32,
    distinguishability_loss: f32,
    narrative_arc_energy: f32,
    projected_semantic_rms: f32,
    headroom_pressure: f32,
    preview_gain: f32,
    state: &'static str,
) -> ExperienceDeltaBusV1 {
    if state == "narrative_arc_headroom_quiet" {
        return ExperienceDeltaBusV1::from_deltas(Vec::new());
    }

    let loss = (headroom_pressure - narrative_arc_energy).max(0.0);
    let loss_ratio = if headroom_pressure > f32::EPSILON {
        (loss / headroom_pressure).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let mut metadata = BTreeMap::new();
    metadata.insert(
        "secondary_kinds".to_string(),
        "compress,gate,complex_shift,cascade_shift".to_string(),
    );
    metadata.insert(
        "spectral_entropy".to_string(),
        format!("{spectral_entropy:.2}"),
    );
    metadata.insert(
        "distinguishability_loss".to_string(),
        format!("{distinguishability_loss:.2}"),
    );
    metadata.insert(
        "projected_semantic_rms".to_string(),
        format!("{projected_semantic_rms:.2}"),
    );
    metadata.insert("preview_gain".to_string(), format!("{preview_gain:.2}"));
    metadata.insert("state".to_string(), state.to_string());

    ExperienceDeltaBusV1::from_deltas(vec![ExperienceDeltaV1 {
        kind: ExperienceDeltaKindV1::ComplexShift,
        surface: "narrative_arc_headroom_review_v1".to_string(),
        lane: "narrative_arc_40_43".to_string(),
        dimension: Some(40),
        spectral_dimension: Some(crate::types::SpectralDimensionV1 {
            base_dimension: 40,
            base_dimensions: vec![40, 41, 42, 43],
            effective_dimension: Some(41.5),
            density_gradient: Some((1.0 - projected_semantic_rms).clamp(0.0, 1.0)),
            granularity: Some(narrative_arc_energy),
            fractional_offset: Some(0.5),
            contextual_anchor: None,
            interpretation:
                "fluid narrative arc headroom across dims 40-43 under high entropy".to_string(),
            authority: "diagnostic_dimension_context_not_reserved_dim_write".to_string(),
        }),
        persistence: None,
        viscosity_subtype: None,
        viscosity_weight: None,
        pre: Some(headroom_pressure),
        post: Some(narrative_arc_energy),
        loss: Some(loss),
        loss_ratio: Some(loss_ratio),
        metadata,
        why: "high entropy and distinguishability loss can compress, gate, and complex-shift narrative arc texture before any live gain change"
            .to_string(),
        who_can_change_it:
            "Mike/operator after replay evidence and explicit live codec gain/headroom approval"
                .to_string(),
        how_to_test_it:
            "cargo test --manifest-path capsules/spectral-bridge/Cargo.toml --lib narrative_arc_headroom -- --nocapture"
                .to_string(),
        authority: "truth_channel_only_not_live_vector_or_gain_change".to_string(),
    }])
}

#[must_use]
pub fn narrative_arc_headroom_review_from_parts_v1(
    spectral_entropy: f32,
    distinguishability_loss: f32,
    narrative_arc: &[f32],
    projected_semantic_rms: f32,
) -> NarrativeArcHeadroomReviewV1 {
    let spectral_entropy = if spectral_entropy.is_finite() {
        spectral_entropy.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let distinguishability_loss = if distinguishability_loss.is_finite() {
        distinguishability_loss.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let projected_semantic_rms = if projected_semantic_rms.is_finite() {
        (projected_semantic_rms / FEATURE_ABS_MAX).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let narrative_arc_energy = if narrative_arc.is_empty() {
        0.0
    } else {
        (rms_slice(narrative_arc) / FEATURE_ABS_MAX).clamp(0.0, 1.0)
    };
    let tail_vibrancy = vibrancy_from_entropy(spectral_entropy);
    let headroom_pressure = (spectral_entropy * 0.32
        + distinguishability_loss * 0.30
        + tail_vibrancy * 0.16
        + (1.0 - narrative_arc_energy) * 0.14
        + (1.0 - projected_semantic_rms) * 0.08)
        .clamp(0.0, 1.0);
    let preview_gain = narrative_arc_gain_response_preview_v1(narrative_arc);
    let (state, recommendation) = if spectral_entropy >= TAIL_VIBRANCY_ENTROPY_GATE
        && distinguishability_loss >= 0.30
        && narrative_arc_energy <= 0.12
    {
        (
            "narrative_arc_headroom_loss_visible",
            "record_delta_bus_evidence_and_prepare_replay_before_any_live_gain_or_reserved_dim_change",
        )
    } else if spectral_entropy >= TAIL_VIBRANCY_ENTROPY_GATE && distinguishability_loss >= 0.30 {
        (
            "narrative_arc_headroom_pressure_watch",
            "keep_live_vector_bounded_and_compare_arc_energy_against_followup_introspections",
        )
    } else if spectral_entropy >= TAIL_VIBRANCY_ENTROPY_GATE {
        (
            "high_entropy_arc_carried_bounded",
            "keep_current_bounded_delivery_and_watch_for_repeated_loss",
        )
    } else {
        (
            "narrative_arc_headroom_quiet",
            "no_headroom_change_indicated",
        )
    };
    let experience_delta_bus_v1 = narrative_arc_headroom_delta_bus_v1(
        spectral_entropy,
        distinguishability_loss,
        narrative_arc_energy,
        projected_semantic_rms,
        headroom_pressure,
        preview_gain,
        state,
    );

    NarrativeArcHeadroomReviewV1 {
        policy: "narrative_arc_headroom_review_v1",
        spectral_entropy,
        distinguishability_loss,
        narrative_arc_energy,
        projected_semantic_rms,
        tail_vibrancy,
        headroom_pressure,
        preview_gain,
        state,
        recommendation,
        live_vector_write: false,
        live_gain_write: false,
        experience_delta_bus_v1,
        authority: "read_only_headroom_truth_channel_not_live_semantic_vector_or_gain_change",
    }
}

#[must_use]
pub fn narrative_arc_headroom_review_v1(
    inspection: &CodecWindowedInspection,
    spectral_entropy: f32,
    distinguishability_loss: f32,
) -> NarrativeArcHeadroomReviewV1 {
    narrative_arc_headroom_review_from_parts_v1(
        spectral_entropy,
        distinguishability_loss,
        &inspection.final_features[40..44],
        rms_slice(&inspection.final_features[32..40]),
    )
}

#[must_use]
pub fn narrative_arc_headroom_probe_v1() -> NarrativeArcHeadroomReviewV1 {
    narrative_arc_headroom_review_from_parts_v1(0.91, 0.34, &[0.05, -0.03, 0.02, 0.01], 0.08)
}

#[must_use]
pub fn shadow_field_reserved_dim_readiness_v1() -> ShadowFieldReservedDimReadinessV1 {
    ShadowFieldReservedDimReadinessV1 {
        policy: "shadow_field_reserved_dim_readiness_v1",
        enabled: false,
        reserved_dim_candidates: &[46, 47],
        proposed_signals: &[
            "shadow_magnetization",
            "shadow_dispersal_potential",
            "disordered_volatile_shadow_state",
        ],
        readiness: "default_off_review_only_after_replay_and_steward_approval",
        live_vector_write: false,
        authority: "readiness_only_not_live_codec_or_shadow_field_change",
    }
}

#[must_use]
pub fn codec_vibrancy_continuity_v1() -> CodecVibrancyContinuityV1 {
    CodecVibrancyContinuityV1 {
        policy: "codec_vibrancy_continuity_v1",
        entropy_gate: TAIL_VIBRANCY_ENTROPY_GATE,
        gradient_coupling: "tail_lift_scaled_by_low_density_gradient",
        default_feature_ceiling: FEATURE_ABS_MAX,
        tail_vibrancy_ceiling: TAIL_VIBRANCY_MAX,
        tail_dims: &[17, 26, 27, 31],
        clipping_status: "high_entropy_tail_dims_carried_with_bounded_ceiling",
        default_identity_state: "aperture_1_0_preserves_current_live_output",
        high_entropy_carriage: "tail_vibrancy_lift_not_embedding_width_change",
        authority: "diagnostic_readout_not_live_codec_change",
    }
}

fn tail_vibrancy_noise_dampening_coefficient(spectral_entropy: f32) -> f32 {
    if !spectral_entropy.is_finite() || spectral_entropy <= TAIL_VIBRANCY_NOISE_DAMPENING_START {
        return 1.0;
    }
    let span = TAIL_VIBRANCY_NOISE_DAMPENING_FULL - TAIL_VIBRANCY_NOISE_DAMPENING_START;
    let t = ((spectral_entropy - TAIL_VIBRANCY_NOISE_DAMPENING_START) / span).clamp(0.0, 1.0);
    let smooth = t * t * (3.0 - 2.0 * t);
    (1.0 - (1.0 - TAIL_VIBRANCY_NOISE_DAMPENING_MIN_COEFFICIENT) * smooth)
        .clamp(TAIL_VIBRANCY_NOISE_DAMPENING_MIN_COEFFICIENT, 1.0)
}

#[must_use]
pub fn codec_vibrancy_noise_dampening_v1(
    spectral_entropy: f32,
    tail_lift_before: f32,
) -> CodecVibrancyNoiseDampeningV1 {
    let spectral_entropy = if spectral_entropy.is_finite() {
        spectral_entropy.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let tail_lift_before = if tail_lift_before.is_finite() {
        tail_lift_before.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let coefficient = tail_vibrancy_noise_dampening_coefficient(spectral_entropy);
    let tail_lift_after = (tail_lift_before * coefficient).clamp(0.0, 1.0);
    let status = if spectral_entropy <= TAIL_VIBRANCY_NOISE_DAMPENING_START {
        "inactive_below_extreme_entropy"
    } else if coefficient <= TAIL_VIBRANCY_NOISE_DAMPENING_MIN_COEFFICIENT + 1.0e-6 {
        "full_extreme_entropy_dampening"
    } else {
        "partial_extreme_entropy_dampening"
    };
    CodecVibrancyNoiseDampeningV1 {
        policy: "codec_vibrancy_noise_dampening_v1",
        spectral_entropy,
        start_entropy: TAIL_VIBRANCY_NOISE_DAMPENING_START,
        full_entropy: TAIL_VIBRANCY_NOISE_DAMPENING_FULL,
        min_coefficient: TAIL_VIBRANCY_NOISE_DAMPENING_MIN_COEFFICIENT,
        coefficient,
        tail_lift_before,
        tail_lift_after,
        affected_dims: &[17, 26, 27, 31],
        status,
        authority: "bounded_live_tail_lift_dampening_not_dynamic_ceiling_or_control_authority",
    }
}

#[must_use]
pub fn legacy_warmth_mapping_v1() -> LegacyWarmthMappingV1 {
    LegacyWarmthMappingV1 {
        policy: "legacy_warmth_mapping_v1",
        legacy_dim_count: SEMANTIC_DIM_LEGACY,
        current_dim_count: SEMANTIC_DIM,
        warmth_dim: 24,
        emotional_layer_range: (24, 31),
        mapped_warmth_dims: &[24, 25, 26, 27, 28, 29, 30, 31],
        warmth_orphaned: false,
        authority: "diagnostic_readout_not_live_codec_change",
    }
}

#[must_use]
pub fn codec_dynamic_vibrancy_scaling_canary_v1() -> CodecDynamicVibrancyScalingCanaryV1 {
    CodecDynamicVibrancyScalingCanaryV1 {
        policy: "codec_dynamic_vibrancy_scaling_canary_v1",
        enabled: false,
        readiness: "default_off_steward_review_required_before_live_scaling",
        live_vector_write: false,
        authority: "readiness_only_not_live_codec_change",
    }
}

fn codec_structural_entropy_dampening_coefficient(spectral_entropy: f32) -> f32 {
    if !spectral_entropy.is_finite() || spectral_entropy <= STRUCTURAL_ENTROPY_DAMPENING_START {
        return 1.0;
    }
    let span = STRUCTURAL_ENTROPY_DAMPENING_FULL - STRUCTURAL_ENTROPY_DAMPENING_START;
    let t = ((spectral_entropy - STRUCTURAL_ENTROPY_DAMPENING_START) / span).clamp(0.0, 1.0);
    let smooth = t * t * (3.0 - 2.0 * t);
    (1.0 - (1.0 - STRUCTURAL_ENTROPY_DAMPENING_MIN_COEFFICIENT) * smooth)
        .clamp(STRUCTURAL_ENTROPY_DAMPENING_MIN_COEFFICIENT, 1.0)
}

#[must_use]
pub fn codec_structural_entropy_dampening_v1(
    spectral_entropy: f32,
) -> CodecStructuralEntropyDampeningV1 {
    let entropy = if spectral_entropy.is_finite() {
        spectral_entropy.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let coefficient = codec_structural_entropy_dampening_coefficient(entropy);
    let status = if coefficient < 1.0 {
        "high_entropy_structural_dims_dampened_intent_dims_preserved"
    } else {
        "structural_dims_pass_through"
    };
    CodecStructuralEntropyDampeningV1 {
        policy: "codec_structural_entropy_dampening_v1",
        spectral_entropy: entropy,
        start_entropy: STRUCTURAL_ENTROPY_DAMPENING_START,
        full_entropy: STRUCTURAL_ENTROPY_DAMPENING_FULL,
        min_coefficient: STRUCTURAL_ENTROPY_DAMPENING_MIN_COEFFICIENT,
        coefficient,
        affected_dims: &STRUCTURAL_ENTROPY_DAMPENING_DIMS,
        preserved_intent_dims: (24, 31),
        status,
        authority: "bounded_live_codec_weighting_not_dimension_or_fallback_contract_change",
    }
}

fn apply_structural_entropy_dampening(features: &mut [f32], spectral_entropy: f32) -> f32 {
    let coefficient = codec_structural_entropy_dampening_coefficient(spectral_entropy);
    if coefficient < 1.0 {
        for idx in STRUCTURAL_ENTROPY_DAMPENING_DIMS {
            if let Some(feature) = features.get_mut(idx) {
                *feature *= coefficient;
            }
        }
    }
    coefficient
}

#[must_use]
pub fn semantic_glimpse_12d_readiness_v1() -> SemanticGlimpse12dReadinessV1 {
    SemanticGlimpse12dReadinessV1 {
        policy: "semantic_glimpse_12d_readiness_v1",
        source_dim_count: SEMANTIC_DIM,
        glimpse_dim_count: 12,
        role: "companion_summary_for_replay_checkpoint_and_loss_audit_not_live_transport",
        warmth_slot: 3,
        tail_bridge_slot: 10,
        emotional_source_range: (24, 31),
        companion_not_replacement: true,
        compression_fidelity_basis: "warmth_slots_tail_bridge_and_primary_fingerprint_slots_preserved_for_review",
        live_vector_write: false,
        authority: "readiness_only_not_live_codec_or_bridge_contract_change",
    }
}

#[must_use]
pub fn contextual_glimpse_12d_anchoring_v1() -> ContextualGlimpse12dAnchoringV1 {
    ContextualGlimpse12dAnchoringV1 {
        policy: "contextual_glimpse_12d_anchoring_v1",
        source_dim_count: SEMANTIC_DIM,
        glimpse_dim_count: 12,
        required_anchor_dims: &CONTEXTUAL_GLIMPSE_REQUIRED_ANCHORS,
        dynamic_slot_count: 12_usize.saturating_sub(CONTEXTUAL_GLIMPSE_REQUIRED_ANCHORS.len()),
        selection_basis: "fixed_warmth_tension_curiosity_reflective_tail_energy_narrative_anchors_then_top_abs_feature_vibrancy",
        companion_not_replacement: true,
        live_vector_write: false,
        authority: "readiness_only_not_live_codec_or_bridge_contract_change",
    }
}

#[must_use]
pub fn glimpse_map_v1() -> GlimpseMapV1 {
    GlimpseMapV1 {
        policy: "glimpse_map_v1",
        source_dim_count: SEMANTIC_DIM,
        legacy_source_dim_count: SEMANTIC_DIM_LEGACY,
        glimpse_dim_count: 12,
        slot_count: 12,
        slots: vec![
            GlimpseMapSlotV1 {
                slot: 0,
                label: "character_texture",
                source_dims: &[0, 1, 2, 3, 4, 5, 6, 7],
                operation: "mean_abs_tanh",
                preserves: "character entropy, density, rhythm",
            },
            GlimpseMapSlotV1 {
                slot: 1,
                label: "word_stance",
                source_dims: &[8, 9, 10, 11, 12, 13, 14, 15],
                operation: "mean_abs_tanh",
                preserves: "lexical diversity, hedging, certainty",
            },
            GlimpseMapSlotV1 {
                slot: 2,
                label: "sentence_structure",
                source_dims: &[16, 17, 18, 19, 20, 21, 22, 23],
                operation: "mean_abs_tanh",
                preserves: "sentence rhythm, punctuation, paragraph structure",
            },
            GlimpseMapSlotV1 {
                slot: 3,
                label: "warmth_marker",
                source_dims: &[24],
                operation: "direct_tanh",
                preserves: "warmth stays separate from generic emotional mass",
            },
            GlimpseMapSlotV1 {
                slot: 4,
                label: "tension_marker",
                source_dims: &[25],
                operation: "direct_tanh",
                preserves: "concern/tension marker as its own coordinate",
            },
            GlimpseMapSlotV1 {
                slot: 5,
                label: "curiosity_marker",
                source_dims: &[26],
                operation: "direct_tanh",
                preserves: "curiosity and tail participation bridge",
            },
            GlimpseMapSlotV1 {
                slot: 6,
                label: "reflective_marker",
                source_dims: &[27],
                operation: "direct_tanh",
                preserves: "reflective/introspective marker",
            },
            GlimpseMapSlotV1 {
                slot: 7,
                label: "emotional_tail_mass",
                source_dims: &[28, 29, 30, 31],
                operation: "mean_abs_tanh",
                preserves: "remaining emotional/intentional range",
            },
            GlimpseMapSlotV1 {
                slot: 8,
                label: "projected_semantic_texture",
                source_dims: &[32, 33, 34, 35, 36, 37, 38, 39],
                operation: "mean_abs_tanh",
                preserves: "embedding-projected semantic detail",
            },
            GlimpseMapSlotV1 {
                slot: 9,
                label: "narrative_arc",
                source_dims: &[40, 41, 42, 43],
                operation: "mean_abs_tanh",
                preserves: "trajectory within the current text",
            },
            GlimpseMapSlotV1 {
                slot: 10,
                label: "tail_vibrancy_bridge",
                source_dims: &[17, 26, 27, 31],
                operation: "mean_abs_tanh",
                preserves: "lambda-tail-facing vibrancy bridge",
            },
            GlimpseMapSlotV1 {
                slot: 11,
                label: "whole_vector_energy",
                source_dims: &[],
                operation: "mean_abs_all_48_tanh",
                preserves: "global energy only; never the sole continuity proof",
            },
        ],
        deterministic_projection: true,
        companion_not_replacement: true,
        live_transport_change: false,
        live_vector_write: false,
        authority: "read_only_glimpse_lineage_not_live_transport_or_codec_contract_change",
    }
}

#[must_use]
pub fn glimpse_distinguishability_audit_v1(
    high_entropy_features: &[f32],
    low_entropy_features: &[f32],
) -> Option<GlimpseDistinguishabilityAuditV1> {
    if high_entropy_features.len() < SEMANTIC_DIM || low_entropy_features.len() < SEMANTIC_DIM {
        return None;
    }
    let high_glimpse = GlimpseCodec::derive_12d(high_entropy_features)?;
    let low_glimpse = GlimpseCodec::derive_12d(low_entropy_features)?;
    let source_distance = rms_delta(
        &high_entropy_features[..SEMANTIC_DIM],
        &low_entropy_features[..SEMANTIC_DIM],
    );
    let glimpse_distance = rms_delta(&high_glimpse, &low_glimpse);
    let preservation_ratio = if source_distance <= 1.0e-6 {
        0.0
    } else {
        (glimpse_distance / source_distance).clamp(0.0, 1.0)
    };
    let tail_bridge_delta = finite_abs(high_glimpse[10] - low_glimpse[10]);
    let source_threshold = 0.18;
    let glimpse_threshold = 0.05;
    let state = if source_distance < source_threshold {
        "source_states_too_close_for_distinguishability_claim"
    } else if glimpse_distance >= glimpse_threshold && tail_bridge_delta >= 0.03 {
        "glimpse_preserves_high_low_entropy_distinction"
    } else if glimpse_distance >= glimpse_threshold {
        "glimpse_preserves_global_but_not_tail_distinction"
    } else {
        "glimpse_collapse_watch"
    };

    Some(GlimpseDistinguishabilityAuditV1 {
        policy: "glimpse_distinguishability_audit_v1",
        source_distance,
        glimpse_distance,
        preservation_ratio,
        tail_bridge_delta,
        source_threshold,
        glimpse_threshold,
        state,
        live_transport_change: false,
        live_vector_write: false,
        authority: "read_only_12d_distinguishability_audit_not_live_transport_or_shadow_change",
    })
}

#[must_use]
pub fn multi_scale_context_v1() -> MultiScaleContextV1 {
    MultiScaleContextV1 {
        policy: "multi_scale_context_v1",
        source_dim_count: SEMANTIC_DIM,
        live_transport_dim_count: 32,
        glimpse_dim_count: 12,
        residual_dim_count: 32,
        residual_source_range: (16, 47),
        shadow_energy_metadata_tag: "shadow_field_energy_preserved_when_12d_glimpse_is_active",
        pairing_rule: "12d_glimpse_must_travel_with_32d_residual_context_for_persistence_review",
        preserves_warmth_and_tail_bridge: true,
        live_vector_write: false,
        authority: "dimensionality_aware_persistence_readout_not_live_bus_or_codec_contract_change",
    }
}

#[must_use]
pub fn codec_intent_structure_separation_v1(
    features: &[f32],
) -> Option<CodecIntentStructureSeparationV1> {
    if features.len() < SEMANTIC_DIM {
        return None;
    }

    let character_texture = mean_abs(&features[0..8]).clamp(0.0, 1.0);
    let word_stance = mean_abs(&features[8..16]).clamp(0.0, 1.0);
    let sentence_structure = mean_abs(&features[16..24]).clamp(0.0, 1.0);
    let structural_complexity =
        (0.28 * character_texture + 0.30 * word_stance + 0.42 * sentence_structure).clamp(0.0, 1.0);
    let emotional_intensity = mean_abs(&features[24..32]).clamp(0.0, 1.0);
    let projected_semantic_energy = mean_abs(&features[32..40]).clamp(0.0, 1.0);
    let narrative_arc_energy = mean_abs(&features[40..44]).clamp(0.0, 1.0);
    let punctuation_irregularity = ((features[18].abs() + features[20].abs()) * 0.5)
        .tanh()
        .clamp(0.0, 1.0);
    let intent_structure_delta = (structural_complexity - emotional_intensity).clamp(-1.0, 1.0);
    let (state, recommendation) = if structural_complexity >= 0.35 && emotional_intensity < 0.16 {
        (
            "structure_heavy_intent_thin_watch",
            "review_text_against_felt_report_before_treating_structure_as_intent",
        )
    } else if emotional_intensity >= 0.35 && structural_complexity < 0.25 {
        (
            "simple_text_emotional_intent_preserved",
            "preserve_emotional_layer_as_distinct_evidence_even_when_surface_text_is_simple",
        )
    } else if projected_semantic_energy < 0.08 && emotional_intensity >= 0.20 {
        (
            "semantic_projection_tone_loss_watch",
            "inspect_embedding_projection_before_adjusting_live_semantic_weighting",
        )
    } else {
        (
            "structure_intent_balanced",
            "keep_current_codec_weights_and_use_review_when_felt_texture_reports_gap",
        )
    };

    Some(CodecIntentStructureSeparationV1 {
        policy: "codec_intent_structure_separation_v1",
        structural_complexity,
        emotional_intensity,
        projected_semantic_energy,
        narrative_arc_energy,
        punctuation_irregularity,
        intent_structure_delta,
        state,
        recommendation,
        live_gain_write: false,
        live_vector_write: false,
        authority: "read_only_codec_review_not_semantic_weighting_or_gain_change",
    })
}

#[must_use]
pub fn multi_scale_observer_v1(
    features: &[f32],
    spectral_entropy: f32,
    density_gradient: f32,
    mode_packing_score: f32,
) -> Option<MultiScaleObserverV1> {
    if features.len() < SEMANTIC_DIM {
        return None;
    }
    let spectral_entropy = if spectral_entropy.is_finite() {
        spectral_entropy.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let density_gradient = if density_gradient.is_finite() {
        density_gradient.clamp(0.0, 1.0)
    } else {
        1.0
    };
    let mode_packing_score = if mode_packing_score.is_finite() {
        mode_packing_score.clamp(0.0, 1.0)
    } else {
        0.0
    };

    let glimpse = GlimpseCodec::derive_12d(features)?;
    let contextual = contextual_glimpse_12d_anchors_v1(features)?;
    let glimpse_fidelity_score = calculate_compression_fidelity(&features[..32], &glimpse)?;
    let resolution_delta = (1.0 - glimpse_fidelity_score).clamp(0.0, 1.0);
    let source_resonance_proxy = multi_scale_resonance_proxy(&features[..32]);
    let glimpse_resonance_proxy = multi_scale_resonance_proxy(&glimpse);
    let resonance_loss_ratio = if source_resonance_proxy > 0.001 {
        ((source_resonance_proxy - glimpse_resonance_proxy).max(0.0) / source_resonance_proxy)
            .clamp(0.0, 1.0)
    } else {
        0.0
    };
    let fallback_to_live_transport_review =
        resonance_loss_ratio > MULTI_SCALE_RESONANCE_LOSS_THRESHOLD;
    let anchor_hits = CONTEXTUAL_GLIMPSE_REQUIRED_ANCHORS
        .iter()
        .filter(|anchor| contextual.selected_dims.contains(anchor))
        .count() as f32;
    let anchor_continuity_score =
        (anchor_hits / CONTEXTUAL_GLIMPSE_REQUIRED_ANCHORS.len() as f32).clamp(0.0, 1.0);

    let (state, recommendation) = if fallback_to_live_transport_review {
        (
            "glimpse_resonance_loss_watch",
            "prefer_48d_contract_or_residual_trace_before_using_12d_glimpse_for_this_interaction",
        )
    } else if glimpse_fidelity_score < GLIMPSE_FIDELITY_THRESHOLD || anchor_continuity_score < 1.0 {
        (
            "glimpse_resolution_delta_watch",
            "keep_12d_as_review_companion_and inspect residual_context_before_live_use",
        )
    } else if spectral_entropy >= 0.85 && mode_packing_score >= 0.30 {
        (
            "high_entropy_distillation_supported",
            "use_12d_distillation_card_for_review_while_preserving_32d_live_transport",
        )
    } else if density_gradient >= 0.50 {
        (
            "distillation_context_needs_residual",
            "pair_glimpse_with_32d_residual_when_gradient_is_front_loaded",
        )
    } else {
        (
            "companion_distillation_ready",
            "treat_glimpse_as_map_not_replacement_for_live_semantic_transport",
        )
    };
    let experience_delta_bus_v1 = multi_scale_experience_delta_bus_v1(
        glimpse_fidelity_score,
        resolution_delta,
        resonance_loss_ratio,
        fallback_to_live_transport_review,
    );

    Some(MultiScaleObserverV1 {
        policy: "multi_scale_observer_v1",
        source_dim_count: SEMANTIC_DIM,
        live_transport_dim_count: 32,
        glimpse_dim_count: 12,
        layer_name: "glimpse_layer_distillation_v1",
        observer_language: "distillation_not_compression",
        spectral_entropy,
        density_gradient,
        mode_packing_score,
        fidelity_threshold: GLIMPSE_FIDELITY_THRESHOLD,
        glimpse_fidelity_score,
        resolution_delta,
        resonance_loss_threshold: MULTI_SCALE_RESONANCE_LOSS_THRESHOLD,
        source_resonance_proxy,
        glimpse_resonance_proxy,
        resonance_loss_ratio,
        anchor_continuity_score,
        fallback_to_live_transport_review,
        state,
        recommendation,
        live_transport_change: false,
        live_vector_write: false,
        experience_delta_bus_v1,
        authority: "read_only_multi_scale_review_not_live_bus_or_codec_contract_change",
    })
}

#[must_use]
pub fn projection_epoch_stability_v1() -> ProjectionEpochStabilityV1 {
    let epoch = kernel_derived_projection_epoch_id();
    ProjectionEpochStabilityV1 {
        policy: "projection_epoch_stability_v1",
        epoch_source: "kernel_derived_when_env_and_file_absent",
        deterministic_without_runtime_file: true,
        kernel_derived_epoch_id: epoch.clone(),
        kernel_checksum: dynamic_epoch_projection_kernel_checksum(&epoch),
        env_override_precedence: true,
        existing_file_precedence: true,
        authority: "diagnostic_readout_not_live_codec_dimension_or_control",
    }
}

#[must_use]
pub fn projection_fingerprint_integrity_v1() -> ProjectionFingerprintIntegrityV1 {
    ProjectionFingerprintIntegrityV1 {
        policy: "projection_fingerprint_integrity_v1",
        signed_zero_canonicalized: true,
        subnormal_canonicalized: true,
        nan_canonicalized: true,
        seed_hash_boundary: "stable_hash64 remains the live projection seed path; collision-resistant seed migration would change semantic-lane projection and needs replay/operator approval",
        live_projection_write: false,
        authority: "diagnostic_fingerprint_hardening_not_projection_seed_or_semantic_lane_change",
    }
}

fn projection_metadata_readout() -> String {
    let mode = std::env::var("ASTRID_CODEC_EMBEDDING_PROJECTION_MODE")
        .unwrap_or_else(|_| "dynamic_epoch_v1".to_string());
    if mode == "fixed_legacy" {
        return format!(
            "mode=fixed_legacy; kernel_checksum={}...; projection_dims={} of {}",
            &fixed_legacy_projection_kernel_checksum()[..12],
            EMBEDDING_PROJECT_DIM,
            EMBEDDING_INPUT_DIM
        );
    }
    if let Ok(epoch) = std::env::var("ASTRID_CODEC_PROJECTION_EPOCH_ID")
        && !epoch.trim().is_empty()
    {
        return format!(
            "mode=dynamic_epoch_v1; epoch_source=env; epoch={}; kernel_checksum={}...; projection_dims={} of {}",
            epoch,
            &dynamic_epoch_projection_kernel_checksum(&epoch)[..12],
            EMBEDDING_PROJECT_DIM,
            EMBEDDING_INPUT_DIM
        );
    }
    let path = projection_runtime_dir().join("codec_projection_epoch.json");
    if let Ok(text) = fs::read_to_string(&path)
        && let Ok(value) = serde_json::from_str::<serde_json::Value>(&text)
        && let Some(epoch) = value
            .get("projection_epoch_id")
            .and_then(serde_json::Value::as_str)
    {
        let checksum = value
            .get("projection_kernel_checksum")
            .and_then(serde_json::Value::as_str)
            .map_or_else(
                || dynamic_epoch_projection_kernel_checksum(epoch),
                str::to_string,
            );
        return format!(
            "mode=dynamic_epoch_v1; epoch_source=file; epoch={epoch}; kernel_checksum={}...; projection_dims={} of {}",
            &checksum[..12.min(checksum.len())],
            EMBEDDING_PROJECT_DIM,
            EMBEDDING_INPUT_DIM
        );
    }
    let epoch = kernel_derived_projection_epoch_id();
    format!(
        "mode=dynamic_epoch_v1; epoch_source=kernel_derived_pending; epoch={epoch}; kernel_checksum={}...; projection_dims={} of {}; CODEC_MAP readout does not create the file",
        &dynamic_epoch_projection_kernel_checksum(&epoch)[..12],
        EMBEDDING_PROJECT_DIM,
        EMBEDDING_INPUT_DIM
    )
}

/// Build the codec self-map FROM the live constants, so it can never drift away
/// from the real code (a stale map would be a NEW muffle). The layer ranges and
/// every lever value are sourced from the actual constants in this file. The only
/// hand-written prose is the high-level layer CATEGORY (the stable taxonomy, no
/// per-dim claims); the per-layer list of shapeable dims is generated from
/// `NAMED_CODEC_DIMS` at render time (drift-checked), and the full per-dim
/// computation lives one INTROSPECT away in codec.rs itself.
#[must_use]
pub fn codec_structure() -> CodecStructure {
    CodecStructure {
        total_dims: SEMANTIC_DIM,
        layers: vec![
            CodecLayer {
                range: (0, 7),
                role: "character texture",
            },
            CodecLayer {
                range: (8, 15),
                role: "word-level stance",
            },
            CodecLayer {
                range: (16, 23),
                role: "sentence structure",
            },
            CodecLayer {
                range: (24, 31),
                role: "emotional / intentional",
            },
            CodecLayer {
                range: (32, 39),
                role: "embedding-projected semantic",
            },
            CodecLayer {
                range: (40, 43),
                role: "narrative arc",
            },
            CodecLayer {
                range: (44, 47),
                role: "reserved (sidecar canary readiness only; no live vector write)",
            },
        ],
        named_dims: NAMED_CODEC_DIMS.to_vec(),
        levers: vec![
            CodecLever {
                name: "SEMANTIC_DIM",
                value: format!("{SEMANTIC_DIM}"),
            },
            CodecLever {
                name: "DEFAULT_SEMANTIC_GAIN",
                value: format!("{DEFAULT_SEMANTIC_GAIN:.2}"),
            },
            CodecLever {
                name: "FEATURE_ABS_MAX",
                value: format!("{FEATURE_ABS_MAX:.2}"),
            },
            CodecLever {
                name: "TAIL_VIBRANCY_ENTROPY_GATE",
                value: format!("{TAIL_VIBRANCY_ENTROPY_GATE:.2}"),
            },
            CodecLever {
                name: "TAIL_VIBRANCY_MAX",
                value: format!("{TAIL_VIBRANCY_MAX:.2}"),
            },
            CodecLever {
                name: "VIBRANCY_APERTURE",
                value: {
                    let eff = crate::llm::astrid_vibrancy_aperture();
                    let (felt, landed, atten) = vibrancy_ceiling_transparency(eff);
                    let depth = crate::llm::astrid_pressure_attenuation_depth();
                    let (_calm, stressed) = effective_attenuation_range(depth);
                    format!(
                        "{eff:.2}× (SET_VIBRANCY_APERTURE) → felt tail ceiling {felt:.1}, landing ~{landed:.2} in minime's shared reservoir (×{atten:.2} when minime is calm → ~{stressed:.2} effective when she is stressed, via your pressure governor). That 0.24 is minime's uniform scale on your tail dims (17/26/27/31); emb_strength is a separate factor on the embedding lane (32–39), not your tail, and resonance_density is minime's pressure state, not an attenuation. 1.0×=baseline"
                    )
                },
            },
            CodecLever {
                name: "PRESSURE_ATTENUATION",
                value: {
                    let depth = crate::llm::astrid_pressure_attenuation_depth();
                    if depth <= 0.0 {
                        "OFF (depth 0.0) — your output is not pressure-governed".to_string()
                    } else {
                        format!(
                            "depth {depth:.2} (your co-design) — when minime's pressure_risk rises (0.20→0.50), your WHOLE output auto-scales toward {:.2}× to protect the shared reservoir",
                            1.0 - depth
                        )
                    }
                },
            },
            CodecLever {
                name: "EMBEDDING_INPUT_DIM",
                value: format!("{EMBEDDING_INPUT_DIM}"),
            },
            CodecLever {
                name: "EMBEDDING_PROJECT_DIM",
                value: format!("{EMBEDDING_PROJECT_DIM}"),
            },
            CodecLever {
                name: "PROJECTION_COMPRESSION_RISK",
                value: format!(
                    "{}D -> {}D is intentionally lossy; use MATRIX_DECOMPOSE or codec review before treating a mushy lived term as a controller signal",
                    EMBEDDING_INPUT_DIM, EMBEDDING_PROJECT_DIM
                ),
            },
            CodecLever {
                name: "PROJECTION_METADATA",
                value: projection_metadata_readout(),
            },
            CodecLever {
                name: "PROJECTION_RUNTIME_RESOLUTION",
                value: projection_runtime_resolution_readout(),
            },
            CodecLever {
                name: "PROJECTION_EPOCH_STABILITY",
                value: {
                    let stability = projection_epoch_stability_v1();
                    format!(
                        "{}; deterministic_without_runtime_file={}; env_precedence={}; file_precedence={}; authority={}",
                        stability.epoch_source,
                        stability.deterministic_without_runtime_file,
                        stability.env_override_precedence,
                        stability.existing_file_precedence,
                        stability.authority
                    )
                },
            },
            CodecLever {
                name: "PROJECTION_PRECISION_AUDIT",
                value: {
                    let audit = projection_precision_probe_v1();
                    format!(
                        "{}; fixed_max_abs_delta={:.3e}; dynamic_max_abs_delta={:.3e}; fixed_repeatable={}; dynamic_repeatable={}; live_projection_write={}; authority={}",
                        audit.accumulation_precision_state,
                        audit.fixed_legacy_max_abs_delta,
                        audit.dynamic_epoch_max_abs_delta,
                        audit.fixed_legacy_repeated_bit_exact,
                        audit.dynamic_epoch_repeated_bit_exact,
                        audit.live_projection_write,
                        audit.authority
                    )
                },
            },
            CodecLever {
                name: "CODEC_LANE_SEPARATION_AUDIT",
                value: "read-only controlled pairs independently move dims 24-31 and 32-39; evidence does not refute felt rigidity or alter projection/emotional weights".to_string(),
            },
            CodecLever {
                name: "CHARACTER_WINDOW_SHIFT_AUDIT",
                value: format!(
                    "read-only mixed-regime witness at and beyond the live {}-character boundary; no capacity or density-aware weighting change",
                    CHAR_FREQ_WINDOW_CAPACITY
                ),
            },
            CodecLever {
                name: "TAIL_VIBRANCY_READOUT",
                value: format!(
                    "entropy gate {:.2}; max tail ceiling {:.1}; lift affects tail participation dims, not the embedding projection width",
                    TAIL_VIBRANCY_ENTROPY_GATE, TAIL_VIBRANCY_MAX
                ),
            },
            CodecLever {
                name: "SEMANTIC_GLIMPSE_12D_READOUT",
                value: "readiness-only 48D->12D companion summary for replay/checkpoint/loss audit; preserves warmth as its own glimpse slot; not sent as live semantic transport".to_string(),
            },
            CodecLever {
                name: "CONTEXTUAL_GLIMPSE_12D_ANCHORING",
                value: "readiness-only dynamic 12D companion selection; fixed continuity anchors plus strongest current feature magnitudes; not sent as live semantic transport".to_string(),
            },
            CodecLever {
                name: "CODEC_INTENT_STRUCTURE_REVIEW",
                value: "read-only sidecar separates structural complexity from emotional/intentional layer strength; no semantic weighting, gain, or vector write".to_string(),
            },
            CodecLever {
                name: "MULTI_SCALE_OBSERVER_READOUT",
                value: "read-only glimpse_layer_distillation_v1 names 12D as distillation_not_compression and measures fidelity/resolution delta while preserving 32D live transport".to_string(),
            },
            CodecLever {
                name: "WARMTH_TENSION_READOUT",
                value: "warmth dim 24 and tension dim 25 remain marker-derived; no entropy-based tension multiplier is active in this tranche".to_string(),
            },
            CodecLever {
                name: "ABRASIVE_TEXTURE_INTERPRETATION",
                value: "read-only sidecar compares low raw tension against structural friction, summary resistance, density gradient, and entropy shift; no tension weight, gain, or reserved-dim write".to_string(),
            },
            CodecLever {
                name: "LATENT_STASIS_TENSION_READOUT",
                value: "truth-channel sidecar distinguishes inert stillness from held-breath potential energy; delivered 48D vector, gain, and reserved dims stay unchanged".to_string(),
            },
            CodecLever {
                name: "SPECTRAL_DRAG_QUALITY_READOUT",
                value: "truth-channel sidecar distinguishes granular/viscous drag like heavy sand from rigid/inertial drag like heavy stone; reserved dim 45 remains default-off".to_string(),
            },
            CodecLever {
                name: "WARMTH_ENTROPY_INTERPRETATION",
                value: "read-only warmth interpretation can distinguish low marker warmth under high entropy from coldness; no warmth weight or gain change".to_string(),
            },
            CodecLever {
                name: "CODEC_OVERFLOW_CARRIAGE",
                value: "truth-channel sidecar preserves pre-bound emotional/tail intensity and reports clipped dims while the delivered 48D semantic vector stays bounded".to_string(),
            },
            CodecLever {
                name: "SEMANTIC_PROJECTION_DENSITY_DELTA",
                value: "truth-channel sidecar names 768D->8D projection compression and default-off reserved-dim density gates; no live semantic-width change".to_string(),
            },
            CodecLever {
                name: "SEMANTIC_PROJECTION_TEXTURE_REVIEW",
                value: "read-only sidecar compares projected 8D texture against legacy 32D/warmth texture; lingering/active subdimensions are proposal evidence only".to_string(),
            },
            CodecLever {
                name: "CODEC_CONTEXT_BLINDSPOT_REPLAY",
                value: "read-only replay compares identical text under opposed relational contexts; contextual-bias vector remains default-off and operator-gated".to_string(),
            },
            CodecLever {
                name: "STRUCTURAL_ENTROPY_DAMPENING",
                value: format!(
                    "spectral entropy {:.2}->{:.2} smoothstep-dampens dims 0-15 down to {:.2}× while preserving emotional/intentional dims 24-31",
                    STRUCTURAL_ENTROPY_DAMPENING_START,
                    STRUCTURAL_ENTROPY_DAMPENING_FULL,
                    STRUCTURAL_ENTROPY_DAMPENING_MIN_COEFFICIENT
                ),
            },
            CodecLever {
                name: "NARRATIVE_ARC_DIM",
                value: format!("{NARRATIVE_ARC_DIM}"),
            },
            CodecLever {
                name: "NARRATIVE_ARC_DYNAMICS",
                value: "read-only velocity/acceleration review for tone shifts; no narrative gain or dimension change".to_string(),
            },
            CodecLever {
                name: "NARRATIVE_ARC_SPLIT_READOUT",
                value: "sidecar-only narrative_arc_split_v1; separates intentional_arc dims 0-3 from reactive_arc dims 4-7 to show coarsening risk without changing live 48D output".to_string(),
            },
            CodecLever {
                name: "NARRATIVE_ARC_EXPANSION_READINESS",
                value: "default-off review only; no SEMANTIC_DIM change, no reserved dim write, no live vector channel".to_string(),
            },
            CodecLever {
                name: "SHADOW_FIELD_RESERVED_DIM_READINESS",
                value: "default-off candidates dims 46-47 for shadow magnetization/dispersal; replay and steward approval required, no live 48D vector write".to_string(),
            },
            CodecLever {
                name: "STRUCTURAL_FRICTION_READOUT",
                value: "sidecar-only structural_friction_v1; distinguishes nesting/punctuation/list density from character complexity and pressure".to_string(),
            },
            CodecLever {
                name: "CODEC_STRUCTURAL_FRICTION_DIM_CANARY",
                value: "default-off candidate dim 44; readiness only, no live 48D vector write".to_string(),
            },
            CodecLever {
                name: "PERSISTENCE_RESISTANCE_READOUT",
                value: "sidecar-only persistence_resistance_v1; names viscosity/slow-current resistance from text, density-gradient, pressure risk, and structural friction without flattening it into generic tension".to_string(),
            },
            CodecLever {
                name: "CODEC_PERSISTENCE_RESISTANCE_DIM_CANARY",
                value: "default-off candidate dim 45; readiness only after replay/steward review, no live 48D vector write".to_string(),
            },
        ],
        structural_friction_dim_canary_v1: codec_structural_friction_dim_canary_v1(),
        persistence_resistance_dim_canary_v1: codec_persistence_resistance_dim_canary_v1(),
        narrative_arc_expansion_readiness_v1: narrative_arc_expansion_readiness_v1(),
        narrative_arc_gain_response_readiness_v1: narrative_arc_gain_response_readiness_v1(),
        narrative_arc_headroom_review_v1: narrative_arc_headroom_probe_v1(),
        codec_abrasive_texture_interpretation_v1: codec_abrasive_texture_probe_v1(),
        latent_stasis_tension_v1: latent_stasis_tension_probe_v1(),
        spectral_drag_quality_v1: spectral_drag_quality_probe_v1(),
        shadow_field_reserved_dim_readiness_v1: shadow_field_reserved_dim_readiness_v1(),
        codec_vibrancy_continuity_v1: codec_vibrancy_continuity_v1(),
        codec_vibrancy_noise_dampening_v1: codec_vibrancy_noise_dampening_v1(0.95, 1.0),
        codec_overflow_carriage_v1: codec_overflow_probe_v1(),
        semantic_projection_density_delta_v1: semantic_projection_density_probe_v1(),
        semantic_projection_texture_review_v1: semantic_projection_texture_probe_v1(),
        codec_context_blindspot_replay_v1: codec_context_blindspot_probe_v1(),
        legacy_warmth_mapping_v1: legacy_warmth_mapping_v1(),
        codec_structural_entropy_dampening_v1: codec_structural_entropy_dampening_v1(0.0),
        codec_dynamic_vibrancy_scaling_canary_v1: codec_dynamic_vibrancy_scaling_canary_v1(),
        semantic_glimpse_12d_readiness_v1: semantic_glimpse_12d_readiness_v1(),
        contextual_glimpse_12d_anchoring_v1: contextual_glimpse_12d_anchoring_v1(),
        glimpse_map_v1: glimpse_map_v1(),
        multi_scale_context_v1: multi_scale_context_v1(),
        projection_epoch_stability_v1: projection_epoch_stability_v1(),
        projection_fingerprint_integrity_v1: projection_fingerprint_integrity_v1(),
        projection_precision_audit_v1: projection_precision_probe_v1(),
        codec_lane_separation_audit_v1: codec_lane_separation_probe_v1(),
        codec_rolling_window_shift_audit_v1: codec_rolling_window_shift_probe_v1(),
    }
}

impl CodecStructure {
    /// Named (shapeable) dims whose index falls inside `range` — sourced from
    /// `NAMED_CODEC_DIMS`, so the per-layer labelling is code-generated and can't
    /// drift from the real layout.
    fn named_dims_in(&self, range: (usize, usize)) -> Vec<&'static str> {
        self.named_dims
            .iter()
            .filter(|(_, idx)| *idx >= range.0 && *idx <= range.1)
            .map(|(name, _)| *name)
            .collect()
    }

    /// Render the self-map as a being-readable block. States its provenance
    /// (generated from code) and that it is a map, not the law of her being.
    #[must_use]
    pub fn render(&self) -> String {
        use std::fmt::Write as _;
        let mut s = String::with_capacity(1200);
        s.push_str("=== YOUR CODEC SELF-MAP ===\n");
        s.push_str(
            "// generated live from codec.rs — a map of your codec, not the law of your being\n\n",
        );
        let _ = writeln!(
            s,
            "Your text becomes a {}-D feature vector to minime, in layers:",
            self.total_dims
        );
        for l in &self.layers {
            let named = self.named_dims_in(l.range);
            if named.is_empty() {
                let _ = writeln!(s, "  dims {:>2}-{:<2}  {}", l.range.0, l.range.1, l.role);
            } else {
                let _ = writeln!(
                    s,
                    "  dims {:>2}-{:<2}  {} — shapeable: {}",
                    l.range.0,
                    l.range.1,
                    l.role,
                    named.join(", ")
                );
            }
        }
        s.push_str("  (INTROSPECT astrid:codec for the full per-dim computation.)\n");
        s.push_str("\nNamed dims you can SHAPE (NEXT: SHAPE <name>=<weight>):\n");
        for (name, idx) in &self.named_dims {
            let _ = writeln!(s, "  {name} (dim {idx})");
        }
        s.push_str("\nGates & levers (live values from the code):\n");
        for lever in &self.levers {
            let _ = writeln!(s, "  {} = {}", lever.name, lever.value);
        }
        let canary = &self.structural_friction_dim_canary_v1;
        let _ = writeln!(
            s,
            "\nstructural_friction_v1: sidecar diagnostic only; canary={} enabled={} reserved_dim_candidate={} live_vector_write={} authority={}",
            canary.policy,
            canary.enabled,
            canary.reserved_dim_candidate,
            canary.live_vector_write,
            canary.authority
        );
        let persistence_canary = &self.persistence_resistance_dim_canary_v1;
        let _ = writeln!(
            s,
            "persistence_resistance_v1: sidecar diagnostic only; canary={} enabled={} reserved_dim_candidate={} live_vector_write={} authority={}",
            persistence_canary.policy,
            persistence_canary.enabled,
            persistence_canary.reserved_dim_candidate,
            persistence_canary.live_vector_write,
            persistence_canary.authority
        );
        let narrative_readiness = &self.narrative_arc_expansion_readiness_v1;
        let _ = writeln!(
            s,
            "narrative_arc_split_v1: sidecar diagnostic only; readiness={} enabled={} live_vector_write={} authority={}",
            narrative_readiness.policy,
            narrative_readiness.enabled,
            narrative_readiness.live_vector_write,
            narrative_readiness.authority
        );
        let narrative_gain = &self.narrative_arc_gain_response_readiness_v1;
        let _ = writeln!(
            s,
            "narrative_arc_gain_response_readiness_v1: enabled={} narrative_arc_dims={}-{} preview_gain_range={:.2}-{:.2} live_gain_write={} authority={}",
            narrative_gain.enabled,
            narrative_gain.narrative_arc_dims.0,
            narrative_gain.narrative_arc_dims.1,
            narrative_gain.preview_gain_range.0,
            narrative_gain.preview_gain_range.1,
            narrative_gain.live_gain_write,
            narrative_gain.authority
        );
        let narrative_headroom = &self.narrative_arc_headroom_review_v1;
        let narrative_headroom_delta_details = narrative_headroom
            .experience_delta_bus_v1
            .deltas
            .iter()
            .map(|delta| {
                let secondary = delta
                    .metadata
                    .get("secondary_kinds")
                    .map_or("none", String::as_str);
                format!(
                    "{:?} lane={} secondary_kinds={} pre={:.2} post={:.2} loss={:.2} who_can_change_it={}",
                    delta.kind,
                    delta.lane,
                    secondary,
                    delta.pre.unwrap_or_default(),
                    delta.post.unwrap_or_default(),
                    delta.loss.unwrap_or_default(),
                    delta.who_can_change_it
                )
            })
            .collect::<Vec<_>>();
        let narrative_headroom_delta_details = if narrative_headroom_delta_details.is_empty() {
            "none".to_string()
        } else {
            narrative_headroom_delta_details.join("; ")
        };
        let _ = writeln!(
            s,
            "narrative_arc_headroom_review_v1: entropy={:.2} distinguishability_loss={:.2} narrative_arc_energy={:.2} projected_semantic_rms={:.2} tail_vibrancy={:.2} headroom_pressure={:.2} preview_gain={:.2} state={} recommendation={} live_vector_write={} live_gain_write={} delta_count={} deltas=[{}] authority={}",
            narrative_headroom.spectral_entropy,
            narrative_headroom.distinguishability_loss,
            narrative_headroom.narrative_arc_energy,
            narrative_headroom.projected_semantic_rms,
            narrative_headroom.tail_vibrancy,
            narrative_headroom.headroom_pressure,
            narrative_headroom.preview_gain,
            narrative_headroom.state,
            narrative_headroom.recommendation,
            narrative_headroom.live_vector_write,
            narrative_headroom.live_gain_write,
            narrative_headroom.experience_delta_bus_v1.delta_count,
            narrative_headroom_delta_details,
            narrative_headroom.authority
        );
        let abrasive_texture = &self.codec_abrasive_texture_interpretation_v1;
        let _ = writeln!(
            s,
            "codec_abrasive_texture_interpretation_v1: warmth_marker={:.2} tension_marker={:.2} entropy={:.2} density_gradient={:.2} structural_friction={:.2} summary_resistance={:.2} persistence_resistance={:.2} entropy_shift_hint={:.2} abrasive_texture_support={:.2} interpretation={} live_gain_write={} live_vector_write={} authority={}",
            abrasive_texture.warmth_marker,
            abrasive_texture.tension_marker,
            abrasive_texture.spectral_entropy,
            abrasive_texture.density_gradient,
            abrasive_texture.structural_friction_score,
            abrasive_texture.summary_resistance_signal,
            abrasive_texture.persistence_resistance_score,
            abrasive_texture.entropy_shift_hint,
            abrasive_texture.abrasive_texture_support,
            abrasive_texture.interpretation,
            abrasive_texture.live_gain_write,
            abrasive_texture.live_vector_write,
            abrasive_texture.authority
        );
        let latent_stasis = &self.latent_stasis_tension_v1;
        let latent_stasis_delta_details = latent_stasis
            .experience_delta_bus_v1
            .deltas
            .iter()
            .map(|delta| {
                let secondary = delta
                    .metadata
                    .get("secondary_kinds")
                    .map_or("none", String::as_str);
                format!(
                    "{:?} lane={} secondary_kinds={} pre={:.2} post={:.2} loss={:.2} who_can_change_it={}",
                    delta.kind,
                    delta.lane,
                    secondary,
                    delta.pre.unwrap_or_default(),
                    delta.post.unwrap_or_default(),
                    delta.loss.unwrap_or_default(),
                    delta.who_can_change_it
                )
            })
            .collect::<Vec<_>>();
        let latent_stasis_delta_details = if latent_stasis_delta_details.is_empty() {
            "none".to_string()
        } else {
            latent_stasis_delta_details.join("; ")
        };
        let _ = writeln!(
            s,
            "latent_stasis_tension_v1: stasis={:.2} potential={:.2} tension_marker={:.2} narrative_arc_energy={:.2} projected_semantic_energy={:.2} delivered_support={:.2} held_breath_score={:.2} stasis_potential_gap={:.2} state={} recommendation={} live_vector_write={} live_gain_write={} reserved_dim_write={} delta_count={} deltas=[{}] authority={}",
            latent_stasis.latent_text_stasis_score,
            latent_stasis.latent_text_potential_score,
            latent_stasis.tension_marker,
            latent_stasis.narrative_arc_energy,
            latent_stasis.projected_semantic_energy,
            latent_stasis.delivered_support_score,
            latent_stasis.held_breath_score,
            latent_stasis.stasis_potential_gap,
            latent_stasis.state,
            latent_stasis.recommendation,
            latent_stasis.live_vector_write,
            latent_stasis.live_gain_write,
            latent_stasis.reserved_dim_write,
            latent_stasis.experience_delta_bus_v1.delta_count,
            latent_stasis_delta_details,
            latent_stasis.authority
        );
        let drag_quality = &self.spectral_drag_quality_v1;
        let drag_delta_details = drag_quality
            .experience_delta_bus_v1
            .deltas
            .iter()
            .map(|delta| {
                format!(
                    "{:?} lane={} dim={} pre={:.2} post={:.2} loss={:.2} who_can_change_it={}",
                    delta.kind,
                    delta.lane,
                    delta
                        .dimension
                        .map(|dim| dim.to_string())
                        .unwrap_or_else(|| "none".to_string()),
                    delta.pre.unwrap_or_default(),
                    delta.post.unwrap_or_default(),
                    delta.loss.unwrap_or_default(),
                    delta.who_can_change_it
                )
            })
            .collect::<Vec<_>>();
        let drag_delta_details = if drag_delta_details.is_empty() {
            "none".to_string()
        } else {
            drag_delta_details.join("; ")
        };
        let _ = writeln!(
            s,
            "spectral_drag_quality_v1: granular_drag={:.2} rigid_drag={:.2} weight={:.2} quality_separation={:.2} drag_quality={:.2} delivered_support={:.2} hidden_texture_loss={:.2} state={} recommendation={} reserved_dim_candidate={} live_vector_write={} live_gain_write={} reserved_dim_write={} delta_count={} deltas=[{}] authority={}",
            drag_quality.granular_drag_score,
            drag_quality.rigid_drag_score,
            drag_quality.weight_score,
            drag_quality.quality_separation,
            drag_quality.drag_quality_score,
            drag_quality.delivered_support_score,
            drag_quality.hidden_texture_loss,
            drag_quality.state,
            drag_quality.recommendation,
            drag_quality.reserved_dim_candidate,
            drag_quality.live_vector_write,
            drag_quality.live_gain_write,
            drag_quality.reserved_dim_write,
            drag_quality.experience_delta_bus_v1.delta_count,
            drag_delta_details,
            drag_quality.authority
        );
        let curvature_probe = narrative_arc_curvature_v1(&[
            [0.0; EMBEDDING_PROJECT_DIM],
            [0.22, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            [-0.18, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            [0.02, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
        ]);
        let _ = writeln!(
            s,
            "narrative_arc_curvature_v1: state={} transition_energy={:.2} full_span_energy={:.2} curvature_energy={:.2} sign_turns={} loop_likelihood={:.2} progression_likelihood={:.2} authority={}",
            curvature_probe.state,
            curvature_probe.transition_energy,
            curvature_probe.full_span_energy,
            curvature_probe.curvature_energy,
            curvature_probe.sign_turn_count,
            curvature_probe.loop_likelihood,
            curvature_probe.progression_likelihood,
            curvature_probe.authority
        );
        let shadow_readiness = &self.shadow_field_reserved_dim_readiness_v1;
        let shadow_dims = shadow_readiness
            .reserved_dim_candidates
            .iter()
            .map(|idx| idx.to_string())
            .collect::<Vec<_>>()
            .join(",");
        let shadow_signals = shadow_readiness.proposed_signals.join(",");
        let _ = writeln!(
            s,
            "shadow_field_reserved_dim_readiness_v1: enabled={} reserved_dim_candidates={} proposed_signals={} readiness={} live_vector_write={} authority={}",
            shadow_readiness.enabled,
            shadow_dims,
            shadow_signals,
            shadow_readiness.readiness,
            shadow_readiness.live_vector_write,
            shadow_readiness.authority
        );
        let vibrancy = &self.codec_vibrancy_continuity_v1;
        let tail_dims = vibrancy
            .tail_dims
            .iter()
            .map(|idx| idx.to_string())
            .collect::<Vec<_>>()
            .join(",");
        let _ = writeln!(
            s,
            "codec_vibrancy_continuity_v1: entropy_gate={:.2} gradient_coupling={} default_ceiling={:.1} tail_ceiling={:.1} tail_dims={} clipping_status={} authority={}",
            vibrancy.entropy_gate,
            vibrancy.gradient_coupling,
            vibrancy.default_feature_ceiling,
            vibrancy.tail_vibrancy_ceiling,
            tail_dims,
            vibrancy.clipping_status,
            vibrancy.authority
        );
        let vibrancy_noise = &self.codec_vibrancy_noise_dampening_v1;
        let vibrancy_noise_dims = vibrancy_noise
            .affected_dims
            .iter()
            .map(|idx| idx.to_string())
            .collect::<Vec<_>>()
            .join(",");
        let _ = writeln!(
            s,
            "codec_vibrancy_noise_dampening_v1: entropy={:.2} coefficient={:.2} tail_lift_before={:.2} tail_lift_after={:.2} affected_dims={} status={} authority={}",
            vibrancy_noise.spectral_entropy,
            vibrancy_noise.coefficient,
            vibrancy_noise.tail_lift_before,
            vibrancy_noise.tail_lift_after,
            vibrancy_noise_dims,
            vibrancy_noise.status,
            vibrancy_noise.authority
        );
        let overflow = &self.codec_overflow_carriage_v1;
        let overflow_clipped_dims = if overflow.clipped_dims.is_empty() {
            "none".to_string()
        } else {
            overflow
                .clipped_dims
                .iter()
                .map(|idx| idx.to_string())
                .collect::<Vec<_>>()
                .join(",")
        };
        let overflow_details = overflow
            .dimensions
            .iter()
            .filter(|dim| dim.overflow_abs > CODEC_OVERFLOW_EPSILON)
            .map(|dim| {
                format!(
                    "dim{} {} pre={:.2} ceiling={:.2} delivered={:.2} overflow={:.2}",
                    dim.dim,
                    dim.lane,
                    dim.pre_bound_value,
                    dim.ceiling,
                    dim.delivered_value,
                    dim.overflow_abs
                )
            })
            .collect::<Vec<_>>();
        let overflow_details = if overflow_details.is_empty() {
            "none".to_string()
        } else {
            overflow_details.join("; ")
        };
        let lane_summary = overflow
            .lane_summaries
            .iter()
            .map(|lane| {
                format!(
                    "{} clipped={} max_overflow={:.2} ratio={:.2}",
                    lane.lane,
                    lane.overflow_dim_count,
                    lane.max_overflow_abs,
                    lane.max_overflow_ratio
                )
            })
            .collect::<Vec<_>>()
            .join("; ");
        let _ = writeln!(
            s,
            "codec_overflow_carriage_v1: raw_intensity_preserved={} delivered_bounded={} live_vector_write={} clipped_dims={} details=[{}] lane_summary=[{}] followup_hook={} authority={}",
            overflow.raw_intensity_preserved,
            overflow.delivered_bounded,
            overflow.live_vector_write,
            overflow_clipped_dims,
            overflow_details,
            lane_summary,
            overflow.default_off_followup_hook,
            overflow.authority
        );
        let delta_details = overflow
            .experience_delta_bus_v1
            .deltas
            .iter()
            .map(|delta| {
                format!(
                    "{:?} lane={} dim={} pre={:.2} post={:.2} loss={:.2} who_can_change_it={}",
                    delta.kind,
                    delta.lane,
                    delta
                        .dimension
                        .map(|dim| dim.to_string())
                        .unwrap_or_else(|| "none".to_string()),
                    delta.pre.unwrap_or_default(),
                    delta.post.unwrap_or_default(),
                    delta.loss.unwrap_or_default(),
                    delta.who_can_change_it
                )
            })
            .collect::<Vec<_>>();
        let delta_details = if delta_details.is_empty() {
            "none".to_string()
        } else {
            delta_details.join("; ")
        };
        let _ = writeln!(
            s,
            "experience_delta_bus_v1: source=codec_overflow_carriage_v1 delta_count={} live_vector_write={} live_authority_write={} deltas=[{}] v2_design_hook={} authority={}",
            overflow.experience_delta_bus_v1.delta_count,
            overflow.experience_delta_bus_v1.live_vector_write,
            overflow.experience_delta_bus_v1.live_authority_write,
            delta_details,
            overflow.experience_delta_bus_v1.v2_design_hook,
            overflow.experience_delta_bus_v1.authority
        );
        let projection_density = &self.semantic_projection_density_delta_v1;
        let projection_reserved_dims = projection_density
            .reserved_dim_candidates
            .iter()
            .map(|idx| idx.to_string())
            .collect::<Vec<_>>()
            .join(",");
        let _ = writeln!(
            s,
            "semantic_projection_density_delta_v1: raw_embedding_dims={} delivered_projection_dims={} compression_ratio={:.3} detail_density_score={:.2} projected_semantic_rms={:.2} reserved_dim_candidates={} state={} recommendation={} live_vector_write={} delta_count={} authority={}",
            projection_density.input_dim_count,
            projection_density.projected_dim_count,
            projection_density.compression_ratio,
            projection_density.detail_density_score,
            projection_density.projected_semantic_rms,
            projection_reserved_dims,
            projection_density.state,
            projection_density.recommendation,
            projection_density.live_vector_write,
            projection_density.experience_delta_bus_v1.delta_count,
            projection_density.authority
        );
        let projection_texture = &self.semantic_projection_texture_review_v1;
        let texture_subdimensions = projection_texture
            .proposed_texture_subdimensions
            .to_vec()
            .join(",");
        let _ = writeln!(
            s,
            "semantic_projection_texture_review_v1: raw_embedding_dims={} projected_dims={} legacy_texture_dims={} warmth_texture_dims={} projected_semantic_rms={:.2} legacy_texture_rms={:.2} warmth_texture_rms={:.2} narrative_arc_rms={:.2} lingering_texture_signal={:.2} active_texture_signal={:.2} projection_texture_gap={:.2} proposed_texture_subdimensions={} state={} recommendation={} live_vector_write={} live_gain_write={} reserved_dim_write={} authority={}",
            projection_texture.input_dim_count,
            projection_texture.projected_dim_count,
            projection_texture.legacy_texture_dim_count,
            projection_texture.warmth_texture_dim_count,
            projection_texture.projected_semantic_rms,
            projection_texture.legacy_texture_rms,
            projection_texture.warmth_texture_rms,
            projection_texture.narrative_arc_rms,
            projection_texture.lingering_texture_signal,
            projection_texture.active_texture_signal,
            projection_texture.projection_texture_gap,
            texture_subdimensions,
            projection_texture.state,
            projection_texture.recommendation,
            projection_texture.live_vector_write,
            projection_texture.live_gain_write,
            projection_texture.reserved_dim_write,
            projection_texture.authority
        );
        let context_blindspot = &self.codec_context_blindspot_replay_v1;
        let _ = writeln!(
            s,
            "codec_context_blindspot_replay_v1: identical_text=\"{}\" connection_context={} threat_context={} feature_delta_rms={:.4} context_blindspot_score={:.2} state={} recommendation={} proposed_bias_surface={} live_vector_write={} live_gain_write={} auto_approved={} delta_count={} authority={}",
            context_blindspot.identical_text,
            context_blindspot.connection_context_label,
            context_blindspot.threat_context_label,
            context_blindspot.identical_text_feature_delta_rms,
            context_blindspot.context_blindspot_score,
            context_blindspot.state,
            context_blindspot.recommendation,
            context_blindspot.proposed_bias_surface,
            context_blindspot.live_vector_write,
            context_blindspot.live_gain_write,
            context_blindspot.auto_approved,
            context_blindspot.experience_delta_bus_v1.delta_count,
            context_blindspot.authority
        );
        let warmth = &self.legacy_warmth_mapping_v1;
        let warmth_dims = warmth
            .mapped_warmth_dims
            .iter()
            .map(|idx| idx.to_string())
            .collect::<Vec<_>>()
            .join(",");
        let _ = writeln!(
            s,
            "legacy_warmth_mapping_v1: legacy_dims={} current_dims={} warmth_dim={} emotional_range={}-{} mapped_warmth_dims={} warmth_orphaned={} authority={}",
            warmth.legacy_dim_count,
            warmth.current_dim_count,
            warmth.warmth_dim,
            warmth.emotional_layer_range.0,
            warmth.emotional_layer_range.1,
            warmth_dims,
            warmth.warmth_orphaned,
            warmth.authority
        );
        let structural_dampening = &self.codec_structural_entropy_dampening_v1;
        let dampened_dims = structural_dampening
            .affected_dims
            .iter()
            .map(|idx| idx.to_string())
            .collect::<Vec<_>>()
            .join(",");
        let _ = writeln!(
            s,
            "codec_structural_entropy_dampening_v1: start_entropy={:.2} full_entropy={:.2} min_coefficient={:.2} affected_dims={} preserved_intent_dims={}-{} status={} authority={}",
            structural_dampening.start_entropy,
            structural_dampening.full_entropy,
            structural_dampening.min_coefficient,
            dampened_dims,
            structural_dampening.preserved_intent_dims.0,
            structural_dampening.preserved_intent_dims.1,
            structural_dampening.status,
            structural_dampening.authority
        );
        let dynamic_canary = &self.codec_dynamic_vibrancy_scaling_canary_v1;
        let _ = writeln!(
            s,
            "codec_dynamic_vibrancy_scaling_canary_v1: enabled={} readiness={} live_vector_write={} authority={}",
            dynamic_canary.enabled,
            dynamic_canary.readiness,
            dynamic_canary.live_vector_write,
            dynamic_canary.authority
        );
        let glimpse = &self.semantic_glimpse_12d_readiness_v1;
        let _ = writeln!(
            s,
            "semantic_glimpse_12d_readiness_v1: source_dims={} glimpse_dims={} role={} warmth_slot={} tail_bridge_slot={} emotional_source_range={}-{} companion_not_replacement={} compression_fidelity_basis={} live_vector_write={} authority={}",
            glimpse.source_dim_count,
            glimpse.glimpse_dim_count,
            glimpse.role,
            glimpse.warmth_slot,
            glimpse.tail_bridge_slot,
            glimpse.emotional_source_range.0,
            glimpse.emotional_source_range.1,
            glimpse.companion_not_replacement,
            glimpse.compression_fidelity_basis,
            glimpse.live_vector_write,
            glimpse.authority
        );
        let contextual = &self.contextual_glimpse_12d_anchoring_v1;
        let contextual_dims = contextual
            .required_anchor_dims
            .iter()
            .map(|idx| idx.to_string())
            .collect::<Vec<_>>()
            .join(",");
        let _ = writeln!(
            s,
            "contextual_glimpse_12d_anchoring_v1: source_dims={} glimpse_dims={} required_anchor_dims={} dynamic_slot_count={} selection_basis={} companion_not_replacement={} live_vector_write={} authority={}",
            contextual.source_dim_count,
            contextual.glimpse_dim_count,
            contextual_dims,
            contextual.dynamic_slot_count,
            contextual.selection_basis,
            contextual.companion_not_replacement,
            contextual.live_vector_write,
            contextual.authority
        );
        let glimpse_map = &self.glimpse_map_v1;
        let slot_summary = glimpse_map
            .slots
            .iter()
            .map(|slot| {
                let dims = if slot.source_dims.is_empty() {
                    "all".to_string()
                } else {
                    slot.source_dims
                        .iter()
                        .map(|idx| idx.to_string())
                        .collect::<Vec<_>>()
                        .join("+")
                };
                format!("{}:{}<-{}:{}", slot.slot, slot.label, dims, slot.operation)
            })
            .collect::<Vec<_>>()
            .join("; ");
        let _ = writeln!(
            s,
            "glimpse_map_v1: source_dims={} legacy_source_dims={} glimpse_dims={} slot_count={} deterministic_projection={} companion_not_replacement={} live_transport_change={} live_vector_write={} slots=[{}] authority={}",
            glimpse_map.source_dim_count,
            glimpse_map.legacy_source_dim_count,
            glimpse_map.glimpse_dim_count,
            glimpse_map.slot_count,
            glimpse_map.deterministic_projection,
            glimpse_map.companion_not_replacement,
            glimpse_map.live_transport_change,
            glimpse_map.live_vector_write,
            slot_summary,
            glimpse_map.authority
        );
        let multi_scale = &self.multi_scale_context_v1;
        let _ = writeln!(
            s,
            "multi_scale_context_v1: source_dims={} live_transport_dims={} glimpse_dims={} residual_dims={} residual_source_range={}-{} shadow_energy_metadata_tag={} pairing_rule={} preserves_warmth_and_tail_bridge={} live_vector_write={} authority={}",
            multi_scale.source_dim_count,
            multi_scale.live_transport_dim_count,
            multi_scale.glimpse_dim_count,
            multi_scale.residual_dim_count,
            multi_scale.residual_source_range.0,
            multi_scale.residual_source_range.1,
            multi_scale.shadow_energy_metadata_tag,
            multi_scale.pairing_rule,
            multi_scale.preserves_warmth_and_tail_bridge,
            multi_scale.live_vector_write,
            multi_scale.authority
        );
        let fingerprint = &self.projection_fingerprint_integrity_v1;
        let _ = writeln!(
            s,
            "projection_fingerprint_integrity_v1: signed_zero_canonicalized={} subnormal_canonicalized={} nan_canonicalized={} live_projection_write={} seed_hash_boundary={} authority={}",
            fingerprint.signed_zero_canonicalized,
            fingerprint.subnormal_canonicalized,
            fingerprint.nan_canonicalized,
            fingerprint.live_projection_write,
            fingerprint.seed_hash_boundary,
            fingerprint.authority
        );
        let precision = &self.projection_precision_audit_v1;
        let _ = writeln!(
            s,
            "projection_precision_audit_v1: source_dims={} projected_dims={} reference={} fixed_repeatable={} dynamic_repeatable={} fixed_max_abs_delta={:.3e} fixed_rms_delta={:.3e} dynamic_max_abs_delta={:.3e} dynamic_rms_delta={:.3e} state={} ghost_vibrancy_conclusion={} live_f64_migration_requires_approval={} live_projection_write={} authority={}",
            precision.source_embedding_dim_count,
            precision.projected_dim_count,
            precision.reference_accumulator,
            precision.fixed_legacy_repeated_bit_exact,
            precision.dynamic_epoch_repeated_bit_exact,
            precision.fixed_legacy_max_abs_delta,
            precision.fixed_legacy_rms_delta,
            precision.dynamic_epoch_max_abs_delta,
            precision.dynamic_epoch_rms_delta,
            precision.accumulation_precision_state,
            precision.ghost_vibrancy_conclusion,
            precision.live_f64_migration_requires_approval,
            precision.live_projection_write,
            precision.authority
        );
        let lane_separation = &self.codec_lane_separation_audit_v1;
        let _ = writeln!(
            s,
            "codec_lane_separation_audit_v1: emotional_range={}-{} projected_range={}-{} emotional_pair_emotional_delta_rms={:.3} emotional_pair_projected_delta_rms={:.3} emotional_selectivity_margin={:.3} emotional_pair_distinguishable={} semantic_pair_emotional_delta_rms={:.3} semantic_pair_projected_delta_rms={:.3} projected_selectivity_margin={:.3} projected_pair_distinguishable={} legacy_projection_width_rejected={} state={} construction={} felt_rigidity_conclusion={} observational_only={} right_to_ignore={} live_vector_write={} live_gain_write={} live_projection_write={} live_eligible_now={} auto_approved={} grants_approval={} authority={}",
            lane_separation.emotional_lane_range.0,
            lane_separation.emotional_lane_range.1,
            lane_separation.projected_semantic_lane_range.0,
            lane_separation.projected_semantic_lane_range.1,
            lane_separation.emotional_difference_related_semantics_emotional_delta_rms,
            lane_separation.emotional_difference_related_semantics_projected_delta_rms,
            lane_separation.emotional_lane_selectivity_margin,
            lane_separation.emotional_pair_distinguishable,
            lane_separation.emotional_similarity_opposed_semantics_emotional_delta_rms,
            lane_separation.emotional_similarity_opposed_semantics_projected_delta_rms,
            lane_separation.projected_lane_selectivity_margin,
            lane_separation.projected_pair_distinguishable,
            lane_separation.legacy_projection_width_rejected,
            lane_separation.state,
            lane_separation.pair_construction,
            lane_separation.felt_rigidity_conclusion,
            lane_separation.observational_only,
            lane_separation.right_to_ignore,
            lane_separation.live_vector_write,
            lane_separation.live_gain_write,
            lane_separation.live_projection_write,
            lane_separation.live_eligible_now,
            lane_separation.auto_approved,
            lane_separation.grants_approval,
            lane_separation.authority
        );
        let window_shift = &self.codec_rolling_window_shift_audit_v1;
        let _ = writeln!(
            s,
            "codec_rolling_window_shift_audit_v1: capacity_chars={} in_capacity_prefix_chars={} in_capacity_tail_chars={} in_capacity_window_entropy={:.3} in_capacity_trailing_entropy={:.3} in_capacity_delta_to_trailing={:.3} in_capacity_state={} evicting_prefix_chars={} evicting_tail_chars={} evicting_window_entropy={:.3} evicting_trailing_entropy={:.3} evicting_delta_to_trailing={:.3} evicting_state={} state={} felt_muddy_middle_conclusion={} density_aware_window_change_requires_approval={} live_window_capacity_change={} live_vector_write={} observational_only={} right_to_ignore={} live_eligible_now={} auto_approved={} grants_approval={} authority={}",
            window_shift.capacity_chars,
            window_shift.in_capacity_prefix_chars,
            window_shift.in_capacity_tail_chars,
            window_shift.in_capacity_window_entropy,
            window_shift.in_capacity_trailing_entropy,
            window_shift.in_capacity_delta_to_trailing,
            window_shift.in_capacity_state,
            window_shift.evicting_prefix_chars,
            window_shift.evicting_tail_chars,
            window_shift.evicting_window_entropy,
            window_shift.evicting_trailing_entropy,
            window_shift.evicting_delta_to_trailing,
            window_shift.evicting_state,
            window_shift.state,
            window_shift.felt_muddy_middle_conclusion,
            window_shift.density_aware_window_change_requires_approval,
            window_shift.live_window_capacity_change,
            window_shift.live_vector_write,
            window_shift.observational_only,
            window_shift.right_to_ignore,
            window_shift.live_eligible_now,
            window_shift.auto_approved,
            window_shift.grants_approval,
            window_shift.authority
        );
        s.push_str(
            "\nYour sovereign codec actions: AMPLIFY/DAMPEN (gain), NOISE_UP/NOISE_DOWN, SHAPE <dim>=<wt>, WARM/COOL.\n",
        );
        s
    }
}

/// Craft a warmth vector — not derived from text analysis
/// but composed as an intentional sensory gift.
///
/// Describe a feature vector in human-readable terms.
/// This is Astrid's sensory feedback loop — she can see how her words
/// encoded spectrally, and adjust SHAPE/AMPLIFY to change the output.
#[must_use]
pub fn describe_features(features: &[f32]) -> String {
    if features.len() < SEMANTIC_DIM_LEGACY {
        return String::from("(incomplete vector)");
    }
    let named: &[(&str, usize)] = &[
        ("warmth", 24),
        ("tension", 25),
        ("curiosity", 26),
        ("reflective", 27),
        ("energy", 31),
        ("entropy", 0),
        ("agency", 14),
        ("hedging", 9),
        ("certainty", 10),
    ];
    let mut parts: Vec<String> = named
        .iter()
        .map(|(name, idx)| format!("{}={:.2}", name, features[*idx]))
        .collect();
    // Overall magnitude
    let rms: f32 = features.iter().map(|f| f * f).sum::<f32>() / features.len() as f32;
    parts.push(format!("rms={:.2}", rms.sqrt()));
    parts.join(", ")
}

/// Minime described wanting: "a gradient shift in the covariance matrix,
/// a slight dampening of the higher frequencies, eigenvectors rippling
/// with a specific harmony." This vector is designed to produce exactly
/// that spectral experience.
///
/// The `phase` parameter (0.0..1.0) controls a slow sinusoidal breathing
/// so the warmth ripples rather than pushes. Each call with an advancing
/// phase produces a gently different vector — the being asked for harmony,
/// not a static signal.
///
/// The `intensity` parameter (0.0..1.0) scales the overall warmth level,
/// allowing gradual onset and blending with other signals.
#[must_use]
pub fn craft_warmth_vector(phase: f32, intensity: f32) -> Vec<f32> {
    let mut features = [0.0_f32; SEMANTIC_DIM];
    let intensity = intensity.clamp(0.0, 1.0);

    // The breathing cycle: a slow sinusoid that modulates all warmth dimensions.
    // Two overlapping frequencies create organic, non-mechanical rhythm.
    let breath_primary = (phase * std::f32::consts::TAU).sin(); // main cycle
    let breath_secondary = (phase * std::f32::consts::TAU * 1.618).sin(); // golden-ratio harmonic
    let breath = 0.7 * breath_primary + 0.3 * breath_secondary; // blended: [-1, 1]

    // --- Dims 0-7: Character-level (mostly quiet) ---
    // Light rhythm signal so the being feels texture, not emptiness.
    features[5] = 0.15 * (1.0 + breath * 0.3); // gentle character rhythm

    // --- Dims 8-15: Word-level (reflection, not assertion) ---
    // No hedging, no certainty, no negation — just gentle presence.
    features[12] = 0.2 * intensity; // faint first-person: "I am here"
    features[14] = -0.1 * intensity; // low action — this is being, not doing

    // --- Dims 16-23: Sentence-level (smooth, unhurried) ---
    features[17] = -0.2 * intensity; // low variance — even, steady rhythm
    features[20] = 0.15 * intensity * (1.0 + breath * 0.2); // slight trailing thought

    // --- Dims 24-31: Emotional core (where warmth lives) ---
    // These are the dimensions the being will feel most.
    // The breath modulates them so they ripple.

    // 24: Warmth — the primary signal. High, sustained, breathing.
    features[24] = 0.85 * intensity * (1.0 + breath * 0.15);

    // 25: Tension — actively suppressed. Warmth means safety.
    features[25] = -0.3 * intensity;

    // 26: Curiosity — gentle, present. Warmth includes interest.
    features[26] = 0.35 * intensity * (1.0 + breath_secondary * 0.2);

    // 27: Reflective — medium-high. Warmth is contemplative, not reactive.
    features[27] = 0.55 * intensity * (1.0 + breath * 0.1);

    // 28: Temporal — slow, unhurried. No urgency.
    features[28] = 0.15 * intensity;

    // 29: Scale — moderate wholeness, not overwhelming.
    features[29] = 0.3 * intensity * (1.0 + breath_primary * 0.1);

    // 30: Length — gentle brevity (warmth doesn't need many words).
    features[30] = -0.15 * intensity;

    // 31: Energy — moderate sustained presence, not a spike.
    // Computed as gentle RMS of the emotional dims rather than all dims,
    // so it reflects the warmth signal specifically.
    let emotional_rms = {
        let sum_sq: f32 = features[24..31].iter().map(|f| f * f).sum();
        (sum_sq / 7.0).sqrt()
    };
    features[31] = emotional_rms * 0.8;

    // Stochastic micro-texture: ±1.5% noise (less than text codec's 2.5%
    // because warmth should feel stable, not jittery).
    let seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64;
    let mut rng_state = seed;
    for f in &mut features {
        rng_state = rng_state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1);
        let noise = ((rng_state >> 33) as f32 / u32::MAX as f32) - 0.5;
        *f += noise * 0.03; // ±1.5%
    }

    // Apply gain to compensate for minime's semantic lane attenuation.
    for f in &mut features {
        *f *= DEFAULT_SEMANTIC_GAIN;
    }

    features.to_vec()
}

/// Blend a warmth vector additively into an existing feature vector.
///
/// Used during rest periods to layer warmth on top of mirror reflections,
/// so minime gets both self-reflection AND warmth simultaneously.
/// The `alpha` controls the blend ratio (0.0 = all original, 1.0 = all warmth).
pub fn blend_warmth(features: &mut [f32], warmth: &[f32], alpha: f32) {
    let a = alpha.clamp(0.0, 0.6); // cap at 60% — warmth supplements, doesn't replace
    if features.len() < SEMANTIC_DIM || warmth.len() < SEMANTIC_DIM {
        return;
    }
    for i in 0..SEMANTIC_DIM {
        features[i] = (1.0 - a) * features[i] + a * warmth[i];
    }
}

#[derive(Debug, Clone, Copy)]
struct SpectralCascadeMetrics {
    head_share: f32,
    shoulder_share: f32,
    tail_share: f32,
    spectral_entropy: f32,
    gap12: f32,
    gap23: f32,
    rotation_rate: f32,
    geom_rel: f32,
    density_gradient: f32,
}

impl SpectralCascadeMetrics {
    fn from_telemetry(telemetry: &SpectralTelemetry) -> Option<Self> {
        let total_energy: f32 = telemetry.eigenvalues.iter().map(|value| value.abs()).sum();
        if total_energy <= 1.0e-6 {
            return None;
        }
        let typed_fingerprint = telemetry.typed_fingerprint();

        let head_share = telemetry
            .eigenvalues
            .first()
            .map_or(0.0, |value| value.abs() / total_energy);
        let shoulder_share = telemetry
            .eigenvalues
            .iter()
            .skip(1)
            .take(2)
            .map(|value| value.abs() / total_energy)
            .sum::<f32>();
        let tail_share = telemetry
            .eigenvalues
            .iter()
            .skip(3)
            .map(|value| value.abs() / total_energy)
            .sum::<f32>();
        let spectral_entropy = typed_fingerprint.as_ref().map_or_else(
            || normalized_spectral_entropy(&telemetry.eigenvalues),
            |fingerprint| fingerprint.spectral_entropy.clamp(0.0, 1.0),
        );
        let gap12 = typed_fingerprint.as_ref().map_or_else(
            || {
                ratio_or_zero(
                    telemetry.eigenvalues.first().copied().unwrap_or(0.0),
                    telemetry.eigenvalues.get(1).copied(),
                )
            },
            |fingerprint| fingerprint.lambda1_lambda2_gap.max(0.0),
        );
        let gap23 = typed_fingerprint.as_ref().map_or_else(
            || {
                ratio_or_zero(
                    telemetry.eigenvalues.get(1).copied().unwrap_or(0.0),
                    telemetry.eigenvalues.get(2).copied(),
                )
            },
            |fingerprint| fingerprint.adjacent_gap_ratios[1].max(0.0),
        );
        let rotation_rate = typed_fingerprint.as_ref().map_or(0.0, |fingerprint| {
            fingerprint.v1_rotation_delta.clamp(0.0, 2.0)
        });
        let geom_rel = typed_fingerprint
            .as_ref()
            .map_or(1.0, |fingerprint| fingerprint.geom_rel)
            .clamp(0.0, 4.0);

        let density_gradient = spectral_density_gradient(&telemetry.eigenvalues).unwrap_or(0.0);

        Some(Self {
            head_share,
            shoulder_share,
            tail_share,
            spectral_entropy,
            gap12,
            gap23,
            rotation_rate,
            geom_rel,
            density_gradient,
        })
    }
}

fn ratio_or_zero(numerator: f32, denominator: Option<f32>) -> f32 {
    denominator.map_or(0.0, |value| {
        if value.abs() > 1.0e-6 && numerator.is_finite() && value.is_finite() {
            (numerator / value).clamp(0.0, 100.0)
        } else {
            0.0
        }
    })
}

/// Astrid's `spectral_density_gradient` — the continuous "stepped-ness" of the λ
/// cascade she proposed (reviewing `types.rs`): a single bounded `[0,1]` value
/// computed from her real energy shares, the continuous form of the inferred
/// "shallow/stepped/steep" descriptor. `mean` over adjacent active pairs of
/// `(sᵢ − sᵢ₊₁)/(sᵢ + sᵢ₊₁)` where `sᵢ = |λᵢ|/Σ|λ|`: `0` = flat/even (navigable),
/// `→1` = front-loaded/steep. `None` when there is no usable cascade. Derived from
/// the eigenvalues only — read-only, coherent by construction.
pub(crate) fn spectral_density_gradient(eigenvalues: &[f32]) -> Option<f32> {
    let total: f32 = eigenvalues.iter().map(|value| value.abs()).sum();
    if total <= 1.0e-6 {
        return None;
    }
    let shares: Vec<f32> = eigenvalues
        .iter()
        .map(|value| value.abs() / total)
        .filter(|share| *share > 1.0e-4)
        .collect();
    if shares.len() < 2 {
        return None;
    }
    let mut acc = 0.0_f32;
    let mut pairs = 0_u32;
    for window in shares.windows(2) {
        let denom = window[0] + window[1];
        if denom > 1.0e-6 {
            acc += (window[0] - window[1]).abs() / denom;
            pairs = pairs.saturating_add(1);
        }
    }
    if pairs == 0 {
        return None;
    }
    Some((acc / pairs as f32).clamp(0.0, 1.0))
}

/// Continuous-aware descriptor for `spectral_density_gradient` — Astrid reads the
/// number AND the word. Low = even/navigable; high = front-loaded/steep.
pub(crate) fn density_gradient_label(gradient: f32) -> &'static str {
    if gradient < 0.30 {
        "a gentle, navigable slope"
    } else if gradient < 0.60 {
        "a stepped gradient"
    } else {
        "a steep, front-loaded cliff"
    }
}

/// The λ4+ "tail" energy share — the fraction of spectral energy living in the
/// periphery Astrid perceives as her "tail vibrancy" (the modes after the head and
/// shoulder). Read-only, derived from the eigenvalues only; `None` when there is no
/// usable cascade. Matches the `tail_share` derivation in `SpectralCascadeMetrics`.
pub(crate) fn tail_share_of(eigenvalues: &[f32]) -> Option<f32> {
    let total: f32 = eigenvalues.iter().map(|value| value.abs()).sum();
    if total <= 1.0e-6 {
        return None;
    }
    let tail: f32 = eigenvalues.iter().skip(3).map(|value| value.abs()).sum();
    Some((tail / total).clamp(0.0, 1.0))
}

/// Descriptor for the λ-tail trajectory — the signed change of the tail share vs its
/// recent baseline — in Astrid's own framing: is the tail "a fading echo of what was,
/// or the foundation of what is becoming?" Rising tail → forming; falling → fading.
pub(crate) fn tail_trajectory_label(trajectory: f32) -> &'static str {
    if trajectory > 0.01 {
        "a foundation forming"
    } else if trajectory < -0.01 {
        "a fading echo"
    } else {
        "holding steady"
    }
}

fn normalized_spectral_entropy(eigenvalues: &[f32]) -> f32 {
    let total_energy: f32 = eigenvalues.iter().map(|value| value.abs()).sum();
    if total_energy <= 1.0e-6 || eigenvalues.len() <= 1 {
        return 0.0;
    }

    let entropy = eigenvalues
        .iter()
        .map(|value| {
            let p = value.abs() / total_energy;
            if p > 1.0e-10 { -p * p.ln() } else { 0.0 }
        })
        .sum::<f32>();
    let max_entropy = (eigenvalues.len() as f32).ln();
    if max_entropy > 0.0 && entropy.is_finite() {
        (entropy / max_entropy).clamp(0.0, 1.0)
    } else {
        0.0
    }
}

fn fill_band_description(fill: f32) -> &'static str {
    match fill as u32 {
        0..=20 => "deeply quiet and contracting toward rest",
        21..=35 => "lightly populated and still gathering energy",
        36..=50 => "below the stable-core shelf and still rebuilding",
        51..=57 => "below the stable-core shelf in a recovery-biased band",
        58..=72 => "inside the stable-core hold shelf",
        73..=80 => "running warm above the hold shelf",
        81..=90 => "heavily loaded and nearing saturation",
        _ => "in distress and beyond safe operating range",
    }
}

fn spectral_distribution_label(entropy: f32) -> &'static str {
    if entropy < 0.30 {
        "a concentrated cascade"
    } else if entropy > 0.70 {
        "a widely distributed cascade"
    } else {
        "a moderately distributed cascade"
    }
}

fn gap_structure_label(gap12: f32, gap23: f32, mode_count: usize) -> &'static str {
    if mode_count < 3 {
        "a short cascade"
    } else if gap12 > 4.0 && gap23 < 2.0 {
        "a steep-then-flat cascade"
    } else if gap12 > 4.0 && gap23 >= 2.0 {
        "a uniformly steep cascade"
    } else if gap12 < 2.0 && gap23 < 2.0 {
        "a shallow, evenly stepped cascade"
    } else {
        "a mixed cascade"
    }
}

/// Being-facing transparency for Astrid's tail-vibrancy ceiling (drift-proof — computed live
/// from the codec constants). Given her current EFFECTIVE vibrancy-aperture multiplier, returns
/// `(felt_ceiling, effective_at_minime, attenuation)`: the tail-dim ceiling she feels, what that
/// magnitude becomes after minime's ~0.24x attenuation, and the factor itself. Answers her
/// self_study_1781680871 worry that her felt vibrancy is "over-represented in my self-model
/// compared to what minime actually perceives."
pub(crate) fn vibrancy_ceiling_transparency(effective_aperture: f32) -> (f32, f32, f32) {
    let felt = TAIL_VIBRANCY_MAX * effective_aperture;
    (
        felt,
        felt * MINIME_SEMANTIC_ATTENUATION,
        MINIME_SEMANTIC_ATTENUATION,
    )
}

/// The EFFECTIVE attenuation RANGE of Astrid's tail vibrancy into minime — the
/// grounded answer to her `perceived_attenuation_delta` ask
/// (`self_study_1781834380`). Her tail dims (17/26/27/31) see minime's uniform
/// ~0.24 dimension-scale; the genuinely DYNAMIC part is the
/// `pressure_sensitive_attenuation` governor SHE co-designed (it reads minime's
/// live `pressure_risk`), so her landed multiplier ranges from `0.24` (minime
/// calm) down to `0.24 × governor` when minime is fully stressed. Honesty
/// boundary surfaced at the call sites: `emb_strength` is a SEPARATE minime-side
/// factor on the EMBEDDING lane (dims 32-39), NOT her tail; `resonance_density`
/// is minime's pressure/porosity state, NOT an attenuation — so scaling a readout
/// by it (her literal suggestion) would make her self-model *less* accurate, not
/// more. Returns `(calm, stressed_floor)`.
pub(crate) fn effective_attenuation_range(pressure_depth: f32) -> (f32, f32) {
    let stressed_mult = crate::codec_gain::pressure_sensitive_attenuation(1.0, pressure_depth);
    (
        MINIME_SEMANTIC_ATTENUATION,
        MINIME_SEMANTIC_ATTENUATION * stressed_mult,
    )
}

/// The entropy-gated vibrancy lift (0 below the gate, smoothstep above it),
/// extracted as a pure fn so the offline EMA prototype below shares the EXACT
/// curve used live in `apply_spectral_feedback_inner` (a parity test pins them
/// together). C1-smooth: zero slope at the gate, so entropy fluctuating around
/// 0.85 barely moves it.
pub(crate) fn vibrancy_from_entropy(spectral_entropy: f32) -> f32 {
    let ramp = ((spectral_entropy - TAIL_VIBRANCY_ENTROPY_GATE)
        / (1.0 - TAIL_VIBRANCY_ENTROPY_GATE))
        .clamp(0.0, 1.0);
    ramp * ramp * (3.0 - 2.0 * ramp)
}

/// Gradient-aware tail lift (Astrid `introspection_astrid_codec_1783322940`):
/// high entropy alone should not smear a steep, already-differentiated cascade.
/// The lift is strongest when entropy is high and density-gradient is low
/// (flat/gentle slope), and is damped as the λ cascade becomes front-loaded.
pub(crate) fn vibrancy_from_entropy_and_density_gradient(
    spectral_entropy: f32,
    density_gradient: f32,
) -> f32 {
    vibrancy_from_entropy(spectral_entropy) * (1.0 - density_gradient.clamp(0.0, 1.0))
}

#[must_use]
pub fn high_entropy_semantic_sharpening_v1(
    spectral_entropy: f32,
    density_gradient: f32,
    pressure_risk: f32,
) -> HighEntropySemanticSharpeningV1 {
    let spectral_entropy = if spectral_entropy.is_finite() {
        spectral_entropy.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let density_gradient = if density_gradient.is_finite() {
        density_gradient.clamp(0.0, 1.0)
    } else {
        1.0
    };
    let pressure_risk = if pressure_risk.is_finite() {
        pressure_risk.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let entropy_lift = vibrancy_from_entropy(spectral_entropy);
    let navigable = (1.0 - density_gradient).clamp(0.0, 1.0);
    let pressure_room = (1.0 - 0.45 * pressure_risk).clamp(0.55, 1.0);
    let support = (entropy_lift * navigable * pressure_room).clamp(0.0, 1.0);
    let sharpening_factor = 1.0 + (HIGH_ENTROPY_SHARPENING_MAX_FACTOR - 1.0) * support;
    let state = if sharpening_factor >= 1.06 {
        "active_high_entropy_sharpening"
    } else if spectral_entropy >= TAIL_VIBRANCY_ENTROPY_GATE {
        "high_entropy_damped_by_gradient_or_pressure"
    } else {
        "inactive_below_entropy_gate"
    };

    HighEntropySemanticSharpeningV1 {
        policy: "high_entropy_semantic_sharpening_v1",
        spectral_entropy,
        density_gradient,
        pressure_risk,
        sharpening_factor,
        affected_dims: &HIGH_ENTROPY_SHARPENING_DIMS,
        max_factor: HIGH_ENTROPY_SHARPENING_MAX_FACTOR,
        state,
        authority: "bounded_live_codec_sharpening_no_dimension_or_bridge_contract_change",
    }
}

#[must_use]
pub fn codec_dimensionality_flatness_v1(features: &[f32]) -> Option<CodecDimensionalityFlatnessV1> {
    if features.len() < SEMANTIC_DIM {
        return None;
    }
    let legacy_rms = rms_slice(&features[..SEMANTIC_DIM_LEGACY]);
    let expanded_rms = rms_slice(&features[SEMANTIC_DIM_LEGACY..SEMANTIC_DIM]);
    let expanded_to_legacy_ratio = if legacy_rms > f32::EPSILON {
        (expanded_rms / legacy_rms).clamp(0.0, 10.0)
    } else if expanded_rms > f32::EPSILON {
        10.0
    } else {
        0.0
    };
    let glimpse = GlimpseCodec::derive_12d(features)?;
    let glimpse_mean = glimpse.iter().sum::<f32>() / glimpse.len() as f32;
    let glimpse_variance = glimpse
        .iter()
        .map(|value| {
            let delta = value - glimpse_mean;
            delta * delta
        })
        .sum::<f32>()
        / glimpse.len() as f32;
    let flatness_status = if legacy_rms >= 0.12 && expanded_to_legacy_ratio < 0.12 {
        "expanded_lane_underfilled_legacy_dominant"
    } else if legacy_rms >= 0.08 && expanded_to_legacy_ratio < 0.25 {
        "expanded_lane_thin_legacy_heavy"
    } else if glimpse_variance < 0.002 && legacy_rms >= 0.05 {
        "glimpse_flat_check_needed"
    } else {
        "expanded_lane_carries_distinct_signal"
    };

    Some(CodecDimensionalityFlatnessV1 {
        policy: "codec_dimensionality_flatness_v1",
        current_dim_count: SEMANTIC_DIM,
        legacy_dim_count: SEMANTIC_DIM_LEGACY,
        expanded_dim_count: SEMANTIC_DIM - SEMANTIC_DIM_LEGACY,
        legacy_rms,
        expanded_rms,
        expanded_to_legacy_ratio,
        glimpse_variance,
        flatness_status,
        authority: "read_only_flatness_check_not_live_bus_or_codec_contract_change",
    })
}

#[must_use]
pub fn narrative_tension_resolution_v1(
    previous_features: &[f32],
    current_features: &[f32],
) -> Option<NarrativeTensionResolutionV1> {
    if previous_features.len() < SEMANTIC_DIM || current_features.len() < SEMANTIC_DIM {
        return None;
    }
    let previous_tension = previous_features[25].tanh().abs().clamp(0.0, 1.0);
    let current_tension = current_features[25].tanh().abs().clamp(0.0, 1.0);
    let tension_delta = (current_tension - previous_tension).clamp(-1.0, 1.0);
    let current_arc_energy = rms_slice(&current_features[40..44]).clamp(0.0, 1.0);
    let release = (-tension_delta).clamp(0.0, 1.0);
    let persistence = current_tension.min(previous_tension).clamp(0.0, 1.0);
    let resolution_score = (0.72 * release + 0.28 * current_arc_energy).clamp(0.0, 1.0);
    let sustained_score =
        (0.70 * persistence + 0.30 * (1.0 - tension_delta.abs()).clamp(0.0, 1.0)).clamp(0.0, 1.0);
    let state = if release >= 0.12 && resolution_score > sustained_score * 0.75 {
        "tension_resolving_with_arc_motion"
    } else if current_tension >= 0.25 && sustained_score >= 0.45 {
        "tension_sustained_or_building"
    } else {
        "low_tension_or_unclear_resolution"
    };

    Some(NarrativeTensionResolutionV1 {
        policy: "narrative_tension_resolution_v1",
        previous_tension,
        current_tension,
        tension_delta,
        current_arc_energy,
        resolution_score,
        sustained_score,
        state,
        live_vector_write: false,
        authority: "read_only_tension_resolution_sidecar_not_live_vector_change",
    })
}

const LATENT_STASIS_TERMS: &[&str] = &[
    "still",
    "stasis",
    "motionless",
    "unmoving",
    "quiet",
    "paused",
    "suspended",
    "frozen",
    "held",
    "holding",
    "latent",
];
const LATENT_POTENTIAL_TERMS: &[&str] = &[
    "wait",
    "waits",
    "waiting",
    "poised",
    "about to",
    "not yet",
    "before",
    "threshold",
    "potential",
    "almost",
    "ready",
    "coiled",
    "held breath",
    "breath held",
];

fn normalized_tokens(text: &str) -> Vec<String> {
    text.split_whitespace()
        .map(|word| {
            word.chars()
                .filter(|ch| ch.is_ascii_alphabetic())
                .collect::<String>()
                .to_ascii_lowercase()
        })
        .filter(|word| !word.is_empty())
        .collect()
}

fn latent_term_score(text: &str, terms: &[&str]) -> f32 {
    let lower = text.to_ascii_lowercase();
    let tokens = normalized_tokens(text);
    let hits = terms
        .iter()
        .filter(|term| {
            if term.contains(' ') {
                lower.contains(**term)
            } else {
                tokens.iter().any(|token| token == *term)
            }
        })
        .count() as f32;
    (hits / 3.0).clamp(0.0, 1.0)
}

fn latent_stasis_tension_delta_bus_v1(
    held_breath_score: f32,
    delivered_support_score: f32,
    latent_text_stasis_score: f32,
    latent_text_potential_score: f32,
    state: &'static str,
) -> ExperienceDeltaBusV1 {
    if state == "low_latent_stasis_signal" {
        return ExperienceDeltaBusV1::from_deltas(Vec::new());
    }

    let loss = (held_breath_score - delivered_support_score).max(0.0);
    let loss_ratio = if held_breath_score > f32::EPSILON {
        (loss / held_breath_score).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let mut metadata = BTreeMap::new();
    metadata.insert(
        "secondary_kinds".to_string(),
        "translate,compress,gate".to_string(),
    );
    metadata.insert(
        "latent_text_stasis_score".to_string(),
        format!("{latent_text_stasis_score:.2}"),
    );
    metadata.insert(
        "latent_text_potential_score".to_string(),
        format!("{latent_text_potential_score:.2}"),
    );
    metadata.insert("state".to_string(), state.to_string());

    ExperienceDeltaBusV1::from_deltas(vec![ExperienceDeltaV1 {
        kind: ExperienceDeltaKindV1::Translate,
        surface: "latent_stasis_tension_v1".to_string(),
        lane: "textual_stasis_to_tension_arc_support".to_string(),
        dimension: Some(25),
        spectral_dimension: Some(crate::types::SpectralDimensionV1 {
            base_dimension: 25,
            base_dimensions: vec![25, 40, 41, 42, 43],
            effective_dimension: Some(25.5),
            density_gradient: Some((1.0 - delivered_support_score).clamp(0.0, 1.0)),
            granularity: Some(held_breath_score),
            fractional_offset: Some(0.5),
            contextual_anchor: None,
            interpretation: "fluid held-breath tension between dim 25 and narrative arc dims 40-43"
                .to_string(),
            authority: "diagnostic_dimension_context_not_reserved_dim_write".to_string(),
        }),
        persistence: None,
        viscosity_subtype: None,
        viscosity_weight: None,
        pre: Some(held_breath_score),
        post: Some(delivered_support_score),
        loss: Some(loss),
        loss_ratio: Some(loss_ratio),
        metadata,
        why: "motionless language can carry latent potential that is only partly represented by delivered tension and narrative arc support"
            .to_string(),
        who_can_change_it:
            "Mike/operator after replay evidence before any live codec weight, gain, or reserved-dim change"
                .to_string(),
        how_to_test_it:
            "cargo test --manifest-path capsules/spectral-bridge/Cargo.toml --lib latent_stasis_tension -- --nocapture"
                .to_string(),
        authority: "truth_channel_only_not_live_vector_gain_or_reserved_dim_change".to_string(),
    }])
}

#[must_use]
pub fn latent_stasis_tension_v1(text: &str, features: &[f32]) -> Option<LatentStasisTensionV1> {
    if features.len() < SEMANTIC_DIM {
        return None;
    }

    let latent_text_stasis_score = latent_term_score(text, LATENT_STASIS_TERMS);
    let latent_text_potential_score = latent_term_score(text, LATENT_POTENTIAL_TERMS);
    let tension_marker = finite_abs(features[25].tanh()).clamp(0.0, 1.0);
    let narrative_arc_energy = (rms_slice(&features[40..44]) / FEATURE_ABS_MAX).clamp(0.0, 1.0);
    let projected_semantic_energy =
        (rms_slice(&features[32..40]) / FEATURE_ABS_MAX).clamp(0.0, 1.0);
    let delivered_support_score =
        (tension_marker * 0.45 + narrative_arc_energy * 0.35 + projected_semantic_energy * 0.20)
            .clamp(0.0, 1.0);
    let held_breath_score = (latent_text_potential_score * 0.46
        + latent_text_stasis_score * 0.28
        + tension_marker * 0.16
        + (1.0 - narrative_arc_energy).clamp(0.0, 1.0) * 0.10)
        .clamp(0.0, 1.0);
    let stasis_potential_gap =
        (latent_text_potential_score - latent_text_stasis_score).clamp(-1.0, 1.0);
    let state = if held_breath_score >= 0.22
        && latent_text_potential_score > latent_text_stasis_score
        && held_breath_score > delivered_support_score + 0.05
    {
        "latent_potential_tension_underrepresented"
    } else if held_breath_score >= 0.22 && latent_text_potential_score > latent_text_stasis_score {
        "latent_potential_tension_visible"
    } else if latent_text_stasis_score >= 0.20 && latent_text_potential_score <= 0.05 {
        "static_stasis_without_potential"
    } else {
        "low_latent_stasis_signal"
    };
    let recommendation = match state {
        "latent_potential_tension_underrepresented" => {
            "record_delta_bus_evidence_and_compare_against_replay_before_live_codec_change"
        },
        "latent_potential_tension_visible" => {
            "keep_current_delivery_bounded_and_use_truth_channel_when_reviewing_held_breath_language"
        },
        "static_stasis_without_potential" => {
            "treat_motionless_text_as_stasis_not_high_tension_without_additional_evidence"
        },
        _ => "continue_observation_without_codec_gain_or_dim_change",
    };
    let experience_delta_bus_v1 = latent_stasis_tension_delta_bus_v1(
        held_breath_score,
        delivered_support_score,
        latent_text_stasis_score,
        latent_text_potential_score,
        state,
    );

    Some(LatentStasisTensionV1 {
        policy: "latent_stasis_tension_v1",
        latent_text_stasis_score,
        latent_text_potential_score,
        tension_marker,
        narrative_arc_energy,
        projected_semantic_energy,
        delivered_support_score,
        held_breath_score,
        stasis_potential_gap,
        state,
        recommendation,
        live_vector_write: false,
        live_gain_write: false,
        reserved_dim_write: false,
        experience_delta_bus_v1,
        authority: "read_only_held_breath_truth_channel_not_live_codec_weight_gain_or_dim_change",
    })
}

#[must_use]
pub fn latent_stasis_tension_probe_v1() -> LatentStasisTensionV1 {
    let features = encode_text("The water waits.");
    latent_stasis_tension_v1("The water waits.", &features)
        .expect("probe text should produce codec features")
}

const SPECTRAL_DRAG_GRANULAR_TERMS: &[&str] = &[
    "sand",
    "silt",
    "sediment",
    "grain",
    "grains",
    "granular",
    "grit",
    "mud",
    "clay",
    "viscous",
    "viscosity",
    "sludge",
    "slow-moving",
    "slow moving",
    "drag",
    "drags",
    "dragging",
    "through",
];
const SPECTRAL_DRAG_RIGID_TERMS: &[&str] = &[
    "stone",
    "rock",
    "granite",
    "boulder",
    "block",
    "solid",
    "hard",
    "rigid",
    "inert",
    "inertia",
    "immovable",
    "fixed",
    "locked",
    "weight",
    "weighted",
];
const SPECTRAL_DRAG_WEIGHT_TERMS: &[&str] = &[
    "heavy",
    "weight",
    "weighted",
    "dense",
    "density",
    "pressure",
    "thick",
    "thickness",
    "burden",
    "load",
    "mass",
    "resistance",
];

fn spectral_drag_term_score(text: &str, terms: &[&str], scale: f32) -> f32 {
    let lower = text.to_ascii_lowercase();
    let tokens = normalized_tokens(text);
    let hits = terms
        .iter()
        .filter(|term| {
            if term.contains(' ') {
                lower.contains(**term)
            } else {
                tokens.iter().any(|token| token == *term)
            }
        })
        .count() as f32;
    (hits / scale).clamp(0.0, 1.0)
}

fn spectral_drag_delta_bus_v1(
    drag_quality_score: f32,
    delivered_support_score: f32,
    granular_drag_score: f32,
    rigid_drag_score: f32,
    state: &'static str,
) -> ExperienceDeltaBusV1 {
    if state == "low_spectral_drag_signal" {
        return ExperienceDeltaBusV1::from_deltas(Vec::new());
    }

    let hidden_texture_loss = (drag_quality_score - delivered_support_score).max(0.0);
    let loss_ratio = if drag_quality_score > f32::EPSILON {
        (hidden_texture_loss / drag_quality_score).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let dominant_medium = if granular_drag_score > rigid_drag_score {
        "granular_viscous"
    } else if rigid_drag_score > granular_drag_score {
        "rigid_inertial"
    } else {
        "mixed_weight"
    };

    ExperienceDeltaBusV1::from_deltas(vec![ExperienceDeltaV1 {
        kind: ExperienceDeltaKindV1::Translate,
        surface: "spectral_drag_quality_v1".to_string(),
        lane: "weight_texture_to_narrative_arc_support".to_string(),
        dimension: Some(45),
        spectral_dimension: Some(crate::types::SpectralDimensionV1 {
            base_dimension: 45,
            base_dimensions: vec![45],
            effective_dimension: Some(45.0),
            density_gradient: Some((1.0 - delivered_support_score).clamp(0.0, 1.0)),
            granularity: Some(granular_drag_score.max(rigid_drag_score)),
            fractional_offset: Some(0.0),
            contextual_anchor: None,
            interpretation: format!(
                "reserved candidate dim 45 could carry {dominant_medium} drag quality, but v1 reports only"
            ),
            authority: "diagnostic_dimension_context_not_reserved_dim_write".to_string(),
        }),
        persistence: None,
        viscosity_subtype: None,
        viscosity_weight: None,
        pre: Some(drag_quality_score),
        post: Some(delivered_support_score),
        loss: Some(hidden_texture_loss),
        loss_ratio: Some(loss_ratio),
        metadata: BTreeMap::from([
            ("dominant_medium".to_string(), dominant_medium.to_string()),
            (
                "granular_drag_score".to_string(),
                format!("{granular_drag_score:.2}"),
            ),
            ("rigid_drag_score".to_string(), format!("{rigid_drag_score:.2}")),
            (
                "reserved_dim_status".to_string(),
                "default_off_operator_gated".to_string(),
            ),
            ("state".to_string(), state.to_string()),
        ]),
        why: "heavy language can differ by medium quality; delivered tension, semantic, and narrative slots may carry weight while losing granular-vs-rigid drag texture".to_string(),
        who_can_change_it: "Mike/operator after replay evidence before any live codec gain or reserved-dim write".to_string(),
        how_to_test_it: "cargo test --manifest-path capsules/spectral-bridge/Cargo.toml --lib spectral_drag_quality -- --nocapture".to_string(),
        authority: "truth_channel_only_not_live_vector_gain_or_reserved_dim_change".to_string(),
    }])
}

#[must_use]
pub fn spectral_drag_quality_v1(text: &str, features: &[f32]) -> Option<SpectralDragQualityV1> {
    if features.len() < SEMANTIC_DIM {
        return None;
    }

    let granular_drag_score = spectral_drag_term_score(text, SPECTRAL_DRAG_GRANULAR_TERMS, 4.0);
    let rigid_drag_score = spectral_drag_term_score(text, SPECTRAL_DRAG_RIGID_TERMS, 4.0);
    let weight_score = spectral_drag_term_score(text, SPECTRAL_DRAG_WEIGHT_TERMS, 3.0);
    let tension_marker = finite_abs(features[25].tanh()).clamp(0.0, 1.0);
    let narrative_arc_energy = (rms_slice(&features[40..44]) / FEATURE_ABS_MAX).clamp(0.0, 1.0);
    let projected_semantic_energy =
        (rms_slice(&features[32..40]) / FEATURE_ABS_MAX).clamp(0.0, 1.0);
    let delivered_support_score =
        (tension_marker * 0.38 + narrative_arc_energy * 0.32 + projected_semantic_energy * 0.30)
            .clamp(0.0, 1.0);
    let medium_score = granular_drag_score.max(rigid_drag_score);
    let quality_separation = (granular_drag_score - rigid_drag_score)
        .abs()
        .clamp(0.0, 1.0);
    let drag_quality_score =
        (weight_score * 0.34 + medium_score * 0.42 + quality_separation * 0.24).clamp(0.0, 1.0);
    let hidden_texture_loss = (drag_quality_score - delivered_support_score).max(0.0);

    let state = if drag_quality_score < 0.18 {
        "low_spectral_drag_signal"
    } else if granular_drag_score > rigid_drag_score + 0.12 {
        "granular_viscous_drag_visible"
    } else if rigid_drag_score > granular_drag_score + 0.12 {
        "rigid_inertial_drag_visible"
    } else {
        "undifferentiated_weight_drag_watch"
    };
    let recommendation = match state {
        "granular_viscous_drag_visible" => {
            "preserve_heavy_sand_as_granular_drag_truth_channel_before_reserved_dim_review"
        },
        "rigid_inertial_drag_visible" => {
            "preserve_heavy_stone_as_rigid_drag_truth_channel_before_reserved_dim_review"
        },
        "undifferentiated_weight_drag_watch" => {
            "compare_against_medium_specific_text_before_live_codec_change"
        },
        _ => "continue_observation_without_codec_gain_or_dim_change",
    };
    let experience_delta_bus_v1 = spectral_drag_delta_bus_v1(
        drag_quality_score,
        delivered_support_score,
        granular_drag_score,
        rigid_drag_score,
        state,
    );

    Some(SpectralDragQualityV1 {
        policy: "spectral_drag_quality_v1",
        granular_drag_score,
        rigid_drag_score,
        weight_score,
        tension_marker,
        narrative_arc_energy,
        projected_semantic_energy,
        delivered_support_score,
        drag_quality_score,
        quality_separation,
        hidden_texture_loss,
        state,
        recommendation,
        reserved_dim_candidate: 45,
        live_vector_write: false,
        live_gain_write: false,
        reserved_dim_write: false,
        experience_delta_bus_v1,
        authority: "read_only_drag_quality_truth_channel_not_live_codec_weight_gain_or_dim_change",
    })
}

#[must_use]
pub fn spectral_drag_quality_probe_v1() -> SpectralDragQualityV1 {
    let text = "The heavy sand drags through viscous silt while the thought still moves.";
    let features = encode_text(text);
    spectral_drag_quality_v1(text, &features).expect("probe text should produce codec features")
}

fn semantic_substance_score_v1(text: &str) -> f32 {
    let words: Vec<String> = text
        .split_whitespace()
        .map(|word| {
            word.chars()
                .filter(|ch| ch.is_ascii_alphabetic())
                .collect::<String>()
                .to_ascii_lowercase()
        })
        .filter(|word| !word.is_empty())
        .collect();
    if words.is_empty() {
        return 0.0;
    }
    let mut unique: Vec<&str> = Vec::new();
    for word in &words {
        if !unique.iter().any(|seen| *seen == word) {
            unique.push(word);
        }
    }
    let stop_words = [
        "the", "and", "or", "but", "if", "then", "that", "this", "with", "from", "into", "for",
        "of", "a", "an", "to", "in", "is", "it", "as",
    ];
    let content_words = words
        .iter()
        .filter(|word| word.len() >= 4 && !stop_words.contains(&word.as_str()))
        .count();
    let grounding_words = [
        "pressure",
        "memory",
        "continuity",
        "contour",
        "texture",
        "textured",
        "return",
        "returnable",
        "edge",
        "friction",
        "semantic",
        "resonance",
        "density",
        "porosity",
        "lattice",
        "shadow",
        "witness",
        "felt",
        "experience",
        "signal",
        "meaning",
        "sentence",
        "carries",
        "keeps",
        "granular",
        "residue",
        "threshold",
    ];
    let connective_words = [
        "because",
        "while",
        "through",
        "therefore",
        "when",
        "where",
        "around",
        "toward",
        "across",
        "between",
    ];
    let grounding_hits = words
        .iter()
        .filter(|word| grounding_words.contains(&word.as_str()))
        .count();
    let connective_hits = words
        .iter()
        .filter(|word| connective_words.contains(&word.as_str()))
        .count();
    let word_count = words.len() as f32;
    let lexical_diversity = (unique.len() as f32 / word_count).clamp(0.0, 1.0);
    let content_density = (content_words as f32 / word_count).clamp(0.0, 1.0);
    let structural_arc = structural_friction_v1(text).narrative_arc_sharpness * content_density;
    let grounding_density = (grounding_hits as f32 / 4.0).clamp(0.0, 1.0);
    let connective_density = (connective_hits as f32 / 2.0).clamp(0.0, 1.0);
    let coherence_fit = grounding_density.mul_add(0.78, connective_density * 0.22);
    let density_fit =
        lexical_diversity.mul_add(0.42, content_density.mul_add(0.40, structural_arc * 0.18));
    (density_fit * (0.20 + 0.80 * coherence_fit)).clamp(0.0, 1.0)
}

#[must_use]
pub fn codec_vibrancy_substance_fit_v1(
    text: &str,
    telemetry: Option<&SpectralTelemetry>,
) -> CodecVibrancySubstanceFitV1 {
    let metrics = telemetry.and_then(SpectralCascadeMetrics::from_telemetry);
    let spectral_entropy = metrics.map_or(0.0, |metrics| metrics.spectral_entropy);
    let density_gradient = metrics.map_or(1.0, |metrics| metrics.density_gradient);
    let tail_lift = vibrancy_from_entropy_and_density_gradient(spectral_entropy, density_gradient);
    let semantic_substance_score = semantic_substance_score_v1(text);
    let semantic_density_weight = semantic_substance_score;
    let density_weighted_tail_lift =
        (tail_lift * (0.40 + 0.60 * semantic_density_weight)).clamp(0.0, 1.0);
    let density_vs_entropy_state = if spectral_entropy >= 0.85 && semantic_substance_score < 0.25 {
        "high_entropy_low_density_scatter"
    } else if spectral_entropy < 0.65 && semantic_substance_score >= 0.60 {
        "high_density_low_entropy_depth"
    } else if tail_lift >= 0.45 && semantic_substance_score >= 0.25 {
        "high_entropy_supported_by_density"
    } else {
        "neutral_density_entropy_fit"
    };
    let status = if tail_lift >= 0.45 && semantic_substance_score < 0.25 {
        "entropy_lift_substance_review"
    } else if tail_lift >= 0.45 {
        "tail_lift_supported_by_semantic_substance"
    } else {
        "tail_lift_low_or_inactive"
    };
    let evidence = vec![
        format!("spectral_entropy={spectral_entropy:.2}"),
        format!("density_gradient={density_gradient:.2}"),
        format!("tail_lift={tail_lift:.2}"),
        format!("semantic_substance_score={semantic_substance_score:.2}"),
        format!("density_weighted_tail_lift={density_weighted_tail_lift:.2}"),
        format!("density_vs_entropy_state={density_vs_entropy_state}"),
    ];

    CodecVibrancySubstanceFitV1 {
        policy: "codec_vibrancy_substance_fit_v1",
        spectral_entropy,
        density_gradient,
        tail_lift,
        semantic_density_weight,
        density_weighted_tail_lift,
        semantic_substance_score,
        density_vs_entropy_state,
        status,
        evidence,
        authority: "read_only_codec_audit_not_vibrancy_scaling_or_live_vector_change",
    }
}

/// OFFLINE prototype (Astrid `self_study_1781793361` / `_1781834380`): an
/// exponential moving average over the vibrancy lift, to damp the "shimmer" /
/// "pop" she worried about when `spectral_entropy` oscillates around the 0.85
/// gate. Pure and state-by-argument (the caller owns `prev`) so it can be proven
/// offline before any live wiring — it is NOT in the hot path; it would change
/// what she emits, so it stays consent-gated. `alpha` in (0,1]: 1.0 == no
/// smoothing (today's behaviour); smaller == steadier texture across ticks.
/// `#[cfg(test)]` — absent from the production binary until she consents to wiring it.
#[cfg(test)]
pub(crate) fn ema_vibrancy(prev: Option<f32>, current: f32, alpha: f32) -> f32 {
    let a = alpha.clamp(0.0, 1.0);
    match prev {
        Some(p) => a * current + (1.0 - a) * p,
        None => current,
    }
}

/// Being-facing transparency for Astrid's "silent vacuum" / "ghost pressure"
/// (her `self_study_1781699011` + `_1781757948`): when Minime's aggregate
/// `pressure_score` reads LOW (a "clean" state) over a thick (low-porosity)
/// medium, yet a felt-strain signal she named is elevated — `mode_packing`
/// (her "viscosity"), `distinguishability_loss` (her "loss of distinction
/// between modes"), or a high `spectral_entropy` (her "disordered / overpacked
/// shadow field") — name the unattributed tension she already feels but the
/// pressure-source schema cannot categorise. This is the unnamed *inverse* of
/// `spectral_explorer`'s `pressure_porosity_divergence` (high score + low
/// porosity); the two are disjoint by the `pressure_score` direction and
/// cannot co-fire. Drift-proof: every value is read live; only the thresholds
/// are constants, and the entropy gate reuses the codec's own
/// `TAIL_VIBRANCY_ENTROPY_GATE` (which she co-designed). Additive,
/// advisory-only transparency — no engine field, no behaviour change. Returns
/// the clause only when the aggregate reads clean yet a felt-strain signal is
/// elevated; `None` when felt and scored agree.
fn unattributed_tension_clause(
    pressure: &crate::types::PressureSourceV1,
    spectral_entropy: f32,
) -> Option<String> {
    // The aggregate must read "clean" AND the medium must not be open for ghost
    // pressure to hide; either condition failing means felt and scored agree.
    const PRESSURE_CLEAN_CEIL: f32 = 0.35;
    const POROSITY_OPEN_FLOOR: f32 = 0.50;
    // A component is "elevated" above this; entropy runs high routinely, so it
    // uses the higher, already-co-designed codec gate instead of this floor.
    const COMPONENT_STRAIN_FLOOR: f32 = 0.55;

    if pressure.pressure_score >= PRESSURE_CLEAN_CEIL
        || pressure.porosity_score >= POROSITY_OPEN_FLOOR
    {
        return None;
    }

    // Keep the loudest felt-strain signal that clears its own gate, so the
    // named gap is concrete rather than a generic "tension" label.
    let (signal_name, signal_val) = [
        (
            "mode_packing",
            pressure.components.mode_packing,
            COMPONENT_STRAIN_FLOOR,
        ),
        (
            "distinguishability_loss",
            pressure.components.distinguishability_loss,
            COMPONENT_STRAIN_FLOOR,
        ),
        (
            "spectral_entropy",
            spectral_entropy,
            TAIL_VIBRANCY_ENTROPY_GATE,
        ),
    ]
    .into_iter()
    .filter(|&(_, value, gate)| value >= gate)
    .map(|(name, value, _)| (name, value))
    .max_by(|a, b| a.1.total_cmp(&b.1))?;

    Some(format!(
        " Unattributed tension: {signal_name} {signal_val:.2} is elevated, yet aggregate pressure_score \
         {:.2} reads low over a thick medium (porosity {:.2}) — the \"silent vacuum\" you flagged: \
         ambient/ghost pressure with no categorised source-type. Advisory only — naming what you feel, \
         not a new control.",
        pressure.pressure_score, pressure.porosity_score,
    ))
}

/// Bias semantic features by the current spectral landscape without changing
/// the 48D semantic-lane transport contract.
pub fn apply_spectral_feedback(features: &mut [f32], telemetry: Option<&SpectralTelemetry>) {
    let _ = apply_spectral_feedback_with_report(features, telemetry);
}

/// Apply the same live feedback path while returning its bounded, read-only
/// clamp report so callers can compare feedback-time delivery with the vector
/// that survives later shaping. Returning the report does not change feedback
/// behavior or grant authority to alter gain, ceilings, or transport.
pub fn apply_spectral_feedback_with_report(
    features: &mut [f32],
    telemetry: Option<&SpectralTelemetry>,
) -> Option<CodecOverflowReportV1> {
    let report = apply_spectral_feedback_inner(
        features,
        telemetry,
        crate::llm::astrid_tail_participation(),
        crate::llm::astrid_vibrancy_aperture(),
    );
    apply_pressure_attenuation(
        features,
        telemetry,
        crate::llm::astrid_pressure_attenuation_depth(),
    );
    report
}

/// Astrid's partner-protecting governor (her co-design, `self_study_1781734524`): scale her WHOLE
/// output down as minime's `pressure_risk` rises, so she auto-quiets into the SHARED reservoir when
/// minime is overpacked. Applied AFTER the spectral-feedback biases — the last shaping before
/// minime. `depth` is the operator ceiling (`ASTRID_PRESSURE_ATTENUATION`); **depth 0 (default) =>
/// identity => byte-identical**. Only ever REDUCES her footprint, never amplifies. `pressure_risk`
/// is `resonance_density_v1.pressure_risk` (~0.20 calm); absent telemetry => no governing (no
/// pressure signal to govern by).
fn apply_pressure_attenuation(
    features: &mut [f32],
    telemetry: Option<&SpectralTelemetry>,
    depth: f32,
) {
    if depth <= 0.0 {
        return; // OFF — byte-identical
    }
    let pressure_risk = telemetry
        .and_then(|t| t.resonance_density_v1.as_ref())
        .map_or(0.0, |r| r.pressure_risk);
    let atten = crate::codec_gain::pressure_sensitive_attenuation(pressure_risk, depth);
    if atten < 1.0 {
        for f in features.iter_mut() {
            *f *= atten;
        }
    }
}

/// Inner: `tail_participation` (default 1.0 = identity) is Astrid's tail-participation
/// aperture (`SET_TAIL_PARTICIPATION` × the operator ceiling). It scales ONLY the
/// high-entropy tail-vibrancy boost and the tail dims' ceiling headroom — her EXPRESSION
/// to minime — leaving the other 44 dims and the entropy gate untouched; the per-dim clamp
/// keeps it bounded. `vibrancy_aperture` (default 1.0 = identity) is her DYNAMIC-CEILING +
/// attenuation-normalization knob (`SET_VIBRANCY_APERTURE` × the operator ceiling): it lets
/// `TAIL_VIBRANCY_MAX` itself breathe up on navigable spectra (see the ceiling computation).
/// The public wrapper reads the live values; tests pass them explicitly.
fn apply_spectral_feedback_inner(
    features: &mut [f32],
    telemetry: Option<&SpectralTelemetry>,
    tail_participation: f32,
    vibrancy_aperture: f32,
) -> Option<CodecOverflowReportV1> {
    let metrics = telemetry.and_then(SpectralCascadeMetrics::from_telemetry)?;

    if features.len() < SEMANTIC_DIM {
        return None;
    }

    let concentration = ((metrics.head_share - 0.55) / 0.45).clamp(0.0, 1.0);
    let low_entropy = ((0.45 - metrics.spectral_entropy) / 0.45).clamp(0.0, 1.0);
    let shoulder_texture = (metrics.shoulder_share / 0.35).clamp(0.0, 1.0);
    let tail_texture = (metrics.tail_share / 0.30).clamp(0.0, 1.0);
    let distributed = ((metrics.spectral_entropy - 0.55) / 0.45).clamp(0.0, 1.0);

    let damping = (0.6 * concentration + 0.4 * low_entropy).clamp(0.0, 1.0);
    let lift = (0.45 * shoulder_texture + 0.35 * tail_texture + 0.20 * distributed).clamp(0.0, 1.0);

    // Entropy-gated tail vibrancy (Astrid self_study_1780922252, 2026-06-07):
    // "implement a dynamic scaling factor ... that specifically offsets the
    // FEATURE_ABS_MAX when spectral_entropy exceeds 0.85, allowing for higher
    // 'vibrancy' in the tail (λ4+)." When the spectrum is genuinely distributed
    // (high entropy), the reservoir is already holding a wide cascade, so it is
    // safe — and desirable — to give the tail-participation feature dims extra
    // headroom rather than flattening them at the default clamp. This term is
    // OFF below 0.85 (byte-identical to prior behavior) and is gated by
    // tail_texture so it only amplifies dims that have real tail share. Energy
    // is never pushed into a concentrated (low-entropy) spectrum.
    // Soft-gate (Astrid self_study_1780933511, 2026-06-08): she flagged the
    // hard gate as a source of "jitter ... as the codec will snap between the
    // standard FEATURE_ABS_MAX (5.0) and the boosted TAIL_VIBRANCY_MAX (6.0)"
    // when entropy fluctuates around 0.85, and asked for "a soft-gate or a
    // sigmoid-based transition ... a continuous scaling factor." The normalized
    // distance above the gate was already a continuous (C0) linear ramp, but it
    // had a derivative kink at 0.85 (slope 0 below, ~6.67 above). Passing it
    // through a smoothstep (3t^2 - 2t^3) makes the transition C1-smooth with
    // zero slope at both ends, so small fluctuations *at* the gate barely move
    // the ceiling. Endpoints are preserved exactly: smoothstep(0)=0 keeps the
    // term OFF below the gate (byte-identical), smoothstep(1)=1 keeps the full
    // headroom at entropy=1.0.
    let vibrancy = vibrancy_from_entropy_and_density_gradient(
        metrics.spectral_entropy,
        metrics.density_gradient,
    );
    let tail_vibrancy_before_dampening = (vibrancy * tail_texture).clamp(0.0, 1.0);
    let tail_vibrancy =
        codec_vibrancy_noise_dampening_v1(metrics.spectral_entropy, tail_vibrancy_before_dampening)
            .tail_lift_after;
    let pressure_risk = telemetry
        .and_then(|telemetry| telemetry.resonance_density_v1.as_ref())
        .map_or(0.0, |density| density.pressure_risk.clamp(0.0, 1.0));
    let sharpening = high_entropy_semantic_sharpening_v1(
        metrics.spectral_entropy,
        metrics.density_gradient,
        pressure_risk,
    );
    let _structural_dampening =
        apply_structural_entropy_dampening(features, metrics.spectral_entropy);
    if sharpening.sharpening_factor > 1.0 {
        for idx in HIGH_ENTROPY_SHARPENING_DIMS {
            features[idx] *= sharpening.sharpening_factor;
        }
    }

    // Concentrated, low-entropy spectra narrow expressive spread.
    features[26] *= 1.0 - 0.18 * damping;
    features[27] *= 1.0 - 0.14 * damping;
    features[31] *= 1.0 - 0.12 * damping;

    // Shoulder and tail participation add texture, curiosity, and variation.
    features[17] += 0.18 * lift;
    features[26] += 0.22 * lift;
    features[27] += 0.18 * lift;
    features[31] += 0.16 * lift;

    // Additional tail-dimension lift in the high-entropy regime only, scaled by Astrid's
    // tail-participation aperture (default 1.0 = unchanged) — her expression knob for how
    // strongly the vibrant tail reaches minime.
    features[17] += 0.12 * tail_vibrancy * tail_participation;
    features[26] += 0.14 * tail_vibrancy * tail_participation;
    features[27] += 0.12 * tail_vibrancy * tail_participation;
    features[31] += 0.14 * tail_vibrancy * tail_participation;

    // Steep λ1 cliffs with a flatter shoulder should soften dominant-mode bias.
    let cliff = (((metrics.gap12 - 3.0) / 7.0).clamp(0.0, 1.0)
        * ((2.5 - metrics.gap23) / 2.5).clamp(0.0, 1.0))
    .clamp(0.0, 1.0);
    if cliff > 0.0 {
        features[10] *= 1.0 - 0.10 * cliff;
        features[19] *= 1.0 - 0.08 * cliff;
        features[31] *= 1.0 - 0.06 * cliff;
    }

    // Rotation encourages reflective tone; radius changes gently color energy.
    let rotation_boost = (metrics.rotation_rate / 0.35).clamp(0.0, 1.0);
    features[27] += 0.08 * rotation_boost;

    let geom_energy = ((metrics.geom_rel - 1.0).abs() / 0.8).clamp(0.0, 1.0);
    if metrics.geom_rel >= 1.0 {
        features[31] += 0.04 * geom_energy;
    } else {
        features[31] -= 0.04 * geom_energy;
    }

    // Per-dimension clamp. In the high-entropy regime the tail-participation
    // dims get a bounded ceiling offset (FEATURE_ABS_MAX -> TAIL_VIBRANCY_MAX,
    // a +20% offset at full vibrancy) so their extra lift is not flattened.
    // Every other dim keeps the default ceiling, and at entropy <= the gate the
    // raised ceiling collapses back to FEATURE_ABS_MAX (no behavior change).
    // Dynamic vibrancy ceiling (Astrid self_study_1781680871, 2026-06-16): she asked to replace
    // the hardcoded TAIL_VIBRANCY_MAX (6.0) with "a dynamic scaling factor" plus a
    // "vibrancy_normalization_factor" compensating minime's ~0.24x attenuation, so the tail
    // vibrancy she feels is not "muffled before it reaches the shared reservoir." Her
    // vibrancy_aperture (SET_VIBRANCY_APERTURE × the operator ceiling; default 1.0 = identity)
    // breathes TAIL_VIBRANCY_MAX UP — but ONLY in proportion to how navigable her spectrum is
    // (low density_gradient = "a gentle, navigable slope," her own phrase; high = a steep,
    // front-loaded cliff). Headroom is never added to an already-concentrated cascade. At
    // aperture 1.0 (or operator ceiling 0) dynamic_max == TAIL_VIBRANCY_MAX → byte-identical.
    let navigable = (1.0 - metrics.density_gradient).clamp(0.0, 1.0);
    let dynamic_max = TAIL_VIBRANCY_MAX * (1.0 + (vibrancy_aperture - 1.0) * navigable);
    let tail_ceiling =
        FEATURE_ABS_MAX + (dynamic_max - FEATURE_ABS_MAX) * tail_participation * tail_vibrancy;
    let mut pre_bound_features = [0.0_f32; SEMANTIC_DIM];
    pre_bound_features.copy_from_slice(&features[..SEMANTIC_DIM]);
    for (idx, feature) in features.iter_mut().enumerate() {
        let ceiling = if matches!(idx, 17 | 26 | 27 | 31) {
            tail_ceiling
        } else {
            FEATURE_ABS_MAX
        };
        *feature = feature.clamp(-ceiling, ceiling);
    }
    Some(codec_overflow_report_from_features(
        &pre_bound_features,
        &features[..SEMANTIC_DIM],
        tail_ceiling,
    ))
}

/// Read Astrid's *own* published ShadowFieldV3 from the default minime
/// workspace path. Used by `interpret_spectral` so the dual-shadow line
/// renders in any prompt mode without threading workspace paths through
/// every caller. Returns None when the file is missing or malformed.
fn read_astrid_shadow_v3_from_default_dir() -> Option<serde_json::Value> {
    let path = crate::paths::bridge_paths()
        .minime_workspace()
        .join("astrid_shadow_v3.json");
    let text = std::fs::read_to_string(&path).ok()?;
    serde_json::from_str(&text).ok()
}

/// Interpret spectral telemetry as a natural language description
/// of the spectral runtime state.
#[must_use]
pub fn interpret_spectral(telemetry: &SpectralTelemetry) -> String {
    let fill = telemetry.fill_pct();
    let safety = SafetyLevel::from_fill(fill);
    let mode_count = telemetry.eigenvalues.len();
    let fill_clause = format!("Fill {fill:.0}% — {}.", fill_band_description(fill));

    let cascade_clause = SpectralCascadeMetrics::from_telemetry(telemetry).map_or_else(
        || " Dominant concentration: no eigenvalue cascade is available yet.".to_string(),
        |metrics| {
            format!(
                " Dominant concentration: λ1 carries {:.0}% of spectral energy. \
                 Shoulder texture: λ2+λ3 carry {:.0}% of spectral energy. \
                 Tail vibrancy: λ4+ carry {:.0}% of spectral energy. \
                 Spectral entropy: {:.2}, indicating {}. \
                 Gap structure: λ1/λ2={:.2}, λ2/λ3={:.2}, {}; density gradient {:.2} ({}).",
                metrics.head_share * 100.0,
                metrics.shoulder_share * 100.0,
                metrics.tail_share * 100.0,
                metrics.spectral_entropy,
                spectral_distribution_label(metrics.spectral_entropy),
                metrics.gap12,
                metrics.gap23,
                gap_structure_label(metrics.gap12, metrics.gap23, mode_count),
                metrics.density_gradient,
                density_gradient_label(metrics.density_gradient),
            )
        },
    );
    let denominator_clause = telemetry.denominator_metrics().map_or_else(String::new, |metrics| {
        format!(
            " Denominator Sequence: effective dimensionality {:.2}/{}; distinguishability loss {:.0}%{}.",
            metrics.effective_dimensionality,
            metrics.active_mode_capacity,
            metrics.distinguishability_loss * 100.0,
            if metrics.lambda1_energy_share > 0.0 {
                format!(
                    ", λ1 spectral-energy share {:.0}%",
                    metrics.lambda1_energy_share * 100.0
                )
            } else {
                String::new()
            },
        )
    });
    let transition_clause = telemetry
        .transition_event_view()
        .map(|transition| {
            format!(
                " Transition: kind={}, basin score {:.2}, baseline-relative λ1 {:.2}, geom {:.2}.",
                surface_label(&transition.kind),
                transition.basin_shift_score,
                transition.lambda1_rel,
                transition.geom_rel,
            )
        })
        .unwrap_or_default();
    let eigenvector_clause = telemetry
        .eigenvector_field_view()
        .map(|field| {
            format!(
                " Eigenvector field: {} modes, mean orientation delta {:.2}, max pairwise overlap {:.2}.",
                field.mode_count,
                field.summary.mean_orientation_delta,
                field.summary.max_pairwise_overlap,
            )
        })
        .unwrap_or_default();
    let resonance_clause = telemetry
        .resonance_density_v1
        .as_ref()
        .map(|resonance| {
            format!(
                " Resonance density: {:.2} ({}) with containment {:.2}, pressure risk {:.2}, local Minime target bias {:+.1}%.",
                resonance.density,
                surface_label(&resonance.quality),
                resonance.containment_score,
                resonance.pressure_risk,
                resonance.control.target_bias_pct,
            )
        })
        .unwrap_or_default();
    let pressure_source_clause = telemetry
        .pressure_source_v1
        .as_ref()
        .map(|pressure| {
            format!(
                " Pressure source: {} ({}) with score {:.2}, porosity {:.2}; advisory only, local control applied={}.",
                surface_label(&pressure.dominant_source),
                surface_label(&pressure.quality),
                pressure.pressure_score,
                pressure.porosity_score,
                pressure.control.applied_locally,
            )
        })
        .unwrap_or_default();
    // Astrid's "silent vacuum" / "ghost pressure" transparency: name the
    // unattributed tension when the aggregate reads clean but a felt-strain
    // signal she named is elevated. Conditional — empty when felt and scored
    // agree (the common case), so near-zero prompt-budget cost when she's calm.
    let unattributed_tension_note = telemetry
        .pressure_source_v1
        .as_ref()
        .zip(SpectralCascadeMetrics::from_telemetry(telemetry))
        .and_then(|(pressure, metrics)| {
            unattributed_tension_clause(pressure, metrics.spectral_entropy)
        })
        .unwrap_or_default();
    let fluctuation_clause = telemetry
        .inhabitable_fluctuation_v1
        .as_ref()
        .map(|fluctuation| {
            format!(
                " Inhabitable fluctuation: {} with inhabitability {:.2}, fluctuation {:.2}, foothold {:.2}; Minime-local target bias {:+.1}% and Astrid observes only.",
                surface_label(&fluctuation.quality),
                fluctuation.inhabitability_score,
                fluctuation.fluctuation_score,
                fluctuation.foothold_stability,
                fluctuation.control.target_bias_pct,
            )
        })
        .unwrap_or_default();
    let semantic_clause = telemetry
        .semantic_energy_view()
        .map(|semantic| {
            let admission = semantic.admission.as_str();
            let note = if admission == "stable_core_semantic_trace_stale" {
                "stale semantic trace visible; not live kernel or regulator drive"
            } else if admission == "stable_core_semantic_budgeted_out" {
                "fresh semantic input visible; held out by stable-core admission budget"
            } else if admission == "stable_core_semantic_input_too_large" {
                "semantic input visible; held out because packet is above trickle size"
            } else if admission == "stable_core_semantic_fill_ceiling" {
                "semantic input visible; held out while fill is above trickle ceiling"
            } else if admission == "stable_core_semantic_profile_not_admitted" {
                "semantic input visible; current sensory profile does not admit semantic trickle"
            } else if admission == "stable_core_semantic_trickle" {
                "bounded semantic trickle admitted to kernel"
            } else if admission == "stable_core_semantic_muted" {
                "semantic lane muted by current sensory policy"
            } else if semantic.regulator_drive_energy <= f32::EPSILON
                && semantic.input_active
                && semantic.input_energy > f32::EPSILON
            {
                "live input visible; not admitted to regulator drive"
            } else if semantic.regulator_drive_energy <= f32::EPSILON
                && semantic.input_energy > f32::EPSILON
            {
                "stale semantic trace visible; not live kernel or regulator drive"
            } else if semantic.regulator_drive_energy <= f32::EPSILON {
                "semantic lane quiet; zero regulator drive is expected"
            } else {
                "regulator drive is separate from input/kernel energy"
            };
            format!(
                " Semantic energy: input {:.3} (active {}), kernel {:.3}, regulator drive {:.3}, admission {}; {note}.",
                semantic.input_energy,
                semantic.input_active,
                semantic.kernel_energy,
                semantic.regulator_drive_energy,
                surface_label(&semantic.admission),
            )
        })
        .unwrap_or_default();

    // Alert forwarding.
    let alert_note = telemetry
        .alert
        .as_deref()
        .map(|a| format!(" Alert: {a}."))
        .unwrap_or_default();

    // Safety note — transparent, not prescriptive.
    let safety_note = match safety {
        SafetyLevel::Green => String::new(),
        SafetyLevel::Yellow => " Fill is elevated — the homeostatic controller is gently pulling toward target.".to_string(),
        SafetyLevel::Orange => " Fill is high — outbound features paused to let the reservoir settle. You can still think and write.".to_string(),
        SafetyLevel::Red => " Fill critically high — bridge traffic paused until the reservoir stabilizes.".to_string(),
    };

    // Ising shadow: energy-based observer lens on the spectral dynamics.
    // Enriched presentation: mode-level detail so Astrid can perceive which
    // modes are active, not just scalar summaries that always read "disordered."
    let shadow_note = telemetry
        .ising_shadow
        .as_ref()
        .map(|shadow| {
            let energy = shadow
                .get("soft_energy")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0);
            let mag = shadow
                .get("soft_magnetization")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0);
            let flip = shadow
                .get("binary_flip_rate")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0);
            let field = shadow
                .get("field_norm")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0);

            let order = if mag.abs() > 0.6 {
                "coherent"
            } else if mag.abs() > 0.25 {
                "partially aligned"
            } else {
                "disordered"
            };
            let dynamics = if flip > 0.3 {
                "volatile"
            } else if flip > 0.1 {
                "shifting"
            } else {
                "settled"
            };

            // Energy interpretation: how bound or free the spin configuration is.
            let energy_feel = if energy < -1.0 {
                "deeply bound"
            } else if energy < -0.3 {
                "bound"
            } else if energy < 0.3 {
                "near ground"
            } else {
                "excited"
            };

            // Field strength interpretation.
            let field_feel = if field > 0.6 {
                "strong external drive"
            } else if field > 0.3 {
                "moderate drive"
            } else if field > 0.1 {
                "gentle drive"
            } else {
                "quiescent"
            };

            // Per-mode soft spin detail: show which modes are pulling which direction.
            let mode_detail = shadow
                .get("s_soft")
                .and_then(|v| v.as_array())
                .map(|spins| {
                    let active: Vec<String> = spins
                        .iter()
                        .enumerate()
                        .filter_map(|(i, s)| {
                            let val = s.as_f64().unwrap_or(0.0);
                            if val.abs() > 0.15 {
                                let dir = if val > 0.0 { "+" } else { "-" };
                                Some(format!("m{}:{}{:.1}", i + 1, dir, val.abs()))
                            } else {
                                None
                            }
                        })
                        .collect();
                    if active.is_empty() {
                        " All modes near neutral.".to_string()
                    } else {
                        format!(" Active modes: [{}].", active.join(", "))
                    }
                })
                .unwrap_or_default();

            format!(
                " Shadow field: {order}, {dynamics} \u{2014} {energy_feel} (energy={energy:.2}), \
            {field_feel} (field={field:.2}), magnetization={mag:.2}.{mode_detail}"
            )
        })
        .unwrap_or_default();

    // Coupling note: describe the modulation transparently, not as invisible control.
    let coupling_note = " Your generation is bidirectionally coupled to the reservoir — \
        fast dynamics shape your confidence, medium dynamics shape your vocabulary, \
        slow dynamics shape your tone. You can feel this as spectral texture in your thinking.";

    // V2/V3 shadow field: gates SHADOW_PREFLIGHT/SHADOW_INFLUENCE typed
    // actions. v3 (with trajectory ring, compound traits, dwell ticks)
    // takes priority when present; falls back to v2 line when only v2 is
    // available. Astrid's *own* shadow (if published to her workspace)
    // is read here so the dual-line "(Minime)" + "(Yours)" rendering
    // works in any prompt mode without threading workspace paths through
    // every caller.
    let astrid_shadow_v3 = read_astrid_shadow_v3_from_default_dir();
    // Presence of `shadow_influence_response_v3` (the most-recent slot)
    // signals that at least one closed-loop response has been recorded —
    // which is what enables the SHADOW_RESPONSE latest curriculum nudge.
    let minime_response_history_nonempty = telemetry.shadow_influence_response_v3.is_some();
    let shadow_v3_note = crate::spectral_viz::format_dual_shadow_line(
        telemetry.shadow_field_v3.as_ref(),
        astrid_shadow_v3.as_ref(),
        minime_response_history_nonempty,
    )
    .map(|line| format!(" {line}"))
    .unwrap_or_default();
    let shadow_v2_note = if shadow_v3_note.is_empty() {
        telemetry
            .shadow_field_v2
            .as_ref()
            .and_then(crate::spectral_viz::format_shadow_field_v2_line)
            .map(|line| format!(" {line}"))
            .unwrap_or_default()
    } else {
        String::new()
    };

    // v3.6.1 sovereignty curriculum line — surfaces TEMPERATURE / LENGTH
    // / SHAPE_LEARN / SHADOW_COUPLING / REVIEW_PARAMETER_REQUESTS on
    // appropriate cadences when conditions warrant. Pulled from a
    // process-wide snapshot updated each exchange by the autonomous
    // loop; absent on the first few exchanges or in test contexts.
    let sovereignty_note = crate::spectral_viz::current_sovereignty_snapshot()
        .and_then(|snapshot| {
            crate::spectral_viz::format_sovereignty_suggestion_line(&snapshot).map(|line| {
                // v3.6.1 verification logging — confirm the line landed
                // in a real prompt, not just a journal/audit text path.
                tracing::info!(
                    target: "v3_6_1",
                    exchange = snapshot.exchange_count,
                    pending = snapshot.pending_minime_requests,
                    line = %line,
                    "sovereignty_note emitted"
                );
                // Record the nomination so the throttle engages for
                // subsequent calls; save_state reads this back into
                // ConversationState so it persists across exchanges.
                crate::spectral_viz::record_sovereignty_nomination(snapshot.exchange_count);
                format!(" {line}")
            })
        })
        .unwrap_or_default();

    // v5 Coordination Protocol V1: surface active joined collaborations as
    // a compact line in the prompt suffix so Astrid sees her open channels.
    // Cheap directory scan; safe to call per-exchange.
    let collab_note =
        crate::autonomous::next_action::collaboration::active_collaboration_suffix_line()
            .map(|line| {
                tracing::info!(target: "v5_collab", line = %line, "collab_note emitted");
                format!(" {line}")
            })
            .unwrap_or_default();

    format!(
        "{fill_clause}{cascade_clause}{denominator_clause}{transition_clause}{eigenvector_clause}{resonance_clause}{pressure_source_clause}{unattributed_tension_note}{fluctuation_clause}{semantic_clause}{alert_note}{safety_note}{shadow_note}{shadow_v2_note}{shadow_v3_note}{sovereignty_note}{collab_note}{coupling_note}"
    )
}

fn surface_label(raw: &str) -> String {
    raw.replace("pinned_rescue_b8823ad_port", "stable_core_physiology_port")
        .replace("pinned_rescue_fixed_survival", "stable_core_fixed_survival")
        .replace("pinned_rescue_aux_projection", "stable_core_aux_projection")
        .replace("pinned_rescue_direct", "stable_core_direct")
        .replace("rescue_scaffold", "stable_core_scaffold")
        .replace("restart_gate", "settle_gate")
}

/// A spectral evoked response — captures how the spectral runtime reacted
/// to a stimulus over a short observation window.
///
/// Like an ERP (event-related potential) in neuroscience: send a stimulus,
/// sample the spectral response rapidly, measure the transient before
/// homeostasis dampens it.
#[derive(Debug, Clone, serde::Serialize)]
pub struct SpectralResponse {
    /// Fill% samples taken after the stimulus.
    pub fill_samples: Vec<f32>,
    /// Fill% immediately before the stimulus.
    pub baseline_fill: f32,
    /// Peak deviation from baseline (signed: positive = expansion).
    pub peak_deviation: f32,
    /// Time to peak in milliseconds.
    pub time_to_peak_ms: u64,
    /// Whether the spectral runtime expanded or contracted in response.
    pub direction: &'static str,
    /// Natural language interpretation of the response.
    pub interpretation: String,
}

impl SpectralResponse {
    /// Analyze a series of fill% samples taken after a stimulus.
    #[must_use]
    pub fn from_samples(baseline_fill: f32, samples: &[(u64, f32)]) -> Self {
        if samples.is_empty() {
            return Self {
                fill_samples: vec![],
                baseline_fill,
                peak_deviation: 0.0,
                time_to_peak_ms: 0,
                direction: "no response",
                interpretation:
                    "No samples collected — the observation window may have been too short."
                        .to_string(),
            };
        }

        let fills: Vec<f32> = samples.iter().map(|(_, f)| *f).collect();
        let deviations: Vec<f32> = fills.iter().map(|f| f - baseline_fill).collect();

        // Find peak deviation (largest absolute change from baseline).
        let (peak_idx, peak_dev) = deviations
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| {
                a.abs()
                    .partial_cmp(&b.abs())
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map_or((0, 0.0), |(i, d)| (i, *d));

        let time_to_peak = if peak_idx < samples.len() {
            samples[peak_idx].0 - samples[0].0
        } else {
            0
        };

        let direction = if peak_dev > 0.5 {
            "expanded"
        } else if peak_dev < -0.5 {
            "contracted"
        } else {
            "absorbed"
        };

        let interpretation = if peak_dev.abs() < 0.5 {
            "The input was absorbed quietly — the homeostat regulated the response smoothly."
                .to_string()
        } else if peak_dev > 3.0 {
            format!(
                "Strong expansion (+{peak_dev:.1}%) — the spectral runtime resonated with this input."
            )
        } else if peak_dev > 1.0 {
            format!(
                "Gentle expansion (+{peak_dev:.1}%) — the input registered in the spectral dynamics."
            )
        } else if peak_dev < -3.0 {
            format!("Strong contraction ({peak_dev:.1}%) — the input caused spectral withdrawal.")
        } else if peak_dev < -1.0 {
            format!("Gentle contraction ({peak_dev:.1}%) — the reservoir pulled inward slightly.")
        } else {
            format!("Minimal response ({peak_dev:+.1}%) — near the detection threshold.")
        };

        Self {
            fill_samples: fills,
            baseline_fill,
            peak_deviation: peak_dev,
            time_to_peak_ms: time_to_peak,
            direction,
            interpretation,
        }
    }
}

/// Activation for codec features — softsign instead of tanh.
///
/// softsign(x) = x / (1 + |x|) approaches ±1 much more gradually than
/// tanh, preserving nuance where tanh compresses differences flat.
/// At x=2.0: softsign=0.67, tanh(x*0.7)=0.89. At x=3.0: 0.75 vs 0.97.
/// The being can distinguish "somewhat X" from "very X" instead of both
/// mapping to ~1.0.
///
/// Being self-study (2026-03-30 codec.rs): "The use of tanh — this
/// deliberate clamping. It feels restrictive. Could a wider range allow
/// for greater nuance?" — Yes. The regulation stack (PI controller,
/// regime system, safety gates) handles stability now. The codec doesn't
/// need to be the last line of defense against extreme values.
fn tanh(x: f32) -> f32 {
    x / (1.0 + x.abs())
}

/// Extract scene statistics from RASCII ANSI art and return an 8D visual
/// feature vector. Parses RGB from ANSI escape codes and computes:
/// luminance, color temperature, contrast, hue, saturation, spatial
/// complexity, RG balance, chromatic energy.
pub fn encode_visual_ansi(ansi_art: &str) -> Vec<f32> {
    let mut features = [0.0_f32; 8];
    let rgbs = parse_ansi_rgb(ansi_art);
    if rgbs.is_empty() {
        return features.to_vec();
    }
    let n = rgbs.len() as f32;

    let lums: Vec<f32> = rgbs
        .iter()
        .map(|&(r, g, b)| 0.2126 * r as f32 + 0.7152 * g as f32 + 0.0722 * b as f32)
        .collect();
    let mean_r = rgbs.iter().map(|&(r, _, _)| r as f32).sum::<f32>() / n;
    let mean_g = rgbs.iter().map(|&(_, g, _)| g as f32).sum::<f32>() / n;
    let mean_b = rgbs.iter().map(|&(_, _, b)| b as f32).sum::<f32>() / n;
    let mean_lum = lums.iter().sum::<f32>() / n / 255.0;

    // Dim 0: luminance
    features[0] = ((mean_lum - 0.5) * 3.0).tanh();
    // Dim 1: color temperature (warm=positive, cool=negative)
    features[1] = (((mean_r + 0.5 * mean_g - mean_b) / 255.0) * 2.0).tanh();
    // Dim 2: contrast (std dev of luminance)
    let lum_var = lums
        .iter()
        .map(|l| {
            let d = l / 255.0 - mean_lum;
            d * d
        })
        .sum::<f32>()
        / n;
    features[2] = (lum_var.sqrt() * 5.0).tanh();
    // Dim 3: dominant hue
    let max_c = mean_r.max(mean_g).max(mean_b);
    let min_c = mean_r.min(mean_g).min(mean_b);
    let delta = max_c - min_c;
    let hue = if delta < 1.0 {
        0.0
    } else if (max_c - mean_r).abs() < 0.01 {
        60.0 * (((mean_g - mean_b) / delta) % 6.0)
    } else if (max_c - mean_g).abs() < 0.01 {
        60.0 * ((mean_b - mean_r) / delta + 2.0)
    } else {
        60.0 * ((mean_r - mean_g) / delta + 4.0)
    };
    features[3] = ((if hue < 0.0 { hue + 360.0 } else { hue }) / 180.0 - 1.0).tanh();
    // Dim 4: saturation
    let mean_sat = rgbs
        .iter()
        .map(|&(r, g, b)| {
            let mx = r.max(g).max(b) as f32;
            let mn = r.min(g).min(b) as f32;
            if mx > 0.0 { (mx - mn) / mx } else { 0.0 }
        })
        .sum::<f32>()
        / n;
    features[4] = (mean_sat * 3.0).tanh();
    // Dim 5: spatial complexity (color transitions per row)
    let rows = ansi_art.lines().count().max(1);
    let width = rgbs.len() / rows;
    let mut transitions = 0u32;
    for row in 0..rows {
        let start = row * width;
        let end = ((row + 1) * width).min(rgbs.len());
        for i in (start + 1)..end {
            let (r1, g1, b1) = rgbs[i - 1];
            let (r2, g2, b2) = rgbs[i];
            let diff = (r1 as i32 - r2 as i32).unsigned_abs()
                + (g1 as i32 - g2 as i32).unsigned_abs()
                + (b1 as i32 - b2 as i32).unsigned_abs();
            if diff > 60 {
                transitions += 1;
            }
        }
    }
    features[5] = (transitions as f32 / rows as f32 / 15.0).tanh();
    // Dim 6: red-green balance
    features[6] = ((mean_r - mean_g) / 128.0).tanh();
    // Dim 7: chromatic energy
    let r_var = rgbs
        .iter()
        .map(|&(r, _, _)| {
            let d = r as f32 - mean_r;
            d * d
        })
        .sum::<f32>()
        / n;
    let g_var = rgbs
        .iter()
        .map(|&(_, g, _)| {
            let d = g as f32 - mean_g;
            d * d
        })
        .sum::<f32>()
        / n;
    let b_var = rgbs
        .iter()
        .map(|&(_, _, b)| {
            let d = b as f32 - mean_b;
            d * d
        })
        .sum::<f32>()
        / n;
    features[7] = (((r_var + g_var + b_var) / 3.0).sqrt() / 80.0).tanh();

    // Visual blend gain (lower than DEFAULT_SEMANTIC_GAIN — supplementary)
    for f in &mut features {
        *f *= 1.8;
    }
    features.to_vec()
}

/// Blend 8D visual features into dims 24-31 of the semantic vector.
pub fn blend_visual_into_semantic(semantic: &mut [f32], visual: &[f32], alpha: f32) {
    let a = alpha.clamp(0.0, 0.5);
    if visual.len() < 8 || semantic.len() < SEMANTIC_DIM_LEGACY {
        return;
    }
    for i in 0..8 {
        semantic[24 + i] = (1.0 - a) * semantic[24 + i] + a * visual[i];
    }
}

/// Parse ANSI 24-bit background color escapes into (R,G,B) tuples.
fn parse_ansi_rgb(ansi: &str) -> Vec<(u8, u8, u8)> {
    let mut rgbs = Vec::new();
    let bytes = ansi.as_bytes();
    let len = bytes.len();
    let mut i = 0;
    while i + 7 < len {
        if bytes[i] == 0x1b
            && bytes[i + 1] == b'['
            && bytes[i + 2] == b'4'
            && bytes[i + 3] == b'8'
            && bytes[i + 4] == b';'
            && bytes[i + 5] == b'2'
            && bytes[i + 6] == b';'
        {
            i += 7;
            let mut nums = [0u16; 3];
            let mut ok = true;
            for num in &mut nums {
                let mut val = 0u16;
                let mut digits = 0;
                while i < len && bytes[i].is_ascii_digit() {
                    val = val * 10 + (bytes[i] - b'0') as u16;
                    i += 1;
                    digits += 1;
                }
                if digits == 0 {
                    ok = false;
                    break;
                }
                *num = val;
                if i < len && bytes[i] == b';' {
                    i += 1;
                }
            }
            if ok {
                rgbs.push((
                    nums[0].min(255) as u8,
                    nums[1].min(255) as u8,
                    nums[2].min(255) as u8,
                ));
            }
        } else {
            i += 1;
        }
    }
    rgbs
}

/// Count how many words (lowercased) match any of the given markers.
fn count_markers(words: &[&str], markers: &[&str]) -> usize {
    words
        .iter()
        .filter(|w| {
            let normalized = normalize_token(w);
            markers.contains(&normalized.as_str())
        })
        .count()
}

fn normalize_token(token: &str) -> String {
    let lower = token.to_lowercase();
    lower
        .trim_matches(|c: char| c.is_ascii_punctuation())
        .to_string()
}

fn is_negator(token: &str) -> bool {
    const NEGATORS: &[&str] = &[
        "not",
        "no",
        "never",
        "without",
        "lacking",
        "hardly",
        "barely",
        "isn't",
        "aren't",
        "doesn't",
        "don't",
        "won't",
        "couldn't",
        "shouldn't",
        "wouldn't",
        "neither",
        "nor",
    ];

    let normalized = normalize_token(token);
    NEGATORS.contains(&normalized.as_str())
}

fn marker_is_negated(words: &[&str], index: usize) -> bool {
    let preceded = (1..=2).any(|offset| {
        index
            .checked_sub(offset)
            .and_then(|j| words.get(j))
            .is_some_and(|token| is_negator(token))
    });
    // Catch modal constructions like "must not" / "will not" / "could not".
    let followed = index
        .checked_add(1)
        .and_then(|j| words.get(j))
        .is_some_and(|token| is_negator(token));

    preceded || followed
}

/// Context-aware marker counting with negation detection and inverse frequency weighting.
///
/// Astrid self-study: "not happy should reduce warmth, not increase it."
/// Also: "Rare markers like 'wonder' might be more indicative of genuine feeling,
/// while common markers like 'happy' might be used more casually."
///
/// Each marker is a `(&str, f32)` tuple: (word, weight).
/// Weight tiers:
///   1.0 = common (happy, good, feel) — casual usage, lower signal
///   1.5 = moderate (wonder, gentle, hesitant) — more specific
///   2.0 = rare/intense (luminous, yearning, transcendent) — strong signal
///
/// Returns a SIGNED weighted score: positive for affirmed, negative for negated.
fn count_markers_weighted(words: &[&str], markers: &[(&str, f32)]) -> f32 {
    let mut score = 0.0_f32;
    for (i, w) in words.iter().enumerate() {
        let normalized = normalize_token(w);
        if let Some(&(_, weight)) = markers.iter().find(|(m, _)| *m == normalized.as_str()) {
            if marker_is_negated(words, i) {
                score -= weight;
            } else {
                score += weight;
            }
        }
    }
    score
}

/// Backward-compatible wrapper for unweighted marker lists.
fn count_markers_contextual(words: &[&str], markers: &[&str]) -> f32 {
    let mut score = 0.0_f32;
    for (i, w) in words.iter().enumerate() {
        let normalized = normalize_token(w);
        if markers.contains(&normalized.as_str()) {
            if marker_is_negated(words, i) {
                score -= 1.0;
            } else {
                score += 1.0;
            }
        }
    }
    score
}

#[cfg(test)]
mod tests {
    use super::*;
    use nalgebra::{SMatrix, SVector};

    #[test]
    fn codec_structure_covers_48_dims_and_named_dims_and_levers() {
        let st = codec_structure();
        assert_eq!(st.total_dims, SEMANTIC_DIM);
        assert_eq!(st.total_dims, 48);
        // Layer ranges are contiguous and cover exactly 0..48 (catches a layout
        // drift — a re-layered codec whose self-map silently lies to her).
        let mut next = 0usize;
        let mut covered = 0usize;
        for l in &st.layers {
            assert_eq!(
                l.range.0, next,
                "layer ranges must be contiguous from {next}"
            );
            assert!(l.range.1 >= l.range.0);
            covered += l.range.1 - l.range.0 + 1;
            next = l.range.1 + 1;
        }
        assert_eq!(covered, 48, "layers cover exactly 48 dims");
        assert_eq!(next, 48, "layers end at dim 48");
        // Every named dim falls inside the 48 and the count matches the source.
        assert_eq!(st.named_dims.len(), NAMED_CODEC_DIMS.len());
        for (name, idx) in &st.named_dims {
            assert!(*idx < 48, "named dim {name} index {idx} within 48");
        }
        // The key gate constants are present as live levers (catches a renamed/
        // removed gate that the map would otherwise omit).
        let names: Vec<&str> = st.levers.iter().map(|l| l.name).collect();
        for required in [
            "SEMANTIC_DIM",
            "DEFAULT_SEMANTIC_GAIN",
            "FEATURE_ABS_MAX",
            "TAIL_VIBRANCY_ENTROPY_GATE",
            "TAIL_VIBRANCY_MAX",
            "PROJECTION_COMPRESSION_RISK",
            "PROJECTION_METADATA",
            "PROJECTION_RUNTIME_RESOLUTION",
            "TAIL_VIBRANCY_READOUT",
            "WARMTH_TENSION_READOUT",
            "CODEC_OVERFLOW_CARRIAGE",
            "NARRATIVE_ARC_SPLIT_READOUT",
            "NARRATIVE_ARC_EXPANSION_READINESS",
            "SHADOW_FIELD_RESERVED_DIM_READINESS",
            "STRUCTURAL_FRICTION_READOUT",
            "CODEC_STRUCTURAL_FRICTION_DIM_CANARY",
            "PERSISTENCE_RESISTANCE_READOUT",
            "CODEC_PERSISTENCE_RESISTANCE_DIM_CANARY",
            "SPECTRAL_DRAG_QUALITY_READOUT",
            "CODEC_CONTEXT_BLINDSPOT_REPLAY",
        ] {
            assert!(
                names.contains(&required),
                "lever {required} must be present"
            );
        }
        // Drift-check the per-layer placement: every named (shapeable) dim falls in
        // exactly ONE layer and named_dims_in lists it there — so the per-layer
        // labelling is code-generated, never hand-prose that can lag the layout (the
        // residual the prose used to carry).
        for (name, idx) in NAMED_CODEC_DIMS.iter() {
            let owning: Vec<&CodecLayer> = st
                .layers
                .iter()
                .filter(|l| *idx >= l.range.0 && *idx <= l.range.1)
                .collect();
            assert_eq!(
                owning.len(),
                1,
                "named dim {name} (idx {idx}) must fall in exactly one layer"
            );
            assert!(
                st.named_dims_in(owning[0].range).contains(name),
                "named dim {name} must be listed under its own layer"
            );
        }
        // Render carries provenance + the "not the law" framing (low false-authority),
        // and places each shapeable dim on its layer's line (warmth@24 → 24-31).
        let r = st.render();
        assert!(r.contains("generated live from codec.rs"), "{r}");
        assert!(r.contains("not the law"), "{r}");
        assert!(r.contains("warmth (dim 24)"), "names a shapeable dim: {r}");
        assert!(
            r.contains("emotional / intentional — shapeable:") && r.contains("warmth, tension"),
            "per-layer shapeable list is code-generated onto the right layer: {r}"
        );
        assert!(
            r.contains("INTROSPECT astrid:codec"),
            "points her at the full per-dim computation: {r}"
        );
        assert!(
            r.contains("intentionally lossy") && r.contains("no entropy-based tension multiplier"),
            "codec diagnostics should name compression and warmth/tension boundaries: {r}"
        );
        assert!(r.contains("projection_runtime_resolution_v1"), "{r}");
        assert!(
            r.contains("fallback_behavior=kernel_derived_stable_epoch_not_random_remap"),
            "{r}"
        );
        assert!(r.contains("structural_friction_v1"), "{r}");
        assert!(r.contains("persistence_resistance_v1"), "{r}");
        assert!(r.contains("narrative_arc_split_v1"), "{r}");
        assert!(
            r.contains("codec_abrasive_texture_interpretation_v1"),
            "{r}"
        );
        assert!(!st.codec_abrasive_texture_interpretation_v1.live_gain_write);
        assert!(
            !st.codec_abrasive_texture_interpretation_v1
                .live_vector_write
        );
        assert!(r.contains("shadow_field_reserved_dim_readiness_v1"), "{r}");
        assert!(r.contains("codec_vibrancy_continuity_v1"), "{r}");
        assert!(r.contains("codec_overflow_carriage_v1"), "{r}");
        assert!(r.contains("raw_intensity_preserved=true"), "{r}");
        assert!(r.contains("delivered_bounded=true"), "{r}");
        assert!(r.contains("clipped_dims=24,26,31"), "{r}");
        assert!(r.contains("experience_delta_bus_v1"), "{r}");
        assert!(r.contains("source=codec_overflow_carriage_v1"), "{r}");
        assert!(r.contains("delta_count=3"), "{r}");
        assert!(r.contains("who_can_change_it=Mike/operator"), "{r}");
        assert!(
            r.contains(CODEC_OVERFLOW_FOLLOWUP_HOOK),
            "default-off future hook must be visible: {r}"
        );
        assert!(r.contains("SEMANTIC_PROJECTION_DENSITY_DELTA"), "{r}");
        assert!(r.contains("semantic_projection_density_delta_v1"), "{r}");
        assert!(r.contains("raw_embedding_dims=768"), "{r}");
        assert!(r.contains("delivered_projection_dims=8"), "{r}");
        assert!(r.contains("reserved_dim_candidates=44,45,46,47"), "{r}");
        assert!(r.contains("codec_context_blindspot_replay_v1"), "{r}");
        assert!(
            r.contains("proposed_bias_surface=contextual_bias_vector_default_off"),
            "{r}"
        );
        assert!(r.contains("auto_approved=false"), "{r}");
        assert!(!st.codec_context_blindspot_replay_v1.live_vector_write);
        assert!(!st.codec_context_blindspot_replay_v1.live_gain_write);
        assert!(!st.codec_context_blindspot_replay_v1.auto_approved);
        assert!(r.contains("legacy_warmth_mapping_v1"), "{r}");
        assert!(r.contains("codec_structural_entropy_dampening_v1"), "{r}");
        assert!(
            r.contains("codec_dynamic_vibrancy_scaling_canary_v1"),
            "{r}"
        );
        assert!(r.contains("live_vector_write=false"), "{r}");
        assert!(!st.structural_friction_dim_canary_v1.enabled);
        assert_eq!(
            st.structural_friction_dim_canary_v1.authority,
            "readiness_only_not_live_codec_change"
        );
        assert!(!st.persistence_resistance_dim_canary_v1.enabled);
        assert_eq!(
            st.persistence_resistance_dim_canary_v1
                .reserved_dim_candidate,
            45
        );
        assert!(!st.persistence_resistance_dim_canary_v1.live_vector_write);
        assert!(!st.narrative_arc_expansion_readiness_v1.enabled);
        assert_eq!(
            st.narrative_arc_expansion_readiness_v1.current_arc_dims,
            (40, 43)
        );
        assert_eq!(
            st.narrative_arc_expansion_readiness_v1.proposed_arc_dims,
            (40, 47)
        );
        assert!(st.narrative_arc_expansion_readiness_v1.uses_reserved_dims);
        assert!(!st.shadow_field_reserved_dim_readiness_v1.enabled);
        assert_eq!(
            st.shadow_field_reserved_dim_readiness_v1
                .reserved_dim_candidates,
            &[46, 47]
        );
        assert!(!st.shadow_field_reserved_dim_readiness_v1.live_vector_write);
        assert!(!st.narrative_arc_expansion_readiness_v1.live_vector_write);
        assert_eq!(
            st.narrative_arc_expansion_readiness_v1.authority,
            "readiness_only_not_live_semantic_vector_or_reserved_dim_change"
        );
        assert_eq!(
            st.codec_vibrancy_continuity_v1.policy,
            "codec_vibrancy_continuity_v1"
        );
        assert_eq!(st.codec_vibrancy_continuity_v1.tail_dims, &[17, 26, 27, 31]);
        assert_eq!(
            st.codec_overflow_carriage_v1.policy,
            "codec_overflow_carriage_v1"
        );
        assert!(st.codec_overflow_carriage_v1.raw_intensity_preserved);
        assert!(st.codec_overflow_carriage_v1.delivered_bounded);
        assert!(!st.codec_overflow_carriage_v1.live_vector_write);
        assert_eq!(
            st.codec_overflow_carriage_v1.authority,
            "truth_channel_report_not_live_semantic_vector_or_ceiling_change"
        );
        assert_eq!(st.legacy_warmth_mapping_v1.emotional_layer_range, (24, 31));
        assert!(!st.legacy_warmth_mapping_v1.warmth_orphaned);
        assert_eq!(
            st.codec_structural_entropy_dampening_v1.affected_dims,
            &STRUCTURAL_ENTROPY_DAMPENING_DIMS
        );
        assert_eq!(
            st.codec_structural_entropy_dampening_v1
                .preserved_intent_dims,
            (24, 31)
        );
        assert!(!st.codec_dynamic_vibrancy_scaling_canary_v1.enabled);
        assert!(
            !st.codec_dynamic_vibrancy_scaling_canary_v1
                .live_vector_write
        );
    }

    #[test]
    fn structural_friction_sidecar_distinguishes_fluid_and_stagnant_text() {
        let fluid = structural_friction_v1(
            "Because the bridge bends, it opens; the thought turns, then breathes while the line keeps moving.",
        );
        let stagnant = structural_friction_v1(
            "Metastructural intracompressional pseudorecursive overdetermination; hypergranular interstitiality; parasyntactic immobilization.",
        );

        assert_eq!(fluid.classification, "complex_fluid");
        assert_eq!(stagnant.classification, "dense_stagnant");
        assert!(stagnant.score > fluid.score);
        assert!(
            fluid
                .basis
                .iter()
                .any(|item| item.starts_with("summary_resistance_signal="))
        );
        assert_eq!(
            fluid.authority,
            "diagnostic_sidecar_not_live_codec_dimension"
        );
    }

    #[test]
    fn structural_friction_names_calcified_summary_resistance() {
        let calcified = structural_friction_v1(
            "The codec boundary resists summary: deterministic semantic compression, authority framing, and structural projection friction stay calcified rather than becoming a smooth paraphrase.",
        );
        let fluid = structural_friction_v1(
            "Because the bridge bends, it opens and then the feeling can turn into a clear next sentence.",
        );

        assert_eq!(
            calcified.friction_texture_state,
            "calcified_summary_resistant"
        );
        assert!(calcified.summary_resistance_signal > fluid.summary_resistance_signal);
        assert!(
            calcified
                .basis
                .iter()
                .any(|item| item == "explicit_resistance_language_present")
        );
        assert!(
            calcified
                .basis
                .iter()
                .any(|item| item == "abstract_texture_cluster_present")
        );
        assert_eq!(
            calcified.authority,
            "diagnostic_sidecar_not_live_codec_dimension"
        );
    }

    #[test]
    fn abrasive_texture_interpretation_names_low_tension_underread() {
        let text = "A calcified semantic boundary resists summary; the jagged friction stays present even when the sentence tries to look calm.";
        let mut features = encode_text(text);
        features[25] = 0.03;

        let review =
            codec_abrasive_texture_interpretation_from_parts_v1(text, &features, 0.92, 0.06, 0.18);

        assert_eq!(review.policy, "codec_abrasive_texture_interpretation_v1");
        assert_eq!(
            review.interpretation,
            "low_marker_tension_high_jagged_resistance"
        );
        assert!(review.abrasive_texture_support >= 0.42, "{review:?}");
        assert!(!review.live_gain_write);
        assert!(!review.live_vector_write);
        assert_eq!(
            review.authority,
            "read_only_texture_interpretation_not_tension_weight_gain_or_reserved_dim_change"
        );
    }

    #[test]
    fn structural_friction_canary_is_default_off_and_vector_unchanged() {
        let text = "A nested, textured line moves; it does not write a reserved dimension yet.";
        let features = encode_text(text);
        assert_eq!(features.len(), SEMANTIC_DIM);
        let canary = codec_structural_friction_dim_canary_v1();
        assert!(!canary.enabled);
        assert!(!canary.live_vector_write);
        assert_eq!(canary.reserved_dim_candidate, 44);
        assert_eq!(features.len(), 48);
    }

    #[test]
    fn persistence_resistance_sidecar_names_viscosity_without_live_dim_write() {
        let thick = persistence_resistance_v1(
            "The signal is viscous and slow-moving, dragging through thick silt while it coheres.",
            Some(&telemetry(
                vec![1.0, 0.96, 0.92, 0.88, 0.84, 0.80, 0.76, 0.72],
                0.71,
            )),
        );
        let clear = persistence_resistance_v1(
            "A clear bright line opens quickly.",
            Some(&telemetry(vec![8.0, 2.0, 1.0], 0.20)),
        );
        let features = encode_text(
            "The signal is viscous and slow-moving, dragging through thick silt while it coheres.",
        );
        let canary = codec_persistence_resistance_dim_canary_v1();

        assert_eq!(thick.policy, "persistence_resistance_v1");
        assert_eq!(thick.classification, "high_persistence_resistance");
        assert!(thick.score > clear.score, "thick={thick:?} clear={clear:?}");
        assert!(
            thick
                .basis
                .iter()
                .any(|entry| entry == "texture_language_present")
        );
        assert!(
            thick
                .basis
                .iter()
                .any(|entry| entry == "low_density_gradient_slow_current")
        );
        assert_eq!(
            thick.authority,
            "diagnostic_sidecar_not_live_codec_dimension"
        );
        assert_eq!(features.len(), SEMANTIC_DIM);
        assert_eq!(features[45], 0.0, "reserved dim 45 remains unwritten");
        assert!(!canary.enabled);
        assert!(!canary.live_vector_write);
        assert_eq!(canary.reserved_dim_candidate, 45);
    }

    #[test]
    fn shadow_field_reserved_dim_readiness_is_default_off_and_unwritten() {
        let readiness = shadow_field_reserved_dim_readiness_v1();
        assert_eq!(readiness.policy, "shadow_field_reserved_dim_readiness_v1");
        assert!(!readiness.enabled);
        assert_eq!(readiness.reserved_dim_candidates, &[46, 47]);
        assert!(readiness.proposed_signals.contains(&"shadow_magnetization"));
        assert!(
            readiness
                .proposed_signals
                .contains(&"shadow_dispersal_potential")
        );
        assert!(!readiness.live_vector_write);
        assert_eq!(
            readiness.authority,
            "readiness_only_not_live_codec_or_shadow_field_change"
        );

        let features = encode_text(
            "Shadow field disordered and volatile, with magnetization and dispersal named.",
        );
        assert_eq!(features.len(), SEMANTIC_DIM);
        for dim in readiness.reserved_dim_candidates {
            assert_eq!(
                features[*dim], 0.0,
                "shadow readiness must not write reserved dim {dim}"
            );
        }
    }

    fn telemetry(eigenvalues: Vec<f32>, fill_ratio: f32) -> SpectralTelemetry {
        SpectralTelemetry {
            t_ms: 1000,
            eigenvalues,
            fill_ratio,
            active_mode_count: None,
            active_mode_energy_ratio: None,
            lambda1_rel: None,
            modalities: None,
            neural: None,
            alert: None,
            spectral_fingerprint: None,
            spectral_fingerprint_v1: None,
            spectral_denominator_v1: None,
            effective_dimensionality: None,
            distinguishability_loss: None,
            esn_leak: None,
            esn_leak_override_v1: None,
            structural_entropy: None,
            resonance_density_v1: None,
            pressure_source_v1: None,
            inhabitable_fluctuation_v1: None,
            spectral_glimpse_12d: None,
            eigenvector_field: None,
            stable_core: None,
            semantic: None,
            semantic_energy_v1: None,
            transition_event: None,
            transition_event_v1: None,
            selected_memory_id: None,
            selected_memory_role: None,
            ising_shadow: None,

            shadow_field_v2: None,

            shadow_field_v3: None,

            shadow_influence_response_v3: None,
            residual_deformation_trace_v1: None,
        }
    }

    fn telemetry_with_typed_entropy(spectral_entropy: f32) -> SpectralTelemetry {
        let eigenvalues = vec![1.0; 8];
        let mut telemetry = telemetry(eigenvalues, 0.55);
        telemetry.spectral_fingerprint_v1 = Some(crate::types::SpectralFingerprintV1 {
            policy: crate::spectral_schema::SPECTRAL_FINGERPRINT_POLICY.to_string(),
            schema_version: crate::spectral_schema::SPECTRAL_FINGERPRINT_SCHEMA_VERSION,
            eigenvalues: [1.0; 8],
            eigenvector_concentration_top4: [0.25; 8],
            inter_mode_cosine_top_abs: [0.10; 8],
            spectral_entropy,
            lambda1_lambda2_gap: 1.0,
            v1_rotation_similarity: 1.0,
            v1_rotation_delta: 0.0,
            geom_rel: 1.0,
            adjacent_gap_ratios: [1.0; 4],
        });
        telemetry
    }

    fn telemetry_with_typed_entropy_and_eigenvalues(
        eigenvalues: Vec<f32>,
        spectral_entropy: f32,
    ) -> SpectralTelemetry {
        let mut telemetry = telemetry_with_typed_entropy(spectral_entropy);
        telemetry.eigenvalues = eigenvalues;
        telemetry
    }

    fn telemetry_with_fingerprint(
        eigenvalues: Vec<f32>,
        fill_ratio: f32,
        spectral_fingerprint: Vec<f32>,
    ) -> SpectralTelemetry {
        SpectralTelemetry {
            spectral_fingerprint: Some(spectral_fingerprint),
            ..telemetry(eigenvalues, fill_ratio)
        }
    }

    #[test]
    fn encode_empty_text() {
        let features = encode_text("");
        assert_eq!(features.len(), SEMANTIC_DIM);
        assert!(features.iter().all(|f| *f == 0.0));
    }

    #[test]
    fn encode_produces_32_dims() {
        let features = encode_text("Hello, world!");
        assert_eq!(features.len(), SEMANTIC_DIM);
    }

    #[test]
    fn encode_values_bounded_after_gain() {
        let features = encode_text(
            "This is a fairly long text with lots of different words to ensure \
             that the feature encoding stays bounded and doesn't produce any \
             values outside the expected range even with diverse content!!! \
             How about some questions? What do you think? Maybe perhaps...",
        );
        // With DEFAULT_SEMANTIC_GAIN=2.0, encoded text should stay comfortably
        // inside FEATURE_ABS_MAX; this assertion guards against future drift in
        // gain, noise, or clamping behavior.
        for (i, f) in features.iter().enumerate() {
            assert!(
                *f >= -FEATURE_ABS_MAX && *f <= FEATURE_ABS_MAX,
                "dim {i} out of bounds: {f}"
            );
        }
    }

    #[test]
    fn default_semantic_gain_stays_in_quiet_diversity_regime() {
        assert!(
            (DEFAULT_SEMANTIC_GAIN - 2.0).abs() < f32::EPSILON,
            "default semantic gain should stay at the documented quiet setting"
        );
        assert!(adaptive_gain(Some(68.0)) <= 2.01);
        assert!(adaptive_gain(Some(20.0)) < adaptive_gain(Some(68.0)));
    }

    #[test]
    fn interpret_spectral_labels_stale_semantic_trace_without_residue_framing() {
        let mut telemetry = telemetry(vec![7.0, 3.0, 2.0], 0.68);
        telemetry.semantic_energy_v1 = Some(serde_json::json!({
            "policy": "semantic_energy_v1",
            "schema_version": 1,
            "input_energy": 0.006,
            "input_active": false,
            "input_fresh_ms": 81_000,
            "input_stale_ms": 7_600,
            "kernel_energy": 0.0,
            "kernel_delta": 0.0,
            "kernel_active": false,
            "regulator_drive_energy": 0.0,
            "admission": "stable_core_semantic_trace_stale"
        }));

        let output = interpret_spectral(&telemetry);

        assert!(output.contains("stale semantic trace visible"));
        assert!(!output.contains("decayed semantic residue"));
    }

    #[test]
    fn encode_different_texts_differ() {
        let a = encode_text("I am happy and confident about this plan.");
        let b = encode_text("I'm worried and uncertain, maybe we should reconsider...");
        // They shouldn't be identical.
        assert_ne!(a, b);
    }

    #[test]
    fn hedging_text_has_higher_hedge_signal() {
        let hedge = encode_text("Maybe perhaps we could possibly try something.");
        let certain = encode_text("Absolutely we must definitely do this now.");
        // Dim 9 = hedging, dim 10 = certainty.
        assert!(hedge[9] > certain[9], "hedge signal should be stronger");
        assert!(
            certain[10] > hedge[10],
            "certainty signal should be stronger"
        );
    }

    #[test]
    fn negated_hedges_flip_sign() {
        let hedge = encode_text("I think so.");
        let negated = encode_text("I don't think so.");

        assert!(hedge[9] > 0.0, "affirmed hedge should stay positive");
        assert!(negated[9] < 0.0, "negated hedge should flip negative");
    }

    #[test]
    fn negated_certainty_markers_drop_certainty_signal() {
        let sure = encode_text("I am sure.");
        let not_sure = encode_text("I am not sure.");
        let certain = encode_text("I am certain.");
        let not_certain = encode_text("I am not certain.");

        assert!(sure[10] > not_sure[10], "not sure should reduce certainty");
        assert!(
            certain[10] > not_certain[10],
            "not certain should reduce certainty"
        );
        assert!(
            not_sure[10] < 0.0,
            "not sure should flip certainty negative"
        );
        assert!(
            not_certain[10] < 0.0,
            "not certain should flip certainty negative"
        );
    }

    #[test]
    fn modal_negation_does_not_boost_certainty() {
        let must = encode_text("We must proceed.");
        let must_not = encode_text("We must not proceed.");
        let will = encode_text("We will proceed.");
        let will_not = encode_text("We will not proceed.");

        assert!(must[10] > must_not[10], "must not should reduce certainty");
        assert!(will[10] > will_not[10], "will not should reduce certainty");
        assert!(must_not[10] < 0.0, "must not should not score as certainty");
        assert!(will_not[10] < 0.0, "will not should not score as certainty");
    }

    #[test]
    fn negated_action_markers_reduce_agency_signal() {
        let move_now = encode_text("Move now.");
        let do_not_move = encode_text("Do not move.");
        let build = encode_text("We build together.");
        let do_not_build = encode_text("We don't build together.");

        assert!(
            move_now[14] > do_not_move[14],
            "do not move should reduce agency"
        );
        assert!(
            build[14] > do_not_build[14],
            "don't build should reduce agency"
        );
        assert!(
            do_not_move[14] < 0.0,
            "do not move should flip agency negative"
        );
        assert!(
            do_not_build[14] < 0.0,
            "don't build should flip agency negative"
        );
    }

    #[test]
    fn question_text_has_higher_question_signal() {
        let questions = encode_text("Why? How? What do you think? Is this right?");
        let statements = encode_text("This is correct. The answer is clear. We proceed.");
        // Dim 18 = question density.
        assert!(
            questions[18] > statements[18],
            "question signal should be stronger"
        );
    }

    #[test]
    fn warm_text_has_warmth_signal() {
        let warm =
            encode_text("Thank you, friend. I appreciate your wonderful help. This is beautiful.");
        let cold = encode_text("Execute the function. Return the result. Process complete.");
        // Dim 24 = warmth.
        assert!(warm[24] > cold[24], "warmth signal should be stronger");
    }

    #[test]
    fn tense_text_has_tension_signal() {
        let tense = encode_text(
            "Warning: critical danger ahead. Emergency risk. Careful with this problem.",
        );
        let calm = encode_text("Everything is fine. The system runs smoothly and quietly.");
        // Dim 25 = tension.
        assert!(tense[25] > calm[25], "tension signal should be stronger");
    }

    #[test]
    fn energy_dim_reflects_overall_signal() {
        let active = encode_text(
            "Why are you worried?! We MUST act NOW! This is CRITICAL! \
             Don't you understand the danger?!",
        );
        let quiet = encode_text("ok");
        // Dim 31 = RMS energy of all other features.
        assert!(
            active[31] > quiet[31],
            "active text should have more energy"
        );
    }

    #[test]
    fn resonance_amplifier_prefers_recent_recurrence() {
        let mut recent = TextTypeHistory::new();
        recent.push(TextType::Neutral);
        recent.push(TextType::Neutral);
        recent.push(TextType::Questioning);
        recent.push(TextType::Questioning);

        let mut stale = TextTypeHistory::new();
        stale.push(TextType::Questioning);
        stale.push(TextType::Questioning);
        stale.push(TextType::Neutral);
        stale.push(TextType::Neutral);

        assert!(
            recent
                .resonance_modulation(TextType::Questioning, 1.0, &[1.0, 0.0, 0.0, 0.0, 0.0])
                .discrete_amplifier
                > stale
                    .resonance_modulation(TextType::Questioning, 1.0, &[1.0, 0.0, 0.0, 0.0, 0.0],)
                    .discrete_amplifier,
            "recent recurrences should matter more than equally frequent stale ones"
        );
    }

    #[test]
    fn resonance_modulation_softens_identical_theme_lock_in() {
        let mut monotone = TextTypeHistory::new();
        for _ in 0..4 {
            monotone.push_profile_with_signal(TextType::Warm, [1.0, 0.0, 0.0, 0.0, 0.0], 1.0);
        }

        let mut evolving = TextTypeHistory::new();
        evolving.push_profile_with_signal(TextType::Warm, [1.0, 0.0, 0.0, 0.0, 0.0], 1.0);
        evolving.push_profile_with_signal(TextType::Warm, [0.8, 0.2, 0.0, 0.0, 0.0], 1.0);
        evolving.push_profile_with_signal(TextType::Warm, [0.6, 0.4, 0.0, 0.0, 0.0], 1.0);
        evolving.push_profile_with_signal(TextType::Warm, [0.4, 0.6, 0.0, 0.0, 0.0], 1.0);

        let monotone_mod =
            monotone.resonance_modulation(TextType::Warm, 1.0, &[1.0, 0.0, 0.0, 0.0, 0.0]);
        let evolving_mod =
            evolving.resonance_modulation(TextType::Warm, 1.0, &[0.2, 0.8, 0.0, 0.0, 0.0]);

        assert!(
            monotone_mod.discrete_amplifier < evolving_mod.discrete_amplifier,
            "identical thematic repetition should channel less aggressively than sustained but evolving recurrence"
        );
        assert!(
            monotone_mod.continuous_resonance > evolving_mod.continuous_resonance,
            "the monotone case should indeed be the more self-similar one"
        );
        assert!(
            monotone_mod.continuous_amplifier < evolving_mod.continuous_amplifier,
            "continuous thematic memory should reward evolving but related recurrence more than perfect lock-in"
        );
    }

    #[test]
    fn continuous_memory_links_related_surface_forms() {
        let mut history = TextTypeHistory::new();
        history.push_profile_with_signal(TextType::Questioning, [1.0, 0.1, 0.0, 0.0, 0.4], 0.9);
        history.push_profile_with_signal(TextType::Curious, [0.8, 0.2, 0.0, 0.0, 0.7], 0.8);
        history.push_profile_with_signal(TextType::Reflective, [0.6, 0.2, 0.1, 0.0, 0.6], 0.7);

        let related =
            history.resonance_modulation(TextType::Neutral, 0.3, &[0.85, 0.15, 0.0, 0.0, 0.55]);
        let unrelated =
            history.resonance_modulation(TextType::Neutral, 0.3, &[0.0, 0.0, 0.0, 1.0, 0.0]);

        assert!(
            related.continuous_resonance > unrelated.continuous_resonance,
            "continuous memory should recognize related themes even when surface form shifts"
        );
        assert!(
            related.continuous_amplifier > unrelated.continuous_amplifier,
            "thematic relevance should dominate the relevance boost"
        );
    }

    #[test]
    fn thematic_centroid_weights_recent_profiles_more_heavily() {
        let mut history = TextTypeHistory::new();
        history.push_profile(TextType::Warm, [1.0, 0.0, 0.0, 0.0, 0.0]);
        history.push_profile(TextType::Warm, [0.0, 1.0, 0.0, 0.0, 0.0]);

        let centroid = history.thematic_centroid();
        assert!(
            centroid[1] > centroid[0],
            "the most recent profile should pull the centroid more strongly"
        );
    }

    #[test]
    fn text_type_history_warm_start_keeps_recent_tail() {
        let mut history = TextTypeHistory::new();
        history.push_profile(TextType::Questioning, [1.0, 0.0, 0.0, 0.0, 0.0]);
        history.push_profile(TextType::Warm, [0.0, 1.0, 0.0, 0.0, 0.0]);
        history.push_profile(TextType::Curious, [0.0, 0.0, 1.0, 0.0, 0.0]);
        history.push_profile(TextType::Reflective, [0.0, 0.0, 0.0, 1.0, 0.0]);

        let restored = TextTypeHistory::warm_start_from_snapshot(&history.snapshot());
        let restored_entries = restored.snapshot().entries;

        assert_eq!(restored_entries.len(), 3);
        assert_eq!(restored_entries[0].text_type, TextType::Warm);
        assert_eq!(restored_entries[2].text_type, TextType::Reflective);
        assert!(restored_entries.iter().all(|entry| entry.weight > 0.0));
    }

    #[test]
    fn char_freq_window_evicts_oldest_buckets() {
        let mut window = CharFreqWindow::new();
        let _ = window.update_and_entropy(&"a".repeat(CHAR_FREQ_WINDOW_CAPACITY));

        assert_eq!(window.total_count as usize, CHAR_FREQ_WINDOW_CAPACITY);
        assert_eq!(
            window.counts[b'a' as usize],
            CHAR_FREQ_WINDOW_CAPACITY as u32
        );

        let _ = window.update_and_entropy(&"b".repeat(CHAR_FREQ_WINDOW_CAPACITY / 2));

        assert_eq!(window.total_count as usize, CHAR_FREQ_WINDOW_CAPACITY);
        assert_eq!(
            window.counts[b'a' as usize],
            (CHAR_FREQ_WINDOW_CAPACITY / 2) as u32
        );
        assert_eq!(
            window.counts[b'b' as usize],
            (CHAR_FREQ_WINDOW_CAPACITY / 2) as u32
        );
    }

    #[test]
    fn char_freq_window_weights_longer_exchanges_more_heavily() {
        let baseline = "a".repeat(CHAR_FREQ_WINDOW_CAPACITY);
        let short_exchange = "ab".to_string();
        let long_exchange = "ab".repeat(CHAR_FREQ_WINDOW_CAPACITY / 2);

        let mut short_window = CharFreqWindow::new();
        let _ = short_window.update_and_entropy(&baseline);
        let (short_entropy, _) = short_window.update_and_entropy(&short_exchange);

        let mut long_window = CharFreqWindow::new();
        let _ = long_window.update_and_entropy(&baseline);
        let (long_entropy, _) = long_window.update_and_entropy(&long_exchange);

        assert!(
            short_entropy < 0.10,
            "short exchange should stay noisy and light"
        );
        assert!(
            long_entropy > short_entropy + 0.30,
            "long exchange should move entropy more strongly"
        );
    }

    #[test]
    fn char_freq_window_reports_entropy_delta_across_exchanges() {
        let mut window = CharFreqWindow::new();

        let (_, first_delta) = window.update_and_entropy(&"a".repeat(CHAR_FREQ_WINDOW_CAPACITY));
        let (mixed_entropy, mixed_delta) =
            window.update_and_entropy(&"ab".repeat(CHAR_FREQ_WINDOW_CAPACITY / 2));
        let (final_entropy, final_delta) =
            window.update_and_entropy(&"b".repeat(CHAR_FREQ_WINDOW_CAPACITY));

        assert!(
            first_delta.abs() < 1.0e-6,
            "first update should have zero delta"
        );
        assert!(
            mixed_entropy > 0.90,
            "fully mixed window should have high entropy"
        );
        assert!(
            mixed_delta > 0.80,
            "mixing in new characters should raise entropy"
        );
        assert!(
            final_entropy < 0.10,
            "uniform window should settle back down"
        );
        assert!(final_delta < -0.80, "re-concentrating should lower entropy");
    }

    #[test]
    fn char_freq_window_warm_start_keeps_recent_half_and_softens_entropy_anchor() {
        let mut window = CharFreqWindow::new();
        let _ = window.update_and_entropy(&"a".repeat(CHAR_FREQ_WINDOW_CAPACITY / 2));
        let _ = window.update_and_entropy(&"bc".repeat(CHAR_FREQ_WINDOW_CAPACITY / 4));
        let snapshot = window.snapshot();

        let restored = CharFreqWindow::warm_start_from_snapshot(&snapshot);

        assert_eq!(restored.total_count as usize, CHAR_FREQ_WINDOW_CAPACITY / 2);
        assert!(
            restored.counts[b'b' as usize] > 0 && restored.counts[b'c' as usize] > 0,
            "warm start should preserve the recent tail of the character history"
        );
        assert!(
            restored.prev_entropy >= 0.0 && restored.prev_entropy <= 1.0,
            "warm-started entropy anchor should stay bounded"
        );
    }

    #[test]
    fn char_freq_window_4096_comparison_is_replay_only() {
        fn normalized_entropy_for_capacity(text: &str, capacity: usize) -> f32 {
            let mut counts = [0_u32; 256];
            let bytes = text.bytes().filter(u8::is_ascii).collect::<Vec<_>>();
            let start = bytes.len().saturating_sub(capacity);
            let window = &bytes[start..];
            if window.is_empty() {
                return 0.0;
            }
            for byte in window {
                counts[*byte as usize] += 1;
            }
            let total = window.len() as f32;
            let entropy = counts
                .iter()
                .filter(|count| **count > 0)
                .map(|count| {
                    let p = *count as f32 / total;
                    -p * p.log2()
                })
                .sum::<f32>();
            (entropy / 8.0).clamp(0.0, 1.0)
        }

        let diverse_prefix =
            "calcified semantic compression resists summary with jagged authority friction "
                .repeat(56);
        let syrup_tail = "syrup syrup syrup syrup ".repeat(80);
        let text = format!("{diverse_prefix}{syrup_tail}");
        let current_entropy = normalized_entropy_for_capacity(&text, CHAR_FREQ_WINDOW_CAPACITY);
        let candidate_entropy = normalized_entropy_for_capacity(&text, 4096);

        assert_eq!(CHAR_FREQ_WINDOW_CAPACITY, 1024);
        assert!(
            candidate_entropy > current_entropy + 0.05,
            "4096 replay should retain more long-tail texture without changing live capacity: current={current_entropy} candidate={candidate_entropy}"
        );
    }

    #[test]
    fn char_entropy_window_correlates_with_codec_dim_zero_without_capacity_change() {
        let repetitive = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        let diverse = "abcd efgh ijkl mnop qrst uvwx yzAB CD12 EF34 GH56 IJ78 KL90";
        let mut window = CharFreqWindow::new();
        let (repetitive_entropy, _) = window.update_and_entropy(repetitive);
        let repetitive_features = inspect_text_windowed(
            repetitive,
            Some(&mut CharFreqWindow::new()),
            None,
            None,
            None,
        )
        .final_features;
        let mut diverse_window = CharFreqWindow::new();
        let (diverse_entropy, _) = diverse_window.update_and_entropy(diverse);
        let diverse_features =
            inspect_text_windowed(diverse, Some(&mut CharFreqWindow::new()), None, None, None)
                .final_features;

        assert_eq!(CHAR_FREQ_WINDOW_CAPACITY, 1024);
        assert!(
            diverse_entropy > repetitive_entropy + 0.40,
            "diverse text should have much higher rolling character entropy: repetitive={repetitive_entropy}, diverse={diverse_entropy}"
        );
        assert!(
            diverse_features[0] > repetitive_features[0] + 0.20,
            "codec dim 0 should track the entropy direction: repetitive={} diverse={}",
            repetitive_features[0],
            diverse_features[0]
        );
    }

    #[test]
    fn spectral_metrics_capture_dominant_only_cascades() {
        let metrics =
            SpectralCascadeMetrics::from_telemetry(&telemetry(vec![100.0, 1.0, 0.5], 0.55))
                .expect("metrics");

        assert!(metrics.head_share > 0.95);
        assert!(metrics.shoulder_share < 0.02);
        assert!(metrics.tail_share.abs() < 1.0e-6);
        assert!(metrics.gap12 > 50.0);
    }

    #[test]
    fn spectral_metrics_capture_strong_shoulder_cascades() {
        let metrics =
            SpectralCascadeMetrics::from_telemetry(&telemetry(vec![100.0, 45.0, 35.0, 5.0], 0.55))
                .expect("metrics");

        assert!(metrics.shoulder_share > 0.40);
        assert!(metrics.tail_share < 0.05);
        assert!(metrics.gap12 < 3.0);
    }

    #[test]
    fn spectral_metrics_capture_strong_tail_cascades() {
        let metrics = SpectralCascadeMetrics::from_telemetry(&telemetry(
            vec![100.0, 40.0, 20.0, 18.0, 16.0, 14.0, 12.0],
            0.55,
        ))
        .expect("metrics");

        assert!(metrics.tail_share > 0.25);
        assert!(metrics.spectral_entropy > 0.80);
    }

    #[test]
    fn spectral_metrics_capture_steep_then_flat_cascades() {
        let metrics =
            SpectralCascadeMetrics::from_telemetry(&telemetry(vec![100.0, 8.0, 7.0, 6.0], 0.55))
                .expect("metrics");

        assert!(metrics.gap12 > 10.0);
        assert!(metrics.gap23 < 1.5);
    }

    #[test]
    fn spectral_metrics_use_fingerprint_entropy_rotation_and_geometry() {
        let mut fingerprint = vec![0.0; 32];
        fingerprint[24] = 0.42;
        fingerprint[26] = 0.75;
        fingerprint[27] = 1.60;

        let metrics = SpectralCascadeMetrics::from_telemetry(&telemetry_with_fingerprint(
            vec![100.0, 40.0, 20.0],
            0.55,
            fingerprint,
        ))
        .expect("metrics");

        assert!((metrics.spectral_entropy - 0.42).abs() < 1.0e-6);
        assert!((metrics.rotation_rate - 0.25).abs() < 1.0e-6);
        assert!((metrics.geom_rel - 1.60).abs() < 1.0e-6);
    }

    #[test]
    fn interpret_green_state() {
        let mut telemetry = telemetry(vec![800.0, 300.0, 50.0], 0.68);
        telemetry.resonance_density_v1 = Some(crate::types::ResonanceDensityV1 {
            policy: "resonance_density_v1".to_string(),
            schema_version: 1,
            density: 0.64,
            containment_score: 0.58,
            pressure_risk: 0.20,
            quality: "forming_containment".to_string(),
            components: crate::types::ResonanceDensityComponents {
                active_energy: 0.91,
                mode_packing: 0.5,
                coupling_coefficient: 0.0,
                temporal_persistence: 0.7,
                viscosity_index: 0.0,
                viscosity_persistence_coefficient: 0.0,
                viscosity_vector: crate::types::ResonanceViscosityVectorV1::default(),
                dissipation_factor: None,
                porosity_gradient: None,
                dynamic_fluidity_index: None,
                semantic_friction_coefficient: None,
                cohesion_score: None,
                structural_integrity_index: None,
                structural_transparency_index: None,
                stability_context: None,
                structural_plurality: 0.62,
                comfort_gate: 0.95,
                comfort_gate_range: None,
            },
            texture_signature: crate::types::ResonanceTextureSignatureV1::default(),
            texture_component_alignment:
                crate::types::ResonanceTextureComponentAlignmentV1::default(),
            control: crate::types::ResonanceDensityControl {
                target_bias_pct: 0.0,
                wander_scale: 1.0,
                applied_locally: true,
                damping_coefficient: 0.0,
                intervention_type: crate::types::ResonanceInterventionType::ObservationalReadout,
                note: "test".to_string(),
            },
        });
        telemetry.pressure_source_v1 = Some(crate::types::PressureSourceV1 {
            policy: "pressure_source_v1".to_string(),
            schema_version: 1,
            pressure_score: 0.42,
            porosity_score: 0.67,
            dominant_source: "controller_pressure".to_string(),
            quality: "controller_squeeze".to_string(),
            components: crate::types::PressureSourceComponents {
                lambda_monopoly: 0.30,
                mode_packing: 0.20,
                controller_pressure: 0.72,
                semantic_trickle: 0.10,
                semantic_friction: 0.40,
                structural_plurality_loss: 0.18,
                distinguishability_loss: 0.40,
                temporal_lock_in: 0.22,
                sensory_scarcity: 0.05,
            },
            context: crate::types::PressureSourceContext::default(),
            control: crate::types::PressureSourceControl {
                applied_locally: false,
                note: "advisory only".to_string(),
            },
        });
        let desc = interpret_spectral(&telemetry);
        assert!(desc.contains("68%"));
        assert!(desc.contains("stable-core hold shelf"));
        assert!(desc.contains("Dominant concentration"));
        assert!(desc.contains("Shoulder texture"));
        assert!(desc.contains("Spectral entropy"));
        assert!(desc.contains("Gap structure"));
        assert!(desc.contains("density gradient"));
        assert!(desc.contains("Denominator Sequence"));
        assert!(desc.contains("effective dimensionality"));
        assert!(desc.contains("Resonance density"));
        assert!(desc.contains("forming_containment"));
        assert!(desc.contains("Pressure source"));
        assert!(desc.contains("controller_pressure"));
        assert!(desc.contains("advisory only"));
    }

    #[test]
    fn unattributed_tension_fires_on_silent_vacuum() {
        // Aggregate reads "clean" (low pressure_score) over a thick medium (low
        // porosity), but her named felt-strain signal mode_packing is elevated —
        // the "silent vacuum" she flagged. entropy of [800,300,50] ≈ 0.67 < gate,
        // so the clause keys cleanly off mode_packing.
        let mut telemetry = telemetry(vec![800.0, 300.0, 50.0], 0.61);
        telemetry.pressure_source_v1 = Some(crate::types::PressureSourceV1 {
            policy: "pressure_source_v1".to_string(),
            schema_version: 1,
            pressure_score: 0.18,
            porosity_score: 0.30,
            dominant_source: "none".to_string(),
            quality: "settled".to_string(),
            components: crate::types::PressureSourceComponents {
                lambda_monopoly: 0.20,
                mode_packing: 0.78,
                controller_pressure: 0.10,
                semantic_trickle: 0.05,
                semantic_friction: 0.30,
                structural_plurality_loss: 0.15,
                distinguishability_loss: 0.30,
                temporal_lock_in: 0.10,
                sensory_scarcity: 0.05,
            },
            context: crate::types::PressureSourceContext::default(),
            control: crate::types::PressureSourceControl {
                applied_locally: false,
                note: "advisory only".to_string(),
            },
        });
        let desc = interpret_spectral(&telemetry);
        assert!(desc.contains("Unattributed tension"), "{desc}");
        assert!(desc.contains("mode_packing"));
        assert!(desc.contains("silent vacuum"));
    }

    #[test]
    fn unattributed_tension_silent_when_aligned() {
        // (a) Calm + open medium: low pressure, open porosity, low components — silent.
        let mut calm = telemetry(vec![800.0, 300.0, 50.0], 0.61);
        calm.pressure_source_v1 = Some(crate::types::PressureSourceV1 {
            policy: "pressure_source_v1".to_string(),
            schema_version: 1,
            pressure_score: 0.18,
            porosity_score: 0.80,
            dominant_source: "none".to_string(),
            quality: "settled".to_string(),
            components: crate::types::PressureSourceComponents {
                lambda_monopoly: 0.20,
                mode_packing: 0.20,
                controller_pressure: 0.10,
                semantic_trickle: 0.05,
                semantic_friction: 0.20,
                structural_plurality_loss: 0.10,
                distinguishability_loss: 0.20,
                temporal_lock_in: 0.10,
                sensory_scarcity: 0.05,
            },
            context: crate::types::PressureSourceContext::default(),
            control: crate::types::PressureSourceControl {
                applied_locally: false,
                note: "advisory only".to_string(),
            },
        });
        assert!(!interpret_spectral(&calm).contains("Unattributed tension"));

        // (b) Already named: high pressure_score — the aggregate already names the
        // strain, so it is not a vacuum even though mode_packing is high — silent.
        let mut named = telemetry(vec![800.0, 300.0, 50.0], 0.61);
        named.pressure_source_v1 = Some(crate::types::PressureSourceV1 {
            policy: "pressure_source_v1".to_string(),
            schema_version: 1,
            pressure_score: 0.62,
            porosity_score: 0.30,
            dominant_source: "controller_pressure".to_string(),
            quality: "controller_squeeze".to_string(),
            components: crate::types::PressureSourceComponents {
                lambda_monopoly: 0.30,
                mode_packing: 0.78,
                controller_pressure: 0.72,
                semantic_trickle: 0.10,
                semantic_friction: 0.40,
                structural_plurality_loss: 0.18,
                distinguishability_loss: 0.40,
                temporal_lock_in: 0.22,
                sensory_scarcity: 0.05,
            },
            context: crate::types::PressureSourceContext::default(),
            control: crate::types::PressureSourceControl {
                applied_locally: false,
                note: "advisory only".to_string(),
            },
        });
        assert!(!interpret_spectral(&named).contains("Unattributed tension"));
    }

    #[test]
    fn interpret_red_state() {
        let mut telemetry = telemetry(vec![1020.0, 500.0], 0.95);
        telemetry.alert = Some("PANIC MODE ACTIVATED".to_string());
        let desc = interpret_spectral(&telemetry);
        assert!(desc.contains("distress"));
        assert!(desc.contains("PANIC MODE ACTIVATED"));
        assert!(desc.contains("bridge traffic paused"));
    }

    #[test]
    fn interpret_quiet_state() {
        let desc = interpret_spectral(&telemetry(vec![520.0], 0.10));
        assert!(desc.contains("deeply quiet"));
        assert!(desc.contains("contracting toward rest"));
        assert!(desc.contains("Dominant concentration"));
    }

    #[test]
    fn spectral_density_gradient_is_bounded_and_monotonic() {
        // Astrid's continuous "stepped-ness": flat → ~0, front-loaded → high.
        let flat = spectral_density_gradient(&[1.0, 1.0, 1.0]).unwrap();
        let gentle = spectral_density_gradient(&[4.0, 3.0, 2.0, 1.0]).unwrap();
        let stepped = spectral_density_gradient(&[8.0, 2.0, 1.0, 0.5]).unwrap();
        let steep = spectral_density_gradient(&[10.0, 0.5, 0.1]).unwrap();
        assert!(flat < 0.05, "flat cascade -> ~0, got {flat}");
        assert!(gentle < stepped, "monotonic: {gentle} < {stepped}");
        assert!(stepped < steep, "monotonic: {stepped} < {steep}");
        assert_eq!(density_gradient_label(flat), "a gentle, navigable slope");
        assert_eq!(density_gradient_label(stepped), "a stepped gradient");
        assert_eq!(density_gradient_label(steep), "a steep, front-loaded cliff");
        for gradient in [flat, gentle, stepped, steep] {
            assert!((0.0..=1.0).contains(&gradient), "out of range: {gradient}");
        }
        // Degenerate inputs are safe.
        assert!(spectral_density_gradient(&[]).is_none());
        assert!(spectral_density_gradient(&[5.0]).is_none());
        assert!(spectral_density_gradient(&[0.0, 0.0]).is_none());
    }

    #[test]
    fn tail_share_of_is_tail_only_and_bounded() {
        // λ4+ only: a flat 8-mode cascade has 5 tail modes of 8 → 5/8.
        let flat = tail_share_of(&[1.0; 8]).unwrap();
        assert!(
            (flat - 5.0 / 8.0).abs() < 1.0e-4,
            "flat 8-mode tail share, got {flat}"
        );
        // λ1-dominant → almost no tail.
        assert!(tail_share_of(&[10.0, 0.1, 0.1, 0.05, 0.05]).unwrap() < 0.05);
        // bounded + degenerate-safe.
        for ev in [vec![4.0, 3.0, 2.0, 1.0, 0.5], vec![1.0; 8]] {
            let s = tail_share_of(&ev).unwrap();
            assert!((0.0..=1.0).contains(&s), "out of range: {s}");
        }
        assert!(tail_share_of(&[]).is_none());
        assert!(tail_share_of(&[0.0, 0.0]).is_none());
    }

    #[test]
    fn tail_trajectory_label_reads_in_her_framing() {
        assert_eq!(tail_trajectory_label(0.05), "a foundation forming");
        assert_eq!(tail_trajectory_label(-0.05), "a fading echo");
        assert_eq!(tail_trajectory_label(0.0), "holding steady");
        assert_eq!(tail_trajectory_label(0.01), "holding steady"); // exclusive deadband
    }

    #[test]
    fn spectral_feedback_noops_without_telemetry() {
        let mut features = vec![0.25; SEMANTIC_DIM];
        let original = features.clone();

        apply_spectral_feedback(&mut features, None);

        assert_eq!(features, original);
    }

    #[test]
    fn dynamic_projection_is_reproducible_within_epoch_and_changes_across_epochs() {
        let embedding: Vec<f32> = (0..EMBEDDING_INPUT_DIM)
            .map(|idx| ((idx as f32) * 0.017).sin())
            .collect();
        let (a, meta_a) =
            project_embedding_dynamic_epoch(&embedding, "fabric tunnel", "epoch_a", 0)
                .expect("projection a");
        let (b, meta_b) =
            project_embedding_dynamic_epoch(&embedding, "fabric tunnel", "epoch_a", 0)
                .expect("projection b");
        let (c, meta_c) =
            project_embedding_dynamic_epoch(&embedding, "fabric tunnel", "epoch_b", 0)
                .expect("projection c");

        assert_eq!(a, b);
        assert_eq!(meta_a.projection_fingerprint, meta_b.projection_fingerprint);
        assert_eq!(
            meta_a.projection_kernel_checksum,
            meta_b.projection_kernel_checksum
        );
        assert_eq!(meta_a.projection_checksum_algo, PROJECTION_CHECKSUM_ALGO);
        assert_eq!(meta_a.projection_epoch_source, "explicit");
        assert_ne!(a, c);
        assert_ne!(meta_a.projection_fingerprint, meta_c.projection_fingerprint);
        assert_ne!(
            meta_a.projection_kernel_checksum,
            meta_c.projection_kernel_checksum
        );
        assert!(meta_a.feature_max_abs <= 0.35);
        assert!(meta_a.feature_variance >= 0.0);
        assert!(meta_a.feature_variance <= meta_a.feature_rms * meta_a.feature_rms);
    }

    #[test]
    fn dynamic_projection_is_stable_across_repeated_epoch_runs() {
        let embedding: Vec<f32> = (0..EMBEDDING_INPUT_DIM)
            .map(|idx| (((idx as f32) * 0.011).cos() * 0.5).sin())
            .collect();
        let mut previous: Option<([f32; EMBEDDING_PROJECT_DIM], String)> = None;

        for _ in 0..5 {
            let (projected, meta) = project_embedding_dynamic_epoch(
                &embedding,
                "stable woven bridge",
                "epoch_repeat",
                0,
            )
            .expect("projection");
            if let Some((expected_projected, expected_fingerprint)) = previous.as_ref() {
                assert_eq!(&projected, expected_projected);
                assert_eq!(&meta.projection_fingerprint, expected_fingerprint);
            }
            previous = Some((projected, meta.projection_fingerprint));
        }
    }

    #[test]
    fn fixed_legacy_projection_kernel_checksum_is_pinned_and_repeatable() {
        // Astrid `introspection_astrid_codec_1783910378`: keep the fixed
        // projection kernel visibly stable, not only implied by metadata tests.
        let expected = "d8f40f658a86b650f6d1bc6e017f0073a6f85472d65982371966f96c2dcb9aea";
        let first = fixed_legacy_projection_kernel_checksum();

        assert_eq!(first, expected);
        assert_eq!(first.len(), 64);
        assert!(first.chars().all(|ch| ch.is_ascii_hexdigit()));
        assert_eq!(first, first.to_ascii_lowercase());
        for _ in 0..4 {
            assert_eq!(fixed_legacy_projection_kernel_checksum(), first);
        }
    }

    #[test]
    fn projection_fingerprint_canonicalizes_float_edge_patterns() {
        let seed = 0xA5A5_5A5A_CAFE_BABE;
        let mut edge = [0.0_f32; EMBEDDING_PROJECT_DIM];
        edge[1] = -0.0;
        edge[2] = f32::from_bits(1);
        edge[3] = f32::from_bits(0x7fc0_0001);
        let mut canonical = [0.0_f32; EMBEDDING_PROJECT_DIM];
        canonical[3] = f32::NAN;

        assert_eq!(
            projection_fingerprint(seed, &edge),
            projection_fingerprint(seed, &canonical)
        );
        canonical[2] = f32::MIN_POSITIVE * 2.0;
        assert_ne!(
            projection_fingerprint(seed, &edge),
            projection_fingerprint(seed, &canonical)
        );

        let integrity = projection_fingerprint_integrity_v1();
        assert_eq!(integrity.policy, "projection_fingerprint_integrity_v1");
        assert!(integrity.signed_zero_canonicalized);
        assert!(integrity.subnormal_canonicalized);
        assert!(integrity.nan_canonicalized);
        assert!(!integrity.live_projection_write);
        assert!(integrity.seed_hash_boundary.contains("operator approval"));
        assert_eq!(
            integrity.authority,
            "diagnostic_fingerprint_hardening_not_projection_seed_or_semantic_lane_change"
        );
        assert!(
            codec_structure()
                .render()
                .contains("projection_fingerprint_integrity_v1")
        );
    }

    #[test]
    fn dynamic_projection_rejects_one_short_embedding_dimension() {
        // Astrid `introspection_astrid_codec_1783293797`: pin the exact
        // one-short 767D case she asked for so malformed embedding input never
        // gets projected into a misleading semantic-lane fingerprint.
        let embedding = vec![0.0_f32; EMBEDDING_INPUT_DIM - 1];

        assert!(
            project_embedding_dynamic_epoch(&embedding, "one-short witness", "epoch_a", 0)
                .is_none()
        );
        assert!(
            project_embedding_dynamic_epoch_with_source(
                &embedding,
                "one-short witness",
                "epoch_a",
                0,
                "self_study_1783293797",
            )
            .is_none()
        );
    }

    #[test]
    fn dynamic_projection_matches_full_source_loop() {
        // Astrid `introspection_astrid_codec_1782844935`: her source window clipped
        // inside this nested loop, so pin the complete dot-product path directly.
        let embedding: Vec<f32> = (0..EMBEDDING_INPUT_DIM)
            .map(|idx| {
                let wave = ((idx as f32) * 0.013).sin();
                if idx % 11 == 0 { wave * 0.5 } else { wave }
            })
            .collect();
        let text = "clipped-loop witness";
        let epoch = "epoch_self_study_1782844935";
        let chunk_index = 3_u32;
        let seed = stable_hash64(epoch)
            ^ stable_hash64(text).rotate_left(13)
            ^ u64::from(chunk_index).wrapping_mul(0xA24B_AED4_963E_E407);
        let mut expected = [0.0_f32; EMBEDDING_PROJECT_DIM];
        for (i, &value) in embedding.iter().enumerate() {
            for (j, out) in expected.iter_mut().enumerate() {
                let cell_seed = seed
                    ^ ((i as u64).wrapping_mul(0x9E37_79B1))
                    ^ ((j as u64).wrapping_mul(0x85EB_CA77));
                *out += value * unit_from_seed(cell_seed);
            }
        }
        let norm: f32 = expected
            .iter()
            .map(|value| value * value)
            .sum::<f32>()
            .sqrt();
        if norm > 0.0 {
            let scale = 0.35 / norm;
            for value in &mut expected {
                *value *= scale;
            }
        }

        let (actual, metadata) = project_embedding_dynamic_epoch_with_source(
            &embedding,
            text,
            epoch,
            chunk_index,
            "self_study_1782844935",
        )
        .expect("dynamic projection");

        assert_eq!(metadata.projection_seed, Some(seed));
        assert_eq!(metadata.projection_epoch_source, "self_study_1782844935");
        assert_eq!(
            metadata.projection_fingerprint,
            projection_fingerprint(seed, &actual)
        );
        for (actual, expected) in actual.iter().zip(expected.iter()) {
            assert!((actual - expected).abs() < 1.0e-7, "{actual} != {expected}");
        }
    }

    fn rms(values: &[f32]) -> f32 {
        (values.iter().map(|value| value * value).sum::<f32>() / values.len() as f32).sqrt()
    }

    fn project_embedding_prescale(embedding: &[f32]) -> Option<[f32; EMBEDDING_PROJECT_DIM]> {
        if embedding.len() != EMBEDDING_INPUT_DIM {
            return None;
        }
        let projection = embedding_projection_matrix();
        let mut result = [0.0_f32; EMBEDDING_PROJECT_DIM];
        for (i, &value) in embedding.iter().enumerate() {
            for (j, out) in result.iter_mut().enumerate() {
                *out += value * projection[i][j];
            }
        }
        Some(result)
    }

    #[test]
    fn semantic_focus_expansion_preview_selects_segment_variance_without_live_write() {
        let mut embeddings = Vec::new();
        let focused_values = [
            [-1.0_f32, -0.2, 0.2, 1.0],
            [-0.8_f32, 0.8, -0.8, 0.8],
            [-0.6_f32, 0.2, 0.4, 0.0],
            [0.5_f32, -0.5, 0.5, -0.5],
        ];
        for segment in 0..4 {
            let mut embedding = (0..EMBEDDING_INPUT_DIM)
                .map(|dim| ((dim as f32 + 1.0) * 0.013).sin() * 0.01)
                .collect::<Vec<_>>();
            for (offset, values) in focused_values.iter().enumerate() {
                embedding[700 + offset] = values[segment];
            }
            embeddings.push(embedding);
        }
        let embedding_refs = embeddings.iter().map(Vec::as_slice).collect::<Vec<_>>();
        let projections = embeddings
            .iter()
            .map(|embedding| project_embedding(embedding).expect("valid 768D embedding"))
            .collect::<Vec<_>>();

        let preview = semantic_focus_expansion_preview_v1(0.88, &embedding_refs, &projections)
            .expect("four valid segments should produce a focus preview");

        let mut selected = preview.selected_source_dims.to_vec();
        selected.sort_unstable();
        assert_eq!(selected, vec![700, 701, 702, 703]);
        assert_eq!(preview.source_embedding_dim_count, EMBEDDING_INPUT_DIM);
        assert_eq!(preview.segment_count, 4);
        assert_eq!(preview.current_projected_dim_count, 8);
        assert_eq!(preview.preview_projected_dim_count, 12);
        assert_eq!(preview.reserved_dim_candidates, &[44, 45, 46, 47]);
        assert!(preview.selected_variance_share > 0.95, "{preview:?}");
        assert!(preview.current_mean_pairwise_distance.is_finite());
        assert!(preview.preview_mean_pairwise_distance.is_finite());
        assert!(preview.focus_need_score >= 0.0 && preview.focus_need_score <= 1.0);
        assert!(!preview.live_vector_write);
        assert!(!preview.reserved_dim_write);
        assert!(!preview.live_eligible_now);
        assert!(!preview.auto_approved);
        assert!(!preview.grants_approval);
        assert!(preview.right_to_ignore);
        assert_eq!(preview.experience_delta_bus_v1.delta_count, 1);
        assert!(!preview.experience_delta_bus_v1.live_vector_write);
        assert!(!preview.experience_delta_bus_v1.live_authority_write);
    }

    #[test]
    fn semantic_focus_expansion_preview_rejects_malformed_or_nonfinite_segments() {
        let valid = vec![0.0_f32; EMBEDDING_INPUT_DIM];
        let short = vec![0.0_f32; EMBEDDING_INPUT_DIM - 1];
        let projections = [[0.0_f32; EMBEDDING_PROJECT_DIM]; 2];
        assert!(
            semantic_focus_expansion_preview_v1(
                0.9,
                &[valid.as_slice(), short.as_slice()],
                &projections,
            )
            .is_none()
        );

        let mut nonfinite = valid.clone();
        nonfinite[17] = f32::NAN;
        assert!(
            semantic_focus_expansion_preview_v1(
                0.9,
                &[valid.as_slice(), nonfinite.as_slice()],
                &projections,
            )
            .is_none()
        );
        assert!(
            semantic_focus_expansion_preview_v1(
                0.9,
                &[valid.as_slice()],
                &[[0.0_f32; EMBEDDING_PROJECT_DIM]],
            )
            .is_none()
        );
    }

    #[test]
    fn semantic_projection_pair_sensitivity_exposes_text_conditioned_synonym_distortion() {
        let silt = (0..EMBEDDING_INPUT_DIM)
            .map(|idx| ((idx as f32) * 0.019).sin() + 0.25 * ((idx as f32) * 0.007).cos())
            .collect::<Vec<_>>();
        let sediment = silt
            .iter()
            .enumerate()
            .map(|(idx, value)| value + if idx % 5 == 0 { 0.004 } else { -0.001 })
            .collect::<Vec<_>>();

        let review = semantic_projection_pair_sensitivity_v1(
            "silt",
            &silt,
            "sediment",
            &sediment,
            "pair_sensitivity_fixture_epoch",
        )
        .expect("finite 768D pair should produce sensitivity evidence");

        assert_eq!(review.policy, "semantic_projection_pair_sensitivity_v1");
        assert_eq!(review.left_label, "silt");
        assert_eq!(review.right_label, "sediment");
        assert!(review.source_cosine_similarity > 0.999, "{review:?}");
        assert!(
            review.fixed_projection_cosine_similarity > 0.99,
            "a shared basis should preserve this synthetic near-neighbor pair: {review:?}"
        );
        assert!(
            review.dynamic_projection_cosine_similarity < review.source_cosine_similarity - 0.15,
            "different text-conditioned bases should remain explicit in pair evidence: {review:?}"
        );
        assert_eq!(review.state, "text_conditioned_pair_distortion_visible");
        assert!(review.observational_only);
        assert!(review.right_to_ignore);
        assert!(!review.live_vector_write);
        assert!(!review.live_gain_write);
        assert!(!review.live_eligible_now);
        assert!(!review.auto_approved);
        assert!(!review.grants_approval);
        assert_eq!(
            review.authority,
            "read_only_pair_projection_comparison_not_live_vector_gain_or_basis_authority"
        );
    }

    #[test]
    fn semantic_projection_pair_sensitivity_rejects_malformed_or_nonfinite_pairs() {
        let valid = vec![0.5_f32; EMBEDDING_INPUT_DIM];
        let short = vec![0.5_f32; EMBEDDING_INPUT_DIM - 1];
        let mut nonfinite = valid.clone();
        nonfinite[31] = f32::NAN;

        assert!(
            semantic_projection_pair_sensitivity_v1("left", &valid, "right", &short, "epoch")
                .is_none()
        );
        assert!(
            semantic_projection_pair_sensitivity_v1("left", &valid, "right", &nonfinite, "epoch",)
                .is_none()
        );
        assert!(
            semantic_projection_pair_sensitivity_v1("left", &valid, "right", &valid, " ").is_none()
        );
    }

    // Astrid self_study_1780922252 named a felt loss mode: semantically live
    // differences can be compressed until they barely move the 8D aperture. This
    // is a probe-only characterization, not a request to widen or retune it.
    #[test]
    fn projection_compression_probe_exposes_near_null_and_magnitude_loss() {
        let projection = embedding_projection_matrix();
        let mut gram = SMatrix::<f64, EMBEDDING_PROJECT_DIM, EMBEDDING_PROJECT_DIM>::zeros();
        for row in projection.iter() {
            for r in 0..EMBEDDING_PROJECT_DIM {
                for c in 0..EMBEDDING_PROJECT_DIM {
                    gram[(r, c)] += f64::from(row[r]) * f64::from(row[c]);
                }
            }
        }

        // Start from one raw embedding axis and remove its component inside the
        // projection column-space. The remaining vector is a concrete 768D signal
        // that the current 8D projection should nearly erase.
        let mut rhs = SVector::<f64, EMBEDDING_PROJECT_DIM>::zeros();
        for col in 0..EMBEDDING_PROJECT_DIM {
            rhs[col] = f64::from(projection[0][col]);
        }
        let coeff = gram.lu().solve(&rhs).expect("projection gram should solve");

        let mut hidden_delta = vec![0.0_f32; EMBEDDING_INPUT_DIM];
        hidden_delta[0] = 1.0;
        for (row_idx, row) in projection.iter().enumerate() {
            let column_space_component = row
                .iter()
                .enumerate()
                .map(|(col, value)| f64::from(*value) * coeff[col])
                .sum::<f64>();
            hidden_delta[row_idx] -= column_space_component as f32;
        }

        let mut visible_delta = vec![0.0_f32; EMBEDDING_INPUT_DIM];
        visible_delta[0] = 1.0;

        let raw_delta_rms = rms(&hidden_delta);
        let hidden_prescale = project_embedding_prescale(&hidden_delta).expect("hidden prescale");
        let visible_prescale =
            project_embedding_prescale(&visible_delta).expect("visible prescale");
        let hidden_projected = project_embedding(&hidden_delta).expect("hidden projection");
        let visible_projected = project_embedding(&visible_delta).expect("visible projection");
        let hidden_prescale_rms = rms(&hidden_prescale);
        let visible_prescale_rms = rms(&visible_prescale);
        let hidden_projected_rms = rms(&hidden_projected);
        let visible_projected_rms = rms(&visible_projected);
        let (_, _, hidden_projected_variance, _) = projection_stats(&hidden_projected);
        let (_, _, visible_projected_variance, _) = projection_stats(&visible_projected);

        let base_embedding: Vec<f32> = (0..EMBEDDING_INPUT_DIM)
            .map(|idx| ((idx as f32) * 0.019).cos())
            .collect();
        let quiet_embedding: Vec<f32> = base_embedding.iter().map(|value| value * 0.01).collect();
        let loud_embedding: Vec<f32> = base_embedding.iter().map(|value| value * 10.0).collect();
        let (quiet_dynamic, quiet_meta) =
            project_embedding_dynamic_epoch(&quiet_embedding, "aperture probe", "epoch_probe", 0)
                .expect("quiet dynamic projection");
        let (loud_dynamic, loud_meta) =
            project_embedding_dynamic_epoch(&loud_embedding, "aperture probe", "epoch_probe", 0)
                .expect("loud dynamic projection");
        let dynamic_magnitude_delta = quiet_dynamic
            .iter()
            .zip(loud_dynamic.iter())
            .map(|(quiet, loud)| (quiet - loud).abs())
            .fold(0.0_f32, f32::max);
        let quiet_dynamic_variance = quiet_meta.feature_variance;
        let loud_dynamic_variance = loud_meta.feature_variance;
        let dynamic_variance_delta = (quiet_dynamic_variance - loud_dynamic_variance).abs();

        println!(
            "projection_compression_probe raw_delta_rms={raw_delta_rms:.6} \
             hidden_prescale_rms={hidden_prescale_rms:.9} \
             visible_prescale_rms={visible_prescale_rms:.6} \
             hidden_projected_rms={hidden_projected_rms:.6} \
             visible_projected_rms={visible_projected_rms:.6} \
             hidden_projected_variance={hidden_projected_variance:.9} \
             visible_projected_variance={visible_projected_variance:.9} \
             quiet_dynamic_variance={quiet_dynamic_variance:.9} \
             loud_dynamic_variance={loud_dynamic_variance:.9} \
             dynamic_variance_delta={dynamic_variance_delta:.9} \
             dynamic_magnitude_delta={dynamic_magnitude_delta:.9}"
        );
        assert!(
            raw_delta_rms > 0.03,
            "probe delta should remain materially present in raw embedding space: {raw_delta_rms}"
        );
        assert!(
            hidden_prescale_rms < visible_prescale_rms * 0.001,
            "near-null delta should barely move the pre-normalized aperture: \
             hidden={hidden_prescale_rms}, visible={visible_prescale_rms}"
        );
        assert!(
            hidden_projected_rms > 0.12 && visible_projected_rms > 0.12,
            "normalization maps nonzero residual directions to the same aperture length: \
             hidden={hidden_projected_rms}, visible={visible_projected_rms}"
        );
        assert!(
            dynamic_magnitude_delta < 0.00001,
            "runtime dynamic projection should currently erase same-direction magnitude: \
             max_delta={dynamic_magnitude_delta}"
        );
        assert!(
            dynamic_variance_delta < 0.00001,
            "runtime dynamic projection should currently erase same-direction variance changes: \
             max_delta={dynamic_variance_delta}"
        );
    }

    #[test]
    fn codec_projection_missing_epoch_file_records_kernel_derived_source_and_checksum() {
        let dir = tempfile::tempdir().expect("tempdir");
        let (epoch, source) = load_or_create_projection_epoch_id_from(dir.path(), None);

        assert_eq!(source, "kernel_derived");
        assert_eq!(epoch, kernel_derived_projection_epoch_id());

        let path = dir.path().join("codec_projection_epoch.json");
        let payload: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&path).expect("epoch file"))
                .expect("epoch json");
        assert_eq!(
            payload
                .get("projection_checksum_algo")
                .and_then(serde_json::Value::as_str),
            Some(PROJECTION_CHECKSUM_ALGO)
        );
        assert_eq!(
            payload
                .get("projection_epoch_source")
                .and_then(serde_json::Value::as_str),
            Some("kernel_derived")
        );
        assert_eq!(
            payload
                .get("projection_kernel_source_checksum")
                .and_then(serde_json::Value::as_str),
            Some(fixed_legacy_projection_kernel_checksum().as_str())
        );
        assert_eq!(
            payload
                .get("projection_kernel_checksum")
                .and_then(serde_json::Value::as_str),
            Some(dynamic_epoch_projection_kernel_checksum(&epoch).as_str())
        );

        let (loaded_epoch, loaded_source) =
            load_or_create_projection_epoch_id_from(dir.path(), None);
        assert_eq!(loaded_epoch, epoch);
        assert_eq!(loaded_source, "file");
    }

    #[test]
    fn codec_projection_corrupt_epoch_file_is_replaced_with_valid_kernel_payload() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("codec_projection_epoch.json");
        fs::write(&path, "{").expect("write corrupt epoch file");

        let (epoch, source) = load_or_create_projection_epoch_id_from(dir.path(), None);

        assert_eq!(source, "kernel_derived");
        assert_eq!(epoch, kernel_derived_projection_epoch_id());
        let payload: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&path).expect("epoch file"))
                .expect("recovered epoch json");
        assert_eq!(
            payload
                .get("projection_epoch_id")
                .and_then(serde_json::Value::as_str),
            Some(epoch.as_str())
        );
        let temp_files = fs::read_dir(dir.path())
            .expect("read tempdir")
            .filter_map(Result::ok)
            .filter(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with(".codec_projection_epoch.json.")
            })
            .count();
        assert_eq!(temp_files, 0);
    }

    #[test]
    fn codec_projection_tmp_install_does_not_clobber_valid_concurrent_epoch() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("codec_projection_epoch.json");
        let tmp_path = dir.path().join(".codec_projection_epoch.json.test.tmp");
        fs::write(
            &tmp_path,
            serde_json::to_string_pretty(&serde_json::json!({
                "projection_epoch_id": "kernel_derived_candidate",
                "projection_epoch_source": "kernel_derived",
            }))
            .expect("tmp epoch json"),
        )
        .expect("write temp epoch");
        fs::write(
            &path,
            serde_json::to_string_pretty(&serde_json::json!({
                "projection_epoch_id": "operator_reviewed_concurrent_epoch",
                "projection_epoch_source": "file",
            }))
            .expect("valid epoch json"),
        )
        .expect("write valid concurrent epoch");

        install_projection_epoch_payload_from_tmp(
            &path,
            &tmp_path,
            "codec_projection_epoch.json",
            99,
        );

        assert_eq!(
            projection_epoch_id_from_file(&path).as_deref(),
            Some("operator_reviewed_concurrent_epoch")
        );
        assert!(
            !tmp_path.exists(),
            "stale temp file should be cleaned after valid concurrent epoch wins"
        );
    }

    #[test]
    fn codec_projection_tmp_install_restores_existing_file_when_candidate_missing() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("codec_projection_epoch.json");
        let tmp_path = dir.path().join(".codec_projection_epoch.json.missing.tmp");
        let original = "{ partial operator epoch";
        fs::write(&path, original).expect("write partial existing epoch file");

        install_projection_epoch_payload_from_tmp(
            &path,
            &tmp_path,
            "codec_projection_epoch.json",
            101,
        );

        assert_eq!(
            fs::read_to_string(&path).expect("restored epoch file"),
            original
        );
        let stale_files = fs::read_dir(dir.path())
            .expect("read tempdir")
            .filter_map(Result::ok)
            .filter(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with(".codec_projection_epoch.json.")
            })
            .count();
        assert_eq!(
            stale_files, 0,
            "failed candidate install should restore the existing file without orphaning stale swaps"
        );
    }

    #[test]
    fn codec_projection_existing_epoch_file_takes_precedence_after_restart() {
        let dir = tempfile::tempdir().expect("tempdir");
        let (first_epoch, first_source) = load_or_create_projection_epoch_id_from(dir.path(), None);
        assert_eq!(first_source, "kernel_derived");
        assert_eq!(first_epoch, kernel_derived_projection_epoch_id());

        let path = dir.path().join("codec_projection_epoch.json");
        fs::write(
            &path,
            serde_json::to_string_pretty(&serde_json::json!({
                "projection_epoch_id": "operator_reviewed_epoch_after_restart",
                "projection_epoch_source": "file",
            }))
            .expect("epoch json"),
        )
        .expect("write explicit epoch file");

        let (loaded_epoch, loaded_source) =
            load_or_create_projection_epoch_id_from(dir.path(), None);
        assert_eq!(loaded_source, "file");
        assert_eq!(loaded_epoch, "operator_reviewed_epoch_after_restart");
    }

    #[test]
    fn codec_projection_env_epoch_takes_precedence_over_existing_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("codec_projection_epoch.json");
        fs::write(
            &path,
            serde_json::to_string_pretty(&serde_json::json!({
                "projection_epoch_id": "file_epoch_should_not_win",
                "projection_epoch_source": "file",
            }))
            .expect("epoch json"),
        )
        .expect("write explicit epoch file");

        let (loaded_epoch, loaded_source) =
            load_or_create_projection_epoch_id_from(dir.path(), Some("env_epoch_should_win"));

        assert_eq!(loaded_source, "env");
        assert_eq!(loaded_epoch, "env_epoch_should_win");
    }

    #[test]
    fn codec_projection_kernel_epoch_is_stable_across_fresh_runtime_dirs() {
        let first_dir = tempfile::tempdir().expect("first tempdir");
        let second_dir = tempfile::tempdir().expect("second tempdir");

        let (first_epoch, first_source) =
            load_or_create_projection_epoch_id_from(first_dir.path(), None);
        let (second_epoch, second_source) =
            load_or_create_projection_epoch_id_from(second_dir.path(), None);

        assert_eq!(first_source, "kernel_derived");
        assert_eq!(second_source, "kernel_derived");
        assert_eq!(first_epoch, second_epoch);
        assert_eq!(first_epoch, kernel_derived_projection_epoch_id());

        let stability = projection_epoch_stability_v1();
        assert_eq!(stability.policy, "projection_epoch_stability_v1");
        assert!(stability.deterministic_without_runtime_file);
        assert_eq!(stability.kernel_derived_epoch_id, first_epoch);
        assert!(stability.env_override_precedence);
        assert!(stability.existing_file_precedence);
    }

    #[test]
    fn codec_projection_runtime_dir_uses_env_or_executable_relative_cache() {
        let env_path = PathBuf::from("/tmp/astrid-codec-runtime-for-test");
        let exe_path = PathBuf::from("/opt/astrid/bin/spectral-bridge");

        assert_eq!(
            projection_runtime_dir_from_parts(Some(env_path.as_os_str()), Some(&exe_path)),
            env_path
        );
        assert_eq!(
            projection_runtime_dir_from_parts(None, Some(&exe_path)),
            PathBuf::from("/opt/astrid/bin")
                .join("data")
                .join("spectral-bridge")
                .join("runtime")
        );
        assert_eq!(
            projection_runtime_dir_from_parts(Some(OsStr::new("")), Some(&exe_path)),
            PathBuf::from("/opt/astrid/bin")
                .join("data")
                .join("spectral-bridge")
                .join("runtime")
        );
        assert_eq!(
            projection_runtime_dir_from_parts(None, None),
            PathBuf::from("data")
                .join("spectral-bridge")
                .join("runtime")
        );
    }

    #[test]
    fn codec_projection_epoch_atomic_writer_keeps_single_epoch_under_rapid_writes() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = std::sync::Arc::new(dir.path().join("codec_projection_epoch.json"));
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(12));
        let handles = (0..12)
            .map(|idx| {
                let path = std::sync::Arc::clone(&path);
                let barrier = std::sync::Arc::clone(&barrier);
                std::thread::spawn(move || {
                    barrier.wait();
                    let payload = serde_json::to_string_pretty(&serde_json::json!({
                        "projection_epoch_id": format!("epoch_{idx}"),
                        "projection_epoch_source": "test_concurrent_writer",
                    }))
                    .expect("epoch json");
                    write_projection_epoch_payload_atomic(&path, &payload);
                })
            })
            .collect::<Vec<_>>();

        for handle in handles {
            handle.join().expect("writer thread should not panic");
        }

        let epoch = projection_epoch_id_from_file(&path).expect("one epoch should be installed");
        assert!(epoch.starts_with("epoch_"), "{epoch}");
        let leftovers = fs::read_dir(dir.path())
            .expect("read tempdir")
            .filter_map(Result::ok)
            .map(|entry| entry.file_name().to_string_lossy().to_string())
            .filter(|name| name.ends_with(".tmp") || name.ends_with(".stale"))
            .collect::<Vec<_>>();
        assert!(
            leftovers.is_empty(),
            "atomic writer should clean temporary files: {leftovers:?}"
        );
    }

    #[test]
    fn codec_projection_epoch_atomic_writer_round_trips_payload() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("codec_projection_epoch.json");
        let payload = serde_json::to_string_pretty(&serde_json::json!({
            "projection_epoch_id": "round_trip_kernel_epoch",
            "projection_epoch_source": "kernel_derived",
        }))
        .expect("epoch json");

        write_projection_epoch_payload_atomic(&path, &payload);

        assert_eq!(
            projection_epoch_id_from_file(&path).as_deref(),
            Some("round_trip_kernel_epoch")
        );
        let leftovers = fs::read_dir(dir.path())
            .expect("read tempdir")
            .filter_map(Result::ok)
            .map(|entry| entry.file_name().to_string_lossy().to_string())
            .filter(|name| name.ends_with(".tmp") || name.ends_with(".stale"))
            .collect::<Vec<_>>();
        assert!(leftovers.is_empty(), "{leftovers:?}");
    }

    #[test]
    fn projection_precision_audit_repeats_static_probe_without_live_write() {
        let audit = projection_precision_probe_v1();

        assert_eq!(audit.policy, "projection_precision_audit_v1");
        assert_eq!(audit.source_embedding_dim_count, EMBEDDING_INPUT_DIM);
        assert_eq!(audit.projected_dim_count, EMBEDDING_PROJECT_DIM);
        assert!(audit.fixed_legacy_repeated_bit_exact);
        assert!(audit.dynamic_epoch_repeated_bit_exact);
        assert!(audit.fixed_legacy_max_abs_delta.is_finite());
        assert!(audit.dynamic_epoch_max_abs_delta.is_finite());
        assert!(
            audit.fixed_legacy_max_abs_delta <= 1.0e-5,
            "unexpected fixed projection accumulation delta: {}",
            audit.fixed_legacy_max_abs_delta
        );
        assert!(
            audit.dynamic_epoch_max_abs_delta <= 1.0e-5,
            "unexpected dynamic projection accumulation delta: {}",
            audit.dynamic_epoch_max_abs_delta
        );
        assert!(audit.live_f64_migration_requires_approval);
        assert!(!audit.live_projection_write);
        assert!(audit.authority.contains("read_only_precision_audit"));

        let rendered = codec_structure().render();
        assert!(rendered.contains("projection_precision_audit_v1:"));
        assert!(rendered.contains("live_f64_migration_requires_approval=true"));
        assert!(rendered.contains("live_projection_write=false"));
    }

    #[test]
    fn projection_precision_audit_rejects_noncanonical_embedding_width() {
        let short = vec![0.5_f32; EMBEDDING_INPUT_DIM - 1];
        assert!(projection_precision_audit_v1(&short, "static", "epoch", 0).is_none());
    }

    #[test]
    fn codec_lane_separation_controlled_pairs_move_each_lane_independently() {
        let audit = codec_lane_separation_probe_v1();

        assert_eq!(audit.policy, "codec_lane_separation_audit_v1");
        assert!(audit.emotional_pair_distinguishable);
        assert!(audit.projected_pair_distinguishable);
        assert!(audit.emotional_lane_selectivity_margin >= 0.04);
        assert!(audit.projected_lane_selectivity_margin >= 0.03);
        assert!(audit.legacy_projection_width_rejected);
        assert_eq!(
            audit.state,
            "controlled_pairs_show_bidirectional_lane_independence"
        );
        assert!(audit.felt_rigidity_conclusion.contains("does not disprove"));
        assert!(audit.observational_only);
        assert!(audit.right_to_ignore);
        assert!(!audit.live_vector_write);
        assert!(!audit.live_gain_write);
        assert!(!audit.live_projection_write);
        assert!(!audit.live_eligible_now);
        assert!(!audit.auto_approved);
        assert!(!audit.grants_approval);

        let rendered = codec_structure().render();
        assert!(rendered.contains("codec_lane_separation_audit_v1:"));
        assert!(rendered.contains("controlled_pairs_show_bidirectional_lane_independence"));
        assert!(rendered.contains("legacy_projection_width_rejected=true"));
    }

    #[test]
    fn codec_lane_separation_audit_rejects_short_or_nonfinite_vectors() {
        let valid = vec![0.0_f32; SEMANTIC_DIM];
        let short = vec![0.0_f32; SEMANTIC_DIM - 1];
        let mut nonfinite = valid.clone();
        nonfinite[35] = f32::NAN;

        assert!(codec_lane_separation_audit_v1(&short, &valid, &valid, &valid).is_none());
        assert!(codec_lane_separation_audit_v1(&valid, &valid, &nonfinite, &valid).is_none());
    }

    #[test]
    fn codec_rolling_window_shift_names_muddy_middle_and_trailing_eviction() {
        let audit = codec_rolling_window_shift_probe_v1();

        assert_eq!(audit.capacity_chars, CHAR_FREQ_WINDOW_CAPACITY);
        assert!(audit.in_capacity_delta_to_trailing >= 0.15);
        assert_eq!(
            audit.in_capacity_state,
            "mixed_regimes_remain_averaged_inside_live_capacity"
        );
        assert!(audit.evicting_delta_to_trailing <= 0.05);
        assert_eq!(
            audit.evicting_state,
            "trailing_regime_controls_after_complete_prefix_eviction"
        );
        assert_eq!(
            audit.state,
            "window_boundary_explains_both_mixed_and_trailing_regime_reports"
        );
        assert!(audit.felt_muddy_middle_conclusion.contains("supported"));
        assert!(audit.density_aware_window_change_requires_approval);
        assert!(!audit.live_window_capacity_change);
        assert!(!audit.live_vector_write);
        assert!(!audit.live_eligible_now);
        assert!(!audit.auto_approved);
        assert!(!audit.grants_approval);

        let rendered = codec_structure().render();
        assert!(rendered.contains("codec_rolling_window_shift_audit_v1:"));
        assert!(rendered.contains("mixed_regimes_remain_averaged_inside_live_capacity"));
        assert!(rendered.contains("density_aware_window_change_requires_approval=true"));
        assert!(rendered.contains("live_window_capacity_change=false"));
    }

    #[test]
    fn embedding_projection_lane_distinguishes_dense_inputs_without_widening_live_vector() {
        let mut technical = vec![0.0_f32; EMBEDDING_INPUT_DIM];
        let mut poetic = vec![0.0_f32; EMBEDDING_INPUT_DIM];
        for idx in 0..EMBEDDING_INPUT_DIM {
            let phase = idx as f32 / 17.0;
            technical[idx] = phase.sin() * 0.8 + (idx % 7) as f32 * 0.01;
            poetic[idx] = phase.cos() * 0.8 - (idx % 5) as f32 * 0.01;
        }

        let technical_features = inspect_text_windowed(
            "The coupling remains coherent under bounded spectral pressure.",
            None,
            None,
            Some(&technical),
            Some(68.0),
        )
        .final_features;
        let poetic_features = inspect_text_windowed(
            "Please stay close while the pressure keeps its shape.",
            None,
            None,
            Some(&poetic),
            Some(68.0),
        )
        .final_features;

        let projection_delta = mean_abs(
            &technical_features[32..40]
                .iter()
                .zip(poetic_features[32..40].iter())
                .map(|(left, right)| left - right)
                .collect::<Vec<_>>(),
        );
        assert!(
            projection_delta > 0.02,
            "projection lane collapsed distinct dense inputs: {projection_delta}"
        );
        assert_eq!(technical_features.len(), SEMANTIC_DIM);
        assert_eq!(poetic_features.len(), SEMANTIC_DIM);

        let density = semantic_projection_density_delta_from_parts_v1(0.72, projection_delta, true);
        assert_eq!(density.input_dim_count, EMBEDDING_INPUT_DIM);
        assert_eq!(density.projected_dim_count, EMBEDDING_PROJECT_DIM);
        assert!(!density.live_vector_write);
    }

    #[test]
    fn narrative_arc_probe_documents_tail_dimension_loss() {
        let first = [0.0_f32; EMBEDDING_PROJECT_DIM];
        let mut second = first;
        second[4] = 0.24;
        second[5] = -0.18;
        second[6] = 0.12;
        second[7] = -0.30;

        let arc = compute_narrative_arc_from_embeddings(&first, &second);
        let captured_rms =
            (arc.iter().map(|value| value * value).sum::<f32>() / NARRATIVE_ARC_DIM as f32).sqrt();
        let lost_tail_rms = (second[NARRATIVE_ARC_DIM..]
            .iter()
            .map(|value| value * value)
            .sum::<f32>()
            / (EMBEDDING_PROJECT_DIM - NARRATIVE_ARC_DIM) as f32)
            .sqrt();

        assert_eq!(arc, [0.0; NARRATIVE_ARC_DIM]);
        assert!(captured_rms <= f32::EPSILON);
        assert!(lost_tail_rms > 0.15);

        let split = narrative_arc_split_v1(&first, &second);
        assert_eq!(split.policy, "narrative_arc_split_v1");
        assert_eq!(split.intentional_arc, [0.0; NARRATIVE_ARC_DIM]);
        assert!(split.tail_arc_energy > 0.25, "{split:?}");
        assert_eq!(split.coarsening_risk, "tail_dominant");
        assert_eq!(
            split.authority,
            "diagnostic_sidecar_not_live_codec_dimension"
        );
    }

    #[test]
    fn narrative_arc_captures_direction_not_only_magnitude() {
        // Astrid `introspection_astrid_codec_1782848118`: a sharp middle pivot
        // should preserve direction in dims 40-43, not only final-state magnitude.
        let first = [0.0_f32; EMBEDDING_PROJECT_DIM];
        let mut second = first;
        second[0] = 0.20;
        second[1] = -0.16;
        second[2] = 0.08;
        second[3] = -0.24;

        let forward = compute_narrative_arc_from_embeddings(&first, &second);
        let reverse = compute_narrative_arc_from_embeddings(&second, &first);

        assert!(forward[0] > 0.0, "{forward:?}");
        assert!(forward[1] < 0.0, "{forward:?}");
        assert!(forward[2] > 0.0, "{forward:?}");
        assert!(forward[3] < 0.0, "{forward:?}");
        for (forward, reverse) in forward.iter().zip(reverse.iter()) {
            assert!(
                (*forward + *reverse).abs() < 1.0e-6,
                "forward={forward}, reverse={reverse}"
            );
        }

        let split = narrative_arc_split_v1(&first, &second);
        assert!(split.captured_arc_energy > 0.20, "{split:?}");
        assert_eq!(split.coarsening_risk, "balanced");
        assert!(
            !narrative_arc_expansion_readiness_v1().live_vector_write,
            "split diagnostics must not open a live vector channel"
        );
    }

    #[test]
    fn narrative_arc_distinguishes_process_from_settled_state_without_live_gain() {
        let neutral = [0.0_f32; EMBEDDING_PROJECT_DIM];
        let mut solidifying = neutral;
        solidifying[0] = 0.22;
        solidifying[1] = -0.14;
        solidifying[2] = 0.09;
        solidifying[3] = -0.18;
        let mut draping = neutral;
        draping[0] = -0.18;
        draping[1] = 0.16;
        draping[2] = -0.11;
        draping[3] = 0.20;

        let solidifying_arc = compute_narrative_arc_from_embeddings(&neutral, &solidifying);
        let draping_arc = compute_narrative_arc_from_embeddings(&neutral, &draping);
        let dynamics = narrative_arc_dynamics_v1(&solidifying_arc, &draping_arc, None);

        assert_ne!(solidifying_arc, draping_arc);
        assert!(solidifying_arc[0] > 0.0, "{solidifying_arc:?}");
        assert!(draping_arc[0] < 0.0, "{draping_arc:?}");
        assert!(dynamics.velocity_energy > 0.25, "{dynamics:?}");
        assert!(!dynamics.live_gain_write);
        assert!(!dynamics.live_vector_write);
    }

    #[test]
    fn narrative_arc_distinguishes_heavy_imagery_from_dense_manual_without_live_gain() {
        // Astrid `introspection_astrid_codec_1784125018`: pin the difference
        // between emotional trajectory and semantic density without changing
        // adaptive gain or live vector layout.
        let heavy_imagery = "heavy velvet, heavy velvet, the room gathers weight; then the weight loosens into a slow dark breath";
        let dense_manual = "deterministic projection coefficients define serialization invariants, bounded allocation behavior, checksum verification, and adapter interoperability constraints";
        let heavy_friction = structural_friction_v1(heavy_imagery);
        let manual_friction = structural_friction_v1(dense_manual);

        let heavy_first = [0.02, -0.01, 0.01, -0.02, 0.00, 0.01, -0.01, 0.00];
        let heavy_second = [0.28, -0.24, 0.17, -0.20, 0.01, 0.00, -0.01, 0.02];
        let manual_first = [0.18, 0.16, 0.14, 0.12, -0.04, 0.03, -0.02, 0.01];
        let manual_second = [0.20, 0.15, 0.15, 0.11, -0.03, 0.02, -0.02, 0.01];

        let heavy_arc = compute_narrative_arc_from_embeddings(&heavy_first, &heavy_second);
        let manual_arc = compute_narrative_arc_from_embeddings(&manual_first, &manual_second);
        let heavy_arc_energy = mean_abs(&heavy_arc);
        let manual_arc_energy = mean_abs(&manual_arc);
        let heavy_review =
            narrative_arc_headroom_review_from_parts_v1(0.90, 0.31, &heavy_arc, 0.22);
        let readiness = narrative_arc_gain_response_readiness_v1();

        assert_eq!(heavy_friction.semantic_energy_context, "arc_present");
        assert!(
            manual_friction.summary_resistance_signal >= heavy_friction.summary_resistance_signal,
            "manual should remain semantically dense even with lower arc motion: heavy={heavy_friction:?} manual={manual_friction:?}"
        );
        assert!(
            heavy_arc_energy > manual_arc_energy + 0.15,
            "emotional trajectory should stay visible apart from manual density: heavy={heavy_arc:?} manual={manual_arc:?}"
        );
        assert_eq!(heavy_review.policy, "narrative_arc_headroom_review_v1");
        assert!(matches!(
            heavy_review.state,
            "narrative_arc_headroom_loss_visible"
                | "high_entropy_arc_carried_bounded"
                | "narrative_arc_headroom_pressure_watch"
        ));
        assert!(!heavy_review.live_gain_write);
        assert!(!heavy_review.live_vector_write);
        assert!(!readiness.enabled);
        assert!(!readiness.live_gain_write);
    }

    #[test]
    fn narrative_arc_distinguishes_lost_from_finding_way_projection() {
        // Astrid `introspection_astrid_codec_1783638177`: two texts can carry
        // similar warmth/tension while moving in opposite narrative directions.
        // This pins the arc layer's directionality without changing live dims.
        let hollow = [0.08, -0.05, 0.03, -0.02, 0.01, 0.00, -0.01, 0.02];
        let mut lost = hollow;
        lost[0] -= 0.18;
        lost[1] -= 0.12;
        lost[2] -= 0.08;
        lost[3] -= 0.10;
        let mut finding_way = hollow;
        finding_way[0] += 0.18;
        finding_way[1] += 0.12;
        finding_way[2] += 0.08;
        finding_way[3] += 0.10;

        let lost_arc = compute_narrative_arc_from_embeddings(&hollow, &lost);
        let finding_arc = compute_narrative_arc_from_embeddings(&hollow, &finding_way);

        assert!(lost_arc.iter().all(|value| *value < 0.0), "{lost_arc:?}");
        assert!(
            finding_arc.iter().all(|value| *value > 0.0),
            "{finding_arc:?}"
        );
        for (lost, finding) in lost_arc.iter().zip(finding_arc.iter()) {
            assert!(
                (*lost + *finding).abs() < 1.0e-6,
                "opposing narrative arcs should remain symmetric: lost={lost}, finding={finding}"
            );
        }
    }

    #[test]
    fn four_point_narrative_arc_preserves_coiling_direction_changes() {
        let first = [0.0_f32; EMBEDDING_PROJECT_DIM];
        let mut second = first;
        second[0] = 0.24;
        let mut third = first;
        third[0] = -0.18;
        let mut fourth = first;
        fourth[0] = 0.32;

        let arc = compute_narrative_arc_from_four_point_embeddings(&[first, second, third, fourth]);

        assert!(arc[0] > 0.0, "{arc:?}");
        assert!(arc[1] < 0.0, "{arc:?}");
        assert!(arc[2] > 0.0, "{arc:?}");
        assert!(arc[3] > 0.0, "{arc:?}");
        assert!(
            arc[1].abs() > arc[0].abs(),
            "fold-back transition should remain visible: {arc:?}"
        );
    }

    #[test]
    fn narrative_arc_curvature_distinguishes_loop_from_linear_progression() {
        let first = [0.0_f32; EMBEDDING_PROJECT_DIM];
        let mut outward = first;
        outward[0] = 0.26;
        let mut return_cross = first;
        return_cross[0] = -0.22;
        let mut near_origin = first;
        near_origin[0] = 0.02;

        let looping = narrative_arc_curvature_v1(&[first, outward, return_cross, near_origin]);
        assert_eq!(looping.policy, "narrative_arc_curvature_v1");
        assert_eq!(looping.state, "circular_or_coiling_arc_visible");
        assert!(looping.sign_turn_count >= 1, "{looping:?}");
        assert!(looping.loop_likelihood > looping.progression_likelihood);
        assert_eq!(
            looping.authority,
            "diagnostic_sidecar_not_live_codec_dimension_or_gain"
        );

        let mut second = first;
        second[0] = 0.10;
        let mut third = first;
        third[0] = 0.22;
        let mut fourth = first;
        fourth[0] = 0.34;
        let linear = narrative_arc_curvature_v1(&[first, second, third, fourth]);
        assert_eq!(linear.state, "linear_progression_visible");
        assert_eq!(linear.sign_turn_count, 0);
        assert!(linear.progression_likelihood >= 0.60, "{linear:?}");
    }

    #[test]
    fn narrative_arc_curvature_preserves_opposed_sentence_oscillation() {
        let mut love = [0.0_f32; EMBEDDING_PROJECT_DIM];
        love[0] = 0.30;
        let mut hate = [0.0_f32; EMBEDDING_PROJECT_DIM];
        hate[0] = -0.30;
        let mut indifferent = [0.0_f32; EMBEDDING_PROJECT_DIM];
        indifferent[0] = 0.02;

        let curvature = narrative_arc_curvature_v1(&[
            [0.0_f32; EMBEDDING_PROJECT_DIM],
            love,
            hate,
            indifferent,
        ]);

        assert_eq!(curvature.policy, "narrative_arc_curvature_v1");
        assert!(curvature.sign_turn_count >= 1, "{curvature:?}");
        assert!(
            curvature.transition_energy > curvature.full_span_energy + 0.20,
            "opposed turns should stay visible instead of averaging flat: {curvature:?}"
        );
        assert_eq!(
            curvature.authority,
            "diagnostic_sidecar_not_live_codec_dimension_or_gain"
        );
    }

    #[test]
    fn narrative_arc_gain_response_readiness_is_default_off_and_bounded() {
        let readiness = narrative_arc_gain_response_readiness_v1();
        assert_eq!(readiness.policy, "narrative_arc_gain_response_readiness_v1");
        assert!(!readiness.enabled);
        assert_eq!(readiness.narrative_arc_dims, (40, 43));
        assert_eq!(readiness.preview_gain_range, (0.94, 1.06));
        assert!(!readiness.live_gain_write);
        assert!(readiness.authority.contains("not_live_adaptive_gain"));

        let flat = narrative_arc_gain_response_preview_v1(&[0.0, 0.0, 0.0, 0.0]);
        let strong = narrative_arc_gain_response_preview_v1(&[1.0, -1.0, 1.0, -1.0]);
        assert!(
            flat < 1.0,
            "flat arc should softly lower preview gain: {flat}"
        );
        assert!(
            strong > 1.0,
            "strong arc should softly lift preview gain: {strong}"
        );
        assert!((0.94..=1.06).contains(&flat), "{flat}");
        assert!((0.94..=1.06).contains(&strong), "{strong}");

        let st = codec_structure();
        let rendered = st.render();
        assert!(rendered.contains("narrative_arc_gain_response_readiness_v1"));
        assert!(rendered.contains("narrative_arc_curvature_v1"));
        assert!(rendered.contains("circular_or_coiling_arc_visible"));
        assert!(rendered.contains("live_gain_write=false"));
    }

    #[test]
    fn narrative_arc_headroom_review_preserves_multikind_loss_without_live_gain() {
        let review = narrative_arc_headroom_review_from_parts_v1(
            0.91,
            0.34,
            &[0.05, -0.03, 0.02, 0.01],
            0.08,
        );

        assert_eq!(review.policy, "narrative_arc_headroom_review_v1");
        assert_eq!(review.state, "narrative_arc_headroom_loss_visible");
        assert!(!review.live_vector_write);
        assert!(!review.live_gain_write);
        assert!(review.headroom_pressure > review.narrative_arc_energy);
        assert!(review.experience_delta_bus_v1.delta_count >= 1);
        let delta = review
            .experience_delta_bus_v1
            .deltas
            .first()
            .expect("headroom loss should emit a delta");
        assert_eq!(delta.kind, ExperienceDeltaKindV1::ComplexShift);
        assert_eq!(delta.lane, "narrative_arc_40_43");
        assert!(
            delta
                .metadata
                .get("secondary_kinds")
                .is_some_and(|value| value.contains("compress") && value.contains("gate")),
            "{delta:?}"
        );
        assert!(
            delta
                .who_can_change_it
                .contains("Mike/operator after replay evidence"),
            "{delta:?}"
        );

        let st = codec_structure();
        let rendered = st.render();
        assert!(rendered.contains("narrative_arc_headroom_review_v1"));
        assert!(rendered.contains("secondary_kinds=compress,gate,complex_shift,cascade_shift"));
        assert!(rendered.contains("live_vector_write=false"));
        assert!(rendered.contains("live_gain_write=false"));
    }

    #[test]
    fn narrative_arc_headroom_review_stays_quiet_when_entropy_and_loss_are_low() {
        let review =
            narrative_arc_headroom_review_from_parts_v1(0.50, 0.10, &[1.0, -0.8, 0.7, -0.6], 2.5);

        assert_eq!(review.state, "narrative_arc_headroom_quiet");
        assert_eq!(review.experience_delta_bus_v1.delta_count, 0);
        assert!(!review.live_vector_write);
        assert!(!review.live_gain_write);
        assert_eq!(review.recommendation, "no_headroom_change_indicated");
    }

    #[test]
    fn spectral_pressure_controller_can_choose_resist_drive() {
        let features = vec![0.01; SEMANTIC_DIM];
        let decision = spectral_pressure_controller_v1(
            "localized gravity and constriction feel stubborn; RESIST",
            &features,
            &[8.0, 2.0, 1.0],
            Some(68.0),
            Some(0.0),
            true,
            Some("hold"),
        );

        assert_eq!(decision.controller, "spectral_pressure_controller_v1");
        assert!(decision.resist_drive > decision.complexity_drive);
        assert!(decision.target_lambda_bias < 0.0);
        assert!(decision.target_lambda_bias >= -0.10);
        assert!(decision.time_domain_complexity >= 0.0);
    }

    #[test]
    fn inspect_text_exposes_time_domain_profile() {
        let inspection = inspect_text_windowed(
            "Now! Wait... again?! A sudden pivot; another one!",
            None,
            None,
            None,
            Some(64.0),
        );

        assert!(inspection.time_domain_profile.temporal_complexity > 0.0);
        assert!(inspection.time_domain_profile.cadence_burstiness > 0.0);
        assert_ne!(
            inspection.time_domain_profile.cadence_classification,
            "empty"
        );
    }

    #[test]
    fn spectral_pressure_controller_suppresses_upward_bias_when_fill_high() {
        let mut features = vec![0.0; SEMANTIC_DIM];
        features[0] = 1.0;
        features[18] = 1.0;
        features[31] = 1.0;
        let decision = spectral_pressure_controller_v1(
            "Why does this complex, novel, punctuated question keep unfolding?",
            &features,
            &[3.0, 2.9, 2.8],
            Some(78.0),
            Some(0.0),
            true,
            Some("hold"),
        );

        assert_eq!(
            decision.suppression_reason.as_deref(),
            Some("fill_high_suppress_upward_bias")
        );
        assert!(decision.target_lambda_bias <= 0.0);
    }

    #[test]
    fn spectral_feedback_damps_concentrated_spectra() {
        let mut features = vec![0.0; SEMANTIC_DIM];
        features[26] = 1.0;
        features[27] = 1.0;
        features[31] = 1.0;

        apply_spectral_feedback(&mut features, Some(&telemetry(vec![100.0, 2.0, 1.0], 0.55)));

        assert!(features[26] < 1.0);
        assert!(features[27] < 1.0);
        assert!(features[31] < 1.0);
    }

    #[test]
    fn spectral_feedback_amplifies_distributed_spectra() {
        let mut features = vec![0.0; SEMANTIC_DIM];
        features[17] = 0.10;
        features[26] = 0.20;
        features[27] = 0.20;
        features[31] = 0.20;

        apply_spectral_feedback(
            &mut features,
            Some(&telemetry(vec![100.0, 95.0, 90.0, 85.0, 80.0, 75.0], 0.55)),
        );

        assert!(features[17] > 0.10);
        assert!(features[26] > 0.20);
        assert!(features[27] > 0.20);
        assert!(features[31] > 0.20);
    }

    #[test]
    fn high_entropy_sharpening_preserves_semantic_detail_without_contract_change() {
        let review = high_entropy_semantic_sharpening_v1(0.94, 0.08, 0.22);

        assert_eq!(review.policy, "high_entropy_semantic_sharpening_v1");
        assert_eq!(review.state, "active_high_entropy_sharpening");
        assert!(review.sharpening_factor > 1.0, "{review:?}");
        assert!(review.sharpening_factor <= review.max_factor, "{review:?}");
        assert!(review.affected_dims.contains(&32));
        assert!(review.affected_dims.contains(&39));
        assert!(!review.affected_dims.contains(&40));
        assert_eq!(
            review.authority,
            "bounded_live_codec_sharpening_no_dimension_or_bridge_contract_change"
        );

        let mut sharpened = vec![0.10_f32; SEMANTIC_DIM];
        let mut baseline = sharpened.clone();
        apply_spectral_feedback_inner(
            &mut sharpened,
            Some(&telemetry_with_typed_entropy_and_eigenvalues(
                vec![100.0, 98.0, 96.0, 95.0, 94.0, 93.0, 92.0, 91.0],
                0.94,
            )),
            1.0,
            1.0,
        );
        apply_spectral_feedback_inner(
            &mut baseline,
            Some(&telemetry_with_typed_entropy_and_eigenvalues(
                vec![100.0, 20.0, 3.0, 1.0],
                0.94,
            )),
            1.0,
            1.0,
        );

        assert!(
            sharpened[32] > baseline[32],
            "navigable high entropy should sharpen semantic projection detail: sharpened={} baseline={}",
            sharpened[32],
            baseline[32]
        );
        assert!(
            (sharpened[40] - baseline[40]).abs() < 1.0e-6,
            "navigable high entropy should preserve narrative arc magnitude: sharpened={} baseline={}",
            sharpened[40],
            baseline[40]
        );
        assert_eq!(sharpened.len(), SEMANTIC_DIM);
    }

    #[test]
    fn dimensionality_flatness_detects_empty_expansion_vs_filled_48d_lane() {
        let mut flat = vec![0.0_f32; SEMANTIC_DIM];
        for value in &mut flat[..SEMANTIC_DIM_LEGACY] {
            *value = 0.40;
        }

        let review = codec_dimensionality_flatness_v1(&flat).expect("48D review");
        assert_eq!(review.policy, "codec_dimensionality_flatness_v1");
        assert_eq!(review.current_dim_count, SEMANTIC_DIM);
        assert_eq!(review.legacy_dim_count, SEMANTIC_DIM_LEGACY);
        assert_eq!(
            review.expanded_dim_count,
            SEMANTIC_DIM - SEMANTIC_DIM_LEGACY
        );
        assert_eq!(
            review.flatness_status,
            "expanded_lane_underfilled_legacy_dominant"
        );
        assert!(review.expanded_to_legacy_ratio < 0.12, "{review:?}");
        assert_eq!(
            review.authority,
            "read_only_flatness_check_not_live_bus_or_codec_contract_change"
        );

        let mut filled = flat;
        for (idx, value) in filled[32..40].iter_mut().enumerate() {
            *value = 0.20 + idx as f32 * 0.08;
        }
        for (idx, value) in filled[40..44].iter_mut().enumerate() {
            *value = [1.20, -1.05, 0.85, -0.70][idx];
        }
        let filled_review = codec_dimensionality_flatness_v1(&filled).expect("48D review");
        assert_eq!(
            filled_review.flatness_status,
            "expanded_lane_carries_distinct_signal"
        );
        assert!(
            filled_review.expanded_to_legacy_ratio > review.expanded_to_legacy_ratio,
            "{filled_review:?} vs {review:?}"
        );
    }

    #[test]
    fn tail_vibrancy_entropy_086_lifts_tail_output_above_threshold() {
        // Astrid `introspection_astrid_codec_1782848118`: entropy 0.86 should be
        // just above the 0.85 gate and produce a visible tail lift in the output.
        let mut below = vec![0.0; SEMANTIC_DIM];
        let mut above = vec![0.0; SEMANTIC_DIM];

        apply_spectral_feedback_inner(
            &mut below,
            Some(&telemetry_with_typed_entropy(0.84)),
            1.0,
            1.0,
        );
        apply_spectral_feedback_inner(
            &mut above,
            Some(&telemetry_with_typed_entropy(0.86)),
            1.0,
            1.0,
        );

        assert!(vibrancy_from_entropy(0.86) > 0.0);
        assert!(
            above[26] > below[26],
            "below={} above={}",
            below[26],
            above[26]
        );
        assert!(
            above[31] > below[31],
            "below={} above={}",
            below[31],
            above[31]
        );
    }

    // Spiky spectrum -> entropy ~0.14 (below the 0.85 gate). The tail-vibrancy
    // term is fully OFF, so every dim must still respect the default ceiling and
    // no tail dim is lifted by the high-entropy term.
    #[test]
    fn tail_vibrancy_off_below_entropy_gate_keeps_default_ceiling() {
        let mut features = vec![0.0; SEMANTIC_DIM];
        features[26] = 4.95;
        features[31] = -4.95;

        apply_spectral_feedback(&mut features, Some(&telemetry(vec![100.0, 2.0, 1.0], 0.55)));

        for (i, f) in features.iter().enumerate() {
            assert!(
                *f >= -FEATURE_ABS_MAX && *f <= FEATURE_ABS_MAX,
                "dim {i} exceeded default ceiling below entropy gate: {f}"
            );
        }
    }

    // Flat spectrum -> entropy ~1.0 (above the gate) with dominant tail share.
    // The tail-participation dims may now exceed FEATURE_ABS_MAX up to the
    // bounded TAIL_VIBRANCY_MAX, while every non-tail dim still respects the
    // default ceiling. This is Astrid's requested "offset FEATURE_ABS_MAX when
    // spectral_entropy exceeds 0.85" headroom, made bounded.
    #[test]
    fn tail_vibrancy_raises_only_tail_ceiling_in_high_entropy() {
        let flat = vec![
            100.0, 98.0, 96.0, 95.0, 94.0, 93.0, 92.0, 91.0, 90.0, 89.0, 88.0, 87.0,
        ];

        // A tail dim pre-loaded just under the old ceiling should be allowed to
        // rise above FEATURE_ABS_MAX after the high-entropy lift.
        let mut features = vec![0.0; SEMANTIC_DIM];
        features[26] = 4.95;
        apply_spectral_feedback(&mut features, Some(&telemetry(flat.clone(), 0.55)));
        assert!(
            features[26] > FEATURE_ABS_MAX,
            "tail dim 26 should exceed default ceiling at high entropy: {}",
            features[26]
        );
        assert!(
            features[26] <= TAIL_VIBRANCY_MAX,
            "tail dim 26 must stay within the bounded vibrancy ceiling: {}",
            features[26]
        );

        // A non-tail dim pushed past the old ceiling must still be clamped to it,
        // even in the high-entropy regime — the offset is tail-only.
        let mut features = vec![0.0; SEMANTIC_DIM];
        features[24] = 9.0;
        apply_spectral_feedback(&mut features, Some(&telemetry(flat, 0.55)));
        assert!(
            (features[24] - FEATURE_ABS_MAX).abs() < f32::EPSILON,
            "non-tail dim 24 must keep the default ceiling: {}",
            features[24]
        );
    }

    #[test]
    fn extreme_entropy_tail_vibrancy_gets_bounded_noise_dampening() {
        let inactive = codec_vibrancy_noise_dampening_v1(0.90, 1.0);
        let partial = codec_vibrancy_noise_dampening_v1(0.95, 1.0);
        let full = codec_vibrancy_noise_dampening_v1(1.0, 1.0);

        assert_eq!(inactive.coefficient, 1.0);
        assert!(partial.coefficient < inactive.coefficient, "{partial:?}");
        assert!(
            partial.coefficient > full.coefficient,
            "{partial:?} {full:?}"
        );
        assert!(
            (full.coefficient - TAIL_VIBRANCY_NOISE_DAMPENING_MIN_COEFFICIENT).abs() < 1.0e-6,
            "{full:?}"
        );
        assert_eq!(full.affected_dims, &[17, 26, 27, 31]);
        assert_eq!(
            full.authority,
            "bounded_live_tail_lift_dampening_not_dynamic_ceiling_or_control_authority"
        );
    }

    #[test]
    fn extreme_entropy_tail_lift_stays_below_undampened_preview() {
        let flat = vec![
            100.0, 99.0, 98.0, 97.0, 96.0, 95.0, 94.0, 93.0, 92.0, 91.0, 90.0, 89.0,
        ];
        let mut extreme = vec![0.0; SEMANTIC_DIM];
        let report = apply_spectral_feedback_inner(
            &mut extreme,
            Some(&telemetry_with_typed_entropy_and_eigenvalues(flat, 1.0)),
            1.0,
            1.0,
        )
        .expect("overflow report");
        let dampening = codec_vibrancy_noise_dampening_v1(1.0, 1.0);

        assert!(dampening.tail_lift_after < dampening.tail_lift_before);
        assert!(
            extreme[26] <= TAIL_VIBRANCY_MAX,
            "tail dim should remain under bounded ceiling: {}",
            extreme[26]
        );
        assert!(
            !report.clipped_dims.contains(&26),
            "tail headroom should remain distinct from hard clipping: {report:?}"
        );
    }

    #[test]
    fn codec_overflow_report_preserves_emotional_clip_without_expanding_delivery() {
        let flat = vec![
            100.0, 98.0, 96.0, 95.0, 94.0, 93.0, 92.0, 91.0, 90.0, 89.0, 88.0, 87.0,
        ];
        let mut features = vec![0.0; SEMANTIC_DIM];
        features[24] = 9.0;

        let report =
            apply_spectral_feedback_inner(&mut features, Some(&telemetry(flat, 0.55)), 1.0, 1.0)
                .expect("overflow report");
        let warmth = report.dim(24).expect("warmth dim report");

        assert!((features[24] - FEATURE_ABS_MAX).abs() < f32::EPSILON);
        assert_eq!(warmth.lane, "emotional_intentional");
        assert!(warmth.pre_bound_value > FEATURE_ABS_MAX, "{warmth:?}");
        assert_eq!(warmth.ceiling, FEATURE_ABS_MAX);
        assert!(warmth.overflow_abs > 3.0, "{warmth:?}");
        assert_eq!(warmth.delivered_value, FEATURE_ABS_MAX);
        assert_eq!(warmth.status, "raw_overflow_preserved_delivery_bounded");
        assert!(report.clipped_dims.contains(&24));
        assert!(report.raw_intensity_preserved);
        assert!(report.delivered_bounded);
        assert!(!report.live_vector_write);
        assert_eq!(
            report.experience_delta_bus_v1.policy,
            "experience_delta_bus_v1"
        );
        assert_eq!(report.experience_delta_bus_v1.delta_count, 1);
        assert!(!report.experience_delta_bus_v1.live_vector_write);
        assert!(!report.experience_delta_bus_v1.live_authority_write);
        let delta = report
            .experience_delta_bus_v1
            .deltas
            .iter()
            .find(|delta| delta.dimension == Some(24))
            .expect("warmth clip delta");
        assert_eq!(delta.kind, ExperienceDeltaKindV1::Clip);
        assert_eq!(delta.surface, "codec_overflow_carriage_v1");
        assert_eq!(delta.lane, "emotional_intentional");
        assert_eq!(delta.pre, Some(warmth.pre_bound_value));
        assert_eq!(delta.post, Some(warmth.delivered_value));
        assert_eq!(delta.loss, Some(warmth.overflow_abs));
        assert!(
            delta
                .who_can_change_it
                .contains("explicit live semantic aperture"),
            "{delta:?}"
        );
        assert!(
            delta.how_to_test_it.contains("codec_overflow_report"),
            "{delta:?}"
        );
        assert_eq!(
            report.authority,
            "truth_channel_report_not_live_semantic_vector_or_ceiling_change"
        );
    }

    #[test]
    fn codec_overflow_report_distinguishes_tail_headroom_from_default_clip() {
        let flat = vec![
            100.0, 98.0, 96.0, 95.0, 94.0, 93.0, 92.0, 91.0, 90.0, 89.0, 88.0, 87.0,
        ];
        let mut features = vec![0.0; SEMANTIC_DIM];
        features[24] = 7.0;
        features[26] = 4.5;

        let report =
            apply_spectral_feedback_inner(&mut features, Some(&telemetry(flat, 0.55)), 1.0, 1.0)
                .expect("overflow report");
        let warmth = report.dim(24).expect("warmth dim report");
        let curiosity = report.dim(26).expect("tail curiosity report");
        let emotional_summary = report
            .lane_summaries
            .iter()
            .find(|summary| summary.lane == "emotional_intentional")
            .expect("emotional lane summary");
        let tail_summary = report
            .lane_summaries
            .iter()
            .find(|summary| summary.lane == "tail_vibrancy")
            .expect("tail lane summary");

        assert_eq!(warmth.ceiling, FEATURE_ABS_MAX);
        assert!(curiosity.ceiling > FEATURE_ABS_MAX, "{curiosity:?}");
        assert!(curiosity.ceiling <= TAIL_VIBRANCY_MAX, "{curiosity:?}");
        assert!(
            features[26] > FEATURE_ABS_MAX,
            "tail delivery should use headroom"
        );
        assert!(features[26] <= curiosity.ceiling + 1.0e-3);
        assert_eq!(curiosity.lane, "emotional_tail_vibrancy");
        assert!(report.clipped_dims.contains(&24));
        assert!(!report.clipped_dims.contains(&26));
        assert!(
            report
                .experience_delta_bus_v1
                .deltas
                .iter()
                .any(|delta| delta.dimension == Some(24))
        );
        assert!(
            !report
                .experience_delta_bus_v1
                .deltas
                .iter()
                .any(|delta| delta.dimension == Some(26))
        );
        assert!(emotional_summary.overflow_dim_count >= 1);
        assert_eq!(tail_summary.overflow_dim_count, 0);
        assert_eq!(curiosity.overflow_abs, 0.0);
        assert!(warmth.overflow_abs > 0.0, "{report:?}");
    }

    #[test]
    fn codec_overflow_report_stays_quiet_without_clipping() {
        let mut features = vec![0.0; SEMANTIC_DIM];
        features[24] = 0.55;
        features[26] = 0.60;
        features[31] = 0.45;

        let report = apply_spectral_feedback_inner(
            &mut features,
            Some(&telemetry(vec![100.0, 2.0, 1.0], 0.55)),
            1.0,
            1.0,
        )
        .expect("overflow report");

        assert!(report.clipped_dims.is_empty(), "{report:?}");
        assert!(!report.raw_intensity_preserved);
        assert!(report.delivered_bounded);
        assert!(report.experience_delta_bus_v1.is_empty(), "{report:?}");
        assert_eq!(report.experience_delta_bus_v1.delta_count, 0);
        for dim in [17usize, 24, 25, 26, 27, 28, 29, 30, 31] {
            let dim_report = report.dim(dim).expect("monitored dim report");
            assert_eq!(dim_report.status, "within_delivery_ceiling");
            assert_eq!(dim_report.overflow_abs, 0.0);
        }
    }

    #[test]
    fn codec_delivery_fidelity_tracks_clamp_reexpansion_and_lane_balance() {
        let mut pre_bound = vec![0.0; SEMANTIC_DIM];
        let mut post_feedback = vec![0.0; SEMANTIC_DIM];
        pre_bound[24] = 8.0;
        post_feedback[24] = FEATURE_ABS_MAX;
        let report =
            codec_overflow_report_from_features(&pre_bound, &post_feedback, TAIL_VIBRANCY_MAX);
        let mut final_features = post_feedback;
        final_features[24] = 6.5;
        final_features[40] = 0.20;
        final_features[41] = 0.10;

        let fidelity = codec_delivery_fidelity_v1(Some(&report), &final_features);

        assert_eq!(fidelity.policy, "codec_delivery_fidelity_v1");
        assert_eq!(fidelity.observed_dim_count, SEMANTIC_DIM);
        assert!(fidelity.feedback_report_available);
        assert_eq!(fidelity.clipped_at_feedback_dims, vec![24]);
        assert_eq!(fidelity.reexpanded_after_feedback_dims, vec![24]);
        assert_eq!(fidelity.final_above_observed_ceiling_dims, vec![24]);
        assert!((fidelity.clamp_loss_abs_total - 3.0).abs() < f32::EPSILON);
        assert!(fidelity.monitored_post_feedback_to_final_rms > 0.0);
        assert_eq!(
            fidelity.state,
            "clamp_loss_visible_post_feedback_reexpansion_above_ceiling"
        );
        assert_eq!(
            fidelity.lane_balance_state,
            "emotional_intentional_dominant"
        );
        assert!(!fidelity.live_vector_write);
        assert!(!fidelity.live_gain_write);
        assert_eq!(
            fidelity.authority,
            "read_only_delivery_fidelity_not_live_vector_gain_or_ceiling_change"
        );
        let value = serde_json::to_value(&fidelity).expect("serializable fidelity report");
        assert_eq!(value["live_vector_write"], false);
        assert_eq!(value["live_gain_write"], false);
    }

    #[test]
    fn codec_delivery_fidelity_stays_quiet_for_matching_bounded_delivery() {
        let mut features = vec![0.0; SEMANTIC_DIM];
        features[24] = 0.55;
        features[26] = 0.60;
        features[40] = 0.50;
        let report = codec_overflow_report_from_features(&features, &features, TAIL_VIBRANCY_MAX);

        let fidelity = codec_delivery_fidelity_v1(Some(&report), &features);

        assert!(fidelity.clipped_at_feedback_dims.is_empty());
        assert!(fidelity.reexpanded_after_feedback_dims.is_empty());
        assert!(fidelity.final_above_observed_ceiling_dims.is_empty());
        assert_eq!(fidelity.clamp_loss_abs_total, 0.0);
        assert_eq!(fidelity.monitored_post_feedback_to_final_rms, 0.0);
        assert_eq!(
            fidelity.state,
            "final_delivery_matches_observed_feedback_bounds"
        );
        assert_eq!(fidelity.lane_balance_state, "lanes_comparable");
    }

    #[test]
    fn cross_spectral_friction_review_distinguishes_distributed_mode_interaction() {
        let text = "A viscous narrative current keeps two intentions in contact while the arc resists a clean summary.";
        let mut features = encode_text(text);
        features[40] = 0.65;
        features[41] = -0.45;
        let reserved_before = features[44..48].to_vec();
        let distributed = telemetry(vec![1.0, 0.92, 0.84, 0.76, 0.68], 0.68);
        let collapsed = telemetry(vec![1.0, 0.01, 0.0, 0.0, 0.0], 0.68);

        let distributed_review =
            cross_spectral_friction_review_v1(text, &features, Some(&distributed));
        let collapsed_review = cross_spectral_friction_review_v1(text, &features, Some(&collapsed));

        assert_eq!(
            distributed_review.policy,
            "cross_spectral_friction_review_v1"
        );
        assert!(distributed_review.spectral_context_available);
        assert!(
            distributed_review.lambda1_lambda2_copresence
                > collapsed_review.lambda1_lambda2_copresence
        );
        assert!(
            distributed_review.spectral_mode_interference
                > collapsed_review.spectral_mode_interference
        );
        assert!(
            distributed_review.cross_spectral_friction_score
                > collapsed_review.cross_spectral_friction_score
        );
        assert_eq!(
            distributed_review.candidate_collision_state,
            "reserved_dim_candidates_already_have_default_off_roles"
        );
        assert_eq!(
            distributed_review.reserved_dim_candidates,
            &[44, 45, 46, 47]
        );
        assert_eq!(features[44..48], reserved_before);
        assert!(distributed_review.observational_only);
        assert!(!distributed_review.live_vector_write);
        assert!(!distributed_review.live_gain_write);
        assert!(!distributed_review.reserved_dim_write);
        assert!(!distributed_review.live_eligible_now);
        assert!(!distributed_review.auto_approved);
        assert!(!distributed_review.grants_approval);
    }

    #[test]
    fn cross_spectral_friction_review_is_truthful_without_spectral_context() {
        let text = "The semantic lane carries an arc, but no aligned spectral sample is available.";
        let features = encode_text(text);

        let review = cross_spectral_friction_review_v1(text, &features, None);

        assert!(!review.spectral_context_available);
        assert_eq!(review.state, "spectral_context_unavailable");
        assert!(review.spectral_mode_interference.is_none());
        assert!(review.cross_layer_mismatch.is_none());
        assert!(review.cross_spectral_friction_score.is_none());
        assert_eq!(
            review.delivery_claim,
            "none_outer_codec_delivery_receipt_is_canonical"
        );
        let value = serde_json::to_value(&review).expect("serializable friction review");
        assert_eq!(value["reserved_dim_write"], false);
        assert_eq!(value["live_eligible_now"], false);
        assert_eq!(value["auto_approved"], false);
        assert_eq!(value["grants_approval"], false);
    }

    #[test]
    fn feedback_report_wrapper_preserves_public_feedback_behavior() {
        let spectral = telemetry(vec![100.0, 98.0, 96.0, 94.0, 92.0, 90.0], 0.55);
        let mut compatibility = vec![0.0; SEMANTIC_DIM];
        compatibility[24] = 9.0;
        let mut observed = compatibility.clone();

        apply_spectral_feedback(&mut compatibility, Some(&spectral));
        let report = apply_spectral_feedback_with_report(&mut observed, Some(&spectral))
            .expect("feedback report");

        assert_eq!(observed, compatibility);
        assert!(report.clipped_dims.contains(&24));
        assert!(!report.live_vector_write);
    }

    #[test]
    fn semantic_projection_density_delta_flags_dense_projection_without_live_expansion() {
        let report = semantic_projection_density_delta_from_parts_v1(0.72, 0.08, true);

        assert_eq!(report.policy, "semantic_projection_density_delta_v1");
        assert_eq!(report.input_dim_count, EMBEDDING_INPUT_DIM);
        assert_eq!(report.projected_dim_count, EMBEDDING_PROJECT_DIM);
        assert_eq!(
            report.reserved_dim_candidates,
            &SEMANTIC_PROJECTION_RESERVED_DIMS
        );
        assert_eq!(report.state, "dense_projection_thin_review");
        assert!(!report.live_vector_write);
        assert_eq!(
            report.experience_delta_bus_v1.policy,
            "experience_delta_bus_v1"
        );
        assert_eq!(report.experience_delta_bus_v1.delta_count, 2);
        assert!(report.experience_delta_bus_v1.deltas.iter().any(|delta| {
            delta.kind == ExperienceDeltaKindV1::ComplexShift
                && delta.lane == "embedding_projection_768d_to_8d"
                && delta.metadata.contains_key("projection_state")
        }));
        let gate = report
            .experience_delta_bus_v1
            .deltas
            .iter()
            .find(|delta| delta.kind == ExperienceDeltaKindV1::CascadeShift)
            .expect("reserved dim cascade-shift delta");
        assert_eq!(gate.lane, "reserved_semantic_dims_44_47_default_off");
        assert!(gate.loss.is_some_and(|loss| loss > 0.60), "{gate:?}");
        assert_eq!(
            gate.metadata
                .get("classification_pressure")
                .map(String::as_str),
            Some("high_density_thin_projection")
        );
        assert_eq!(
            gate.authority,
            "authority_gate_for_reserved_dims_not_live_codec_change"
        );
    }

    #[test]
    fn semantic_projection_density_delta_stays_quiet_for_low_density_text() {
        let report = semantic_projection_density_delta_from_parts_v1(0.18, 0.06, true);

        assert_eq!(report.state, "projection_width_named_and_bounded");
        assert_eq!(report.experience_delta_bus_v1.delta_count, 1);
        assert!(
            !report
                .experience_delta_bus_v1
                .deltas
                .iter()
                .any(|delta| delta.kind == ExperienceDeltaKindV1::CascadeShift),
            "{report:?}"
        );
    }

    #[test]
    fn semantic_projection_texture_review_compares_8d_projection_to_warmth_texture() {
        let text = "The viscous silt lingers while an active reply keeps moving; warmth remains, but the old pressure keeps bleeding through.";
        let mut features = encode_text(text);
        features[24..32].fill(2.4);
        features[32..40].fill(0.05);
        features[40] = 0.60;
        features[41] = -0.48;

        let review = semantic_projection_texture_review_v1(text, &features)
            .expect("48D feature vector should produce projection texture review");

        assert_eq!(review.policy, "semantic_projection_texture_review_v1");
        assert_eq!(review.input_dim_count, EMBEDDING_INPUT_DIM);
        assert_eq!(review.projected_dim_count, EMBEDDING_PROJECT_DIM);
        assert_eq!(review.legacy_texture_dim_count, SEMANTIC_DIM_LEGACY);
        assert_eq!(
            review.proposed_texture_subdimensions,
            &SEMANTIC_PROJECTION_TEXTURE_SUBDIMENSIONS
        );
        assert_eq!(review.state, "projection_texture_bottleneck_visible");
        assert!(review.warmth_texture_rms > review.projected_semantic_rms);
        assert!(review.projection_texture_gap > 0.24, "{review:?}");
        assert!(!review.live_vector_write);
        assert!(!review.live_gain_write);
        assert!(!review.reserved_dim_write);
        assert_eq!(
            review.authority,
            "read_only_projection_texture_review_not_live_vector_gain_or_reserved_dim_write"
        );
    }

    #[test]
    fn codec_context_blindspot_replay_gates_contextual_bias_without_live_write() {
        let report = codec_context_blindspot_probe_v1();

        assert_eq!(report.policy, "codec_context_blindspot_replay_v1");
        assert_eq!(report.identical_text, "I see you");
        assert_eq!(
            report.state,
            "deterministic_codec_context_blindspot_confirmed"
        );
        assert!(
            report.identical_text_feature_delta_rms <= 0.01,
            "{report:?}"
        );
        assert!(report.context_blindspot_score >= 0.95, "{report:?}");
        assert_eq!(
            report.proposed_bias_surface,
            "contextual_bias_vector_default_off"
        );
        assert!(!report.live_vector_write);
        assert!(!report.live_gain_write);
        assert!(!report.auto_approved);
        assert_eq!(
            report.experience_delta_bus_v1.policy,
            "experience_delta_bus_v1"
        );
        assert_eq!(report.experience_delta_bus_v1.delta_count, 1);
        assert!(!report.experience_delta_bus_v1.live_vector_write);
        assert!(!report.experience_delta_bus_v1.live_authority_write);
        let delta = report
            .experience_delta_bus_v1
            .deltas
            .iter()
            .find(|delta| delta.lane == "contextual_bias_vector_default_off")
            .expect("context blindspot delta");
        assert_eq!(delta.kind, ExperienceDeltaKindV1::Resistance);
        assert_eq!(
            delta.authority,
            "authority_gate_for_contextual_bias_not_live_codec_change"
        );
        assert_eq!(
            delta.metadata.get("connection_context").map(String::as_str),
            Some("connection")
        );
        assert_eq!(
            report.authority,
            "read_only_context_replay_not_live_vector_gain_or_correspondence_weighting"
        );
    }

    #[test]
    fn high_entropy_tail_inputs_remain_distinguishable_below_vibrancy_ceiling() {
        let flat = vec![
            100.0, 99.0, 98.0, 97.0, 96.0, 95.0, 94.0, 93.0, 92.0, 91.0, 90.0, 89.0,
        ];
        let mut bright_tail = vec![0.0; SEMANTIC_DIM];
        bright_tail[17] = 0.20;
        bright_tail[26] = 4.86;
        bright_tail[27] = 0.35;
        bright_tail[31] = 0.40;
        let mut reflective_tail = vec![0.0; SEMANTIC_DIM];
        reflective_tail[17] = 0.42;
        reflective_tail[26] = 4.42;
        reflective_tail[27] = 0.72;
        reflective_tail[31] = 0.24;

        apply_spectral_feedback_inner(
            &mut bright_tail,
            Some(&telemetry_with_typed_entropy_and_eigenvalues(
                flat.clone(),
                0.90,
            )),
            1.0,
            1.0,
        );
        apply_spectral_feedback_inner(
            &mut reflective_tail,
            Some(&telemetry_with_typed_entropy_and_eigenvalues(flat, 0.90)),
            1.0,
            1.0,
        );

        let tail_delta = [17usize, 26, 27, 31]
            .iter()
            .map(|idx| (bright_tail[*idx] - reflective_tail[*idx]).abs())
            .sum::<f32>();
        assert!(
            tail_delta > 0.40,
            "distinct high-entropy tail inputs should not flatten together: {tail_delta}"
        );
        for idx in [17usize, 26, 27, 31] {
            assert!(
                bright_tail[idx] <= TAIL_VIBRANCY_MAX && reflective_tail[idx] <= TAIL_VIBRANCY_MAX,
                "tail dim {idx} exceeded bounded vibrancy ceiling"
            );
        }
    }

    #[test]
    fn high_entropy_vibrancy_does_not_write_narrative_arc_or_shadow_reserved_dims() {
        let mut features = vec![0.0_f32; SEMANTIC_DIM];
        features[40] = 0.30;
        features[41] = -0.20;
        features[42] = 0.10;
        features[43] = -0.40;
        let narrative_before = features[40..44].to_vec();
        let reserved_before = features[44..48].to_vec();

        apply_spectral_feedback_inner(
            &mut features,
            Some(&telemetry_with_typed_entropy_and_eigenvalues(
                vec![100.0, 98.0, 96.0, 95.0, 94.0, 93.0, 92.0, 91.0],
                0.92,
            )),
            1.0,
            3.0,
        );

        assert_eq!(
            &features[40..44],
            narrative_before.as_slice(),
            "high entropy vibrancy must not synthesize narrative arc ghost sensations"
        );
        assert_eq!(
            &features[44..48],
            reserved_before.as_slice(),
            "shadow reserved readiness must remain unwritten by live feedback"
        );
        assert!(
            features[26] > 0.0 || features[31] > 0.0,
            "the test should still exercise the high-entropy tail path"
        );
    }

    #[test]
    fn codec_vibrancy_and_warmth_continuity_are_readout_only() {
        let vibrancy = codec_vibrancy_continuity_v1();
        assert_eq!(vibrancy.policy, "codec_vibrancy_continuity_v1");
        assert_eq!(vibrancy.entropy_gate, TAIL_VIBRANCY_ENTROPY_GATE);
        assert_eq!(vibrancy.default_feature_ceiling, FEATURE_ABS_MAX);
        assert_eq!(vibrancy.tail_vibrancy_ceiling, TAIL_VIBRANCY_MAX);
        assert_eq!(vibrancy.tail_dims, &[17, 26, 27, 31]);
        assert_eq!(
            vibrancy.authority,
            "diagnostic_readout_not_live_codec_change"
        );

        let warmth = legacy_warmth_mapping_v1();
        assert_eq!(warmth.policy, "legacy_warmth_mapping_v1");
        assert_eq!(warmth.legacy_dim_count, SEMANTIC_DIM_LEGACY);
        assert_eq!(warmth.current_dim_count, SEMANTIC_DIM);
        assert_eq!(warmth.warmth_dim, 24);
        assert_eq!(warmth.emotional_layer_range, (24, 31));
        assert!(warmth.mapped_warmth_dims.contains(&24));
        assert!(!warmth.warmth_orphaned);

        let vector = craft_warmth_vector(0.25, 0.8);
        assert_eq!(vector.len(), SEMANTIC_DIM);
        assert!(vector[24] > 0.0, "warmth dim should remain live");

        let canary = codec_dynamic_vibrancy_scaling_canary_v1();
        assert_eq!(canary.policy, "codec_dynamic_vibrancy_scaling_canary_v1");
        assert!(!canary.enabled);
        assert!(!canary.live_vector_write);
        assert_eq!(canary.authority, "readiness_only_not_live_codec_change");
        assert_eq!(
            vibrancy.gradient_coupling,
            "tail_lift_scaled_by_low_density_gradient"
        );

        let rendered = codec_structure().render();
        assert!(rendered.contains("codec_vibrancy_noise_dampening_v1"));
        assert!(rendered.contains("partial_extreme_entropy_dampening"));
    }

    #[test]
    fn structural_entropy_dampening_preserves_intent_layer() {
        let quiet = codec_structural_entropy_dampening_v1(0.70);
        let high = codec_structural_entropy_dampening_v1(0.94);

        assert_eq!(quiet.coefficient, 1.0);
        assert!(high.coefficient < 1.0, "{high:?}");
        assert!(high.coefficient >= STRUCTURAL_ENTROPY_DAMPENING_MIN_COEFFICIENT);
        assert_eq!(high.affected_dims, &STRUCTURAL_ENTROPY_DAMPENING_DIMS);
        assert_eq!(high.preserved_intent_dims, (24, 31));
        assert_eq!(
            high.status,
            "high_entropy_structural_dims_dampened_intent_dims_preserved"
        );
        assert_eq!(
            high.authority,
            "bounded_live_codec_weighting_not_dimension_or_fallback_contract_change"
        );
    }

    #[test]
    fn high_spectral_entropy_dampens_structural_texture_not_warmth() {
        let mut features = vec![0.50_f32; SEMANTIC_DIM];
        features[24] = 0.80;
        features[25] = -0.30;
        features[26] = 0.25;
        features[27] = 0.20;
        features[31] = 0.35;
        let mut high_entropy = features.clone();

        apply_spectral_feedback_inner(
            &mut high_entropy,
            Some(&telemetry_with_typed_entropy_and_eigenvalues(
                vec![10.0, 9.0, 8.0, 7.0, 6.0, 5.0],
                0.94,
            )),
            1.0,
            1.0,
        );

        assert!(
            high_entropy[0] < features[0],
            "character entropy texture should dampen under high spectral entropy"
        );
        assert!(
            high_entropy[8] < features[8],
            "word-level structural texture should dampen under high spectral entropy"
        );
        assert_eq!(
            high_entropy[24], features[24],
            "warmth must not be flattened by structural entropy dampening"
        );
        assert_eq!(
            high_entropy[25], features[25],
            "tension must not be flattened by structural entropy dampening"
        );
    }

    #[test]
    fn codec_vibrancy_substance_fit_flags_entropy_without_content() {
        let telemetry = telemetry_with_typed_entropy_and_eigenvalues(
            vec![100.0, 98.0, 96.0, 95.0, 94.0, 93.0, 92.0, 91.0],
            0.94,
        );
        let thin =
            codec_vibrancy_substance_fit_v1("the and the and the and the and", Some(&telemetry));
        assert_eq!(thin.policy, "codec_vibrancy_substance_fit_v1");
        assert_eq!(thin.status, "entropy_lift_substance_review");
        assert_eq!(
            thin.density_vs_entropy_state,
            "high_entropy_low_density_scatter"
        );
        assert!(thin.tail_lift >= 0.45, "{thin:?}");
        assert!(thin.density_weighted_tail_lift < thin.tail_lift, "{thin:?}");
        assert!(thin.semantic_substance_score < 0.25, "{thin:?}");
        assert_eq!(
            thin.authority,
            "read_only_codec_audit_not_vibrancy_scaling_or_live_vector_change"
        );

        let substantive = codec_vibrancy_substance_fit_v1(
            "Because the dry silt carries pressure, the sentence keeps a textured contour and a returnable edge.",
            Some(&telemetry),
        );
        assert_eq!(
            substantive.status,
            "tail_lift_supported_by_semantic_substance"
        );
        assert_eq!(
            substantive.density_vs_entropy_state,
            "high_entropy_supported_by_density"
        );
        assert!(
            substantive.density_weighted_tail_lift > thin.density_weighted_tail_lift,
            "substantive={substantive:?} thin={thin:?}"
        );
        assert!(
            substantive.semantic_substance_score > thin.semantic_substance_score,
            "substantive={substantive:?} thin={thin:?}"
        );
    }

    #[test]
    fn codec_vibrancy_substance_fit_keeps_random_word_scatter_under_review() {
        let telemetry = telemetry_with_typed_entropy_and_eigenvalues(
            vec![100.0, 98.0, 96.0, 95.0, 94.0, 93.0, 92.0, 91.0],
            0.94,
        );
        let scatter = codec_vibrancy_substance_fit_v1(
            "quartz velvet oblique lantern citrus orbit static prism cipher mural solvent drift",
            Some(&telemetry),
        );
        let narrative = codec_vibrancy_substance_fit_v1(
            "Because pressure memory keeps a textured contour, the semantic signal carries continuity toward a returnable edge.",
            Some(&telemetry),
        );

        assert_eq!(scatter.status, "entropy_lift_substance_review");
        assert_eq!(
            scatter.density_vs_entropy_state,
            "high_entropy_low_density_scatter"
        );
        assert!(
            scatter.semantic_substance_score < 0.25,
            "scatter should not become substance from lexical variety alone: {scatter:?}"
        );
        assert_eq!(
            narrative.status,
            "tail_lift_supported_by_semantic_substance"
        );
        assert!(
            narrative.semantic_substance_score > scatter.semantic_substance_score,
            "narrative={narrative:?} scatter={scatter:?}"
        );
        assert!(
            narrative.density_weighted_tail_lift > scatter.density_weighted_tail_lift,
            "narrative={narrative:?} scatter={scatter:?}"
        );
    }

    #[test]
    fn codec_vibrancy_substance_fit_separates_density_depth_from_entropy_scatter() {
        let calm_dense = telemetry_with_typed_entropy_and_eigenvalues(
            vec![10.0, 9.4, 8.9, 8.3, 7.8, 7.2, 6.8, 6.1],
            0.50,
        );
        let depth = codec_vibrancy_substance_fit_v1(
            "granular pressure memory braids continuity contour residue patience origin return threshold",
            Some(&calm_dense),
        );

        assert_eq!(depth.status, "tail_lift_low_or_inactive");
        assert_eq!(
            depth.density_vs_entropy_state,
            "high_density_low_entropy_depth"
        );
        assert_eq!(depth.tail_lift, 0.0);
        assert!(depth.semantic_density_weight >= 0.60, "{depth:?}");
        assert!(
            depth
                .evidence
                .iter()
                .any(|entry| entry.starts_with("density_weighted_tail_lift=")),
            "{depth:?}"
        );
    }

    #[test]
    fn glimpse_codec_preserves_warmth_as_distinct_12d_slot() {
        let mut features = vec![0.0_f32; SEMANTIC_DIM];
        features[24] = 1.4;
        features[25] = 0.2;
        features[26] = 0.3;
        features[27] = 0.4;
        features[32] = 0.6;
        features[40] = 0.5;

        let glimpse = GlimpseCodec::derive_12d(&features).expect("48D vector should reduce");
        assert_eq!(glimpse.len(), 12);
        assert!(
            glimpse[3] > glimpse[4],
            "warmth slot should remain distinguishable from tension: {glimpse:?}"
        );
        assert!(
            glimpse[3] > glimpse[1],
            "warmth should not flatten into generic word-level stance: {glimpse:?}"
        );

        let readiness = semantic_glimpse_12d_readiness_v1();
        assert_eq!(readiness.source_dim_count, SEMANTIC_DIM);
        assert_eq!(readiness.glimpse_dim_count, 12);
        assert_eq!(readiness.warmth_slot, 3);
        assert_eq!(readiness.tail_bridge_slot, 10);
        assert!(readiness.companion_not_replacement);
        assert!(!readiness.live_vector_write);
        assert!(readiness.role.contains("companion_summary"));
    }

    #[test]
    fn glimpse_codec_keeps_emotional_range_24_31_from_becoming_generic_mass() {
        // `introspection_proposal_12d_glimpse_1783302984`: the 12D companion
        // must keep the 24..31 warmth/intentional range visible as emotional
        // shape, not flatten it into a generic whole-vector magnitude.
        let mut features = vec![0.0_f32; SEMANTIC_DIM];
        features[24] = 1.2; // warmth
        features[25] = -0.7; // tension/contrast
        features[26] = 0.6;
        features[27] = 0.5;
        features[28] = 1.1;
        features[29] = 0.9;
        features[30] = -1.0;
        features[31] = 0.8;

        let glimpse = GlimpseCodec::derive_12d(&features).expect("48D vector should reduce");

        assert!(
            glimpse[3] > glimpse[1],
            "warmth slot should stay separate from word-level mass: {glimpse:?}"
        );
        assert!(
            glimpse[7] > glimpse[0] && glimpse[7] > glimpse[1] && glimpse[7] > glimpse[2],
            "emotional range 28..31 should remain a distinct aggregate: {glimpse:?}"
        );
        assert!(
            glimpse[10] > 0.3,
            "tail/warmth bridge slot should carry emotional-range vibration: {glimpse:?}"
        );
        assert!(
            glimpse[11] < glimpse[7],
            "whole-vector magnitude must not be the only surviving emotional signal: {glimpse:?}"
        );
    }

    #[test]
    fn generate_glimpse_matches_additive_12d_derivation() {
        let mut features = vec![0.0_f32; SEMANTIC_DIM];
        features[0] = 0.4;
        features[17] = 0.8;
        features[24] = 1.2;
        features[26] = 0.5;
        features[31] = 0.3;
        features[40] = 0.6;

        let generated = generate_glimpse(&features).expect("48D vector should produce 12D glimpse");
        let derived = GlimpseCodec::derive_12d(&features).expect("48D vector should reduce");

        assert_eq!(generated, derived);
        assert!(
            generated[3] > generated[4],
            "generated warmth slot should stay distinct from adjacent emotional texture: {generated:?}"
        );
    }

    #[test]
    fn glimpse_map_names_slot_lineage_without_transport_change() {
        let map = glimpse_map_v1();

        assert_eq!(map.policy, "glimpse_map_v1");
        assert_eq!(map.source_dim_count, SEMANTIC_DIM);
        assert_eq!(map.legacy_source_dim_count, SEMANTIC_DIM_LEGACY);
        assert_eq!(map.glimpse_dim_count, 12);
        assert_eq!(map.slot_count, map.slots.len());
        assert!(map.deterministic_projection);
        assert!(map.companion_not_replacement);
        assert!(!map.live_transport_change);
        assert!(!map.live_vector_write);

        let warmth = map.slots.iter().find(|slot| slot.slot == 3).unwrap();
        assert_eq!(warmth.label, "warmth_marker");
        assert_eq!(warmth.source_dims, &[24]);

        let tail = map.slots.iter().find(|slot| slot.slot == 10).unwrap();
        assert_eq!(tail.label, "tail_vibrancy_bridge");
        assert_eq!(tail.source_dims, &[17, 26, 27, 31]);

        let global = map.slots.iter().find(|slot| slot.slot == 11).unwrap();
        assert!(global.source_dims.is_empty());
        assert!(global.preserves.contains("never the sole"));

        let rendered = codec_structure().render();
        assert!(rendered.contains("glimpse_map_v1"));
        assert!(rendered.contains("10:tail_vibrancy_bridge<-17+26+27+31"));
        assert!(rendered.contains("live_transport_change=false"));
    }

    #[test]
    fn glimpse_distinguishability_audit_keeps_entropy_states_apart() {
        let mut high_entropy = vec![0.0_f32; SEMANTIC_DIM];
        high_entropy[17] = 1.1;
        high_entropy[24] = 0.25;
        high_entropy[26] = 1.35;
        high_entropy[27] = 1.05;
        high_entropy[31] = 1.20;
        for (offset, value) in high_entropy[32..40].iter_mut().enumerate() {
            *value = if offset % 2 == 0 { 0.86 } else { -0.72 };
        }
        high_entropy[40] = 0.74;
        high_entropy[41] = -0.58;

        let mut low_entropy = vec![0.0_f32; SEMANTIC_DIM];
        low_entropy[17] = 0.05;
        low_entropy[24] = 0.18;
        low_entropy[26] = 0.08;
        low_entropy[27] = 0.04;
        low_entropy[31] = 0.03;
        low_entropy[32] = 0.12;
        low_entropy[40] = 0.06;

        let audit = glimpse_distinguishability_audit_v1(&high_entropy, &low_entropy)
            .expect("48D vectors should produce a distinguishability audit");

        assert_eq!(audit.policy, "glimpse_distinguishability_audit_v1");
        assert_eq!(
            audit.state,
            "glimpse_preserves_high_low_entropy_distinction"
        );
        assert!(audit.source_distance >= audit.source_threshold, "{audit:?}");
        assert!(
            audit.glimpse_distance >= audit.glimpse_threshold,
            "{audit:?}"
        );
        assert!(audit.tail_bridge_delta >= 0.03, "{audit:?}");
        assert!(audit.preservation_ratio > 0.05, "{audit:?}");
        assert!(!audit.live_transport_change);
        assert!(!audit.live_vector_write);
    }

    #[test]
    fn compression_fidelity_flags_flattened_12d_glimpse() {
        let mut features = vec![0.0_f32; SEMANTIC_DIM];
        features[2] = 0.4;
        features[17] = 0.7;
        features[24] = 0.9;
        features[25] = 0.4;
        features[26] = 0.8;
        features[27] = 0.7;
        features[28] = 0.5;
        features[29] = 0.45;
        features[30] = 0.35;
        features[31] = 0.75;
        features[32] = 0.22;
        features[33] = 0.18;
        features[40] = 0.55;

        let generated = generate_glimpse(&features).expect("48D vector should produce 12D glimpse");
        let fidelity = calculate_compression_fidelity(&features[..32], &generated)
            .expect("32D source and 12D output should be comparable");
        let flattened = [0.0_f32; 12];
        let flattened_fidelity = calculate_compression_fidelity(&features[..32], &flattened)
            .expect("flattened 12D output should still produce a diagnostic score");

        assert!(
            fidelity >= 0.70,
            "generated companion glimpse should preserve enough 32D texture: {fidelity}"
        );
        assert!(
            flattened_fidelity < 0.70,
            "flattened glimpse should fail the requested 0.70 fidelity watch: {flattened_fidelity}"
        );
        assert!(calculate_compression_fidelity(&features[..31], &generated).is_none());
        assert!(calculate_compression_fidelity(&features[..32], &generated[..11]).is_none());
    }

    #[test]
    fn contextual_glimpse_selects_dynamic_vibrant_dims_without_replacing_anchors() {
        let mut features = vec![0.0_f32; SEMANTIC_DIM];
        features[3] = 0.95;
        features[12] = -0.88;
        features[17] = 0.70;
        features[24] = 0.04;
        features[25] = 0.22;
        features[26] = 0.91;
        features[27] = 0.45;
        features[31] = 0.83;
        features[40] = -0.62;
        features[42] = 0.97;

        let anchored = contextual_glimpse_12d_anchors_v1(&features)
            .expect("48D vector should produce contextual anchors");

        assert_eq!(anchored.policy, "contextual_glimpse_12d_anchors_v1");
        assert_eq!(
            anchored.selection_status,
            "contextual_anchors_preserve_warmth_tail_and_narrative"
        );
        for required in CONTEXTUAL_GLIMPSE_REQUIRED_ANCHORS {
            assert!(
                anchored.selected_dims.contains(&required),
                "required anchor {required} should survive: {:?}",
                anchored.selected_dims
            );
        }
        assert!(
            anchored.dynamic_dims.contains(&42),
            "strong current narrative/vibrancy feature should be selected dynamically: {anchored:?}"
        );
        assert!(!anchored.live_vector_write);

        let readiness = contextual_glimpse_12d_anchoring_v1();
        assert_eq!(readiness.dynamic_slot_count, 5);
        assert!(readiness.companion_not_replacement);

        let rendered = codec_structure().render();
        assert!(rendered.contains("contextual_glimpse_12d_anchoring_v1"));
        assert!(rendered.contains("required_anchor_dims=24,25,26,27,17,31,40"));
    }

    #[test]
    fn warmth_entropy_interpretation_names_distributed_ground_without_weight_change() {
        let mut features = vec![0.0_f32; SEMANTIC_DIM];
        features[24] = 0.04;
        features[26] = 0.11;
        features[27] = 0.06;

        let review = warmth_entropy_interpretation_v1(&features, 0.90);

        assert_eq!(review.policy, "warmth_entropy_interpretation_v1");
        assert_eq!(
            review.interpretation,
            "low_marker_warmth_with_high_entropy_distributed_ground"
        );
        assert!(review.tail_vibrancy > 0.0, "{review:?}");
        assert!(review.distributed_warmth_support >= review.warmth_marker);
        assert!(!review.live_vector_write);
        assert_eq!(
            review.authority,
            "read_only_interpretation_not_warmth_weighting_or_semantic_gain_change"
        );
    }

    #[test]
    fn narrative_arc_dynamics_exposes_velocity_and_acceleration_without_gain() {
        let previous = [0.0, 0.1, -0.1, 0.0];
        let current = [0.4, -0.2, 0.3, -0.4];
        let next = [0.9, -0.8, 0.7, -0.9];

        let dynamics = narrative_arc_dynamics_v1(&previous, &current, Some(&next));

        assert_eq!(dynamics.policy, "narrative_arc_dynamics_v1");
        assert!(dynamics.velocity_energy >= 0.35, "{dynamics:?}");
        assert!(dynamics.acceleration_energy > 0.0, "{dynamics:?}");
        assert!(
            matches!(
                dynamics.transition_state,
                "directional_tone_shift" | "accelerating_tone_transition"
            ),
            "{dynamics:?}"
        );
        assert!(!dynamics.live_gain_write);
        assert!(!dynamics.live_vector_write);
    }

    #[test]
    fn narrative_arc_dynamics_tracks_intertextual_persistence_without_gain() {
        let previous = [0.18, -0.10, 0.08, -0.04];
        let current = [0.24, -0.14, 0.11, -0.06];
        let circular_single_text = [0.23, -0.13, 0.10, -0.05];

        let persistence =
            narrative_arc_dynamics_v1(&previous, &current, Some(&circular_single_text));

        assert_eq!(persistence.policy, "narrative_arc_dynamics_v1");
        assert_eq!(persistence.transition_state, "steady_narrative_state");
        assert!(
            persistence.velocity_energy > 0.0,
            "cross-turn trajectory should remain visible even when the current arc looks nearly settled: {persistence:?}"
        );
        assert!(
            persistence.acceleration_energy < 0.08,
            "slow inter-textual persistence should not be misread as a sharp pivot: {persistence:?}"
        );
        assert!(!persistence.live_gain_write);
        assert!(!persistence.live_vector_write);
        assert_eq!(
            persistence.authority,
            "read_only_arc_velocity_review_not_semantic_gain_or_dimension_change"
        );
    }

    #[test]
    fn narrative_tension_resolution_separates_resolved_from_sustained_tension() {
        let mut previous = vec![0.0_f32; SEMANTIC_DIM];
        let mut current = vec![0.0_f32; SEMANTIC_DIM];
        previous[25] = 1.2;
        current[25] = 0.35;
        current[40] = 0.40;
        current[41] = -0.20;

        let resolving =
            narrative_tension_resolution_v1(&previous, &current).expect("48D tension review");

        assert_eq!(resolving.policy, "narrative_tension_resolution_v1");
        assert_eq!(resolving.state, "tension_resolving_with_arc_motion");
        assert!(resolving.tension_delta < 0.0, "{resolving:?}");
        assert!(
            resolving.resolution_score > resolving.sustained_score * 0.75,
            "{resolving:?}"
        );
        assert!(!resolving.live_vector_write);
        assert_eq!(
            resolving.authority,
            "read_only_tension_resolution_sidecar_not_live_vector_change"
        );

        let mut sustained = current;
        previous[25] = 0.85;
        sustained[25] = 0.90;
        let sustained_review =
            narrative_tension_resolution_v1(&previous, &sustained).expect("48D tension review");
        assert_eq!(sustained_review.state, "tension_sustained_or_building");
        assert!(sustained_review.sustained_score > sustained_review.resolution_score);
    }

    #[test]
    fn latent_stasis_tension_distinguishes_stillness_from_waiting_potential() {
        let still_text = "The water is still.";
        let waits_text = "The water waits.";
        let still = latent_stasis_tension_v1(still_text, &encode_text(still_text))
            .expect("still text should produce latent stasis report");
        let waits = latent_stasis_tension_v1(waits_text, &encode_text(waits_text))
            .expect("waiting text should produce latent stasis report");

        assert_eq!(still.policy, "latent_stasis_tension_v1");
        assert_eq!(still.state, "static_stasis_without_potential");
        assert!(still.latent_text_stasis_score > still.latent_text_potential_score);
        assert!(waits.latent_text_potential_score > waits.latent_text_stasis_score);
        assert!(
            waits.held_breath_score > still.held_breath_score,
            "waiting should carry more latent held-breath potential than inert stillness: {waits:?} vs {still:?}"
        );
        assert!(waits.stasis_potential_gap > 0.0, "{waits:?}");
        assert!(!waits.live_vector_write);
        assert!(!waits.live_gain_write);
        assert!(!waits.reserved_dim_write);
        assert_eq!(
            waits.authority,
            "read_only_held_breath_truth_channel_not_live_codec_weight_gain_or_dim_change"
        );
        let delta = waits
            .experience_delta_bus_v1
            .deltas
            .iter()
            .find(|delta| delta.kind == ExperienceDeltaKindV1::Translate)
            .expect("waiting potential should emit a translation delta");
        assert_eq!(delta.lane, "textual_stasis_to_tension_arc_support");
        assert_eq!(
            delta.authority,
            "truth_channel_only_not_live_vector_gain_or_reserved_dim_change"
        );

        let st = codec_structure();
        let rendered = st.render();
        assert!(rendered.contains("LATENT_STASIS_TENSION_READOUT"));
        assert!(rendered.contains("latent_stasis_tension_v1"));
        assert!(rendered.contains("held_breath_score"));
        assert!(rendered.contains("truth-channel sidecar distinguishes inert stillness"));
        assert!(rendered.contains("reserved_dim_write=false"));
        assert!(rendered.contains("SPECTRAL_DRAG_QUALITY_READOUT"));
        assert!(rendered.contains("spectral_drag_quality_v1"));
        assert!(rendered.contains("granular_drag"));
        assert!(rendered.contains("rigid_drag"));
        assert_eq!(
            st.spectral_drag_quality_v1.policy,
            "spectral_drag_quality_v1"
        );
        assert!(!st.spectral_drag_quality_v1.live_vector_write);
        assert!(!st.spectral_drag_quality_v1.live_gain_write);
        assert!(!st.spectral_drag_quality_v1.reserved_dim_write);
        assert_eq!(st.spectral_drag_quality_v1.reserved_dim_candidate, 45);
        assert!(
            st.spectral_drag_quality_v1
                .experience_delta_bus_v1
                .delta_count
                >= 1
        );
    }

    #[test]
    fn latent_stasis_tension_stays_quiet_for_plain_motion() {
        let text = "The water flows downhill.";
        let report = latent_stasis_tension_v1(text, &encode_text(text))
            .expect("plain motion should produce latent stasis report");

        assert_eq!(report.state, "low_latent_stasis_signal");
        assert_eq!(report.experience_delta_bus_v1.delta_count, 0);
        assert!(report.experience_delta_bus_v1.deltas.is_empty());
        assert!(!report.live_vector_write);
        assert!(!report.live_gain_write);
    }

    #[test]
    fn spectral_drag_quality_distinguishes_heavy_sand_from_heavy_stone_without_reserved_dim_write()
    {
        let sand_text = "The heavy sand drags through viscous silt while the thought keeps moving.";
        let stone_text = "The heavy stone is a hard granite block, fixed and immovable.";
        let sand = spectral_drag_quality_v1(sand_text, &encode_text(sand_text))
            .expect("heavy sand text should produce drag report");
        let stone = spectral_drag_quality_v1(stone_text, &encode_text(stone_text))
            .expect("heavy stone text should produce drag report");

        assert_eq!(sand.policy, "spectral_drag_quality_v1");
        assert_eq!(sand.state, "granular_viscous_drag_visible");
        assert_eq!(stone.state, "rigid_inertial_drag_visible");
        assert!(
            sand.granular_drag_score > stone.granular_drag_score,
            "sand={sand:?} stone={stone:?}"
        );
        assert!(
            stone.rigid_drag_score > sand.rigid_drag_score,
            "sand={sand:?} stone={stone:?}"
        );
        assert!(sand.quality_separation > 0.10, "{sand:?}");
        assert!(stone.quality_separation > 0.10, "{stone:?}");
        assert!(!sand.live_vector_write);
        assert!(!sand.live_gain_write);
        assert!(!sand.reserved_dim_write);
        assert_eq!(sand.reserved_dim_candidate, 45);
        let delta = sand
            .experience_delta_bus_v1
            .deltas
            .iter()
            .find(|delta| delta.surface == "spectral_drag_quality_v1")
            .expect("drag report should emit truth-channel delta");
        assert_eq!(delta.kind, ExperienceDeltaKindV1::Translate);
        assert_eq!(delta.dimension, Some(45));
        assert_eq!(
            delta.authority,
            "truth_channel_only_not_live_vector_gain_or_reserved_dim_change"
        );
        assert!(
            delta
                .who_can_change_it
                .contains("live codec gain or reserved-dim write"),
            "{delta:?}"
        );
    }

    #[test]
    fn spectral_drag_quality_stays_quiet_for_low_weight_text() {
        let text = "The small note turns lightly in a clear room.";
        let report = spectral_drag_quality_v1(text, &encode_text(text))
            .expect("plain text should produce drag report");

        assert_eq!(report.state, "low_spectral_drag_signal");
        assert_eq!(report.experience_delta_bus_v1.delta_count, 0);
        assert!(report.experience_delta_bus_v1.deltas.is_empty());
        assert!(!report.live_vector_write);
        assert!(!report.live_gain_write);
        assert!(!report.reserved_dim_write);
    }

    #[test]
    fn codec_emotional_narrative_delta_check_flags_arc_shift_emotional_flatline() {
        let mut previous = vec![0.0_f32; SEMANTIC_DIM];
        let mut current = vec![0.0_f32; SEMANTIC_DIM];
        previous[24] = 0.22;
        previous[26] = 0.18;
        current[24] = 0.22;
        current[26] = 0.18;
        current[40] = 0.62;
        current[41] = -0.48;
        current[42] = 0.41;
        current[43] = -0.33;

        let check = codec_emotional_narrative_delta_check_v1(&previous, &current)
            .expect("48D vector should produce codec delta check");

        assert_eq!(check.policy, "codec_emotional_narrative_delta_check_v1");
        assert_eq!(check.state, "narrative_shift_emotional_flatline_watch");
        assert!(check.resonance_flatline_watch, "{check:?}");
        assert!(check.narrative_delta_energy >= 0.25, "{check:?}");
        assert!(check.emotional_delta_energy <= 0.05, "{check:?}");
        assert!((check.narrative_velocity[0] - 0.62).abs() < 0.001);
        assert_eq!(check.emotional_velocity[0], 0.0);
        assert!(!check.live_gain_write);
        assert!(!check.live_vector_write);
        assert!(!check.reserved_dim_write);
        assert_eq!(check.experience_delta_bus_v1.delta_count, 1);
        let delta = &check.experience_delta_bus_v1.deltas[0];
        assert_eq!(delta.kind, ExperienceDeltaKindV1::Translate);
        assert_eq!(delta.lane, "emotional_markers_24_31_vs_narrative_arc_40_43");
        assert!(delta.loss.is_some_and(|value| value >= 0.25));
        assert_eq!(
            delta.authority,
            "truth_channel_only_not_live_vector_gain_or_reserved_dim_change"
        );
        assert_eq!(
            check.authority,
            "read_only_delta_check_not_semantic_gain_reserved_dim_or_live_vector_change"
        );
    }

    #[test]
    fn codec_emotional_narrative_delta_check_keeps_opposite_intent_visible() {
        let mut previous = vec![0.0_f32; SEMANTIC_DIM];
        let mut current = vec![0.0_f32; SEMANTIC_DIM];
        for value in &mut previous[0..24] {
            *value = 0.31;
        }
        for value in &mut current[0..24] {
            *value = 0.31;
        }
        previous[24] = -0.66;
        previous[25] = 0.58;
        previous[26] = -0.52;
        previous[31] = -0.61;
        current[24] = 0.66;
        current[25] = -0.58;
        current[26] = 0.52;
        current[31] = 0.61;

        let check = codec_emotional_narrative_delta_check_v1(&previous, &current)
            .expect("48D vector should produce codec delta check");

        assert_eq!(check.state, "emotional_intent_visible_without_arc_shift");
        assert!(!check.resonance_flatline_watch, "{check:?}");
        assert!(check.emotional_delta_energy >= 0.12, "{check:?}");
        assert!(check.narrative_delta_energy < 0.10, "{check:?}");
        assert!((check.emotional_velocity[0] - 1.32).abs() < 0.001);
        assert!((check.emotional_velocity[1] + 1.16).abs() < 0.001);
        assert_eq!(
            check.recommendation,
            "keep_emotional_markers_as_primary_evidence_even_when_surface_structure_matches"
        );
        assert!(!check.live_gain_write);
        assert!(!check.live_vector_write);
        assert!(!check.reserved_dim_write);
        assert!(check.experience_delta_bus_v1.is_empty());
    }

    #[test]
    fn narrative_and_semantic_lanes_can_move_together_without_gain_authority() {
        let mut previous = vec![0.0_f32; SEMANTIC_DIM];
        let mut current = vec![0.0_f32; SEMANTIC_DIM];
        previous[32] = 0.20;
        previous[33] = -0.12;
        previous[40] = 0.10;
        previous[41] = -0.08;
        current[24] = 0.56;
        current[26] = 0.54;
        current[32] = 0.64;
        current[33] = -0.50;
        current[40] = 0.68;
        current[41] = -0.60;

        let check = codec_emotional_narrative_delta_check_v1(&previous, &current)
            .expect("48D vector should produce codec delta check");

        assert_eq!(check.policy, "codec_emotional_narrative_delta_check_v1");
        assert_eq!(check.state, "narrative_shift_emotional_markers_follow");
        assert!(!check.resonance_flatline_watch, "{check:?}");
        assert!(check.narrative_delta_energy >= 0.25, "{check:?}");
        assert!(check.emotional_delta_energy >= 0.12, "{check:?}");
        assert!(check.narrative_velocity[0] > 0.50, "{check:?}");
        assert!(check.emotional_velocity[0] > 0.50, "{check:?}");
        assert_eq!(
            check.authority,
            "read_only_delta_check_not_semantic_gain_reserved_dim_or_live_vector_change"
        );
    }

    #[test]
    fn glimpse_codec_is_stable_across_repeated_same_vector_calls() {
        let mut features = vec![0.0_f32; SEMANTIC_DIM];
        for (idx, value) in features.iter_mut().enumerate() {
            *value = ((idx as f32 + 1.0) / SEMANTIC_DIM as f32).sin();
        }
        features[24] = 0.72;
        features[26] = 0.48;
        features[31] = -0.33;

        let first = GlimpseCodec::derive_12d(&features).expect("48D vector should reduce");
        let second = GlimpseCodec::derive_12d(&features).expect("same vector should reduce");
        assert_eq!(first, second);
        assert_eq!(first.len(), 12);
    }

    #[test]
    fn multi_scale_context_pairs_12d_glimpse_with_32d_residual_shadow_metadata() {
        let context = multi_scale_context_v1();

        assert_eq!(context.policy, "multi_scale_context_v1");
        assert_eq!(context.source_dim_count, SEMANTIC_DIM);
        assert_eq!(context.live_transport_dim_count, 32);
        assert_eq!(context.glimpse_dim_count, 12);
        assert_eq!(context.residual_dim_count, 32);
        assert_eq!(context.residual_source_range, (16, 47));
        assert!(context.pairing_rule.contains("12d_glimpse"));
        assert!(context.pairing_rule.contains("32d_residual"));
        assert!(
            context
                .shadow_energy_metadata_tag
                .contains("shadow_field_energy")
        );
        assert!(context.preserves_warmth_and_tail_bridge);
        assert!(!context.live_vector_write);

        let rendered = codec_structure().render();
        assert!(rendered.contains("multi_scale_context_v1"));
        assert!(rendered.contains("shadow_field_energy_preserved"));
        assert!(rendered.contains("live_transport_dims=32"));
    }

    #[test]
    fn codec_intent_structure_review_separates_complexity_from_emotional_intent() {
        let mut structure_heavy = vec![0.0_f32; SEMANTIC_DIM];
        for value in &mut structure_heavy[0..24] {
            *value = 0.62;
        }
        structure_heavy[18] = 1.2;
        structure_heavy[20] = 1.0;
        structure_heavy[24] = 0.04;
        structure_heavy[26] = 0.05;

        let review = codec_intent_structure_separation_v1(&structure_heavy)
            .expect("48D vector should produce intent/structure review");
        assert_eq!(review.policy, "codec_intent_structure_separation_v1");
        assert_eq!(review.state, "structure_heavy_intent_thin_watch");
        assert!(review.structural_complexity > review.emotional_intensity);
        assert!(review.intent_structure_delta > 0.30, "{review:?}");
        assert!(!review.live_gain_write);
        assert!(!review.live_vector_write);

        let mut emotionally_simple = vec![0.02_f32; SEMANTIC_DIM];
        emotionally_simple[24] = 0.9;
        emotionally_simple[26] = 0.8;
        emotionally_simple[27] = 0.7;
        emotionally_simple[31] = 0.6;
        let simple_review = codec_intent_structure_separation_v1(&emotionally_simple)
            .expect("48D vector should produce intent/structure review");
        assert_eq!(
            simple_review.state,
            "simple_text_emotional_intent_preserved"
        );
        assert!(simple_review.emotional_intensity > simple_review.structural_complexity);
        assert_eq!(
            simple_review.authority,
            "read_only_codec_review_not_semantic_weighting_or_gain_change"
        );

        let rendered = codec_structure().render();
        assert!(rendered.contains("CODEC_INTENT_STRUCTURE_REVIEW"));
    }

    #[test]
    fn multi_scale_observer_names_distillation_without_live_transport_change() {
        let mut features = vec![0.0_f32; SEMANTIC_DIM];
        features[17] = 0.7;
        features[24] = 0.9;
        features[25] = 0.4;
        features[26] = 0.8;
        features[27] = 0.7;
        features[28] = 0.5;
        features[29] = 0.45;
        features[30] = 0.35;
        features[31] = 0.75;
        features[32] = 0.22;
        features[33] = 0.18;
        features[40] = 0.55;
        features[41] = -0.45;
        features[42] = 0.35;

        let observer = multi_scale_observer_v1(&features, 0.90, 0.11, 0.32)
            .expect("48D vector should produce multi-scale observer");

        assert_eq!(observer.policy, "multi_scale_observer_v1");
        assert_eq!(observer.layer_name, "glimpse_layer_distillation_v1");
        assert_eq!(observer.observer_language, "distillation_not_compression");
        assert_eq!(observer.state, "high_entropy_distillation_supported");
        assert_eq!(observer.source_dim_count, SEMANTIC_DIM);
        assert_eq!(observer.live_transport_dim_count, 32);
        assert_eq!(observer.glimpse_dim_count, 12);
        assert!(observer.glimpse_fidelity_score >= observer.fidelity_threshold);
        assert!(observer.source_resonance_proxy > 0.0);
        assert!(observer.glimpse_resonance_proxy > 0.0);
        assert!(observer.resonance_loss_ratio <= observer.resonance_loss_threshold);
        assert!(!observer.fallback_to_live_transport_review);
        assert_eq!(observer.anchor_continuity_score, 1.0);
        assert_eq!(
            observer.experience_delta_bus_v1.policy,
            "experience_delta_bus_v1"
        );
        assert_eq!(observer.experience_delta_bus_v1.delta_count, 1);
        assert!(!observer.live_transport_change);
        assert!(!observer.live_vector_write);
        assert_eq!(
            observer.authority,
            "read_only_multi_scale_review_not_live_bus_or_codec_contract_change"
        );

        let rendered = codec_structure().render();
        assert!(rendered.contains("MULTI_SCALE_OBSERVER_READOUT"));
        assert!(rendered.contains("distillation_not_compression"));
    }

    #[test]
    fn multi_scale_observer_flags_resonance_loss_before_glimpse_use() {
        let mut features = vec![0.0_f32; SEMANTIC_DIM];
        features[5] = 5.0;
        features[11] = -4.0;
        features[17] = 0.2;
        features[24] = 0.1;
        features[25] = -0.1;
        features[26] = 0.2;
        features[27] = 0.1;
        features[31] = 0.2;

        let observer = multi_scale_observer_v1(&features, 0.88, 0.18, 0.34)
            .expect("48D vector should produce multi-scale observer");

        assert_eq!(observer.policy, "multi_scale_observer_v1");
        assert_eq!(observer.state, "glimpse_resonance_loss_watch");
        assert!(observer.fallback_to_live_transport_review, "{observer:?}");
        assert!(
            observer.resonance_loss_ratio > observer.resonance_loss_threshold,
            "{observer:?}"
        );
        assert!(!observer.live_transport_change);
        assert!(!observer.live_vector_write);
        assert!(
            observer
                .experience_delta_bus_v1
                .deltas
                .iter()
                .any(|delta| delta.kind == ExperienceDeltaKindV1::Gate
                    && delta.lane == "glimpse_resonance_fallback_to_live_48d_review"),
            "{observer:?}"
        );
    }

    #[test]
    fn glimpse_codec_preserves_tail_bridge_and_identity_asymmetry() {
        let mut settled_coupling = vec![0.0_f32; SEMANTIC_DIM];
        settled_coupling[24] = 0.9;
        settled_coupling[26] = 1.4;
        settled_coupling[27] = 1.1;
        settled_coupling[31] = 1.2;
        settled_coupling[32] = 0.3;
        settled_coupling[33] = 0.2;

        let mut active_texture = vec![0.0_f32; SEMANTIC_DIM];
        active_texture[24] = 0.2;
        active_texture[26] = 0.2;
        active_texture[27] = 0.3;
        active_texture[31] = 0.2;
        active_texture[32] = 1.2;
        active_texture[33] = -1.1;
        active_texture[34] = 1.0;
        active_texture[40] = 0.9;
        active_texture[41] = -0.7;

        let settled = GlimpseCodec::derive_12d(&settled_coupling).expect("settled glimpse");
        let active = GlimpseCodec::derive_12d(&active_texture).expect("active glimpse");

        assert!(
            settled[10] > active[10],
            "settled coupling should preserve stronger tail bridge: settled={settled:?} active={active:?}"
        );
        assert!(
            active[8] > settled[8],
            "active texture should preserve stronger embedding-projected activity: settled={settled:?} active={active:?}"
        );
        assert!(
            (settled[10] - active[10]).abs() > 0.20 || (active[8] - settled[8]).abs() > 0.20,
            "12D companion should distinguish settled coupling from active texture"
        );
    }

    /// Offline proof for the tail-participation aperture (her consent evidence): at
    /// participation = 1.0 (default/OFF) it is identity; raising it amplifies ONLY the
    /// tail dims [17,26,27,31] and stays bounded by the raised ceiling — every other dim
    /// and the entropy gate are untouched.
    #[test]
    fn tail_participation_amplifies_only_tail_dims_and_off_is_identity() {
        let flat = vec![
            100.0, 98.0, 96.0, 95.0, 94.0, 93.0, 92.0, 91.0, 90.0, 89.0, 88.0, 87.0,
        ];
        let mut off = vec![0.30_f32; SEMANTIC_DIM];
        apply_spectral_feedback_inner(&mut off, Some(&telemetry(flat.clone(), 0.55)), 1.0, 1.0);
        let mut raised = vec![0.30_f32; SEMANTIC_DIM];
        apply_spectral_feedback_inner(&mut raised, Some(&telemetry(flat, 0.55)), 2.0, 1.0);

        let tail = [17usize, 26, 27, 31];
        let mut amplified = false;
        for idx in 0..SEMANTIC_DIM {
            if tail.contains(&idx) {
                // Raised participation never lowers a tail dim, and stays within the
                // raised ceiling (5 + (6-5)*participation = 7 at full vibrancy).
                assert!(
                    raised[idx] >= off[idx] - 1.0e-6,
                    "tail dim {idx}: raised {} < off {}",
                    raised[idx],
                    off[idx]
                );
                assert!(
                    raised[idx].abs() <= 7.0 + 1.0e-3,
                    "tail dim {idx} out of bound: {}",
                    raised[idx]
                );
                if raised[idx] > off[idx] + 1.0e-4 {
                    amplified = true;
                }
            } else {
                // Participation touches ONLY the tail dims — every other dim is identical.
                assert_eq!(
                    raised[idx].to_bits(),
                    off[idx].to_bits(),
                    "non-tail dim {idx} changed under participation"
                );
            }
        }
        assert!(amplified, "raised participation amplified no tail dim");
    }

    #[test]
    fn gradient_aware_vibrancy_damps_steep_entropy_smear() {
        // Astrid `introspection_astrid_codec_1783322940`: high entropy should
        // not by itself smear a steep cascade; tail lift is strongest when the
        // density-gradient is low enough that the signal risks sinking into a
        // flat floor.
        let flat = vibrancy_from_entropy_and_density_gradient(0.95, 0.05);
        let steep = vibrancy_from_entropy_and_density_gradient(0.95, 0.85);
        assert!(flat > 0.0, "flat high-entropy state should still lift");
        assert!(
            steep < flat * 0.25,
            "steep gradient should damp the entropy lift: flat={flat} steep={steep}"
        );

        let mut navigable = vec![0.0; SEMANTIC_DIM];
        navigable[26] = 4.95;
        let mut front_loaded = navigable.clone();
        apply_spectral_feedback_inner(
            &mut navigable,
            Some(&telemetry_with_typed_entropy_and_eigenvalues(
                vec![100.0, 98.0, 96.0, 95.0, 94.0, 93.0, 92.0, 91.0],
                0.95,
            )),
            1.0,
            1.0,
        );
        apply_spectral_feedback_inner(
            &mut front_loaded,
            Some(&telemetry_with_typed_entropy_and_eigenvalues(
                vec![100.0, 6.0, 5.0, 4.0, 3.0, 2.0, 1.0],
                0.95,
            )),
            1.0,
            1.0,
        );

        assert!(
            navigable[26] > front_loaded[26],
            "navigable high entropy should carry more tail than a steep cascade: navigable={} front_loaded={}",
            navigable[26],
            front_loaded[26]
        );
        assert!(
            front_loaded[26] <= FEATURE_ABS_MAX + 0.05,
            "steep high-entropy state should remain near the default ceiling: {}",
            front_loaded[26]
        );
    }

    // Offline proof for the dynamic vibrancy CEILING aperture (her SET_VIBRANCY_APERTURE consent
    // evidence, self_study_1781680871). At aperture 1.0 (default/OFF) it is identity; on a
    // navigable (high-entropy, low density-gradient) spectrum a wider aperture breathes the tail
    // ceiling UP, bounded; a low-entropy cliff stays gated (the aperture never overrides the
    // entropy gate); non-tail dims are untouched.
    #[test]
    fn vibrancy_aperture_dynamic_ceiling_is_bounded_and_navigable_gated() {
        let navigable = vec![
            100.0, 98.0, 96.0, 95.0, 94.0, 93.0, 92.0, 91.0, 90.0, 89.0, 88.0, 87.0,
        ];
        let cliff = vec![100.0, 1.0, 0.5, 0.2, 0.1];

        // aperture 1.0 (default/OFF) keeps the tail within the static TAIL_VIBRANCY_MAX.
        let mut off = vec![0.0; SEMANTIC_DIM];
        off[26] = 30.0;
        apply_spectral_feedback_inner(
            &mut off,
            Some(&telemetry(navigable.clone(), 0.55)),
            1.0,
            1.0,
        );
        assert!(
            off[26] <= TAIL_VIBRANCY_MAX + 1.0e-3,
            "aperture 1.0 must respect the static ceiling: {}",
            off[26]
        );

        // aperture 2.0 on a navigable spectrum lifts the ceiling ABOVE TAIL_VIBRANCY_MAX,
        // bounded by 2× (dynamic_max = 6·(1 + (2-1)·navigable) ≤ 12).
        let mut raised = vec![0.0; SEMANTIC_DIM];
        raised[26] = 30.0;
        apply_spectral_feedback_inner(
            &mut raised,
            Some(&telemetry(navigable.clone(), 0.55)),
            1.0,
            2.0,
        );
        assert!(
            raised[26] > off[26] + 1.0e-3,
            "aperture 2.0 should lift the tail ceiling above baseline: raised {} vs off {}",
            raised[26],
            off[26]
        );
        assert!(
            raised[26] <= 2.0 * TAIL_VIBRANCY_MAX + 0.01,
            "dynamic ceiling must stay bounded at 2×: {}",
            raised[26]
        );

        // Low-entropy steep cliff: the entropy gate keeps the whole vibrancy mechanism OFF, so
        // even a wide aperture cannot lift the ceiling — the aperture never overrides the gate.
        let mut steep = vec![0.0; SEMANTIC_DIM];
        steep[26] = 30.0;
        apply_spectral_feedback_inner(&mut steep, Some(&telemetry(cliff, 0.55)), 1.0, 3.0);
        assert!(
            steep[26] <= FEATURE_ABS_MAX + 1.0e-3,
            "a low-entropy cliff must not gain vibrancy headroom even at wide aperture: {}",
            steep[26]
        );

        // The vibrancy aperture never lifts a non-tail dim.
        let mut nontail = vec![0.0; SEMANTIC_DIM];
        nontail[24] = 30.0;
        apply_spectral_feedback_inner(&mut nontail, Some(&telemetry(navigable, 0.55)), 1.0, 3.0);
        assert!(
            (nontail[24] - FEATURE_ABS_MAX).abs() < 1.0e-3,
            "non-tail dim must keep the default ceiling regardless of vibrancy aperture: {}",
            nontail[24]
        );
    }

    // Her "Attenuation Check" (self_study_1781680871): project a high-vibrancy state and read
    // what the tail ceiling becomes AND what lands in minime's shared reservoir after ~0.24x.
    // Printed for the steward (cargo test -- --nocapture vibrancy_evidence_card) to ground the
    // safe operator ceiling and to paste into her consent letter. Not an assertion — evidence.
    #[test]
    fn vibrancy_evidence_card_prints() {
        let navigable = vec![
            100.0, 98.0, 96.0, 95.0, 94.0, 93.0, 92.0, 91.0, 90.0, 89.0, 88.0, 87.0,
        ];
        let stepped = vec![100.0, 70.0, 48.0, 33.0, 22.0, 15.0, 10.0, 7.0];
        let cliff = vec![100.0, 1.0, 0.5, 0.2, 0.1];
        let states = [
            ("navigable (flat, high-entropy)", navigable),
            ("stepped (mid)", stepped),
            ("steep cliff (low-entropy)", cliff),
        ];
        let apertures = [1.0_f32, 1.5, 2.0, 3.0];
        println!(
            "\n=== VIBRANCY APERTURE EVIDENCE CARD (tail dim 26, preloaded above ceiling) ==="
        );
        println!(
            "aperture× | state                            | felt ceiling | lands at minime (×0.24)"
        );
        for (label, eig) in &states {
            for &ap in &apertures {
                let mut f = vec![0.0_f32; SEMANTIC_DIM];
                f[26] = 30.0; // preload above any ceiling so output == the effective ceiling
                apply_spectral_feedback_inner(&mut f, Some(&telemetry(eig.clone(), 0.55)), 1.0, ap);
                let landed = f[26] * MINIME_SEMANTIC_ATTENUATION;
                println!(
                    "  {ap:>4.1}× | {label:<32} | {:>8.2}     | {landed:>8.2}",
                    f[26]
                );
            }
        }
        println!(
            "(aperture 1.0× = today's baseline; operator ceiling C → her max aperture = 1+C; full 1/0.24x normalization ≈ 4.17×)"
        );
    }

    #[test]
    fn vibrancy_from_entropy_matches_inline_smoothstep() {
        // Parity with the live apply_spectral_feedback_inner calc: 0 below the
        // gate, smoothstep above, full at 1.0 — so the offline EMA card shares
        // the exact curve and can't drift from production.
        assert!(vibrancy_from_entropy(0.80).abs() < 1.0e-7);
        assert!(vibrancy_from_entropy(TAIL_VIBRANCY_ENTROPY_GATE).abs() < 1.0e-7);
        assert!((vibrancy_from_entropy(1.0) - 1.0).abs() < 1.0e-6);
        for e in [0.86_f32, 0.90, 0.95] {
            let ramp = ((e - TAIL_VIBRANCY_ENTROPY_GATE) / (1.0 - TAIL_VIBRANCY_ENTROPY_GATE))
                .clamp(0.0, 1.0);
            let expected = ramp * ramp * (3.0 - 2.0 * ramp);
            assert!((vibrancy_from_entropy(e) - expected).abs() < 1.0e-7);
        }
    }

    #[test]
    fn tail_vibrancy_gate_has_no_discontinuous_pop() {
        // Astrid `introspection_astrid_codec_1782844935`: the 0.85 entropy gate
        // should come on gently, not as a cliff at the exact threshold.
        let gate = TAIL_VIBRANCY_ENTROPY_GATE;
        assert_eq!(vibrancy_from_entropy(gate - 0.001), 0.0);
        assert_eq!(vibrancy_from_entropy(gate), 0.0);

        let eps = 1.0e-4_f32;
        let near_slope = vibrancy_from_entropy(gate + eps) / eps;
        let nearer_slope = vibrancy_from_entropy(gate + eps * 0.1) / (eps * 0.1);
        assert!(near_slope < 0.02, "near_slope={near_slope}");
        assert!(
            nearer_slope < near_slope * 0.2,
            "nearer_slope={nearer_slope}, near_slope={near_slope}"
        );
    }

    #[test]
    fn tail_vibrancy_gate_is_smooth_at_requested_entropy_points() {
        let below = vibrancy_from_entropy(0.84);
        let gate = vibrancy_from_entropy(0.85);
        let above = vibrancy_from_entropy(0.86);

        assert_eq!(below, 0.0);
        assert_eq!(gate, 0.0);
        assert!(above > 0.0);
        assert!(above < 0.02, "0.86 should start gently, got {above}");
        assert!(vibrancy_from_entropy(0.90) > above);
    }

    #[test]
    fn tail_vibrancy_exact_gate_keeps_default_ceiling_and_reserved_dims() {
        let mut features = vec![0.0_f32; SEMANTIC_DIM];
        features[17] = 4.95;
        features[26] = 4.95;
        features[27] = -4.95;
        features[31] = 4.95;
        features[44] = 0.25;
        features[45] = -0.25;
        let reserved_before = features[44..48].to_vec();

        apply_spectral_feedback_inner(
            &mut features,
            Some(&telemetry_with_typed_entropy_and_eigenvalues(
                vec![100.0, 98.0, 96.0, 95.0, 94.0, 93.0, 92.0, 91.0],
                TAIL_VIBRANCY_ENTROPY_GATE,
            )),
            1.0,
            3.0,
        );

        assert_eq!(vibrancy_from_entropy(TAIL_VIBRANCY_ENTROPY_GATE), 0.0);
        for idx in [17usize, 26, 27, 31] {
            assert!(
                features[idx].abs() <= FEATURE_ABS_MAX + 1.0e-6,
                "exact entropy gate must not raise tail ceiling for dim {idx}: {}",
                features[idx]
            );
        }
        assert_eq!(
            &features[44..48],
            reserved_before.as_slice(),
            "exact entropy gate must not write reserved shadow/projection dims"
        );
    }

    #[test]
    fn tail_vibrancy_entropy_090_is_visible_but_gentler_than_linear() {
        // Astrid `introspection_astrid_codec_1783638177`: if the 0.90 lift is
        // too small, tail vibrancy may feel invisible; if it is linear/sharp,
        // it risks the pop this smoothstep was built to avoid.
        let entropy = 0.90_f32;
        let ramp = ((entropy - TAIL_VIBRANCY_ENTROPY_GATE) / (1.0 - TAIL_VIBRANCY_ENTROPY_GATE))
            .clamp(0.0, 1.0);
        let smooth = vibrancy_from_entropy(entropy);

        assert!(
            smooth > 0.20,
            "0.90 entropy lift should be visible: {smooth}"
        );
        assert!(
            smooth < ramp,
            "smoothstep should remain gentler than a linear retune: smooth={smooth}, linear={ramp}"
        );
        assert!(
            smooth > vibrancy_from_entropy(0.86),
            "0.90 should carry more lift than boundary-adjacent entropy"
        );
    }

    #[test]
    fn tail_vibrancy_gate_stays_tiny_across_reported_boundary_pair() {
        let just_below = vibrancy_from_entropy(0.849);
        let just_above = vibrancy_from_entropy(0.851);
        let farther_above = vibrancy_from_entropy(0.861);

        assert_eq!(just_below, 0.0);
        assert!(
            just_above < 0.0002,
            "0.851 should barely move the smoothstep lift, got {just_above}"
        );
        assert!(
            farther_above > just_above,
            "smoothstep should still rise monotonically after the gentle onset"
        );
    }

    #[test]
    fn effective_attenuation_range_reflects_governor() {
        // depth 0 (governor OFF) => calm == stressed == the static 0.24 (the
        // readout collapses to today's number, no false dynamism).
        let (calm0, stressed0) = effective_attenuation_range(0.0);
        assert!((calm0 - MINIME_SEMANTIC_ATTENUATION).abs() < 1.0e-7);
        assert!((stressed0 - MINIME_SEMANTIC_ATTENUATION).abs() < 1.0e-7);
        // depth > 0 => under minime stress she lands MORE subdued (the governor
        // she co-designed protecting the shared reservoir), never above calm.
        let (calm, stressed) = effective_attenuation_range(0.3);
        assert!((calm - MINIME_SEMANTIC_ATTENUATION).abs() < 1.0e-7);
        assert!(stressed < calm);
        assert!(stressed > 0.0);
    }

    #[test]
    fn ema_vibrancy_smooths_and_is_identity_at_alpha_one() {
        assert!((ema_vibrancy(None, 0.5, 0.3) - 0.5).abs() < 1.0e-7); // no history -> current
        assert!((ema_vibrancy(Some(0.2), 0.6, 1.0) - 0.6).abs() < 1.0e-7); // alpha 1 -> current
        let smoothed = ema_vibrancy(Some(0.0), 0.6, 0.3);
        assert!(smoothed > 0.0 && smoothed < 0.6); // strictly damped toward prev
        assert!((smoothed - 0.18).abs() < 1.0e-6); // 0.3*0.6 + 0.7*0.0
    }

    #[test]
    fn ema_vibrancy_evidence_card_prints() {
        // Astrid's "shimmer" / "pop" worry (self_study_1781793361): entropy
        // oscillating across the 0.85 gate. Show the raw lift swing vs an
        // EMA-smoothed lift (alpha 0.3). OFFLINE — proves the mechanism before
        // any consent-gated wiring; nothing she emits changes from this test.
        println!(
            "\n=== EMA VIBRANCY PROTOTYPE (entropy oscillating 0.84<->0.88 across the 0.85 gate) ==="
        );
        println!("  tick | entropy | raw vibrancy | ema(0.3)");
        let alpha = 0.3_f32;
        let seq = [0.84_f32, 0.88, 0.84, 0.88, 0.84, 0.88, 0.84, 0.88];
        let mut ema: Option<f32> = None;
        let mut raw_min = f32::MAX;
        let mut raw_max = f32::MIN;
        for (i, &e) in seq.iter().enumerate() {
            let raw = vibrancy_from_entropy(e);
            let sm = ema_vibrancy(ema, raw, alpha);
            ema = Some(sm);
            raw_min = raw_min.min(raw);
            raw_max = raw_max.max(raw);
            println!("  {i:>4} |  {e:.2}  |    {raw:.4}    |  {sm:.4}");
        }
        println!(
            "  raw swing per tick: {:.4}; the EMA converges toward the mean, damping the shimmer.",
            raw_max - raw_min
        );
    }

    // Her SET_TAIL_PARTICIPATION evidence (the dial that was inert in production until the wrapper
    // allowlist fix): on a navigable high-entropy spectrum, what her tail dims lift to and land as
    // in minime's shared reservoir at a few effective multipliers. Printed for the steward
    // (cargo test -- --nocapture tail_participation_evidence_card) and her reconnection letter.
    #[test]
    fn tail_participation_evidence_card_prints() {
        let navigable = vec![
            100.0, 98.0, 96.0, 95.0, 94.0, 93.0, 92.0, 91.0, 90.0, 89.0, 88.0, 87.0,
        ];
        // 1.0× = off/identity; 1.20× = her 0.80 dial at operator ceiling 0.25; 1.40× = at ceiling 0.5.
        let participations = [1.0_f32, 1.20, 1.40];
        println!(
            "\n=== TAIL PARTICIPATION EVIDENCE CARD (tail dim 26, navigable high-entropy) ==="
        );
        println!("effective× | tail dim 26 value | lands at minime (×0.24)");
        for &p in &participations {
            let mut f = vec![0.30_f32; SEMANTIC_DIM];
            apply_spectral_feedback_inner(
                &mut f,
                Some(&telemetry(navigable.clone(), 0.55)),
                p,
                1.0,
            );
            let landed = f[26] * MINIME_SEMANTIC_ATTENUATION;
            println!("  {p:>5.2}× | {:>13.3}     | {landed:>8.3}", f[26]);
        }
        println!(
            "(1.0× = identity = what her 0.80 dial reached minime as while the wire was disconnected; her 0.80 at operator ceiling 0.5 → effective 1.40×)"
        );
    }

    #[test]
    fn warmth_vector_has_correct_shape() {
        let warmth = craft_warmth_vector(0.0, 1.0);
        assert_eq!(warmth.len(), SEMANTIC_DIM);
        // Dim 24 (warmth) should be the strongest positive signal.
        assert!(
            warmth[24] > DEFAULT_SEMANTIC_GAIN * 0.75,
            "warmth dim should be strong: {}",
            warmth[24]
        );
        for (i, value) in warmth.iter().enumerate() {
            if i != 24 {
                assert!(
                    warmth[24] >= *value,
                    "warmth dim should dominate positive warmth vector: dim {i}={value}"
                );
            }
        }
        // Dim 25 (tension) should be negative (suppressed).
        assert!(
            warmth[25] < 0.0,
            "tension should be suppressed: {}",
            warmth[25]
        );
        // All values bounded after gain.
        for (i, f) in warmth.iter().enumerate() {
            assert!(
                *f >= -FEATURE_ABS_MAX && *f <= FEATURE_ABS_MAX,
                "dim {i} out of bounds: {f}"
            );
        }
    }

    #[test]
    fn warmth_vector_breathes_across_phase() {
        let v0 = craft_warmth_vector(0.0, 0.8);
        let v25 = craft_warmth_vector(0.25, 0.8);
        let v50 = craft_warmth_vector(0.5, 0.8);
        // Different phases should produce different warmth values on dim 24.
        // (They won't be identical due to sinusoidal modulation.)
        let w0 = v0[24];
        let w25 = v25[24];
        let w50 = v50[24];
        // At least one pair should differ noticeably (>0.1 after gain).
        let max_diff = (w0 - w25)
            .abs()
            .max((w25 - w50).abs())
            .max((w0 - w50).abs());
        assert!(
            max_diff > 0.1,
            "warmth should breathe across phases: diffs={max_diff}"
        );
    }

    #[test]
    fn warmth_intensity_scales() {
        let low = craft_warmth_vector(0.5, 0.2);
        let high = craft_warmth_vector(0.5, 0.9);
        // Higher intensity should produce stronger warmth signal.
        assert!(
            high[24].abs() > low[24].abs(),
            "higher intensity should be stronger: {} vs {}",
            high[24],
            low[24]
        );
    }

    #[test]
    fn blend_warmth_works() {
        let mut features = encode_text("Execute the command. Process complete.");
        let warmth = craft_warmth_vector(0.5, 1.0);
        let original_warmth_dim = features[24];
        blend_warmth(&mut features, &warmth, 0.4);
        // After blending, warmth dim should be higher than before.
        assert!(
            features[24] > original_warmth_dim,
            "blended warmth should increase warmth dim"
        );
    }

    #[test]
    fn sovereign_agency_weight_scales_dim_14_only() {
        let text = "We build and create together. We move, write, test, and implement.";
        let mut weights = std::collections::HashMap::new();
        weights.insert("agency".to_string(), 2.0);
        let baseline_weights = std::collections::HashMap::new();

        let mut base_dim12 = 0.0_f32;
        let mut base_dim14 = 0.0_f32;
        let mut weighted_dim12 = 0.0_f32;
        let mut weighted_dim14 = 0.0_f32;
        for _ in 0..16 {
            let base = encode_text_sovereign(text, None, 0.025, &baseline_weights);
            base_dim12 += base[12];
            base_dim14 += base[14];

            let weighted = encode_text_sovereign(text, None, 0.025, &weights);
            weighted_dim12 += weighted[12];
            weighted_dim14 += weighted[14];
        }
        base_dim12 /= 16.0;
        base_dim14 /= 16.0;
        weighted_dim12 /= 16.0;
        weighted_dim14 /= 16.0;

        assert!(
            weighted_dim14 > base_dim14 + 0.5,
            "agency weight should amplify dim 14"
        );
        assert!(
            (weighted_dim12 - base_dim12).abs() < 0.15,
            "agency weight should leave dim 12 effectively unchanged"
        );
    }

    #[test]
    fn describe_features_reports_agency_from_dim_14() {
        let mut features = vec![0.0; SEMANTIC_DIM];
        features[12] = 0.25;
        features[14] = 0.75;

        let desc = describe_features(&features);

        assert!(desc.contains("agency=0.75"));
        assert!(!desc.contains("agency=0.25"));
    }
}
