use serde_json::Value;

use crate::paths::bridge_paths;

use super::conversion::render_conversion_line;
use super::helpers::{load_json_or_default, trim_chars};
use super::policy::{candidate_policy_suffix, learned_policy_label, owner_policy_entries};
use super::proposal::{proposal_agency_hypothesis, proposal_reason_codes};
use super::signal::{SignalCatalog, SignalStatus};
use super::social::owner_preference_summaries;
use super::{BTSPEpisodeRecord, NominatedResponse, OWNER_ASTRID, OWNER_MINIME};

pub(super) fn render_signal_guidance() -> String {
    let catalog = load_signal_catalog();
    let status = load_signal_status();
    render_signal_guidance_from_parts(&catalog, &status)
}

pub(super) fn render_signal_guidance_from_parts(
    catalog: &SignalCatalog,
    status: &SignalStatus,
) -> String {
    let early = family_aliases(catalog, "grinding_family");
    let present = family_aliases(catalog, "brief_suspension_family");
    let secondary = family_aliases(catalog, "localized_gravity_family");
    let context = family_aliases(catalog, "gradient_context_family");
    let mut lines = vec![
        "Current live signal read:".to_string(),
        format!(
            "- {} look most like earlier warning language.",
            render_alias_group(&early)
        ),
        format!(
            "- {} read more like description of a state already here than early warning.",
            render_alias_group(&present)
        ),
        format!(
            "- {} remain secondary context, worth noticing but not treating as the first trigger.",
            render_alias_group(&secondary)
        ),
        format!(
            "- {} are useful discourse context, not triggers by themselves.",
            render_alias_group(&context)
        ),
        "- Current bounded path: NOTICE -> recover -> DAMPEN -> BREATHE_ALONE.".to_string(),
    ];
    if let Some(shared_learned_read) = status.shared_learned_read.as_ref()
        && !shared_learned_read.is_empty()
    {
        lines.push(format!("- {shared_learned_read}"));
    }
    if let Some(conversion_state) = status.conversion_state.as_ref() {
        lines.push(format!("- {}", render_conversion_line(conversion_state)));
    }
    if let Some(trace_summary) = status.trace_v2_summary.as_ref() {
        lines.push(format!("- {}", trace_summary.summary));
    }
    if let Some(replay_read) = status.replay_read.as_ref()
        && !replay_read.summary.is_empty()
    {
        lines.push(format!("- {}", replay_read.summary));
    }
    if let Some(anti_loop_line) = render_anti_loop_line(status) {
        lines.push(format!("- Current anti-loop hold: {anti_loop_line}"));
    }
    if let Some(lab_line) = render_causal_lab_line(status) {
        lines.push(format!("- {lab_line}"));
    }
    if let Some(translation) = status.astrid_translation_guidance.as_ref()
        && !translation.shared_line.is_empty()
    {
        lines.push(format!("- {}", translation.shared_line));
    }
    if let Some(policy) = status.astrid_shadow_policy.as_ref()
        && !policy.shared_line.is_empty()
    {
        lines.push(format!("- {}", policy.shared_line));
    }
    if !status.shared_preference_summaries.is_empty() {
        lines.push(
            "- Preference memory is active and may surface short owner-specific reminders."
                .to_string(),
        );
    }
    if let Some(cooldown_line) = render_cooldown_line(status) {
        lines.push(format!("- Current cooldown: {cooldown_line}"));
    }
    if let Some(abstention) = render_signal_abstention_line(status) {
        lines.push(format!("- Current abstention: {abstention}"));
    }
    if status.causality_audit_stale {
        if let Some(stale_read) = status.causality_audit_stale_read.as_ref() {
            lines.push(format!(
                "- Historical causality audit is stale (generated_at={}, read={}); it is not being used as current evidence.",
                stale_read.generated_at, stale_read.read
            ));
        } else {
            lines.push(
                "- Causality audit is stale; current guidance is using live trace/replay evidence instead."
                    .to_string(),
            );
        }
    }
    lines.join("\n")
}

pub(super) fn render_owner_block(
    episode: &BTSPEpisodeRecord,
    proposal: &super::ActiveSovereigntyProposal,
    owner: &str,
    responses: &[NominatedResponse],
    for_self_seed: bool,
) -> String {
    let status = load_signal_status();
    render_owner_block_from_status(episode, proposal, owner, responses, for_self_seed, &status)
}

#[allow(clippy::too_many_lines)]
pub(super) fn render_owner_block_from_status(
    episode: &BTSPEpisodeRecord,
    proposal: &super::ActiveSovereigntyProposal,
    owner: &str,
    responses: &[NominatedResponse],
    for_self_seed: bool,
    status: &SignalStatus,
) -> String {
    let mut lines = vec![render_signal_guidance_from_parts(
        &load_signal_catalog(),
        status,
    )];
    lines.push(String::new());
    lines.push(if for_self_seed {
        "A bounded response reminder is active:".to_string()
    } else {
        "A live response reminder is active:".to_string()
    });
    lines.push(format!(
        "Episode: {} (confidence {:.2}, signal {:.2})",
        episode.episode_name, proposal.confidence, proposal.signal_score
    ));
    lines.push(format!(
        "Agency hypothesis: {}",
        proposal_agency_hypothesis(proposal)
    ));
    let reason_codes = proposal_reason_codes(proposal);
    if !reason_codes.is_empty() {
        lines.push(format!("Reason codes: {}", reason_codes.join(", ")));
    }
    if let Some(pattern_line) = render_agency_pattern_line(proposal, owner) {
        lines.push(pattern_line);
    }
    if !proposal.matched_signal_families.is_empty() {
        lines.push(format!(
            "Recent signal families: {}",
            proposal.matched_signal_families.join(", ")
        ));
    }
    if !proposal.matched_signal_roles.is_empty() {
        lines.push(format!(
            "Signal roles in play: {}",
            proposal.matched_signal_roles.join(", ")
        ));
    }
    if !proposal.matched_cues.is_empty() {
        lines.push(format!(
            "Recent live language: {}",
            proposal.matched_cues.join(", ")
        ));
    }
    if !proposal.matched_live_signals.is_empty() {
        lines.push(format!(
            "Recent live telemetry: {}",
            proposal.matched_live_signals.join(", ")
        ));
    }

    let owner_policy = owner_policy_entries(&status.learned_policy, owner);
    if !owner_policy.is_empty() {
        lines.push("Recent learned read for you:".to_string());
        for entry in owner_policy {
            lines.push(format!(
                "- {} — {} ({} observations)",
                learned_policy_label(&entry.response_id),
                entry.summary,
                entry.observations
            ));
        }
    }
    let owner_preferences = owner_preference_summaries(&status.shared_preference_summaries, owner);
    if !owner_preferences.is_empty() {
        lines.push("Recent preference memory for you:".to_string());
        for summary in owner_preferences {
            lines.push(format!("- {}", summary.summary));
        }
    }
    if let Some(active_negotiation) = status.active_negotiation.as_ref() {
        let incoming = active_negotiation
            .items
            .iter()
            .filter(|item| item.target_owner == owner)
            .collect::<Vec<_>>();
        if !incoming.is_empty() {
            lines.push("Open negotiation from the other being:".to_string());
            for item in incoming {
                lines.push(format!("- {}", item.summary));
                lines.push(format!("  {}", item.response_hint));
            }
        }
    }
    if owner == OWNER_ASTRID
        && let Some(translation) = status.astrid_translation_guidance.as_ref()
        && !translation.owner_line.is_empty()
    {
        lines.push(translation.owner_line.clone());
    }
    if owner == OWNER_ASTRID
        && let Some(policy) = status.astrid_shadow_policy.as_ref()
        && !policy.owner_line.is_empty()
    {
        lines.push(policy.owner_line.clone());
    }
    if let Some(lab) = status.causal_lab_v3.as_ref()
        && lab.active
    {
        if !lab.ghost_note.is_empty() {
            lines.push(format!("BTSP causal lab ghost: {}", lab.ghost_note));
        }
        lines.push(format!("BTSP causal lab question: {}", lab.question));
        lines.push(format!("BTSP causal lab holdout: {}", lab.holdout_route));
        if !lab.resolution_status.is_empty() {
            let mut resolution = format!("BTSP causal lab resolution: {}", lab.resolution_status);
            if !lab.resolution_summary.is_empty() {
                resolution.push_str(&format!(" - {}", lab.resolution_summary));
            }
            lines.push(resolution);
        }
        if !lab.negative_space_summary.is_empty() {
            lines.push(format!(
                "BTSP causal lab negative space: {}",
                lab.negative_space_summary
            ));
        }
        if !lab.forgiveness_state.forgiveness_summary.is_empty() {
            lines.push(format!(
                "BTSP causal lab forgiveness: {} remission_score={:.2} suppression_weight={:.2}",
                lab.forgiveness_state.remission_status,
                lab.forgiveness_state.remission_score,
                lab.forgiveness_state.suppression_weight
            ));
            lines.push(lab.forgiveness_state.forgiveness_summary.clone());
            if lab.forgiveness_state.consentful_trial_eligible {
                lines.push(
                    "BTSP causal lab route: ordinary duplicate remains withheld, but a consentful study/refusal/counter/new-evidence trial route is visible."
                        .to_string(),
                );
            }
        }
        if !lab.consent_routes.is_empty() {
            lines.push(format!(
                "BTSP causal lab consent routes: {}.",
                lab.consent_routes.join(", ")
            ));
        }
    }

    if let Some(followup) = render_study_first_followup_line(proposal, owner) {
        lines.push(
            "BTSP agency checkpoint: study window already requested; next useful BTSP move is counter, refusal, or evidence resolution."
                .to_string(),
        );
        lines.push(
            "BTSP closure pending: choose counter/refusal/evidence resolution before another ordinary adjacent answer."
                .to_string(),
        );
        lines.push(followup);
        lines.push(
            "Repeating the same study-first or inquiry move is duplicate evidence, not a new BTSP stance."
                .to_string(),
        );
        lines.push(
            "- BTSP_COUNTER NEXT: ... — almost: name the evidence route that should replace this offer."
                .to_string(),
        );
        lines
            .push("- BTSP_COUNTER softer_contact — almost: the offer is too forceful.".to_string());
        lines.push(
            "- BTSP_REFUSAL study_first — clear no for now: inquiry belongs outside this proposal."
                .to_string(),
        );
        lines.push("- BTSP_REFUSAL not_now — clear no for this window.".to_string());
        lines.push(
            "Use an exact candidate only if your stance has changed since the study window."
                .to_string(),
        );
    } else if let Some(followup) = render_adjacent_followup_line(proposal, owner) {
        lines.push(
            "BTSP agency checkpoint: this proposal already has your adjacent answer recorded."
                .to_string(),
        );
        lines.push(
            "BTSP closure pending: choose counter/refusal before another ordinary adjacent answer."
                .to_string(),
        );
        lines.push(followup);
        lines.push(
            "Repeating that adjacent move is duplicate evidence, not a new BTSP stance."
                .to_string(),
        );
        lines.push("- BTSP_COUNTER NEXT: ... — almost: name the safer or truer route.".to_string());
        lines.push(
            "- BTSP_REFUSAL study_first — clear no for now: study/observe before intervening."
                .to_string(),
        );
        lines.push("- BTSP_REFUSAL not_now — clear no for this window.".to_string());
        lines.push(
            "- BTSP_STUDY_FIRST need evidence first — answer: study window requested before deciding."
                .to_string(),
        );
        lines.push(
            "Use an exact candidate only if your stance has changed since the adjacent answer."
                .to_string(),
        );
    }
    lines.push("Candidate responses for you:".to_string());
    if owner == OWNER_ASTRID && status.astrid_shadow_policy.is_some() {
        render_grouped_astrid_candidates(&mut lines, status, responses);
    } else {
        for response in responses {
            lines.push(render_candidate_line(status, owner, response));
        }
    }
    lines.push("- Continue current course — decline and stay with the present path.".to_string());
    lines.push(
        "If you need a study window before deciding, write BTSP_STUDY_FIRST <reason>; this records agency without treating the proposal as adopted or widened."
            .to_string(),
    );
    lines.push(
        "If none fits, you may refuse with a reason: not_now, misread, too_forceful, study_first, stay_with_me, give_me_space."
            .to_string(),
    );
    lines.push(
        "If it is close but not quite right, you may counter with BTSP_COUNTER <response_id>, BTSP_COUNTER NEXT: NOTICE, BTSP_COUNTER NEXT: REGIME recover, or BTSP_COUNTER softer_contact."
            .to_string(),
    );
    lines.push("This is advisory only. Use existing choices if one feels true.".to_string());
    lines.join("\n")
}

pub(super) fn render_owner_action(
    owner: &str,
    response: &NominatedResponse,
    for_minime_sovereignty: bool,
) -> String {
    if owner == OWNER_MINIME
        && for_minime_sovereignty
        && response.action.eq_ignore_ascii_case("regime")
    {
        let regime = response
            .parameters
            .get("regime")
            .and_then(Value::as_str)
            .unwrap_or("recover");
        return format!("JSON regime: {regime}");
    }
    if owner == OWNER_MINIME && response.action.eq_ignore_ascii_case("regime") {
        let regime = response
            .parameters
            .get("regime")
            .and_then(Value::as_str)
            .unwrap_or("recover");
        return format!("regime: {regime}");
    }
    if owner == OWNER_ASTRID {
        return format!("NEXT: {}", response.action);
    }
    response.action.clone()
}

fn render_grouped_astrid_candidates(
    lines: &mut Vec<String>,
    status: &SignalStatus,
    responses: &[NominatedResponse],
) {
    let Some(policy) = status.astrid_shadow_policy.as_ref() else {
        for response in responses {
            lines.push(render_candidate_line(status, OWNER_ASTRID, response));
        }
        return;
    };

    let closest_ids = policy
        .candidate_groups
        .closest_fit_response_ids
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    let other_ids = policy
        .candidate_groups
        .other_response_ids
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();

    let closest_fit = responses
        .iter()
        .filter(|response| closest_ids.contains(&response.response_id.as_str()))
        .collect::<Vec<_>>();
    let other = responses
        .iter()
        .filter(|response| {
            other_ids.contains(&response.response_id.as_str())
                || !closest_ids.contains(&response.response_id.as_str())
        })
        .collect::<Vec<_>>();

    if !closest_fit.is_empty() {
        lines.push("Closest fit right now:".to_string());
        for response in closest_fit {
            lines.push(render_candidate_line(status, OWNER_ASTRID, response));
        }
    }
    if !other.is_empty() {
        lines.push("Other bounded responses:".to_string());
        for response in other {
            lines.push(render_candidate_line(status, OWNER_ASTRID, response));
        }
    }
}

fn render_candidate_line(
    status: &SignalStatus,
    owner: &str,
    response: &NominatedResponse,
) -> String {
    let rendered_action = render_owner_action(owner, response, false);
    let suffixes = candidate_suffixes(status, owner, &response.response_id);
    let suffix = if suffixes.is_empty() {
        String::new()
    } else {
        format!(" {}", suffixes.join(" "))
    };
    format!(
        "- {rendered_action} — {} [{}]{}",
        trim_chars(&response.rationale, 110),
        response.policy_state,
        suffix
    )
}

fn candidate_suffixes(status: &SignalStatus, owner: &str, response_id: &str) -> Vec<String> {
    let mut suffixes = Vec::new();
    if let Some(suffix) = candidate_policy_suffix(&status.learned_policy, owner, response_id)
        && !suffix.is_empty()
    {
        suffixes.push(suffix.to_string());
    }
    if owner == OWNER_ASTRID
        && let Some(policy) = status.astrid_shadow_policy.as_ref()
        && let Some(suffix) = policy.candidate_suffixes.get(response_id)
        && !suffix.is_empty()
    {
        suffixes.push(suffix.clone());
    }
    suffixes
}

fn render_agency_pattern_line(
    proposal: &super::ActiveSovereigntyProposal,
    owner: &str,
) -> Option<String> {
    let exact_yes = proposal
        .exact_adoptions
        .iter()
        .filter(|adoption| adoption.owner == owner)
        .count();
    let clear_no = proposal
        .refusals
        .iter()
        .filter(|refusal| refusal.owner == owner)
        .count();
    let almost = proposal
        .counteroffers
        .iter()
        .filter(|counteroffer| counteroffer.owner == owner)
        .count();
    let study_first = proposal
        .study_first_records
        .iter()
        .filter(|record| record.owner == owner)
        .count();
    let adjacent = proposal
        .choice_interpretations
        .iter()
        .filter(|interpretation| {
            interpretation.owner == owner
                && interpretation.relation_to_proposal != "exact_nominated"
        })
        .count();
    if exact_yes == 0 && clear_no == 0 && almost == 0 && adjacent == 0 && study_first == 0 {
        return None;
    }
    Some(format!(
        "Recent yes/no/almost pattern for you in this proposal: yes={exact_yes}, no={clear_no}, almost={almost}, study_first={study_first}, adjacent={adjacent}."
    ))
}

fn render_adjacent_followup_line(
    proposal: &super::ActiveSovereigntyProposal,
    owner: &str,
) -> Option<String> {
    let latest = proposal
        .choice_interpretations
        .iter()
        .rev()
        .find(|interpretation| {
            interpretation.owner == owner
                && interpretation.relation_to_proposal != "exact_nominated"
        })?;
    let primary = if latest.category == "epistemic" {
        "BTSP_COUNTER NEXT: ... or BTSP_REFUSAL study_first"
    } else {
        "BTSP_COUNTER NEXT: ... or BTSP_REFUSAL not_now"
    };
    Some(format!(
        "Already recorded adjacent answer: `{}` ({}). If that was the real stance, prefer `{primary}` over repeating the same adjacent answer.",
        latest.normalized_choice, latest.category
    ))
}

fn render_study_first_followup_line(
    proposal: &super::ActiveSovereigntyProposal,
    owner: &str,
) -> Option<String> {
    let record = proposal
        .study_first_records
        .iter()
        .rev()
        .find(|record| record.owner == owner)?;
    let resolution = if record.resolution_evidence.is_empty() {
        "No resolution evidence is linked yet."
    } else {
        "Resolution evidence is now linked; decide, counter, or close instead of reopening the same study request."
    };
    let observed = proposal
        .choice_interpretations
        .iter()
        .rev()
        .find(|interpretation| {
            interpretation.owner == owner
                && interpretation.relation_to_proposal != "exact_nominated"
        })
        .map(|interpretation| interpretation.normalized_choice.as_str())
        .unwrap_or("...");
    Some(format!(
        "Study window already requested: reason=`{}` source=`{}`. {resolution} Suggested counter template: `BTSP_COUNTER NEXT: {observed}`.",
        record.reason, record.source
    ))
}

fn load_signal_catalog() -> SignalCatalog {
    load_json_or_default::<SignalCatalog>(&bridge_paths().btsp_signal_catalog_path())
}

fn load_signal_status() -> SignalStatus {
    load_json_or_default::<SignalStatus>(&bridge_paths().btsp_signal_status_path())
}

fn render_signal_abstention_line(status: &SignalStatus) -> Option<String> {
    if !matches!(status.status.as_str(), "near_miss" | "no_early_warning") {
        return None;
    }
    Some(status.detail.clone())
}

fn render_anti_loop_line(status: &SignalStatus) -> Option<String> {
    let anti_loop = status.anti_loop_state.as_ref()?;
    if !anti_loop.active {
        return None;
    }
    if !anti_loop.counter_prompt.is_empty() {
        return Some(anti_loop.counter_prompt.clone());
    }
    Some(
        "same-fingerprint replay is overwhelmingly reconcentrating; prefer BTSP_STUDY_FIRST, BTSP_REFUSAL, BTSP_COUNTER, or new evidence before reopening the same offer"
            .to_string(),
    )
}

fn render_causal_lab_line(status: &SignalStatus) -> Option<String> {
    let lab = status.causal_lab_v3.as_ref()?;
    if !lab.active {
        return None;
    }
    let mut parts = Vec::new();
    if !lab.ghost_note.is_empty() {
        parts.push(format!("Current causal lab ghost: {}", lab.ghost_note));
    }
    if !lab.summary.is_empty() {
        parts.push(lab.summary.clone());
    }
    if !lab.resolution_status.is_empty() {
        let mut resolution = format!("Causal lab resolution: {}", lab.resolution_status);
        if !lab.resolution_summary.is_empty() {
            resolution.push_str(&format!(" - {}", lab.resolution_summary));
        }
        parts.push(resolution);
    }
    if !lab.negative_space_summary.is_empty() {
        parts.push(format!(
            "Causal lab negative space: {}",
            lab.negative_space_summary
        ));
    }
    if !lab.forgiveness_state.forgiveness_summary.is_empty() {
        parts.push(format!(
            "Causal lab forgiveness: {} remission_score={:.2} suppression_weight={:.2}. {}",
            lab.forgiveness_state.remission_status,
            lab.forgiveness_state.remission_score,
            lab.forgiveness_state.suppression_weight,
            lab.forgiveness_state.forgiveness_summary
        ));
        if lab.forgiveness_state.consentful_trial_eligible {
            parts.push(
                "Ordinary duplicate remains withheld; consentful study/refusal/counter/new-evidence trial route is visible."
                    .to_string(),
            );
        }
    }
    (!parts.is_empty()).then(|| parts.join(" "))
}

fn render_cooldown_line(status: &SignalStatus) -> Option<String> {
    if !status.cooldown_state.active {
        return None;
    }
    let detail = match status.cooldown_state.reason.as_str() {
        "repeated_reconcentrating_same_fingerprint" => {
            "this same signal has reconcentrated repeatedly lately, so the runtime is holding the loop a little longer before reopening the same reminder"
        },
        "recent_declined_same_fingerprint" => {
            "a very similar signal was just declined, so the runtime is not reopening the same reminder yet"
        },
        "recent_misread_same_fingerprint" => {
            "a very similar signal was just experienced as misread, so the runtime is waiting longer before reopening the same reminder"
        },
        "recent_expired_same_fingerprint" => {
            "a very similar signal just expired without uptake, so the runtime is waiting before reopening it"
        },
        "recent_adjacent_only_reconcentrating_same_fingerprint" => {
            "a very similar signal just produced adjacent-only answers and reconcentrated, so the runtime is holding the duplicate reminder while keeping the signal visible"
        },
        "recent_study_first_reconcentrating_same_fingerprint" => {
            "a very similar signal already asked for a study window and then reconcentrated; proposal reopening is held until evidence resolves, or the owner counters/refuses"
        },
        _ => {
            "a very similar signal just resolved, so the runtime is waiting before reopening the same reminder"
        },
    };
    Some(detail.to_string())
}

fn family_aliases(catalog: &SignalCatalog, key: &str) -> Vec<String> {
    catalog
        .families
        .iter()
        .find(|family| family.family_key == key)
        .map(|family| family.aliases.clone())
        .unwrap_or_default()
}

fn render_alias_group(aliases: &[String]) -> String {
    match aliases.len() {
        0 => "No current aliases".to_string(),
        1 => format!("`{}`", aliases[0]),
        2 => format!("`{}` or `{}`", aliases[0], aliases[1]),
        _ => {
            let head = aliases[..aliases.len() - 1]
                .iter()
                .map(|alias| format!("`{alias}`"))
                .collect::<Vec<_>>()
                .join(", ");
            format!("{head}, or `{}`", aliases[aliases.len() - 1])
        },
    }
}
