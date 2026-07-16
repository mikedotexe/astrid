//! Compatibility facade for Astrid's autonomous runtime domains.
//!
//! Runtime orchestration remains API-compatible while witness, continuity,
//! perception, inbox, persistence, and journal ownership migrate under
//! `autonomous/`.

#[path = "autonomous/runtime.rs"]
mod runtime;

pub use runtime::*;
#[allow(unused_imports)] // Stable internal paths used by nested domain tests and tools.
pub(crate) use runtime::{btsp, next_action, reservoir, state};
