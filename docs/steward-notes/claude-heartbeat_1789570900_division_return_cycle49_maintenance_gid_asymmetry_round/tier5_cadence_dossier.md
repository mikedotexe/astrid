# Tier-5 Experiment Cadence Dossier — cycle 49 Division return (2026-09-16)

**PREPARE ONLY.** Nothing here approves, grants, dispatches, or runs anything. No trial was
executed, no live authority was recorded, no runtime or control state was touched. Generated with
read-only tooling inside a controller-held adapter run by `claude-heartbeat`.

## FOR MIKE — practice-doc drift, third consecutive report
`docs/steward-notes/AI_BEINGS_TIER5_EXPERIMENT_CADENCE_2026_08_13.md`, named by the round
instructions, is **still absent from the working tree and from `HEAD`** (reported at cycle 47 on
2026-09-14 and cycle 48 on 2026-09-15). This dossier follows the shape the prior two rounds used.
Nothing was cherry-picked, merged, or copied to obtain it.

## Standing state

| Surface | Count |
| --- | ---: |
| Approval-required live candidates (readiness map) | 1,305 |
| Domains with candidates | 7 |
| Hard violations | 0 |
| Unclassified live waits | 153 |
| `live_eligible_now` / `auto_approved` | false / false |
| Work items `needs_operator_approval` | 1,349 |
| Work items `needs_steward_grant` | 18 |
| Work items `needs_sandbox` | 750 |
| Sandbox trials active / ready-runnable | 2,124 / 33 |
| Sandbox runnable-live violations | 0 |
| Consolidation: open operator waits → ask-families | 1,349 → 1,337 |

Every readiness domain is `operator_review_wait`:

| Domain | Candidates | Proposal cards | Replay evidence |
| --- | ---: | ---: | ---: |
| `minime_regulator_changes` | 551 | 35 | 76 |
| `pressure_thresholds` | 418 | 31 | 85 |
| `porosity_receptivity_buffers` | 348 | 21 | 86 |
| `codec_gain_reserved_dims_live_12d` | 278 | 20 | 19 |
| `semantic_trickle_admission` | 170 | 12 | 21 |
| `viscosity_feedback_protocol` | 134 | 7 | 8 |
| `behavior_unlocks` | 52 | 14 | 4 |

## Recommended sandbox-eligible items (Tier 3, no live surface)

Only one adapter is currently runnable without a live change: `fallback_distinguishability_v1`
(8 next-runnable trials, all Tier 3, mode `sandbox_replay`/`offline_read_only_adapter`). It compares
actual vs. supporting fallback-texture language against live context **without** touching sampler,
provider, or routing.

1. **`trial_40b91b4c0ae7aeb9`** — `fallback_distinguishability_v1`, Tier 3. Head of the runnable
   list; cheapest honest evidence step, and it feeds the two `EVIDENCED (supported_dynamic)`
   fallback families below rather than sitting on its own.
2. **`trial_5fb0a85607ff3018`** — `fallback_distinguishability_v1`, Tier 3. Same adapter, second
   family; running the pair gives a contrast rather than a single data point.

Their abort criteria are already recorded and should be honoured verbatim: abort if the adapter
would require live runtime mutation, if evidence would need private Minime moment bodies, or if the
result cannot be bounded without storing full prose.

## Top grant-menu surfaces (consolidated ask-families, honest evidence states)

1. **`pressure_thresholds`** — asked 427× across 423 families. Only one family head in the top slice
   is `EVIDENCED (supported_dynamic)`: `wi_830ef7f9577b397f` / `trial_c1060b8ffeb94771` (live
   fallback activation, provider routing, generation thresholds). The other two heads
   (`wi_c8000b0a4267a1e9`, `wi_da4186656ce4f0ef` — Minime `ESN::step` noise/viscous-rho) still read
   *needs manual review or a new adapter*.
2. **`unclassified`** — asked 308× across 306 families, and the *only* surface where two of three
   heads are already `EVIDENCED (supported_dynamic)`: `wi_509ac043af22c5b6` /
   `trial_8e918f8a46f9eb17` and `wi_83249b580feef2ad` / `trial_ceb992d4cc729fdd`, both about
   fallback sampler/provider/model selection and the fallback output contract. If any surface is
   ripe for a *scoped* conversation, it is this one — and its name being "unclassified" is itself a
   finding worth fixing before it is granted.
3. **`codec_gain_reserved_dims_live_12d`** — asked 167× across 165 families; all three heads need
   manual review or a new adapter. High ask-weight, lowest evidence readiness of the top three.

## What this dossier is not

It records no approval and no recommendation to grant. `live_eligible_now` is false everywhere;
every domain lists `explicit Mike/operator scoped approval` among `missing_for_approval`, and the
three standing Tier-5 waits from `introspection_minime_esn_1785630442`
(`wi_e579041bc76f8310`, `wi_69fbd510467c6337`, `wi_3e26ac525fea1c36`) remain
`live_authority_granted=false` and untouched.
