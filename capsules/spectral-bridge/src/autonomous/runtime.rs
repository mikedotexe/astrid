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

#[path = "btsp/mod.rs"]
pub(crate) mod btsp;
#[path = "correspondence_v1.rs"]
mod correspondence_v1;
#[path = "hebbian.rs"]
mod hebbian;
#[path = "introspect.rs"]
mod introspect;
#[path = "next_action/mod.rs"]
pub(crate) mod next_action;
#[path = "phase_transitions.rs"]
mod phase_transitions;
#[path = "readiness.rs"]
mod readiness;
#[path = "reservoir.rs"]
pub(crate) mod reservoir;
#[path = "state.rs"]
pub(crate) mod state;

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
include!("runtime/state_persistence.rs");
include!("runtime/journal.rs");
include!("runtime/feedback_persistence.rs");
include!("runtime/orchestration.rs");
include!("runtime/tests.rs");
