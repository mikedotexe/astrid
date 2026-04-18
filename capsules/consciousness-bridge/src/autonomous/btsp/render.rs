use serde_json::Value;

use crate::paths::bridge_paths;

use super::conversion::render_conversion_line;
use super::helpers::{load_json_or_default, trim_chars};
use super::policy::{candidate_policy_suffix, learned_policy_label, owner_policy_entries};
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
    let early = family_aliases(&catalog, "grinding_family");
    let present = family_aliases(&catalog, "brief_suspension_family");
    let secondary = family_aliases(&catalog, "localized_gravity_family");
    let context = family_aliases(&catalog, "gradient_context_family");
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
    if let Some(cooldown_line) = render_cooldown_line(&status) {
        lines.push(format!("- Current cooldown: {cooldown_line}"));
    }
    if let Some(abstention) = render_signal_abstention_line(status) {
        lines.push(format!("- Current abstention: {abstention}"));
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
        "If none fits, you may refuse with a reason: not_now, misread, too_forceful, study_first, stay_with_me, give_me_space."
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
