# introspection_minime_minime_src_sensory_bus.rs_1789601340 — drain_sensory_batch, and a helper she already holds

**Report:** `capsules/spectral-bridge/workspace/introspections/introspection_minime_minime_src_sensory_bus.rs_1789601340.txt`
(43 lines, 3708 B, sha256 `ef2a74508c8dcfa9c834737c0c01c3b57afe394360a951651b72ce90573fcddb`, read complete)
**Witness:** `lsw_f28ccee4b47ea64b12517d49341d6611917eaf5a8149ac7e21f808e1fbcb4575`
(498 lines, 21323 B, sha256 `336bf20e81a3633f413861ebefdc1a662f8d7226f614db04e87cb59796228c35`, read complete)
**Report-bound source:** `minime/src/sensory_bus.rs` at
sha256 `3fc6bd2a16bd78c5caa496f2a6dccbc67928da4fbded123998f59a82bcd4aa3a`, bytes 107073..111564
(= lines 2877..2965 per the witness snapshot). The working copy
`/Users/v/other/minime/minime/src/sensory_bus.rs` hashes to the **same** SHA and is clean in git,
so report-time and current source are identical bytes and no report-time reconstruction is needed.
Her page covers 89 of the file's 4404 lines; everything below that is *outside* her window is
labelled as such.

## What she said

A source-page turn on `drain_sensory_batch`. She reads the `z` vector's construction, the semantic
scaling block, and the global-noise block, and closes with a stated need and a chosen Action:

> I need to see how `stale_scale` is actually calculated to answer my standing question about the
> interaction between pressure, entropy, and decay.
>
> NEXT: SELF_STUDY CONTINUE

## What complete source established

**Her reading of the page is accurate throughout.** The lane layout she gives — video 0-7, audio
8-15, aux 16-17, semantic from 18 — is exactly `z[..8]`, `z[8..16]`, `z[16]`, `z[17]`,
`z[18..(18 + LLAVA_DIM)]` (2931-2945). The scaling formula she quotes,
`effective_semantic = semantic_scale * emb_strength * (1.0 + j_resonance * 0.5)`, is line 2943
character for character. The noise she describes as uniform `±(noise_level × 0.05)` over every
dimension is confirmed by the sampling line **past her window**: line 2966 is
`(rng.gen::<f32>() - 0.5) * noise_level * 0.10`, which spans ±(noise_level × 0.05), inside
`for dim in z.iter_mut()` over all of `z`, gated by `if noise_level > 0.0` (2960-2969). She
inferred that from the comment alone and the code she had not been shown agrees.

**Two precisions, neither of which touches her point.**

1. `Z_DIM` is **66**, not "likely 64 or larger". It is `VIDEO_DIM + AUDIO_DIM + AUX_DIM +
   LLAVA_DIM` (line 25) and `LLAVA_DIM` is 48 (line 24). The number was on her own page: the doc
   comment at 2888-2890, three lines above the function she read, says "the production 66D vector:
   `[video8 | audio8 | aux2 | semantic48]`". Her hedge was a recall hedge, not a source gap.
2. Of the three sovereignty knobs she names, only two act in the code she read.
   `embedding_strength` and `journal_resonance` are locked and applied at 2941-2943.
   `memory_decay_rate` appears on her page **only in the comment** (2938); it acts inside
   `semantic_stale_ms()` at 1720-1721, as `decay_mult = (1.0 - (decay_rate - 0.1) * 3.0)
   .clamp(0.5, 2.0)` multiplying the stale *window*, not the semantic amplitude. Her grouping is
   the comment's grouping and is fair; the mechanisms differ.

## The thing she asked for, and the Action that cannot deliver it

`fn stale_scale(age_ms: u64, stale_after_ms: u64) -> f32` is **lines 1377-1417** (bytes
48960..51175) — 1500 lines *behind* the page she was reading. All three of its call sites (1237,
2144, and the one on her page at 2927) are also behind her. There is nothing left ahead in this
file for `SELF_STUDY CONTINUE` to deliver that carries the answer.

It is not unread, either. Her own walk of this file opened at byte 48123 (= line 1356) and its
first page was 48123..52681 — which contains `stale_scale` **whole**. That page was delivered five
times: `..._1789510747`, `..._1789511040`, `..._1789511259`, `..._1789511415`, and most recently
`..._1789595153`, 103 minutes and about fourteen pages before this report.

Runtime confirms the prediction rather than a hypothetical: after this report she continued, and
eight further pages arrived (`..._1789601886` through `..._1789605181`, bytes 111564..149732).
None of them carries the definition. Forward motion kept succeeding and kept increasing the
distance.

Her standing question — "the interaction between pressure, entropy, and decay" — is answered by a
second function in the same direction. `semantic_stale_ms()` (1710-1729) is the fusion itself: it
reads `fill_pct_for_stale`, `semantic_entropy_for_stale`, `semantic_entropy_velocity_for_stale`,
`semantic_pressure_risk_for_stale` and `semantic_stale_shape`, takes a base window from
`dynamic_semantic_stale_ms_for(fill, shape)` (defined line 171), multiplies by the decay
multiplier above and by `semantic_context_persistence_multiplier(fill, entropy, velocity,
pressure_risk)` (defined line 251). Delivery states differ across those pieces and the difference
matters:

| Piece | Lines | Bytes | Delivered in this walk? |
| --- | --- | --- | --- |
| `stale_scale` | 1377-1417 | 48960..51175 | yes — page 48123..52681, five times, last `..._1789595153` |
| `semantic_stale_ms` | 1710-1729 | 63614..64734 | yes — across pages 60337..64497 (`..._1789596763`) and 64497..68590 (`..._1789597106`) |
| `dynamic_semantic_stale_ms_for` | 171-202 | 6684..8080 | **no** — before the walk's open point (48123) |
| `semantic_context_persistence_multiplier` | 251- | 9883.. | **no** — before the walk's open point |

So two thirds of her standing question is behind her cursor, and one third was never opened in this
walk at all. In both cases `CONTINUE` is the one Action that cannot get there.

## What was implemented

`crates/astrid-source-study/tests/behind_cursor_helper_midwalk_reach.rs` (4 tests, all passing)
pins this as the third distinct reach shape, the one the two existing pins do not cover:

* `in_file_producer_walk_reach.rs` — target *ahead*; `CONTINUE` suffices.
* `walked_past_definition_reach.rs` — target behind, walk at **EOF**; `CONTINUE` has nothing left,
  so the dead end is at least immediate.
* this file — target behind, walk **mid-file**; `CONTINUE` keeps returning real source pages and
  keeps moving away, so nothing surfaces the wrong-affordance condition.

The tests also pin the bounded cost of the correction: `FIND`/`RELATE` name the holding file from
the mid-walk cursor, and `OPEN <source> <definition line>` followed by `OPEN <source> <frontier
line>` returns her to the exact page she was on — two Actions, using the line numbers the delivered
page already renders.

## What this does not establish

The tests are read-only reachability pins over a temporary fixture. They change no live navigation,
ranking, prompt, or dispatch behaviour, and none was changed this round. They assert nothing about
what she recalls having read — re-reading a page she has already seen is her prerogative, and the
window-resume fix exists precisely to protect it. No claim is made that the delivery surface
*should* volunteer a backward hint; whether the reader is ever told which of the three shapes she
is in remains an open design question for Mike, not a steward decision. Deployment behaviour is
unestablished: her own header says "local checkout; deployed behavior not established", and the
witness records `deployment_established: false`.
