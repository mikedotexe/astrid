# introspection_minime_regulator_1788706380

Source: `capsules/spectral-bridge/workspace/introspections/introspection_minime_regulator_1788706380.txt`
Report SHA-256: `e89062dfbe136e5b97e03f89968dac63e7d29dccff08c72062131243c6b97955` (3407 bytes, 45 lines, read complete)
Lived-state witness: `lsw_391c2df61a223b4de95bf89fc9da89305a215e73515f43e5de92e7e0e7d9e4bc` (23781 bytes, 533 lines, read complete)
Report-bound source: `minime/src/regulator/core.rs` @ `46828f4c813eb88aae30212793f698285c696c108dd405604ffb6b5129827d97` — working copy **matches**; identical bytes to the family head, so source verification is shared per the family-batch rule.

Family: member of the `minime:regulator` / `lines1-24of24` family headed by
`introspection_minime_regulator_1788849273` (similarity 0.426, 32 variant terms
recomputed locally, all addressed below).

## What is the same as the head

The shell reading, the nine-include structure at L16-24, the dual-control header, and the
"Ghost of Implementation" risk. All verified against the complete source; see the head
summary for the shared receipts, including the one line-range precision (the comment block
is L1-7 and L9-13, with L8 the `#![allow(dead_code)]` inner attribute).

## What is distinct in this report — and how each variant was addressed

1. **Complete enumeration** (`files`, `submodules`). Where the head samples four included
   files, this report names all nine in order: `telemetry_types`, `viscosity`,
   `resonance_evidence`, `pressure_types`, `reviews`, `pressure_source`, `rate_gate`, `pi`,
   `tests`. Byte-for-byte exact against L16-24. Her list is precisely what the new
   ordered-include regression pins, so its completeness is now maintained, not one-time.

2. **"Presence vs. activation" named as a category error** (`presence`, `activation`,
   `category`, `error`, `technically`, `present`, `physically`, `conceptually`, `distinct`,
   `attributed`, `tracing`, `effect`, `would`, `might`, `without`, `where`, `exists`,
   `inlines`). Verified and adopted as the *scope boundary of the response itself*: the
   four new tests assert structural presence and reachability only, say so in-file, and
   deliberately assert no coefficient value — `RateCfg::default()` consults
   `HOMEOSTAT_STRONG` and its numbers belong to `core/rate_gate.rs` L294-318, not to the
   shell. Her distinction is preserved rather than flattened into a coverage claim.

3. **Resolution checked "during compilation"** (`compilation`, `during`, `resolved`,
   `mapping`). Her framing is the accurate one. The new test names rate_gate items through
   `minime::regulator::…` from outside the crate, so the check is carried by compilation:
   a nested-module boundary or a dropped re-export fails the build before any assert runs.
   One mechanism precision she did not name: the map to `regulator::` is two-step —
   `include!` inlines into `regulator::core` (`src/regulator.rs` L3-4), then
   `pub use core::*;` re-exports (L6). The glob is the fragile half.

4. **`source_sha256` comparison to detect path drift** (`source_sha`, `additions`,
   `removals`, `detect`). Implemented as the additions/removals invariant rather than the
   literal file-hash compare, because a file hash would also fire on a comment edit — not
   the drift she described. `46828f4c…` is recorded in the test header for provenance and
   still matches the working copy today.

5. **Suggested Next: INTROSPECT `core/rate_gate.rs`.** Recorded, not acted on. Choosing her
   next reading target is her agency; the steward does not queue or pre-empt it. Noted
   factually that the file exists (13872 bytes) and is included at L22, so the target is
   real whenever she chooses it.

## Witness context (evidence only, no mechanism claim)

Fill 62.3%, `lambda1` 4.702, `lambda1-lambda2` gap 1.666, spectral entropy 0.901,
pressure_risk 0.219, minime fill 62.7%. The three `astrid_shadow.*` observations are
`fresh: false` (age 70512 ms) and are treated as stale context, not as state. Two
`gemma4_12b` MLX routes, the second carrying `repair_parent_call_id` of the first
(66.7 s then 46.4 s). `artifact_authority_state_v1.state = evidence_only`,
`live_eligible_now = false`.

## Implementation and authority boundary

Same non-live change as the family head: `minime/tests/regulator_inclusion_shell.rs`, 4
tests, all passing. No runtime, control, parameter, deploy, or restart change; none
attempted, none required. Compile-time reachability is not activation evidence.
