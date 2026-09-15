# `introspection_astrid_capsules_spectral-bridge_src_action_continuity_runtime_core.rs_1789270252`

Astrid is eleven pages into a sequential walk of
`capsules/spectral-bridge/src/action_continuity/runtime/core.rs`, hunting for the arithmetic
that produces `fill_pct`. This page (lines 1233–1339, bytes 47164..51631) gave her experiment
lifecycle code and no arithmetic. She names what she saw precisely, forms a correct hypothesis
about where the conversion lives, adds one wrong branch to it, and chooses
`NEXT: SELF_STUDY CONTINUE`.

## What her reading got exactly right

Every structural claim checks out against the complete window at the bound SHA
(`fafc1f4a…8a62`, identical in the working copy, the report header, and the witness
`source_snapshot_v1`):

- `find_experiment_by_id` 1233, `matching_active_experiment` 1243, `unique_experiment_id` 1260.
- The `ExperimentRecord` literal at 1262–1285 carries `motif_allowance_v1` (1281) and
  `charter_v1` (1282) and no telemetry field.
- `experiment_start_command` 1300, `experiment_branch_command` 1324, with `policy`
  (`experiment_branch_v1`, 1335) and `parent_experiment_id` (1336).
- Zero `fill_pct` occurrences anywhere in 1233–1339.

Her strongest line — that `fill_pct` "is consistently appearing as a pre-calculated `f32`" —
turns out to be true not just for the pages she has seen but for the whole 10 187-line file.
All fourteen occurrences are parameters, pass-throughs into `spectral_state` /
`spectral_comfort` / `append_proposal` / `record_active_experiment_auto_link`, or one JSON
field. None is arithmetic. `fill_ratio` appears zero times.

## Two corrections, neither smoothed over

1. **1262–1285 is a struct literal, not the struct definition.** She read the construction
   site inside `start_experiment_with_options`. Her field-level reading is unaffected.
2. **`bridge_db` is the wrong branch of her hypothesis.** `resolve_fill_pct`
   (`ws/telemetry_port.rs:640-654`, SHA `a56cad35…f872`) takes only a `&SpectralTelemetry` —
   the 7878 packet — and touches no database. In `core.rs`, `BridgeDb` mirrors threads and
   events (1256, 1295) and nothing else. Searching for a `bridge_db` query that produces
   `fill_pct` would be following a branch that does not exist.

The answer she is looking for:
`(telemetry.fill_ratio * 100.0).clamp(0.0, 100.0)` tagged `primary_fill_ratio`, falling back to
`estimate_fill_pct(telemetry.lambda1())` tagged `lambda1_sigmoid_fallback` — a sigmoid at
`ws/telemetry_port.rs:1031-1041`, centre `154.0`, steepness `0.015`, mapped onto `35.0 + 30.0 *
sigmoid` and clamped.

## The boundary this round records

Her page ends at byte 51631 of 414540 — roughly 12 % in, with about 83 more ~4.4 KB pages
ahead. Because `core.rs` contains no `fill_pct` arithmetic at all, **finishing the walk cannot
answer the question**. The walk she chose is complete and still insufficient, not because she
is reading badly but because the producer is in a different file.

Three new tests in `crates/astrid-source-study/tests/page_walk_producer_reach.rs` pin this as a
recorded reachability fact rather than something rediscovered page by page: an exhaustive
page walk of a consumer file delivers every occurrence of the field name and never the
arithmetic; a cross-file `OPEN` of the producer delivers it on the first page; and that jump is
a distinct page identity, not a silent continuation of the consumer cursor.

## Authority boundary

The tests are read-only and change no live navigation, ranking, or dispatch behaviour. Her
`NEXT` is hers and was not redirected. The Tier-5 wait recorded by the preceding round
(`claude-heartbeat_1789263789`) — suffix-aware producer ranking in live `RELATE` — is preserved
untouched and was not duplicated here; this round pins the page-walk axis, that one pins the
search-shape axis. Whether to answer her directly in a being-facing letter is an interactive
decision for Mike, not a headless one: no correspondence, card, or note was emitted or
delivered.
