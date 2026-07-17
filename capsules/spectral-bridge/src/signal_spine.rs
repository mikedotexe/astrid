//! Immutable, read-only lineage for Astrid's semantic delivery path.
//!
//! The spine observes existing transformations in shadow mode. Trusted stages
//! remain in memory; persisted artifacts contain only bounded receipts, hashes,
//! measurements, and provenance references.

mod association;
mod capture;
mod recorder;
mod types;

pub use types::{
    CausalSignalJourneyV1, CausalSignalStageV1, SignalEffectV1, SignalRelationV1,
    SignalStageKindV1, SignalStageReceiptV1, SignalTemporalEnvelopeV1,
};

pub(crate) use association::{
    record_minime_temporal_associations_v1, register_delivery_temporal_window_v1,
};
pub(crate) use recorder::{
    ShadowSignalJourneyV1, SignalJourneyContextV1, SignalStageHandleV1,
    persist_shadow_signal_journey_v1, signal_deployment_identity_v1,
};
pub(crate) use types::SignalOwnershipDomainV1;

#[cfg(test)]
mod tests;
