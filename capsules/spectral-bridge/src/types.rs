//! Compatibility facade for the bridge's established schema surface.
//!
//! New ownership-specific code belongs in `types/`; downstream callers retain
//! the stable `crate::types::*` path through these re-exports.

#[path = "types/schema.rs"]
mod schema;

pub use schema::*;
