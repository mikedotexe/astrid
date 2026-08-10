# astrid-storage

[![License: MIT OR Apache-2.0](https://img.shields.io/badge/License-MIT%20OR%20Apache--2.0-blue.svg)](../../LICENSE-MIT)
[![MSRV: 1.94](https://img.shields.io/badge/MSRV-1.94-blue)](https://www.rust-lang.org)

**The persistence layer. Disk for the OS.**

An operating system needs disk. Astrid uses a raw key-value contract for
capsule and system state. The embedded implementation is SurrealKV; larger
deployments can place a compatible service behind the same contract.

## Storage model

Capsules and kernel services need fast, isolated byte storage. Keeping this
crate to that contract avoids shipping an unused query engine and its
dependency graph in every appliance.

| Deployment | KV backend |
|---|---|
| Dev / single-agent | SurrealKV (embedded LSM-tree) |
| Production / multi-node | Deployment-selected compatible KV service |

The multi-node placement path has not been deployed in production yet.

## Namespace isolation

Every KV operation is scoped to a namespace. WASM guests receive a `ScopedKvStore` bound to `wasm:{capsule_id}` and never see the raw key structure. The kernel uses `system:*` namespaces for internal state.

Internally, keys are stored as `"{namespace}\0{key}"`. The null-byte separator is the isolation boundary. Empty namespaces, empty keys, and keys containing null bytes are rejected at validation before reaching the storage engine. `SurrealKvStore` uses transactional range scans bounded by the null-byte separator, so a namespace scan is O(keys in namespace), not O(total keys).

## Secret storage

The `SecretStore` trait provides synchronous credential storage (called from synchronous Extism host functions that bridge to async via `block_on`). Three implementations:

- `KvSecretStore` stores secrets in the KV tier with a `__secret:` key prefix. Works everywhere. No OS-level encryption at rest.
- `KeychainSecretStore` (`keychain` feature) uses the OS keychain via the `keyring` crate. Per-capsule isolation via service name scoping.
- `FallbackSecretStore` (`keychain` feature) probes the keychain once at construction. If accessible, all operations go to keychain. If not, all go to KV. No per-operation fallback that could scatter secrets across both backends.

The `build_secret_store` convenience constructor picks the best available backend.

## Identity

`IdentityStore` manages users and cross-platform identity links. A Discord user, a Telegram user, and a CLI user can all resolve to the same `AstridUserId`. Platform names are normalized (case, whitespace). Path-injection characters (`/`, `\0`) in platform names, user IDs, and display names are rejected before key construction.

## Feature flags

| Feature | Enables |
|---|---|
| `kv` | `SurrealKvStore` (persistent embedded KV) |
| `keychain` | `KeychainSecretStore` + `FallbackSecretStore` |
| `full` | persistent KV support |

`MemoryKvStore` and `KvSecretStore` are always available with no feature flags.

## Usage

```toml
[dependencies]
astrid-storage = { workspace = true, features = ["full"] }
```

```rust
use std::sync::Arc;
use astrid_storage::kv::{MemoryKvStore, ScopedKvStore};

let store = Arc::new(MemoryKvStore::new());
let scoped = ScopedKvStore::new(store, "wasm:my-plugin")?;

scoped.set("config", b"{}".to_vec()).await?;
scoped.set_json("prefs", &serde_json::json!({"key": "value"})).await?;
let loaded: serde_json::Value = scoped.get_json("prefs").await?.unwrap();
```

## Development

```bash
cargo test -p astrid-storage --all-features
```

## License

Dual MIT/Apache-2.0. See [LICENSE-MIT](../../LICENSE-MIT) and [LICENSE-APACHE](../../LICENSE-APACHE).
