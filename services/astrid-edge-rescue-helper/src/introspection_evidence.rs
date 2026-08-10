//! Root-produced, metadata-only projections for the read-only code introspector.

use std::collections::BTreeSet;
use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::build::verify_sealed_build;
use crate::config::{Config, valid_hex64, valid_identifier};
use crate::fs_guard::{atomic_write, canonical_json, read_regular, sha256};
use crate::generation::validate_release_manifest;
use crate::manifest::{Candidate, PatchBundle, SourceSnapshot, bounded_changed_lines};
use crate::native::CommandReceipt;
use crate::{Error, Result};

const BUILD_SCHEMA: &str = "astrid.edge_self_change.build_evidence_view.v1";
const DIFF_SCHEMA: &str = "astrid.edge_self_change.generation_diff_view.v1";
const MAXIMUM_COMMANDS: usize = 128;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredEvidence {
    schema: String,
    candidate_id: String,
    source_id: String,
    source_revision: String,
    commands: Vec<CommandReceipt>,
    candidate_replay_sha256: String,
    package_replay_sha256: String,
    immutable_invariants: bool,
    offline_locked: bool,
    network_policy: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct BaseBoundPatch {
    schema: String,
    candidate: Candidate,
    patch: PatchBundle,
}

#[derive(Debug, Serialize)]
struct BuildEvidenceView<'a> {
    schema: &'static str,
    appliance_id: &'a str,
    generated_at: u64,
    build_id: &'a str,
    candidate_id: &'a str,
    candidate_sha256: &'a str,
    generation_id: &'a str,
    base_generation: &'a str,
    source_id: &'a str,
    source_revision: &'a str,
    target: &'a str,
    bundle_sha256: &'a str,
    tests_sha256: &'a str,
    privilege_envelope: &'a str,
    gates: &'a [CommandReceipt],
    invariants: BuildInvariants<'a>,
    lifecycle: Lifecycle<'a>,
    provenance: &'static str,
    projection_sha256: &'static str,
}

#[derive(Debug, Serialize)]
struct BuildInvariants<'a> {
    candidate_replay_sha256: &'a str,
    package_replay_sha256: &'a str,
    immutable_invariants: bool,
    offline_locked: bool,
    network_policy: &'a str,
}

#[derive(Debug, Serialize)]
struct GenerationDiffView<'a> {
    schema: &'static str,
    appliance_id: &'a str,
    generated_at: u64,
    generation_id: &'a str,
    base_generation: &'a str,
    build_id: &'a str,
    candidate_id: &'a str,
    candidate_sha256: &'a str,
    source_id: &'a str,
    parent_source_id: &'a str,
    files: &'a [DiffFile],
    total_changed_lines: usize,
    truncated: bool,
    lifecycle: Lifecycle<'a>,
    provenance: &'static str,
    projection_sha256: &'static str,
}

#[derive(Debug, Serialize)]
struct DiffFile {
    path: String,
    source_sha256: String,
    content_sha256: String,
    changed_lines: usize,
}

#[derive(Debug, Serialize)]
struct Lifecycle<'a> {
    status: &'a str,
    events: [LifecycleEvent<'a>; 1],
}

#[derive(Debug, Serialize)]
struct LifecycleEvent<'a> {
    phase: &'a str,
    recorded_at: u64,
    authority: &'static str,
}

pub(super) fn publish(config: &Config, artifact: &Path, generation: &Path) -> Result<()> {
    let build = verify_sealed_build(config, artifact, None)?;
    let generated_at = unix_seconds();
    let evidence = load_evidence(
        config,
        artifact,
        &build.candidate_id,
        &build.source_revision,
    )?;
    let bound = load_bound_patch(config, artifact, &build.candidate_sha256)?;
    if bound.candidate.candidate_id != build.candidate_id
        || bound.candidate.base_generation != build.base_generation
        || bound.patch.candidate_id != build.candidate_id
        || bound.patch.base_generation != build.base_generation
        || bound.patch.files.len() > config.policy.maximum_files
    {
        return Err(Error::new("introspection projection patch binding failed"));
    }

    let base_root = config.roots.releases.join(&build.base_generation);
    let base_identity = validate_release_manifest(config, &base_root)?;
    let base =
        SourceSnapshot::load_for_generation(config, &base_root, base_identity.operator_initial)?;
    let derived = SourceSnapshot::validate_embedded_generation_snapshot(config, generation)?;
    let parent_source_id = derived
        .parent_source_id()
        .ok_or_else(|| Error::new("derived source has no parent source identity"))?;
    if bound.patch.source_id != base.source_id() || parent_source_id != base.source_id() {
        return Err(Error::new(
            "introspection projection source lineage differs from the exact base",
        ));
    }
    let (files, total_changed_lines) = diff_files(config, &base, &bound.patch)?;
    let lifecycle = Lifecycle {
        status: "installed_pending_stage_verification",
        events: [LifecycleEvent {
            phase: "generation_installed",
            recorded_at: generated_at,
            authority: "immutable_root_rescue_helper",
        }],
    };
    let build_view = BuildEvidenceView {
        schema: BUILD_SCHEMA,
        appliance_id: &config.appliance_id,
        generated_at,
        build_id: &build.build_id,
        candidate_id: &build.candidate_id,
        candidate_sha256: &build.candidate_sha256,
        generation_id: &build.generation_id,
        base_generation: &build.base_generation,
        source_id: &evidence.source_id,
        source_revision: &evidence.source_revision,
        target: &build.target,
        bundle_sha256: &build.bundle_sha256,
        tests_sha256: &build.tests_sha256,
        privilege_envelope: &build.privilege_envelope,
        gates: &evidence.commands,
        invariants: BuildInvariants {
            candidate_replay_sha256: &evidence.candidate_replay_sha256,
            package_replay_sha256: &evidence.package_replay_sha256,
            immutable_invariants: evidence.immutable_invariants,
            offline_locked: evidence.offline_locked,
            network_policy: &evidence.network_policy,
        },
        lifecycle,
        provenance: "immutable_machine_evidence_not_astrid_authorship",
        projection_sha256: "",
    };
    let diff_view = GenerationDiffView {
        schema: DIFF_SCHEMA,
        appliance_id: &config.appliance_id,
        generated_at,
        generation_id: &build.generation_id,
        base_generation: &build.base_generation,
        build_id: &build.build_id,
        candidate_id: &build.candidate_id,
        candidate_sha256: &build.candidate_sha256,
        source_id: derived.source_id(),
        parent_source_id,
        files: &files,
        total_changed_lines,
        truncated: false,
        lifecycle: Lifecycle {
            status: "installed_pending_stage_verification",
            events: [LifecycleEvent {
                phase: "generation_installed",
                recorded_at: generated_at,
                authority: "immutable_root_rescue_helper",
            }],
        },
        provenance: "immutable_machine_evidence_not_astrid_authorship",
        projection_sha256: "",
    };
    write_projection(config, "build-evidence", &build.build_id, &build_view, true)?;
    write_projection(
        config,
        "generation-diffs",
        &build.generation_id,
        &diff_view,
        true,
    )
}

fn load_evidence(
    config: &Config,
    artifact: &Path,
    candidate_id: &str,
    source_revision: &str,
) -> Result<StoredEvidence> {
    let bytes = read_regular(&artifact.join("evidence.json"), 16 * 1024 * 1024)?;
    let value: Value = serde_json::from_slice(&bytes)?;
    if canonical_json(&value)? != bytes {
        return Err(Error::new("build evidence is not canonical JSON"));
    }
    let evidence: StoredEvidence = serde_json::from_value(value)?;
    if evidence.schema != "astrid.edge_rescue_helper.build_evidence.v1"
        || evidence.candidate_id != candidate_id
        || evidence.source_revision != source_revision
        || !valid_identifier(&evidence.candidate_id)
        || !evidence.source_id.starts_with("cpu-edge:")
        || evidence.commands.is_empty()
        || evidence.commands.len() > MAXIMUM_COMMANDS
        || !valid_hex64(&evidence.candidate_replay_sha256)
        || !valid_hex64(&evidence.package_replay_sha256)
        || !evidence.immutable_invariants
        || !evidence.offline_locked
        || evidence.network_policy != config.policy.network_policy
        || evidence.commands.iter().any(|command| {
            command.label.is_empty()
                || command.label.len() > 96
                || !valid_hex64(&command.executable_sha256)
                || !valid_hex64(&command.argv_sha256)
                || command.exit_code != Some(0)
                || command.timed_out
        })
    {
        return Err(Error::new(
            "build evidence cannot enter the sanitized introspection projection",
        ));
    }
    Ok(evidence)
}

fn load_bound_patch(
    config: &Config,
    artifact: &Path,
    candidate_sha256: &str,
) -> Result<BaseBoundPatch> {
    let bytes = read_regular(
        &artifact.join("candidate-patch.json"),
        config.policy.maximum_candidate_bytes,
    )?;
    let value: Value = serde_json::from_slice(&bytes)?;
    if canonical_json(&value)? != bytes {
        return Err(Error::new("base-bound patch is not canonical JSON"));
    }
    let bound: BaseBoundPatch = serde_json::from_value(value)?;
    if bound.schema != "astrid.edge_rescue_helper.base_bound_patch.v1"
        || bound.candidate.digest()? != candidate_sha256
        || sha256(&canonical_json(&bound.patch)?) != bound.candidate.patch_sha256
    {
        return Err(Error::new("base-bound patch projection binding failed"));
    }
    Ok(bound)
}

fn diff_files(
    config: &Config,
    base: &SourceSnapshot,
    patch: &PatchBundle,
) -> Result<(Vec<DiffFile>, usize)> {
    let mut paths = BTreeSet::new();
    let mut changed_total = 0_usize;
    let mut files = Vec::with_capacity(patch.files.len());
    for file in &patch.files {
        if !paths.insert(file.path.as_str()) || !valid_hex64(&file.content_sha256) {
            return Err(Error::new(
                "generation diff contains duplicate or invalid paths",
            ));
        }
        let (source_sha256, original) = base.verified_source(&file.path)?;
        if source_sha256 != file.source_sha256
            || sha256(file.content.as_bytes()) != file.content_sha256
        {
            return Err(Error::new("generation diff source/content hash failed"));
        }
        let original = std::str::from_utf8(&original)
            .map_err(|_| Error::new("generation diff source is not UTF-8"))?;
        let remaining = config
            .policy
            .maximum_changed_lines
            .saturating_sub(changed_total);
        let changed_lines = bounded_changed_lines(original, &file.content, remaining);
        changed_total = changed_total.saturating_add(changed_lines);
        files.push(DiffFile {
            path: file.path.clone(),
            source_sha256,
            content_sha256: file.content_sha256.clone(),
            changed_lines,
        });
    }
    if changed_total > config.policy.maximum_changed_lines {
        return Err(Error::new(
            "generation diff exceeds immutable changed-line policy",
        ));
    }
    Ok((files, changed_total))
}

fn write_projection<T: Serialize>(
    config: &Config,
    kind: &str,
    identifier: &str,
    core: &T,
    require_root: bool,
) -> Result<()> {
    if !matches!(kind, "build-evidence" | "generation-diffs") || !valid_identifier(identifier) {
        return Err(Error::new("introspection projection identity is invalid"));
    }
    let root = config.roots.supervisor_state.join("introspection-evidence");
    require_projection_directory(config, &root, require_root)?;
    let directory = root.join(kind);
    require_projection_directory(config, &directory, require_root)?;
    let mut projection = serde_json::to_value(core)?;
    if projection.get("projection_sha256").and_then(Value::as_str) != Some("") {
        return Err(Error::new(
            "introspection projection hash field must be blank before canonical hashing",
        ));
    }
    let digest = sha256(&canonical_json(&projection)?);
    projection
        .as_object_mut()
        .ok_or_else(|| Error::new("introspection projection is not an object"))?
        .insert("projection_sha256".to_owned(), Value::String(digest));
    let bytes = canonical_json(&projection)?;
    if bytes.len() > 256 * 1024 {
        return Err(Error::new(
            "introspection projection exceeds its byte bound",
        ));
    }
    atomic_write(
        &directory.join(format!("{identifier}.json")),
        &bytes,
        0o440,
        false,
    )
}

fn require_projection_directory(config: &Config, path: &Path, require_root: bool) -> Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    let expected_uid = if require_root {
        0
    } else {
        nix::unistd::geteuid().as_raw()
    };
    if !metadata.is_dir()
        || metadata.file_type().is_symlink()
        || metadata.uid() != expected_uid
        || metadata.gid() != config.identities.steward_gid
        || metadata.mode() & 0o7777 != 0o2750
    {
        return Err(Error::new(
            "introspection projection directory ownership or mode failed",
        ));
    }
    Ok(())
}

fn unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs())
}

#[cfg(test)]
mod tests {
    use super::{Lifecycle, LifecycleEvent};
    use crate::fs_guard::{canonical_json, sha256};

    #[test]
    fn self_hash_excludes_the_envelope_and_detects_core_tamper() {
        let core = Lifecycle {
            status: "installed_pending_stage_verification",
            events: [LifecycleEvent {
                phase: "generation_installed",
                recorded_at: 1,
                authority: "immutable_root_rescue_helper",
            }],
        };
        let mut value = serde_json::json!({
            "schema":"fixture.v1",
            "lifecycle":core,
            "projection_sha256":"",
        });
        let digest = sha256(&canonical_json(&value).unwrap());
        value["projection_sha256"] = serde_json::json!(digest);
        let mut tampered = value.clone();
        tampered["projection_sha256"] = serde_json::json!("");
        tampered["lifecycle"]["status"] = serde_json::json!("accepted");
        assert_ne!(sha256(&canonical_json(&tampered).unwrap()), digest);
    }
}
