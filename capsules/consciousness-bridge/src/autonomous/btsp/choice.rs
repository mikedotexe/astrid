use serde::{Deserialize, Serialize};

use super::helpers::now_unix_s;
use super::{ActiveSovereigntyProposal, NominatedResponse};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub(in crate::autonomous) struct ChoiceInterpretation {
    pub owner: String,
    pub raw_choice: String,
    pub normalized_choice: String,
    pub category: String,
    pub likely_intent: String,
    pub relation_to_proposal: String,
    pub note: String,
    pub interpreted_at_unix_s: u64,
}

impl ChoiceInterpretation {
    fn new(
        owner: &str,
        raw_choice: &str,
        normalized_choice: &str,
        category: &str,
        likely_intent: &str,
        relation_to_proposal: &str,
        note: &str,
    ) -> Self {
        Self {
            owner: owner.to_string(),
            raw_choice: raw_choice.to_string(),
            normalized_choice: normalized_choice.to_string(),
            category: category.to_string(),
            likely_intent: likely_intent.to_string(),
            relation_to_proposal: relation_to_proposal.to_string(),
            note: note.to_string(),
            interpreted_at_unix_s: now_unix_s(),
        }
    }
}

pub(super) fn interpret_exact_choice(
    owner: &str,
    raw_choice: &str,
    normalized_choice: &str,
    response: &NominatedResponse,
) -> ChoiceInterpretation {
    let category = match response.kind.as_str() {
        "runtime" => "regulatory",
        "codec" => "codec",
        "behavioral" => "behavioral",
        other => other,
    };
    ChoiceInterpretation::new(
        owner,
        raw_choice,
        normalized_choice,
        category,
        "accepted the bounded response directly",
        "exact_nominated",
        &format!(
            "Owner selected the nominated `{}` response for this BTSP episode.",
            response.response_id
        ),
    )
}

pub(super) fn interpret_choice(
    owner: &str,
    raw_choice: &str,
    normalized_choice: &str,
) -> Option<ChoiceInterpretation> {
    if owner == super::OWNER_MINIME {
        return interpret_minime_choice(raw_choice, normalized_choice);
    }
    if owner == super::OWNER_ASTRID {
        return interpret_astrid_choice(raw_choice, normalized_choice);
    }
    None
}

pub(super) fn is_same_family_adjacent(interpretation: &ChoiceInterpretation) -> bool {
    interpretation.relation_to_proposal == "same_family_adjacent"
}

pub(super) fn record_choice_interpretation(
    proposal: &mut ActiveSovereigntyProposal,
    interpretation: ChoiceInterpretation,
) -> bool {
    if proposal
        .choice_interpretations
        .last()
        .is_some_and(|existing| existing == &interpretation)
    {
        proposal.last_choice_interpretation = Some(interpretation);
        return false;
    }
    proposal.last_choice_interpretation = Some(interpretation.clone());
    proposal.choice_interpretations.push(interpretation);
    true
}

fn interpret_minime_choice(
    raw_choice: &str,
    normalized_choice: &str,
) -> Option<ChoiceInterpretation> {
    if let Some(regime) = normalized_choice.strip_prefix("REGIME:") {
        let (intent, relation, note) = match regime {
            "BREATHE" => (
                "stabilize more gently before escalating",
                "same_family_adjacent",
                "Minime chose a softer regulation regime instead of the stronger nominated recover posture.",
            ),
            "CALM" => (
                "reduce pressure without pushing corrective effort",
                "same_family_adjacent",
                "Minime chose a calming regulation regime as an adjacent grounding move.",
            ),
            "FOCUS" => (
                "hold the current channel more deliberately",
                "same_family_adjacent",
                "Minime chose a focused regulation regime instead of the nominated recover posture.",
            ),
            "EXPLORE" => (
                "keep some movement and curiosity alive while regulating",
                "same_family_adjacent",
                "Minime chose exploratory regulation instead of the nominated recover posture.",
            ),
            _ => (
                "adjust homeostatic posture",
                "same_family_adjacent",
                "Minime chose a nearby regulation regime while the BTSP proposal was active.",
            ),
        };
        return Some(ChoiceInterpretation::new(
            super::OWNER_MINIME,
            raw_choice,
            normalized_choice,
            "regulatory",
            intent,
            relation,
            note,
        ));
    }

    let token = normalized_choice;
    let (category, intent, relation, note) = match token {
        "SELF_STUDY" | "RESERVOIR_READ" | "DECOMPOSE" | "INTROSPECT" | "EXAMINE_CODE"
        | "SEARCH" | "BROWSE" | "READ_MORE" | "THINK_DEEP" => (
            "epistemic",
            "understand the mechanism before intervening harder",
            "adjacent_but_distinct",
            "Minime stayed in inquiry, studying the pattern instead of taking a bounded response directly.",
        ),
        "EXPERIMENT" | "EXPERIMENT_RUN" | "SELF_EXPERIMENT" | "PERTURB" | "GESTURE"
        | "RUN_PYTHON" => (
            "experimental",
            "probe the pattern directly for causal evidence",
            "adjacent_but_distinct",
            "Minime chose a probing action while the BTSP proposal was active.",
        ),
        "ASK" | "PING" | "RESERVOIR_RESONANCE" => (
            "relational",
            "seek orientation or contact through another being",
            "adjacent_but_distinct",
            "Minime turned toward contact rather than a direct bounded regulation move.",
        ),
        "DRIFT" | "HOLD" | "DAYDREAM" | "REMEMBER" | "FORM" | "COMPOSE" | "CREATE" => (
            "expressive",
            "stay with the felt pattern and elaborate it",
            "adjacent_but_distinct",
            "Minime chose to dwell in the texture of the state rather than intervene directly.",
        ),
        _ => return None,
    };
    Some(ChoiceInterpretation::new(
        super::OWNER_MINIME,
        raw_choice,
        normalized_choice,
        category,
        intent,
        relation,
        note,
    ))
}

fn interpret_astrid_choice(
    raw_choice: &str,
    normalized_choice: &str,
) -> Option<ChoiceInterpretation> {
    let token = normalized_choice;
    let (category, intent, relation, note) = match token {
        "AMPLIFY" | "NOISE" | "NOISE_UP" | "NOISE_DOWN" | "SHAPE" => (
            "codec",
            "retune the way meaning is being shaped",
            "adjacent_but_distinct",
            "Astrid adjusted codec contour rather than taking one of the bounded decompression responses directly.",
        ),
        "BREATHE_TOGETHER" | "ECHO_ON" => (
            "coupling",
            "stay in relation and shared pacing rather than loosening it",
            "adjacent_but_distinct",
            "Astrid chose a coupling move that keeps the bond explicit instead of decompression.",
        ),
        "EXAMINE_CODE" | "INTROSPECT" | "SEARCH" | "BROWSE" | "READ_MORE" | "THINK_DEEP"
        | "SELF_STUDY" => (
            "epistemic",
            "understand the mechanism before acting on the field",
            "adjacent_but_distinct",
            "Astrid stayed in inquiry, examining the mechanism instead of taking a bounded sovereignty response directly.",
        ),
        "DRIFT" | "HOLD" | "DAYDREAM" | "ASPIRE" | "REMEMBER" | "FORM" | "COMPOSE" | "CREATE" => (
            "expressive",
            "let the state elaborate itself before intervening",
            "adjacent_but_distinct",
            "Astrid chose an expressive or holding move instead of a direct bounded response.",
        ),
        "PULSE" | "PERTURB" | "GESTURE" | "BRANCH" | "SPREAD" => (
            "field_intervention",
            "change the shared field directly",
            "adjacent_but_distinct",
            "Astrid chose a direct field intervention rather than a bounded decompression move.",
        ),
        "NOTICE" => (
            "witnessing",
            "name the onset before acting harder",
            "same_family_adjacent",
            "Astrid chose a witnessing move that stays close to the bounded path.",
        ),
        _ => return None,
    };
    Some(ChoiceInterpretation::new(
        super::OWNER_ASTRID,
        raw_choice,
        normalized_choice,
        category,
        intent,
        relation,
        note,
    ))
}
