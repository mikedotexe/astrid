//! A staged process can bind its identity before the canonical manifest is published.
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use serde_json::{Value, json};
use sha2::{Digest, Sha256};

pub(crate) const MANIFEST_SCHEMA: &str = "stack_build_manifest_v1";
pub(crate) const MANIFEST_AUTHORITY: &str = "build_manifest_witness_not_deploy_authority";
const MAX_MANIFEST_BYTES: u64 = 1_048_576;

#[derive(Clone, Debug)]
pub(crate) struct DeploymentBinding {
    pub identity: String,
    pub manifest_sha256: String,
    pub binary_sha256: String,
}

#[derive(Debug)]
struct PinnedManifest {
    path: PathBuf,
    bytes: Vec<u8>,
    binding: DeploymentBinding,
}

static PINNED: OnceLock<PinnedManifest> = OnceLock::new();

/// Explicit selection is validated once, before any runtime work is admitted.
/// Without the new CLI option, legacy workspace-based resolution is unchanged.
pub fn configure(manifest: Option<&Path>) -> Result<(), String> {
    let Some(path) = manifest else { return Ok(()) };
    let executable = std::env::current_exe().map_err(|error| error.to_string())?;
    let pinned = pin_at(path, &executable)?;
    PINNED
        .set(pinned)
        .map_err(|_| "deployment manifest already configured".to_string())
}

fn pin_at(path: &Path, executable: &Path) -> Result<PinnedManifest, String> {
    let path = path.canonicalize().map_err(|error| error.to_string())?;
    let bytes = read_bytes(&path)?;
    let binding = bind_bytes(&bytes, executable)?;
    Ok(PinnedManifest {
        path,
        bytes,
        binding,
    })
}

#[must_use]
pub fn verification_receipt() -> Option<Value> {
    PINNED.get().map(|pinned| {
        json!({
            "schema": "bridge_staged_manifest_verification_v1",
            "verified": true,
            "manifest_path": pinned.path,
            "manifest_sha256": pinned.binding.manifest_sha256,
            "deployment_identity": pinned.binding.identity,
            "binary_sha256": pinned.binding.binary_sha256,
            "authority": "build_manifest_witness_not_deploy_authority",
            "live_eligible_now": false,
            "live_authority_granted": false,
        })
    })
}

pub(crate) fn manifest_path(workspace: &Path) -> PathBuf {
    PINNED.get().map_or_else(
        || workspace.join("deployment_manifests/spectral-bridge.json"),
        |pinned| pinned.path.clone(),
    )
}

/// Identity readers use the startup bytes, not a moving canonical build record.
pub(crate) fn manifest_bytes(path: &Path) -> Result<Vec<u8>, String> {
    if let Some(pinned) = PINNED.get()
        && path == pinned.path
    {
        return Ok(pinned.bytes.clone());
    }
    read_bytes(path)
}

/// A staged process never falls back to a mutable canonical helper when its
/// manifest omits or no longer matches that helper.
pub(crate) fn runtime_artifact(name: &str, fallback: &Path) -> Result<PathBuf, String> {
    let Some(pinned) = PINNED.get() else {
        return Ok(fallback.to_path_buf());
    };
    artifact_from_bytes(&pinned.bytes, name)
}

fn artifact_from_bytes(bytes: &[u8], name: &str) -> Result<PathBuf, String> {
    let value: Value = serde_json::from_slice(bytes).map_err(|error| error.to_string())?;
    let artifact = value
        .get("artifacts")
        .and_then(|items| items.get(name))
        .ok_or_else(|| format!("staged deployment is missing artifact {name}"))?;
    let path = artifact
        .get("path")
        .and_then(Value::as_str)
        .map(PathBuf::from)
        .ok_or_else(|| format!("staged artifact {name} has no path"))?;
    let expected = artifact
        .get("sha256")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("staged artifact {name} has no hash"))?;
    if !fs::symlink_metadata(&path)
        .map_err(|error| error.to_string())?
        .is_file()
    {
        return Err(format!("staged artifact {name} must be a regular file"));
    }
    if file_sha256(&path)? != expected {
        return Err(format!("staged artifact {name} changed"));
    }
    Ok(path)
}

fn read_bytes(path: &Path) -> Result<Vec<u8>, String> {
    let metadata =
        fs::symlink_metadata(path).map_err(|error| format!("read {}: {error}", path.display()))?;
    if !metadata.is_file() {
        return Err("deployment manifest must be a regular file".to_string());
    }
    let mut bytes = Vec::new();
    File::open(path)
        .map_err(|error| error.to_string())?
        .take(MAX_MANIFEST_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    if bytes.len() > usize::try_from(MAX_MANIFEST_BYTES).unwrap_or(usize::MAX) {
        return Err("deployment manifest exceeds the size limit".to_string());
    }
    Ok(bytes)
}

pub(crate) fn read_bound_manifest(
    path: &Path,
    executable: &Path,
) -> Result<DeploymentBinding, String> {
    let bytes = read_bytes(path)?;
    if let Some(pinned) = PINNED.get()
        && path == pinned.path
        && bytes != pinned.bytes
    {
        return Err("deployment manifest changed after startup binding".to_string());
    }
    bind_bytes(&bytes, executable)
}

fn bind_bytes(bytes: &[u8], executable: &Path) -> Result<DeploymentBinding, String> {
    let manifest: Value = serde_json::from_slice(bytes)
        .map_err(|error| format!("decode deployment manifest: {error}"))?;
    let string_at = |pointer: &str| {
        manifest
            .pointer(pointer)
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .ok_or_else(|| format!("deployment manifest is missing {pointer}"))
    };
    if manifest.get("schema").and_then(Value::as_str) != Some(MANIFEST_SCHEMA)
        || manifest.get("authority").and_then(Value::as_str) != Some(MANIFEST_AUTHORITY)
        || manifest
            .pointer("/repository/available")
            .and_then(Value::as_bool)
            != Some(true)
        || manifest
            .pointer("/artifacts/spectral-bridge/exists")
            .and_then(Value::as_bool)
            != Some(true)
    {
        return Err("deployment manifest is not a valid witness manifest".to_string());
    }
    let head = string_at("/repository/head")?;
    let binary_sha256 = string_at("/artifacts/spectral-bridge/sha256")?;
    if !(7..=64).contains(&head.len())
        || !head.bytes().all(|byte| byte.is_ascii_hexdigit())
        || binary_sha256.len() != 64
        || !binary_sha256.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return Err("deployment manifest source or binary identity is malformed".to_string());
    }
    let recorded = PathBuf::from(string_at("/artifacts/spectral-bridge/path")?)
        .canonicalize()
        .map_err(|error| format!("resolve recorded bridge binary: {error}"))?;
    let executable = executable
        .canonicalize()
        .map_err(|error| format!("resolve bridge executable: {error}"))?;
    if recorded != executable {
        return Err("deployment manifest is bound to a different bridge executable".to_string());
    }
    if file_sha256(&executable)? != binary_sha256 {
        return Err(
            "deployment manifest bridge binary hash does not match the executable".to_string(),
        );
    }
    Ok(DeploymentBinding {
        identity: format!("astrid:{head}:bridge:{binary_sha256}"),
        manifest_sha256: hex::encode(Sha256::digest(bytes)),
        binary_sha256,
    })
}

fn file_sha256(path: &Path) -> Result<String, String> {
    let mut file = File::open(path).map_err(|error| error.to_string())?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 65_536];
    loop {
        let count = file.read(&mut buffer).map_err(|error| error.to_string())?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    Ok(hex::encode(hasher.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> (tempfile::TempDir, PathBuf, PathBuf) {
        let root = tempfile::tempdir().unwrap();
        let binary = root.path().join("staged-bridge");
        fs::write(&binary, b"synthetic executable").unwrap();
        let manifest = root.path().join("candidate.json");
        fs::write(
            &manifest,
            serde_json::to_vec(&json!({
                "schema": MANIFEST_SCHEMA, "authority": MANIFEST_AUTHORITY,
                "repository": {"available": true, "head": "1234567"},
                "artifacts": {"spectral-bridge": {"exists":true, "path":binary,
                    "sha256":hex::encode(Sha256::digest(b"synthetic executable"))}},
            }))
            .unwrap(),
        )
        .unwrap();
        (root, manifest, binary)
    }

    #[test]
    fn candidate_binding_does_not_require_canonical_manifest_publication() {
        let (root, manifest, binary) = fixture();
        let canonical = root.path().join("canonical.json");
        fs::write(&canonical, b"old live record").unwrap();
        let pinned = pin_at(&manifest, &binary).unwrap();
        assert!(
            pinned
                .binding
                .identity
                .starts_with("astrid:1234567:bridge:")
        );
        assert_eq!(fs::read(canonical).unwrap(), b"old live record");
        fs::write(manifest, b"changed after startup").unwrap();
        assert!(serde_json::from_slice::<Value>(&pinned.bytes).is_ok());
    }

    #[test]
    fn wrong_executable_or_oversized_manifest_is_refused() {
        let (root, manifest, binary) = fixture();
        let other = root.path().join("other");
        fs::copy(&binary, &other).unwrap();
        assert!(
            pin_at(&manifest, &other)
                .unwrap_err()
                .contains("different bridge executable")
        );
        fs::write(&manifest, vec![b' '; 1_048_577]).unwrap();
        assert!(
            pin_at(&manifest, &binary)
                .unwrap_err()
                .contains("size limit")
        );
    }

    #[test]
    fn staged_helper_missing_or_changed_is_not_silently_replaced() {
        let (_root, manifest, binary) = fixture();
        let bytes = fs::read(manifest).unwrap();
        assert!(artifact_from_bytes(&bytes, "substrate-probe-v2").is_err());
        assert_eq!(
            artifact_from_bytes(&bytes, "spectral-bridge").unwrap(),
            binary
        );
        fs::write(binary, b"changed helper").unwrap();
        assert!(
            artifact_from_bytes(&bytes, "spectral-bridge")
                .unwrap_err()
                .contains("changed")
        );
    }
}
