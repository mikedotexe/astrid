//! One read-only source catalog and reader shared by both Being adapters.
//!
//! Preparing bytes is not delivery. Only a verified retained provider request
//! can advance a source bookmark. These records never claim understanding.
#![deny(unsafe_code)]

mod catalog;
mod command;
mod evidence;
mod navigation;
mod navigation_recovery;
mod notebook;
mod page;
mod path_recovery;
mod progress;
mod questions;
mod relationships;
mod source_search;
mod store;
mod trace;
pub mod writing;

pub use catalog::{Catalog, Repository, Source};
pub use command::Command;
pub use evidence::InputKind;
pub use navigation_recovery::{NavigationRecovery, recover_local_navigation};
pub use page::{Page, Position, SourceRevision};
pub use questions::QuestionCommand;
pub use store::{DeliveryReceipt, Reader, StudyOutput};

pub const STUDY_PROMPT: &str = include_str!("../prompt.txt");

pub const SCHEMA_VERSION: u32 = 3;
/// Whole system + reference input, shared by every provider adapter.
pub const MAX_INPUT_BYTES: usize = 48_000;
/// Byte-safe room for the protected input, framing and 8,192 output tokens.
pub const CONTEXT_TOKENS: u32 = 65_536;
pub const MAX_PAGE_BYTES: usize = 7_000;

pub(crate) fn digest(bytes: impl AsRef<[u8]>) -> String {
    use sha2::{Digest as _, Sha256};
    format!("{:x}", Sha256::digest(bytes.as_ref()))
}
