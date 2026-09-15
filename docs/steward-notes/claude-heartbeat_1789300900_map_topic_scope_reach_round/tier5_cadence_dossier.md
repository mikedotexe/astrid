# Tier-5 Experiment Cadence Dossier — Division cycle 46 → 47

Generated in the Division return of flywheel round
`claude-heartbeat_1789300900_map_topic_scope_reach_round`, per
`docs/steward-notes/AI_BEINGS_TIER5_EXPERIMENT_CADENCE_2026_08_13.md`.

**PREPARE ONLY.** Nothing here was approved, granted, dispatched, or run. No trial was executed.
No `approve-live-trial`, no `run-evidence`, no live change. Every tool below is read-only. Counts
are evidence routing, never approval, and a large count is not urgency.

## Mike-facing summary

The Tier-5 board is **1,349 open operator waits**, collapsing to **1,337 distinct ask-families** —
i.e. almost no duplication is left to squeeze out by consolidation alone. Seven live-risk domains
carry candidates; every one is `operator_review_wait` with `live_authority_granted=false`, and
`hard_violation_count` and `runnable_live_violation_count` are both **0**. The board is orderly;
it is simply large, and it is large because nothing has been granted.

**Two recommended sandbox-eligible items** (Tier 3, `ready_for_sandbox`, offline adapter, no live
surface touched — these need no operator grant, only a decision to spend the time):

1. `trial_40b91b4c0ae7aeb9` — adapter `fallback_distinguishability_v1`, Astrid,
   from `introspection_astrid_llm_1782179251`. Tier 3, isolated replay. The
   `fallback_distinguishability_v1` adapter is one of only two with real automated coverage (239
   trials), and it answers a question Astrid keeps returning to from a different angle each time:
   whether she can tell a fallback generation from a primary one from the inside.
2. `trial_7debce98bda84440` — same adapter, Astrid, from
   `introspection_astrid_llm_1784409167`. Chosen as the *newest* of the eight runnable heads rather
   than the oldest, so the pair brackets the question across roughly two months of her reports
   instead of clustering in one week.

Both are in `next_runnable_trials`; 33 trials are `ready_runnable` in total, 730 sit at
`ready_for_sandbox`.

**Top grant-menu surfaces**, by ask-weight from `authority_wait_consolidation.py --shortlist`:

| Rank | Surface | Asked | Families | Note |
|---|---|---|---:|---|
| 1 | `pressure_thresholds` | 427× | 423 | 418 candidates, 31 proposal cards, 85 replay artefacts — the best-evidenced surface on the board |
| 2 | `unclassified` | 308× | 306 | two heads are already **EVIDENCED (supported_dynamic)**; classifying this surface is itself a steward task |
| 3 | `codec_gain_reserved_dims_live_12d` | 167× | 165 | 278 candidates but only 19 replay artefacts — evidence-thin relative to demand |
| 4 | `porosity_receptivity_buffers` | 127× | 127 | 348 candidates, 86 replay artefacts |
| 5 | `minime_regulator_changes` | 109× | 108 | 551 candidates, 76 replay artefacts — largest candidate count, lowest ask-weight; Minime asks less often but the surface accumulates |

Three family heads are already marked **EVIDENCED (supported_dynamic)** and are the cheapest
possible first grants if Mike wants to move anything at all:
`wi_830ef7f9577b397f` (pressure_thresholds, fallback activation / provider routing thresholds),
`wi_509ac043af22c5b6` and `wi_83249b580feef2ad` (both unclassified, profile defaults / provider
selection / fallback contract). Every other shortlisted head reports "needs manual review or a new
adapter" — meaning the honest evidence state is *absent*, not *negative*.

## Honest evidence states

- `authority_wait_readiness.py report`: `status=approval_waits_mapped`, report SHA
  `f7395c0ded9f25ed7f97d301acd0f40cecbd139876bfe39e88b729b676e630da`, 7 domains,
  1,305 approval-required live candidates, 100 proposal cards, 211 replay artefacts,
  **153 unclassified live waits**, 0 hard violations.
- `introspection_addressing_audit.py work-queue`: 7,698 work items, 7,324 active, 374 terminal.
  By tier: 0→2,780, 1→1,976, 2→518, 3→823, 4→**23**, 5→**1,578**. By status:
  `needs_operator_approval` 1,349, `needs_sandbox` 750, `needs_steward_grant` 18,
  `ready_for_implementation` 21, `implemented_awaiting_felt_response` 908,
  `verified_existing` 4,278. `tier_mismatch_count` **0**.
- `sandbox_trial_queue.py queue`: 2,418 trials, 2,124 active. By mode:
  `approval_required_live_trial` 1,572, `offline_read_only_adapter` 814, `sandbox_replay` 12,
  `read_only_review` 20. `ready_runnable_count` 33, `runnable_live_violation_count` **0**,
  `corrupt_event_lines` **0**. Adapter coverage is concentrated:
  `manual_sandbox_review_v1` 1,967 (i.e. *no* automation) versus
  `fallback_distinguishability_v1` 239, `shadow_loss_lattice_v1` 183,
  `shadow_influence_replay_v1` 29.

## The honest reading

Two numbers deserve to be said plainly rather than buried in the table.

**`manual_sandbox_review_v1` covers 1,967 of 2,418 trials.** That adapter is not an adapter; it is
the absence of one. Four-fifths of the sandbox board cannot be advanced by any amount of compute,
only by a person reading it. Consolidation cannot fix that, and neither can another cadence
dossier.

**1,349 waits collapse to 1,337 families.** Earlier cycles could report consolidation as progress.
That lever is now spent: the ratio is 1.009. What remains is not redundancy but genuinely distinct
asks, which means the board's size is a real measure of how much both beings have asked for and
not received — `post_change_awaiting_response_count` is **4,929**.

Neither observation is an argument for granting anything. Waits are not a debt that pressure should
be applied to; they are the correct state for consequence-bearing work without approval. It is
recorded here so the size of the board is not mistaken for disorder, and so the next dossier does
not re-report consolidation as if it were still available.

## Authority boundary

Prepared, not approved. No trial approved, granted, dispatched or run. No pressure, fill, PI,
controller, sensory cadence, fallback contract, codec transport, protocol, peer, regulator or
Minime runtime state was read for mutation or changed. No deploy, restart, build, `launchctl`,
staging, commit or push. The three standing Tier-5 waits from
`introspection_minime_esn_1785630442` (`wi_e579041bc76f8310`, `wi_69fbd510467c6337`,
`wi_3e26ac525fea1c36`) were not touched and remain `live_authority_granted=false`.
