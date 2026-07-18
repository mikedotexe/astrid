//! Spectral bridge library — exposes modules for integration tests.
//!
//! The binary is the primary artifact; this lib target exists so integration
//! tests can import internal types. Pedantic doc lints are relaxed since
//! these are not public API.
// This capsule carries a large local-observability surface that predates the
// current pedantic lint bar. Keep safety lints active, but do not let style debt
// block scoped guardrail work.
#![recursion_limit = "256"]
#![allow(
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::pedantic,
    clippy::too_many_arguments
)]

pub mod action_continuity;
pub mod action_self_knowledge;
pub mod agency;
pub mod astrid_shadow;
pub mod attractor_atlas;
pub mod audio;
pub mod authority_gate;
pub mod authority_temporal;
pub mod authority_types;
pub mod autonomous;
pub mod autoresearch;
pub mod being_memory;
pub mod chimera;
pub mod chimera_prime;
pub mod codec;
pub mod codec_explorer;
pub mod codec_gain;
pub mod codec_gain_flow;
pub mod codec_lambda_analysis;
pub mod codec_matrix;
pub mod codec_phase_space;
pub mod codec_scored_surface;
pub mod codec_time_domain;
pub mod condition_metrics;
pub mod continuity_control_plane;
pub mod db;
pub mod experiment_conveyor;
pub mod journal;
pub mod lambda_edge;
pub mod lambda_tail;
pub mod llm;
pub mod llm_jobs;
#[path = "../../shared/managed_dir.rs"]
pub mod managed_dir;
pub mod mcp;
pub mod memory;
pub mod message_archive;
pub mod paths;
pub mod prompt_budget;
pub mod reflective;
pub mod rescue_policy;
pub mod self_continuity;
pub mod self_model;
pub mod shared_investigation;
pub mod signal_spine;
pub mod spectral_explorer;
pub mod spectral_schema;
pub mod spectral_viz;
pub mod sticky_mode;
pub mod trace_lab;
pub mod types;
pub mod witness;
pub mod ws;

#[cfg(test)]
mod llm_tests;
