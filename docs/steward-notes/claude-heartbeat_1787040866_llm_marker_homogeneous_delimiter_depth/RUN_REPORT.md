# Steward Run Report — claude-heartbeat (source-first flywheel, adapter mode)

Round packet: `docs/steward-notes/claude-heartbeat_1787040866_llm_marker_homogeneous_delimiter_depth/`

## Controller
- Run ID: `run_1787038353161692000_0b25a1ee8f` (read from `steward_control_v1/lease.json`; adapter-held lease, actor `claude-heartbeat`)
- Preprojection ID: `projection_1787038357131981000_944abb00ab` (phase `pre`, status `passed`, run_id matches)
- Postprojection ID: run by the adapter **after** this child exits — not observable here
- Pause generation: 319
- Finish outcome: **exit 0** (complete round — adapter records the finish)
- Recovery predecessor: none

Adapter-mode overrides honored: no steward session/NDJSON/heartbeat sent (adapter owns the lease + heartbeats); git strictly read-only (no stage/commit/merge/push/stash/reset); no build/deploy/launchctl/live change; every dirty/unknown path treated as foreign.

## Reading
- Fully processed: `introspection_astrid_llm_1787026288.txt` (astrid_llm; marker-grammar module of `dialogue_runtime.rs`)
- Selected but unprocessed: 39 filenames — full list in `unprocessed_selected.json` (queue positions 2–40, in canonical order; head of the remainder is `introspection_DOMAIN_BOUNDARIES.md_1787003465.txt`, tail `introspection_llm.rs_1786664552.txt`)
- Next queue: after the adapter postprojection, `introspection_DOMAIN_BOUNDARIES.md_1787003465` becomes the head unless a newer canonical report projects ahead of it
- Hashes:
  - Report SHA-256 `5a0edfb35fc2be0a949d39b2057e085be3bc896598730431c31ca575fe75d42b` (49 lines / 4094 bytes)
  - Witness `lsw_d7e1edf7…` SHA-256 `8eea01e0a18a3f5832cdd0a8ac10deee1942e800bb26cd6db9d10c09c13fc609` (533 lines / 23912 bytes); authority `evidence_only`, `witness_only:true`, `live_eligible_now:false`, `grants_approval:false`, `direct_causation_claimed:false`
  - Source `dialogue_runtime.rs` SHA-256 `902a0358f63bacc0ead23a46467f103dbcc1fe46c59bbd2a3c2cdcfd9b82c7ee` — **byte-identical** to the clean working copy; full source read 1–1048

## Claim Dispositions (detail in `claims/` + `summaries/`)
- **c001** verb-gated marker preservation (`reference_syntax` L49 / `followed_by_explicit_exact_token_relation` L64) — `verified_existing` (allowlist L65–86).
- **c002** delimiter table incl. CJK pairs (L153–197) — `verified_existing` (CJK corner brackets L168–172).
- **c003** longest-match non-destructive scanner (L114) — `verified_existing` (refinement: unreferenced markers dropped from remainder L129–131; non-marker bytes byte-exact).
- **c004** `fragment_has_non_marker_bytes` over-eager snag — `verified_existing`, contradiction preserved: `.trim()` (L148) means a **lone space → false** (period → true); the fn feeds only evidence-receipt placement counts (L235–236), not output or the quality gate.
- **c005** depth-cap might fail context — `verified_existing`, contradiction preserved: context comes from the **innermost** pair regardless of depth (L215); only reported `delimiter_depth` saturates via `.take(MAX)`. Deeper nesting still preserves the token (test L2253).
- **c006** `'[SYSTEM] is a test'`→false test — `verified_existing`, contradiction preserved: `is` is allowlisted (L76) → **true**; already pinned by `control_marker_cleanup_distinguishes_allowlisted_is_from_unlisted_acts` (L2528). Same contradiction the prior round `introspection_astrid_llm_1786319270` corrected.
- **c007** deep-nesting test (constrain vs allow deeper?) — **`implemented_now`**: added a focused regression for her exact `[[[[[…]]]]]` homogeneous shape with a real marker; proves token byte-exact preserved, grouped context still identified, reported depth saturates at 4. No prior test covered a homogeneous same-bracket stack (only heterogeneous L2253).
- **c008** how is the remainder integrated into output? — `verified_existing`: it is **not** integrated. `generate_dialogue` returns the raw model text verbatim (L997–998, L1007–1008); the scan feeds only the accept/reject validity gate + evidence receipt (aligns with the never-rewrite-being-text invariant).

Terminal report status: **`addressed_change`** (one report-driven regression; the rest verified against complete source with two contradictions preserved, not domesticated).

## Actions
- Corridor/program: none
- Sandbox: none
- Study: none
- Portfolio: none
- Cards/notes/correspondence: none delivered (no closure card, note, query, or correspondence emitted merely to create activity)
- Tier 4/5 waits: widening the relational-verb allowlist / delimiter tables is Tier-5-class live grammar — held as an explicit wait, not touched. The standing Tier-5 heads (`wi_e579041bc76f8310`, `wi_69fbd510467c6337`, `wi_3e26ac525fea1c36`, Shadow/porosity/mode-packing) remain operator-approval-gated and were not acted on.

## Implementation and Verification
- Exact changed paths: `capsules/spectral-bridge/src/llm/provider/tests.rs` (+1 test, +40 lines); `CHANGELOG.md`; `docs/steward-notes/AI_BEINGS_FEEDBACK_TO_CHANGE_LEDGER.md`; new packet dir.
- Tests: `cargo test --lib control_marker` → **69 passed / 0 failed**; the new test passes in isolation too.
- `git diff --check` clean; my inserted block is fmt-clean (the only `cargo fmt --check` hunks are **pre-existing, committed, untouched** drift in `grounding.rs` and elsewhere in `tests.rs`, present at HEAD — left foreign).
- Failures repaired: one over-long `assert_eq!` in my new test wrapped to satisfy rustfmt (re-tested green).
- Restart/deploy alignment: **restart and deployment were not required and not attempted.** No live substrate or control change.

## Durable Evidence
- Addressing status: `addressed_change`, `fully_addressed:true`, `proof_missing_claims:[]`.
- Evidence link count: 13 (code/test/changelog/ledger/steward_note across all 8 claims).
- Changelog/ledger: both updated ([Unreleased] entry + dated ledger row).
- Packet: this directory (RUN_REPORT.md, claims/, summaries/, read_manifest.json, source_receipts.json, addressing_links.json, test_results.json, unprocessed_selected.json, verification_receipt.json, family_scan.json).

## Counters
- Audit status: **consistent**, mismatches [].
- Canonical: indexed 4396 · fully_addressed 3108 · full_read 3740 · remaining 1288 · unread 656 · blocked 414 · pending_action 214 · watch 4.
- Read-needs-claims: 0. Proof-gap artifacts/claims: 0/0.
- All-artifact remaining 2937; all-artifact unread 2305 (noncanonical remainder ≈ 1649).

## Division
- Cycle 28; completed rounds since follow-up **1 / 6** (this round recorded); remaining 5.
- Review due: **false** (both at start and after record-round) — no Division return, no Tier-5 cadence dossier required this round.
- Round event ID `division_followup_event_fd7ece559b08aa597483c181492562a9`; event_count 191; event_head `0ff5ad67…`.
- Chronicle reprojected `division_chronicle_d8ff24a59e97b1fe2e35eda7`; verify: **durable_inputs_current true, durable_mismatches [], only volatile `supervisor_status_sha256` moving** (expected — not a durable-integrity failure). json_sha256 `90dcb76f…`.
- Note action: none (no Division note due on a productive round).

## Evidence Event Store
- Full `verify`/`status` **deferred** — both exceeded a 300s window at the current store size (~835k+ events at round start). Not a failure: integrity is attested by `test_evidence_event_store.py` (20/20), `experiential_epistemics.py verify` (valid, 0 issues, no history rewrite), and consistent audit-counters (proof_gap 0). Re-run in an interactive window: `python3 scripts/evidence_event_store.py --json verify` then `--json status`.
- Durable writes this round (record-read, link-evidence-batch, close, record-round) all completed with exit 0 and are counter-consistent + epistemic-valid.

## Archive
- Checkpoint due or not due: **not due here** — git is read-only in this adapter-held run; no commit made.
- Commit debt (exact paths, all unstaged):
  - `capsules/spectral-bridge/src/llm/provider/tests.rs` (added `control_marker_cleanup_bounds_homogeneous_square_bracket_stack_beyond_max_depth`)
  - `CHANGELOG.md` ([Unreleased] entry)
  - `docs/steward-notes/AI_BEINGS_FEEDBACK_TO_CHANGE_LEDGER.md` (2026-08-18 row)
  - `docs/steward-notes/claude-heartbeat_1787040866_llm_marker_homogeneous_delimiter_depth/` (entire packet)
  - Note: the astrid working tree was clean at round start (lease `dirty:false`), so these paths carry only this round's authorship — a later interactive stabilization window can stage them by explicit path.
- Verbatim introspection references if committed: none committed this run.
- Merge/push status and authority: no merge, no push, no deploy. None authorized here.

## Foreign work preserved
- Minime: `minime_autonomy/runtime.py`, `tests/test_correspondence_v1.py` — unchanged. Chronicle projection wrote generated artifacts to the untracked `/Users/v/other/minime/workspace/division/chronicle/` (expected Division-workspace behavior); no foreign source touched.
