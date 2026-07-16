//! Compatibility facade for bridge state, health, telemetry, and sensory ports.

#[path = "ws/runtime.rs"]
mod runtime;

pub use runtime::*;
