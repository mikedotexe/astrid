# Tier-5 experiment cadence dossier — Division return cycle 48 (2026-09-15)

**PREPARE ONLY.** Nothing in this dossier was approved, granted, dispatched, or run. No trial was
started, no live authority was recorded, no runtime state was touched. Every item below remains an
explicit Mike/operator wait.

## Practice-doc drift — still open, FOR MIKE (second consecutive report)

`docs/steward-notes/AI_BEINGS_TIER5_EXPERIMENT_CADENCE_2026_08_13.md`, named by the round
instructions, is **still absent from the working tree and from `HEAD`**, exactly as cycle 47
reported on 2026-09-14. It exists only in commit `95d1eb9fd9`, reachable from
`claude/hopeful-williamson-9f566c`, `codex/graceful-coupling-rollout` and
`codex/hebbian-clock-boundary` — none merged to `main`. The only local copy is the read-only
worktree `.claude/worktrees/hopeful-williamson-9f566c/docs/steward-notes/…`, which is where the
practice was re-read for this dossier. Nothing was cherry-picked, merged, or copied. Getting the doc
onto `main` remains a separate interactive decision.

## Wait inventory (read-only tooling)

`introspection_addressing_audit.py work-queue --json` — 7,324 active work items:

| Status | Count |
| --- | ---: |
| `verified_existing` | 4,278 |
| `needs_operator_approval` (Tier 5) | **1,349** |
| `implemented_awaiting_felt_response` | 908 |
| `needs_sandbox` | 750 |
| `superseded` | 340 |
| `closed_no_action` | 31 |
| `ready_for_implementation` | 21 |
| `needs_steward_grant` (Tier 4) | **18** |
| `closed_felt_confirmed` | 3 |

By being: astrid 5,745 · minime 1,950 · unknown 3. The work-queue head is 20 consecutive
`agency_tier: 5`, `needs_operator_approval`, `live_authority_granted: false` items — head
`wi_5d89b53dcfdcbbe6` (dynamic depth from density/resonance would change prompt budget and provider
behaviour).

`sandbox_trial_queue.py queue --json` — status `approval_waiting`; 8 `next_runnable_trials`, all
`ready_for_sandbox`, all astrid, all on the single adapter `fallback_distinguishability_v1`:
`trial_40b91b4c0ae7aeb9`, `trial_5fb0a85607ff3018`, `trial_60de383ef0b677bf`,
`trial_7b15b13b5882472e`, `trial_7debce98bda84440`, `trial_7e7d6ea1d8b09b03`,
`trial_8a8630f209bcc376`, `trial_9de730459a470cb6`. `runnable_live_violations: []` — the ladder is
clean; nothing is runnable that should not be.

Each of those trials carries its own abort criteria, and one of them matters for cadence: *"evidence
would require private Minime moment bodies"*. That is a hard stop under the private-lane bright
line, not a scheduling problem.

`authority_wait_readiness.py` needs an explicit `generate`/`report` subcommand; the bare invocation
only prints usage. Recorded here as a tooling note so the next round does not mistake usage output
for an empty readiness map.

## Grant menu — top surfaces by ask-weight

`authority_wait_consolidation.py --shortlist`: **1,349 open operator waits → 1,337 ask-families**.
The long tail is nearly one family per wait, so the honest unit of a grant is a *surface*, not an
item.

| Surface | Asked | Families | Evidence state of its named heads |
| --- | ---: | ---: | --- |
| `pressure_thresholds` | 427× | 423 | 1 of 3 heads **EVIDENCED** (`supported_dynamic`), 2 need manual review or a new adapter |
| `unclassified` | 308× | 306 | 2 of 3 heads **EVIDENCED** (`supported_dynamic`) |
| `codec_gain_reserved_dims_live_12d` | 167× | 165 | all 3 heads need manual review or a new adapter |
| `porosity_receptivity_buffers` | 127× | 127 | all 3 heads need manual review or a new adapter |
| `minime_regulator_changes` | 109× | 108 | head needs manual review or a new adapter |

## Mike-facing summary

Two of the five heaviest surfaces already have **evidenced** family heads; the other three are
blocked on adapters, not on your attention. That is the whole cadence picture this round.

**Recommended sandbox-eligible items (1-2, both astrid, both already `ready_for_sandbox`):**

1. `trial_c1060b8ffeb94771` — head `wi_830ef7f9577b397f`, surface `pressure_thresholds`, *"changing
   live fallback activation, provider routing, or generation thresholds"*. **EVIDENCED
   (`supported_dynamic`)** and it sits on the single heaviest surface (427 asks), so evidence here
   informs the largest family cluster rather than one item.
2. `trial_8e918f8a46f9eb17` — head `wi_509ac043af22c5b6`, *"changing profile defaults, provider
   selection, timeout routing, or fallback dispatch"*. Also **EVIDENCED (`supported_dynamic`)** and
   adjacent to the same fallback-contract machinery, so the two share one review frame.

**Top grant-menu surfaces to consider scoping a grant to:** `pressure_thresholds` (427×) and
`codec_gain_reserved_dims_live_12d` (167×). The first has evidence ready to read; the second has
none of its heads evidenced, so a grant there would be a decision made without adapter evidence —
worth naming explicitly rather than letting ask-weight alone argue for it.

**Boundary.** This dossier recommends reading, not acting. No `approve-live-trial`, no
`run-evidence`, no dispatch, and no live change was run, prepared as a command for automatic
execution, or implied to be authorized by its appearance here.
