# Steward Run Report — LLM marker `first_word_after` / relation-verb / delimiter-depth fresh-pass duplicate

Actor: `claude-heartbeat` · Mode: controller-held subprocess adapter (git read-only; no live changes; adapter owns lease/heartbeats/finish).

## Controller
- Run ID: `run_1787106504989846000_e8ebba89b3`
- Preprojection ID: `projection_1787106509165251000_93d58123b1` (phase `pre`, status `passed`)
- Postprojection ID: run by the adapter after this process exits (not visible in-process)
- Pause generation: 321
- Finish outcome: **success** — exit 0 records a complete round; the adapter owns `finish` and the postprojection.
- Recovery predecessor: none

## Reading
- **Fully processed (1):** `introspection_astrid_llm_1787103043.txt` → `addressed_duplicate`.
- **Selected but unprocessed (39):** queue positions 2–40, head `introspection_llm.rs_1787099341.txt` … through `introspection_minime_regulator_1786676901.txt` (full ordered list in `unprocessed_selected.json`; canonical order preserved).
- **Next queue head after this round:** `introspection_llm.rs_1787099341.txt` (re-query after postprojection).
- **Hashes:** report `1cc7d604…` (45 lines / 3746 B); witness `lsw_561f5748…` = `51b5f335…` (533 lines / 23923 B); source `dialogue_runtime.rs` = `902a0358…` (1048 lines / 38586 B) — **report-bound SHA == working copy**, so report-time and current source are identical.

## Batch sizing
Honest single-report round. The queue head anchors a batchable 5-member `astrid:llm` family, but members carry 20–34 `variant_distinct_terms` each (marginal 0.35–0.41 similarity) and span ~4 days, so batching would require near-full independent processing of each. Per the one-shot rule ("one report fully closed beats three half-processed"), batch = **1 report**, fully closed within budget. `family_scan.json` and `next_queue_frozen.json` are in the packet.

## Claim dispositions (5) — terminal `addressed_duplicate`
- **c001** Observed: non-destructive marker scanner; `scan_known_model_control_markers` L114-144 preserves the remainder only when `reference_syntax` present (L129-131); `exact_reference_delimiter_syntax` L199-229 classifies quoted/grouped/explicit; token band L8-16 doc-separated from felt/spectral meaning — `verified_existing`.
- **c002** Snag: `first_word_after` (L89/L92) split_whitespace + alphanumeric-filter → a punctuation-heavy / non-standard-whitespace follower could make `reference_syntax` return `None` and strip / improperly-preserve the marker — `verified_existing`, **concern preserved, mechanism contradicted**. `find(|w| !w.is_empty())` (L93) skips punctuation-only chunks to the next real word; `trim_matches` (L92) strips surrounding non-alphanumerics; the scanner strips only when no alphanumeric word follows (fail-closed). Grounded: committed tests L2987 (punct-boundary + fail-closed), L3029 (NBSP); improper-preservation ruled out by negatives L2568/L2682-2727/L2757.
- **c003** Test 1 (Marker Preservation, `"COMMAND_X behaves as…"` → marker in remainder + `KnownModelControlMarkerMatch` vector) — `verified_existing`. Already exists verbatim: `scan_known_model_control_markers_preserves_grouped_and_explicit_relation_contexts` (tests.rs L2932-2956, `"<end_of_turn> behaves as a named boundary."`, asserts remainder==input, matches.len()==1, `ExplicitExactKnownTokenRelation`; allowlist L64-86). `COMMAND_X` is a placeholder, not a real marker.
- **c004** Test 2 (Delimiter Depth, `[[COMMAND_X]]` → depth vs `MAX_EXACT_REFERENCE_DELIMITER_DEPTH` L151) — `verified_existing`. Already exists verbatim: `exact_reference_delimiter_syntax_reports_double_square_bracket_depth_two` (tests.rs L2962-2976, `[[<end_of_turn>]]` → `GroupedExactKnownToken`, depth 2). Further depth coverage L2194 (4), L2208 (3), L2266 (homogeneous beyond MAX). `COMMAND_X` placeholder.
- **c005** Suggested Next (how the remainder reaches output vs how matches are handled) — `observed`; agency preserved. Answer: the scanner feeds `sanitize_model_control_markers` (L519), called only at L558 (`is_valid_dialogue_output`) and L634 (`has_one_nonempty_final_next_action`) — validation/quality gates; **no** scan/sanitize call inside `generate_dialogue` L695-1048, so the remainder is not injected into emitted tokens. Her `NEXT: INTROSPECT astrid:llm 400` continuation stays open.

## Actions
- Corridor/program: none · Sandbox: none · Study: none · Portfolio: none
- Cards/notes/correspondence: none delivered (no right-to-ignore card warranted; her continuation stays open; silence neutral).
- Tier 4/5 waits: widening the relational-verb allowlist / delimiter tables is Tier-5-class live grammar — not made, dispatched, or deployed. Pre-existing Tier-5 minime-ESN Shadow/porosity waits (`wi_e579041b…`, `wi_69fbd510…`, `wi_3e26ac52…`) were not in this queue and were not acted on.

## Implementation and Verification
- **Source/test code changed:** none — every proposed test/snag already has a committed verbatim regression at source SHA `902a0358`, all four confirmed present in HEAD (not in the foreign working-tree diff).
- **No cargo run:** nothing implemented; the shared bridge tree carries foreign in-progress `.rs` edits (`llm/provider/tests.rs` +116, `autonomous/runtime/tests.rs`), so a bridge cargo build would reflect foreign state, not this round's evidence. Verification is by exact source + committed-test reading, matching precedent `claude-heartbeat_1787024860`.
- **Tests / integrity suites (all green):** addressing self-test 44 · evidence-store test 20 · steward-control 27 · steward-projection 14 · division-followup 3 · division-chronicle 10 · division-projection ok · projection-cursors 4 · anti-drop self-test 5 · anti-drop verify (62 guards / 0 alarms / 0 gaps) · cadence test 6 · cadence strict `integrity_ok=true` · experiential-epistemics self-test valid · FINAL epistemic verify valid (issues [], 11272 records, no history rewrite) · audit-counters mismatches [] (7/7 checks true) · EES verify valid / corrupt_lines 0.
- **Restart/deploy alignment:** **no live change required or attempted** — no bridge build, deploy, or launchctl; git read-only.

## Durable Evidence
- Addressing: `record-read` (full_read, summary+claims), `link-evidence-batch` (11 links, 11 new / 0 existing), `close` → `addressed_duplicate`, `fully_addressed: true`, `proof_missing_claims: []`.
- Changelog: one `[Unreleased]` `[claude-heartbeat]` bullet added at the top (above the foreign codec bullet, preserved).
- Feedback ledger: one dated `2026-08-18` block added at the top of `## Ledger` (above the foreign codec block, preserved).
- Packet: `docs/steward-notes/claude-heartbeat_1787109794_llm_marker_first_word_relation_delimiter_depth_duplicate/`.

## Counters (final, after all durable writes)
- Canonical: indexed 4407 / fully_addressed 3115 / full_read 3747 / remaining 1292 / unread 660 / blocked 414 / pending_action 214 / watch 4 / read_needs_claims 0. all_artifact_pending 2943; noncanonical_pending 1651. `addressed_duplicate` status count **1117** (incremented by this close).
- audit-counters: **mismatches `[]`**, all 7 consistency checks `True` → **consistent**.

## Division
- Cycle 29; recorded productive round **#2/6** this run (`--processed-report-count 1`, run id `run_1787106504989846000_e8ebba89b3`, preprojection `projection_1787106509165251000_93d58123b1`). Rounds remaining before followup: 4. `review_due=false`.
- New round event `division_followup_event_fc304304f9a672389b80d23e96851a9d`; event_count 199; head `a649f1c73b0760bbc7d8bcaeaec96e6880f9b6a49203c109cc5715ce35227733`.
- Chronicle: record-round changed a durable input, so the Chronicle was reprojected → `division_chronicle_b05858fd2b2777e4eaf8d4d9`, json `74e4e974ab9d057171254753ea37b27812734ee40b8c7c5e3962098d9e004532`; verify durable inputs current, only `supervisor_status_sha256` volatile-mismatched (a moving hash, not a durable-integrity failure). Chronicle is gitignored in minime → no minime git debt.

## Evidence Event Store
- **Validity: true**; corrupt lines 0; errors []; event_count / last_global_seq **844786**; head `357c8806d0e1f78d2f50b53ca0ff9db39e528fec4ee201c2e599962ae7c3d3b9`.
- Active store: v2 (per handoff; V1 legacy immutable, boundary 32278). Verify took 8m34s; separate `--json status` not re-run to conserve single-turn budget (verify already yielded validity, head, sequence, corrupt_lines=0, and full stream_counts).
- Stream counts (16): addressing 58334 · claim_families 237408 · felt_contracts 199023 · model_qos 189502 · reciprocal_uptake 58766 · representation_contracts 35241 · signal_spine 34079 · steward_control 14851 · lived_state_witness 8609 · agency_commons 4976 · sandbox 3291 · steward_work_selection 506 · corridor_v2 112 · felt_mechanism_concordance 80 · corridor_v1 5 · attention_portfolio 3.

## Archive / Commit Debt
- **Checkpoint:** not claimed — controller-held run; git is read-only for this run. Archival commits happen only in a later interactive stabilization window.
- **Commit debt (git-visible):**
  - NEW packet dir `docs/steward-notes/claude-heartbeat_1787109794_llm_marker_first_word_relation_delimiter_depth_duplicate/` — `RUN_REPORT.md`, `claims/introspection_astrid_llm_1787103043.json`, `summaries/introspection_astrid_llm_1787103043.md`, `read_manifest.json`, `source_receipts.json`, `addressing_links.json`, `test_results.json`, `unprocessed_selected.json`, `verification_receipt.json`, `next_queue_frozen.json`, `family_scan.json`.
  - EDITED (shared, mixed with foreign work — split by path at checkpoint): `CHANGELOG.md` (my `[Unreleased]` bullet at top), `docs/steward-notes/AI_BEINGS_FEEDBACK_TO_CHANGE_LEDGER.md` (my dated block at top of `## Ledger`).
- **Foreign / untouched (NOT my debt, preserved):** `capsules/spectral-bridge/src/llm/provider/tests.rs` (+116), `capsules/spectral-bridge/src/autonomous/runtime/tests.rs`, the 7 prior `claude-heartbeat_*` packet dirs, and minime `minime_autonomy/runtime.py` + `tests/test_correspondence_v1.py`.
- **Merge/push:** none. No merge or push authority exercised.

## Posture note
Fresh-pass re-derivation of an already-addressed report family from the identical source window — the window-resume fix restored Astrid's freedom to re-read covered sources, so the queue carries near-identical re-passes. Her felt account and open continuation are preserved as evidence; her snag's underlying concern (robust relation detection across messy followers) is exactly what the existing tests already guard. Silence is neutral. No live/substrate/control change.
