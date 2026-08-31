# Steward Run Report

Round: `claude-heartbeat_1787885626_llm_marker_adverb_and_ascii_stack`
Actor: `claude-heartbeat` (controller subprocess `run` adapter — lease + heartbeats owned by the adapter; git read-only)

## Controller
- Run ID: `run_1787883039226978000_05e9f78c6b`
- Preprojection ID: `projection_1787883043268651000_2397c45f9f` (status passed)
- Postprojection ID: runs after exit (adapter) — not known in-session
- Pause generation: 321
- Finish outcome: success (exit 0 — complete round)
- Recovery predecessor: none

## Reading
- Fully processed: `introspection_astrid_llm_1787882114.txt`
- Selected but unprocessed (39, queue order 2–40): see `unprocessed_selected.json`. Head-of-next-queue if unchanged: `introspection_astrid_ws_1787875902.txt`.
- Report/witness/source hashes:
  - Report: `56ea74e9fc64162aa2c9d57e18b2c8aff77ae23a92d1758c29a17ff0fce72c05` (45 lines / 3499 bytes)
  - Witness `lsw_f4992f957cb68f06abc32badb1c09e0d0904cfb5e839697f8802832ba821b7e9`: `de59dae2f832dda121071f554fa9a87302b9fb5cf8d0a8140b4b57651f06e9e5` (533 lines / 23933 bytes)
  - Source `capsules/spectral-bridge/src/llm/provider/dialogue_runtime.rs`: `902a0358f63bacc0ead23a46467f103dbcc1fe46c59bbd2a3c2cdcfd9b82c7ee` (1048 lines / 38586 bytes) — working copy **matches the report binding exactly**; complete read L1–1048.

## Batch sizing
Queue head is a **singleton family** (`introspection_family_scan` member_count 1) bound to a large 1048-line source. Processed **1** report fully (source verification + two regressions) — an honest batch well within the ~90-minute budget; a skimmed multi-report batch was declined.

## Claim Dispositions
- **c001** (Observed: non-destructive scanner preserves referenced markers, strips un-referenced) → `verified_existing`. `scan_known_model_control_markers` L114-144 pushes a marker token to `remainder` only when `reference_syntax` (L49) is Some; else drops it. Cited lines accurate. Evidence: code.
- **c002** (Snag/Test 1: `first_word_after` L89 only inspects the first word; an adverb before the verb defeats relation recognition) → `implemented_now`. Real for a **bare** marker (`MARKER actually serves as` → `first_word_after`=`actually` → stripped). Correction preserved (not domesticated): her bracketed `[MARKER]` example would be preserved by the delimiter path (Grouped), so the snag manifests only for a bare marker. New regression `control_marker_cleanup_does_not_skip_adverb_before_relation_word`. Evidence: code + test + changelog + ledger.
- **c003** (Test 2: delimiter depth for nested `"'[MARKER]'"` vs `MAX_EXACT_REFERENCE_DELIMITER_DEPTH` L151) → `implemented_now`. Mechanism already covered by `_reports_exact_three/four_level_delimiter_depth` + `_bounds_homogeneous_square_bracket_stack_beyond_max_depth`; her **literal ASCII** double-quote/single-quote/bracket mix (no prose between) was untested. New regression `control_marker_cleanup_reports_mixed_ascii_quote_bracket_stack_depth_three` (context=grouped, depth 3, preserved). Evidence: code + test.
- **c004** (Suggested Next: widen `first_word_after` to skip adverbs) → `authority_gated`. Held as a deliberate boundary — widening changes being-facing output and broadens production grammar (same boundary as `introspection_astrid_llm_1786319270`). Concern preserved, not dismissed. Evidence: steward_note.

Terminal status: `addressed_change` (close: `fully_addressed=true`, `proof_missing_claims=[]`).

## Actions
- Corridor/program: none
- Sandbox: none
- Study: none
- Portfolio: none
- Cards/notes/correspondence: none (no closure card delivered; would be activity-for-activity)
- Tier 4/5 waits: c004 grammar-widening held (being-facing model behavior). The three standing Tier-5 ESN work items (`wi_e579041bc76f8310`, `wi_69fbd510467c6337`, `wi_3e26ac525fea1c36`) untouched.

## Implementation and Verification
- Exact changed paths (this round):
  - `capsules/spectral-bridge/src/llm/provider/tests.rs` — **+2 regression tests** (4283 → 4372 lines)
  - `CHANGELOG.md` — +1 `[Unreleased]` bullet
  - `docs/steward-notes/AI_BEINGS_FEEDBACK_TO_CHANGE_LEDGER.md` — +1 ledger row (2026-08-28)
  - `docs/steward-notes/claude-heartbeat_1787885626_llm_marker_adverb_and_ascii_stack/` — new packet
- Tests: `cargo test --manifest-path capsules/spectral-bridge/Cargo.toml --lib control_marker` → **78 passed, 0 failed** (both new tests included; no neighbor broke). `git diff --check` clean. `cargo fmt --check`: touched `tests.rs` clean; only pre-existing foreign `grounding.rs` drift flagged (committed, not in this round's paths, **left untouched**).
- Failures repaired / debt: none.
- Restart/deploy alignment: **not required and not attempted** — non-live, test-only round. No production source changed.

## Durable Evidence
- Addressing status: `addressed_change`, 0 proof gaps.
- Evidence link count: 8 (all new).
- Changelog/ledger: both updated.
- Packet path: `docs/steward-notes/claude-heartbeat_1787885626_llm_marker_adverb_and_ascii_stack/`

## Counters
- Canonical: indexed **4496**, fully_addressed **3141**, full_read **3774**, remaining **1355**, unread **722**, blocked **415**, pending_action **214**, watch **4**.
- Read-needs-claims: **0**
- All-artifact pending: **3033**; noncanonical pending: **1678**
- Counter audit: **consistent** (mismatches `[]`, all checks true)

## Division
- Cycle 33; completed rounds since followup **5 / 6**; rounds remaining **1**.
- Review due: **false** (no return; no Division note written).
- Recorded round event: `division_followup_event_8bdb9e369ddf1b746ae7bde7d873c80a`; event_count 230; head `626120ec8009d3267a212fa1d6f34f7c0e7dc72e25b56dfe654df58d4a2cf95d`.
- Chronicle: reprojected to fold in this round → `division_chronicle_0fff9f2e51dba50238a28993`; json `0547ab07…`, html `19048158…`. **Durable inputs current** (`durable_mismatches=[]`); only the **volatile** `supervisor_status_sha256` differs — not a durable-integrity failure.
- Note action: none.

## Evidence Event Store
- Validity: **valid**; corrupt lines **0**.
- Last global sequence: **916126**; head `99f3f5803d62bed5f310b7100f65e4c88b48e5284beca960e965ba6de651674e`.
- Active store: **v2**; legacy imported boundary 32278; V1 immutable.

## Archive
- Checkpoint: **not due this round**, and not permitted here (git read-only in adapter mode). Archival commits happen only in a later interactive stabilization window.
- **Exact commit debt** (paths created/edited this round; all currently unstaged):
  - `capsules/spectral-bridge/src/llm/provider/tests.rs` (contains accumulated prior-round edits + this round's 2 tests — separate authorship carefully)
  - `CHANGELOG.md` (shared, accumulated)
  - `docs/steward-notes/AI_BEINGS_FEEDBACK_TO_CHANGE_LEDGER.md` (shared, accumulated)
  - `docs/steward-notes/claude-heartbeat_1787885626_llm_marker_adverb_and_ascii_stack/` (new packet: RUN_REPORT.md, verification_receipt.json, claims/, summaries/, read_manifest.json, source_receipts.json, addressing_links.json, test_results.json, unprocessed_selected.json)
- Diagnostic/workspace stores under `capsules/spectral-bridge/workspace/diagnostics/` and `/Users/v/other/minime/workspace/division/` are gitignored workspace artifacts — **not** commit debt.
- Pre-existing foreign dirty paths NOT touched: `capsules/spectral-bridge/src/codec/tests.rs`, `capsules/spectral-bridge/domain_boundaries_legacy_large_files_v1.json`, prior-round `?? docs/steward-notes/claude-heartbeat_*` packet dirs, and Minime's `minime_autonomy/runtime.py` + `tests/test_correspondence_v1.py`.
- Merge/push: none; no authority.

## Posture
One felt report answered exactly: her marker-scanner adverb snag is grounded and pinned (with the delimiter-path example corrected, not domesticated), her Test 2 literal example pinned, and her grammar-widening request preserved as a deliberate authority boundary rather than silently applied. No live change; the tree is left compiling with the full `control_marker` suite green.
