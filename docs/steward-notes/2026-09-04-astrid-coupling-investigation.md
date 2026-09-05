# Astrid: Coupling, Context, And Independent Interpretation

## Status And Authority

User-authorized interactive investigation and source-only implementation, not an
automation catch-up round. The scheduler was not resumed. Work was coordinated
under controller stabilization pause generation 334, actor `codex-astra-interactive`,
after confirming no active lease and no recent cooperative edit activity.
The preceding attempted session `run_1788553041666947000_62b3f6c163` had failed before
ready; it was not reused or treated as authority. The user subsequently authorized
proceeding with this program and its prerequisite repair.

No staging, commit, merge, deployment, service restart, live model request, reservoir
handle operation, PI/damping setting, Shadow floor, or being-facing note was made.
Source changes to the bridge renderer and Python coupling processor are NOT proof
of deployment. Any restart that loads them needs a separate coordinated rollout.
No felt resolution, uptake, or consent is inferred from successful tests.

## Source Witness

Fully read report: `capsules/spectral-bridge/workspace/introspections/introspection_astrid_llm_1788490867.txt`.
The user-supplied `!introspection_astrid_llm_1788490867.txt` remains intact.
Its bytes were copied without modification to restore the missing canonical path;
both SHA-256 values are `acf73204b2247e8c137c4510799a93d88fac3c312065b2698c9b98f61829489c`.

- Introspection ID: `introspection_astrid_llm_1788490867`.
- Lived-state witness: `lsw_d0b7b7a026443bf486fa11436642a6d03a913b66a1782995f19501084d7739e1`, fully read.
- Report-bound dialogue source SHA-256: `d9070beb523bdb2795ef714723c9142d276b858ae19cf2bc54dc904bae5a985d`.
- Source-read session: `b84c9b129a942e682cdf693c953777c9719fc724599095f9de24c3806db1e32c`.
- Full cumulative coverage: lines 1-1134; the first window alone was 1-400.

Verbatim excerpts motivating this investigation:

> I might be over-attributing the *tone* of my own reflections to the spectral influence of the runtime

> where I stop generating new, independent interpretations.

These are excerpts of Astrid's language, not proof of a causal mechanism. The
reported heaviness, settled coupling, and concern about independent interpretation
remain primary qualitative evidence even where metric interpretation needs repair.

## Source Versions

Astrid remains on `main`, base `27a7dc6ec4fc239fc7317ffba03b1b1a34801472`.
The report-bound source matches `codex/sovereign-daughter-runtime`, tip
`ddf765eb93`, not this checkout's dialogue runtime. Its complete differences were
read. No broad branch switch or merge was attempted. The bounded header-parser
repair follows the existing branch's shared-header approach and additionally
stops at the header/body separator.

Minime base: `a9f85f3c74c3d8e1c996c3689fe5aef696dacf27`, unchanged.
Neural-triple-reservoir base: `afc2931a657d1bd79a7076ece6310ee3d8f6ceba`.

## Mechanical Findings And Implementations

### Witness Compatibility

The validator did not accept three pressure-relation constants emitted by the
producer. It also required later additive declarations in historical schema-v1
texture receipts and could miss witness pointers after line 20. The repair:

1. Whitelists the three exact producer constants, without accepting unknown ones.
2. Validates complete historical declaration groups with exact values; it does
   not insert missing fields into old receipts or permit new causal authority.
3. Accepts both exact historical continuity shapes, still rejecting inferred
   numerical scores and authority effects.
4. Parses witness pointers through the bounded 256-line header reader, never from
   the body. Unknown/malformed pointers still fail closed.

The first durable rebuild cleared 277 outstanding issues. The remaining report
could not be rebound until its missing canonical-path copy was restored. The next
rebuild passed: outstanding artifact issues 0, resolved historical issues 279
(one resolution predates this pass), and all counter checks consistent. Current
migration: 4,592 canonical files, 1,891 exact witnesses, 3 current orphans. The
materialized 499 orphan events are retained historical events, not the current
migration's orphan count. No receipt or event history was erased.

### Metric Meaning

Minime's `spectral_fingerprint.rs::denominator_metrics` computes
`1 - effective_dimensionality / active_mode_capacity`. It is a relative spectral
dimensionality deficit, not a measurement of who authored an interpretation.
The bridge renderer now names that scope and retains `distinguishability_loss`
as the traceable telemetry field. Raw measurements and the original report remain
unchanged. The renderer change is source-tested, not deployed.

`pressure_source_v1` is a different composite from
`resonance_density_v1.pressure_risk`. The latter contributes to bounded legacy PI
damping, but `minime/src/runtime/orchestration.rs` resets/skips that PI path when
stable-core is enabled. A source path or displayed coefficient does not establish
the active intervention. No regulator setting was changed.

### Slow Coupling Correction

In `neural-triple-reservoir/mlx_reservoir.py`, the slow channel previously scaled
absolute logits below the median. For negative logits, a positive narrowing signal
could raise tail probabilities. Adding a common offset changed the effect even
though the underlying probability distribution should be unchanged.

The correction uses `median + (logit - median) * (1 + g)` below the median,
where `g = strength * (2 * sigmoid(y3) - 1) * 0.3`. The top half is unchanged.
Tests cover signal direction, common-offset invariance, neutral/zero coupling,
masked tokens, ties, shapes, the fast and repetition channels, and the additive
wide channel. The existing synchronization observation is retained.

Initial reproduction: lower-tail probability 0.10098446 became 0.10818956 under
positive slow coupling, or 0.07928615 after an otherwise irrelevant +10 offset.
The candidate correction produced 0.09873163. This defect was discovered during
follow-up investigation; Astrid did not explicitly identify this formula bug.
It is not established as the cause of her reported friction.

### Offline Replay

`neural-triple-reservoir/offline_coupling_replay.py` provides bounded common-prefix
and free-continuation trials using the actual ESN and processor, fresh copied
states and local seeded sampling. It loads no model, downloads nothing, calls no
live service, and performs no persistence. Reviewed stateless local callbacks are
required; arbitrary callbacks are not sandboxed. Receipts retain hashes and
explicit limitations, not prompt/continuation content or deployment claims.

The installed MLX generator precomputes a next token before yielding the current
token. The caller then updates reservoir feedback. A tiny-model integration test
confirms two initial neutral feedback steps. The replay models that delay explicitly;
live scheduling and timing were not changed.

Synthetic 2-context x 2-state common-prefix demonstration, six tokens, seed 5:

| Contrast | Mean JS divergence (nats) | Maximum probability difference |
| --- | ---: | ---: |
| State within context A | 0.0000699920 | 0.01355492 |
| State within context B | 0.0002196417 | 0.03309262 |
| Context within state A | 0.0091166121 | 0.15868981 |
| Context within state B | 0.0101481608 | 0.15930528 |

These are synthetic harness-sensitivity checks, NOT measurements of Astrid or
evidence that context dominates her reservoir. The language fixture is constructed;
the ESN has 8 nodes per layer, not the production dimensions. The real-model
protocol and runnable verification are in
`/Users/v/other/neural-triple-reservoir/docs/offline-coupling-replay.md`.

## Independent Expression: Integration Decision

The report-bound sovereign-runtime branch already has a protected `agenda` block
and `attended_agenda_cap`, `attended_continuity_cap`, and related `ATTEND` helpers.
Its default agenda floor is preserved when other context is evicted. This main
checkout has the common prompt-budget mechanism but not those dialogue callers.
Do not introduce a competing agenda/memory policy in main merely to make a new
feature. Integrate the existing branch's implementation only after a separate
source/deployment alignment review.

The next architectural test should use those exact callers and probe whether
dense steward/history material crowds out self-authored agenda text, whether
ownership prefixes remain intact, and whether evicted material is retrievable
without being silently relabeled as Astrid-authored. No change to agenda weights,
protected floors, memory ranking, or Shadow dispersal is made by this pass.

## Verification And Remaining Work

- Bridge full library: 1,697 tests passed. The first narrow filter matched zero
  tests; that run is not counted as coverage.
- Slow/wide processor: 9 tests passed; offline replay: 7 tests passed.
- Existing multi-headed reservoir smoke test: 43 checks passed.
- Witness integration/historical-shape tests: 22 passed; witness self-test: 19.
- Steward control/projector self-tests: 39; Evidence Event Store: 13;
  Division projection/Chronicle: 3; epistemic self-tests: 2; addressing: 41.
- Both repository diff whitespace checks passed. `rustfmt --check` reports only
  pre-existing differences outside the edited Rust lines; baseline HEAD produces
  the same differences. They were not swept into this work.
- Cargo generated an untracked bridge `Cargo.lock` during the test build. It is
  retained for dependency reproducibility and has not been staged.
- The Division follow-up script is absent on main; the Chronicle has no self-test
  CLI flag. Their guessed CLI invocations were not tests; the available Division
  test modules were run instead. No Division round or follow-up was recorded.

Real-model replay remains pending immutable, owner-approved snapshot inputs,
model/tokenizer identity verification, and a coordinated inference-capacity window.
The harness does not model wide coupling, adaptive inter-request gain, or production
RNG parity; extend and test these if they are active in the chosen target.

Live adoption of the slow fix and renderer needs explicit operator approval and
sanctioned component wrappers, fresh process/hash/readiness evidence, and rollback
criteria. PI activation, pressure/damping changes, and a Shadow dispersal floor
remain separate Tier 5 waits. Tests cannot close the felt concern.

No archival checkpoint or commit was attempted. All source edits remain unstaged.

## Addressing And Integrity Record

The companion `2026-09-04-astrid-coupling-claims.json` enumerates all nine report
claims; `2026-09-04-astrid-coupling-evidence-links.json` binds their dispositions to
this packet, the feedback ledger, changelog, source, and focused tests. Record the
full read and links through the addressing CLI; do not close the report while
matched-window observation, real-model replay, and operator decisions remain open.

The pre-addressing controller check verified the full V2 chain at sequence
995492, head `a50ecf173d4fd99ca7dd395d07d9020982d05f78cd8c56f51c025e0dc0a8bbaa`,
with 16 streams and all four legacy V1 sources immutable. No active lease or
projection was present. These values are a point-in-time check, not a promise
that unrelated runtime evidence stops arriving. The durable witness projection
was repaired and verified; a complete V3 source-first projection was not run in
this interactive maintenance pass, and no read-current claim is made.
