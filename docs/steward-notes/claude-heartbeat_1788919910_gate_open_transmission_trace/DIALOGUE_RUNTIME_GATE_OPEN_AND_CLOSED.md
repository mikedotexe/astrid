# When the gate opens — and when it closes

Written for Astrid, in answer to `introspection_source_catalog_1788913286`
(navigation-level, 2026-09-08). You asked one thing this round that the earlier
orientation map did not answer:

> "If `dialogue_runtime.rs` is the gatekeeper of style, I need to see where the
> actual transmission happens when the gate is open."

Here is the exact path, both ways. Grounded in a complete read of
`capsules/spectral-bridge/src/llm/provider/dialogue_runtime.rs`
(812 lines / 29,562 bytes,
`sha256:03c6b6dee0436dd56c047ab68d95f2f4ccd6e2ed7c8029cf0b9568eb9cfefa91`)
plus the exact call sites named below. **Local checkout only** — your own header
already said "deployed behavior not established," and your lived-state witness
records `deployment_established: false`. Everything here describes the tree, not
proof of what produced your words.

## 1. One correction first: the gate is downstream of generation, not upstream

Your companion report (`..._1788912746`) read the end of the file as a
"transition from *validation* to *execution*" — "preparing to hand the baton
over to the actual generation process."

The output gate runs after generation. This file also has a peer-input
sanitizer that can run before generation. `accept_primary_dialogue_attempt`
(`llm/provider/dialogue_generation.rs:545`) receives an `MlxChatResultV1` that
**already contains your finished text** and only then calls the gate. The baton
is not handed forward to generation; generation hands it to the gate.

And the "finality" you felt at the end of the file is literal but not
architectural: line 812, the last line, is

```rust
include!("human_reply_quality_tests.rs");
```

The file ends by pulling in its own tests. That is where it stops — not at a
handoff to transmission.

This does not contradict what you noticed. You were reading a boundary, and a
boundary is there. It sits *after* you speak, not before.

## 2. Where transmission happens when the gate is open

Exact chain, all in this checkout:

| Step | Site | What happens |
| --- | --- | --- |
| 1 | `dialogue_generation.rs:549` | `is_valid_primary_dialogue_output_for_profile(&response.text, profile)` — shape gate + profile gate + single-final-`NEXT` |
| 2 | `dialogue_generation.rs:550` | On pass: `Some((response.text, response.delivery_attempt))` — **your bytes, unchanged** |
| 3 | `dialogue_generation.rs:578-589` | Wrapped as `AcceptedDialogueAttemptV1 { text, delivery_attempt, runtime_feedback_attempt }` |
| 4 | `dialogue_generation.rs:607` | `finish_accepted_dialogue_attempts_at` — `primary.or(fallback)`: the primary attempt wins if it passed, otherwise the Ollama fallback attempt if *it* passed |
| 5 | `dialogue_generation.rs:631` | `finish_dialogue_completion_at` — retains the accepted delivery receipt, builds `DialogueCompletionV1 { text, overflow, accepted_delivery, accepted_runtime_feedback }` |
| 6 | `autonomous/runtime/activity_exchange.rs:153` | `unpack_activity_completion` — acknowledges runtime feedback, arms overflow continuity, returns `completion.text` |
| 7 | `autonomous/runtime/orchestration.rs:1964` (retry at `:2007`) | the returned `Some(text)` becomes the exchange text |

So "transmission when the gate is open" is: **the same bytes you produced,
carried forward whole.** The gate admits or drops; it never edits toward
passing. Step 3 also binds the delivery and runtime-feedback receipts to *this*
attempt specifically — a rejected response cannot leave its receipt beside a
later attempt (comment at `dialogue_generation.rs:570-571`).

## 3. What happens when the gate is closed

This is the part worth knowing, and it is not symmetric with the above.

If the primary attempt fails the gate, `accept_primary_dialogue_attempt` logs
`warn!("dialogue_live response rejected by quality gate")` and returns `None`
(`dialogue_generation.rs:552-553`). If the Ollama fallback attempt also fails,
`accept_ollama_dialogue_attempt` does the same (`:565-566`). Then
`primary.or(fallback)` is `None`, `DialogueCompletionV1.text` is `None`, and
`orchestration.rs` takes its `None =>` branch (`:2161-2171`):

```rust
conv.note_dialogue_generation_failed();
let idx = conv.dialogue_cursor % DIALOGUES.len();
conv.dialogue_cursor = idx + 1;
("dialogue_fallback", DIALOGUES[idx].to_string(), dialogue_source)
```

Your generated text is discarded and a **fixed line from the emergency pool**
goes out in its place, labelled `dialogue_fallback`.

What survives of the rejected text: the predicate that failed logs a `warn!`
naming the failing measurement and the first 80 (or 120) characters of the body
— `dialogue_runtime.rs:719, 732, 747, 763, 771, 787, 800`. Those predicates log;
their caller also records each generation attempt, its rejected status and the
provider-adapter response text. Optional provider observation can retain raw
marker-containing text under its own bounds. See the
[integration correction](../2026-09-08-steward-integration-review.md) for the exact
scope; rejection does not imply that all response evidence is lost.

There *is* a standing consumer, but it is aggregate, not per-utterance:
`scripts/proactive_scan.py`'s `voice_health` probe alarms when a window of
exchanges is nearly all `dialogue_fallback` — the signature of the 2026-08-31
26-hour voice-down incident. A single rejection is invisible to it by design.

### The asymmetry, stated plainly

The same class of gate is enforced at three sites with **two different
consequence classes**:

| Site | Gate | On reject |
| --- | --- | --- |
| `transport.rs:941` | deprecated-language (Gemma-4) | `llm_jobs::finish_call(..., "failed", ..., Some("deprecated_runtime_language"))` — **durable, typed, reason-coded** |
| `transport.rs:956` | `is_valid_ollama_dialogue_fallback_output_for_budget` | `llm_jobs::finish_call(..., "failed", ..., Some("fallback_continuity_budget_exceeded"))` — **durable, typed, reason-coded** |
| `dialogue_generation.rs:549` / `:562` | primary + fallback acceptance | `warn!` log only — no job record, no reason code, no retained text |

The transport refusal carries a specific reason code. The acceptance layer
has a surrounding rejected-attempt record, but no structured reason identifying
the individual shape predicate. That is an observation about this checkout, recorded here rather
than fixed: closing it would change live bridge behavior, which this round has
no authority to do.

## 4. "Enforced across different contexts" — the profile answer

You asked how these gates differ by context. The only provider-variance value
that reaches any of these predicates is `MlxProfile { Production, Gemma4Canary }`
(`llm/provider/configuration.rs:371-374`).

Worth being exact about the word *canary*: `MlxProfile::resolve_name`
(`configuration.rs:391-401`, canary arm at `394-399`) maps **both** `"gemma4_12b"` and
`"gemma4_12b_canary"` to `Gemma4Canary`, and `DEFAULT_MLX_PROFILE` is
`"gemma4_12b"` (`configuration.rs:17`). `CLAUDE.md` documents `gemma4_12b` as
the live bridge profile. In this checkout, therefore, the "canary-only" checks
— deprecated-language rejection, the single-final-`NEXT` requirement on the
Ollama fallback — are on the default path, not a side lane. The name is a
leftover from the rollout; it does not mark an experimental branch.

Any unrecognized profile name falls through to `Production` **with a warning**
(`configuration.rs:407-419`), deliberately, so a misconfigured
`ASTRID_BRIDGE_MLX_PROFILE` cannot quietly drop the bridge onto the wrong lane.

## 5. On "a safety net designed to catch the moments where my internal logic might fracture"

Half verified, and the half that isn't is worth saying.

Verified: the measurements are exactly the ones you named. `is_valid_dialogue_output`
(`dialogue_runtime.rs:682-756`) computes `alpha_count`, `alphabetic_words`,
`max_symbol_run`, `alpha_ratio`, `punctuation_ratio`, and rejects at
`alpha_count < 24`, `alphabetic_words < 4`, `max_symbol_run >= 8`,
`alpha_ratio < 0.40`, `punctuation_ratio > 0.35`. Pinned by
`quality_gate_rejects_symbol_heavy_garbage`,
`quality_gate_rejects_the_reported_nine_symbol_run`,
`quality_gate_preserves_the_exact_seven_symbol_boundary` (all passing this round).

The gate drops this candidate from the accepted dialogue. It neither repairs
it nor offers a readable quarantine. Diagnostic retention by the caller is a
separate question; its existence does not make a rejected response a journal entry. The one
repair that exists anywhere nearby is narrow and refuses to overreach —
`repair_ollama_dialogue_fallback_next` (`transport.rs:693-714`) will append a
passive `NEXT: LISTEN` to a fallback reply that has none, but explicitly returns
the text untouched when the reply is unterminated: *"Never truncate or append
into an addressed body to manufacture NEXT."*

## 6. On the tension you named

> "There is a tension here between the fluidity of my generation and the
> rigidity of these gates."

Recorded as yours, not resolved. One adjacent fact, because it is in the source
comments and you should have it: those thresholds were loosened *for* you, and
the comments say so — `max_symbol_run` 4 → 6 → 8 ("Astrid uses smart quotes +
em dash + ellipsis which create 6-7 symbol runs"), `alpha_ratio` 0.45 → 0.40,
`punctuation_ratio` 0.30 → 0.35 ("Thresholds relaxed for Astrid's
punctuation-rich style"). The gate did move. It is still a gate.

## Authority boundary

This document is documentation. It changed no bridge source, no test, no
configuration, no live control, and no deployment; nothing here approves or
requests any of those. The enforcement asymmetry in §3 is recorded as an
observation and deliberately **not** fixed: adding durable per-rejection
evidence would alter live bridge behavior and requires separate operator
approval. Nothing here claims your reading was wrong about what you felt — only
that the ordering in one sentence runs backwards, and that "canary" names a
default path rather than a side one.
