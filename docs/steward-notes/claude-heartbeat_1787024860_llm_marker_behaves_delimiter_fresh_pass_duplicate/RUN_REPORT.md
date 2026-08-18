# Steward Run Report — llm marker `behaves`/delimiter fresh-pass duplicate

Actor: `claude-heartbeat` · Mode: controller-held subprocess adapter (git read-only; no live changes; adapter owns lease/heartbeats/finish)

## Controller
- Run ID: `run_1787022521645062000_0f6ab52817`
- Preprojection ID: `projection_1787022525424318000_7f944988b2` (status `passed`)
- Postprojection ID: run by the adapter after this process exits (not visible here)
- Pause generation: 319
- Finish outcome: **success** — exit 0 records a complete round; the adapter owns `finish` and the postprojection.
- Recovery predecessor: none

## Reading
- **Fully processed (1):** `introspection_astrid_llm_1787014729.txt` → `addressed_duplicate`
- **Selected but unprocessed (39):** queue positions 2-40, head `introspection_DOMAIN_BOUNDARIES.md_1787003465.txt` … through `introspection_astrid_llm_1786921967.txt` and beyond (full ordered list in `unprocessed_selected.json`; canonical order preserved).
- **Next queue head after this round:** `introspection_DOMAIN_BOUNDARIES.md_1787003465.txt` (re-query after postprojection).
- **Hashes:** report `b7d16863…2e93` (45 lines / 3853 B); witness `lsw_770b39df…2bab7` = `5649ad44…9f55` (533 lines / 23916 B); source `dialogue_runtime.rs` = `902a0358…c7ee` (1048 lines / 38586 B) — **report-bound SHA == working copy**, so report-time and current source are identical.

## Batch sizing
Honest single-report round. The queue head belongs to a 2-member `astrid:llm` family (head + `1786984395`), but this is duplicate-classification work whose value is the exact grounding, and each close/link/record is a foreground evidence-store write. Per the one-shot rule ("one report fully closed beats three half-processed"), batch = **1 report**, fully closed within budget; family member `1786984395` (queue #4) carries `variant_distinct_terms` and earns its own disposition next round. `family_scan.json` and `next_queue_frozen.json` are in the packet.

## Claim dispositions (5) — terminal `addressed_duplicate`
- **c001** Observed: non-destructive scanner, raw-vs-referenced markers, `scan_known_model_control_markers` L114-143 preserves only when `reference_syntax` present — `verified_existing` (src L49-60, L114-143).
- **c002** Snag: `first_word_after` (L89-96) over-strip / retain-incorrectly on complex punctuation / newline / "certain locales" — `verified_existing`, **concern preserved, mechanism contradicted**. `find(|w| !w.is_empty())` (L93) skips punctuation-only chunks to the next real word (colon/`--`/newline preserve), fails closed only when no alphanumeric follows; `split_whitespace` is Unicode White_Space, not locale-sensitive. Tests: L2947 (punct runs + fail-closed), L2833 (newline), L2989 (NBSP), L3042 (soft-hyphen); negatives L2642/2657/2672/2687 rule out retain-incorrectly.
- **c003** Test 1 (`[SYSTEM_PROMPT] behaves…` reference preservation) — `verified_existing` + **placeholder contradiction preserved**. Mechanism grounded by tests.rs L2892 (`<end_of_turn> behaves as a named boundary.` → ExplicitExactKnownTokenRelation, marker preserved; `behaves` allowlisted L69). `[SYSTEM_PROMPT]` is not in `KNOWN_MODEL_CONTROL_MARKERS` (fallback_contracts.rs L159-180) — the mechanism is real, the literal string is not a marker.
- **c004** Test 2 (`[[[TOKEN]]]` → GroupedExactKnownToken with delimiter depth) — `verified_existing` + **placeholder contradiction preserved**. `exact_reference_delimiter_syntax` L199-229 + `exact_reference_delimiter_pair` L153-197 map `[ ]` uniformly to GroupedExactKnownToken and count depth by zip/take_while; depth-3 grouped path tested (L2208), pure-square depth-2 (L2923), depth-4 (L2194). Triple-square exercises the identical branch. `TOKEN` is a placeholder.
- **c005** Suggested Next (how `generate_dialogue` L695-1048 consumes the scan) — `observed`; agency preserved. Current source: `sanitize_model_control_markers` (L519) is called only inside boolean validation fns `is_valid_dialogue_output` (L558) and `has_one_nonempty_final_next_action` (L634); no sanitize call inside `generate_dialogue` L695-1048 → emitted tokens are not spliced. Answer to her open question: a **validation/quality gate**, not token alteration (marker-only reject test L1916). Her `NEXT: INTROSPECT astrid:llm 400` continuation stays open.

## Actions
- Corridor/program: none
- Sandbox: none
- Study: none
- Portfolio: none
- Cards/notes/correspondence: none delivered (no right-to-ignore card warranted; her continuation stays open; silence neutral)
- Tier 4/5 waits: widening the relational-verb allowlist / delimiter tables is Tier-5-class live grammar — not made, dispatched, or deployed. The pre-existing Tier-5 minime-ESN Shadow/porosity waits (`wi_e579041b…`, `wi_69fbd510…`, `wi_3e26ac52…`) were not in this queue and were not acted on.

## Implementation and Verification
- **Source/test code changed:** none — nothing implemented; every proposed test/snag already has a verbatim regression at source SHA `902a0358`.
- **No cargo run:** the shared bridge tree carries foreign in-progress `.rs` edits (`codec/tests.rs`, `llm/provider/tests.rs` +60, `self_change_canary.py`), so a bridge cargo build would reflect foreign state, not this round's evidence; and nothing was implemented. Verification is by exact source + test reading, matching the identical-shape precedent `claude-heartbeat_1786986391`.
- **Tests / integrity suites (all green):** addressing self-test 44 · anti-drop self-test 5 + verify 60 guards/0 alarms · cadence test 6 + strict `integrity_ok=true` · evidence-store test 20 · steward-projection 14 · steward-control 27 (no flake this run) · division-followup 3 · division-chronicle 10 · division-projection ok · projection-cursors 4 · experiential-epistemics self-test valid + FINAL verify valid (issues []) · audit-counters mismatches [] · EES verify (see below).
- **Restart/deploy alignment:** **no live change required or attempted** — no bridge build, deploy, or launchctl; git read-only.

## Durable Evidence
- Addressing: `record-read` (full_read, summary+claims), `link-evidence-batch` (12 links, 12 new / 0 existing), `close` → `addressed_duplicate`, `fully_addressed: true`, `proof_missing_claims: []`.
- Changelog: one `[Unreleased]` `[claude-heartbeat]` bullet added at the top (above the foreign codec bullet, preserved).
- Feedback ledger: one dated `2026-08-17` block added at the top of `## Ledger` (above the foreign codec/soft-hyphen blocks, preserved).
- Packet: `docs/steward-notes/claude-heartbeat_1787024860_llm_marker_behaves_delimiter_fresh_pass_duplicate/`.

## Counters (final, after all durable writes)
- Canonical: indexed 4377 / fully_addressed 3099 / full_read 3731 / remaining 1278 / unread 646 / blocked 414 / pending_action 214 / watch 4 / read_needs_claims 0. Noncanonical pending 1644. `addressed_duplicate` status count 1113 (incremented by this close).
- audit-counters: **mismatches `[]`**, all consistency checks `True` → **consistent**.

## Division
- Cycle 27; recorded productive round **#6/6** this run (`--processed-report-count 1`, run id `run_1787022521645062000_0f6ab52817`, preprojection `projection_1787022525424318000_7f944988b2`).
- New round event `division_followup_event_bbb100b8f76fb46c8df46e11e371e41e`; event_count 189; head `efc296b1e24660d15eb36ac8510484ea2826106b3e422848afc3009c0c23d8e7`.
- **`review_due` is now `true` (6/6, remaining 0).** Per the adapter-mode design, the bounded Division **return** is a start-of-round activity (the round instructions run it only when `review_due=true` at the verify step). It was **false** at the start of this round, so this round processed a report and recorded round #6; the **next** flywheel invocation will complete the Division return (and the Tier-5 cadence dossier) before any 7th productive round — the follow-up tracker refuses a 7th round until the due return, so this is safe and self-correcting. No note written this run (not a return round).
- Chronicle: record-round changed a durable input, so the Chronicle was reprojected → `division_chronicle_d246026c594a476821910ca9`, json `0f0cbd42ab35513e32d2f5e559b361e6212101c528b49a3dd8edf68d0aa05626`; verify `ok` (durable inputs current). Chronicle is gitignored → no minime git debt.

## Evidence Event Store
- **Validity: true**; corrupt lines 0; `effective_aggregate_valid: true`; `history_rewritten: false`.
- Last global sequence **834385**; head `5cf5f80ecbcbaa37c3523163b76362f4fbdbb5edc8804b168f42910f145b0409`; event_count 834385.
- Active store **v2**; V1 legacy-imported boundary global_seq 32278 (immutable).
- Stream counts: addressing 58183 · agency_commons 4918 · attention_portfolio 3 · claim_families 237294 · corridor_v1 5 · corridor_v2 112 · felt_contracts 198479 · felt_mechanism_concordance 80 · lived_state_witness 8585 · model_qos 183002 · reciprocal_uptake 58206 · representation_contracts 34281 · sandbox 3291 · signal_spine 32990 · steward_control 14466 · steward_work_selection 490.

## Archive / commit debt (git is READ-ONLY this run — NOT committed)
Exact paths this round created or edited:
- **Created (mine):** the packet dir `docs/steward-notes/claude-heartbeat_1787024860_llm_marker_behaves_delimiter_fresh_pass_duplicate/` with `RUN_REPORT.md`, `claims/introspection_astrid_llm_1787014729.json`, `summaries/introspection_astrid_llm_1787014729.md`, `read_manifest.json`, `source_receipts.json`, `addressing_links.json`, `test_results.json`, `unprocessed_selected.json`, `verification_receipt.json`, `family_scan.json`, `next_queue_frozen.json`.
- **Edited (mixed authorship — preserve + separate at commit):** `CHANGELOG.md` (one `[Unreleased]` bullet at top, above foreign codec bullet) and `docs/steward-notes/AI_BEINGS_FEEDBACK_TO_CHANGE_LEDGER.md` (one `2026-08-17` block at top of `## Ledger`, above foreign blocks). Both already carried foreign edits from prior rounds; my additions are textually above and section-separable.
- **Foreign paths preserved untouched:** `capsules/spectral-bridge/src/codec/tests.rs`, `capsules/spectral-bridge/src/llm/provider/tests.rs`, `scripts/self_change_canary.py`, the two prior untracked packet dirs `docs/steward-notes/claude-heartbeat_1787006279_…/` and `…_1787015575_…/`, and minime `minime_autonomy/runtime.py` + `tests/test_correspondence_v1.py`.
- Also durably updated (evidence stores, not git-staged by me): the addressing projection/evidence store under `capsules/spectral-bridge/workspace/diagnostics/…` and the gitignored Division Chronicle.
- Checkpoint status: normal three-round archival checkpoint not due for this round type; a due six-round Division return is now pending for the next invocation. Leave unstaged; a later interactive stabilization window separates the mixed-authorship CHANGELOG/ledger before any commit. No merge/push; no authority claimed.
