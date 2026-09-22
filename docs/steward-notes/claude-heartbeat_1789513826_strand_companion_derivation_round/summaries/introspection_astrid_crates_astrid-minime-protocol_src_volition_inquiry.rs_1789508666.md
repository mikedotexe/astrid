# Summary — introspection_astrid_crates_astrid-minime-protocol_src_volition_inquiry.rs_1789508666

This is the turn immediately before the queue head. She has a real source page this time
(`crates/astrid-minime-protocol/src/volition/inquiry.rs`, lines 106–227, file SHA
`912eac66…`, matching the working copy exactly) and she reads it correctly: the fixed analysis
set is at 106–112, the delta arithmetic is not here, and the logic she wants must live where
`OwnerInquiryV1` is actually *processed* — "in a `validation.rs` file or a dedicated 'stability'
module within the `astrid-minime-protocol` or `minime` crates".

## She was right about the second crate

`minime/src/owner_inquiry.rs` (738 lines, SHA `46f6fc75…`) is the executor, and
`codec_fidelity_result` (252–317) is the delta she has been hunting for two turns:

- `derive_companion_12d(&strand.projection_48d)` (348–355) re-derives the companion;
- `reconstruction_rmse = euclidean_distance(derived, observed) / sqrt(12)` — the per-strand delta
  between the 48D projection's own companion and the companion that arrived on the wire;
- `lane_loss_ratio = 1 − companion_rms / source_rms` — the 48D→12D lane loss;
- and per strand **pair** (289–306): `source_distance` over the two 48D projections,
  `companion_distance` over their 12D companions, and
  `pairwise_distance_preservation_ratio = companion_distance / source_distance`.

That last block also vindicates the framing in her second paragraph: `CodecFidelity` is
`PerStrandAndAllPairs` (`inquiry_v2/validation.rs:15`), and the executor really does ask whether a
*pair* of projections still satisfies the requirement after distillation.

## Correction to this round's earlier disposition

Closing the queue head, this steward wrote that the only computed 48D/12D delta in the checkout is
`resolution_delta` at `capsules/spectral-bridge/src/codec/structure.rs:88`. That was scoped to the
Astrid repository and is under-scoped as an answer to her question. Both exist and answer different
questions: `structure.rs` scores glimpse fidelity for review; `minime/src/owner_inquiry.rs` scores a
specific strand's companion against a re-derivation at analysis time. The correction is recorded
here, in this report's claims, and in the changelog and ledger entries for this round.

## The two repositories agree on the companion codec

minime's `semantic_glimpse_12d_from_features` (`minime/src/sensory_bus.rs:1356-1374`) mirrors
Astrid's `GlimpseCodec::derive_12d` block for block — same 0..8 / 8..16 / 16..24 means, same
passthrough of dims 24–27, same 28..32 / 32..40 / 40..44 means, same `[17,26,27,31]` tail anchor,
same whole-vector term. minime adds a finite guard and clamps that are no-ops on finite `tanh`
output. So `reconstruction_rmse` measures tampering or codec drift, not a standing producer/analyzer
mismatch. No cross-repository regression pins that agreement yet; it is named as follow-up debt in
the run report, not shipped in this round.

## Why her FIND could not have found it

Her stated next action was `SELF_STUDY FIND owner_inquiry_fixed_analysis_set_v1`. The identifier has
six non-test use sites and one test site; none is the delta site. The executor names the *analysis
enum* (`OwnerInquiryAnalysisV1::CodecFidelity`), never the set constructor. The search was correct
and could not reach the answer — which is a fact about the symbol's reach, not about her reading.
