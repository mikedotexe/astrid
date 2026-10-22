//! Shared transport foundation. No live adapter is changed by this crate.
//!
//! The operator selects trusted configuration; request authors supply only a
//! public query or page URL. The current signed policy/state/token are reread
//! during work. Fetching is not model execution or live-substrate authority.
#![deny(unsafe_code)]

mod authority;
mod guard;
mod network;
mod operator;
mod transport;

use std::path::PathBuf;

use astrid_minime_protocol::public_web::{PublicWebOperationV2, PublicWebOwnerV2};
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};

pub use authority::{PublicWebPreview, preview};
pub use operator::{
    PublicWebBundle, PublicWebMutationPreview, PublicWebStatus, load_approval, mutate,
    preview_mutation, status,
};
pub use transport::fetch;

pub const CONFIG_SCHEMA: &str = "astrid.public_web.config.v2";
pub const REQUEST_SCHEMA: &str = "astrid.public_web.request.v2";
pub const RECEIPT_SCHEMA: &str = "astrid.public_web.receipt.v2";

/// Operator-selected paths, not model-controlled request arguments. The
/// delegation root is the existing `delegated_capabilities` directory.
#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PublicWebConfig {
    pub schema: String,
    pub delegation_root: PathBuf,
    pub policy_path: PathBuf,
    pub state_path: PathBuf,
    pub state_floor_path: PathBuf,
    pub owner_lock_dir: PathBuf,
    pub storage_path: PathBuf,
    pub expected_owner: PublicWebOwnerV2,
    pub expected_policy_sha256: String,
    pub minimum_state_revision: u64,
}

impl PublicWebConfig {
    #[must_use]
    pub fn sha256(&self) -> String {
        digest(&serde_json::to_vec(self).unwrap_or_default())
    }
    /// Load an operator-owned configuration without following symlinks.
    ///
    /// # Errors
    /// Rejects missing, public-readable, oversized, or malformed configuration.
    pub fn load(path: &std::path::Path) -> std::result::Result<Self, PublicWebErrorCode> {
        serde_json::from_slice(&guard::read_private(path)?)
            .map_err(|_| PublicWebErrorCode::InvalidConfig)
    }
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PublicWebRequest {
    pub schema: String,
    pub operation: PublicWebOperationV2,
    /// A literal search query, or an HTTP(S) page URL. Never a shell command.
    pub target: String,
}

impl PublicWebRequest {
    #[must_use]
    pub fn sha256(&self) -> String {
        digest(&serde_json::to_vec(self).unwrap_or_default())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PublicWebErrorCode {
    InvalidRequest,
    InvalidConfig,
    AuthorityUnavailable,
    UntrustedIssuer,
    InvalidCapability,
    InvalidPolicy,
    OwnerMismatch,
    InvalidState,
    StatePending,
    InvalidApproval,
    StaleApproval,
    PublicationFailed,
    Revoked,
    OperationNotGranted,
    AuthorityChanged,
    UnsafeFilesystem,
    OwnerBusy,
    StorageHeadroom,
    UnsafeUrl,
    UnsafeAddress,
    DnsFailed,
    TransportFailed,
    Deadline,
    BodyLimit,
    IncompleteBody,
    UnsupportedEncoding,
    UnsupportedMedia,
    InvalidHeaders,
    RedirectLimit,
    InvalidRedirect,
    HttpStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublicWebFailure {
    pub schema: String,
    pub complete: bool,
    pub code: PublicWebErrorCode,
    pub request_sha256: String,
    pub decoded_bytes_observed: u64,
    pub wire_bytes_observed: u64,
    pub redirects_followed: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublicWebReceipt {
    pub schema: String,
    pub complete: bool,
    pub request_sha256: String,
    pub policy_sha256: String,
    pub owner_sha256: String,
    pub final_url_sha256: String,
    pub body_sha256: String,
    pub decoded_bytes_observed: u64,
    pub wire_bytes_observed: u64,
    pub redirects_followed: u8,
    pub http_status: u16,
}

/// Payload-bearing response, distinct from the content-free receipt.
#[derive(Serialize, Deserialize)]
pub struct PublicWebResponse {
    pub final_url: String,
    pub content_type: String,
    pub body: Vec<u8>,
    pub receipt: PublicWebReceipt,
}

pub(crate) type Result<T> = std::result::Result<T, PublicWebErrorCode>;

pub(crate) fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[cfg(test)]
mod tests;
