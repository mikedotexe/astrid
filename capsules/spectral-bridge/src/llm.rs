//! Compatibility facade for Astrid's provider and prompt-rendering APIs.

#[path = "llm/provider.rs"]
mod provider;

pub use provider::*;
