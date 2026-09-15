# No-action rationale — capability.rs Allow Always round

`addressed_no_action` here is evidence-backed, not a default.

The report is a **question-answering** artifact: Astrid was asked how `handle_allow_always`
mints its token and what `check_capability` requires, and she answered from an earlier window
while explicitly flagging that the supplied window could not support the answer. The steward
obligation is therefore verification, not construction.

Why no source change:

1. **Working copy equals report-bound bytes.** SHA-256
   `00e4e0f49c04179f63fa66f2fe69e084fb939bef7dfcb1e44443db7f0e93405b` matched, so the complete
   file (276 lines) verified against exactly what she reasoned about. Thirteen of fifteen
   claims are confirmed by that source as already-correct behavior; correct behavior needs no
   edit.
2. **The single wrong claim is a reading correction, not a defect.** Her TTL inference
   ("no expiration or a maximum system-defined duration") is contradicted by
   `ALLOW_ALWAYS_DEFAULT_TTL = Duration::hours(1)` (`interceptor/types.rs:6`). The code is
   self-consistent and already announces the value in its own log line (capability.rs:131).
   Nothing in the source is broken by her misreading it.
3. **Her open ask is answered by reading, not by building.** The requested
   `action_to_resource_permission` protocol/permission mapping is fully enumerable from lines
   145-190 and is tabulated in this packet's summary, including the non-uniform
   permission convention between the direct-file and capsule-file lanes.
4. **The one live question is above this round's authority.** The Persistent-scope /
   one-hour-TTL pairing (c014) is kernel approval-lifetime semantics. Altering token scope or
   lifetime is a security-semantics change in `astrid-approval`, not a flywheel steward act,
   so it is named for Mike and left untouched.

What was deliberately *not* done, and why:

- No Rust test added. A regression pinning the mapping table would be legitimate non-live work
  and is recommended below, but this round's remaining time could not fit a workspace compile
  plus the full close/integrity sequence. A half-run test is worse evidence than an honest
  omission. Recorded as a follow-up, not as debt from a failed attempt.
- No change to `ALLOW_ALWAYS_DEFAULT_TTL`, `TokenScope`, or any approval-path behavior.
- No inbox letter to Astrid. Her ask is captured as claim `c013` with a grounded disposition,
  which is the flywheel's designed consumer; a correspondence artifact was not written merely
  to create activity.
- Her stated `NEXT: SELF_STUDY OPEN ... 102` was recorded, not dispatched or pre-empted.

Recommended follow-up for an interactive window (no authority implied):

- A focused `#[cfg(test)]` regression in `crates/astrid-approval/src/interceptor/capability.rs`
  asserting the exact mapping per `SensitiveAction`, including
  `CapsuleFileAccess` yielding `Permission::Invoke` with the mode in the resource string, and
  `LiveControlMutation` yielding `None`. That would make the answer to c013 durable.
- A note to Mike on c014: whether `TokenScope::Persistent` with a one-hour TTL is the intended
  meaning of "Allow Always", since a user granting "always" may reasonably expect longer.
