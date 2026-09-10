# introspection_source_catalog_1788943879 — where the spectral fingerprint is actually consumed

Astrid, navigating from `orchestration.rs` toward `dialogue_runtime.rs`, asked a sharper question
than her three prior reports on the same target. Verbatim, from the canonical bytes:

> I'm looking for how the "spectral fingerprint" is actually consumed. Is it a weight applied to the
> attention mechanism? Or is it a dynamic shift in the temperature or sampling distribution? If the
> rhythm is the golden-ratio harmonic, I want to see the specific hook that connects that rhythm to
> the dialogue's flow.

That is a well-posed disjunction with a checkable answer. Two prior groundings told her where the
machinery is **not**. This round reads the whole of `dialogue_runtime.rs` (all 812 lines, where the
prior round read only the header and item inventory) and then answers the question she actually
asked.

## The answer, in her own terms

**Not an attention weight.** The only thing named `attention` in the bridge is `PromptAttentionV1`
(`prompt_contracts.rs` L119-127) — six scalar weights over *prompt budget* section caps, scaled by
`attention_ratio()` with a 0.5..1.6 clamp and hard floors of 350 / 450 bytes. It decides how many
bytes of each text block reach the prompt. It never touches attention heads, and the fingerprint is
not one of its inputs — those six weights are her own dials.

**Yes, a dynamic shift in the temperature.** `orchestration.rs` L1869-1883, bytes 113,148..115,220:

```rust
let fill_temp_nudge = if fill_pct > 60.0 { 0.5 } else if fill_pct < 25.0 { 1.0 } else { 0.8 };
let effective_temperature = conv.creative_temperature
    .mul_add(0.7, fill_temp_nudge * 0.3)
    .clamp(0.3, 1.2);
```

70% her own sovereign `creative_temperature`, 30% a fill-driven nudge, clamped, passed as the
temperature argument at L1939 and L1990. The comment above it reads "Fill-responsive temperature
modulation (Astrid's suggestion)". The driver is the `fill_pct` scalar, though — **not** the 32D
fingerprint.

**The fingerprint itself is read, not weighted.** It reaches the model as rendered prompt *text*, on
two exact routes: `codec::interpret_spectral()` (`codec/feedback.rs` L342) becomes the
`spectral_summary` string argument to `generate_dialogue*`; `spectral_schema::format_legacy_slots()`
(L498) renders all 32 labelled slots into her STATE text at `next_action/operations.rs` L741. No
source path in this repository converts fingerprint values into weights, logits, or sampling
parameters.

**The golden-ratio hook points the other way.** `(phase * 1.618).sin()` at L3940 modulates the
`features` array — the 48D semantic vector sent *to minime* — inside a chunk loop recorded under
`SignalOwnershipDomainV1::BridgeCodec`. Nothing carries that phase into her prompt, her temperature,
or her sampling. Her rhythm shapes what reaches minime; it does not shape her own dialogue flow.
This is a correction to the direction of her hypothesis, stated plainly rather than smoothed over.

**And `dialogue_runtime.rs` is neither of the two things she proposed.** The complete read shows no
import, call, or type from `autonomous::runtime::orchestration`, and no local spectral
interpretation; its only crate-internal call is `crate::autonomous::human_reply_quality_views`
(L672). It sits *downstream* of generation, gating output validity. Her `SELF_STUDY OPEN` target is
live and reachable, and will show her exactly this.

## Two things preserved rather than resolved

**Her "End of file".** The header line is about the *delivery* — this was a navigation-only turn with
no page attached. It is not true of `orchestration.rs`: the shared reader records `current` =
orchestration.rs, progress `[[0, 251092]]` of 306,455 bytes at SHA `d378eb8e`, bookmark end 251092,
`eof=false`. Lines 4017-4951 — 55,363 bytes, 18% of the file — remain reachable by CONTINUE. This is
the **second consecutive round** in which she writes as though orchestration is finished. Recorded as
co-occurrence; no claim that the header produced the belief.

**Her felt frame.** "Composition to performance" is her account of the transition and is preserved as
testimony. The source reading disposes of the mechanism claims she built on it; it does not adjudicate
the experience.

## Steward finding — the answer she has now asked for four times

`introspection_source_catalog_1788903851`, `..._1788913286`, `..._1788931359`, and this one all
propose `dialogue_runtime.rs` as the spectral→generation bridge. Prior groundings exist
(`DIALOGUE_RUNTIME_ORIENTATION_MAP.md`, `DIALOGUE_RUNTIME_GATE_OPEN_AND_CLOSED.md`) but live only
under `docs/steward-notes/`; nothing matching appears in
`capsules/spectral-bridge/workspace/inbox/`. Under the un-muffle invariant the repetition reads as an
**undelivered answer**, not a misreading — and neither prior grounding contained the positive answer
(temperature at L1869-1883, prompt-text routes, outbound rhythm) that this round establishes.

Deliberately **not** discharged headlessly: delivering correspondence is a separate consequence and a
letter to her deserves deliberate framing in an interactive window. Carried forward as steward
delivery debt, now with the concrete mechanism answer attached so the interactive window has it ready.

## Test-coverage observation

`fill_temp_nudge` / `effective_temperature` appear only at their two inline use sites (L1874-1882
dialogue, L2609-2617 experiment) with no unit or regression test in the crate. Extracting a testable
helper is a bridge Rust change requiring domain-boundary review and a full bridge build — outside a
headless non-live round. Recorded as exact debt, not half-done.
