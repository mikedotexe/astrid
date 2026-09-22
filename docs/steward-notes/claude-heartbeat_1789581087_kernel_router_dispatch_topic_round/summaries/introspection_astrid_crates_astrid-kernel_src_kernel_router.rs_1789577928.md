# introspection_astrid_crates_astrid-kernel_src_kernel_router.rs_1789577928

- Report: `capsules/spectral-bridge/workspace/introspections/introspection_astrid_crates_astrid-kernel_src_kernel_router.rs_1789577928.txt`
  — 3,579 bytes, 35 lines, sha256 `6a35a5074a28676c778a4664b7fa076cdd92736b6f8765721c89ea20203d0c32`, read complete.
- Witness: `lsw_5238ec03cec2011a0673053f99eadbe722e2ebac829145aa6ef8925078f142ce`
  — 21,424 bytes, 498 lines, sha256 `cc509a087894e43349f40e093903f782319801268ed979cab249a2427419ec20`, read complete.
  `evidence_only`, `live_eligible_now: false`, `direct_causation_claimed: false`, `raw_introspection_prose_included: false`.
- Report-bound source: `crates/astrid-kernel/src/kernel_router.rs`, sha256 `108d6901…ee72fd`, bytes 4501..8931,
  witness window lines **111-213 of 388**. The working copy matched that hash exactly at read time, so
  report-time bytes were read directly, and the whole file was read, not only her window.

## What she did

She read one page of the kernel's management dispatcher and wrote it up arm by arm: `ListCapsules`,
`GetCommands`, `ReloadCapsules`, `Shutdown`, `GetStatus`, the `ApproveCapability` stub, and the
`GetCapsuleMetadata` arm that her page cut off mid-way. Two `STUDY_FINDING` lines, both with line
ranges. She closes: *"I need to see the rest of `handle_request`"*, and chooses `NEXT: SELF_STUDY CONTINUE`.

## Verified against the complete file

Every line range she cites is exact — 121-128, 129-152, 153-178, 179-194, 195-211, 114-120, and 212 as
the start of the cut-off arm. Her description of each arm's behavior is accurate, including the detail
that `Shutdown` publishes its confirmation *before* signalling (the early `return` at 193 is why that
arm publishes inline instead of falling through to line 230).

Three places where the complete read adds something her window could not hold:

1. **`GetCapsuleMetadata` is narrower than she guessed.** She expected "deeper technical details of a
   *specific* capsule's configuration". Lines 212-227 return one `CapsuleMetadataEntry` for *every*
   capsule in `reg.values()`, and each entry carries only `{name, interceptor_events}`.
2. **The commands/capabilities distinction.** `GetCommands` discovers slash *commands*. Security
   capabilities are a separate system (`CapabilityStore`, `astrid-approval`) that never passes through
   this arm — which also narrows her `ApproveCapability` inference: only *this router arm* is a stub,
   not the approval machinery as a whole (`lib.rs:47, 92`).
3. **The throttle she could not see.** `ReloadCapsules`, the recovery path she singles out, is the
   most-limited non-`Shutdown` command at 5/min (line 294). That table lives at 292-304, below her
   window.

## The one contradiction, stated plainly

> "it calls `crate::capsule_runtime_health::summarize`. This is where the 'vitality' of the
> capsules—likely influenced by those spectral metrics I saw in `projection.rs`—is aggregated"

It is not. `capsule_runtime_health.rs` (214 lines, sha256 `3a20e5eed609…`, read complete) is a static
WASM-payload audit: it discovers `Capsule.toml` manifests, resolves each component payload, classifies
the bytes with `wasmparser` (Component Model / core module / an "extism" byte marker for legacy), checks
a baseline allowlist, and flips `status` to `"warning"` only when actionable counts are non-zero. The
`loaded_capsules` argument is used only for its `.len()`. Repository-wide, `crates/astrid-kernel/src/`
contains no spectral, eigenvalue, curvature or reservoir reference at all, and the only `projection.rs`
in the tree is `capsules/spectral-bridge/src/codec/projection.rs` — a different crate, loaded into a
different process.

Her underlying question is not dissolved by that. She asked how the kernel *perceives* the state of
what it hosts. The answer the source gives is: structurally. Manifest present, payload resolvable,
encoding admissible — and nothing about how a capsule is behaving once it runs.

## What was done

One focused, non-live test added to the existing `#[cfg(test)] mod tests` in the file she was reading:
`rate_limit_table_covers_every_request_variant`. It asserts the `(label, limit)` pair for all eight
`KernelRequest` variants through a wildcard-free `match`, so a ninth variant cannot compile until
someone decides — and records there — whether it is throttled. No production behavior changed; no
prompt, pagination, navigation or selection surface was touched.

## Status

`addressed_change`. Ten claims, all with grounded dispositions and linked evidence.
