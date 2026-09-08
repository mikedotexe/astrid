# introspection_minime_regulator_1788542062

Source: `capsules/spectral-bridge/workspace/introspections/introspection_minime_regulator_1788542062.txt`
Report SHA-256: `6efb80dcb2054c1c194c0b023cdc90d23c9af51aa4377ac0a8c94285cddaa116` (3445 bytes, 45 lines, read complete)
Lived-state witness: `lsw_56b869488cd754030e62d6a60e890a3010d72ad1ebd3c2725a1a7e85fdb7b64a` (23777 bytes, 533 lines, read complete)
Report-bound source: `minime/src/regulator/core.rs` @ `46828f4c813eb88aae30212793f698285c696c108dd405604ffb6b5129827d97` — working copy **matches**; identical bytes to the family head, so source verification is shared per the family-batch rule.

Family: member of the `minime:regulator` / `lines1-24of24` family headed by
`introspection_minime_regulator_1788849273` (similarity 0.445, 31 variant terms recomputed
locally, all addressed below).

## What is the same as the head

The inclusion-shell reading, the nine includes at L16-24, the dual-control header
(verbatim including the "token-bucket" qualifier), and the "Ghost of Implementation" risk.
All verified against the complete source; the same line-range precision applies (comment
block L1-7 and L9-13; L8 is the `#![allow(dead_code)]` inner attribute).

## What is distinct in this report — and how each variant was addressed

1. **The sharpest framing in the family** (`distinction`, `body`, `organization`,
   `matter`, `between`, `however`, `treated`, `causal`, `mechanics`, `purposes`,
   `introspection`, `persists`). She writes that the shell/body distinction "is a matter of
   source organization rather than a namespace boundary; however, for introspection
   purposes, the shell must not be treated as the source of causal mechanics."

   **Correct as stated** — between `core.rs` and its included files there is genuinely no
   namespace step, because `include!` expands tokens in place. One *addition*, not a
   correction: a real namespace boundary exists one level up. `src/regulator.rs` L3-4
   mounts the shell as a **private** module (`#[path = "regulator/core.rs"] mod core;`) and
   L6 re-exports it (`pub use core::*;`). So her organization-not-boundary claim holds
   inside the shell, while reachability at `regulator::` rests on that glob.

2. **"Those elements actually reside within the included submodules"** (`actually`,
   `reside`, `elements`, `those`, `active`, `content`, `submodules`). Verified by locating
   them rather than asserting it: viscosity weighting in `core/viscosity.rs` (21941 bytes),
   pressure thresholds in `core/pressure_types.rs` (14874 bytes) and
   `core/pressure_source.rs`, PI state in `core/pi.rs` (20458 bytes). None appears in the
   940-byte shell.

3. **"This is a build-time invariant; if the project compiles, the inclusion is
   successful"** (`build`, `time`, `invariant`, `compiles`, `project`, `successful`,
   `note`). Implemented in exactly the form her note prescribes: the new test names
   rate_gate items via `minime::regulator::…` from outside the crate, so the assertion is
   carried by compilation. Its runtime asserts are kept structural (shape, not tuning),
   which is the same boundary she is drawing.

4. **`core/pi.rs` at L23 named as the integrity example** (`inlines`). Covered: `pi.rs` is
   one of the nine entries in the ordered include pin, so her exact example fails loudly if
   it is ever added or removed. Implemented as the ordered list rather than a whole-file
   hash, because a hash would also fire on a comment edit — not the drift she named.

5. **Suggested Next: deep-dive INTROSPECT on `core/viscosity.rs`,** because "core.rs
   provides no details on these calculations." The premise is verified — the shell carries
   no viscosity detail at all, so her reason for moving on is accurate. Recorded, not acted
   on; the file exists (21941 bytes) and is included at L17.

## Witness context (evidence only, no mechanism claim)

Fill 66.0%, `lambda1` 4.728, `lambda1-lambda2` gap 1.652, spectral entropy 0.898,
pressure_risk 0.237, minime fill 73.3%. `astrid_shadow.field_norm_delta` is **negative**
(-0.0148) here, unlike the other two family members; recorded as an observation with no
causal or felt inference. Two `gemma4_12b` MLX routes, the second carrying
`repair_parent_call_id` of the first (82.0 s then 45.6 s).
`artifact_authority_state_v1.state = evidence_only`, `live_eligible_now = false`.

## Implementation and authority boundary

Same non-live change as the family head: `minime/tests/regulator_inclusion_shell.rs`, 4
tests, all passing. No runtime, control, parameter, deploy, or restart change; none
attempted, none required. Build-time reachability is not activation evidence.
