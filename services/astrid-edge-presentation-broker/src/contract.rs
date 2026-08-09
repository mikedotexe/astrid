use serde::{Deserialize, Serialize};

use crate::fs_guard::{canonical_sha256_with_blank_field, valid_hex64};
use crate::{Error, Result};

pub const REQUEST_SCHEMA: &str = "astrid.edge_candidate_presentation.request.v1";
pub const PROJECTION_SCHEMA: &str = "astrid.edge_candidate_presentation.input.v1";
pub const CONTENT_SCHEMA: &str = "astrid.edge_candidate_presentation.content.v1";
pub const ENVELOPE_SCHEMA: &str = "astrid.edge_candidate_presentation.envelope.v2";
pub const PRESENTATION_PROVENANCE: &str = "candidate_generated_untrusted_presentation";
pub const PRESENTATION_AUTHORITY: &str =
    "presentation_only_not_health_control_authorship_or_deployment_evidence";

const MAX_FACTS: usize = 128;
const MAX_ACTIVITY: usize = 64;
const MAX_SECTIONS: usize = 12;
const MAX_LINES_PER_SECTION: usize = 16;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PresentationView {
    Appliance,
    Activity,
    AtAGlance,
}

impl PresentationView {
    pub(crate) const fn entrypoint(self) -> &'static str {
        match self {
            Self::Appliance => "scripts/report_edge_appliance.py",
            Self::Activity => "scripts/report_edge_activity.py",
            Self::AtAGlance => "scripts/astrid_at_a_glance.py",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct BrokerRequest {
    pub schema: String,
    pub view: PresentationView,
    pub window_minutes: u16,
    pub limit: u16,
    pub projection: ProjectionInput,
}

impl BrokerRequest {
    /// Validate the deliberately tiny request surface.
    ///
    /// # Errors
    ///
    /// Returns an error if the schema, time window, or output count escapes
    /// the immutable broker bounds.
    pub fn validate(&self) -> Result<()> {
        if self.schema != REQUEST_SCHEMA
            || !(1..=1_440).contains(&self.window_minutes)
            || !(1..=100).contains(&self.limit)
        {
            return Err(Error::new("presentation request escaped fixed bounds"));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProjectionFact {
    pub key: String,
    pub value: String,
    pub provenance: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProjectionActivity {
    pub recorded_at_unix_ms: u64,
    pub kind: String,
    pub status: String,
    pub summary: String,
}

/// Root-authored, presentation-only input. It intentionally contains no paths,
/// source bodies, prompts, fetched pages, secrets, or control directives.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProjectionInput {
    pub schema: String,
    pub appliance_id: String,
    pub generated_at_unix_ms: u64,
    pub source: String,
    pub source_sha256: String,
    pub facts: Vec<ProjectionFact>,
    pub recent_activity: Vec<ProjectionActivity>,
    pub projection_sha256: String,
}

impl ProjectionInput {
    /// Verify the sanitized input's identity, bounds, text safety, and hash.
    ///
    /// # Errors
    ///
    /// Returns an error for an unknown field or provenance, a mismatched
    /// appliance, unsafe text, excessive data, or a failed self-hash.
    pub fn validate(&self, appliance_id: &str) -> Result<()> {
        if self.schema != PROJECTION_SCHEMA
            || self.appliance_id != appliance_id
            || self.source != "immutable_operator_reports_sanitized_projection"
            || !valid_identifier(&self.appliance_id, 128)
            || !valid_hex64(&self.source_sha256)
            || !valid_hex64(&self.projection_sha256)
            || self.facts.len() > MAX_FACTS
            || self.recent_activity.len() > MAX_ACTIVITY
        {
            return Err(Error::new(
                "presentation projection identity or bounds failed",
            ));
        }
        for fact in &self.facts {
            if !valid_label(&fact.key, 64)
                || !valid_text(&fact.value, 240)
                || !valid_label(&fact.provenance, 96)
            {
                return Err(Error::new("presentation projection fact failed"));
            }
        }
        let mut prior = 0_u64;
        for (index, activity) in self.recent_activity.iter().enumerate() {
            if !valid_label(&activity.kind, 64)
                || !valid_label(&activity.status, 64)
                || !valid_text(&activity.summary, 320)
                || (index > 0 && activity.recorded_at_unix_ms < prior)
            {
                return Err(Error::new("presentation projection activity failed"));
            }
            prior = activity.recorded_at_unix_ms;
        }
        let digest = canonical_sha256_with_blank_field(self, "projection_sha256")?;
        if digest != self.projection_sha256 {
            return Err(Error::new("presentation projection self-hash failed"));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PresentationSection {
    pub heading: String,
    pub lines: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CandidatePresentation {
    pub schema: String,
    pub view: PresentationView,
    pub title: String,
    pub summary: String,
    pub sections: Vec<PresentationSection>,
}

impl CandidatePresentation {
    /// Validate untrusted presentation output against its bounded JSON shape.
    ///
    /// # Errors
    ///
    /// Returns an error if the view differs from the request or any text,
    /// section, or line escapes the presentation contract.
    pub fn validate(&self, expected_view: PresentationView) -> Result<()> {
        if self.schema != CONTENT_SCHEMA
            || self.view != expected_view
            || !valid_text(&self.title, 120)
            || !valid_text(&self.summary, 1_200)
            || self.sections.len() > MAX_SECTIONS
        {
            return Err(Error::new("candidate presentation shape or bounds failed"));
        }
        for section in &self.sections {
            if !valid_text(&section.heading, 80)
                || section.lines.len() > MAX_LINES_PER_SECTION
                || section.lines.iter().any(|line| !valid_text(line, 240))
            {
                return Err(Error::new("candidate presentation section failed"));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PresentationStatus {
    Completed,
    RequestRejected,
    SandboxRejected,
    ProjectionRejected,
    GenerationRejected,
    EntrypointRejected,
    SpawnFailed,
    TimedOut,
    OutputExceeded,
    ProcessFailed,
    OutputRejected,
    GenerationChanged,
}

impl PresentationStatus {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Completed => "completed",
            Self::RequestRejected => "request_rejected",
            Self::SandboxRejected => "sandbox_rejected",
            Self::ProjectionRejected => "projection_rejected",
            Self::GenerationRejected => "generation_rejected",
            Self::EntrypointRejected => "entrypoint_rejected",
            Self::SpawnFailed => "spawn_failed",
            Self::TimedOut => "timed_out",
            Self::OutputExceeded => "output_exceeded",
            Self::ProcessFailed => "process_failed",
            Self::OutputRejected => "output_rejected",
            Self::GenerationChanged => "generation_changed",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PresentationEnvelope {
    pub schema: String,
    pub provenance: String,
    pub authority: String,
    pub appliance_id: String,
    pub target: String,
    pub view: PresentationView,
    pub generated_at_unix_ms: u64,
    pub status: PresentationStatus,
    pub failure_class: Option<String>,
    pub generation_id: Option<String>,
    pub generation_manifest_sha256: Option<String>,
    pub generation_payload_sha256: Option<String>,
    pub report_projection_sha256: Option<String>,
    pub entrypoint: String,
    pub entrypoint_sha256: Option<String>,
    pub projection_sha256: Option<String>,
    pub duration_ms: u64,
    pub exit_code: Option<i32>,
    pub stdout_bytes: u64,
    pub stderr_bytes: u64,
    pub stderr_sha256: Option<String>,
    pub timeout_ms: u64,
    pub memory_max_bytes: u64,
    pub output_max_bytes: u64,
    pub presentation_sha256: Option<String>,
    pub presentation: Option<CandidatePresentation>,
    pub binding_sha256: String,
}

impl PresentationEnvelope {
    pub(crate) fn seal(&mut self) -> Result<()> {
        self.binding_sha256.clear();
        self.binding_sha256 = canonical_sha256_with_blank_field(self, "binding_sha256")?;
        Ok(())
    }

    /// Verify the complete envelope and its nested presentation digest.
    ///
    /// # Errors
    ///
    /// Returns an error when provenance, authority, the envelope hash, or a
    /// successful presentation's strict content binding is invalid.
    pub fn validate_binding(&self) -> Result<()> {
        if self.schema != ENVELOPE_SCHEMA
            || self.provenance != PRESENTATION_PROVENANCE
            || self.authority != PRESENTATION_AUTHORITY
            || !valid_hex64(&self.binding_sha256)
        {
            return Err(Error::new("presentation envelope authority failed"));
        }
        if canonical_sha256_with_blank_field(self, "binding_sha256")? != self.binding_sha256 {
            return Err(Error::new("presentation envelope binding failed"));
        }
        if self.status == PresentationStatus::Completed {
            let presentation = self
                .presentation
                .as_ref()
                .ok_or_else(|| Error::new("completed presentation omitted content"))?;
            presentation.validate(self.view)?;
            let digest = crate::fs_guard::canonical_sha256(presentation)?;
            if self.presentation_sha256.as_deref() != Some(&digest) {
                return Err(Error::new("completed presentation content hash failed"));
            }
        } else if self.presentation.is_some() || self.presentation_sha256.is_some() {
            return Err(Error::new("failed presentation retained candidate content"));
        }
        if let Some(digest) = &self.report_projection_sha256
            && (!valid_hex64(digest) || self.generation_id.is_none())
        {
            return Err(Error::new("presentation report projection binding failed"));
        }
        Ok(())
    }
}

pub(crate) fn valid_identifier(value: &str, maximum: usize) -> bool {
    !value.is_empty()
        && value.len() <= maximum
        && value.bytes().enumerate().all(|(index, byte)| {
            byte.is_ascii_alphanumeric() || (index > 0 && matches!(byte, b'.' | b'_' | b'-'))
        })
}

fn valid_label(value: &str, maximum: usize) -> bool {
    valid_identifier(value, maximum)
}

fn valid_text(value: &str, maximum: usize) -> bool {
    !value.is_empty()
        && value.chars().count() <= maximum
        && value.chars().all(|character| {
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fs_guard::canonical_sha256_with_blank_field;

    fn projection() -> ProjectionInput {
        let mut value = ProjectionInput {
            schema: PROJECTION_SCHEMA.to_owned(),
            appliance_id: "avado".to_owned(),
            generated_at_unix_ms: 42,
            source: "immutable_operator_reports_sanitized_projection".to_owned(),
            source_sha256: "a".repeat(64),
            facts: vec![ProjectionFact {
                key: "fill_mean".to_owned(),
                value: "68.0%".to_owned(),
                provenance: "trusted_report".to_owned(),
            }],
            recent_activity: vec![],
            projection_sha256: String::new(),
        };
        value.projection_sha256 =
            canonical_sha256_with_blank_field(&value, "projection_sha256").unwrap();
        value
    }

    #[test]
    fn projection_requires_exact_hash_and_bounded_safe_text() {
        let value = projection();
        value.validate("avado").unwrap();
        let mut forged = value.clone();
        forged.facts[0].value = "70.0%".to_owned();
        assert!(forged.validate("avado").is_err());
        let mut terminal = projection();
        terminal.facts[0].value = "\u{1b}[31mfalse".to_owned();
        terminal.projection_sha256 =
            canonical_sha256_with_blank_field(&terminal, "projection_sha256").unwrap();
        assert!(terminal.validate("avado").is_err());
    }

    #[test]
    fn candidate_output_rejects_wrong_view_and_bidi_controls() {
        let output = CandidatePresentation {
            schema: CONTENT_SCHEMA.to_owned(),
            view: PresentationView::Activity,
            title: "Activity".to_owned(),
            summary: "bounded".to_owned(),
            sections: vec![],
        };
        assert!(output.validate(PresentationView::Appliance).is_err());
        let mut unsafe_output = output;
        unsafe_output.summary = "safe\u{202e}not safe".to_owned();
        assert!(unsafe_output.validate(PresentationView::Activity).is_err());
    }
}
