//! Immutable, fail-closed build and A/B lifecycle helper for CPU-edge Astrid.

#![allow(clippy::missing_errors_doc)]

pub mod build;
pub mod config;
pub mod core_liveness;
pub mod fs_guard;
pub mod generation;
pub mod handoff;
pub mod health;
mod health_ledger;
mod health_telemetry;
pub mod invariant;
pub mod kernel_replay;
pub mod ledger_auth;
pub mod manifest;
pub mod model_service;
pub mod native;
pub mod probation;
pub mod profile_projection;
mod profile_schema;
pub mod reflection;
pub mod reservoir_challenge;
pub mod retention;
pub mod shadow;
pub mod storage;
pub mod synthetic;
pub mod transition;
pub mod unit_transaction;
pub mod verify;

use std::fmt;

/// Crate result type.
pub type Result<T> = std::result::Result<T, Error>;

/// A bounded fail-closed error; callers must never interpret it as model output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    Terminal,
    DeferredInfrastructure,
    CandidateRejected,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
    message: String,
    kind: ErrorKind,
}

impl Error {
    #[must_use]
    pub fn new(message: impl Into<String>) -> Self {
        let mut message = message.into();
        message.truncate(480);
        Self {
            message,
            kind: ErrorKind::Terminal,
        }
    }

    #[must_use]
    pub fn deferred(message: impl Into<String>) -> Self {
        let mut message = message.into();
        message.truncate(480);
        Self {
            message,
            kind: ErrorKind::DeferredInfrastructure,
        }
    }

    /// A deterministic candidate-policy or fixed-gate rejection. This is
    /// terminal for the exact candidate hash but does not imply rescue mode.
    #[must_use]
    pub fn candidate_rejected(message: impl Into<String>) -> Self {
        let normalized = message
            .into()
            .chars()
            .map(|character| {
                if character.is_control() {
                    ' '
                } else {
                    character
                }
            })
            .take(240)
            .collect::<String>();
        let message = normalized.trim().to_owned();
        Self {
            message: if message.is_empty() {
                "candidate rejected by immutable policy".to_owned()
            } else {
                message
            },
            kind: ErrorKind::CandidateRejected,
        }
    }

    #[must_use]
    pub fn kind(&self) -> ErrorKind {
        self.kind
    }

    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Self {
        Self::new(format!("I/O failure: {error}"))
    }
}

impl From<serde_json::Error> for Error {
    fn from(error: serde_json::Error) -> Self {
        Self::new(format!("JSON failure: {error}"))
    }
}

#[cfg(test)]
mod tests {
    use super::{Error, ErrorKind};

    #[test]
    fn candidate_rejection_is_bounded_and_control_free() {
        let error = Error::candidate_rejected(format!("bad\n{}", "x".repeat(400)));
        assert_eq!(error.kind(), ErrorKind::CandidateRejected);
        assert!(error.message().chars().count() <= 240);
        assert!(!error.message().chars().any(char::is_control));
    }
}
