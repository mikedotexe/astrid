//! Checked stage activation before sockets, heartbeat producers or autonomy.
use std::fs::File;
use std::io::Read;
use std::path::Path;

use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use super::{SavedState, self_control_v2::deployment_handoff};

fn checkpoint(path: &Path) -> Result<Value, String> {
    const LIMIT: u64 = 64 * 1024 * 1024;
    let mut bytes = Vec::new();
    File::open(path)
        .map_err(|error| format!("open deployment checkpoint: {error}"))?
        .take(LIMIT + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > LIMIT {
        return Err("deployment checkpoint exceeds 64 MiB".to_string());
    }
    let state: SavedState = serde_json::from_slice(&bytes)
        .map_err(|error| format!("invalid deployment checkpoint: {error}"))?;
    Ok(
        json!({"sha256":hex::encode(Sha256::digest(&bytes)), "exchange_count":state.exchange_count,
        "decoded_by_runtime_schema":true}),
    )
}

pub fn inspect_deployment_inputs() -> Result<Value, String> {
    Ok(
        json!({"schema":"bridge_deployment_inputs_v1", "mutation_performed":false,
        "checkpoint":checkpoint(&crate::paths::bridge_paths().state_path())?,
        "self_control":deployment_handoff::inspect_startup()?}),
    )
}

pub fn apply_deployment_startup() -> Result<Value, String> {
    // Decode the conversation before consuming the one-shot state handoff.
    let checkpoint = checkpoint(&crate::paths::bridge_paths().state_path())?;
    Ok(
        json!({"schema":"bridge_deployment_startup_v1", "pid":std::process::id(),
        "checkpoint":checkpoint, "self_control":deployment_handoff::apply_startup()?,
        "admission_started":false, "authority":"operator_maintenance_witness_only"}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_or_incompatible_checkpoint_cannot_be_silently_defaulted() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("state.json");
        assert!(checkpoint(&path).is_err());
        std::fs::write(&path, b"{}").unwrap();
        assert!(
            checkpoint(&path)
                .unwrap_err()
                .contains("invalid deployment checkpoint")
        );
        std::fs::write(&path, b"{\"exchange_count\":3}").unwrap();
        assert!(checkpoint(&path).is_err());
    }
}
