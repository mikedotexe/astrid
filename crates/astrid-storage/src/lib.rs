//! Astrid Storage — unified persistence layer.
//!
//! Provides raw key-value storage for the Astrid runtime:
//!
//! # Raw Key-Value ([`KvStore`])
//!
//! Direct byte-level `get`/`set`/`delete` backed by **`SurrealKV`** — an embedded,
//! versioned, ACID-compliant LSM-tree KV store. Zero query overhead.
//!
//! Primary use case: WASM guest storage with scoped namespaces per plugin.
//!
//! Enable with the **`kv`** feature.
//!
//! # Scaling
//!
//! | Deployment | KV backend |
//! |------------|------------|
//! | Dev / single-agent | `SurrealKV` (embedded) |
//! | Production / multi-node | deployment-selected KV service |
//!
//! Scaling remains a configuration concern rather than a second in-process
//! query engine.
//!
//! # Feature Flags
//!
//! - **`kv`** — `SurrealKV` raw key-value store
//! - **`full`** — all persistent KV features

#![deny(unsafe_code)]
#![deny(missing_docs)]
#![deny(clippy::all)]
#![deny(unreachable_pub)]
#![deny(clippy::unwrap_used)]
#![cfg_attr(test, allow(clippy::unwrap_used))]

pub mod error;
pub mod identity;
pub mod kv;
pub mod secret;

pub use error::{StorageError, StorageResult};
pub use identity::{IdentityError, IdentityStore, KvIdentityStore};
pub use kv::{KvEntry, KvStore, MemoryKvStore, ScopedKvStore};
pub use secret::{KvSecretStore, SecretStore, SecretStoreError, build_secret_store};

#[cfg(feature = "keychain")]
pub use secret::{FallbackSecretStore, KeychainSecretStore};

#[cfg(feature = "kv")]
pub use kv::SurrealKvStore;
