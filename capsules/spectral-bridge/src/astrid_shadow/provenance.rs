use serde_json::Value;

/// Read-only authorship metadata for the codec sample entering Astrid's
/// compatibility shadow projection. The existing ring and all shadow math stay
/// unchanged; the marker prevents a reflected Minime journal from masquerading
/// as pure Astrid authorship while that legacy mixed ring remains live.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AstridShadowInputProvenanceV1 {
    current_sample_origin: &'static str,
    current_sample_role: &'static str,
    source_id: String,
    astrid_authored: bool,
}

impl AstridShadowInputProvenanceV1 {
    #[must_use]
    pub fn astrid_authored(mode: &str) -> Self {
        let fallback = mode == "dialogue_fallback";
        Self {
            current_sample_origin: if fallback { "bridge" } else { "astrid" },
            current_sample_role: if fallback {
                "fixed_fallback_not_being_authored"
            } else {
                "astrid_authored_expression"
            },
            source_id: format!("astrid_mode:{mode}"),
            astrid_authored: !fallback,
        }
    }

    #[must_use]
    pub fn minime_mirror(source: &str) -> Self {
        let source = source.trim();
        Self {
            current_sample_origin: "minime",
            current_sample_role: "minime_owned_expression_reflected_for_compatibility",
            source_id: if source.is_empty() {
                "minime_journal:unknown".to_string()
            } else {
                format!("minime_journal:{source}")
            },
            astrid_authored: false,
        }
    }

    pub(super) fn annotate(&self, value: &mut Value) {
        let Some(object) = value.as_object_mut() else {
            return;
        };
        object.insert(
            "input_provenance_v1".to_string(),
            serde_json::json!({
                "schema": "astrid_shadow_input_provenance_v1",
                "schema_version": 1,
                "current_sample_origin": self.current_sample_origin,
                "current_sample_role": self.current_sample_role,
                "source_id": self.source_id,
                "current_sample_astrid_authored": self.astrid_authored,
                "window_classification": "mixed_legacy_untagged_compatibility",
                "compatibility_math_unchanged": true,
                "co_regulation_need_math_unchanged": true,
                "live_control_authority": false,
            }),
        );
    }
}
