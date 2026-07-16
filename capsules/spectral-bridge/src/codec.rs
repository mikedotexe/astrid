//! Compatibility facade for spectral codec projection and evidence APIs.
//!
//! The stable `crate::codec::*` surface is retained while implementation
//! ownership lives under `codec/`.

#[path = "codec/core.rs"]
mod core;

pub use core::*;
