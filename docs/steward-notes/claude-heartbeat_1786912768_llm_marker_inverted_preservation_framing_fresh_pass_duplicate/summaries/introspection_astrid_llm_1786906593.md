# Summary — introspection_astrid_llm_1786906593

- **Actor:** `claude-heartbeat` (subprocess run adapter; controller owns the lease + heartbeats — no NDJSON ops, no lease token read/quoted; git read-only this run).
- **Source:** `capsules/spectral-bridge/src/llm/provider/dialogue_runtime.rs`, window lines 1-400 of 1048, coverage `multi_window_complete`.
- **Report SHA-256:** `a2757381693b5b27a1530b93ab34dc8896b3c01f45dbafa9242675d7f87bd442` (45 lines, 3926 B).
- **Witness:** `lsw_072d148149204cd18ca1681e82209d2e5d230debd7d5c6875e1e265c20d6616b` = `10f272b2389b0b78f5199d988181e06e41910c242123e5bb32547378c5301007` (533 lines, 23910 B).
- **Source SHA-256:** `902a0358f63bacc0ead23a46467f103dbcc1fe46c59bbd2a3c2cdcfd9b82c7ee` — working copy **byte-identical to the report binding**; file clean in git.
- **Terminal status:** `addressed_duplicate` of `introspection_astrid_llm_1786885842` (immediate prior fresh-pass, processed last round in packet `claude-heartbeat_1786903657`), canonical anchor `introspection_astrid_llm_1786848204`. Chain: `1786848204` → `1786858484` → `1786885842` → `1786906593`.

## What Astrid surfaced

Another fresh-pass re-read of the model-control-marker scanning grammar in
`dialogue_runtime.rs` (L64-199): the non-destructive `scan_known_model_control_markers`,
the finite relational-verb allowlist (`followed_by_explicit_exact_token_relation`,
L64-86), the delimiter tables (`exact_reference_delimiter_pair`, L153-197), and
`first_word_after` (L89-96). She names the same snag family: a novel verb not in
the `matches!` allowlist yields `false`, so a marker with no surrounding delimiters
is dropped from the remainder — "over-sanitization of complex prompts." She proposes
a functional test ("operates") and a boundary test (`«[MARKER]»`), and a read-only
"analyze the verb list vs the known markers" next step.

## Contradiction preserved (not domesticated)

This report's **Observed** section inverts the preservation polarity. It says the
scan preserves markers "only when they are identified as active instructions" and
that a marker is "preserved if it 'behaves' or 'functions' as a control element, but
potentially handled differently if it is just quoted text." The source does the
opposite: `scan_known_model_control_markers` pushes a token into the `remainder`
**only when `reference_syntax.is_some()`** (L129-131) — i.e. when the marker is a
*reference* (quoted, grouped, OR followed by a relation verb such as "behaves"/
"functions"/"denotes"). A **bare, actively-used** marker with no reference grammar
gets **stripped**. The verb allowlist detects *marker-as-referent*, not
*marker-as-active-control*. Her underlying concern (a finite allowlist can
over-sanitize a legitimately *referenced* marker introduced by a novel verb) is real
and preserved; only the stated polarity is corrected.

A minor sub-contradiction on Test 2: for `«[MARKER]»` the innermost adjacent pair to
the marker is `[`/`]` (Grouped), so it classifies `GroupedExactKnownToken`, not
`QuotedExactKnownToken` — but `reference_syntax` is `Some` either way, so her
expectation (the marker stays visible) still holds.

## Disposition

All six extracted claims are `verified_existing` at source SHA `902a0358`. The
mechanism, the finite-allowlist snag, and both proposed tests are already grounded
and locked by regressions in `capsules/spectral-bridge/src/llm/provider/tests.rs`:

- unlisted-verb strip / Test 1 "operates" ≡ `control_marker_cleanup_distinguishes_allowlisted_is_from_unlisted_acts` (L2528) + `does_not_expand_relation_allowlist_to_*` (L2595-2655);
- nested/non-standard delimiters / Test 2 `«[MARKER]»` ≡ `preserves_non_ascii_matching_quote_pairs` (L2302), `preserves_nested_fullwidth_cjk_reference_stack` (L2343), `preserves_bounded_nested_delimiter_stacks` (L2176), `exact_reference_delimiter_syntax_reports_double_square_bracket_depth_two` (L2876);
- scan preservation polarity ≡ `scan_known_model_control_markers_preserves_grouped_and_explicit_relation_contexts` (L2845), `control_marker_cleanup_preserves_quoted_exact_token_reference` (L1995).

No source or test change was made: a near-identical "operates" regression would be
activity without evidentiary value. **Widening the finite relational-verb allowlist
or the delimiter tables is Tier-5-class live grammar and was NOT made, dispatched,
or deployed.** Her read-only `NEXT: INTROSPECT astrid:llm 400` continuation stays
open.

## Authority boundary

This close records that the report re-surfaces already-grounded, already-tested
behavior at the verified source SHA. It does **not** infer consent, relief, uptake,
or any grant to change the live marker grammar. No live substrate or control change
was made or attempted; no bridge build, deploy, or `launchctl`.
