//! Evidence-only cache for three whitelisted Minime-authored scalar fields.
//!
//! This cache is not a sensor or runtime input. Model prompts, codec,
//! controllers, shadow state, telemetry handling, and dispatch never read it.
//! Read and parse failures deliberately drop values instead of retaining a
//! last-known scalar as pseudo-current presence. They can only change later
//! sidecar metadata, where the source relation is recorded explicitly.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{OnceLock, RwLock};
use std::time::{Duration, UNIX_EPOCH};

use serde_json::{Value, json};

const EVIDENCE_REFRESH_INTERVAL: Duration = Duration::from_millis(500);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PeerEvidenceSourceStatusV1 {
    Observed,
    FileMissing,
    FileUnreadable,
    JsonMalformed,
}

#[derive(Debug, Clone)]
pub(super) struct PeerEvidenceSnapshotV1 {
    pub value: Option<Value>,
    pub file_modified_unix_ms: Option<u64>,
    pub status: PeerEvidenceSourceStatusV1,
}

impl Default for PeerEvidenceSnapshotV1 {
    fn default() -> Self {
        Self {
            value: None,
            file_modified_unix_ms: None,
            status: PeerEvidenceSourceStatusV1::FileMissing,
        }
    }
}

static SNAPSHOT: OnceLock<RwLock<PeerEvidenceSnapshotV1>> = OnceLock::new();
static REFRESHER: OnceLock<()> = OnceLock::new();

fn cache() -> &'static RwLock<PeerEvidenceSnapshotV1> {
    SNAPSHOT.get_or_init(|| RwLock::new(PeerEvidenceSnapshotV1::default()))
}

fn modified_unix_ms(metadata: &fs::Metadata) -> Option<u64> {
    metadata
        .modified()
        .ok()?
        .duration_since(UNIX_EPOCH)
        .ok()?
        .as_millis()
        .try_into()
        .ok()
}

fn load(path: &Path) -> PeerEvidenceSnapshotV1 {
    let metadata = match fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return PeerEvidenceSnapshotV1::default();
        },
        Err(_) => {
            return PeerEvidenceSnapshotV1 {
                status: PeerEvidenceSourceStatusV1::FileUnreadable,
                ..PeerEvidenceSnapshotV1::default()
            };
        },
    };
    let file_modified_unix_ms = modified_unix_ms(&metadata);
    let raw = match fs::read_to_string(path) {
        Ok(raw) => raw,
        Err(_) => {
            return PeerEvidenceSnapshotV1 {
                file_modified_unix_ms,
                status: PeerEvidenceSourceStatusV1::FileUnreadable,
                ..PeerEvidenceSnapshotV1::default()
            };
        },
    };
    let parsed: Value = match serde_json::from_str(&raw) {
        Ok(value) => value,
        Err(_) => {
            return PeerEvidenceSnapshotV1 {
                file_modified_unix_ms,
                status: PeerEvidenceSourceStatusV1::JsonMalformed,
                ..PeerEvidenceSnapshotV1::default()
            };
        },
    };
    let value = json!({
        "fill_pct": parsed.get("fill_pct").and_then(Value::as_f64),
        "spectral_entropy": parsed.get("spectral_entropy").and_then(Value::as_f64),
        "structural_entropy": parsed.get("structural_entropy").and_then(Value::as_f64),
    });
    PeerEvidenceSnapshotV1 {
        value: Some(value),
        file_modified_unix_ms,
        status: PeerEvidenceSourceStatusV1::Observed,
    }
}

fn refresh(path: &Path) {
    let next = load(path);
    if let Ok(mut current) = cache().write() {
        *current = next;
    }
}

pub(super) fn initialize(path: PathBuf) {
    let _ = cache();
    REFRESHER.get_or_init(|| {
        let _ = std::thread::Builder::new()
            .name("lived-state-peer-evidence-cache".to_string())
            .spawn(move || loop {
                refresh(&path);
                std::thread::sleep(EVIDENCE_REFRESH_INTERVAL);
            });
    });
}

pub(super) fn snapshot() -> PeerEvidenceSnapshotV1 {
    cache()
        .read()
        .map_or_else(|_| PeerEvidenceSnapshotV1::default(), |value| value.clone())
}

#[cfg(test)]
pub(super) fn load_for_test(path: &Path) -> PeerEvidenceSnapshotV1 {
    load(path)
}
