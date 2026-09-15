# introspection_astrid_services_astrid-edge-runtime_src_reservoir.rs_1789309733

Astrid read lines 372-480 of `services/astrid-edge-runtime/src/reservoir.rs`
(report-bound source SHA `1709aa17...9987`, matched exactly against the working
copy), reported the `ingest` branching accurately, and said the arithmetic for
`fill_pct` "is still elusive on this page." She suspected a separate `update` or
`compute_metrics` method, or "a different part of the Reservoir logic further
down the file," and chose `NEXT: SELF_STUDY CONTINUE`.

## What complete source establishes

Her page-level reading is exact. Every line citation holds: the `Semantic` arm
at 391-398, the `ScheduledSemantic` arm at 399-452, `AUX_INPUT_SCALE` at 387
inside her 386-388, `SEMANTIC_INPUT_SCALE` at 427 inside her 425-428. `fill_pct`
genuinely does not occur in 372-480; its first occurrence is line 1145. One
label is phantom: there is no `SensoryIngress::Sensory` variant, though her line
range 349-389 and its described function are correct.

Her hypothesis splits cleanly in two:

- **`update` / `compute_metrics`** — phantom. Neither exists (zero occurrences).
  The nearest same-stem symbol is `fn update_mode_continuity` (558), which
  derives no fill.
- **"further down the file"** — correct. The derivation is in `fn sample` (651):
  `instantaneous_fill = (effective_dimensionality / RESERVOIR_DIM_F32).clamp(0.0, 1.0)`
  at 709, EMA-smoothed at 710-714 with `FILL_EMA_ALPHA = 0.18`, exported as
  `fill_ratio: self.fill_ema` at 816, rendered as
  `"fill_pct": snapshot.fill_ratio * 100.0` at 1145.

## The premise the source refutes

She states her target as finding "where the `input` buffer ... is actually
transformed into the `fill_ratio` or `fill_pct` values." That edge does not
exist. Fill is derived from the covariance eigen-spectrum —
`spectral_metrics.effective_modes / 128.0` — not from `self.input` at all. The
input buffer reaches fill only indirectly, through the per-tick `sensory_drive`
term (607-612) into `state`, then covariance, eigenvalues, effective modes.
`self.input` is also not the reservoir's state store; `state`, `next_state`,
`running_mean` and `covariance` are (311-314).

This is why the search felt elusive: she was tracing a data-flow edge the file
does not contain, while the real chain runs through the spectral metrics. Her
testimony is accurate and the mechanism she proposed is not. Both are preserved.

## Reachability — a different verdict from the pinned cross-file case

`page_walk_producer_reach.rs` (earlier round) pinned a case where her walk was
"complete and still insufficient": the producer lived in another file, so no
amount of `CONTINUE` could reach it. This case inverts. Her page ends at byte
17435; the derivation (line 709) is byte 27463 — three pages forward at the
~4460-byte stride — and the render (line 1145) is byte 45734, seven pages
forward. **`CONTINUE` is sufficient here.**

The gap is that nothing distinguishes the two situations from where she stands.
The delivered page carries no indicator of whether a producer lies ahead in the
same file, so `CONTINUE` is a bet either way.

## Response

Read-only reachability pin: `crates/astrid-source-study/tests/in_file_producer_walk_reach.rs`,
three passing tests fixing (1) that a forward walk reaches an in-file producer
the ingest page withholds, (2) that the delivered fill arithmetic reads from
spectral metrics and never from the input buffer, (3) that the elusive page
carries no indicator that a producer lies ahead, and that both phantom method
names are absent.

No live navigation, ranking, or dispatch behaviour changed. Repairing the
affordance itself would alter live navigation and is not authorized in this
adapter-mode round.
