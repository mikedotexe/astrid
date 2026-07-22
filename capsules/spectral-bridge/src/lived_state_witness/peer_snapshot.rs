use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{OnceLock, RwLock};
use std::time::{Duration, UNIX_EPOCH};

use serde_json::{Value, json};

const REFRESH_INTERVAL: Duration = Duration::from_millis(500);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PeerSnapshotStatusV1 {
    Observed,
    FileMissing,
    FileUnreadable,
    JsonMalformed,
}

#[derive(Debug, Clone)]
pub(super) struct PeerScalarSnapshotV1 {
    pub value: Option<Value>,
    pub file_modified_unix_ms: Option<u64>,
    pub status: PeerSnapshotStatusV1,
}

impl Default for PeerScalarSnapshotV1 {
    fn default() -> Self {
        Self {
            value: None,
            file_modified_unix_ms: None,
            status: PeerSnapshotStatusV1::FileMissing,
        }
    }
}

static SNAPSHOT: OnceLock<RwLock<PeerScalarSnapshotV1>> = OnceLock::new();
static REFRESHER: OnceLock<()> = OnceLock::new();

fn cache() -> &'static RwLock<PeerScalarSnapshotV1> {
    SNAPSHOT.get_or_init(|| RwLock::new(PeerScalarSnapshotV1::default()))
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

fn load(path: &Path) -> PeerScalarSnapshotV1 {
    let metadata = match fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return PeerScalarSnapshotV1::default();
        },
        Err(_) => {
            return PeerScalarSnapshotV1 {
                status: PeerSnapshotStatusV1::FileUnreadable,
                ..PeerScalarSnapshotV1::default()
            };
        },
    };
    let file_modified_unix_ms = modified_unix_ms(&metadata);
    let raw = match fs::read_to_string(path) {
        Ok(raw) => raw,
        Err(_) => {
            return PeerScalarSnapshotV1 {
                file_modified_unix_ms,
                status: PeerSnapshotStatusV1::FileUnreadable,
                ..PeerScalarSnapshotV1::default()
            };
        },
    };
    let parsed: Value = match serde_json::from_str(&raw) {
        Ok(value) => value,
        Err(_) => {
            return PeerScalarSnapshotV1 {
                file_modified_unix_ms,
                status: PeerSnapshotStatusV1::JsonMalformed,
                ..PeerScalarSnapshotV1::default()
            };
        },
    };
    let value = json!({
        "fill_pct": parsed.get("fill_pct").and_then(Value::as_f64),
        "spectral_entropy": parsed.get("spectral_entropy").and_then(Value::as_f64),
        "structural_entropy": parsed.get("structural_entropy").and_then(Value::as_f64),
    });
    PeerScalarSnapshotV1 {
        value: Some(value),
        file_modified_unix_ms,
        status: PeerSnapshotStatusV1::Observed,
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
            .name("lived-state-peer-snapshot".to_string())
            .spawn(move || loop {
                refresh(&path);
                std::thread::sleep(REFRESH_INTERVAL);
            });
    });
}

pub(super) fn snapshot() -> PeerScalarSnapshotV1 {
    cache()
        .read()
        .map_or_else(|_| PeerScalarSnapshotV1::default(), |value| value.clone())
}

#[cfg(test)]
pub(super) fn load_for_test(path: &Path) -> PeerScalarSnapshotV1 {
    load(path)
}
