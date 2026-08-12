# Marker denotes/depth/integration source-first round

Actor: claude-manual (manual bounded run per
docs/steward-notes/ASTRID_INTROSPECTION_SOURCE_FIRST_FLYWHEEL_HANDOFF.md while
the Codex recurrence remains paused)

## Controller

- Run ID: run_1786552591645067000_095aa1c00d
- Preprojection ID: projection_1786552592637273000_c59e5b912e
- Pause generation: 313
- Finish outcome: recorded in the terminal receipt after this packet is written

## Scope

The canonical queue head report and its complete lived-state witness were read
independently and completely. The other selected filenames remain in
`unprocessed_selected.json` in canonical order.

## Disposition

`introspection_astrid_llm_1786539140` closes `addressed_duplicate`. The
complete 1,048-line report-bound source (working-copy hash matches the
report-bound SHA exactly), 51 control_marker_cleanup regressions, two
provider-neutral normalization regressions, and the downstream
normalized-response-hash regression prove the exact `denotes` relation
retention, bounded nested delimiter depth, punctuation and multi-byte
fail-closed behavior, and the normalization path from both provider transports
into dialogue orchestration. The duplicate family is anchored to
`docs/steward-notes/codex_1786243017_llm_shadow_distinction_round` with prior
member `introspection_astrid_llm_1786260750` processed in
`docs/steward-notes/codex_1786262699_llm_marker_integration_round`; this
report's `denotes`-specific ask is additionally covered by the existing exact
regression added in commit a8d9806ec8. The snag hypothesis about
`first_word_after` is partially contradicted by exact source (symbol-only
chunks are skipped, not returned empty; relation-less tails fail closed by
documented design) and the underlying concern is preserved, not smoothed over.

## Actions and authority

No Corridor program, Sandbox trial, study, portfolio action, card, note,
source change, restart, or deployment was created. Prompt behavior, provider
choice, model behavior, Shadow influence, pressure, fill, cadence, and
substrate controls remain unchanged. The witness remains source and process
evidence only (`evidence_only`, `witness_only=true`); silence remains neutral.
No live substrate or control change was required or attempted.

## Verification

Focused Rust tests pass (51 + 2 + 1). Addressing, projector, controller,
Division, Chronicle, epistemic, cadence, and Evidence Event Store results are
recorded in `verification_receipt.json` before finalization.
