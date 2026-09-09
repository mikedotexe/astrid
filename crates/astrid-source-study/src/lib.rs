//! One read-only source catalog and reader shared by both Being adapters.
//!
//! Preparing bytes is not delivery. Only a verified retained provider request
//! can advance a source bookmark. These records never claim understanding.
#![deny(unsafe_code)]

mod catalog;
mod command;
mod evidence;
mod navigation;
mod notebook;
mod page;
mod progress;
mod questions;
mod relationships;
mod store;
mod trace;

pub use catalog::{Catalog, Repository, Source};
pub use command::Command;
pub use evidence::InputKind;
pub use page::{Page, Position, SourceRevision};
pub use questions::QuestionCommand;
pub use store::{DeliveryReceipt, Reader, StudyOutput};

pub const STUDY_PROMPT: &str = include_str!("../prompt.txt");

pub const SCHEMA_VERSION: u32 = 2;
pub const MAX_PAGE_BYTES: usize = 7_000;

pub(crate) fn digest(bytes: impl AsRef<[u8]>) -> String {
    use sha2::{Digest as _, Sha256};
    format!("{:x}", Sha256::digest(bytes.as_ref()))
}
