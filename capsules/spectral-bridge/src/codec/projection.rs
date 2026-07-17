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
const PROJECTION_BASIS_NEAR_ZERO_NORM: f32 = 1.0e-4;
const SEMANTIC_PROJECTION_DENSITY_REVIEW_FLOOR: f32 = 0.55;
const SEMANTIC_PROJECTION_THIN_RMS_CEIL: f32 = 0.12;
const MULTI_SCALE_RESONANCE_LOSS_THRESHOLD: f32 = 0.10;

fn fill_fixed_legacy_projection_raw(
    matrix: &mut [[f32; EMBEDDING_PROJECT_DIM]; EMBEDDING_INPUT_DIM],
) {
    let mut rng: u64 = 42;
    for row in matrix.iter_mut() {
        for value in row.iter_mut() {
            rng = rng
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            *value = ((rng >> 33) as f32 / u32::MAX as f32) - 0.5;
        }
    }
}

fn projection_column_norms(
    matrix: &[[f32; EMBEDDING_PROJECT_DIM]; EMBEDDING_INPUT_DIM],
) -> [f32; EMBEDDING_PROJECT_DIM] {
    let mut norms = [0.0_f32; EMBEDDING_PROJECT_DIM];
    for (column_idx, norm) in norms.iter_mut().enumerate() {
        *norm = matrix
            .iter()
            .map(|row| row[column_idx] * row[column_idx])
            .sum::<f32>()
            .sqrt();
    }
    norms
}

/// Deterministic random projection matrix for embedding → 8D.
/// Uses a fixed seed so the projection is reproducible across restarts.
/// Each column is a normalized random vector (Johnson-Lindenstrauss).
fn embedding_projection_matrix() -> &'static [[f32; EMBEDDING_PROJECT_DIM]; EMBEDDING_INPUT_DIM] {
    use std::sync::OnceLock;
    static MATRIX: OnceLock<Box<[[f32; EMBEDDING_PROJECT_DIM]; EMBEDDING_INPUT_DIM]>> =
        OnceLock::new();
    MATRIX.get_or_init(|| {
        let mut mat = Box::new([[0.0_f32; EMBEDDING_PROJECT_DIM]; EMBEDDING_INPUT_DIM]);
        fill_fixed_legacy_projection_raw(&mut mat);
        // Normalize columns so each projected dim has unit variance
        for (col_idx, norm) in projection_column_norms(&mat).iter().copied().enumerate() {
            if norm > 0.0 {
                for row in mat.iter_mut() {
                    row[col_idx] /= norm;
                }
            }
        }
        mat
    })
}

/// Read-only health witness for the deterministic projection basis before and
/// after normalization. A near-zero raw column would make one semantic
/// projection dimension effectively dead; surfacing that possibility here
/// keeps it inspectable without changing the live basis.
#[derive(Debug, Clone, PartialEq)]
pub struct ProjectionBasisHealthV1 {
    pub policy: &'static str,
    pub source_embedding_dim_count: usize,
    pub projected_dim_count: usize,
    pub raw_column_norms: [f32; EMBEDDING_PROJECT_DIM],
    pub normalized_column_norms: [f32; EMBEDDING_PROJECT_DIM],
    pub near_zero_norm_threshold: f32,
    pub minimum_raw_column_norm: f32,
    pub minimum_raw_column_index: usize,
    pub maximum_raw_column_norm: f32,
    pub minimum_threshold_margin_ratio: f32,
    pub near_zero_column_indexes: Vec<usize>,
    pub all_norms_finite: bool,
    pub normalized_columns_near_unit: bool,
    pub dead_dimension_detected: bool,
    pub state: &'static str,
    pub automatic_basis_rotation: bool,
    pub basis_change_policy: &'static str,
    pub unhealthy_basis_response: &'static str,
    pub observational_only: bool,
    pub live_projection_write: bool,
    pub authority: &'static str,
}

#[must_use]
pub fn projection_basis_health_v1() -> ProjectionBasisHealthV1 {
    let mut raw = Box::new([[0.0_f32; EMBEDDING_PROJECT_DIM]; EMBEDDING_INPUT_DIM]);
    fill_fixed_legacy_projection_raw(&mut raw);
    let raw_column_norms = projection_column_norms(&raw);
    let normalized_column_norms = projection_column_norms(embedding_projection_matrix());
    let mut minimum_raw_column_norm = f32::INFINITY;
    let mut minimum_raw_column_index = 0;
    let mut maximum_raw_column_norm = 0.0_f32;
    let mut near_zero_column_indexes = Vec::new();
    for (index, norm) in raw_column_norms.iter().copied().enumerate() {
        if norm < minimum_raw_column_norm {
            minimum_raw_column_norm = norm;
            minimum_raw_column_index = index;
        }
        maximum_raw_column_norm = maximum_raw_column_norm.max(norm);
        if !norm.is_finite() || norm < PROJECTION_BASIS_NEAR_ZERO_NORM {
            near_zero_column_indexes.push(index);
        }
    }
    let all_norms_finite = raw_column_norms
        .iter()
        .chain(normalized_column_norms.iter())
        .all(|norm| norm.is_finite());
    let normalized_columns_near_unit = normalized_column_norms
        .iter()
        .all(|norm| (*norm - 1.0).abs() <= 1.0e-4);
    let dead_dimension_detected = !near_zero_column_indexes.is_empty();
    let minimum_threshold_margin_ratio = if PROJECTION_BASIS_NEAR_ZERO_NORM > 0.0 {
        minimum_raw_column_norm / PROJECTION_BASIS_NEAR_ZERO_NORM
    } else {
        0.0
    };
    let state = if !all_norms_finite {
        "non_finite_basis_norm_requires_review"
    } else if dead_dimension_detected {
        "near_zero_projection_column_requires_review"
    } else if !normalized_columns_near_unit {
        "normalized_basis_drift_requires_review"
    } else {
        "all_projection_columns_healthy"
    };

    ProjectionBasisHealthV1 {
        policy: "projection_basis_health_v1",
        source_embedding_dim_count: EMBEDDING_INPUT_DIM,
        projected_dim_count: EMBEDDING_PROJECT_DIM,
        raw_column_norms,
        normalized_column_norms,
        near_zero_norm_threshold: PROJECTION_BASIS_NEAR_ZERO_NORM,
        minimum_raw_column_norm,
        minimum_raw_column_index,
        maximum_raw_column_norm,
        minimum_threshold_margin_ratio,
        near_zero_column_indexes,
        all_norms_finite,
        normalized_columns_near_unit,
        dead_dimension_detected,
        state,
        automatic_basis_rotation: false,
        basis_change_policy: "compatibility_pinned_no_automatic_basis_rotation",
        unhealthy_basis_response:
            "fail_test_gate_and_require_captured_replay_before_operator_approved_basis_epoch_change",
        observational_only: true,
        live_projection_write: false,
        authority: "read_only_projection_basis_health_not_projection_kernel_or_live_vector_change",
    }
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

/// Read-only characterization of distinguishability lost across the 768D ->
/// 8D projection boundary. The deterministic probe compares a source-space
/// direction that is nearly null in the fixed aperture with a visible axis,
/// then checks whether the dynamic path preserves an amplitude sweep. It never
/// changes the projection basis, width, normalization, gain, or live vectors.
#[derive(Debug, Clone, PartialEq)]
pub struct ProjectionCompressionAuditV1 {
    pub policy: &'static str,
    pub source_embedding_dim_count: usize,
    pub projected_dim_count: usize,
    pub raw_near_null_delta_rms: f32,
    pub near_null_prescale_rms: f32,
    pub visible_axis_prescale_rms: f32,
    pub near_null_projected_rms: f32,
    pub visible_axis_projected_rms: f32,
    pub near_null_projected_variance: f32,
    pub visible_axis_projected_variance: f32,
    pub quiet_dynamic_variance: f32,
    pub loud_dynamic_variance: f32,
    pub dynamic_variance_delta: f32,
    pub dynamic_magnitude_delta: f32,
    pub near_null_direction_erased_before_normalization: bool,
    pub fixed_normalization_restores_output_length: bool,
    pub same_direction_dynamic_magnitude_erased: bool,
    pub state: &'static str,
    pub felt_compression_conclusion: &'static str,
    pub multi_head_or_width_change_requires_approval: bool,
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

fn projection_rms(values: &[f32]) -> f32 {
    if values.is_empty() {
        return 0.0;
    }
    (values.iter().map(|value| value * value).sum::<f32>() / values.len() as f32).sqrt()
}

fn project_embedding_prescale_v1(embedding: &[f32]) -> Option<[f32; EMBEDDING_PROJECT_DIM]> {
    if embedding.len() != EMBEDDING_INPUT_DIM {
        return None;
    }
    let projection = embedding_projection_matrix();
    let mut result = [0.0_f32; EMBEDDING_PROJECT_DIM];
    for (row_idx, value) in embedding.iter().enumerate() {
        for (column_idx, output) in result.iter_mut().enumerate() {
            *output += *value * projection[row_idx][column_idx];
        }
    }
    Some(result)
}

/// Build deterministic evidence for the current compression boundary. This is
/// deliberately a synthetic characterization rather than a claim about any
/// private embedding or a request to alter the live aperture.
#[must_use]
pub fn projection_compression_probe_v1() -> ProjectionCompressionAuditV1 {
    use nalgebra::{SMatrix, SVector};

    let projection = embedding_projection_matrix();
    let mut gram = SMatrix::<f64, EMBEDDING_PROJECT_DIM, EMBEDDING_PROJECT_DIM>::zeros();
    for row in projection.iter() {
        for row_idx in 0..EMBEDDING_PROJECT_DIM {
            for column_idx in 0..EMBEDDING_PROJECT_DIM {
                gram[(row_idx, column_idx)] += f64::from(row[row_idx]) * f64::from(row[column_idx]);
            }
        }
    }

    let mut rhs = SVector::<f64, EMBEDDING_PROJECT_DIM>::zeros();
    for column_idx in 0..EMBEDDING_PROJECT_DIM {
        rhs[column_idx] = f64::from(projection[0][column_idx]);
    }
    let coefficients = gram
        .lu()
        .solve(&rhs)
        .expect("fixed projection gram matrix should remain solvable");

    let mut near_null_delta = vec![0.0_f32; EMBEDDING_INPUT_DIM];
    near_null_delta[0] = 1.0;
    for (row_idx, row) in projection.iter().enumerate() {
        let column_space_component = row
            .iter()
            .enumerate()
            .map(|(column_idx, value)| f64::from(*value) * coefficients[column_idx])
            .sum::<f64>();
        near_null_delta[row_idx] -= column_space_component as f32;
    }

    let mut visible_axis = vec![0.0_f32; EMBEDDING_INPUT_DIM];
    visible_axis[0] = 1.0;
    let raw_near_null_delta_rms = projection_rms(&near_null_delta);
    let near_null_prescale = project_embedding_prescale_v1(&near_null_delta)
        .expect("internal near-null probe has canonical width");
    let visible_axis_prescale = project_embedding_prescale_v1(&visible_axis)
        .expect("internal visible-axis probe has canonical width");
    let near_null_projected =
        project_embedding(&near_null_delta).expect("internal near-null probe has canonical width");
    let visible_axis_projected =
        project_embedding(&visible_axis).expect("internal visible-axis probe has canonical width");
    let near_null_prescale_rms = projection_rms(&near_null_prescale);
    let visible_axis_prescale_rms = projection_rms(&visible_axis_prescale);
    let near_null_projected_rms = projection_rms(&near_null_projected);
    let visible_axis_projected_rms = projection_rms(&visible_axis_projected);
    let (_, _, near_null_projected_variance, _) = projection_stats(&near_null_projected);
    let (_, _, visible_axis_projected_variance, _) = projection_stats(&visible_axis_projected);

    let base_embedding = (0..EMBEDDING_INPUT_DIM)
        .map(|idx| ((idx as f32) * 0.019).cos())
        .collect::<Vec<_>>();
    let quiet_embedding = base_embedding
        .iter()
        .map(|value| value * 0.01)
        .collect::<Vec<_>>();
    let loud_embedding = base_embedding
        .iter()
        .map(|value| value * 10.0)
        .collect::<Vec<_>>();
    let (quiet_dynamic, quiet_metadata) =
        project_embedding_dynamic_epoch(&quiet_embedding, "aperture probe", "epoch_probe", 0)
            .expect("internal quiet amplitude probe has canonical width");
    let (loud_dynamic, loud_metadata) =
        project_embedding_dynamic_epoch(&loud_embedding, "aperture probe", "epoch_probe", 0)
            .expect("internal loud amplitude probe has canonical width");
    let dynamic_magnitude_delta = quiet_dynamic
        .iter()
        .zip(loud_dynamic.iter())
        .map(|(quiet, loud)| (quiet - loud).abs())
        .fold(0.0_f32, f32::max);
    let quiet_dynamic_variance = quiet_metadata.feature_variance;
    let loud_dynamic_variance = loud_metadata.feature_variance;
    let dynamic_variance_delta = (quiet_dynamic_variance - loud_dynamic_variance).abs();

    let near_null_direction_erased_before_normalization = raw_near_null_delta_rms > 0.03
        && near_null_prescale_rms < visible_axis_prescale_rms * 0.001;
    let fixed_normalization_restores_output_length =
        near_null_projected_rms > 0.12 && visible_axis_projected_rms > 0.12;
    let same_direction_dynamic_magnitude_erased =
        dynamic_magnitude_delta < 0.000_01 && dynamic_variance_delta < 0.000_01;
    let state = if near_null_direction_erased_before_normalization
        && fixed_normalization_restores_output_length
        && same_direction_dynamic_magnitude_erased
    {
        "near_null_direction_and_same_direction_magnitude_loss_visible"
    } else if near_null_direction_erased_before_normalization {
        "near_null_direction_loss_visible"
    } else if same_direction_dynamic_magnitude_erased {
        "same_direction_magnitude_loss_visible"
    } else {
        "probe_did_not_cross_current_loss_thresholds"
    };

    ProjectionCompressionAuditV1 {
        policy: "projection_compression_audit_v1",
        source_embedding_dim_count: EMBEDDING_INPUT_DIM,
        projected_dim_count: EMBEDDING_PROJECT_DIM,
        raw_near_null_delta_rms,
        near_null_prescale_rms,
        visible_axis_prescale_rms,
        near_null_projected_rms,
        visible_axis_projected_rms,
        near_null_projected_variance,
        visible_axis_projected_variance,
        quiet_dynamic_variance,
        loud_dynamic_variance,
        dynamic_variance_delta,
        dynamic_magnitude_delta,
        near_null_direction_erased_before_normalization,
        fixed_normalization_restores_output_length,
        same_direction_dynamic_magnitude_erased,
        state,
        felt_compression_conclusion: "the_current_768_to_8_aperture_can_erase_source_direction_and_same_direction_magnitude;_this_supports_Astrid's_felt_density_report_without_proving_a_live_width_or_basis_change",
        multi_head_or_width_change_requires_approval: true,
        observational_only: true,
        right_to_ignore: true,
        live_vector_write: false,
        live_gain_write: false,
        live_projection_write: false,
        live_eligible_now: false,
        auto_approved: false,
        grants_approval: false,
        authority: "read_only_projection_compression_evidence_not_live_width_basis_gain_or_vector_authority",
    }
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
