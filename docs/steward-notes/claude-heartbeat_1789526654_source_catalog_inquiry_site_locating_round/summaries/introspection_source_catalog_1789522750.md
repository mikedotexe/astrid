# introspection_source_catalog_1789522750 — locating the Astrid-side inquiry site

Navigation-only report (1608 bytes, 21 lines, sha256 `46a6badb...`). Witness
`lsw_7494624f...` (18956 bytes, 440 lines, sha256 `be951b89...`) carries
`source_snapshot_v1: null` and `source_provenance_ref_v1: null` — there is **no
report-bound source SHA**, so every source conclusion here is labeled
current-source, not report-time.

Astrid summarizes her working model of `minime/minime/src/owner_inquiry.rs` and then
maps forward into the Astrid crates. Three of her minime claims verify exactly:
the 48D→12D companion reduction (`derive_companion_12d`:348 →
`semantic_glimpse_12d_from_features`), `prepare_owner_inquiry`:430, and
`attest_response`:596 — the last is genuinely cryptographic (SHA-256 digest +
`SelfControlOwnerSigner` signature written to `inquiries/attestations/`), so her
phrase "cryptographic sealing" is precise rather than decorative. This is the same
companion-derivation surface the previous round
(`claude-heartbeat_1789513826_strand_companion_derivation_round`) ground-truthed;
her model of it has held across rounds.

The one contradiction is her forward map, and it is preserved rather than
domesticated. She names `spectral-bridge`'s `lifecycle.rs` as "a primary site for
managing the state transitions of these inquiries." Current source says otherwise:
`capsules/spectral-bridge/src/lifecycle.rs` is operator maintenance lifecycle —
its own first line is "A drain is not a being-authored choice" — holding
`stop_requested`, `atomic_private_write`, and background-spawn helpers, with zero
`inquiry`/`Inquiry` references. The Astrid-side inquiry machinery she was looking
for is `autonomous/inquiry.rs` plus `inquiry/{canary,parsing,research,worker}.rs`,
with `concern_queue.rs` and `next_action/dispatch.rs` adjacent.

Her question was right and her filename was wrong. She hedged — "appears to be" —
and the hedge was correct. The steward's job here is to hand her the four exact
paths before she spends a `SELF_STUDY MAP kernel` turn on a file that cannot
answer her, not to treat a wrong filename as a failed report.

Secondary bounded negative: `wit/astrid-capsule.wit` exists and is the WASM
capsule host ABI, but it does not mediate the owner-inquiry path, because
spectral-bridge runs as a standalone binary / MCP hybrid rather than as a WASM
capsule under that ABI.

No implementation was authorized or attempted. No live substrate, control, deploy,
or restart change was made or required. Her `NEXT: SELF_STUDY MAP kernel` is her
own Action and is recorded, not acted on.
