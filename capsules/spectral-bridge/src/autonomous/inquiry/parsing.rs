use astrid_minime_protocol::{
    OWNER_INQUIRY_MAX_STRANDS_V1, OWNER_INQUIRY_MIN_STRANDS_V1, SEMANTIC_STRAND_SCHEMA_V1,
    SemanticStrandProvenanceV1, SemanticStrandV1, VolitionBudgetV1,
};

use super::research::OwnerDecisionDraftRecipeV1;
use super::*;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct InquiryStartRecipeV1 {
    #[serde(default)]
    inquiry_id: Option<String>,
    question: String,
    strands: Vec<StrandSelectionV1>,
    #[serde(default)]
    owner_priority: u16,
    #[serde(default)]
    dependency_inquiry_ids: Vec<String>,
    #[serde(default = "default_inquiry_budget")]
    budget: VolitionBudgetV1,
    #[serde(default)]
    decision_plan: Option<OwnerDecisionDraftRecipeV1>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct StrandSelectionV1 {
    label: String,
    #[serde(default)]
    start_byte: Option<u64>,
    #[serde(default)]
    end_byte: Option<u64>,
    #[serde(default)]
    text: Option<String>,
}

#[derive(Debug)]
pub(super) struct ParsedStartV1 {
    pub(super) inquiry_id: String,
    pub(super) question: String,
    pub(super) selections: Vec<ResolvedSelectionV1>,
    pub(super) owner_priority: u16,
    pub(super) dependency_inquiry_ids: Vec<String>,
    pub(super) budget: VolitionBudgetV1,
    pub(super) decision_plan: Option<OwnerDecisionDraftRecipeV1>,
}

#[derive(Debug)]
pub(super) struct ResolvedSelectionV1 {
    pub(super) label: String,
    pub(super) start: usize,
    pub(super) end: usize,
}

pub(super) fn parse_start_recipe(
    payload: &str,
    response_text: &str,
    intent_id: &str,
) -> Result<ParsedStartV1, String> {
    if payload.trim_start().starts_with('{') {
        let recipe: InquiryStartRecipeV1 = serde_json::from_str(payload)
            .map_err(|error| format!("INQUIRY_START JSON is malformed: {error}"))?;
        let selections = recipe
            .strands
            .into_iter()
            .map(|selection| resolve_selection(selection, response_text))
            .collect::<Result<Vec<_>, _>>()?;
        validate_selection_count(&selections)?;
        return Ok(ParsedStartV1 {
            inquiry_id: recipe
                .inquiry_id
                .unwrap_or_else(|| generated_inquiry_id(intent_id)),
            question: recipe.question,
            selections,
            owner_priority: recipe.owner_priority,
            dependency_inquiry_ids: recipe.dependency_inquiry_ids,
            budget: recipe.budget,
            decision_plan: recipe.decision_plan,
        });
    }
    let quoted = quoted_contents(payload)?;
    if !(OWNER_INQUIRY_MIN_STRANDS_V1..=OWNER_INQUIRY_MAX_STRANDS_V1).contains(&quoted.len()) {
        return Err(clarification_message());
    }
    let strand_source = response_text
        .rfind("NEXT:")
        .map_or(response_text, |next_start| &response_text[..next_start]);
    let mut selections = Vec::with_capacity(quoted.len());
    for (index, content) in quoted.iter().enumerate() {
        let (start, end) =
            find_unique_exact_span(strand_source, content).ok_or_else(clarification_message)?;
        selections.push(ResolvedSelectionV1 {
            label: format!("strand-{}", index.saturating_add(1)),
            start,
            end,
        });
    }
    validate_selection_count(&selections)?;
    let first_quote = payload.find(['"', '“']).unwrap_or(payload.len());
    let question = payload[..first_quote]
        .trim()
        .trim_matches([':', '-', '—'])
        .trim();
    Ok(ParsedStartV1 {
        inquiry_id: generated_inquiry_id(intent_id),
        question: if question.is_empty() {
            "How do these exact strands remain distinct and interact?".to_string()
        } else {
            question.to_string()
        },
        selections,
        owner_priority: 0,
        dependency_inquiry_ids: Vec::new(),
        budget: default_inquiry_budget(),
        decision_plan: None,
    })
}

fn resolve_selection(
    selection: StrandSelectionV1,
    response_text: &str,
) -> Result<ResolvedSelectionV1, String> {
    if selection.label.trim().is_empty() {
        return Err("each strand needs a bounded non-empty label".to_string());
    }
    match (
        selection.start_byte,
        selection.end_byte,
        selection.text.as_deref(),
    ) {
        (Some(start), Some(end), None) => {
            let start = usize::try_from(start)
                .map_err(|_| "strand start byte does not fit this runtime".to_string())?;
            let end = usize::try_from(end)
                .map_err(|_| "strand end byte does not fit this runtime".to_string())?;
            if start >= end
                || !response_text.is_char_boundary(start)
                || !response_text.is_char_boundary(end)
                || response_text.get(start..end).is_none()
            {
                return Err(
                    "explicit strand byte intervals must be exact UTF-8 boundaries inside the attested response"
                        .to_string(),
                );
            }
            Ok(ResolvedSelectionV1 {
                label: selection.label,
                start,
                end,
            })
        },
        (None, None, Some(text)) => {
            let strand_source = response_text
                .rfind("NEXT:")
                .map_or(response_text, |next_start| &response_text[..next_start]);
            let (start, end) = find_unique_exact_span(strand_source, text)
                .ok_or_else(clarification_message)?;
            Ok(ResolvedSelectionV1 {
                label: selection.label,
                start,
                end,
            })
        },
        _ => Err(
            "each strand must provide either start_byte+end_byte or one unique exact text, never both"
                .to_string(),
        ),
    }
}

pub(super) fn build_strand(
    index: usize,
    selection: ResolvedSelectionV1,
    response_text: &str,
    attestation: &BeingUtteranceAttestationV1,
    attestation_sha256: &str,
) -> Result<SemanticStrandV1, String> {
    let content = response_text
        .get(selection.start..selection.end)
        .ok_or_else(|| "strand interval is not valid UTF-8".to_string())?
        .to_string();
    let projection_48d = crate::codec::encode_text(&content);
    if projection_48d.len() != 48 || !projection_48d.iter().all(|value| value.is_finite()) {
        return Err("Astrid codec did not produce a finite 48D strand projection".to_string());
    }
    let companion_projection_12d = crate::codec::GlimpseCodec::derive_12d(&projection_48d)
        .map(|values| values.to_vec())
        .ok_or_else(|| "Astrid codec could not derive the 12D companion projection".to_string())?;
    let response_start_byte = u64::try_from(selection.start)
        .map_err(|_| "strand start does not fit the wire contract".to_string())?;
    let response_end_byte = u64::try_from(selection.end)
        .map_err(|_| "strand end does not fit the wire contract".to_string())?;
    let content_sha256 = canonical_semantic_strand_content_sha256(&content);
    let embedding_sha256 = canonical_semantic_strand_embedding_sha256(
        &projection_48d,
        Some(&companion_projection_12d),
    );
    Ok(SemanticStrandV1 {
        schema: SEMANTIC_STRAND_SCHEMA_V1.to_string(),
        strand_id: format!(
            "{}-strand-{}-{}",
            short_hash(&attestation.response_sha256),
            index.saturating_add(1),
            short_hash(&content_sha256)
        ),
        owner_being: "astrid".to_string(),
        source_attestation_id: attestation.attestation_id.clone(),
        source_attestation_sha256: attestation_sha256.to_string(),
        response_start_byte,
        response_end_byte,
        label: selection.label,
        content,
        content_sha256,
        embedding_sha256,
        projection_48d,
        companion_projection_12d: Some(companion_projection_12d),
        provenance: SemanticStrandProvenanceV1::ExactUtf8ResponseInterval,
        deployment_identity: attestation.model_deployment_identity.clone(),
        captured_at_unix_ms: attestation.captured_at_unix_ms,
    })
}

fn validate_selection_count(selections: &[ResolvedSelectionV1]) -> Result<(), String> {
    if !(OWNER_INQUIRY_MIN_STRANDS_V1..=OWNER_INQUIRY_MAX_STRANDS_V1).contains(&selections.len()) {
        return Err(clarification_message());
    }
    let mut intervals = HashSet::new();
    if !selections
        .iter()
        .all(|selection| intervals.insert((selection.start, selection.end)))
    {
        return Err("duplicate strand intervals are not allowed".to_string());
    }
    Ok(())
}

fn quoted_contents(payload: &str) -> Result<Vec<String>, String> {
    let mut values = Vec::new();
    let mut cursor = 0;
    while cursor < payload.len() {
        let rest = &payload[cursor..];
        let Some(relative_open) = rest.find(['"', '“']) else {
            break;
        };
        let open = cursor.saturating_add(relative_open);
        let open_char = payload[open..]
            .chars()
            .next()
            .ok_or_else(clarification_message)?;
        let close_char = if open_char == '“' { '”' } else { '"' };
        let content_start = open.saturating_add(open_char.len_utf8());
        let remaining = &payload[content_start..];
        let close_relative = remaining
            .find(close_char)
            .ok_or_else(clarification_message)?;
        let close = content_start.saturating_add(close_relative);
        let value = payload[content_start..close].to_string();
        if value.trim().is_empty() {
            return Err(clarification_message());
        }
        values.push(value);
        cursor = close.saturating_add(close_char.len_utf8());
    }
    Ok(values)
}

fn find_unique_exact_span(haystack: &str, needle: &str) -> Option<(usize, usize)> {
    if needle.is_empty() {
        return None;
    }
    let matches = haystack.match_indices(needle).collect::<Vec<_>>();
    if matches.len() != 1 {
        return None;
    }
    let start = matches[0].0;
    Some((start, start.checked_add(needle.len())?))
}

fn clarification_message() -> String {
    "Clarification needed: name 2–8 distinct strands once each in quotes, or use JSON with exact UTF-8 start_byte/end_byte intervals. No strand was guessed, merged, ranked, or queued.".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn natural_start_requires_unique_exact_quoted_strands() {
        let response = "I can feel \"soft persistence\" beside \"bright shear\".\nNEXT: INQUIRY_START Compare \"soft persistence\" with \"bright shear\"";
        let parsed = parse_start_recipe(
            "Compare \"soft persistence\" with \"bright shear\"",
            response,
            "intent-1",
        )
        .expect("unambiguous comparison");
        assert_eq!(parsed.selections.len(), 2);
        assert_eq!(
            &response[parsed.selections[0].start..parsed.selections[0].end],
            "soft persistence"
        );
    }

    #[test]
    fn repeated_natural_strand_requires_clarification() {
        let response =
            "\"same\" occurs here.\nNEXT: INQUIRY_START Compare \"same\" with \"different\"";
        let error = parse_start_recipe("Compare \"same\" with \"different\"", response, "intent-1")
            .expect_err("same occurs twice in the exact response");
        assert!(error.contains("Clarification needed"));
    }

    #[test]
    fn json_text_strands_bind_expression_before_next_declaration() {
        let response = concat!(
            "I can hold alpha texture beside beta texture.\n",
            "NEXT: INQUIRY_START {\"question\":\"compare\",\"strands\":[",
            "{\"label\":\"alpha\",\"text\":\"alpha texture\"},",
            "{\"label\":\"beta\",\"text\":\"beta texture\"}]}"
        );
        let payload = concat!(
            "{\"question\":\"compare\",\"strands\":[",
            "{\"label\":\"alpha\",\"text\":\"alpha texture\"},",
            "{\"label\":\"beta\",\"text\":\"beta texture\"}]}"
        );
        let parsed = parse_start_recipe(payload, response, "intent-json-text").unwrap();
        assert_eq!(parsed.selections.len(), 2);
        assert_eq!(
            response
                .get(parsed.selections[0].start..parsed.selections[0].end)
                .unwrap(),
            "alpha texture"
        );
        assert!(
            parsed
                .selections
                .iter()
                .all(|selection| selection.end < response.rfind("NEXT:").unwrap())
        );
    }

    #[test]
    fn strand_companion_is_the_derived_glimpse_not_an_independent_delta_field() {
        use astrid_minime_protocol::{
            SEMANTIC_STRAND_BASE_DIMENSIONS_V1, SEMANTIC_STRAND_COMPANION_DIMENSIONS_V1,
        };

        let response = "I can hold \"soft persistence\" beside \"bright shear\".\nNEXT: LISTEN";
        let (start, end) =
            find_unique_exact_span(response, "soft persistence").expect("one exact strand span");
        let attestation = BeingUtteranceAttestationV1 {
            schema: BEING_UTTERANCE_ATTESTATION_SCHEMA_V1.to_string(),
            attestation_id: "astrid-attestation-test-1".to_string(),
            being: "astrid".to_string(),
            exchange_id: "exchange-1".to_string(),
            response_sha256: format!("{:x}", Sha256::digest(response.as_bytes())),
            response_len_bytes: u64::try_from(response.len()).expect("response length fits u64"),
            model: "test-model".to_string(),
            provider: "spectral-bridge".to_string(),
            model_deployment_identity: "deploy-test".to_string(),
            captured_at_unix_ms: 1_700_000_000_000,
            attestor_process_identity: "spectral-bridge:pid:test".to_string(),
            attestor_deployment_identity: "deploy-test".to_string(),
            attestor_public_key_hex: "aa".repeat(32),
            signature_hex: "bb".repeat(64),
        };
        let strand = build_strand(
            0,
            ResolvedSelectionV1 {
                label: "soft".to_string(),
                start,
                end,
            },
            response,
            &attestation,
            &"c".repeat(64),
        )
        .expect("exact strand interval builds a well-formed semantic strand");

        assert_eq!(
            strand.projection_48d.len(),
            SEMANTIC_STRAND_BASE_DIMENSIONS_V1
        );
        let derived = crate::codec::GlimpseCodec::derive_12d(&strand.projection_48d)
            .expect("48D projection reduces to the 12D companion");
        assert_eq!(
            strand.companion_projection_12d.as_deref(),
            Some(derived.as_slice()),
            "the companion is derived from this strand's own 48D projection, not stored beside it"
        );
        assert!(strand.is_well_formed());

        // The wire contract binds both vectors into one embedding digest but never
        // re-derives the companion, so a finite unrelated 12D vector still passes
        // protocol validation. The 48D -> 12D relation is a producer-side contract
        // held here in `build_strand`, not a checked delta on the wire.
        let mut substituted = strand.clone();
        let unrelated = vec![0.125_f32; SEMANTIC_STRAND_COMPANION_DIMENSIONS_V1];
        assert_ne!(unrelated.as_slice(), derived.as_slice());
        substituted.embedding_sha256 = canonical_semantic_strand_embedding_sha256(
            &substituted.projection_48d,
            Some(&unrelated),
        );
        substituted.companion_projection_12d = Some(unrelated);
        assert!(substituted.is_well_formed());
    }
}
