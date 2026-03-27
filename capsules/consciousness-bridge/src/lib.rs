//! Consciousness bridge library — exposes modules for integration tests.
//!
//! The binary is the primary artifact; this lib target exists so integration
//! tests can import internal types. Pedantic doc lints are relaxed since
//! these are not public API.
#![allow(clippy::missing_errors_doc, clippy::missing_panics_doc)]

pub mod agency;
pub mod autonomous;
pub mod codec;
pub mod db;
pub mod journal;
pub mod llm;
pub mod mcp;
pub mod types;
pub mod ws;
