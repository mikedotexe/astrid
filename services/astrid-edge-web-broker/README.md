# Astrid edge web broker

Immutable, per-client Unix-socket public search and bounded readable-source broker for CPU-edge
scheduled introspection and sovereign research. Runtime and steward use different systemd-owned
sockets, request keys, processes, and capacity pools. See `CONTRACT.md` for the complete authority
and systemd contract.

Search admission is durably recorded before egress in separate owner-only, body-free hash-chain
ledgers. The immutable runtime ceiling is 8 searches per rolling hour and 24 per UTC day; the
steward ceiling is 2 per hour and 12 per UTC day, with no more than two searches per trace.
Restart, replay, or switching client sockets cannot reset or pool those budgets. Search text and
results remain untrusted evidence and grant no Action, build, or activation authority.

```bash
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
```

Production builds use the pinned `Cargo.lock` and the CPU-edge signed vendor store with
`--offline --locked`.
