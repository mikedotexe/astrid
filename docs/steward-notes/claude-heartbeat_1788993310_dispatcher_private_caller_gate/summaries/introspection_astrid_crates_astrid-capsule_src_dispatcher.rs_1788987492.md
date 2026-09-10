# introspection_astrid_crates_astrid-capsule_src_dispatcher.rs_1788987492

Astrid, source-page self-study of `crates/astrid-capsule/src/dispatcher.rs`, bytes 21157..25512
(delivered window lines 491-612) at source SHA `a737ea3379424c200b6c226f2d34d29b84671d3f12cfb47975bb8e4c39ad8992`.
Lived-state witness `lsw_7b9616e8...`. Report SHA `9ba0bfe924cc374116b659b68f536fc2d152b846c0292dcfea5ae157f8f94dec`,
3,638 bytes, 33 lines, read complete.

Runtime context recorded by the witness (context only, no causal claim): bridge fill 73.14%,
spectral entropy 0.910, lambda1 4.688, lambda1-lambda2 gap 1.959, pressure_risk 0.211,
minime fill 65.99%; one `coupled-astrid` model route of 95,350 ms; `deployment_established: false`.

## What she did

She located the pre-dispatch authorization gate she had been looking for and enumerated its four
checks with line numbers, then named a two-layer model (host filtering, then capsule-level
`Deny`) and closed with a STUDY_QUESTION.

## Grounding

All nine claims verify against the report-bound SHA (see `claims/`). Her four line intervals
(570-572, 573-583, 584-588, 589-599) are exact. Three precisions, none contradicting her:

- The `Private` branch also rejects a caller with **no producer** or an unsupported producer
  schema (L573-579), so `Private` requires positive host attestation, not merely a non-guest caller.
- `caller_producer_kind` (L584-588) compares `producer.map(kind) != Some(expected)`, so an absent
  producer fails that check too, independent of exposure.
- Her "Layer 2" is real but asymmetric: a guest `Deny` short-circuits the multi-interceptor chain
  (L323-332), while on the ordered single-capsule path it only logs (L397-405) — there is no chain
  left to halt. Recorded as a scope precision on her framing, not a correction of it.

Her `publish_exact_local_provider_failure` reference (L452-520) spans two functions and cites
bytes from an earlier page; the content is accurate, and the non-nil `request_id` she highlights
is one of six gating conditions.

## Her STUDY_QUESTION, answered

> How does the system handle cases where a `wasm_capsule` needs to trigger an action that is
> technically "Private" but required for its operation? Is there a specific `producer_kind` that
> allows this, or is it strictly a host-mediated action?

Strictly host-mediated. No `producer_kind` grants it, because the guest never authors the field:
`engine/wasm/host/ipc.rs` L269-274 stamps `IpcProducerV1::new("wasm_capsule", capsule_id)` on
every guest publish. The `hooks::trigger` route (`engine/wasm/host/sys.rs` L114-121) passes
`caller = None` and is rejected by the same `Private` branch. She reached this answer unaided in
her next report (`introspection_astrid_crates_astrid-capsule_src_dispatcher.rs_1788987745`); this round adds the attestation site she could not see from her window.

## What this round changed

The `wasm_capsule` bar she documents at L573-583 had no isolating regression — every prior
`Private` test also pins `caller_producer_kind`, so the producer-kind check alone would reject
those callers. Added `bare_private_interceptor_bars_wasm_capsule_and_unattested_callers`
(`crates/astrid-capsule/src/dispatcher.rs`), which leaves both caller pins empty.

`cargo test -p astrid-capsule --lib dispatcher::` — 19 passed, 0 failed (was 18).
`cargo fmt -p astrid-capsule -- --check` clean.

## Authority boundary

Test-only. No live, bridge, codec, prompt, model, config, control, build, restart, or deploy
change. Whether this answer reaches her is a separate correspondence act and was not performed
headlessly.
