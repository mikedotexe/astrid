#![deny(unsafe_code)]
#![recursion_limit = "256"]

mod attestation;
mod authored_transaction;
mod candidate;
mod candidate_ledger;
mod config;
mod context_provenance;
mod evidence;
mod gate;
mod handoff;
mod inquiry;
#[cfg(test)]
mod inquiry_tests;
mod integration;
mod lifecycle;
mod maintenance;
mod model_lock;
mod owned;
mod prompt;
mod provider;
mod publication;
mod reflection;
mod reporting;
mod runner;
mod schedule;
mod source;
mod source_review;
mod util;
mod web;

pub use config::{Config, GateConfig, OwnedInput, REQUIRED_OWNED_INPUTS, WebBrokerConfig};
#[cfg(debug_assertions)]
#[doc(hidden)]
pub use runner::run_once_without_root_guard_for_test;
pub use runner::{RunRequest, RunResult, run_once};

/// Derive the public verification key for scheduled-authorship attestations.
///
/// The root bootstrap calls this once after validating the immutable helper
/// configuration, then installs only the returned public key for the mutable
/// runtime and operator report.  The private intent key never crosses that
/// boundary.
///
/// # Errors
///
/// Returns an error when the configured private attestor key cannot be read or
/// does not satisfy the immutable steward's credential invariants.
pub fn scheduled_authorship_verifying_key_hex(config: &Config) -> Result<String> {
    Ok(attestation::HmacSigner::from_file(&config.attestor_key)?
        .scheduled_authorship_verifying_key_hex())
}

pub const CONFIG_SCHEMA: &str = "astrid.edge.steward_helper.config.v1";
pub const RECEIPT_SCHEMA: &str = "astrid.edge.scheduled_introspection.helper_receipt.v2";
pub const REFLECTION_SCHEMA: &str = "astrid.edge.scheduled_introspection.model_reflection.v2";

#[derive(Debug)]
pub struct Error(String);

impl Error {
    #[must_use]
    pub fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Self {
        Self(error.to_string())
    }
}

impl From<serde_json::Error> for Error {
    fn from(error: serde_json::Error) -> Self {
        Self(error.to_string())
    }
}

pub type Result<T> = std::result::Result<T, Error>;
