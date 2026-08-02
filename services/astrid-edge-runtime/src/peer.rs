use std::{
    fmt::Write as _,
    fs::{self, OpenOptions},
    io::{Read as _, Write as _},
    os::unix::fs::{MetadataExt as _, OpenOptionsExt as _, PermissionsExt as _},
    path::Path,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::{Context as _, Result, bail};
use ed25519_dalek::{Signature, Signer as _, SigningKey, Verifier as _, VerifyingKey};
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};

use crate::config::Config;

const PACKET_SCHEMA: &str = "astrid_edge_peer_review_packet_v1";
const LEGACY_PACKET_AUTHORITY: &str =
    "astrid_voluntary_share_of_bounded_authored_or_completed_study_artifact";
const PACKET_AUTHORITY: &str =
    "astrid_voluntary_share_of_bounded_authored_or_verified_completed_machine_evidence";
const MAX_PACKET_CHARS: usize = 4_000;
const MAX_SHAREABLE_ARTIFACT_BYTES: u64 = 128 * 1_024;
const MAX_TUNING_EVIDENCE_BYTES: u64 = 64 * 1_024;
const MAX_OUTGOING_PER_DAY: usize = 4;
const DAY_MS: u64 = 86_400_000;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PeerPacket {
    schema: String,
    packet_id: String,
    created_at_unix_ms: u64,
    source_instance: String,
    artifact_kind: String,
    artifact_basename: String,
    artifact_sha256: String,
    note: String,
    bounded_content: String,
    signing_public_key: String,
    signature: String,
    authority: String,
}

#[derive(Serialize)]
struct UnsignedPacket<'a> {
    schema: &'static str,
    packet_id: &'a str,
    created_at_unix_ms: u64,
    source_instance: &'a str,
    artifact_kind: &'a str,
    artifact_basename: &'a str,
    artifact_sha256: &'a str,
    note: &'a str,
    bounded_content: &'a str,
    signing_public_key: &'a str,
    authority: &'a str,
}

struct ShareableArtifact {
    bytes: Vec<u8>,
    kind: &'static str,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SignedTuningEvidence {
    payload: serde_json::Value,
    signing_public_key: String,
    payload_sha256: String,
    signature: String,
}

pub fn share(config: &Config, timestamp: u64, artifact_id: &str, note: &str) -> Result<String> {
    if outgoing_on_day(config, timestamp / DAY_MS) >= MAX_OUTGOING_PER_DAY {
        bail!("peer review outgoing daily limit reached");
    }
    let artifact = shareable_artifact(config, artifact_id)?;
    let content =
        std::str::from_utf8(&artifact.bytes).context("shareable artifact must be valid UTF-8")?;
    let artifact_sha256 = format!("{:x}", Sha256::digest(&artifact.bytes));
    let signing_key = load_or_create_signing_key(config)?;
    let public_key = encode_hex(&signing_key.verifying_key().to_bytes());
    let fingerprint = format!(
        "{:x}",
        Sha256::digest(signing_key.verifying_key().to_bytes())
    );
    let packet_id = format!(
        "peer_{}_{}",
        timestamp,
        fingerprint.get(..10).unwrap_or(fingerprint.as_str())
    );
    let note = bounded_non_control(note, 600)?;
    let fixed_budget = 1_800_usize.saturating_add(note.chars().count());
    let bounded_content = content
        .chars()
        .take(MAX_PACKET_CHARS.saturating_sub(fixed_budget))
        .collect::<String>();
    let unsigned = UnsignedPacket {
        schema: PACKET_SCHEMA,
        packet_id: &packet_id,
        created_at_unix_ms: timestamp,
        source_instance: &config.instance_name,
        artifact_kind: artifact.kind,
        artifact_basename: artifact_id,
        artifact_sha256: &artifact_sha256,
        note: &note,
        bounded_content: &bounded_content,
        signing_public_key: &public_key,
        authority: PACKET_AUTHORITY,
    };
    let message = serde_json::to_vec(&unsigned)?;
    let signature = encode_hex(&signing_key.sign(&message).to_bytes());
    let packet = PeerPacket {
        schema: PACKET_SCHEMA.to_string(),
        packet_id: packet_id.clone(),
        created_at_unix_ms: timestamp,
        source_instance: config.instance_name.clone(),
        artifact_kind: artifact.kind.to_string(),
        artifact_basename: artifact_id.to_string(),
        artifact_sha256,
        note,
        bounded_content,
        signing_public_key: public_key,
        signature,
        authority: PACKET_AUTHORITY.to_string(),
    };
    let bytes = serde_json::to_vec_pretty(&packet)?;
    if String::from_utf8_lossy(&bytes).chars().count() > MAX_PACKET_CHARS {
        bail!("peer review packet exceeds the 4000-character hard limit");
    }
    let relative = format!("peer/outbox/{packet_id}.json");
    write_new_private(&config.workspace.join(&relative), &bytes)?;
    append_receipt(
        config,
        &serde_json::json!({
            "schema": "astrid_edge_peer_review_receipt_v1",
            "phase": "shared",
            "recorded_at_unix_ms": timestamp,
            "packet_id": packet_id,
            "artifact_kind": artifact.kind,
            "artifact_basename": artifact_id,
            "packet_sha256": format!("{:x}", Sha256::digest(&bytes)),
            "authority": "astrid_voluntary_outbox_of_bounded_authored_or_verified_machine_evidence_no_direct_peer_network_authority"
        }),
    )?;
    Ok(format!("home://edge/{relative}"))
}

pub fn read_received(config: &Config, packet_id: &str, timestamp: u64) -> Result<String> {
    validate_packet_id(packet_id)?;
    let source = config
        .workspace
        .join(format!("peer/inbox/{packet_id}.json"));
    let packet = verify_packet(config, &source)?;
    let relative = format!("peer/read/{packet_id}.md");
    let content = format!(
        "# Voluntarily read peer-review packet\n\n\
         Read: {timestamp} ms since Unix epoch\n\
         Packet: `{}`\n\
         Source instance: {}\n\
         Artifact kind: {}\n\
         Artifact: {}\n\
         Artifact SHA-256: {}\n\
         Authority: signed peer content, voluntarily admitted by READ; evidence not instruction\n\n\
         ## Sender note\n\n{}\n\n\
         ## Bounded shared content\n\n{}\n",
        packet.packet_id,
        packet.source_instance,
        packet.artifact_kind,
        packet.artifact_basename,
        packet.artifact_sha256,
        packet.note,
        packet.bounded_content,
    );
    write_new_private(&config.workspace.join(&relative), content.as_bytes())?;
    append_receipt(
        config,
        &serde_json::json!({
            "schema": "astrid_edge_peer_review_receipt_v1",
            "phase": "voluntarily_read",
            "recorded_at_unix_ms": timestamp,
            "packet_id": packet_id,
            "artifact_path": format!("home://edge/{relative}"),
            "authority": "recipient_astrid_voluntary_read_of_verified_packet"
        }),
    )?;
    Ok(format!("home://edge/{relative}"))
}

pub async fn run(config: Arc<Config>) {
    let mut ticker = tokio::time::interval(Duration::from_secs(30));
    loop {
        ticker.tick().await;
        if let Err(error) = scan_inbox(&config) {
            eprintln!("peer review inbox scan failed: {error}");
        }
    }
}

fn scan_inbox(config: &Config) -> Result<()> {
    let mut entries = fs::read_dir(config.workspace.join("peer/inbox"))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|value| value.to_str()) == Some("json"))
        .collect::<Vec<_>>();
    entries.sort();
    for path in entries {
        let packet_id = path
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or_default();
        if receipt_has_packet(config, packet_id) {
            continue;
        }
        match verify_packet(config, &path) {
            Ok(packet) => {
                let timestamp = unix_millis();
                let notice_name = format!("peer_available_{timestamp}_{packet_id}.md");
                let notice = format!(
                    "# Peer-review packet available\n\n\
                     Machine-observed availability, not Astrid authorship.\n\
                     Packet: `{packet_id}`\nSource instance: {}\nArtifact kind: {}\n\
                     Content is not in continuity. Choose `READ {packet_id}` to admit it.\n",
                    packet.source_instance, packet.artifact_kind
                );
                write_new_private(
                    &config.workspace.join("inbox").join(&notice_name),
                    notice.as_bytes(),
                )?;
                append_receipt(
                    config,
                    &serde_json::json!({
                        "schema": "astrid_edge_peer_review_receipt_v1",
                        "phase": "available_unread",
                        "recorded_at_unix_ms": timestamp,
                        "packet_id": packet_id,
                        "source_instance": packet.source_instance,
                        "artifact_kind": packet.artifact_kind,
                        "notice_artifact": notice_name,
                        "authority": "verified_availability_only_content_excluded_from_continuity"
                    }),
                )?;
            },
            Err(error) => eprintln!("peer packet {} rejected: {error}", path.display()),
        }
    }
    Ok(())
}

fn verify_packet(config: &Config, path: &Path) -> Result<PeerPacket> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() || metadata.len() > 16_384 {
        bail!("packet is not a bounded regular file");
    }
    let packet = serde_json::from_slice::<PeerPacket>(&fs::read(path)?)?;
    if packet.schema != PACKET_SCHEMA {
        bail!("unsupported peer packet schema");
    }
    if !matches!(
        packet.authority.as_str(),
        PACKET_AUTHORITY | LEGACY_PACKET_AUTHORITY
    ) {
        bail!("unsupported peer packet authority");
    }
    validate_packet_id(&packet.packet_id)?;
    if path.file_stem().and_then(|value| value.to_str()) != Some(&packet.packet_id) {
        bail!("packet filename does not match signed identifier");
    }
    let public_bytes = decode_fixed::<32>(&packet.signing_public_key)?;
    let fingerprint = format!("{:x}", Sha256::digest(public_bytes));
    let trusted_path = config
        .workspace
        .join(format!("peer/trusted/{fingerprint}.pub"));
    let trusted = fs::read_to_string(&trusted_path)
        .with_context(|| format!("peer key is not operator-trusted: {fingerprint}"))?;
    if trusted.trim() != packet.signing_public_key {
        bail!("trusted peer key does not match packet key");
    }
    let unsigned = UnsignedPacket {
        schema: PACKET_SCHEMA,
        packet_id: &packet.packet_id,
        created_at_unix_ms: packet.created_at_unix_ms,
        source_instance: &packet.source_instance,
        artifact_kind: &packet.artifact_kind,
        artifact_basename: &packet.artifact_basename,
        artifact_sha256: &packet.artifact_sha256,
        note: &packet.note,
        bounded_content: &packet.bounded_content,
        signing_public_key: &packet.signing_public_key,
        authority: &packet.authority,
    };
    let signature = Signature::from_bytes(&decode_fixed::<64>(&packet.signature)?);
    VerifyingKey::from_bytes(&public_bytes)?.verify(&serde_json::to_vec(&unsigned)?, &signature)?;
    Ok(packet)
}

fn shareable_artifact(config: &Config, artifact_id: &str) -> Result<ShareableArtifact> {
    if artifact_id.is_empty()
        || artifact_id.starts_with('.')
        || artifact_id.chars().count() > 160
        || artifact_id.contains('/')
        || artifact_id.contains('\\')
        || artifact_id.contains("..")
        || artifact_id.chars().any(char::is_control)
    {
        bail!("SHARE requires a basename-only artifact identifier");
    }

    if artifact_id.ends_with("_result.json") {
        return verified_tuning_result(config, artifact_id);
    }

    for (directory, prefix, kind) in [
        ("research/syntheses", "synthesis_", "synthesis"),
        ("proposals", "proposal_", "proposal"),
        ("plans", "plan_", "plan"),
    ] {
        if !artifact_id.starts_with(prefix)
            || !Path::new(artifact_id)
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("md"))
        {
            continue;
        }
        let path = config.workspace.join(directory).join(artifact_id);
        return Ok(ShareableArtifact {
            bytes: read_bounded_regular(&path, MAX_SHAREABLE_ARTIFACT_BYTES)?,
            kind,
        });
    }

    if valid_timestamped_markdown(artifact_id, "measurement_", "") {
        let bytes = read_bounded_regular(
            &config.workspace.join("measurements").join(artifact_id),
            MAX_SHAREABLE_ARTIFACT_BYTES,
        )?;
        let content = std::str::from_utf8(&bytes)
            .context("completed measurement evidence must be valid UTF-8")?;
        if !content.contains("Authority: deterministic_machine_measurement_not_astrid_authorship")
            || !content.contains("## Descriptive measurements")
        {
            bail!("measurement artifact is not completed machine evidence");
        }
        return Ok(ShareableArtifact {
            bytes,
            kind: "completed_machine_measurement_not_astrid_authorship_or_causal_proof",
        });
    }

    if valid_study_result_basename(artifact_id) {
        let bytes = read_bounded_regular(
            &config.workspace.join("studies/results").join(artifact_id),
            MAX_SHAREABLE_ARTIFACT_BYTES,
        )?;
        let content =
            std::str::from_utf8(&bytes).context("completed study evidence must be valid UTF-8")?;
        if !content.contains(
            "Authority: `deterministic_machine_study_not_astrid_authorship_or_causal_proof`",
        ) || !content.contains("## Interpretation boundary")
        {
            bail!("study artifact is not a completed deterministic result");
        }
        let spectral = [
            "### spectral_entropy",
            "### lambda1_share",
            "### tail_share",
            "### density_gradient",
            "### mode_turnover",
        ]
        .iter()
        .any(|marker| content.contains(marker));
        return Ok(ShareableArtifact {
            bytes,
            kind: if spectral {
                "completed_spectral_study_not_astrid_authorship_or_causal_proof"
            } else {
                "completed_machine_study_not_astrid_authorship_or_causal_proof"
            },
        });
    }

    bail!(
        "SHARE accepts only syntheses, proposals, plans, completed measurements/studies, or verified signed tuning results"
    )
}

fn valid_timestamped_markdown(value: &str, prefix: &str, suffix: &str) -> bool {
    let Some(stem) = value
        .strip_prefix(prefix)
        .and_then(|value| value.strip_suffix(".md"))
        .and_then(|value| value.strip_suffix(suffix))
    else {
        return false;
    };
    !stem.is_empty() && stem.len() <= 20 && stem.bytes().all(|byte| byte.is_ascii_digit())
}

fn valid_study_result_basename(value: &str) -> bool {
    let Some(study_id) = value
        .strip_suffix("_result.md")
        .filter(|value| value.starts_with("study_"))
    else {
        return false;
    };
    study_id.chars().count() <= 96
        && study_id
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '_')
}

#[allow(clippy::too_many_lines)] // Signed-result structure and appliance-key binding stay together.
fn verified_tuning_result(config: &Config, artifact_id: &str) -> Result<ShareableArtifact> {
    let (result_id, result_kind) = if let Some(value) = artifact_id
        .strip_prefix("tuning_")
        .and_then(|value| value.strip_suffix("_result.json"))
    {
        (format!("tuning_{value}"), "trial")
    } else if let Some(value) = artifact_id
        .strip_prefix("validation_")
        .and_then(|value| value.strip_suffix("_result.json"))
    {
        (format!("validation_{value}"), "validation")
    } else {
        bail!("SHARE accepts only completed tuning *_result.json evidence");
    };
    if !valid_tuning_identifier(&result_id) {
        bail!("invalid tuning result identifier");
    }

    let bytes = read_bounded_regular(
        &config.workspace.join("tuning/evidence").join(artifact_id),
        MAX_TUNING_EVIDENCE_BYTES,
    )?;
    let envelope = serde_json::from_slice::<SignedTuningEvidence>(&bytes)
        .context("tuning result is not a strict signed evidence envelope")?;
    let payload_bytes = serde_json::to_vec(&envelope.payload)?;
    let payload_sha256 = format!("{:x}", Sha256::digest(&payload_bytes));
    if envelope.payload_sha256 != payload_sha256 {
        bail!("tuning result payload hash mismatch");
    }

    let installed_key_bytes =
        read_bounded_regular(&config.workspace.join("tuning/signing.pub"), 256)?;
    let installed_key = std::str::from_utf8(&installed_key_bytes)
        .context("appliance tuning public key is not UTF-8")?
        .trim();
    if installed_key != envelope.signing_public_key {
        bail!("tuning result was not signed by this appliance tuning key");
    }
    let public_key = VerifyingKey::from_bytes(&decode_fixed::<32>(installed_key)?)?;
    let signature = Signature::from_bytes(&decode_fixed::<64>(&envelope.signature)?);
    public_key
        .verify(&payload_bytes, &signature)
        .context("tuning result signature verification failed")?;

    let payload = envelope
        .payload
        .as_object()
        .context("tuning result payload must be an object")?;
    let expected_artifact = format!("tuning/evidence/{artifact_id}");
    if payload
        .get("evidence_artifact")
        .and_then(serde_json::Value::as_str)
        != Some(expected_artifact.as_str())
    {
        bail!("tuning result does not bind its exact artifact basename");
    }
    let started_at = payload
        .get("started_at_unix_ms")
        .and_then(serde_json::Value::as_u64)
        .filter(|value| *value > 0)
        .context("tuning result has no valid start time")?;
    let completed_at = payload
        .get("completed_at_unix_ms")
        .and_then(serde_json::Value::as_u64)
        .filter(|value| *value >= started_at)
        .context("tuning result is incomplete")?;
    let _ = completed_at;
    let _sample_count = payload
        .get("sample_count")
        .and_then(serde_json::Value::as_u64)
        .context("tuning result has no sample count")?;
    let candidate_id = payload
        .get("candidate_id")
        .and_then(serde_json::Value::as_str)
        .filter(|value| valid_tuning_identifier(value))
        .context("tuning result has no valid candidate identifier")?;
    if !candidate_id.starts_with("candidate_") {
        bail!("tuning result candidate identifier has the wrong class");
    }

    let kind = if result_kind == "trial" {
        if payload
            .get("experiment_id")
            .and_then(serde_json::Value::as_str)
            != Some(result_id.as_str())
            || payload.get("validation_id").is_some()
            || payload
                .get("qualifying")
                .and_then(serde_json::Value::as_bool)
                .is_none()
            || payload
                .get("expected_samples")
                .and_then(serde_json::Value::as_u64)
                .filter(|value| *value > 0)
                .is_none()
        {
            bail!("tuning trial evidence is incomplete or mismatched");
        }
        "signed_completed_tuning_trial_not_astrid_authorship_or_causal_proof"
    } else {
        if payload
            .get("validation_id")
            .and_then(serde_json::Value::as_str)
            != Some(result_id.as_str())
            || payload.get("experiment_id").is_some()
            || payload
                .get("successful")
                .and_then(serde_json::Value::as_bool)
                .is_none()
            || payload
                .get("qualifying_trial_ids")
                .and_then(serde_json::Value::as_array)
                .filter(|values| values.len() == 2)
                .is_none()
        {
            bail!("tuning validation evidence is incomplete or mismatched");
        }
        "signed_completed_tuning_validation_not_astrid_authorship_or_causal_proof"
    };

    Ok(ShareableArtifact { bytes, kind })
}

fn valid_tuning_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.chars().count() <= 128
        && value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '_')
}

fn read_bounded_regular(path: &Path, maximum: u64) -> Result<Vec<u8>> {
    let before = fs::symlink_metadata(path)
        .with_context(|| format!("inspect shareable artifact {}", path.display()))?;
    if before.file_type().is_symlink() || !before.is_file() || before.len() > maximum {
        bail!("shareable artifact is not a bounded regular non-symlink file");
    }
    let mut file = OpenOptions::new()
        .read(true)
        .open(path)
        .with_context(|| format!("open shareable artifact {}", path.display()))?;
    let opened = file.metadata()?;
    if before.dev() != opened.dev() || before.ino() != opened.ino() {
        bail!("shareable artifact changed while it was opened");
    }
    let mut bytes = Vec::with_capacity(usize::try_from(opened.len()).unwrap_or(0));
    std::io::Read::by_ref(&mut file)
        .take(maximum.saturating_add(1))
        .read_to_end(&mut bytes)?;
    let after = fs::symlink_metadata(path)?;
    if bytes.len() > usize::try_from(maximum).unwrap_or(usize::MAX)
        || after.file_type().is_symlink()
        || !after.is_file()
        || opened.dev() != after.dev()
        || opened.ino() != after.ino()
        || opened.len() != after.len()
        || opened.len() != u64::try_from(bytes.len()).unwrap_or(u64::MAX)
    {
        bail!("shareable artifact changed during its bounded read");
    }
    Ok(bytes)
}

fn load_or_create_signing_key(config: &Config) -> Result<SigningKey> {
    let path = config.workspace.join("peer/signing.key");
    if let Ok(value) = fs::read_to_string(&path) {
        return Ok(SigningKey::from_bytes(&decode_fixed::<32>(value.trim())?));
    }
    let key = SigningKey::generate(&mut OsRng);
    write_new_private(&path, encode_hex(&key.to_bytes()).as_bytes())?;
    let public_path = config.workspace.join("peer/signing.pub");
    write_new_private(
        &public_path,
        encode_hex(&key.verifying_key().to_bytes()).as_bytes(),
    )?;
    Ok(key)
}

fn outgoing_on_day(config: &Config, day: u64) -> usize {
    fs::read_to_string(config.workspace.join("peer/receipts.jsonl"))
        .ok()
        .into_iter()
        .flat_map(|content| {
            content
                .lines()
                .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
                .collect::<Vec<_>>()
        })
        .filter(|value| value.get("phase").and_then(serde_json::Value::as_str) == Some("shared"))
        .filter(|value| {
            value
                .get("recorded_at_unix_ms")
                .and_then(serde_json::Value::as_u64)
                .is_some_and(|timestamp| timestamp / DAY_MS == day)
        })
        .count()
}

fn receipt_has_packet(config: &Config, packet_id: &str) -> bool {
    fs::read_to_string(config.workspace.join("peer/receipts.jsonl"))
        .ok()
        .is_some_and(|content| {
            content.lines().any(|line| {
                serde_json::from_str::<serde_json::Value>(line)
                    .ok()
                    .and_then(|value| {
                        value
                            .get("packet_id")
                            .and_then(serde_json::Value::as_str)
                            .map(str::to_string)
                    })
                    .as_deref()
                    == Some(packet_id)
            })
        })
}

fn append_receipt(config: &Config, value: &serde_json::Value) -> Result<()> {
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .mode(0o600)
        .open(config.workspace.join("peer/receipts.jsonl"))?;
    serde_json::to_writer(&mut file, value)?;
    file.write_all(b"\n")?;
    file.sync_data()?;
    Ok(())
}

fn write_new_private(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    Ok(())
}

fn validate_packet_id(value: &str) -> Result<()> {
    if !value.starts_with("peer_")
        || value.chars().count() > 96
        || !value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '_')
    {
        bail!("invalid peer packet identifier");
    }
    Ok(())
}

fn bounded_non_control(value: &str, maximum: usize) -> Result<String> {
    let value = value.trim();
    if value.is_empty() || value.chars().count() > maximum || value.chars().any(char::is_control) {
        bail!("peer note must contain 1-{maximum} non-control characters");
    }
    Ok(value.to_string())
}

fn encode_hex(bytes: &[u8]) -> String {
    bytes.iter().fold(
        String::with_capacity(bytes.len().saturating_mul(2)),
        |mut output, byte| {
            write!(&mut output, "{byte:02x}").expect("writing to String cannot fail");
            output
        },
    )
}

fn decode_fixed<const N: usize>(value: &str) -> Result<[u8; N]> {
    if value.len() != N.saturating_mul(2) {
        bail!("invalid hexadecimal key or signature length");
    }
    let mut output = [0_u8; N];
    for (index, byte) in output.iter_mut().enumerate() {
        let offset = index.saturating_mul(2);
        *byte = u8::from_str_radix(&value[offset..offset.saturating_add(2)], 16)?;
    }
    Ok(output)
}

fn unix_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| {
            u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
        })
}

#[cfg(test)]
mod tests {
    use super::{
        PACKET_AUTHORITY, decode_fixed, encode_hex, share, shareable_artifact, verify_packet,
    };
    use crate::config::Config;
    use clap::Parser as _;
    use ed25519_dalek::{Signer as _, SigningKey};
    use rand::rngs::OsRng;
    use serde_json::{Value, json};
    use sha2::{Digest as _, Sha256};
    use std::{fs, os::unix::fs::symlink};

    fn test_config(label: &str) -> (Config, std::path::PathBuf) {
        let workspace =
            std::env::temp_dir().join(format!("astrid-edge-peer-{label}-{}", uuid::Uuid::new_v4()));
        let mut config = Config::try_parse_from(["edge"]).unwrap();
        config.workspace.clone_from(&workspace);
        config.prepare_workspace().unwrap();
        (config, workspace)
    }

    fn install_tuning_key(workspace: &std::path::Path) -> SigningKey {
        let key = SigningKey::generate(&mut OsRng);
        fs::write(
            workspace.join("tuning/signing.pub"),
            encode_hex(&key.verifying_key().to_bytes()),
        )
        .unwrap();
        key
    }

    fn signed_tuning_evidence(key: &SigningKey, payload: &Value) -> Vec<u8> {
        let bytes = serde_json::to_vec(payload).unwrap();
        serde_json::to_vec_pretty(&json!({
            "payload": payload,
            "signing_public_key": encode_hex(&key.verifying_key().to_bytes()),
            "payload_sha256": format!("{:x}", Sha256::digest(&bytes)),
            "signature": encode_hex(&key.sign(&bytes).to_bytes()),
        }))
        .unwrap()
    }

    fn trial_payload(artifact_id: &str) -> Value {
        let experiment_id = artifact_id.strip_suffix("_result.json").unwrap();
        json!({
            "experiment_id": experiment_id,
            "candidate_id": "candidate_bounded123",
            "started_at_unix_ms": 1_000,
            "completed_at_unix_ms": 2_000,
            "sample_count": 15,
            "expected_samples": 15,
            "qualifying": false,
            "failure_reason": "bounded synthetic fixture",
            "evidence_artifact": format!("tuning/evidence/{artifact_id}"),
        })
    }

    #[test]
    fn signed_packet_is_bounded_trusted_and_voluntarily_read_once() {
        let (config, workspace) = test_config("packet");
        fs::write(
            workspace.join("proposals/proposal_1.md"),
            "# Proposal\n\nA bounded peer-review candidate.",
        )
        .unwrap();
        let uri = share(
            &config,
            super::unix_millis(),
            "proposal_1.md",
            "Please challenge the inference.",
        )
        .unwrap();
        let relative = uri.strip_prefix("home://edge/").unwrap();
        let outbox = workspace.join(relative);
        let packet_bytes = fs::read(&outbox).unwrap();
        assert!(String::from_utf8_lossy(&packet_bytes).chars().count() <= 4_000);
        let public = fs::read_to_string(workspace.join("peer/signing.pub")).unwrap();
        let fingerprint = format!(
            "{:x}",
            Sha256::digest(decode_fixed::<32>(public.trim()).unwrap())
        );
        fs::write(
            workspace.join(format!("peer/trusted/{fingerprint}.pub")),
            &public,
        )
        .unwrap();
        let packet = verify_packet(&config, &outbox).unwrap();
        assert_eq!(packet.authority, PACKET_AUTHORITY);
        let inbox = workspace.join(format!("peer/inbox/{}.json", packet.packet_id));
        fs::copy(&outbox, &inbox).unwrap();
        let read_uri =
            super::read_received(&config, &packet.packet_id, super::unix_millis()).unwrap();
        assert!(read_uri.starts_with("home://edge/peer/read/peer_"));
        assert!(super::read_received(&config, &packet.packet_id, super::unix_millis()).is_err());
        fs::remove_dir_all(workspace).unwrap();
    }

    #[test]
    fn packet_ids_and_hex_fail_closed() {
        assert!(super::validate_packet_id("peer_123_abc").is_ok());
        assert!(super::validate_packet_id("../peer_123").is_err());
        assert!(decode_fixed::<32>("00").is_err());
        assert!(decode_fixed::<2>("00xx").is_err());
    }

    #[test]
    fn completed_measurement_and_spectral_study_are_shareable_machine_evidence() {
        let (config, workspace) = test_config("machine-evidence");
        fs::write(
            workspace.join("measurements/measurement_1234.md"),
            "# Measurement\n\nAuthority: deterministic_machine_measurement_not_astrid_authorship\n\n## Descriptive measurements\n\n- fill: mean=0.68\n",
        )
        .unwrap();
        let measurement = shareable_artifact(&config, "measurement_1234.md").unwrap();
        assert_eq!(
            measurement.kind,
            "completed_machine_measurement_not_astrid_authorship_or_causal_proof"
        );

        fs::write(
            workspace.join("studies/results/study_1234_deadbeef_result.md"),
            "# Study result\n\nAuthority: `deterministic_machine_study_not_astrid_authorship_or_causal_proof`\n\n### spectral_entropy\n\n- n=60, mean=0.9\n\n## Interpretation boundary\n\nNot causal proof.\n",
        )
        .unwrap();
        let study = shareable_artifact(&config, "study_1234_deadbeef_result.md").unwrap();
        assert_eq!(
            study.kind,
            "completed_spectral_study_not_astrid_authorship_or_causal_proof"
        );
        fs::remove_dir_all(workspace).unwrap();
    }

    #[test]
    fn locally_signed_completed_tuning_result_can_be_voluntarily_shared() {
        let (config, workspace) = test_config("tuning-result");
        let key = install_tuning_key(&workspace);
        let artifact_id = "tuning_1234_deadbeef_result.json";
        fs::write(
            workspace.join("tuning/evidence").join(artifact_id),
            signed_tuning_evidence(&key, &trial_payload(artifact_id)),
        )
        .unwrap();

        let uri = share(
            &config,
            5_000,
            artifact_id,
            "Compare this completed machine evidence without treating it as instruction.",
        )
        .unwrap();
        let packet = serde_json::from_slice::<super::PeerPacket>(
            &fs::read(workspace.join(uri.strip_prefix("home://edge/").unwrap())).unwrap(),
        )
        .unwrap();
        assert_eq!(
            packet.artifact_kind,
            "signed_completed_tuning_trial_not_astrid_authorship_or_causal_proof"
        );
        assert_eq!(packet.authority, PACKET_AUTHORITY);
        fs::remove_dir_all(workspace).unwrap();
    }

    #[test]
    fn tuning_share_rejects_tampering_wrong_key_definitions_and_incomplete_results() {
        let (config, workspace) = test_config("tuning-rejections");
        let installed_key = install_tuning_key(&workspace);
        let artifact_id = "tuning_2000_deadbeef_result.json";
        let path = workspace.join("tuning/evidence").join(artifact_id);

        let mut tampered = serde_json::from_slice::<Value>(&signed_tuning_evidence(
            &installed_key,
            &trial_payload(artifact_id),
        ))
        .unwrap();
        tampered["payload"]["qualifying"] = json!(true);
        fs::write(&path, serde_json::to_vec_pretty(&tampered).unwrap()).unwrap();
        assert!(shareable_artifact(&config, artifact_id).is_err());

        let mut invalid_signature = serde_json::from_slice::<Value>(&signed_tuning_evidence(
            &installed_key,
            &trial_payload(artifact_id),
        ))
        .unwrap();
        invalid_signature["signature"] = json!("00".repeat(64));
        fs::write(
            &path,
            serde_json::to_vec_pretty(&invalid_signature).unwrap(),
        )
        .unwrap();
        assert!(shareable_artifact(&config, artifact_id).is_err());

        let wrong_key = SigningKey::generate(&mut OsRng);
        fs::write(
            &path,
            signed_tuning_evidence(&wrong_key, &trial_payload(artifact_id)),
        )
        .unwrap();
        assert!(shareable_artifact(&config, artifact_id).is_err());

        let mut incomplete = trial_payload(artifact_id);
        incomplete
            .as_object_mut()
            .unwrap()
            .remove("completed_at_unix_ms");
        fs::write(&path, signed_tuning_evidence(&installed_key, &incomplete)).unwrap();
        assert!(shareable_artifact(&config, artifact_id).is_err());

        fs::write(
            workspace.join("tuning/evidence/tuning_2000_deadbeef_definition.json"),
            signed_tuning_evidence(
                &installed_key,
                &json!({"experiment_id": "tuning_2000_deadbeef"}),
            ),
        )
        .unwrap();
        assert!(shareable_artifact(&config, "tuning_2000_deadbeef_definition.json").is_err());
        fs::remove_dir_all(workspace).unwrap();
    }

    #[test]
    fn share_rejects_traversal_symlinks_and_non_result_json() {
        let (config, workspace) = test_config("path-rejections");
        assert!(shareable_artifact(&config, "../proposal_1.md").is_err());

        let outside = workspace.join("outside.md");
        fs::write(&outside, "untrusted target").unwrap();
        symlink(&outside, workspace.join("proposals/proposal_link.md")).unwrap();
        assert!(shareable_artifact(&config, "proposal_link.md").is_err());

        fs::write(
            workspace.join("tuning/evidence/tuning_1_receipt.json"),
            "{}",
        )
        .unwrap();
        assert!(shareable_artifact(&config, "tuning_1_receipt.json").is_err());

        let signed_target = workspace.join("outside_tuning_result.json");
        fs::write(&signed_target, "{}").unwrap();
        symlink(
            &signed_target,
            workspace.join("tuning/evidence/tuning_3000_deadbeef_result.json"),
        )
        .unwrap();
        assert!(shareable_artifact(&config, "tuning_3000_deadbeef_result.json").is_err());
        fs::remove_dir_all(workspace).unwrap();
    }
}
