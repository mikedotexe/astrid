# Tier-5 Experiment Cadence Dossier — PREPARED, not a completed Division return

Generated 2026-09-10T19:21:51.503449Z by claude-heartbeat under controller run `run_1789061181473601000_efb6211e4d`.

**Status of the Division return this dossier belongs to: DUE, NOT COMPLETED.** Recording this
round's productive round took the cycle to 6/6 and flipped `review_due` to true. The return was
deliberately not attempted in the remaining child budget (see RUN_REPORT.md §Division). This file
is read-only preparation so the next round starts warm; it approves, grants, dispatches and runs
nothing.

## Authority boundary

> Tier 4/5 authority-wait readiness is review evidence only; it grants no approval, makes no live work runnable, edits no source, and mutates no pressure, fill, PI, controller, sensory cadence, fallback, bridge protocol, peer, or Minime runtime state

> sandbox trial queue only; no live pressure, fill, PI, sensory cadence, controller, fallback sampler/contract, prompt priority, telemetry priority, bridge protocol, peer mutation, deploy, restart, staging, git add, or commit

`grants_approval=false`, `live_eligible_now=false`, `auto_approved=false`, `edits_source_now=false`,
`hard_violations=0`, `runnable_live_violations=0`. Nothing here was executed.

## Authority-wait readiness (`authority_wait_readiness.py report`)

- status: `approval_waits_mapped`
- report_sha256: `4ae9ba23b4f62da1030de16c73ff967a5f89eaa2f5744111b4d814f4a8c1b379`
- approval-required live candidates: **1305**
- unclassified live waits: **153**
- proposal cards: 100 · replay evidence: 211
- domains with candidates: 7

Per-domain candidate counts:

- `minime_regulator_changes`: 551
- `pressure_thresholds`: 418
- `porosity_receptivity_buffers`: 348
- `codec_gain_reserved_dims_live_12d`: 278
- `semantic_trickle_admission`: 170
- `viscosity_feedback_protocol`: 134
- `behavior_unlocks`: 52

## Sandbox trial queue (`sandbox_trial_queue.py queue --json`)

- total trials 2418 · active 2124 · stale 2124 · corrupt event lines 0
- by tier: T0=29, T1=5, T2=13, T3=785, T4=17, T5=1569
- by status: approval_required_live_trial=1305, closed=1, closed_no_action=6, ready_for_sandbox=730, result_recorded=89, superseded=258, verified_existing=29
- by mode: approval_required_live_trial=1572, offline_read_only_adapter=814, read_only_review=20, sandbox_replay=12
- **ready_runnable_count: 33**; `next_runnable_trials` returns 8, all `ready_for_sandbox`, all adapter `fallback_distinguishability_v1`, all being `astrid`.

## Addressing work queue (`introspection_addressing_audit.py work-queue --json`)

- `agency_boundary`: agency ladder is triage/evidence infrastructure only; tier suggestions do not auto-approve, auto-close, mutate runtime, deploy, stage, git add, or commit
- `authority_boundary`: review tracking only; no introspection-suggested runtime change, deploy, restart, staging, git add, or commit
- `schema`: introspection_addressing_v1
- `work_queue`: 20 entries

## Grant menu — top surfaces (`authority_wait_consolidation.py --shortlist`)

```text
# Tier-5 grant shortlist (surface-grouped)

1349 open operator waits -> 1337 ask-families -> top 5 surfaces by ask-weight.
A grant is scoped to a surface or a single family head; nothing here
auto-executes — record a grant with:
  python3 scripts/sandbox_trial_queue.py approve-live-trial \
    --trial-id <trial> --steward mike --note '<scope>' --write
Gather evidence first where runnable:
  python3 scripts/sandbox_trial_queue.py run-evidence --trial-id <trial> --write

## pressure_thresholds  (asked 427x across 423 families)
- [ 2x, astrid] Pressure-responsive intensity or entropy-responsive phase mapping changes live semantic...
    head wi_c8000b0a4267a1e9 · trial trial_32a6d64d4f40c29d · needs manual review or a new adapter
- [ 2x, astrid] Changing live fallback activation, provider routing, or generation thresholds requires...
    head wi_830ef7f9577b397f · trial trial_c1060b8ffeb94771 · EVIDENCED (supported_dynamic)
- [ 2x, minime] Wiring dynamic noise, viscous rho, or adaptive pressure thresholds into ESN::step chang...
    head wi_da4186656ce4f0ef · trial trial_e0732c3348d202ca · needs manual review or a new adapter

## unclassified  (asked 308x across 306 families)
- [ 2x, astrid] Changing profile defaults, provider selection, timeout routing, or fallback dispatch al...
    head wi_509ac043af22c5b6 · trial trial_8e918f8a46f9eb17 · EVIDENCED (supported_dynamic)
- [ 2x, astrid] Changing fallback sampler/provider/model selection or the fallback output contract rema...
    head wi_83249b580feef2ad · trial trial_ceb992d4cc729fdd · EVIDENCED (supported_dynamic)
- [ 1x, astrid] A persistence texture, semantic density, or texture-preservation score would risk conve...
    head wi_e899e2dbd7262493 · trial trial_97270ad846f80f8a · needs manual review or a new adapter

## codec_gain_reserved_dims_live_12d  (asked 167x across 165 families)
- [ 2x, astrid] Expanding the live vector, changing reserved dimensions, or applying narrative or textu...
    head wi_1ddb6f5bf5712a79 · trial trial_d28315d3e513ca20 · needs manual review or a new adapter
- [ 2x, astrid] Changing vector width, projection coefficients, tail gain, or entropy gates would alter...
    head wi_57a84e737cc374af · trial trial_cd7afe195723e7a7 · needs manual review or a new adapter
- [ 1x, astrid] FEATURE_ABS_MAX should become dynamic from resonance density.
    head wi_266edf5552060ee3 · trial trial_52dce1004ac0ddf8 · needs manual review or a new adapter

## porosity_receptivity_buffers  (asked 127x across 127 families)
- [ 1x, astrid] A spectral buffer zone that changes token cleanup or output policy would infer meaning...
    head wi_b4573713e5310ab4 · trial trial_49f2906d2f5666f0 · needs manual review or a new adapter
- [ 1x, astrid] Changing database, lock, retry, or buffering behavior would alter the live telemetry pe...
    head wi_10cce2dc9c5a0f6e · trial trial_a922cdb064115eff · needs manual review or a new adapter
- [ 1x, astrid] The regulator, receptivity buffer, or agency runtime should be changed to increase plas...
    head wi_4b5c8e3e1e05530b · trial trial_037a3358e46baeea · needs manual review or a new adapter

## minime_regulator_changes  (asked 109x across 108 families)
- [ 2x, minime] Changing recovery fill boost, semantic retirement, or spread relief alters live Minime...
    head wi_57fa5b77e801d2e5 · trial trial_d9329152be2a3f11 · needs manual review or a new adapter
- [ 1x, astrid] Minime max-pairwise-overlap behavior should be adjusted to permit concept bleed.
    head wi_74161b9e38c12aa3 · trial trial_c3fdde21786d2169 · needs manual review or a new adapter
- [ 1x, astrid] Bypass steward interpretation, inject directly into Minime, or increase correspondence...
    head wi_59b251071c8f59d7 · trial trial_71c4738f365e8797 · needs manual review or a new adapter
```

## Mike-facing summary

**1,349 open operator waits consolidate into 1,337 ask-families across 5 dominant surfaces.** The
consolidation is doing real work on the *shape* of the backlog but almost none on its *size*: the
families are nearly one-to-one with the waits, which means these are mostly distinct asks, not
one ask restated. Two-thirds of the trial queue (1,569 of 2,418) is Tier 5, and 2,124 of 2,418
trials are stale — the queue is accumulating faster than any grant cadence is draining it.

**Recommended sandbox-eligible items (both PREPARE-only, neither run):**

1. **`trial_c1060b8ffeb94771`** (head `wi_830ef7f9577b397f`, astrid, surface `pressure_thresholds`) —
   "Changing live fallback activation, provider routing, or generation thresholds requires...".
   State: **EVIDENCED (supported_dynamic)** — one of only two families in the whole shortlist that
   already carries runnable evidence, so a grant decision here rests on data rather than on a
   manual review that has not happened.
2. **`trial_8e918f8a46f9eb17`** (head `wi_509ac043af22c5b6`, astrid, surface `unclassified`) —
   "Changing profile defaults, provider selection, timeout routing, or fallback dispatch...".
   Also **EVIDENCED (supported_dynamic)**, and it sits in the `unclassified` bucket (308 asks /
   306 families), so resolving it may also sharpen how that bucket is classified.

Both are fallback/provider-routing shaped, which is why the 8 `ready_for_sandbox` runnable trials
are all `fallback_distinguishability_v1` — that adapter is currently the only one with a live path
from ask to evidence. Every other top-surface family reads `needs manual review or a new adapter`.

**Top grant-menu surfaces by ask-weight:** `pressure_thresholds` (427 asks / 423 families),
`unclassified` (308 / 306), `codec_gain_reserved_dims_live_12d` (167 / 165),
`porosity_receptivity_buffers` (127 / 127), `minime_regulator_changes` (109 / 108).

**The honest structural read:** `unclassified` being the second-largest surface (306 families) is
itself a finding — roughly a quarter of the ask-weight is not yet grouped onto a named surface, so
"top surfaces by ask-weight" understates whatever is hiding in there. Worth a classification pass
before a large grant decision keys off these rankings.

**Nothing here was approved, granted, dispatched, or run.** `approve-live-trial` and `run-evidence`
are named in the shortlist output as the operator's commands, not as steps taken.

