use std::fs;
use std::io::{Read, Write};
use std::os::unix::fs::{FileTypeExt, MetadataExt, PermissionsExt};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::contract::{
    BrokerRequest, PROJECTION_SCHEMA, ProjectionActivity, ProjectionFact, ProjectionInput,
    REQUEST_SCHEMA, valid_identifier,
};
use crate::fs_guard::{canonical_json, canonical_sha256_with_blank_field, sha256};
use crate::{Error, PresentationEnvelope, PresentationStatus, PresentationView, Result};

pub const BROKER_SOCKET: &str = "/run/astrid-edge-presentation/broker.sock";
const MAX_TRUSTED_REPORT_BYTES: usize = 256 * 1024;
const MAX_BROKER_RESPONSE_BYTES: usize = 96 * 1024;
const MAX_FACTS: usize = 128;
const MAX_ACTIVITY: usize = 64;

const APPLIANCE_FACT_ALLOWLIST: &[&str] = &[
    "report_version",
    "instance_name",
    "hostname",
    "astrid_service_state",
    "astrid_service_restarts",
    "edge_service_state",
    "edge_service_restarts",
    "selected_model",
    "selected_model_context",
    "selected_model_max_output",
    "autonomy_attempts_today",
    "autonomy_authored_turns_today",
    "autonomy_transport_recoveries_today",
    "autonomy_total_attempts",
    "autonomy_total_authored_turns",
    "autonomy_total_transport_recoveries",
    "autonomy_last_prompt_chars",
    "ollama_loaded_model_count",
    "ollama_loaded_models",
    "memory_available_mib",
    "swap_used_mib",
    "audio_fresh",
    "audio_source",
    "aux_fresh",
    "aux_source",
    "current_fill_pct",
    "target_fill_pct",
    "effective_dimensionality",
    "fill_settled_mean_pct",
    "fill_settled_min_pct",
    "fill_settled_max_pct",
    "fill_settled_inside_65_73_5_pct",
    "hindsight_continuity_status",
    "hindsight_continuity_from_previous_checkpoint_valid",
    "hindsight_current_epoch_integrity_violation_count",
    "scheduled_introspection_latest_status",
    "scheduled_introspection_latest_response_sha256",
    "self_change_enabled",
    "self_change_mode",
    "self_change_active_generation",
    "self_change_previous_generation",
    "self_change_operator_pipeline_phase",
    "self_change_latest_build_status",
    "self_change_latest_activation_status",
    "self_change_probation_status",
    "self_change_latest_rollback_status",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientFormat {
    Text,
    KeyValue,
    Json,
}

#[derive(Debug, Clone)]
pub struct ClientOptions {
    pub appliance_id: String,
    pub view: PresentationView,
    pub window_minutes: u16,
    pub limit: u16,
    pub format: ClientFormat,
}

/// Send one immutable-report projection to the local presentation broker.
///
/// # Errors
///
/// Returns an error for malformed client options, excessive trusted report
/// input, an unsafe broker socket, transport failure, or an invalid broker
/// response. Candidate failures remain successful, explicitly labeled
/// presentation envelopes rather than client errors.
pub fn run_client(options: &ClientOptions, trusted_report: &[u8]) -> Result<String> {
    validate_options(options, trusted_report)?;
    verify_socket(Path::new(BROKER_SOCKET), true)?;
    let projection = build_projection(options, trusted_report)?;
    let request = BrokerRequest {
        schema: REQUEST_SCHEMA.to_owned(),
        view: options.view,
        window_minutes: options.window_minutes,
        limit: options.limit,
        projection,
    };
    request.validate()?;
    let response = exchange(Path::new(BROKER_SOCKET), &request)?;
    response.validate_binding()?;
    if response.appliance_id != options.appliance_id || response.view != options.view {
        return Err(Error::new(
            "presentation response identity differs from request",
        ));
    }
    render(&response, options.format)
}

fn validate_options(options: &ClientOptions, trusted_report: &[u8]) -> Result<()> {
    if !valid_identifier(&options.appliance_id, 128)
        || !(1..=1_440).contains(&options.window_minutes)
        || !(1..=100).contains(&options.limit)
        || trusted_report.is_empty()
        || trusted_report.len() > MAX_TRUSTED_REPORT_BYTES
    {
        return Err(Error::new(
            "presentation client options or input escaped bounds",
        ));
    }
    Ok(())
}

fn verify_socket(path: &Path, require_root_owner: bool) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| Error::new("presentation socket has no parent"))?;
    let parent_metadata = fs::symlink_metadata(parent)?;
    let socket_metadata = fs::symlink_metadata(path)?;
    if !parent_metadata.is_dir()
        || parent_metadata.file_type().is_symlink()
        || parent_metadata.permissions().mode() & 0o022 != 0
        || (require_root_owner && parent_metadata.uid() != 0)
        || !socket_metadata.file_type().is_socket()
        || socket_metadata.file_type().is_symlink()
        || socket_metadata.nlink() != 1
        || socket_metadata.permissions().mode() & 0o007 != 0
        || (require_root_owner && socket_metadata.uid() != 0)
    {
        return Err(Error::new("presentation broker socket identity is unsafe"));
    }
    Ok(())
}

fn build_projection(options: &ClientOptions, trusted_report: &[u8]) -> Result<ProjectionInput> {
    let text = std::str::from_utf8(trusted_report)
        .map_err(|_| Error::new("trusted report is not UTF-8"))?;
    let generated_at_unix_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| {
            u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
        });
    let (facts, recent_activity) = match options.view {
        PresentationView::Appliance | PresentationView::AtAGlance => {
            (appliance_facts(text), Vec::new())
        },
        PresentationView::Activity => (Vec::new(), activity_lines(text, generated_at_unix_ms)),
    };
    let mut value = ProjectionInput {
        schema: PROJECTION_SCHEMA.to_owned(),
        appliance_id: options.appliance_id.clone(),
        generated_at_unix_ms,
        source: "immutable_operator_reports_sanitized_projection".to_owned(),
        source_sha256: sha256(trusted_report),
        facts,
        recent_activity,
        projection_sha256: String::new(),
    };
    value.projection_sha256 = canonical_sha256_with_blank_field(&value, "projection_sha256")?;
    value.validate(&options.appliance_id)?;
    Ok(value)
}

fn appliance_facts(text: &str) -> Vec<ProjectionFact> {
    text.lines()
        .filter_map(|line| line.split_once('='))
        .filter(|(key, _)| APPLIANCE_FACT_ALLOWLIST.contains(key))
        .filter_map(|(key, value)| {
            let value = sanitize(value, 240);
            (!value.is_empty()).then(|| ProjectionFact {
                key: key.to_owned(),
                value,
                provenance: "immutable_operator_report".to_owned(),
            })
        })
        .take(MAX_FACTS)
        .collect()
}

fn activity_lines(text: &str, recorded_at_unix_ms: u64) -> Vec<ProjectionActivity> {
    text.lines()
        .filter(|line| {
            let line = line.trim();
            !line.is_empty()
                && !line.starts_with("RESPONSE_PROVENANCE_COUNTS")
                && !line.contains("/home/")
                && !line.contains("/media/")
        })
        .map(|line| ProjectionActivity {
            recorded_at_unix_ms,
            kind: "trusted_activity_line".to_owned(),
            status: "observed".to_owned(),
            summary: sanitize(line, 320),
        })
        .filter(|item| !item.summary.is_empty())
        .take(MAX_ACTIVITY)
        .collect()
}

fn sanitize(value: &str, maximum: usize) -> String {
    value
        .chars()
        .filter(|character| {
            !character.is_control()
                && !matches!(
                    character,
                    '\u{061c}'
                        | '\u{200e}'
                        | '\u{200f}'
                        | '\u{202a}'..='\u{202e}'
                        | '\u{2066}'..='\u{2069}'
                )
        })
        .take(maximum)
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn exchange(path: &Path, request: &BrokerRequest) -> Result<PresentationEnvelope> {
    let mut stream = UnixStream::connect(path)?;
    let timeout = Some(Duration::from_secs(35));
    stream.set_read_timeout(timeout)?;
    stream.set_write_timeout(timeout)?;
    let request = canonical_json(request)?;
    stream.write_all(&request)?;
    stream.shutdown(std::net::Shutdown::Write)?;
    let maximum = MAX_BROKER_RESPONSE_BYTES
        .checked_add(1)
        .ok_or_else(|| Error::new("presentation response bound overflow"))?;
    let mut response = Vec::with_capacity(16 * 1024);
    stream.take(maximum as u64).read_to_end(&mut response)?;
    if response.len() > MAX_BROKER_RESPONSE_BYTES {
        return Err(Error::new(
            "presentation broker response exceeded its bound",
        ));
    }
    serde_json::from_slice(&response).map_err(Into::into)
}

fn render(value: &PresentationEnvelope, format: ClientFormat) -> Result<String> {
    match format {
        ClientFormat::Json => {
            String::from_utf8(canonical_json(value)?).map_err(|_| Error::new("JSON is not UTF-8"))
        },
        ClientFormat::KeyValue => Ok(render_key_value(value)),
        ClientFormat::Text => Ok(render_text(value)),
    }
}

fn render_key_value(value: &PresentationEnvelope) -> String {
    let mut lines = vec![
        format!("candidate_presentation_provenance={}", value.provenance),
        format!("candidate_presentation_authority={}", value.authority),
        format!("candidate_presentation_status={}", value.status.as_str()),
        format!(
            "candidate_presentation_generation={}",
            value.generation_id.as_deref().unwrap_or("unavailable")
        ),
        format!(
            "candidate_presentation_entrypoint_sha256={}",
            value.entrypoint_sha256.as_deref().unwrap_or("unavailable")
        ),
        format!(
            "candidate_presentation_report_projection_sha256={}",
            value
                .report_projection_sha256
                .as_deref()
                .unwrap_or("initial_generation_inventory_bound")
        ),
        format!(
            "candidate_presentation_binding_sha256={}",
            value.binding_sha256
        ),
    ];
    if let Some(failure) = &value.failure_class {
        lines.push(format!("candidate_presentation_failure_class={failure}"));
    }
    if let Some(presentation) = &value.presentation {
        lines.push(format!(
            "candidate_presentation_title={}",
            presentation.title
        ));
        lines.push(format!(
            "candidate_presentation_summary={}",
            presentation.summary
        ));
        for (section_index, section) in presentation.sections.iter().enumerate() {
            for (line_index, line) in section.lines.iter().enumerate() {
                lines.push(format!(
                    "candidate_presentation_section_{section_index}_{line_index}={}: {line}",
                    section.heading
                ));
            }
        }
    }
    lines.join("\n")
}

fn render_text(value: &PresentationEnvelope) -> String {
    let mut lines = vec![
        String::new(),
        "Candidate-generated presentation (UNTRUSTED; presentation only)".to_owned(),
        format!(
            "  status={:?} generation={} entrypoint_sha256={} report_projection_sha256={}",
            value.status.as_str(),
            value.generation_id.as_deref().unwrap_or("unavailable"),
            value.entrypoint_sha256.as_deref().unwrap_or("unavailable"),
            value
                .report_projection_sha256
                .as_deref()
                .unwrap_or("initial_generation_inventory_bound")
        ),
        format!("  binding_sha256={}", value.binding_sha256),
    ];
    if let Some(failure) = &value.failure_class {
        lines.push(format!("  unavailable: {failure}"));
    }
    if value.status == PresentationStatus::Completed
        && let Some(presentation) = &value.presentation
    {
        lines.push(format!(
            "  {} — {}",
            presentation.title, presentation.summary
        ));
        for section in &presentation.sections {
            lines.push(format!("  [{}]", section.heading));
            lines.extend(section.lines.iter().map(|line| format!("    {line}")));
        }
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn options(view: PresentationView) -> ClientOptions {
        ClientOptions {
            appliance_id: "avado".to_owned(),
            view,
            window_minutes: 60,
            limit: 10,
            format: ClientFormat::Text,
        }
    }

    #[test]
    fn appliance_projection_is_allowlisted_and_path_free() {
        let input = b"report_version=16\nfill_settled_mean_pct=68.0\nsecret_key=never\npath=/home/user/private\n";
        let value = build_projection(&options(PresentationView::Appliance), input).unwrap();
        assert_eq!(value.facts.len(), 2);
        assert!(value.facts.iter().all(|item| item.key != "secret_key"));
        assert!(!serde_json::to_string(&value).unwrap().contains("/home/"));
        value.validate("avado").unwrap();
    }

    #[test]
    fn activity_projection_strips_controls_and_paths() {
        let input = b"ACTION JOURNAL test\n/home/user/private\nWEB search\x1b[31m\n";
        let value = build_projection(&options(PresentationView::Activity), input).unwrap();
        assert_eq!(value.recent_activity.len(), 2);
        let encoded = serde_json::to_string(&value).unwrap();
        assert!(!encoded.contains("/home/"));
        assert!(!encoded.contains("\\u001b"));
    }

    #[test]
    fn renderer_never_blurs_candidate_output_with_trusted_status() {
        let mut value = PresentationEnvelope {
            schema: crate::contract::ENVELOPE_SCHEMA.to_owned(),
            provenance: crate::contract::PRESENTATION_PROVENANCE.to_owned(),
            authority: crate::contract::PRESENTATION_AUTHORITY.to_owned(),
            appliance_id: "avado".to_owned(),
            target: "x86_64-unknown-linux-gnu".to_owned(),
            view: PresentationView::Appliance,
            generated_at_unix_ms: 1,
            status: PresentationStatus::OutputRejected,
            failure_class: Some("candidate_entrypoint_json_contract_rejected".to_owned()),
            generation_id: Some("gen-a".to_owned()),
            generation_manifest_sha256: Some("a".repeat(64)),
            generation_payload_sha256: Some("b".repeat(64)),
            report_projection_sha256: Some("e".repeat(64)),
            entrypoint: "scripts/report_edge_appliance.py".to_owned(),
            entrypoint_sha256: Some("c".repeat(64)),
            projection_sha256: Some("d".repeat(64)),
            duration_ms: 1,
            exit_code: Some(0),
            stdout_bytes: 10,
            stderr_bytes: 0,
            stderr_sha256: None,
            timeout_ms: 1_000,
            memory_max_bytes: 128 * 1024 * 1024,
            output_max_bytes: 8_192,
            presentation_sha256: None,
            presentation: None,
            binding_sha256: String::new(),
        };
        value.seal().unwrap();
        let text = render(&value, ClientFormat::Text).unwrap();
        assert!(text.contains("UNTRUSTED; presentation only"));
        assert!(text.contains("unavailable"));
        assert!(!text.contains("healthy"));
    }
}
