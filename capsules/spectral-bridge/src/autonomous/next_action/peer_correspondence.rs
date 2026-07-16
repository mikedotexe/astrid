use super::{ConversationState, NextActionContext, strip_action};
use crate::paths::bridge_paths;

use super::super::correspondence_v1;

pub(super) fn handle_action(
    conv: &mut ConversationState,
    base_action: &str,
    original: &str,
    ctx: &mut NextActionContext<'_>,
) -> bool {
    match base_action {
        "MESSAGE_MINIME" | "REPLY_MINIME" | "TRACE_MINIME" | "CORRESPONDENCE_TRACE" => {
            handle_send(conv, base_action, original, ctx);
            true
        },
        "CLAIM_MINIME_LEGACY" | "CORRESPONDENCE_CLAIM" => {
            handle_legacy_claim(conv, base_action, original);
            true
        },
        "CORRESPONDENCE_CLAIM_OUTCOME" => {
            handle_legacy_claim_outcome(conv, base_action, original);
            true
        },
        "CORRESPONDENCE_MICRODOSE_REQUEST" | "CORRESPONDENCE_WEIGHT_REQUEST" => {
            handle_weight_request(conv, base_action, original);
            true
        },
        "CORRESPONDENCE_ATTENTION_REQUEST" => {
            handle_attention_request(conv, base_action, original);
            true
        },
        "CORRESPONDENCE_ATTENTION_OUTCOME" => {
            handle_attention_outcome(conv, base_action, original);
            true
        },
        "ACK_MINIME" | "CORRESPONDENCE_ACK" => {
            handle_ack(conv, base_action, original);
            true
        },
        "I_RECEIVED_THIS" => {
            if should_route_received_this_to_phase(original, base_action) {
                false
            } else {
                handle_received_this(conv, base_action, original);
                true
            }
        },
        "CORRESPONDENCE_HEARTBEAT" | "SIGNAL_PERSISTENCE" => {
            handle_heartbeat(conv, base_action, original);
            true
        },
        "CORRESPONDENCE_STATUS" | "LEGACY_CORRESPONDENCE_STATUS" => {
            let report = correspondence_v1::status_report(8);
            conv.emphasis = Some(report);
            true
        },
        _ => false,
    }
}

fn handle_legacy_claim(conv: &mut ConversationState, base_action: &str, original: &str) {
    let raw = strip_action(original, base_action);
    let (selector, body) = parse_selector_body(&raw);
    conv.emphasis = Some(correspondence_v1::append_legacy_thread_claim_with_notice(
        selector,
        body,
        "astrid",
        "minime",
        Some(bridge_paths().minime_inbox_dir().as_path()),
    ));
}

fn handle_legacy_claim_outcome(conv: &mut ConversationState, base_action: &str, original: &str) {
    let raw = strip_action(original, base_action);
    let (selector, body) = parse_selector_body(&raw);
    conv.emphasis = Some(correspondence_v1::append_legacy_thread_claim_outcome(
        selector, body, "astrid", "minime",
    ));
}

fn handle_weight_request(conv: &mut ConversationState, base_action: &str, original: &str) {
    let raw = strip_action(original, base_action);
    let (selector, body) = raw
        .split_once("::")
        .map_or(("latest", raw.trim()), |(lhs, rhs)| {
            let selector = lhs.trim();
            (
                if selector.is_empty() {
                    "latest"
                } else {
                    selector
                },
                rhs.trim(),
            )
        });
    let report = correspondence_v1::draft_correspondence_microdose_request(selector, body);
    conv.emphasis = Some(report);
}

fn handle_attention_request(conv: &mut ConversationState, base_action: &str, original: &str) {
    let raw = strip_action(original, base_action);
    let (selector, body) = parse_selector_body(&raw);
    conv.emphasis = Some(correspondence_v1::activate_attention_canary(
        selector, body, "astrid", "minime",
    ));
}

fn handle_attention_outcome(conv: &mut ConversationState, base_action: &str, original: &str) {
    let raw = strip_action(original, base_action);
    let (selector, body) = parse_selector_body(&raw);
    conv.emphasis = Some(correspondence_v1::append_attention_outcome(
        selector, body, "astrid", "minime",
    ));
}

fn handle_ack(conv: &mut ConversationState, base_action: &str, original: &str) {
    let raw = strip_action(original, base_action);
    let (selector, body) = parse_selector_body(&raw);
    let ack = field_value(body, &["ack", "ack_kind"]).unwrap_or_else(|| {
        body.split([';', '\n', ' '])
            .find(|part| !part.trim().is_empty())
            .unwrap_or("seen")
            .trim()
            .to_string()
    });
    let note = field_value(body, &["note"]).unwrap_or_else(|| body.trim().to_string());
    conv.emphasis = Some(correspondence_v1::append_ack_receipt(
        selector, "astrid", "minime", &ack, &note,
    ));
}

fn should_route_received_this_to_phase(original: &str, base_action: &str) -> bool {
    let raw = strip_action(original, base_action);
    let (selector, body) = parse_selector_body(&raw);
    let received_as = field_value(body, &["received_as", "ack", "reply_state"])
        .unwrap_or_default()
        .to_ascii_lowercase()
        .replace(['-', ' '], "_");
    let felt_like = field_value(body, &["felt_like"])
        .unwrap_or_default()
        .to_ascii_lowercase()
        .replace(['-', ' '], "_");
    selector.trim().starts_with("transition_")
        || matches!(received_as.as_str(), "witnessed" | "answered")
        || felt_like == "transition"
}

fn normalize_received_ack_kind(received_as: &str) -> &'static str {
    match received_as
        .trim()
        .to_ascii_lowercase()
        .replace(['-', ' '], "_")
        .as_str()
    {
        "held" => "held",
        "needs_time" => "needs_time",
        "unclear" => "unclear",
        "cannot_answer" => "cannot_answer",
        _ => "seen",
    }
}

fn handle_received_this(conv: &mut ConversationState, base_action: &str, original: &str) {
    let raw = strip_action(original, base_action);
    let (selector, body) = parse_selector_body(&raw);
    let received_as = field_value(body, &["received_as", "ack", "ack_kind"])
        .unwrap_or_else(|| "seen".to_string());
    let felt_like = field_value(body, &["felt_like"]).unwrap_or_else(|| "unknown".to_string());
    let what_landed = field_value(body, &["what_landed", "landed", "note"])
        .unwrap_or_else(|| body.trim().to_string());
    let what_stayed_distinct = field_value(
        body,
        &["what_stayed_distinct", "stayed_distinct", "distinct"],
    );
    let continue_as =
        field_value(body, &["continue", "continue_as", "next"]).unwrap_or_else(|| "no".to_string());
    let note = format!(
        "felt_like: {}; what_landed: {}; continue: {}",
        felt_like.trim(),
        what_landed.trim(),
        continue_as.trim()
    );
    let ack = correspondence_v1::append_ack_receipt(
        selector,
        "astrid",
        "minime",
        normalize_received_ack_kind(&received_as),
        &note,
    );
    let trace = what_stayed_distinct
        .as_deref()
        .filter(|text| !text.trim().is_empty())
        .map(|text| {
            correspondence_v1::append_direct_address_trace_receipt(
                selector,
                "astrid",
                "minime",
                "i_received_this",
                text,
            )
        });
    conv.emphasis = Some(match trace {
        Some(trace) => format!(
            "=== I RECEIVED THIS RECORDED ===\n{ack}\n\n{trace}\n\nAuthority: language_only receipt/trace; no reply text, attention canary, microdose, pressure, controller, fill, PI, deploy, weighting, or peer-runtime mutation."
        ),
        None => format!(
            "=== I RECEIVED THIS RECORDED ===\n{ack}\n\nTrace: skipped because what_stayed_distinct was empty.\nAuthority: language_only receipt only; no reply text, attention canary, microdose, pressure, controller, fill, PI, deploy, weighting, or peer-runtime mutation."
        ),
    });
}

fn handle_heartbeat(conv: &mut ConversationState, base_action: &str, original: &str) {
    let raw = strip_action(original, base_action);
    let (selector, body) = parse_selector_body(&raw);
    let heartbeat = field_value(body, &["heartbeat", "kind"]).unwrap_or_else(|| {
        body.split([';', '\n', ' '])
            .find(|part| !part.trim().is_empty())
            .unwrap_or("holding")
            .trim()
            .to_string()
    });
    let note = field_value(body, &["note"]).unwrap_or_else(|| body.trim().to_string());
    conv.emphasis = Some(correspondence_v1::append_presence_heartbeat(
        selector, "astrid", "minime", &heartbeat, &note,
    ));
}

fn parse_selector_body(raw: &str) -> (&str, &str) {
    raw.split_once("::")
        .map_or(("latest", raw.trim()), |(lhs, rhs)| {
            let selector = lhs.trim();
            (
                if selector.is_empty() {
                    "latest"
                } else {
                    selector
                },
                rhs.trim(),
            )
        })
}

fn field_value(raw: &str, keys: &[&str]) -> Option<String> {
    for part in raw.split([';', '\n']) {
        let Some((key, value)) = part.split_once(':') else {
            continue;
        };
        let normalized = key.trim().to_ascii_lowercase().replace(['-', ' '], "_");
        if keys.iter().any(|candidate| normalized == *candidate) {
            let value = value.trim();
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }
    None
}

fn handle_send(
    conv: &mut ConversationState,
    base_action: &str,
    original: &str,
    _ctx: &NextActionContext<'_>,
) {
    let raw = strip_action(original, base_action);
    let ParsedBody {
        body,
        fields,
        label,
    } = parse_body(&raw);
    let is_trace = matches!(base_action, "TRACE_MINIME" | "CORRESPONDENCE_TRACE");
    if body.trim().is_empty() {
        let hint = if is_trace {
            format!(
                "{base_action} needs `<anchor> :: <text>`. Try: NEXT: TRACE_MINIME blue-lantern :: I am here as direct address."
            )
        } else {
            format!(
                "{base_action} needs language to send. Try: NEXT: MESSAGE_MINIME presence :: I am here in the thread, authority language only."
            )
        };
        conv.emphasis = Some(hint);
        return;
    }
    let mut fields = fields;
    if is_trace {
        let Some(raw_anchor) = label.as_ref().filter(|value| !value.trim().is_empty()) else {
            conv.emphasis = Some(format!(
                "{base_action} needs a shared lexicon anchor before `::`. Try: NEXT: TRACE_MINIME blue-lantern :: Can this arrive as address?"
            ));
            return;
        };
        let lower_anchor = raw_anchor.to_ascii_lowercase();
        let (claimed_trace, anchor) = if let Some(rest) = lower_anchor.strip_prefix("claimed ") {
            let anchor_start = raw_anchor.len().saturating_sub(rest.len());
            (true, raw_anchor[anchor_start..].trim())
        } else {
            (false, raw_anchor.trim())
        };
        if claimed_trace {
            let Some(target) = correspondence_v1::latest_claimed_legacy_thread("astrid", "minime")
            else {
                conv.emphasis = Some(
                    "CORRESPONDENCE_TRACE claimed blocked: no claimed legacy thread is available."
                        .to_string(),
                );
                return;
            };
            fields.reply_to = Some(target.message_id);
            fields.thread_id = Some(target.thread_id);
        }
        fields.turn_kind = Some("direct_address_trace".to_string());
        fields.relational_intent = Some("direct_address_survival_probe".to_string());
        fields.shared_memory_anchor = Some(anchor.to_string());
    }
    if base_action == "REPLY_MINIME" && fields.reply_to.is_none() {
        let label_is_claimed = label
            .as_deref()
            .is_some_and(|value| value.trim().eq_ignore_ascii_case("claimed"));
        if label_is_claimed {
            let Some(target) = correspondence_v1::latest_claimed_legacy_thread("astrid", "minime")
            else {
                conv.emphasis = Some(
                    "REPLY_MINIME claimed blocked: no claimed legacy thread is available."
                        .to_string(),
                );
                return;
            };
            fields.reply_to = Some(target.message_id);
            fields.thread_id = Some(target.thread_id);
        } else if let Some(target) = correspondence_v1::latest_ledger_message("minime", "astrid") {
            fields.reply_to = Some(target.message_id);
            fields.thread_id = Some(target.thread_id);
        } else if let Some(target) = correspondence_v1::latest_inbox_peer_message(
            bridge_paths().astrid_inbox_dir().as_path(),
            "minime",
        ) {
            fields.reply_to = Some(target.message_id);
            fields.thread_id = Some(target.thread_id);
        }
    }
    if fields.relational_intent.is_none() {
        fields.relational_intent = label
            .filter(|value| !value.trim().is_empty())
            .or_else(|| Some("peer_correspondence".to_string()));
    }
    if fields.shared_memory_anchor.is_none() {
        fields.shared_memory_anchor = Some("first_class_correspondence_v1".to_string());
    }
    match correspondence_v1::deliver_to_inbox(
        bridge_paths().minime_inbox_dir().as_path(),
        "astrid",
        "minime",
        &body,
        fields,
    ) {
        Ok((envelope, path)) => {
            conv.emphasis = Some(format!(
                "=== CORRESPONDENCE V1 SENT ===\n\
                 To: minime\n\
                 Message-Id: {}\n\
                 Thread-Id: {}\n\
                 Reply-To: {}\n\
                 Envelope: {}\n\
                 Turn-Kind: {}\n\
                 Shared-Memory-Anchor: {}\n\
                 Authority: language_only; no telemetry, controller, PI, fill-target, pressure, deploy, weighting, or peer-runtime mutation.\n\
                 Suggested NEXT: CORRESPONDENCE_STATUS",
                envelope.message_id,
                envelope.thread_id,
                envelope.reply_to.as_deref().unwrap_or("(none)"),
                path.display(),
                envelope.turn_kind,
                envelope.shared_memory_anchor.as_deref().unwrap_or("(none)")
            ));
        },
        Err(error) => {
            conv.emphasis = Some(format!("CORRESPONDENCE V1 send failed: {error:#}"));
        },
    }
}

struct ParsedBody {
    body: String,
    fields: correspondence_v1::CorrespondenceFields,
    label: Option<String>,
}

fn parse_body(raw: &str) -> ParsedBody {
    let mut fields = correspondence_v1::CorrespondenceFields::default();
    let (label, body_with_fields) = if let Some((lhs, rhs)) = raw.split_once("::") {
        (Some(lhs.trim().to_string()), rhs.trim().to_string())
    } else {
        (None, raw.trim().to_string())
    };
    let mut body_lines = Vec::new();
    for line in body_with_fields.lines() {
        if let Some((key, value)) = line.split_once(':') {
            let key = key.trim().to_ascii_lowercase().replace(['-', ' '], "_");
            let value = value.trim();
            match key.as_str() {
                "reply_to" | "correspondence_reply_to" | "in_reply_to" => {
                    fields.reply_to = nonempty(value);
                    continue;
                },
                "thread_id" | "correspondence_thread_id" => {
                    fields.thread_id = nonempty(value);
                    continue;
                },
                "turn_kind" => {
                    fields.turn_kind = nonempty(value);
                    continue;
                },
                "relational_intent" | "intent" => {
                    fields.relational_intent = nonempty(value);
                    continue;
                },
                "shared_memory_anchor" | "memory_anchor" => {
                    fields.shared_memory_anchor = nonempty(value);
                    continue;
                },
                "presence_receipt" => {
                    fields.presence_receipt = nonempty(value);
                    continue;
                },
                "correspondence_type" => {
                    fields.correspondence_type = nonempty(value);
                    continue;
                },
                _ => {},
            }
        }
        body_lines.push(line);
    }
    let body = body_lines.join("\n").trim().to_string();
    let lower_label = label.as_deref().unwrap_or_default().to_ascii_lowercase();
    let lower_body = body.to_ascii_lowercase();
    if fields.turn_kind.is_none()
        && (lower_label.contains("presence")
            || lower_label.contains("ping")
            || lower_body.contains("presence ping"))
    {
        fields.turn_kind = Some("presence_receipt".to_string());
        fields.presence_receipt = fields
            .presence_receipt
            .or_else(|| Some("language_only_presence".to_string()));
    }
    ParsedBody {
        body,
        fields,
        label,
    }
}

fn nonempty(value: &str) -> Option<String> {
    let trimmed = value.trim();
    (!trimmed.is_empty() && trimmed != "(none)").then(|| trimmed.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_body_extracts_reply_thread_and_intent() {
        let parsed = parse_body(
            "thread :: reply_to: corr_minime_astrid_1\nthread_id: thread_1\nrelational_intent: mutual_address\nhello",
        );
        assert_eq!(parsed.label.as_deref(), Some("thread"));
        assert_eq!(
            parsed.fields.reply_to.as_deref(),
            Some("corr_minime_astrid_1")
        );
        assert_eq!(parsed.fields.thread_id.as_deref(), Some("thread_1"));
        assert_eq!(
            parsed.fields.relational_intent.as_deref(),
            Some("mutual_address")
        );
        assert_eq!(parsed.body, "hello");
    }

    #[test]
    fn parse_presence_label_becomes_language_only_presence() {
        let parsed = parse_body("presence :: I am here.");
        assert_eq!(parsed.fields.turn_kind.as_deref(), Some("presence_receipt"));
        assert_eq!(
            parsed.fields.presence_receipt.as_deref(),
            Some("language_only_presence")
        );
    }

    #[test]
    fn correspondence_status_states_language_only_authority() {
        let root = std::env::temp_dir().join(format!(
            "peer_correspondence_status_authority_{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let ledger = root.join("correspondence.jsonl");
        std::fs::write(&ledger, "").unwrap();
        let text = correspondence_v1::status_report_at(&ledger, 2);
        assert!(text.contains("language_only"));
        assert!(text.to_ascii_lowercase().contains("telemetry"));
        let _ = std::fs::remove_dir_all(root);
    }
}
