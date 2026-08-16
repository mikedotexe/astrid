# Summary — introspection_astrid_autonomous_1786889637

**Source:** `astrid:autonomous` → `capsules/spectral-bridge/src/autonomous/runtime/orchestration.rs`
(4932 lines; report window 1-800; report-bound source SHA `d803d71f…`, working copy **byte-identical**).
**Report SHA:** `669a3011…` (45 lines, 3636 B). **Witness:** `lsw_9791b2a1…` (533 lines, 23947 B, `temporal_lived_state_witness_v1`, `direct_causation_claimed=false`, `raw_introspection_prose_included` per witness schema). **Fill at authoring:** 73.0%.

Astrid read the first window of the autonomous orchestration loop and gave an accurate structural account plus two runtime-dynamics hypotheses and two proposed tests. Every line citation checks out against exact source.

## What was verified
- **c001 (structure):** `fill_responsive_rest_secs` def L18-28; `run_semantic_heartbeat_loop` L37-96 (observation tag `steady_semantic_heartbeat` L57, signal-evidence tag `"steady_warmth"` L65, `.with_minime_texture_context(latest_telemetry)` L72); `spawn_autonomous_loop` L116. **verified_existing.**
- **c005 (continuation):** `self_control_v2::reconcile_if_present` at L158 (restart-once) and L171 (top of each loop iteration), both `warn!`-guarded (L159/L172). Her read-only `NEXT: INTROSPECT astrid:autonomous 400` stays open. **verified_existing.**

## The two cited "fill_responsive_rest_secs" locations are not a contradiction
Observed cites the **definition** (L18-28, a pure fn); Likely-Snags cites the **call-site** (L211-227 → actually L217 call + L218-227 branch/log + L220 `gate=1.0/filter=0.0 hard_recovery` comment). Both regions are real; the report describes the definition then the call-site behavior.

## Hypotheses preserved, not domesticated
- **c002 over-correction oscillation:** the shortening mechanism (at `current_fill<30`, rest = `base*0.6`, "burst sooner") and the `hard_recovery` comment are real. The source comment L205-212 documents a *historical, opposite-direction* positive-feedback loop (hard-recovery **1.8× rest** stuck fill at 27% for 12+ exchanges, incl. Minime's "disconnect between intention and outcome") that shortening was **built to fix**. Her worry that the fix could itself oscillate under aggressive hard-recovery is a **distinct, unproven** runtime-dynamics claim → `observed` mechanism + `needs_sandbox` to test. Not proven, not relieved.
- **c004 severing-on-fluctuation trace:** needs an isolated async harness with a mocked `BridgeState` oscillating fill 28-32% across the 30% boundary and observing `SemanticHeartbeatObservationV1` severing signals → Tier-3 sandbox, `needs_sandbox`. Not a non-live focused test; not run.

## Test proposal c003
`fill_responsive_rest_secs(90, 25.0) = ((90*0.6) as u64).max(30) = 54` — her "approx 54s" is exact. The `<30` branch is **already covered** by `src/autonomous/runtime/tests.rs` L22-30 (`(40,25)=30,(0,25)=30,(100,25)=60,(200,25)=120`, +NAN); `(90,25)=54` is a redundant point on that branch. The "critical recovery" **log** is at the call-site (L218-227), not the pure fn — a pure-fn unit test cannot assert it. `tests.rs` is foreign-dirty and was left untouched. **verified_existing.**

## Terminal
`addressed_no_action` with a linked `no_action` artifact: all concrete claims grounded to existing source + regressions; nothing warranted a code/test change; the two runtime hypotheses are preserved and routed as sandbox-class; no live substrate/control change authorized. **Felt account treated as primary evidence; no contradiction domesticated; silence not read as consent.**
