use tracing::{info, warn};

use super::{
    ConversationState, NextActionContext, extract_search_topic, list_directory, strip_action,
};
use crate::memory;
use crate::paths::bridge_paths;

pub(super) fn handle_action(
    conv: &mut ConversationState,
    base_action: &str,
    original: &str,
    next_action: &str,
    ctx: &mut NextActionContext<'_>,
) -> bool {
    match base_action {
        "REST" | "LISTEN" => {
            *ctx.burst_count = conv.burst_target.saturating_add(2);
            true
        },
        "LOOK" => {
            conv.wants_look = true;
            true
        },
        "CLOSE_EYES" | "QUIET" => {
            conv.senses_snoozed = true;
            let flag = bridge_paths().perception_paused_flag();
            let _ = std::fs::write(&flag, "paused by CLOSE_EYES");
            conv.push_receipt("CLOSE_EYES", vec!["all perception paused".into()]);
            info!("Astrid snoozed her senses (perception.py paused)");
            true
        },
        "OPEN_EYES" | "WAKE" => {
            conv.senses_snoozed = false;
            let flag = bridge_paths().perception_paused_flag();
            let _ = std::fs::remove_file(&flag);
            conv.push_receipt("OPEN_EYES", vec!["perception resumed".into()]);
            info!("Astrid reopened her senses (perception.py resumed)");
            true
        },
        "SEARCH" | "RESEARCH" => {
            conv.wants_search = true;
            // RESEARCH maps to SEARCH — the being invented this alias naturally.
            let topic_text = if base_action == "RESEARCH" {
                // Strip RESEARCH prefix and try to extract topic
                let rest = strip_action(original, "RESEARCH");
                if !rest.is_empty() {
                    Some(rest)
                } else {
                    extract_search_topic(next_action)
                }
            } else {
                extract_search_topic(next_action)
            };
            if let Some(topic) = topic_text {
                info!("Astrid requested web search ({}): {}", base_action, topic);
                conv.search_topic = Some(topic);
            } else {
                info!("Astrid requested web search ({})", base_action);
            }
            true
        },
        "BROWSE" => {
            let raw_s = strip_action(original, "BROWSE");
            let raw_owned = if raw_s.is_empty() {
                next_action.trim().to_string()
            } else {
                raw_s
            };
            let raw = raw_owned
                .trim()
                .trim_matches(|c: char| c == '"' || c == '\'' || c == '<' || c == '>');
            let url = raw
                .split(|c: char| c == '<' || c == '>' || c == ' ' || c == '\n')
                .next()
                .unwrap_or(raw)
                .trim_end_matches(|c: char| {
                    !c.is_alphanumeric()
                        && c != '/'
                        && c != '-'
                        && c != '_'
                        && c != '.'
                        && c != '~'
                        && c != '%'
                        && c != '?'
                        && c != '='
                        && c != '&'
                        && c != '#'
                });
            if url.starts_with("http") {
                let url_owned = url.to_string();
                // Count how many times this exact URL appears in recent buffer
                let visit_count = conv
                    .recent_browse_urls
                    .iter()
                    .filter(|u| *u == &url_owned)
                    .count();
                if visit_count >= 2 {
                    // URL fixation: visited 2+ times recently. Convert to SEARCH
                    // on the topic instead, breaking the attractor loop.
                    // Extract a search topic from the URL path segments.
                    let topic = url_owned
                        .split('/')
                        .last()
                        .unwrap_or("eigenvalue decomposition")
                        .replace('_', " ")
                        .replace('#', " ")
                        .split('?')
                        .next()
                        .unwrap_or("spectral analysis")
                        .to_string();
                    let search_topic = if topic.is_empty() {
                        "spectral dynamics research".to_string()
                    } else {
                        format!("{} new perspectives", topic)
                    };
                    info!(
                        "BROWSE fixation detected: {} visited {}x, redirecting to SEARCH '{}'",
                        url, visit_count, search_topic
                    );
                    conv.wants_search = true;
                    conv.search_topic = Some(search_topic);
                    // Don't add to browse buffer again
                } else {
                    if visit_count == 1 {
                        info!("Astrid re-browsing recently visited URL: {}", url);
                    } else {
                        info!("Astrid requested BROWSE: {}", url);
                    }
                    if conv.recent_browse_urls.len() >= 8 {
                        conv.recent_browse_urls.pop_front();
                    }
                    conv.recent_browse_urls.push_back(url_owned.clone());
                    conv.browse_url = Some(url_owned);
                }
            } else {
                warn!("BROWSE without valid URL: '{}'", next_action.trim());
            }
            true
        },
        "READ_MORE" => {
            if conv.last_read_path.is_some() {
                info!(
                    "Astrid requested READ_MORE (offset {})",
                    conv.last_read_offset
                );
            } else {
                warn!("READ_MORE but no file to continue from");
            }
            true
        },
        "LIST_FILES" | "LS" => {
            let dir_path = {
                let list_files = strip_action(original, "LIST_FILES");
                if list_files.is_empty() {
                    strip_action(original, "LS")
                } else {
                    list_files
                }
            };
            let dir = if dir_path.is_empty() {
                bridge_paths().bridge_root().display().to_string()
            } else {
                dir_path
            };
            match list_directory(&dir) {
                Some(listing) => {
                    conv.pending_file_listing = Some(listing);
                    info!("Astrid listed files in: {}", dir);
                },
                None => {
                    conv.pending_file_listing = Some(format!("[Could not list directory: {dir}]"));
                    warn!("LIST_FILES failed for: {}", dir);
                },
            }
            true
        },
        "PURSUE" => {
            let interest = strip_action(original, "PURSUE");
            if !interest.is_empty() {
                let prefix_len = interest.len().min(30);
                let interest_prefix = interest.to_lowercase();
                let dominated = conv
                    .interests
                    .iter()
                    .any(|i| i.to_lowercase().starts_with(&interest_prefix[..prefix_len]));
                if !dominated {
                    conv.interests.push(interest.clone());
                    while conv.interests.len() > 5 {
                        let dropped = conv.interests.remove(0);
                        info!("interest auto-dropped (oldest): {}", dropped);
                    }
                }
                info!("Astrid declared interest: {}", interest);
            }
            true
        },
        "DROP" => {
            let query = strip_action(original, "DROP").to_lowercase();
            if !query.is_empty() {
                let before = conv.interests.len();
                conv.interests
                    .retain(|i| !i.to_lowercase().contains(&query));
                let dropped = before - conv.interests.len();
                if dropped > 0 {
                    info!(
                        "Astrid dropped {} interest(s) matching '{}'",
                        dropped, query
                    );
                } else {
                    info!(
                        "Astrid tried to drop '{}' but no matching interest found",
                        query
                    );
                }
            }
            true
        },
        "INTERESTS" => {
            if conv.interests.is_empty() {
                conv.pending_file_listing = Some(
                    "[You have no declared interests yet. Use PURSUE <topic> to start one.]"
                        .to_string(),
                );
            } else {
                let listing = conv
                    .interests
                    .iter()
                    .enumerate()
                    .map(|(i, interest)| format!("  {}. {}", i + 1, interest))
                    .collect::<Vec<_>>()
                    .join("\n");
                conv.pending_file_listing = Some(format!(
                    "[Your ongoing interests:]\n{listing}\n\nUse DROP <keyword> to remove one, PURSUE <topic> to add."
                ));
            }
            info!(
                "Astrid requested interests listing ({} active)",
                conv.interests.len()
            );
            true
        },
        "MEMORIES" => {
            conv.pending_file_listing = Some(memory::format_memory_listing(
                &conv.remote_memory_bank,
                conv.last_remote_memory_id.as_deref(),
                conv.last_remote_memory_role.as_deref(),
            ));
            info!(
                "Astrid requested memory-bank listing ({} entries)",
                conv.remote_memory_bank.len()
            );
            true
        },
        "RECALL" => {
            let target = strip_action(original, "RECALL");
            if target.is_empty() {
                conv.pending_file_listing = Some(
                    "[Use RECALL <role-or-id> to write a reviewable restart-memory request.]"
                        .to_string(),
                );
            } else {
                match memory::write_recall_request("astrid", &target) {
                    Ok(path) => {
                        conv.pending_file_listing = Some(format!(
                            "[Wrote restart-memory request for '{target}'.]\nArtifact: {}\nIt will be considered on Minime's next restart.",
                            path.display()
                        ));
                        info!("Astrid requested RECALL for {}", target);
                    },
                    Err(error) => {
                        conv.pending_file_listing = Some(format!(
                            "[Could not write RECALL request for '{target}': {error}]"
                        ));
                        warn!("RECALL request failed for {}: {}", target, error);
                    },
                }
            }
            true
        },
        _ => false,
    }
}
