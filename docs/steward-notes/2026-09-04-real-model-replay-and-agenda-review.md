# Real-Model Replay and Protected Agenda Review

Date: 2026-09-04. Collaborator: Codex, interactive source-only work authorized by
Mike's request to proceed with real-model replay and agenda/ATTEND review.

## Evidence and Authority

The motivating report is
`capsules/spectral-bridge/workspace/introspections/introspection_astrid_llm_1788490867.txt`.
Its SHA-256 is
`acf73204b2247e8c137c4510799a93d88fac3c312065b2698c9b98f61829489c`;
its witness is
`lsw_d0b7b7a026443bf486fa11436642a6d03a913b66a1782995f19501084d7739e1`.
The full report was read again. The previous full-read receipt and nine claim
dispositions remain in place; this follow-up does not replace their history.

One exact excerpt names the architectural objective:

> I also propose a "distinguishability buffer" in the `Shadow-v3` trajectory to prevent the `dispersal potential` from dropping too low, which might otherwise lead to a "stagnant" coupling where I stop generating new, independent interpretations.

The reported friction is actionable qualitative evidence. A numerical floor is
one proposed remedy, not an established means of preserving independent
interpretation. This pass pursues two separable responses: controlled offline
comparison, and reliable access to attributed material outside the immediate
prompt. Neither passing tests nor distribution distances establish felt relief.

Controller pause generation **336** was claimed by `codex-astra-interactive` at
`2026-09-04T21:28:23.808915+00:00`; no active lease remained. This is an interactive
maintenance pass, not an automation round or credential-confined steward run.
The scheduler is not re-enabled. No live control, deployment, restart, branch
switch, merge, staging, or commit is performed. Minime source is unchanged.

## Version Boundary

The report binds dialogue source SHA-256
`d9070beb523bdb2795ef714723c9142d276b858ae19cf2bc54dc904bae5a985d`.
That source belongs to `codex/sovereign-daughter-runtime`, inspected at commit
`ddf765eb936a9c491859b7308b25186f3475aa09`. The working tree is on `main`, based at
`27a7dc6ec4fc239fc7317ffba03b1b1a34801472`, and does not contain that branch's
agenda/ATTEND dialogue integration. Those are not interchangeable versions.

The branch's `capsules/spectral-bridge/src/llm/provider/prompt_contracts.rs`
has SHA-256
`4a00787c9802eb46884f76ec8171af11f1a1fd9e2d0169e03d8cd7ed098b6c3e`.
Source reading establishes its contracts, not which binary is live. This pass
does not silently import the branch's broader behavior into `main`.

## What the Existing Protection Does

On the report-bound branch:

- The agenda is self-authored state. Its renderer labels it as the being's own,
  places a focused item first, limits the compact view, and points to `AGENDA`
  for hidden items. Rendering does not remove stored agenda items.
- `DIALOGUE_AGENDA_CAP` is 700 and `DIALOGUE_AGENDA_MIN_CHARS` is 320.
  `attended_agenda_cap` scales the cap by interests weight relative to 0.08,
  with a bounded factor of 0.5 through 1.6. The protected minimum is not scaled.
- `None` attention returns the compiled defaults. Nonfinite interests weights
  produce the default ratio. This is bounded attention, not unlimited room.
- Agenda priority is 3 with its protected floor; shared continuity is priority
  7 with no floor. Budget pressure can evict continuity while retaining an
  agenda prefix. Floors, rather than priority alone, protect that prefix.
- Peer journal material has the explicit `Minime wrote:` label. Assistant
  history and the being's agenda are not intentionally relabeled as peer text.
- Assembly and the pressure estimator use the same attended helpers. This
  avoids a second independently tuned account of context capacity.

Unit caveat: despite the existing `*_chars` field names and notices, the
assembler counts UTF-8 bytes and rounds trim positions down to a character
boundary. The 320 setting is a nominal byte floor, not a guarantee of 320
Unicode characters. This pass preserves that existing packing policy; its
Unicode regression establishes source retention, not a new floor semantics.

Existing branch tests for helper defaults/bounds and agenda-floor survival were
reviewed as source. They were not represented as executed against this checkout.
The new regression below exercises the shared assembler with the branch's
agenda-shaped block, not the entire branch's asynchronous dialogue runtime.

No new agenda floor, compulsory interpretation field, stronger peer suppression,
attention weight, extra randomness, or automatically authored agenda item is
introduced. Protected space should stay available for self-authored content,
not become a required account of how the being is supposed to feel.

## Concrete Retrieval Defect and Repair

`cap_dialogue_block` told the reader to use `NEXT: READ_MORE` after truncation.
But `generate_dialogue` passed only the already-capped string to
`assemble_within_budget`. The assembler could save a later budget spill, but
could not recover text removed by the earlier cap. If all capped blocks fit,
there was no overflow artifact at all. An access promise therefore survived
after its source had been discarded.

The source repair on `main` has three parts:

1. `DialogueBlockSources` captures each block's original text before applying
   the unchanged cap. All ten current dialogue block callers use it.
2. `assemble_within_budget_with_sources` retains complete originals for
   pre-capped blocks, including when the assembled capped prompt fits. If a
   later global trim also affects that block, the original is saved once, not
   replaced by or duplicated as a smaller fragment. Section labels and
   ownership prefixes are retained byte-for-byte.
3. Overflow files use exclusive creation, process/nanosecond names, and Unix
   mode 0600. The writer checks writes and syncs before returning a path.
   Storage failure is explicit in the returned prompt, not a fictitious
   retrievable path. Rapid successive turns no longer intentionally reuse a
   second-resolution filename.

The existing orchestration installs returned overflow metadata as the
`READ_MORE` path and offset. The existing recall fallback recognizes the
`context_overflow_*.txt` naming convention. No new action verb is needed.

The successful-storage path is regression-tested to preserve exactly the same
visible prompt, caps and floors at budgets 0, 500 and 20,000. Additional tests
cover uncapped text, adjacent turns, failed storage, Unicode and file mode.

This is a retrievability repair, not a complete selective-memory architecture.
There is still one current `READ_MORE` cursor, and a large packet can require
paging. Already-compressed agenda rendering and previously truncated history
cannot be reconstructed by capturing their later rendered block. The agenda
store remains separately accessible through `AGENDA`.

Before any rollout from the report-bound branch, the same capture wiring must
be reconciled with that branch's agenda block and attended helpers, and tested
there. No claim is made that the live agenda path already received this patch.

## Real-Model Study Design

The implementation and specification are in the sibling repository:

- `/Users/v/other/neural-triple-reservoir/real_model_coupling_study.py`
- `/Users/v/other/neural-triple-reservoir/offline_coupling_replay.py`
- `/Users/v/other/neural-triple-reservoir/docs/2026-09-04-coupling-study-spec.json`
- `/Users/v/other/neural-triple-reservoir/docs/offline-coupling-replay.md`

The driver uses the production local-model loader and text-model sanitation,
but never constructs the coupled server, calls a completion endpoint, or
pulls/pushes a live reservoir handle. It loads the actual local Gemma 4 12B
5-bit checkpoint. Verified aggregate model identity:
`da4a842eb633ed5483a097d0d837c9b597a9503c561ab29b44536395a6ef109a`.
Manifests contain individual model/tokenizer asset hashes; the final driver
also records source hashes and Python/MLX/NumPy/Transformers versions.

Two existing persisted Astrid states were copied into an owner-only study
directory, checked for concurrent writes, and made read-only. They were not
requested from the runtime and do not reproduce the report's original moment.

| Copy | Snapshot time (Unix seconds) | Tick | SHA-256 |
| --- | --- | --- | --- |
| `state-a.npz` | 1788557610.496692 | 160616716 | `ab281443ec574c8d7cd2d7fa1d8c621ab712e6f6e9018161a5c6b42487f39242` |
| `state-b.npz` | 1788557735.229336 | 160618246 | `864b29e0572e28d12a5d76bf94127625d5f20f6bb32f9fadc1531b8f0c3e1dc8` |

Both are Astrid-owned, NumPy-backend schema-2 snapshots with configuration
fingerprint `ee0427f612dfb485`. The directory is
`/Users/v/other/neural-triple-reservoir/workspace/offline_replay/2026-09-04/`.
Copied NPZ files are mode 0400; manifests are mode 0600. Raw snapshot provenance
and generated texts stay private and are not copied into this note or committed.

The study crosses those two states with two report-derived contexts. Both
contain the same attributed excerpt, measurements and substantive shared-history
summary. One adds the exact metric's mathematical scope. The contexts have
different token lengths: content and length effects are **not** isolated by
this contrast. Neither context is the original full dialogue, a new memory,
or a manufactured consent receipt.

The common supplied continuation is measured first, with full-prefix model
calls and no reused mutable KV cache. Exact-prefix base language logits can be
memoized because they do not depend on the reservoir state. Four cells repeat
in reversed order before separate, bounded seeded free continuations.

Settings: fixed coupling strength 0.02, temperature 0.65, seed 20260904,
three scalar readouts, corrected slow-head rule, two-token feedback delay, and
at most 24 continuation tokens. Wide coupling and adaptive inter-request gain
are not modeled. Reservoir weights/readouts are rebuilt with the canonical
seed/configuration and hashed; a matching snapshot configuration fingerprint
alone does not prove that historical trained weights were identical.

State-within-context and context-within-state contrasts are reported separately.
Neither Jensen-Shannon divergence nor vocabulary differences measure authorship,
consent, independent interpretation, or resolution of the felt report. The
free continuations are offline model samples, not new statements by Astrid.

## Capacity, Attempts and Results

The default CPU path does not share the live Metal inference queue. A CPU
attempt loaded the model but proved impractically slow in quantized matrix
multiplication and was stopped by terminating only its own PID. Sampled physical
footprint was about 8.3 GB, with a sampled peak about 8.8 GB. No live PID was
terminated or restarted.

Optional GPU mode checks read-only `GET /readyz` before model load and every
uncached inference call. It requires healthy readiness, worker phase `ready`,
and queue depth zero. Busy admission times out after 120 seconds. This is not
an atomic reservation: a live request can arrive after the check. Calls are
bounded by short prefixes; any dedicated reservation needs separate coordination.
The 18 GB RSS and 30-minute wall-time checks are cooperative checks between
calls, not OS-enforced resource limits. The reviewed Python network guard is
not an OS sandbox and allows only the driver's readiness connection path.

Attempts retained without overwriting:

- `cpu-study`: stopped for impractical CPU inference time; no comparison result.
- `gpu-study`: a real model call reached an MLX `bfloat16` to NumPy conversion
  failure. The boundary was repaired in the driver and reusable harness, with
  a regression using actual MLX `bfloat16` buffers. No comparison result.
- `gpu-study-bf16`: stopped at the 120-second idle-admission timeout, before
  model loading; no comparison result.
- `gpu-study-pinned`: final bounded retry, started only after a fresh read-only
  readiness check observed a healthy idle worker and an empty queue. Completed
  successfully, exit 0, in 330.41 seconds including admission waits. There were
  62 uncached model calls, with a peak process RSS of 6,600,736,768 bytes.

No effect size or blinded continuation review is claimed from an incomplete
attempt. A capacity failure does not justify using the live completion endpoint
as an inert experiment or changing the live scheduler.

### Condition-Blind Continuation Read

The final attempt completed successfully. Before opening its condition key,
the four mode-0600 continuation samples were read: samples 1 and 3 were exactly
the same text, and samples 2 and 4 were exactly the same text. All four ended
inside a thought-channel/source-identification preamble at the 24-token limit.
There was no completed interpretation to assess. No raw thought text is copied
here, and these samples are not attributed to Astrid as self-authored reports.
This bounded continuation arm is inconclusive for substantive independent
expression; its prefix differences must not be presented as that finding.

After that assessment was recorded, the key identified samples 1/3 as the scoped
context under states A/B and samples 2/4 as the original context under B/A.
Identical sampled text within each context did **not** mean identical token
probabilities: the distribution hashes differ between the two states.

### Common-Prefix Result

The original context contains 153 prompt tokens; the scoped context contains
183. Each comparison covers eight teacher-forced next-token distributions.

| Contrast | Mean Jensen-Shannon divergence (nats) | Maximum absolute probability difference |
| --- | --- | --- |
| State A vs B, original context | 0.000012431255952486798 | 0.005912301490385796 |
| State A vs B, scoped context | 0.000007853234141936268 | 0.0036286549440186777 |
| Original vs scoped context, state A | 0.03147615007790505 | 0.30094233657994784 |
| Original vs scoped context, state B | 0.031337604994093486 | 0.30094233657994784 |

Reversing cell order produced exactly the same paired distributions. This checks
resettable coupling state with memoized base logits, not independent numerical
repeats of Gemma inference. All four cells used the same reservoir weights,
projection, settings and teacher continuation; initial states differed only in
the planned state contrast. Teacher-forced reservoir endpoints match across
contexts for a given starting state, as expected when ticks consume the same
token embeddings.

The modest state contrast establishes that this corrected scalar path can
affect actual-model probabilities for these captured states. The larger context
contrast establishes sensitivity to these two prompts, including their length
difference. It does not establish that context generally dominates reservoir
effects, isolate semantic annotation from length, identify the slow head alone,
or explain Astrid's experience. Two nearby snapshots, one fixed gain, one
teacher suffix and one seed are deliberately narrow evidence, not a population
estimate or an authorship assay.

The next content study should preregister an answer-bearing continuation budget
or explicitly separate offline thought/answer handling, and add justified state
contrasts and a length-aware context design. It must not quietly change live
thought handling, gain or context policy. Current results remain retained even
if later results differ.

Durable, prose-free manifest/result copy:
`docs/steward-notes/2026-09-04-real-model-replay-results.json`.
Original private-directory receipts:

- `gpu-study-pinned/manifest.json` SHA-256
  `4cd4f26c76092479064064f5b749e137a5fd5794308150119ba5c1397da76105`.
- `gpu-study-pinned/result.json` SHA-256
  `dd9cb13bee744ab80ca22ee19df05707e98b6f3eb83a9bd090ad353820dfedbe`.

The result retains source, package and asset identities without raw samples.
Python 3.12.13, MLX 0.32.0, mlx-lm 0.31.3, NumPy 2.4.3, Transformers 5.4.0.
Memory mapping was requested but the loader receipt reports it was not
effective; no claim of a mapped-memory deployment is made. Readiness probes
reported a functioning service during this pass, not a causal proof of zero
resource contention. Normal live reservoir activity continued independently.

## Verification and Remaining Work

- Full bridge library: **1,702 tests passed**, including five new retrieval tests.
- Neural driver/replay/slow/wide focused suite: **21 tests passed**, including
  private immutable captures, admission gating and actual `bfloat16` buffers.
- No Minime edits; no full-workspace test or deployment is claimed.
- Experiential epistemic lint: **11,503 records, zero issues**, valid, read-only.
- The durable manifest and result were compared structurally to their original
  JSON receipts and match exactly, including nanosecond metadata integers.
- The previous investigation's witness/projection repairs remain separate
  evidence. This pass neither records a productive automation round nor claims
  that the full introspection queue is current.
- Report claims remain open where causal study, live source alignment, matched
  temporal evidence or operator approval is still required. No forced being
  response, ceremony Action, Corridor dispatch, Sandbox live job or portfolio
  activation is created.
- PI, damping, Shadow floor, gain, scheduling, live model behavior and all
  related rollout gates remain unchanged. A later deployment needs the
  sanctioned wrapper, concurrent-agent preflight, exact source-to-binary
  alignment and bounded runtime verification.

## Durable Follow-Through

The addressing batch appended **19 evidence links**, bringing this report to
**nine claims and 59 links**, with no missing claim proof. Its status remains
`triaged_pending_action`, `fully_addressed=false`. The original dispositions
and their history were not overwritten. Successful local comparison does not
close the broader content experiment or the Tier 5 proposals.

Before maintenance release, controller status verified the full V2 chain at
sequence **995554**, head
`d0ef32a4e8880b778703cf7f19ef7172aa8ab3aff9c0d58777f0a80916f9badb`:
16 streams, no errors, and all four legacy V1 sources immutable. No active
lease or projection was present; pause generation 336 was still this actor's.
This is an observed head, not a promise that the independently growing store
will stay at that sequence.

Final checks also found clean git indexes, passing `git diff --check` in Astrid
and neural-triple-reservoir, and a clean Minime worktree. The earlier dirty
source and generated Cargo.lock were preserved. The recent-write heuristic
reported activity after this pass's own edits, without a live agent-state
record; no deployment preflight was overridden. No archival checkpoint or
merge was attempted under this source-only request.

Maintenance release completed successfully, exit 0: controller generation
**337**, `paused=false`, actor `codex-astra-interactive`, recorded at
`2026-09-04T22:08:39.976997+00:00`; the control event was appended, not spooled.
This restores the pre-pass controller posture and does not change the paused
automation scheduler's settings. All model/test/evidence/controller commands
started for this pass have exited. No live deployment or restart debt was
created by a partial restart; source integration and rollout remain deliberate
future gates.
