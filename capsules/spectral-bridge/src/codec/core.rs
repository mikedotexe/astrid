//! Core spectral codec implementation: text projection and evidence derivation.
//!
//! The codec maps text into minime's 48-dimensional semantic lane
//! and interprets spectral telemetry as natural language.
//!
//! Dim layout:
//!   0-7:   Character-level statistics (entropy, density, rhythm)
//!   8-15:  Word-level features (lexical diversity, hedging, certainty)
//!   16-23: Sentence-level structure (length variance, question density)
//!   24-31: Emotional/intentional markers (warmth, tension, curiosity)
//!   32-39: Embedding-projected semantic features (nomic-embed-text → 8D)
//!   40-43: Narrative arc (semantic shift from first half to second half)
//!   44-47: Reserved
//!
//! The encoder is deterministic — no neural network, no external API.
//! It extracts structural and statistical properties of text that
//! create a unique spectral fingerprint. The same text always produces
//! the same features, but similar texts produce similar features.

// The codec intentionally uses floating-point arithmetic for feature
// extraction. Statistical casts feed bounded tanh outputs, while projection
// accumulation precision stays explicitly measurable against an f64 reference
// before any live migration is considered.
#![allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::arithmetic_side_effects
)]

include!("projection.rs");
include!("pressure.rs");
include!("text_history.rs");
include!("encoding.rs");
include!("evidence_types.rs");
include!("interpretation.rs");
include!("projection_evidence.rs");
include!("focus_evidence.rs");
include!("structural_evidence.rs");
include!("structure.rs");
include!("cascade.rs");
include!("feedback.rs");
include!("visual.rs");
include!("tests.rs");
