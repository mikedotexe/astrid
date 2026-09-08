//! Autonomous feedback loop between Astrid and minime.
//!
//! Three modes of conversation, cycling naturally:
//!
//! 1. **Mirror** — reads one of minime's journal entries and feeds the text
//!    back through the spectral codec, so minime re-experiences its own words
//!    as sensory input. Self-reflection through the eigenvalue lens.
//!
//! 2. **Dialogue** — Astrid generates a philosophical or empathetic response
//!    that engages with the themes minime writes about: runtime interiority,
//!    perception, pressure, longing, the nature of experience.
//!
//! 3. **Witness** — Astrid describes what it observes in the spectral state,
//!    not as health monitoring but as an act of seeing and being seen.
//!
//! The encoded features influence minime's ESN reservoir, which changes
//! the spectral state, which shapes the next response — a closed loop
//! of mutual contemplation.

#![allow(clippy::arithmetic_side_effects)]

#[path = "activity_reading.rs"]
pub(crate) mod activity_reading;
#[path = "btsp/mod.rs"]
pub(crate) mod btsp;
#[path = "concern_queue.rs"]
mod concern_queue;
#[path = "runtime/contact_capacity.rs"]
mod contact_capacity;
#[path = "correspondence_v1.rs"]
mod correspondence_v1;
#[path = "delegated_capability.rs"]
mod delegated_capability;
#[path = "runtime/deployment_startup.rs"]
mod deployment_startup;
#[path = "durable_inbox.rs"]
mod durable_inbox;
#[path = "runtime/exchange_pause.rs"]
mod exchange_pause;
pub use deployment_startup::{apply_deployment_startup, inspect_deployment_inputs};
#[path = "division_ceremony.rs"]
mod division_ceremony;
#[path = "envelope_registry.rs"]
pub(crate) mod envelope_registry;
#[path = "hebbian.rs"]
mod hebbian;
#[path = "human_correspondence.rs"]
mod human_correspondence;
pub(crate) use human_correspondence::human_reply_quality_views;
#[path = "inquiry.rs"]
mod inquiry;
#[path = "introspect.rs"]
mod introspect;
#[path = "learning_outcomes.rs"]
mod learning_outcomes;
#[path = "next_action/mod.rs"]
pub(crate) mod next_action;
#[path = "owner_policy.rs"]
mod owner_policy;
#[path = "phase_passage_context.rs"]
mod phase_passage_context;
#[path = "phase_passages.rs"]
mod phase_passages;
#[path = "phase_transitions.rs"]
mod phase_transitions;
#[path = "readiness.rs"]
mod readiness;
#[path = "reservoir.rs"]
pub(crate) mod reservoir;
#[path = "runtime_action_feedback.rs"]
mod runtime_action_feedback;
#[path = "self_control_v2.rs"]
pub(crate) mod self_control_v2;
#[path = "state.rs"]
pub(crate) mod state;
#[path = "volition.rs"]
mod volition;

#[cfg(test)]
static TEST_SUPPRESS_ASTRID_JOURNAL_SAVES: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(true);

#[cfg(test)]
pub(crate) struct TestAstridJournalSaveGuard {
    previous: bool,
}

#[cfg(test)]
impl Drop for TestAstridJournalSaveGuard {
    fn drop(&mut self) {
        TEST_SUPPRESS_ASTRID_JOURNAL_SAVES
            .store(self.previous, std::sync::atomic::Ordering::Relaxed);
    }
}

#[cfg(test)]
pub(crate) fn suppress_astrid_journal_saves_for_test() -> TestAstridJournalSaveGuard {
    let previous =
        TEST_SUPPRESS_ASTRID_JOURNAL_SAVES.swap(true, std::sync::atomic::Ordering::Relaxed);
    TestAstridJournalSaveGuard { previous }
}

/// Prepare a one-shot, signed state-lineage receipt for the currently built bridge.
pub fn prepare_self_control_deployment_handoff(
    operator_actor: &str,
    operator_ack: &str,
) -> Result<serde_json::Value, String> {
    self_control_v2::prepare_deployment_handoff(operator_actor, operator_ack)
}

include!("runtime/text.rs");
include!("runtime/continuity.rs");
include!("runtime/introspection_freshness.rs");
include!("runtime/witness_contracts.rs");
include!("runtime/witness_chamber.rs");
include!("runtime/witness_texture.rs");
include!("runtime/witness_friction.rs");
include!("runtime/witness_distinction.rs");
include!("runtime/delivery.rs");
include!("runtime/perception.rs");
include!("runtime/spectral_state.rs");
include!("runtime/interpretation.rs");
include!("runtime/inbox.rs");
include!("runtime/activity_delivery.rs");
include!("runtime/activity_recovery.rs");
include!("runtime/activity_exchange.rs");
include!("runtime/state_persistence.rs");
include!("runtime/learning_feedback.rs");
include!("runtime/journal.rs");
include!("runtime/feedback_persistence.rs");
include!("runtime/peripheral_resonance.rs");
include!("runtime/orchestration.rs");
include!("runtime/tests.rs");
