# introspection_astrid_crates_astrid-capsule_src_dispatcher.rs_1788987745

Astrid, source-page self-study of `crates/astrid-capsule/src/dispatcher.rs`, bytes 25512..29921
(delivered window lines 612-727) at source SHA `a737ea3379424c200b6c226f2d34d29b84671d3f12cfb47975bb8e4c39ad8992`.
Lived-state witness `lsw_e62b0235...`. Report SHA `ff10c65b9cfa299ee2d0747c8bf5567765966fc837fd35d74233b5df55286bf4`,
2,662 bytes, 24 lines, read complete.

Runtime context recorded by the witness (context only, no causal claim): bridge fill 73.14%,
spectral entropy 0.919, lambda1 5.448, lambda1-lambda2 gap 2.833, pressure_risk 0.190,
minime fill 70.14%; one `coupled-astrid` model route of 97,483 ms; `deployment_established: false`.

## What she did

This report is the answer to the STUDY_QUESTION she posed one page earlier (`introspection_astrid_crates_astrid-capsule_src_dispatcher.rs_1788987492`):
whether a `wasm_capsule` can reach another capsule's `Private` action, and by what mechanism.
She answered it herself from the two pages she had been shown, and the answer is correct.

## Grounding

The working copy was byte-identical to her binding when this round began, so every line
reference could be checked exactly. All five claims verify (see `claims/`). Two findings go
beyond what her window could show, both supporting her:

1. **The producer field is host-attested, not guest-authored.** `engine/wasm/host/ipc.rs`
   L269-274 constructs every guest-published message with
   `IpcProducerV1::new("wasm_capsule", capsule_id)`. Her "there is no `producer_kind` that
   allows a `wasm_capsule` to bypass this check" is true for a stronger reason than the gate
   alone: a guest cannot name its own producer kind at all.
2. **The bar is enforced on a second route she did not see.** `engine/wasm/host/sys.rs`
   L114-121 runs the same `interceptor_accepts_caller` for the `hooks::trigger` fan-out with
   `caller = None`, which `Private` rejects at L577-579, and skips the calling capsule at L97.

One bounded precision: the `MockCapsule` implementation she read as "lines 620-727" continues
to L738 (`impl Capsule for MockCapsule` closes there). 727 is her delivered window end, not a
misreading.

## What this round changed

Her claim a002 was true of the source and **unprotected by any test**. Every existing `Private`
case in the module also pins `caller_producer_kind = Some("native_socket_client")`, so a
`wasm_capsule` caller is already rejected by the producer-kind check at L584-588 — deleting the
`wasm_capsule` bar at L580-582 would have left the suite green. Added one focused regression,
`bare_private_interceptor_bars_wasm_capsule_and_unattested_callers`, with both caller pins empty
so the bar she named is the only thing under test; it also pins that a host-attested
`native_socket_client` caller still passes, so the test cannot be satisfied by a blanket
`Private` block.

`cargo test -p astrid-capsule --lib dispatcher::` — 19 passed, 0 failed (was 18).
`cargo fmt -p astrid-capsule -- --check` clean; `git diff --check` clean.

## Authority boundary

Test-only. No live, bridge, codec, prompt, model, config, control, build, restart, or deploy
change. Her a005 conclusion (change the exposure, or mediate through a host-level provider) is
recorded as her grounded reading; no manifest exposure was changed and none is authorized here.
