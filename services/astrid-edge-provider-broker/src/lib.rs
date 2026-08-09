#![deny(unsafe_code)]

mod auth;
mod config;
mod http;
mod receipt;
mod server;
mod upstream;

pub use config::Config;
pub use server::{run, run_warmup_client};

use std::fmt;

pub const CONFIG_SCHEMA: &str = "astrid.edge.provider_broker.config.v1";
pub const INFERENCE_PATH: &str = "/v1/chat/completions";
pub const WARMUP_PATH: &str = "/internal/warmup";
pub const UNLOAD_PATH: &str = "/internal/unload";
pub const BROKER_AUTHORITY: &str = "astrid-edge-provider";

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub struct Error(String);

impl Error {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    #[must_use]
    pub fn code(&self) -> &'static str {
        if self.0.contains("busy") || self.0.contains("quota") {
            "busy"
        } else if self.0.contains("authentication") || self.0.contains("peer") {
            "unauthorized"
        } else if self.0.contains("maintenance") || self.0.contains("reflection") {
            "lease_denied"
        } else if self.0.contains("upstream") {
            "upstream_error"
        } else {
            "invalid_request"
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
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
