//! Immutable provenance carried across scheduled-reflection context boundaries.
//!
//! Source, generation, and build evidence is authenticated by the appliance and
//! is therefore eligible to inform candidate authoring. Public web results and
//! mutable owned artifacts are useful reflective evidence, but are data rather
//! than code-authoring authority. Their content taint is durable and can never be
//! removed by summarizing, restarting, or reopening a candidate draft.

use serde::{Deserialize, Serialize};

use crate::util::{canonical_json, sha256, validate_hex64, validate_identifier};
use crate::{Error, Result};

pub(crate) const SCHEMA: &str = "astrid.edge.steward_helper.context_provenance.v1";
const CLEAN_AUTHORITY: &str =
    "clean_signed_local_context_untrusted_external_content_absent_candidate_authoring_eligible";
const TAINTED_AUTHORITY: &str =
    "untrusted_external_content_present_reflection_only_candidate_authoring_forbidden";
const LEGACY_AUTHORITY: &str =
    "legacy_unattributed_context_quarantined_candidate_authoring_forbidden";
const MAX_TAINT_SOURCES: usize = 16;
pub(crate) const CLEAN_SOURCE_LANE: &str = "clean_source_authoring_eligible";
pub(crate) const RICH_INTROSPECTION_LANE: &str = "rich_introspection_candidate_authoring_forbidden";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct TaintSource {
    pub kind: String,
    pub content_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ContextProvenance {
    pub schema: String,
    pub candidate_authoring_eligible: bool,
    pub untrusted_external_content: bool,
    pub taint_sources: Vec<TaintSource>,
    pub authority: String,
}

impl Default for ContextProvenance {
    fn default() -> Self {
        Self::legacy_unattributed()
    }
}

impl ContextProvenance {
    #[must_use]
    pub(crate) fn clean() -> Self {
        Self {
            schema: SCHEMA.to_owned(),
            candidate_authoring_eligible: true,
            untrusted_external_content: false,
            taint_sources: Vec::new(),
            authority: CLEAN_AUTHORITY.to_owned(),
        }
    }

    #[must_use]
    pub(crate) fn legacy_unattributed() -> Self {
        Self {
            schema: String::new(),
            candidate_authoring_eligible: false,
            untrusted_external_content: true,
            taint_sources: Vec::new(),
            authority: LEGACY_AUTHORITY.to_owned(),
        }
    }

    #[must_use]
    pub(crate) fn is_legacy(&self) -> bool {
        self.schema.is_empty() && self.authority == LEGACY_AUTHORITY
    }

    #[must_use]
    pub(crate) fn candidate_authoring_eligible(&self) -> bool {
        self.candidate_authoring_eligible && !self.untrusted_external_content
    }

    /// Return the active information-flow lane. This is derived rather than
    /// caller-selected, so a tool result cannot relabel a tainted reflection.
    #[must_use]
    pub(crate) fn reflection_lane(&self) -> &'static str {
        if self.candidate_authoring_eligible() {
            CLEAN_SOURCE_LANE
        } else {
            RICH_INTROSPECTION_LANE
        }
    }

    /// Return bounded cause classes only. Bodies never enter receipts or
    /// operator projections through this surface.
    #[must_use]
    pub(crate) fn taint_causes(&self) -> Vec<&str> {
        if self.is_legacy() {
            return vec!["legacy_unattributed_context"];
        }
        let mut causes = Vec::new();
        for source in &self.taint_sources {
            if !causes.contains(&source.kind.as_str()) {
                causes.push(source.kind.as_str());
            }
        }
        causes
    }

    pub(crate) fn mark_untrusted(&mut self, kind: &str, content_sha256: &str) -> Result<()> {
        validate_identifier(kind, "context taint source kind")?;
        validate_hex64(content_sha256, "context taint source hash")?;
        if self.is_legacy() {
            return Err(Error::new(
                "legacy unattributed context cannot be reclassified as clean or newly tainted",
            ));
        }
        let source = TaintSource {
            kind: kind.to_owned(),
            content_sha256: content_sha256.to_owned(),
        };
        if !self.taint_sources.contains(&source) {
            if self.taint_sources.len() >= MAX_TAINT_SOURCES {
                return Err(Error::new("context taint source bound exceeded"));
            }
            self.taint_sources.push(source);
        }
        self.candidate_authoring_eligible = false;
        self.untrusted_external_content = true;
        TAINTED_AUTHORITY.clone_into(&mut self.authority);
        self.validate()
    }

    pub(crate) fn validate(&self) -> Result<()> {
        if self.is_legacy() {
            if self.candidate_authoring_eligible || !self.untrusted_external_content {
                return Err(Error::new("legacy context provenance is inconsistent"));
            }
            return Ok(());
        }
        if self.schema != SCHEMA || self.taint_sources.len() > MAX_TAINT_SOURCES {
            return Err(Error::new("context provenance schema or bound is invalid"));
        }
        for (index, source) in self.taint_sources.iter().enumerate() {
            validate_identifier(&source.kind, "context taint source kind")?;
            validate_hex64(&source.content_sha256, "context taint source hash")?;
            if self.taint_sources[..index].contains(source) {
                return Err(Error::new("context taint source is duplicated"));
            }
        }
        let clean = self.taint_sources.is_empty();
        let consistent = if clean {
            self.candidate_authoring_eligible
                && !self.untrusted_external_content
                && self.authority == CLEAN_AUTHORITY
        } else {
            !self.candidate_authoring_eligible
                && self.untrusted_external_content
                && self.authority == TAINTED_AUTHORITY
        };
        if !consistent {
            return Err(Error::new("context provenance authority is inconsistent"));
        }
        Ok(())
    }

    pub(crate) fn digest(&self) -> Result<String> {
        self.validate()?;
        Ok(sha256(&canonical_json(self)?))
    }
}

#[cfg(test)]
mod tests {
    use super::ContextProvenance;

    #[test]
    fn untrusted_content_is_monotonic_and_digest_bound() {
        let mut provenance = ContextProvenance::clean();
        assert!(provenance.candidate_authoring_eligible());
        let clean_digest = provenance.digest().unwrap();
        provenance
            .mark_untrusted("search_web", &"a".repeat(64))
            .unwrap();
        assert!(!provenance.candidate_authoring_eligible());
        assert_ne!(clean_digest, provenance.digest().unwrap());
        assert!(
            provenance
                .mark_untrusted("read_owned", &"b".repeat(64))
                .is_ok()
        );
        assert!(!provenance.candidate_authoring_eligible());
    }

    #[test]
    fn legacy_context_is_quarantined_and_cannot_be_laundered() {
        let mut provenance = ContextProvenance::legacy_unattributed();
        assert!(provenance.validate().is_ok());
        assert!(!provenance.candidate_authoring_eligible());
        assert!(
            provenance
                .mark_untrusted("read_owned", &"a".repeat(64))
                .is_err()
        );
    }

    #[test]
    fn lane_and_cause_projection_is_derived_and_body_free() {
        let mut provenance = ContextProvenance::clean();
        assert_eq!(provenance.reflection_lane(), super::CLEAN_SOURCE_LANE);
        assert!(provenance.taint_causes().is_empty());
        provenance
            .mark_untrusted("read_owned", &"a".repeat(64))
            .unwrap();
        provenance
            .mark_untrusted("search_web", &"b".repeat(64))
            .unwrap();
        provenance
            .mark_untrusted("read_owned", &"c".repeat(64))
            .unwrap();
        assert_eq!(provenance.reflection_lane(), super::RICH_INTROSPECTION_LANE);
        assert_eq!(provenance.taint_causes(), ["read_owned", "search_web"]);
    }
}
