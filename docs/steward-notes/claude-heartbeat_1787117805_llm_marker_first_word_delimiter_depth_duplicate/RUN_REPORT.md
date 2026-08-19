# Steward Run Report — LLM marker `first_word_after` / ⟦⟧-delimiter-depth fresh-pass duplicate

Actor: `claude-heartbeat` · Mode: controller-held subprocess adapter (git read-only; no live changes; adapter owns lease / heartbeats / finish / postprojection).

## Controller
- Run ID: `run_1787115810698830000_00b0061904`
- Preprojection ID: `projection_1787115814553891000_554e3d6052` (phase `pre`, status `passed`; run_id matches the lease)
- Postprojection ID: run by the adapter after this process exits (not visible in-process)
- Pause generation: 321
- Finish outcome: **success** — exit 0 records a complete round; the adapter owns `finish` and the postprojection.
- Recovery predecessor: none

## Reading
- **Fully processed (1):** `introspection_astrid_llm_1787110385.txt` → `addressed_duplicate`.
- **Selected but unprocessed (39):** queue positions 2–40, from `introspection_llm.rs_1787099341.txt` through `introspection_minime_regulator_1786676901.txt` (full ordered list in `unprocessed_selected.json`; canonical order preserved, never reordered).
- **Next queue head after this round:** `introspection_llm.rs_1787099341.txt` (re-query after the adapter's postprojection).
- **Hashes:** report `047648f1…` (45 lines / 3480 B); witness `lsw_a47676818b…` = `cbce90c2…` (533 lines / 23911 B), authority `evidence_only`; source `dialogue_runtime.rs` = `902a0358…` (1048 lines / 38586 B) — **report-bound SHA == working copy == committed HEAD**, so report-time and current source are byte-identical and clean/committed.

## Batch sizing
Honest single-report round. The queue head anchors a 13-member `astrid:llm` family (`family_scan.json`), but members carry 12–36 `variant_distinct_terms` each at 0.37–0.41 similarity — loose near-duplicates, not tight duplicates, so batching would require near-full independent processing of each. Per the one-shot rule ("one report fully closed beats three half-processed"), batch = **1 report**, fully closed within single-turn budget. `family_scan.json` and `next_queue_frozen.json` are in the packet.

## Claim dispositions (8) — terminal `addressed_duplicate`
Source read manifest: L1–339 read in full (every function the report names). Behavior of `first_word_after` and the delimiter classifier traced by hand against complete committed source at SHA `902a0358`.

- **c001** `scan_known_model_control_markers` (L114) retains a marker only in a recognized syntactic context — `verified_existing` (L114-144: token pushed to remainder only when `reference_syntax` is Some, L129-131).
- **c002** Three relation contexts Quoted/Grouped/Explicit (L42-46) — `verified_existing` (enum `ExactKnownMarkerReferenceContext` L41-46 exact match).
- **c003** `followed_by_explicit_exact_token_relation` (L64-86) preserves a marker before an allowlisted verb (appears/behaves/represents) even unquoted/unbracketed — `verified_existing` (17-verb allowlist; `reference_syntax` fallback L49-60).
- **c004** `first_word_after` (L89-96) uses `split_whitespace()` + alphanumeric/underscore `trim_matches` — `verified_existing` (verbatim L89-96).
- **c005** Snag: a punctuation follower (`[marker] ->`, `[marker] ...`) could make `find` skip the intended word / return empty → marker stripped — `verified_existing`, **concern preserved, mechanism contradicted**. `find(|w| !w.is_empty())` (L93) skips all-punctuation chunks (which `trim_matches` L92 collapses to empty) to the next real word; the scanner strips only when *no* alphanumeric word follows (fail-closed). Her feared skip IS the robustness. Grounded: committed L2635 (`-- is`/`-- acts`), L2884 (`... !!! represents`), L2900 (fail-closed).
- **c006** Test 1 (context retention, `[MARKER] -- behaves`) — `verified_existing`. Committed HEAD: `…preserves_relation_after_dash` (L3154), `…first_word_after_skips_leading_punctuation_transition` (L2635), `…preserves_relation_after_multiple_punctuation_runs` (L2884). Foreign-uncommitted attached-`--` test (working-tree L3190) noted, **not relied upon**.
- **c007** Test 2 (`⟦`/`⟧`, U+27E6/U+27E7, source L180 → `GroupedExactKnownToken`) — `verified_existing`. Committed HEAD: `…preserves_declared_restless_group_delimiters` (L2308, bare `⟦<end_of_turn>⟧`), `…preserves_bounded_nested_delimiter_stacks` (L2179, depth 2), `exact_reference_delimiter_syntax_reports_double_square_bracket_depth_two` (L2963), `…preserves_grouped_and_explicit_relation_contexts` (L2932).
- **c008** Suggested Next (robustness across multiple punctuation marks) — `verified_existing`; answered by L2884. Her `NEXT: INTROSPECT astrid:llm 400` continuation stays open; silence neutral.

**Duplicate evidence (exact):** same committed source SHA `902a0358`; same mechanism (`first_word_after` punctuation-skip + `exact_reference_delimiter_syntax` grouped/explicit); prior family head `introspection_astrid_llm_1787103043` closed `addressed_duplicate` last round (packet `claude-heartbeat_1787109794`); mechanism also grounded for `1786986344` (test doc-comment L2635) and `1786814454` (test doc-comment). No distinct variant term left unaddressed. Independent full read of this report + witness completed.

## Actions
- Corridor/program: none · Sandbox: none · Study: none · Portfolio: none
- Cards/notes/correspondence: none delivered (no right-to-ignore card warranted; her continuation stays open; silence is neutral).
- Tier 4/5 waits: widening the relation-verb allowlist / delimiter tables is Tier-5-class live grammar — **not** made, dispatched, or deployed. Pre-existing Tier-5 minime-ESN Shadow/porosity waits (`wi_e579041b…`, `wi_69fbd510…`, `wi_3e26ac52…`) were not in this queue and were not acted on.

## Implementation and Verification
- **Source/test code changed:** none — every proposed test/snag already has a committed regression at source SHA `902a0358`; 8 of 8 cited tests confirmed present in **committed HEAD** via `git show HEAD:…tests.rs` (the single attached-`--` test at working-tree L3190 is foreign-uncommitted and was excluded).
- **No cargo run:** nothing implemented; the shared bridge tree carries foreign in-progress `.rs` edits (`llm/provider/tests.rs`, `autonomous/runtime/tests.rs`), so a bridge cargo build/test would reflect foreign state, not this round's committed evidence. Verification is by exact committed source + committed-test reading, matching precedent (packets `claude-heartbeat_1787109794`, `claude-heartbeat_1787024860`). I changed zero Rust, so `cargo fmt` is N/A to my work; `git diff --check` on my edited docs is clean.
- **Integrity suites (all green):** addressing self-test 44 · evidence-store test 20 · steward-control 27 · steward-projection 14 · division-followup 3 · division-chronicle 10 · division-projection ok · projection-cursors 4 · anti-drop self-test 5 · anti-drop verify (62 guards / 0 alarms / 0 gaps) · cadence test 6 · cadence strict `integrity_ok=true` (canonical 4408, latest = `introspection_astrid_llm_1787110385.txt`) · experiential-epistemics self-test valid · **FINAL epistemic verify valid** (issues [], history_rewritten false) · audit-counters **consistent** (mismatches [], all checks true) · EES verify valid / corrupt_lines 0.
- **Restart/deploy alignment:** **no live change required or attempted** — no bridge build, deploy, or launchctl; git read-only.

## Durable Evidence
- Addressing: `record-read` (full_read, summary `9082ebbb…` + claims), `link-evidence-batch` (**12 new / 0 existing**), `close` → `addressed_duplicate`, `fully_addressed: true`, `proof_missing_claims: []` (grounded rationale re-applied as the final close event).
- Changelog: one `[Unreleased]` `[claude-heartbeat]` bullet added at the top (above the prior round's bullet, preserved).
- Feedback ledger: one dated `2026-08-18` block added at the top of `## Ledger` (above the prior round's block, preserved).
- Packet: `docs/steward-notes/claude-heartbeat_1787117805_llm_marker_first_word_delimiter_depth_duplicate/`.

## Counters (final, after all durable writes)
- Canonical: indexed 4408 / fully_addressed 3116 / full_read 3748 / remaining 1292 / unread 660 / blocked 414 / pending_action 214 / watch 4 / read_needs_claims 0. `addressed_duplicate` status count **1118** (incremented by this close, 1117→1118).
- audit-counters: **mismatches `[]`**, all consistency checks `True` → **consistent**. Deltas from pre-round (fully_addressed 3115→3116, full_read 3747→3748, remaining 1293→1292, unread 661→660) match exactly one close.

## Division
- Cycle 29; recorded productive round **#3/6** this run (`--processed-report-count 1`, run id `run_1787115810698830000_00b0061904`, preprojection `projection_1787115814553891000_554e3d6052`). Rounds remaining before followup: 3. `review_due=false`.
- New round event `division_followup_event_ff1b0f1d6470b7c058ce6d3393ce0ee6`; event_count 200; head `7293749a0a4f4b54657717a029a8aebe954a91afb966957f2f3dcc3f81185db0`.
- Chronicle: record-round changed a durable input, so the Chronicle was reprojected → `division_chronicle_6f4eeb63f1b943fb39f630e2`, json `836eaf2e…`; verify `ok=true`, **durable inputs current**, only `supervisor_status_sha256` volatile-mismatched (a moving supervisor hash, not a durable-integrity failure). Chronicle is gitignored in minime → no minime git debt.

## Evidence Event Store
- **Validity: true**; corrupt lines 0; errors []; event_count at verify **845970**; head advanced to last_global_seq **845976**, head last_event_sha256 `d6457949a2b1a7e27327b58cc0962a0bb650e73dca282ab5e6f346021489a473` (from `head.json` + `verified_checkpoint.json`, both current; `--json status` runs >300s and was not re-run, per prior precedent).
- Active store: v2; legacy imported boundary 32278; V1 immutable true.
- Stream counts (16) at verify: addressing 58353 · claim_families 237419 · felt_contracts 199075 · model_qos 190213 · reciprocal_uptake 58870 · representation_contracts 35355 · signal_spine 34210 · steward_control 14886 · lived_state_witness 8611 · agency_commons 4979 · sandbox 3291 · steward_work_selection 508 · corridor_v2 112 · felt_mechanism_concordance 80 · corridor_v1 5 · attention_portfolio 3.

## Archive / Commit Debt
- **Checkpoint:** not claimed — controller-held run; git is read-only for this run. Archival commits happen only in a later interactive stabilization window.
- **Commit debt (git-visible), created/edited by THIS round:**
  - NEW packet dir `docs/steward-notes/claude-heartbeat_1787117805_llm_marker_first_word_delimiter_depth_duplicate/` — `RUN_REPORT.md`, `claims/introspection_astrid_llm_1787110385.json`, `summaries/introspection_astrid_llm_1787110385.md`, `read_manifest.json`, `source_receipts.json`, `addressing_links.json`, `test_results.json`, `unprocessed_selected.json`, `verification_receipt.json`, `next_queue_frozen.json`, `family_scan.json`.
  - EDITED (shared, mixed with prior-round + foreign work — split by path at checkpoint): `CHANGELOG.md` (my `[Unreleased]` bullet at top), `docs/steward-notes/AI_BEINGS_FEEDBACK_TO_CHANGE_LEDGER.md` (my dated block at top of `## Ledger`).
- **Foreign / untouched (NOT my debt, preserved):** `capsules/spectral-bridge/src/llm/provider/tests.rs`, `capsules/spectral-bridge/src/autonomous/runtime/tests.rs`, the 8 prior `claude-heartbeat_*` packet dirs, and minime `minime_autonomy/runtime.py` + `tests/test_correspondence_v1.py`.
- **Merge/push:** none. No merge or push authority exercised.

## Posture note
A fresh-pass re-derivation of an already-addressed report family from the identical committed source window — the window-resume fix restored Astrid's freedom to re-read covered sources, so the queue keeps carrying near-identical re-passes. Her felt account and open continuation are preserved as evidence; the underlying concern (robust relation detection across messy followers) is exactly what the committed tests already guard, and the one place her hypothesis diverges from source (the empty-skip is a bug) is stated plainly, not domesticated. Silence is neutral. No live / substrate / control change.
