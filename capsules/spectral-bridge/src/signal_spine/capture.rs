use std::collections::{HashMap, HashSet};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{SyncSender, TrySendError, sync_channel};
use std::sync::{Mutex, OnceLock};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const CAPTURE_QUEUE_CAPACITY: usize = 64;
const MAX_CAPTURE_DURATION_MS: u64 = 2 * 60 * 60 * 1_000;
const MAX_CAPTURE_JOURNEYS: u32 = 256;

#[derive(Debug, Clone, Deserialize)]
struct CaptureAuthorityStateV1 {
    schema: String,
    schema_version: u8,
    state: String,
    live_eligible_now: bool,
    auto_approved: bool,
    grants_approval: bool,
    edits_source_now: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub(super) struct CaptureWindowRequestV1 {
    schema: String,
    schema_version: u8,
    capture_window_id: String,
    started_at_unix_ms: u64,
    expires_at_unix_ms: u64,
    journey_limit: u32,
    actor: String,
    acknowledgement: String,
    full_vector_dimensions: usize,
    raw_response_prose_included: bool,
    capture_can_delay_dispatch: bool,
    witness_only: bool,
    artifact_authority_state_v1: CaptureAuthorityStateV1,
}

impl CaptureWindowRequestV1 {
    pub(super) fn load(root: &Path, now_ms: u64) -> Option<Self> {
        let path = root.join("capture_window.json");
        let bytes = fs::read(path).ok()?;
        let request: Self = serde_json::from_slice(&bytes).ok()?;
        if request.schema != "signal_spine_capture_window_v1"
            || request.schema_version != 1
            || request.capture_window_id.trim().is_empty()
            || request.actor.trim().is_empty()
            || request.acknowledgement.trim().is_empty()
            || request.full_vector_dimensions != 48
            || request.raw_response_prose_included
            || request.capture_can_delay_dispatch
            || !request.witness_only
            || request.artifact_authority_state_v1.schema != "artifact_authority_state_v1"
            || request.artifact_authority_state_v1.schema_version != 1
            || request.artifact_authority_state_v1.state != "evidence_only"
            || request.artifact_authority_state_v1.live_eligible_now
            || request.artifact_authority_state_v1.auto_approved
            || request.artifact_authority_state_v1.grants_approval
            || request.artifact_authority_state_v1.edits_source_now
            || request.journey_limit == 0
            || request.journey_limit > MAX_CAPTURE_JOURNEYS
            || request.expires_at_unix_ms <= request.started_at_unix_ms
            || request
                .expires_at_unix_ms
                .saturating_sub(request.started_at_unix_ms)
                > MAX_CAPTURE_DURATION_MS
            || now_ms < request.started_at_unix_ms
            || now_ms >= request.expires_at_unix_ms
        {
            return None;
        }
        let completed = root
            .join("captures")
            .join(&request.capture_window_id)
            .join("journeys");
        let count = fs::read_dir(completed)
            .ok()
            .map_or(0, |entries| entries.filter_map(Result::ok).count());
        (count < request.journey_limit as usize).then_some(request)
    }

    pub(super) fn id(&self) -> &str {
        &self.capture_window_id
    }

    pub(super) const fn journey_limit(&self) -> u32 {
        self.journey_limit
    }

    pub(super) fn completed_journey_count(&self, root: &Path) -> usize {
        root.join("captures")
            .join(&self.capture_window_id)
            .join("journeys")
            .read_dir()
            .ok()
            .map_or(0, |entries| entries.filter_map(Result::ok).count())
    }
}

#[derive(Debug)]
pub(super) struct PendingVectorCaptureV1 {
    pub(super) stage_id: String,
    pub(super) fixture_sha256: String,
    pub(super) vector: Vec<f32>,
}

impl PendingVectorCaptureV1 {
    pub(super) fn new(stage_id: String, vector: &[f32]) -> Self {
        let bytes = vector_bytes(vector);
        Self {
            stage_id,
            fixture_sha256: format!("{:x}", Sha256::digest(&bytes)),
            vector: vector.to_vec(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct VectorFixtureV1 {
    schema: &'static str,
    schema_version: u8,
    stage_id: String,
    vector_dimensions: usize,
    vector_sha256: String,
    vector: Vec<f32>,
    raw_response_prose_included: bool,
}

#[derive(Debug)]
struct CaptureWriteJobV1 {
    root: PathBuf,
    capture_window_id: String,
    journey_id: String,
    captures: Vec<PendingVectorCaptureV1>,
}

#[derive(Debug)]
pub(super) enum CaptureSubmitResultV1 {
    NotArmed,
    Accepted(Vec<(String, String, String, usize)>),
    QueueFull,
    InvalidVectorDimensions,
    WindowUnavailable,
    JourneyLimitReached,
}

struct CaptureWriterV1 {
    tx: SyncSender<CaptureWriteJobV1>,
}

fn capture_reservations() -> &'static Mutex<HashMap<String, HashSet<String>>> {
    static RESERVATIONS: OnceLock<Mutex<HashMap<String, HashSet<String>>>> = OnceLock::new();
    RESERVATIONS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn release_capture_reservation(window_id: &str, journey_id: &str) {
    if let Ok(mut reservations) = capture_reservations().lock()
        && let Some(journeys) = reservations.get_mut(window_id)
    {
        journeys.remove(journey_id);
        if journeys.is_empty() {
            reservations.remove(window_id);
        }
    }
}

impl CaptureWriterV1 {
    fn global() -> &'static Self {
        static WRITER: OnceLock<CaptureWriterV1> = OnceLock::new();
        WRITER.get_or_init(|| {
            let (tx, rx) = sync_channel::<CaptureWriteJobV1>(CAPTURE_QUEUE_CAPACITY);
            std::thread::Builder::new()
                .name("signal-spine-capture".to_string())
                .spawn(move || {
                    while let Ok(job) = rx.recv() {
                        if let Err(error) = write_capture_job(&job) {
                            let _ = write_capture_failure(&job, &error.to_string());
                        }
                        release_capture_reservation(&job.capture_window_id, &job.journey_id);
                    }
                })
                .expect("signal spine capture writer thread must start");
            CaptureWriterV1 { tx }
        })
    }

    fn try_submit(&self, job: CaptureWriteJobV1) -> Result<(), TrySendError<CaptureWriteJobV1>> {
        self.tx.try_send(job)
    }
}

pub(super) fn try_submit_captures(
    root: &Path,
    journey_id: &str,
    captures: Vec<PendingVectorCaptureV1>,
    now_ms: u64,
) -> CaptureSubmitResultV1 {
    if captures.is_empty() {
        return CaptureSubmitResultV1::NotArmed;
    }
    if captures.iter().any(|capture| capture.vector.len() != 48) {
        return CaptureSubmitResultV1::InvalidVectorDimensions;
    }
    let Some(window) = CaptureWindowRequestV1::load(root, now_ms) else {
        return CaptureSubmitResultV1::WindowUnavailable;
    };
    {
        let Ok(mut reservations) = capture_reservations().lock() else {
            return CaptureSubmitResultV1::QueueFull;
        };
        let reserved = reservations.get(window.id()).map_or(0, HashSet::len);
        if window
            .completed_journey_count(root)
            .saturating_add(reserved)
            >= window.journey_limit() as usize
        {
            return CaptureSubmitResultV1::JourneyLimitReached;
        }
        reservations
            .entry(window.id().to_string())
            .or_default()
            .insert(journey_id.to_string());
    }
    let references = captures
        .iter()
        .map(|capture| {
            (
                capture.stage_id.clone(),
                capture.fixture_sha256.clone(),
                format!(
                    "captures/{}/fixtures/{}.json",
                    window.id(),
                    capture.fixture_sha256
                ),
                capture.vector.len(),
            )
        })
        .collect::<Vec<_>>();
    let job = CaptureWriteJobV1 {
        root: root.to_path_buf(),
        capture_window_id: window.id().to_string(),
        journey_id: journey_id.to_string(),
        captures,
    };
    match CaptureWriterV1::global().try_submit(job) {
        Ok(()) => CaptureSubmitResultV1::Accepted(references),
        Err(TrySendError::Full(_)) | Err(TrySendError::Disconnected(_)) => {
            release_capture_reservation(window.id(), journey_id);
            CaptureSubmitResultV1::QueueFull
        },
    }
}

fn vector_bytes(vector: &[f32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(vector.len().saturating_mul(4));
    for value in vector {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes
}

fn ensure_owner_only_dir(path: &Path) -> std::io::Result<()> {
    fs::create_dir_all(path)?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
}

fn write_owner_only(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        ensure_owner_only_dir(parent)?;
    }
    let mut file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .mode(0o600)
        .open(path)?;
    file.set_permissions(fs::Permissions::from_mode(0o600))?;
    file.write_all(bytes)?;
    file.flush()?;
    Ok(())
}

fn write_capture_job(job: &CaptureWriteJobV1) -> std::io::Result<()> {
    let capture_root = job.root.join("captures").join(&job.capture_window_id);
    let fixtures = capture_root.join("fixtures");
    ensure_owner_only_dir(&fixtures)?;
    for capture in &job.captures {
        let fixture = VectorFixtureV1 {
            schema: "signal_vector_fixture_v1",
            schema_version: 1,
            stage_id: capture.stage_id.clone(),
            vector_dimensions: capture.vector.len(),
            vector_sha256: capture.fixture_sha256.clone(),
            vector: capture.vector.clone(),
            raw_response_prose_included: false,
        };
        let bytes = serde_json::to_vec(&fixture).map_err(std::io::Error::other)?;
        write_owner_only(
            &fixtures.join(format!("{}.json", capture.fixture_sha256)),
            &bytes,
        )?;
    }
    let marker = serde_json::to_vec(&serde_json::json!({
        "schema": "signal_capture_journey_marker_v1",
        "schema_version": 1,
        "capture_window_id": job.capture_window_id,
        "journey_id": job.journey_id,
        "fixture_count": job.captures.len(),
        "raw_response_prose_included": false,
        "live_control_authority": false,
    }))
    .map_err(std::io::Error::other)?;
    write_owner_only(
        &capture_root
            .join("journeys")
            .join(format!("{}.json", job.journey_id)),
        &marker,
    )
}

fn write_capture_failure(job: &CaptureWriteJobV1, error: &str) -> std::io::Result<()> {
    let digest = format!("{:x}", Sha256::digest(error.as_bytes()));
    let payload = serde_json::to_vec(&serde_json::json!({
        "schema": "signal_capture_gap_v1",
        "schema_version": 1,
        "capture_window_id": job.capture_window_id,
        "journey_id": job.journey_id,
        "reason": "asynchronous_fixture_write_failed",
        "error_sha256": digest,
        "dossier_sufficient": false,
        "raw_response_prose_included": false,
        "live_control_authority": false,
    }))
    .map_err(std::io::Error::other)?;
    write_owner_only(
        &job.root
            .join("capture_gaps")
            .join(format!("{}.json", job.journey_id)),
        &payload,
    )
}
