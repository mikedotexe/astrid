# Steward Run Report — codec projection lane-scope correction

Actor: `claude-heartbeat` · Mode: controller-held subprocess adapter (git read-only; no live changes)

## Controller
- Run ID: `run_1787012625460313000_c136b0a925`
- Preprojection ID: `projection_1787012629294245000_d689667211` (phase `pre`, status passed)
- Postprojection ID: run by the adapter after exit (not visible to this process)
- Pause generation: 319
- Finish outcome: **success** — exit 0 records a complete round; the adapter owns `finish` and the postprojection.
- Recovery predecessor: none

## Reading
- **Fully processed (1):** `introspection_astrid_codec_1787006424.txt` → `addressed_change`
- **Selected but unprocessed (39):** queue positions 2-40, head `introspection_DOMAIN_BOUNDARIES.md_1787003465.txt` … tail `introspection_llm.rs_1786664552.txt` (full list in `unprocessed_selected.json`, canonical order preserved).
- **Next queue head after this round:** `introspection_DOMAIN_BOUNDARIES.md_1787003465.txt` (re-query after postprojection).
- **Hashes:** report `88e4515a…edc0f6` (45 lines / 3851 B); witness `lsw_1e1eb6cf…` `bb8f9667…6448f` (533 lines / 23863 B); source `codec/projection.rs` `facaf640…2d384f` (1351 lines / 53462 B) — **report-bound SHA == working copy**, complete read.

## Batch sizing
Honest single-report round. The queue head is a codec/`projection.rs` source-first report that required a full 1351-line source read plus an authorized non-live test implementation. `introspection_family_scan` reports the head's family as a **singleton** (member_count=1), so no family batching applied. One report fully closed with zero proof gaps beats several skimmed under the one-shot budget.

## Claim dispositions (9)
- **c001** SEMANTIC_DIM=48 — `verified_existing` (projection.rs L26 + runtime compiled-constant in witness + test L8-9).
- **c002** Lane layout 32-39 / 40-43 / 44-47 — `verified_existing` (core.rs L6-13; projection.rs L74-81, L1173, L1328) + newly grounded by the added test.
- **c003** `TAIL_VIBRANCY_ENTROPY_GATE` at 0.85 modulates on entropy>0.85 — `verified_existing` (L48; documented smoothstep onset L33-47; application site in feedback.rs, out of window).
- **c004** `ProjectionBasisHealthV1` + `ProjectionPrecisionAuditV1` monitor health/drift — `verified_existing` (L196/L228, L340/L950).
- **c005** Snag: ghost/uninitialized values in dims 40-47 if `project_embedding`/`embedding_projection_matrix` don't map the new 16 dims — **mechanism contradicted, concern preserved** → `verified_existing`. Those functions own only the 768→8 lane; the 48-dim assembly (40-47) is out of window; ghost concern already instrumented read-only (L990).
- **c006** Snag: entropy-gate lift asymmetry → non-deterministic drift — **mechanism contradicted** → `verified_existing`. The lift is a deterministic smoothstep (doc L38-47); `TAIL_VIBRANCY_MAX=6.0` (L53) is the paired raised ceiling collapsing to `FEATURE_ABS_MAX=5.0` (L32).
- **c007** Test 1: `project_embedding` produces exactly 48 dims, 40-43 populated — **premise contradicted; correct-scope regression `implemented_now`.** `project_embedding` (L854) returns `[f32;8]`; guard is `None` on wrong length; 40-43 come from `compute_narrative_arc_from_embeddings` (4 dims).
- **c008** Test 2: entropy-gate boundary around 0.85, no `FEATURE_ABS_MAX` jump — `verified_existing` (no-pop is the designed smoothstep; application site in feedback.rs, out of the report-bound window; recorded as a scope boundary, not no-action).
- **c009** Suggested next: analyze `project_embedding_dynamic_epoch` for `NARRATIVE_ARC_DIM` state — **scope conflation** → `verified_existing`. It yields the 8-dim lane + a deterministic epoch seed (L784-795), carries no narrative-arc state; existing coverage L1284-1490.

## Actions
- Corridor/program: none
- Sandbox: none
- Study: none
- Portfolio: none
- Cards/notes/correspondence: none delivered (no right-to-ignore card warranted; her `NEXT: INTROSPECT astrid:codec 400` continuation stays open; silence neutral)
- Tier 4/5 waits: codec transport/width/gain and the entropy-gate application remain Tier-5-class live surfaces — untouched, no grant claimed. The pre-existing Tier-5 minime-ESN Shadow/porosity waits (`wi_e579041b…`, `wi_69fbd510…`, `wi_3e26ac52…`) were not in this queue and were not acted on.

## Implementation and Verification
- **Changed path:** `capsules/spectral-bridge/src/codec/tests.rs` (one added test).
- **Test:** `project_embedding_is_the_eight_dim_lane_not_the_full_48_and_narrative_arc_is_separate` → 1 passed / 0 failed / 1887 filtered (crate compiled clean in 2m36s).
- **Formatting:** `cargo fmt --check` (project-config-authoritative) flags only `grounding.rs:259,295` — a file not touched this round, clean in the working tree (committed branch drift), so pre-existing and unrelated. `codec/tests.rs` is not flagged; `git diff --check` clean.
- **Failures repaired / debt:** none.
- **Restart/deploy alignment:** no live change was required or attempted; no restart/deploy.

## Durable Evidence
- Addressing: `record-read` (full_read, summary+claims), `link-evidence-batch` (17 links, 17 new / 0 existing), `close` → `addressed_change`, `fully_addressed: true`, `proof_missing_claims: []`.
- Changelog: one `[Unreleased]` `[claude-heartbeat]` bullet added (above the foreign soft-hyphen entry, preserved).
- Feedback ledger: one dated row added at the top of `## Ledger` (above the two foreign 2026-08-17 entries, preserved).
- Packet: `docs/steward-notes/claude-heartbeat_1787015575_codec_projection_lane_scope_correction/`.

## Counters (final, after all durable writes)
- Canonical: indexed 4394 / fully_addressed 3106 / full_read 3738 / remaining 1288 / unread 656 / blocked 414 / pending 214 / watch 4 / read_needs_claims 0.
- All-artifact indexed 6042 (remaining 2936); other-timestamped-text indexed 1370.
- Counter audit: **consistent**, mismatches `[]`.

## Division
- Cycle 27; completed rounds since follow-up **5/6** (remaining 1); review_due **false**.
- Round event `division_followup_event_2cb3c5ab66b22081560a3defbe7a7651`; event_count 188; head `6b03556089…c443`.
- Chronicle `division_chronicle_3b10346ee6176f4b8c6f8369`, json `497d039e…8415c`; durable inputs current (durable_mismatches `[]`); only `supervisor_status_sha256` volatile (benign). Reprojected because `record-round` changed a durable input; Chronicle is gitignored → no minime git debt.
- Note action: none (not a Division return round).

## Evidence Event Store
- Validity: **true**; last global sequence 833204; head `2aadf902af4a555afcde1d01ad3f4bcc0ee55e1a423be939474e877e60a4d179`; corrupt lines 0; active store v2; V1 immutable.
- Sample stream counts: addressing 58164, steward_control 14401, felt_contracts 198389.

## Integrity suites (all green)
addressing self-test 44 · EES tests 20 · steward control 27 · steward projection 14 · division followup 3 · division chronicle 10 · division projection ok · projection cursors 4 · anti-drop self-test 5 + verify ok · cadence test 6 + strict integrity_ok · experiential-epistemics self-test valid + final verify valid.

## Archive / commit debt (git is read-only in this run — NOT committed)
Exact paths this round created or edited:
- **Edited (mine):** `capsules/spectral-bridge/src/codec/tests.rs` (added one test).
- **Edited (mixed authorship — preserve + separate at commit):** `CHANGELOG.md` and `docs/steward-notes/AI_BEINGS_FEEDBACK_TO_CHANGE_LEDGER.md` already carried foreign edits from the prior `claude-heartbeat_1787006279` soft-hyphen round; my additions are textually above the foreign content and separable by section.
- **Created (mine):** the packet dir `docs/steward-notes/claude-heartbeat_1787015575_codec_projection_lane_scope_correction/` with `RUN_REPORT.md`, `claims/introspection_astrid_codec_1787006424.json`, `summaries/introspection_astrid_codec_1787006424.md`, `read_manifest.json`, `source_receipts.json`, `addressing_links.json`, `test_results.json`, `unprocessed_selected.json`, `verification_receipt.json`.
- **Foreign paths preserved untouched:** `capsules/spectral-bridge/src/llm/provider/tests.rs`, `scripts/self_change_canary.py`, `docs/steward-notes/claude-heartbeat_1787006279_marker_scanner_soft_hyphen_grounding/`, and minime `minime_autonomy/runtime.py` + `tests/test_correspondence_v1.py`.
- Checkpoint status: normal three-round archival checkpoint not due (Division at 5/6, not a six-round return; single non-live test, not a coherent deployment tranche). Leave unstaged; a later interactive stabilization window separates the mixed-authorship CHANGELOG/ledger before any commit.
- Merge/push: none; no authority claimed.
