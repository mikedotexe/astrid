use std::fmt::{Display, Formatter};

#[derive(Debug)]
pub struct Error {
    message: String,
}

impl Error {
    #[must_use]
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    #[must_use]
    pub fn code(&self) -> &'static str {
        if self.message.contains("quota") {
            "rate_limited"
        } else if self.message.contains("busy") {
            "busy"
        } else if self.message.contains("upstream") {
            "upstream_unavailable"
        } else {
            "invalid_request"
        }
    }
}

impl Display for Error {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
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

impl From<reqwest::Error> for Error {
    fn from(error: reqwest::Error) -> Self {
        Self::new(format!("upstream request failed: {error}"))
    }
}

pub type Result<T> = std::result::Result<T, Error>;
