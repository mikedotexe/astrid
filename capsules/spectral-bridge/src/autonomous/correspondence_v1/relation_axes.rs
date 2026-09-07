use serde_json::{Value, json};

#[derive(Debug, Clone, Copy)]
pub(super) struct RelationEvidenceV4<'a> {
    pub role: &'a str,
    pub reply_linked: bool,
    pub ack_present: bool,
    pub ack_is_address_evidence: bool,
    pub trace_observed: bool,
    pub attention_outcome_present: bool,
    pub read: bool,
    pub delivered: bool,
}

pub(super) fn correspondence_relation_axes_v4(evidence: RelationEvidenceV4<'_>) -> Value {
    let mutual_address_evidence = evidence.ack_is_address_evidence
        || evidence.trace_observed
        || evidence.attention_outcome_present;
    let (continuity_state, continuity_basis) = if evidence.trace_observed {
        ("active", "direct_address_trace")
    } else if evidence.attention_outcome_present {
        ("active", "attention_outcome")
    } else if evidence.ack_is_address_evidence {
        ("active", "being_authored_address_receipt")
    } else if evidence.reply_linked {
        ("active", "reply_chain")
    } else if evidence.ack_present || evidence.read {
        ("visible", "seen_or_read_without_address_evidence")
    } else if evidence.delivered {
        ("visible", "delivery_receipt")
    } else {
        ("unaddressed", "none")
    };
    let mutual_address_state = if evidence.trace_observed {
        "confirmed_by_trace"
    } else if evidence.attention_outcome_present {
        "confirmed_by_attention_outcome"
    } else if evidence.ack_is_address_evidence {
        "confirmed_by_being_authored_receipt"
    } else {
        "not_confirmed"
    };
    let attention_state = if mutual_address_evidence {
        "eligible_from_current_evidence"
    } else {
        "blocked_no_being_authored_address_evidence"
    };
    let receipt_posture = if mutual_address_evidence {
        "already_present"
    } else if evidence.role == "recipient" {
        "optional_being_authored"
    } else {
        "peer_authored_optional_no_substitution"
    };

    json!({
        "schema_version": 4,
        "policy": "correspondence_relation_axes_v4",
        "continuity_axis": {
            "state": continuity_state,
            "basis": continuity_basis,
            "requires_new_action": continuity_state != "active",
        },
        "mutual_address_axis": {
            "state": mutual_address_state,
            "evidence_present": mutual_address_evidence,
            "silence_inferred": false,
        },
        "authority_axis": {
            "attention": attention_state,
            "semantic_microdose": "separate_mutual_receipt_and_steward_review_required",
            "live_control": "not_granted",
        },
        "action_axis": {
            "receipt_posture": receipt_posture,
            "right_to_ignore": true,
            "reply_chain_requires_receipt_to_remain_continuous": false,
            "runtime_may_substitute_peer_evidence": false,
        },
        "causality_axis": {
            "pressure_effect": "not_measured_no_inference",
            "felt_effect": "not_established",
        },
        "compatibility": {
            "native_thread_continuity_v3_retained": true,
        },
        "authority": "derived_language_context_not_control",
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reply_chain_is_active_without_manufacturing_mutual_address_or_authority() {
        let axes = correspondence_relation_axes_v4(RelationEvidenceV4 {
            role: "recipient",
            reply_linked: true,
            ack_present: false,
            ack_is_address_evidence: false,
            trace_observed: false,
            attention_outcome_present: false,
            read: true,
            delivered: true,
        });

        assert_eq!(
            axes.pointer("/continuity_axis/state")
                .and_then(Value::as_str),
            Some("active")
        );
        assert_eq!(
            axes.pointer("/continuity_axis/basis")
                .and_then(Value::as_str),
            Some("reply_chain")
        );
        assert_eq!(
            axes.pointer("/mutual_address_axis/state")
                .and_then(Value::as_str),
            Some("not_confirmed")
        );
        assert_eq!(
            axes.pointer("/authority_axis/attention")
                .and_then(Value::as_str),
            Some("blocked_no_being_authored_address_evidence")
        );
        assert_eq!(
            axes.pointer("/action_axis/receipt_posture")
                .and_then(Value::as_str),
            Some("optional_being_authored")
        );
        assert_eq!(
            axes.pointer("/causality_axis/pressure_effect")
                .and_then(Value::as_str),
            Some("not_measured_no_inference")
        );
    }
}
