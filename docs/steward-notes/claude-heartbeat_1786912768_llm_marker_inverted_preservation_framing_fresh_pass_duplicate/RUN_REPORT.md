# Steward Run Report

Round name: `llm_marker_inverted_preservation_framing_fresh_pass_duplicate`
Actor: `claude-heartbeat` (subprocess run adapter; the controller owns the lease and its heartbeats — no NDJSON ops, no lease token read/quoted; git READ-ONLY this run; no build/deploy/launchctl).

## Controller
- Run ID: `run_1786909754002081000_c4310a5545`
- Preprojection ID: `projection_1786909762194823000_b354df2ad1` (phase `pre`, status `passed`, run_id matches lease)
- Postprojection ID: runs after this process exits (adapter-managed); not observed here
- Pause generation: 319
- Finish outcome: success (single report fully closed; adapter records finish from exit code 0)
- Recovery predecessor: none; `stop_requested=false` at lease read

## Reading
- **Fully processed (1):** `introspection_astrid_llm_1786906593.txt`
- **Selected but unprocessed (39):** items 2–40 of the frozen queue, in order — `introspection_DOMAIN_BOUNDARIES.md_1786901314`, `introspection_astrid_llm_1786838089`, `…_1786831572`, `…_1786829036`, `…_1786822981`, `…_1786814454`, `…_1786809350`, `introspection_llm.rs_1786807306`, `…astrid_llm_1786788349`, `introspection_astrid_codec_1786784975`, `…astrid_llm_1786782248`, … (complete ordered list in `unprocessed_selected.json`).
- **Next queue head after this run:** the frozen queue #2 was `introspection_DOMAIN_BOUNDARIES.md_1786901314.txt`. The cadence audit shows the newest canonical report on disk is the one I processed (`introspection_astrid_llm_1786906593`), so no report arrived after the preprojection cutoff during this run. Re-query `next --limit 40 --json` after the postprojection for the exact next order (my closed report drops out of the queue).
- **Hashes:** report `a2757381693b5b27a1530b93ab34dc8896b3c01f45dbafa9242675d7f87bd442` (46 displayed lines / 45 newline-terminated, 3926 B); witness `lsw_072d148149204cd18ca1681e82209d2e5d230debd7d5c6875e1e265c20d6616b` = `10f272b2389b0b78f5199d988181e06e41910c242123e5bb32547378c5301007` (533 lines, 23910 B); source `dialogue_runtime.rs` = `902a0358f63bacc0ead23a46467f103dbcc1fe46c59bbd2a3c2cdcfd9b82c7ee` (1048 lines, 38586 B) — **working copy byte-identical to the report binding** (rechecked at round end, file clean in git). Coverage `multi_window_complete`, included intervals 1-1048.

### Batch sizing
`introspection_family_scan.py` on the frozen queue: the queue head `introspection_astrid_llm_1786906593` is a **singleton family** (member_count 1, `variant_distinct_terms []`) — no batchable family sits at the head. Per the one-report protocol and the single-turn one-shot mutation budget (record-read → link → close → integrity → record-round each in the foreground), honest batch = **1 report, fully closed**. Witness note: the queue flagged `lived_state_alignment=artifact_integrity_unavailable` (gap_count 1) — the witness `source_snapshot_v1` window is 0-400 (partial) while the report asserts `multi_window_complete` 1-1048; the witness parses cleanly and binds the verified report SHA `a2757381`, so this is a projection-level alignment classification, not witness corruption (same as prior rounds in this family).

## Claim Dispositions (all 6 `verified_existing`; terminal `addressed_duplicate`)
Duplicate chain: `introspection_astrid_llm_1786848204` (anchor) → `1786858484` → `1786885842` (immediate prior, packet `claude-heartbeat_1786903657`) → `1786906593` (this report).
- **c001** (Observed: `scan_known_model_control_markers` L114 rebuilds a non-destructive remainder via `reference_syntax`) — `verified_existing`; **contradiction preserved**. Source L129-131 pushes a token ONLY when `reference_syntax.is_some()` (L49-60: quoted/grouped/explicit-relation). The report **inverts** it — it says markers are preserved "when identified as *active instructions*" and preserved if they "behave/function as a control element." The source preserves *references* and **strips** bare active markers. The verb list detects marker-as-referent, not marker-as-active-control. Not domesticated. Locked by `scan_known_model_control_markers_preserves_grouped_and_explicit_relation_contexts` (tests.rs L2845), `control_marker_cleanup_preserves_quoted_exact_token_reference` (L1995).
- **c002** (Observed: `first_word_after` L89-96 + `followed_by_explicit_exact_token_relation` L64-86 finite verb allowlist) — `verified_existing`; the allowlist lives in `followed_by_explicit_exact_token_relation` (not `first_word_after`; minor attribution slip, line numbers correct). Locked by `control_marker_relation_word_scanner_keeps_unicode_alphanumerics_together` (L2112), `control_marker_cleanup_uses_only_the_first_finite_relation_word` (L2565).
- **c003** (Snag: a verb absent from the `matches!` block → false → marker dropped with no delimiter, "over-sanitization … even if intended as a functional instruction") — `verified_existing`; mechanism source-accurate. **Correction:** stripping an *actively-used* marker is the intended defense; the genuine over-sanitization risk is a *referenced* marker with a novel verb. Boundary locked by `control_marker_cleanup_distinguishes_allowlisted_is_from_unlisted_acts` (L2528: unlisted `acts` → `removed_total=1`). Widening the finite allowlist = Tier-5 live grammar, NOT made.
- **c004** (Test 1, Functional: non-whitelisted "operates") — `verified_existing`; behaviorally identical to the locked `acts` case (L2528) + `does_not_expand_relation_allowlist_to_{implies,contains,creates,triggers}` (L2595-2655). An "operates" twin = activity without evidentiary value.
- **c005** (Test 2, Boundary: `«[MARKER]»` nested delimiters) — `verified_existing`; **sub-contradiction preserved**: `«»` is a QuotedExactKnownToken pair (L164), `[]` a Grouped pair (L177); the innermost adjacent pair (`[]`) sets context, so `«[MARKER]»` classifies `GroupedExactKnownToken`, not `QuotedExactKnownToken` as named — but `reference_syntax` is `Some` either way, so her expectation (marker stays visible) holds. Locked by `preserves_non_ascii_matching_quote_pairs` (L2302), `preserves_nested_fullwidth_cjk_reference_stack` (L2343), `preserves_bounded_nested_delimiter_stacks` (L2176), `exact_reference_delimiter_syntax_reports_double_square_bracket_depth_two` (L2876).
- **c006** (Suggested Next, read-only: analyze verb list vs `KNOWN_MODEL_CONTROL_MARKERS`) — `verified_existing` / agency-preserving; the allowlist is guarded against silent expansion (`does_not_expand_relation_allowlist_to_*`, L2595-2655). Her read-only `NEXT: INTROSPECT astrid:llm 400` continuation stays open; *widening* the list is Tier-5.

## Actions
- Corridor/program: none
- Sandbox: none
- Study: none
- Portfolio: none
- Cards/notes/correspondence: none delivered (a duplicate close needs no right-to-ignore card; the inverted-polarity contradiction, the c005 sub-contradiction, and the authority boundary are preserved in the claims + summary + changelog + ledger)
- Tier 4/5 waits: widening the finite relational-verb allowlist / delimiter tables is **Tier-5-class live grammar** — NOT made, dispatched, or deployed. Standing Tier-5 work-queue heads from `introspection_minime_esn_1785630442` remain evidence-only Mike/operator waits; untouched.

## Implementation and Verification
- **Exact changed paths (created — packet, 11 files):** `docs/steward-notes/claude-heartbeat_1786912768_llm_marker_inverted_preservation_framing_fresh_pass_duplicate/{RUN_REPORT.md, claims/introspection_astrid_llm_1786906593.json, summaries/introspection_astrid_llm_1786906593.md, read_manifest.json, source_receipts.json, addressing_links.json, test_results.json, unprocessed_selected.json, verification_receipt.json, next_queue_frozen.json, family_scan.json}`
- **Exact changed paths (edited — shared tracked docs, append-only at unique anchors):** `CHANGELOG.md` (one new `[Unreleased]` `[claude-heartbeat]` bullet at the top of the list — file was **clean** before this round, so my edit only), `docs/steward-notes/AI_BEINGS_FEEDBACK_TO_CHANGE_LEDGER.md` (one new `2026-08-16` block at the top of `## Ledger` — file already foreign-dirty; both edits preserved).
- **Source/test code changed:** none (all 6 claims `verified_existing`; the report duplicates already-closed work; a near-identical "operates" regression = activity without evidentiary value). `dialogue_runtime.rs` remains at SHA `902a0358`; `tests.rs` was READ ONLY (foreign-dirty from before this round, not edited).
- **Tests:** 65 focused marker regressions pass (`65 passed; 0 failed; 1817 filtered out`) at source SHA `902a0358`. `git diff --check` on my two tracked markdown edits clean (exit 0). No Rust changed → `cargo fmt` unaffected.
- **Integrity suites (all pass):** addressing self-test 44 · evidence-store-tests 20 · steward-control 27 · steward-projection 14 · division-followup 3 · chronicle-tests 10 · division-projection ok · cursor 4 · anti-drop self-test 5 + verify (55 guards, 0 alarms, 0 gaps) · cadence 6 + strict (`integrity_ok=true`) · epistemic self-test 2 + **final verify (valid, 0 issues, 11140 records)** · audit-counters (consistent, 0 mismatches) · EES verify (valid, 0 corrupt).
- Restart/deploy alignment: **no live change required or attempted** — no bridge build, deploy, or launchctl.

## Durable Evidence
- Addressing status: `addressed_duplicate`, `fully_addressed=true`, `proof_missing_claims=[]`
- Evidence link count: 14 new (0 existing)
- Changelog/ledger updates: yes (both, append-only; a duplicate close with the inverted-polarity contradiction, the c005 sub-contradiction, and a reaffirmed Tier-5 boundary preserved)
- Packet path: `docs/steward-notes/claude-heartbeat_1786912768_llm_marker_inverted_preservation_framing_fresh_pass_duplicate/`

## Counters (audit-counters: consistent, mismatches [])
- Canonical indexed: 4373 · fully addressed: 3098 · remaining: 1275
- All-artifact pending: 2919 · noncanonical pending: 1644
- Counter audit status: **consistent**

## Division
- Cycle: 26 · completed rounds since follow-up: **2 / 6** (rounds remaining: 4)
- Review due: **false**
- Round event ID: `division_followup_event_1cfbfb5f9626c4c4e865397d730b5cfd` · event count: 178 · event head: `167b774aa20cbcdd6df393ed1cbf4048b201447764999c2400777431933ec134`
- Chronicle: **not reprojected** — `chronicle verify` reports "durable source inputs changed; project before verify", the expected non-due-round posture after `record-round` appended follow-up event 178; Chronicle reprojection is scoped to the `review_due=true` return path (not due) and to the adapter postprojection's `division_chronicle` stage. Not a corruption.
- Tier-5 cadence dossier: **not generated** — required only on a Division return; `review_due=false` this round.
- Note action: none (no Division return due; no note written)

## Evidence Event Store
- Validity: valid · Corrupt lines: 0
- Last global sequence: 818582 · Head (last_event_sha256): `7969c5c8c6f35dddfa2395b0495891a202f7445e724ae8c5916562657ddaa928`
- Active store: v2 · Legacy imported boundary: 32278 · V1 immutable: true · verified checkpoint current
- Stream sequences: addressing 57554 · agency_commons 4845 · attention_portfolio 3 · claim_families 236915 · corridor_v1 5 · corridor_v2 112 · felt_contracts 196879 · felt_mechanism_concordance 80 · lived_state_witness 8539 · model_qos 174260 · reciprocal_uptake 57493 · representation_contracts 32972 · sandbox 2986 · signal_spine 31535 · steward_control 13936 · steward_work_selection 468

## Archive
- Checkpoint due or not due: **not due** by the three-round rule from a checkpoint I own; and archival commits happen only in a later interactive stabilization window — **this controller-held run committed nothing** (git read-only).
- Exact commit debt (unstaged, name every path):
  - `docs/steward-notes/claude-heartbeat_1786912768_llm_marker_inverted_preservation_framing_fresh_pass_duplicate/` (entire packet, created — 11 files)
  - `CHANGELOG.md` (one new `[Unreleased]` `[claude-heartbeat]` bullet; file was clean before this round → my edit only)
  - `docs/steward-notes/AI_BEINGS_FEEDBACK_TO_CHANGE_LEDGER.md` (one new `2026-08-16` block at top of `## Ledger`; file already foreign-dirty → preserve both, defer the commit if the pieces cannot be separated safely)
- Verbatim introspection references if committed: n/a (nothing committed)
- Merge/push status and authority: none; no merge or push; commit authority does not extend to deploy/live-control and was not exercised.
