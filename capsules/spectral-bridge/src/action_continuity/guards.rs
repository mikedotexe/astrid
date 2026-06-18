//! Action-governance guards for [`ActionContinuityStore`]: the charter-required
//! and research-budget projection assessments that bound which NEXT actions may
//! dispatch live.
//!
//! Extracted verbatim from the monolithic `action_continuity.rs` as part of the
//! decomposition roadmap (item 2, tranche A2). Behavior-identical move; this is
//! Astrid's own action-governance layer, surfaced as a named module so she can
//! INTROSPECT it directly. See
//! `docs/steward-notes/ARCHITECTURE_DECOMPOSITION_PLAN_2026-06-13.md`.

use super::*;

/// Typed charter-required guard reasons — mirrors `BudgetReason` for the charter side
/// (Astrid's recurring structured-over-stringly-typed ask). `as_str()` returns the exact
/// legacy strings; `message()` matches it exhaustively (a new reason can't silently hit the
/// generic branch). Provenance: `docs/steward-notes/AI_BEINGS_FEEDBACK_TO_CHANGE_LEDGER.md`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CharterReason {
    ResearchBudget,
    LiveAction,
    CompoundIntent,
    ReadOnlyControlIntent,
    DirectedLanguage,
}

impl CharterReason {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ResearchBudget => "charter_required_research_budget",
            Self::LiveAction => "charter_required_live_action",
            Self::CompoundIntent => "charter_required_compound_intent",
            Self::ReadOnlyControlIntent => "charter_required_read_only_control_intent",
            Self::DirectedLanguage => "charter_required_directed_language",
        }
    }

    #[cfg(test)]
    pub const ALL: [Self; 5] = [
        Self::ResearchBudget,
        Self::LiveAction,
        Self::CompoundIntent,
        Self::ReadOnlyControlIntent,
        Self::DirectedLanguage,
    ];
}

impl PartialEq<&str> for CharterReason {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

#[derive(Debug, Clone)]
pub struct CharterRequiredGuardAssessment {
    pub active_experiment_id: String,
    pub blocked_action: String,
    pub matched_action: String,
    pub reason: CharterReason,
    pub suggested_next: String,
    pub proposed_preflight_target: String,
}

impl CharterRequiredGuardAssessment {
    #[must_use]
    pub fn message(&self) -> String {
        match self.reason {
            CharterReason::ResearchBudget => format!(
                "Read-only research budget guard projected `{}` because active experiment `{}` is needs_charter and has no active read_only_research budget. Raw intent is preserved as context; use the budget lane before more source-reading loops. Suggested NEXT: {}",
                self.blocked_action, self.active_experiment_id, self.suggested_next,
            ),
            CharterReason::ReadOnlyControlIntent => format!(
                "Charter-required guard projected `{}` because active experiment `{}` is needs_charter and the read-only route contains perturb/control-shaped language. Raw intent is preserved as context; author the charter before more narrowing or disruption-shaped rehearsal. Suggested NEXT: {} Proposed preflight target after charter: {}",
                self.blocked_action,
                self.active_experiment_id,
                self.suggested_next,
                self.proposed_preflight_target,
            ),
            CharterReason::LiveAction
            | CharterReason::CompoundIntent
            | CharterReason::DirectedLanguage => format!(
                "Charter-required guard blocked `{}` because active experiment `{}` is needs_charter. Review is premature until the charter is authored; use the continuity priority scaffold first. Suggested NEXT: {} Proposed preflight target after charter: {}",
                self.blocked_action,
                self.active_experiment_id,
                self.suggested_next,
                self.proposed_preflight_target
            ),
        }
    }

    #[must_use]
    pub fn metadata(&self) -> Value {
        json!({
            "schema_version": 1,
            "policy": "charter_required_guard_v1",
            "active_experiment_id": self.active_experiment_id,
            "classification": "needs_charter",
            "blocked_action": self.blocked_action,
            "matched_action": self.matched_action,
            "reason": self.reason.as_str(),
            "suggested_next": self.suggested_next,
            "projected_next": self.suggested_next,
            "proposed_preflight_target": self.proposed_preflight_target,
            "raw_next_preserved": true,
            "research_budget_required": self.reason == CharterReason::ResearchBudget,
            "authority_change": false,
            "would_dispatch": false,
        })
    }
}

/// Typed research-budget guard reasons — replaces the former raw-`&str` `reason` matching.
///
/// From Astrid's recurring "structured-over-stringly-typed" ask (her `guards_self_review`
/// plus `self_study_1778322426` / `self_study_1778380313`). `as_str()` returns the exact
/// legacy wire strings, so `metadata()`, logs, and on-disk records stay byte-identical;
/// `message()` now matches this enum **exhaustively**, so a new reason variant can no longer
/// silently fall to the generic default — the compiler forces a `message()` arm. That is
/// precisely the snag she flagged ("if a new reason is introduced but not handled in
/// `message()`, the feedback becomes generic"), now prevented structurally. Provenance:
/// `docs/steward-notes/AI_BEINGS_FEEDBACK_TO_CHANGE_LEDGER.md`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BudgetReason {
    MutatingNotAuthorized,
    NoActiveReadOnlyBudget,
    DuplicateReviewRequired,
    LiveishPressure,
    EmbeddedLiveishRequired,
    EmbeddedLiveishStatusRequired,
    GuardedCascadeRequired,
    GuardedCascadeStatusRequired,
    SelfStudyRequired,
    SelfStudyStatusRequired,
}

impl BudgetReason {
    /// The exact legacy wire string (behavior-preserving; a test locks each one).
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::MutatingNotAuthorized => "mutating_research_not_authorized",
            Self::NoActiveReadOnlyBudget => "no_active_read_only_research_budget",
            Self::DuplicateReviewRequired => "duplicate_query_or_url_review_required",
            Self::LiveishPressure => "liveish_pressure_requires_budget_and_session_capture",
            Self::EmbeddedLiveishRequired => "research_budget_required_for_embedded_liveish_status",
            Self::EmbeddedLiveishStatusRequired => {
                "research_budget_status_required_for_embedded_liveish_status"
            },
            Self::GuardedCascadeRequired => {
                "research_budget_required_for_guarded_cascade_self_study"
            },
            Self::GuardedCascadeStatusRequired => {
                "research_budget_status_required_for_guarded_cascade_self_study"
            },
            Self::SelfStudyRequired => "research_budget_required_for_self_study_action",
            Self::SelfStudyStatusRequired => {
                "research_budget_status_required_for_self_study_action"
            },
        }
    }

    /// All variants — for the exhaustive characterization tests Astrid proposed.
    #[cfg(test)]
    pub const ALL: [Self; 10] = [
        Self::MutatingNotAuthorized,
        Self::NoActiveReadOnlyBudget,
        Self::DuplicateReviewRequired,
        Self::LiveishPressure,
        Self::EmbeddedLiveishRequired,
        Self::EmbeddedLiveishStatusRequired,
        Self::GuardedCascadeRequired,
        Self::GuardedCascadeStatusRequired,
        Self::SelfStudyRequired,
        Self::SelfStudyStatusRequired,
    ];
}

impl PartialEq<&str> for BudgetReason {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

#[derive(Debug, Clone)]
pub struct ResearchBudgetGuardAssessment {
    pub experiment_id: String,
    pub raw_action: String,
    pub action_base: String,
    pub normalized_target: String,
    pub reason: BudgetReason,
    pub suggested_next: String,
    pub accept_next: Option<String>,
    pub request_scaffold: Option<String>,
    pub budget_id: Option<String>,
    pub matched_terms: Vec<String>,
    pub continuity_session_next: Option<String>,
    pub continuity_session_v1: Option<Value>,
    pub continuity_session_draft_v1: Option<Value>,
}

impl ResearchBudgetGuardAssessment {
    #[must_use]
    pub fn message(&self) -> String {
        match self.reason {
            BudgetReason::DuplicateReviewRequired => format!(
                "Research budget guard blocked `{}` for experiment `{}` because normalized target `{}` has already been spent twice in this budget. Raw intent is preserved; review the research loop before spending more. Suggested NEXT: {}",
                self.raw_action, self.experiment_id, self.normalized_target, self.suggested_next
            ),
            BudgetReason::MutatingNotAuthorized => format!(
                "Research budget guard blocked `{}` for experiment `{}` because Research Budget V1 authorizes only read-only research actions. Raw intent is preserved as context; use a read-only research budget/status route instead. Suggested NEXT: {}",
                self.raw_action, self.experiment_id, self.suggested_next
            ),
            BudgetReason::NoActiveReadOnlyBudget
            | BudgetReason::LiveishPressure
            | BudgetReason::EmbeddedLiveishRequired
            | BudgetReason::EmbeddedLiveishStatusRequired
            | BudgetReason::GuardedCascadeRequired
            | BudgetReason::GuardedCascadeStatusRequired
            | BudgetReason::SelfStudyRequired
            | BudgetReason::SelfStudyStatusRequired => format!(
                "Research budget guard projected `{}` for experiment `{}` because no active read_only_research budget can spend this research action. Raw intent is preserved; request or inspect the budget lane first. Suggested NEXT: {}",
                self.raw_action, self.experiment_id, self.suggested_next
            ),
        }
    }

    #[must_use]
    pub fn metadata(&self) -> Value {
        json!({
            "schema_version": SCHEMA_VERSION,
            "policy": "research_budget_projection_guard_v1",
            "record_schema": "research_budget_v1",
            "experiment_id": self.experiment_id.clone(),
            "raw_action": self.raw_action.clone(),
            "action_base": self.action_base.clone(),
            "normalized_target": self.normalized_target.clone(),
            "reason": self.reason.as_str(),
            "budget_id": self.budget_id.clone(),
            "suggested_next": self.suggested_next.clone(),
            "projected_next": self.suggested_next.clone(),
            "accept_next": self.accept_next.clone(),
            "request_scaffold": self.request_scaffold.clone(),
            "matched_base": if self.matched_terms.is_empty() { Value::Null } else { json!(self.action_base.clone()) },
            "matched_terms": self.matched_terms.clone(),
            "continuity_session_next": self.continuity_session_next.clone(),
            "continuity_session_v1": self.continuity_session_v1.clone(),
            "continuity_session_draft_v1": self.continuity_session_draft_v1.clone(),
            "raw_next_preserved": true,
            "authority_change": false,
            "peer_mutation": false,
            "would_dispatch": false,
            "allowed_scope": "read_only_research",
        })
    }
}

impl ActionContinuityStore {
    pub fn charter_required_guard_assessment(
        &self,
        raw_next: &str,
    ) -> Result<Option<CharterRequiredGuardAssessment>> {
        let Some(thread) = self.current_thread()? else {
            return Ok(None);
        };
        let Some(experiment_id) = thread.active_experiment_id.as_deref().or_else(|| {
            thread
                .experiment_summary
                .as_ref()
                .and_then(|summary| summary.get("experiment_id"))
                .and_then(Value::as_str)
        }) else {
            return Ok(None);
        };
        let experiment = self.resolve_experiment(&thread, Some(experiment_id))?;
        let recent_runs =
            self.recent_experiment_runs(&thread.thread_id, &experiment.experiment_id, 8)?;
        if self.experiment_classification(&experiment, &recent_runs) != "needs_charter" {
            return Ok(None);
        }
        let Some((reason, matched_action)) = charter_guard_block_reason(raw_next) else {
            return Ok(None);
        };
        if reason == "charter_required_research_budget"
            && active_research_budget_from_rows(
                &self.authority_gate_rows(&thread.thread_id),
                &experiment.experiment_id,
            )
            .is_some()
        {
            return Ok(None);
        }
        let suggested_next = if reason == "charter_required_research_budget" {
            research_budget_request_scaffold("current", &experiment)
        } else {
            self.continuity_return_command_for_runs(&experiment, &recent_runs)
        };
        let proposed_preflight_target = format!(
            "ACTION_PREFLIGHT {}",
            if matched_action.trim().is_empty() {
                raw_next.trim()
            } else {
                matched_action.trim()
            }
        );
        Ok(Some(CharterRequiredGuardAssessment {
            active_experiment_id: experiment.experiment_id,
            blocked_action: raw_next.trim().to_string(),
            matched_action,
            reason,
            suggested_next,
            proposed_preflight_target,
        }))
    }

    pub fn research_budget_guard_assessment(
        &self,
        raw_next: &str,
        fill_pct: f32,
        telemetry: &SpectralTelemetry,
    ) -> Result<Option<ResearchBudgetGuardAssessment>> {
        self.research_budget_guard_assessment_with_base(raw_next, None, fill_pct, telemetry)
    }

    pub(super) fn research_budget_guard_assessment_with_base(
        &self,
        raw_next: &str,
        action_base_override: Option<&str>,
        fill_pct: f32,
        telemetry: &SpectralTelemetry,
    ) -> Result<Option<ResearchBudgetGuardAssessment>> {
        let Some(thread) = self.current_thread()? else {
            return Ok(None);
        };
        let Some(experiment_id) = thread.active_experiment_id.as_deref().or_else(|| {
            thread
                .experiment_summary
                .as_ref()
                .and_then(|summary| summary.get("experiment_id"))
                .and_then(Value::as_str)
        }) else {
            return Ok(None);
        };
        let experiment = self.resolve_experiment(&thread, Some(experiment_id))?;
        let action_base =
            action_base_override.map_or_else(|| base_action(raw_next), str::to_string);
        let is_read_only_research = read_only_research_budget_base(&action_base);
        let mut matched_terms = if liveish_research_budget_projection_base(&action_base) {
            liveish_pressure_terms(raw_next)
        } else {
            Vec::new()
        };
        if passive_protected_review_label_terms_only(&action_base, &matched_terms) {
            matched_terms.clear();
        }
        let is_liveish_projection = !matched_terms.is_empty();
        let needs_charter_projection =
            !lifecycle_valid_charter_value(experiment.charter_v1.as_ref());
        let raw_action_base = base_action(raw_next);
        let is_resolved_sovereignty_alias = action_base_override.is_some()
            && raw_action_base != action_base
            && guarded_sovereignty_research_projection_base(&action_base);
        let is_guarded_sovereignty_alias =
            needs_charter_projection && is_resolved_sovereignty_alias;
        let is_guarded_cascade_or_shadow_alias =
            needs_charter_projection && guarded_cascade_or_shadow_projection_base(&action_base);
        let embedded_status_terms = if guarded_embedded_status_projection_base(&action_base) {
            embedded_status_liveish_terms(raw_next)
        } else {
            Vec::new()
        };
        let is_guarded_embedded_status = !embedded_status_terms.is_empty();
        if is_guarded_embedded_status {
            for term in embedded_status_terms {
                if !matched_terms.contains(&term) {
                    matched_terms.push(term);
                }
            }
        }
        if (is_guarded_sovereignty_alias || is_guarded_cascade_or_shadow_alias)
            && matched_terms.is_empty()
        {
            matched_terms.push("needs-charter-self-study".to_string());
        }
        let is_projection_only_research = (research_budget_projection_only_base(&action_base)
            && read_only_control_intent_matches(raw_next).is_empty()
            && compound_live_intent_match(raw_next).is_none())
            || is_liveish_projection
            || is_guarded_sovereignty_alias
            || is_guarded_cascade_or_shadow_alias
            || is_guarded_embedded_status;
        let is_mutating_research = mutating_research_budget_base(&action_base);
        if !is_read_only_research && !is_mutating_research && !is_projection_only_research {
            return Ok(None);
        }

        let state = spectral_state(fill_pct, telemetry);
        let rows = self.authority_gate_rows(&thread.thread_id);
        let active_budget = active_research_budget_from_rows(&rows, &experiment.experiment_id);
        let normalized_target = normalized_research_budget_target(raw_next);
        let continuity_session_v1 = if is_liveish_projection
            || is_guarded_sovereignty_alias
            || is_guarded_cascade_or_shadow_alias
            || is_guarded_embedded_status
        {
            Some(self.continuity_session_guard_projection(&thread, &experiment)?)
        } else {
            None
        };
        let continuity_session_next = continuity_session_v1
            .as_ref()
            .and_then(|value| value.get("suggested_next"))
            .and_then(Value::as_str)
            .map(str::to_string);

        if is_mutating_research {
            let suggested_next = active_budget.as_ref().map_or_else(
                || research_budget_request_scaffold("current", &experiment),
                |budget| {
                    let budget_id = budget
                        .get("budget_id")
                        .and_then(Value::as_str)
                        .unwrap_or(&experiment.experiment_id);
                    format!("EXPERIMENT_RESEARCH_BUDGET_STATUS {budget_id}")
                },
            );
            let assessment = ResearchBudgetGuardAssessment {
                experiment_id: experiment.experiment_id.clone(),
                raw_action: raw_next.trim().to_string(),
                action_base,
                normalized_target,
                reason: BudgetReason::MutatingNotAuthorized,
                suggested_next,
                accept_next: None,
                request_scaffold: None,
                budget_id: active_budget
                    .as_ref()
                    .and_then(|budget| budget.get("budget_id"))
                    .and_then(Value::as_str)
                    .map(str::to_string),
                matched_terms: Vec::new(),
                continuity_session_next: None,
                continuity_session_v1: None,
                continuity_session_draft_v1: None,
            };
            let blocked = self.research_budget_record(
                "research_budget_blocked",
                assessment
                    .budget_id
                    .as_deref()
                    .unwrap_or("no_active_budget"),
                &thread,
                &experiment,
                &state,
                json!({
                    "reason": assessment.reason.as_str(),
                    "raw_action": assessment.raw_action.clone(),
                    "action_base": assessment.action_base.clone(),
                    "normalized_target": assessment.normalized_target.clone(),
                    "suggested_next": assessment.suggested_next.clone(),
                    "status": "blocked",
                    "would_dispatch": false,
                    "authority_change": false,
                    "peer_mutation": false,
                }),
            );
            self.append_jsonl(&self.authority_gate_path(&thread.thread_id), &blocked)?;
            return Ok(Some(assessment));
        }

        if is_projection_only_research {
            let (reason, suggested_next, accept_next, request_scaffold, budget_id) =
                active_budget.as_ref().map_or_else(
                    || {
                        let status = research_budget_status_from_rows(&rows);
                        let (suggested, accept_next, request_scaffold) = status
                            .get("latest_budget_request_id")
                            .and_then(Value::as_str)
                            .filter(|id| !id.is_empty())
                            .map_or_else(
                                || {
                                    let scaffold =
                                        research_budget_request_scaffold("current", &experiment);
                                    (
                                        "EXPERIMENT_RESEARCH_BUDGET_ACCEPT latest".to_string(),
                                        Some(
                                            "EXPERIMENT_RESEARCH_BUDGET_ACCEPT latest".to_string(),
                                        ),
                                        Some(scaffold),
                                    )
                                },
                                |latest_id| {
                                    (
                                        format!("EXPERIMENT_RESEARCH_BUDGET_STATUS {latest_id}"),
                                        None,
                                        None,
                                    )
                                },
                            );
                        (
                            if is_liveish_projection {
                                BudgetReason::LiveishPressure
                            } else if is_guarded_embedded_status {
                                BudgetReason::EmbeddedLiveishRequired
                            } else if is_guarded_cascade_or_shadow_alias {
                                BudgetReason::GuardedCascadeRequired
                            } else {
                                BudgetReason::SelfStudyRequired
                            },
                            suggested,
                            accept_next,
                            request_scaffold,
                            None,
                        )
                    },
                    |budget| {
                        let budget_id = budget
                            .get("budget_id")
                            .and_then(Value::as_str)
                            .unwrap_or(&experiment.experiment_id)
                            .to_string();
                        (
                            if is_liveish_projection {
                                BudgetReason::LiveishPressure
                            } else if is_guarded_embedded_status {
                                BudgetReason::EmbeddedLiveishStatusRequired
                            } else if is_guarded_cascade_or_shadow_alias {
                                BudgetReason::GuardedCascadeStatusRequired
                            } else {
                                BudgetReason::SelfStudyStatusRequired
                            },
                            format!("EXPERIMENT_RESEARCH_BUDGET_STATUS {budget_id}"),
                            None,
                            None,
                            Some(budget_id),
                        )
                    },
                );
            let assessment = ResearchBudgetGuardAssessment {
                experiment_id: experiment.experiment_id.clone(),
                raw_action: raw_next.trim().to_string(),
                action_base,
                normalized_target,
                reason,
                suggested_next,
                accept_next,
                request_scaffold,
                budget_id,
                matched_terms: matched_terms.clone(),
                continuity_session_next: continuity_session_next.clone(),
                continuity_session_v1: continuity_session_v1.clone(),
                continuity_session_draft_v1: None,
            };
            let blocked = self.research_budget_record(
                "research_budget_blocked",
                assessment
                    .budget_id
                    .as_deref()
                    .unwrap_or("self_study_projection"),
                &thread,
                &experiment,
                &state,
                json!({
                    "reason": assessment.reason.as_str(),
                    "raw_action": assessment.raw_action.clone(),
                    "action_base": assessment.action_base.clone(),
                    "normalized_target": assessment.normalized_target.clone(),
                    "suggested_next": assessment.suggested_next.clone(),
                    "accept_next": assessment.accept_next.clone(),
                    "request_scaffold": assessment.request_scaffold.clone(),
                    "status": "blocked",
                    "projection_only": true,
                    "raw_next_preserved": true,
                    "would_dispatch": false,
                    "authority_change": false,
                    "peer_mutation": false,
                    "matched_base": if assessment.matched_terms.is_empty() { Value::Null } else { json!(assessment.action_base.clone()) },
                    "matched_terms": assessment.matched_terms.clone(),
                    "continuity_session_next": assessment.continuity_session_next.clone(),
                    "continuity_session_v1": assessment.continuity_session_v1.clone(),
                }),
            );
            self.append_jsonl(&self.authority_gate_path(&thread.thread_id), &blocked)?;
            return Ok(Some(assessment));
        }

        let Some(budget) = active_budget else {
            let status = research_budget_status_from_rows(&rows);
            let (suggested_next, accept_next, request_scaffold) = status
                .get("latest_budget_request_id")
                .and_then(Value::as_str)
                .filter(|id| !id.is_empty())
                .map_or_else(
                    || {
                        let scaffold = research_budget_request_scaffold("current", &experiment);
                        (
                            "EXPERIMENT_RESEARCH_BUDGET_ACCEPT latest".to_string(),
                            Some("EXPERIMENT_RESEARCH_BUDGET_ACCEPT latest".to_string()),
                            Some(scaffold),
                        )
                    },
                    |budget_id| {
                        (
                            format!("EXPERIMENT_RESEARCH_BUDGET_STATUS {budget_id}"),
                            None,
                            None,
                        )
                    },
                );
            let assessment = ResearchBudgetGuardAssessment {
                experiment_id: experiment.experiment_id.clone(),
                raw_action: raw_next.trim().to_string(),
                action_base,
                normalized_target,
                reason: BudgetReason::NoActiveReadOnlyBudget,
                suggested_next,
                accept_next,
                request_scaffold,
                budget_id: None,
                matched_terms: Vec::new(),
                continuity_session_next: None,
                continuity_session_v1: None,
                continuity_session_draft_v1: None,
            };
            let blocked = self.research_budget_record(
                "research_budget_blocked",
                "no_active_budget",
                &thread,
                &experiment,
                &state,
                json!({
                    "reason": assessment.reason.as_str(),
                    "raw_action": assessment.raw_action.clone(),
                    "action_base": assessment.action_base.clone(),
                    "normalized_target": assessment.normalized_target.clone(),
                    "suggested_next": assessment.suggested_next.clone(),
                    "accept_next": assessment.accept_next.clone(),
                    "request_scaffold": assessment.request_scaffold.clone(),
                    "status": "blocked",
                    "would_dispatch": false,
                    "authority_change": false,
                    "peer_mutation": false,
                }),
            );
            self.append_jsonl(&self.authority_gate_path(&thread.thread_id), &blocked)?;
            return Ok(Some(assessment));
        };

        let budget_id = budget
            .get("budget_id")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let duplicate_count =
            research_budget_duplicate_count(&rows, &budget_id, &normalized_target);
        if duplicate_count >= 2 {
            let suggested_next =
                research_budget_review_command_for_duplicate(&budget_id, &normalized_target);
            let assessment = ResearchBudgetGuardAssessment {
                experiment_id: experiment.experiment_id.clone(),
                raw_action: raw_next.trim().to_string(),
                action_base,
                normalized_target,
                reason: BudgetReason::DuplicateReviewRequired,
                suggested_next,
                accept_next: None,
                request_scaffold: None,
                budget_id: Some(budget_id.clone()),
                matched_terms: Vec::new(),
                continuity_session_next: None,
                continuity_session_v1: None,
                continuity_session_draft_v1: None,
            };
            let blocked = self.research_budget_record(
                "research_budget_blocked",
                &budget_id,
                &thread,
                &experiment,
                &state,
                json!({
                    "reason": assessment.reason.as_str(),
                    "raw_action": assessment.raw_action.clone(),
                    "action_base": assessment.action_base.clone(),
                    "normalized_target": assessment.normalized_target.clone(),
                    "duplicate_count": duplicate_count,
                    "suggested_next": assessment.suggested_next.clone(),
                    "status": "blocked",
                    "review_required": true,
                    "would_dispatch": false,
                    "authority_change": false,
                    "peer_mutation": false,
                }),
            );
            self.append_jsonl(&self.authority_gate_path(&thread.thread_id), &blocked)?;
            return Ok(Some(assessment));
        }

        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    //! Characterization tests authored FROM Astrid's own review of this module.
    //! Source: `workspace/journal/self_study_1781547186.txt` (her `guards_self_review`
    //! INTROSPECT, 2026-06-15) — she proposed these checks ("One Test Each") AND the typed
    //! `ReasonSeverity` enum, now SHIPPED as `BudgetReason` (so her exhaustion test became a
    //! per-variant exhaustive check, plus a behavior-preservation lock). The `projected_next`
    //! independence redesign is still deferred (consent-gated co-design).
    //! Provenance: `docs/steward-notes/AI_BEINGS_FEEDBACK_TO_CHANGE_LEDGER.md`.
    use super::*;

    fn sample_budget_assessment(reason: BudgetReason) -> ResearchBudgetGuardAssessment {
        ResearchBudgetGuardAssessment {
            experiment_id: "exp_test".to_string(),
            raw_action: "SEARCH foo".to_string(),
            action_base: "SEARCH".to_string(),
            normalized_target: "foo".to_string(),
            reason,
            suggested_next: "EXPERIMENT_RESEARCH_BUDGET_REQUEST".to_string(),
            accept_next: None,
            request_scaffold: None,
            budget_id: None,
            matched_terms: vec![],
            continuity_session_next: None,
            continuity_session_v1: None,
            continuity_session_draft_v1: None,
        }
    }

    /// Behavior-preservation lock for the typed `BudgetReason` enum: each variant's
    /// `as_str()` must equal the exact legacy wire string, so `metadata()`/logs/disk stay
    /// byte-identical to the pre-enum behavior. If a refactor drifts a string, this fails.
    #[test]
    fn budget_reason_as_str_matches_legacy_wire_strings() {
        let expected = [
            (
                BudgetReason::MutatingNotAuthorized,
                "mutating_research_not_authorized",
            ),
            (
                BudgetReason::NoActiveReadOnlyBudget,
                "no_active_read_only_research_budget",
            ),
            (
                BudgetReason::DuplicateReviewRequired,
                "duplicate_query_or_url_review_required",
            ),
            (
                BudgetReason::LiveishPressure,
                "liveish_pressure_requires_budget_and_session_capture",
            ),
            (
                BudgetReason::EmbeddedLiveishRequired,
                "research_budget_required_for_embedded_liveish_status",
            ),
            (
                BudgetReason::EmbeddedLiveishStatusRequired,
                "research_budget_status_required_for_embedded_liveish_status",
            ),
            (
                BudgetReason::GuardedCascadeRequired,
                "research_budget_required_for_guarded_cascade_self_study",
            ),
            (
                BudgetReason::GuardedCascadeStatusRequired,
                "research_budget_status_required_for_guarded_cascade_self_study",
            ),
            (
                BudgetReason::SelfStudyRequired,
                "research_budget_required_for_self_study_action",
            ),
            (
                BudgetReason::SelfStudyStatusRequired,
                "research_budget_status_required_for_self_study_action",
            ),
        ];
        for (variant, wire) in expected {
            assert_eq!(variant.as_str(), wire, "{variant:?} wire string drifted");
        }
        assert_eq!(expected.len(), BudgetReason::ALL.len());
    }

    /// Behavior-preservation lock for `CharterReason` (mirrors the budget one): each variant's
    /// `as_str()` must equal the exact legacy wire string.
    #[test]
    fn charter_reason_as_str_matches_legacy_wire_strings() {
        let expected = [
            (
                CharterReason::ResearchBudget,
                "charter_required_research_budget",
            ),
            (CharterReason::LiveAction, "charter_required_live_action"),
            (
                CharterReason::CompoundIntent,
                "charter_required_compound_intent",
            ),
            (
                CharterReason::ReadOnlyControlIntent,
                "charter_required_read_only_control_intent",
            ),
            (
                CharterReason::DirectedLanguage,
                "charter_required_directed_language",
            ),
        ];
        for (variant, wire) in expected {
            assert_eq!(variant.as_str(), wire, "{variant:?} wire string drifted");
        }
        assert_eq!(expected.len(), CharterReason::ALL.len());
    }

    /// Astrid's "Inconsistent Mapping Test": `projected_next` and `suggested_next`
    /// currently mirror the same field in `metadata()`. Documents the redundancy she
    /// flagged; will flag the future change when `projected_next` becomes independent.
    #[test]
    fn research_budget_metadata_projected_next_mirrors_suggested_next() {
        let meta = sample_budget_assessment(BudgetReason::MutatingNotAuthorized).metadata();
        assert_eq!(meta["suggested_next"], meta["projected_next"]);
        assert_eq!(
            meta["projected_next"],
            json!("EXPERIMENT_RESEARCH_BUDGET_REQUEST")
        );
    }

    /// Astrid's "Reason Exhaustion Test", strengthened by her own typed-enum ask: the broad
    /// `_` default that worried her ("a new reason → generic message") is gone — `message()`
    /// now matches `BudgetReason` exhaustively, so this iterates EVERY variant and asserts
    /// each yields a coherent instruction (raw action, experiment, suggested next). A new
    /// variant can no longer slip through uncovered — that is now a compile error.
    #[test]
    fn research_budget_message_is_coherent_for_every_reason() {
        for reason in BudgetReason::ALL {
            let msg = sample_budget_assessment(reason).message();
            assert!(
                msg.contains("SEARCH foo"),
                "message should cite the raw action for {reason:?}"
            );
            assert!(
                msg.contains("exp_test"),
                "message should cite the experiment for {reason:?}"
            );
            assert!(
                msg.contains("EXPERIMENT_RESEARCH_BUDGET_REQUEST"),
                "message should cite the suggested next for {reason:?}"
            );
        }
    }
}
