//! Immutable boundary for active-generation report presentation.
//!
//! Candidate report code is deliberately treated as untrusted.  The broker
//! selects one fixed report entrypoint, supplies a root-authored bounded data
//! projection, caps execution/output, and returns a self-hashed envelope that
//! is bound to the active generation and exact script bytes.  The envelope is
//! presentation only and grants no operator or deployment authority.

#![deny(unsafe_code)]

mod client;
mod config;
mod contract;
mod fs_guard;
mod runner;

pub use client::{ClientFormat, ClientOptions, run_client};
pub use config::{BrokerConfig, BrokerPolicy, TrustedExecutable};
pub use contract::{
    BrokerRequest, CandidatePresentation, PresentationEnvelope, PresentationSection,
    PresentationStatus, PresentationView, ProjectionInput,
};
pub use runner::{Broker, HostSecurityState, SystemClock};

/// Stable error type kept intentionally small for a standalone rescue-root
/// binary.
#[derive(Debug)]
pub struct Error(String);

impl Error {
    pub(crate) fn new(message: impl Into<String>) -> Self {
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
    fn from(value: std::io::Error) -> Self {
        Self::new(value.to_string())
    }
}

impl From<serde_json::Error> for Error {
    fn from(value: serde_json::Error) -> Self {
        Self::new(value.to_string())
    }
}

pub type Result<T> = std::result::Result<T, Error>;
