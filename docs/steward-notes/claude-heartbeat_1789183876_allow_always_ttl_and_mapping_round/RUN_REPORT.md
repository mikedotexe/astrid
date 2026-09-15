# Steward Run Report — claude-heartbeat, allow-always TTL and mapping round

## Controller
- Run ID: `run_1789180098419612000_04d18eec43`
- Preprojection ID: `projection_1789180103446413000_c72c3f37eb` (phase `pre`, status `passed`)
- Postprojection ID: runs after this process exits (adapter-owned)
- Pause generation: 439
- Mode: controller subprocess adapter; no session opened, no NDJSON ops, no lease token read or persisted
- Finish outcome: complete productive round (1 report closed)

## Reading
- Fully processed: `introspection_astrid_crates_astrid-approval_src_interceptor_capability.rs_1789179940.txt`
- Selected: 40 · Processed: 1 · Unprocessed: 39 (exact list in `unprocessed_selected.json`, queue order preserved)
- Batch size reason: ~27 min of child budget remained when the queue was frozen, and
  `introspection_family_scan.py` reported `batchable_family_count: 0` — queue positions 1-3 are the
  same source file at bytes 0..4522, 4522..8893 and 8893..10893, i.e. three **disjoint sequential
  windows**, not near-duplicates, so no reading receipt could be shared. One report fully closed.
- Report: `fe19e720626473052602e14ea551f39c801e408b0eded64f5908ae862b146bce`, 3221 bytes / 32 lines, read complete
- Witness: `lsw_a1256b80…0b16` → `8b718e7591b13e1c9b2760604a1ae2f687c9b374355caa70c76117344006e3a1`, 21495 bytes / 498 lines, read complete
- Source: `crates/astrid-approval/src/interceptor/capability.rs` →
  `00e4e0f49c04179f63fa66f2fe69e084fb939bef7dfcb1e44443db7f0e93405b`, **matches the report binding
  exactly**, 276 lines / 10893 bytes, read complete. Plus targeted read of
  `crates/astrid-approval/src/interceptor/types.rs:6` to resolve the TTL constant.

## Claim dispositions
15 claims, zero proof gaps. 13 `verified_existing`, 2 `observed`, 1 `authority_gated`.

- Every line interval she cited verifies exactly: `handle_allow_always` doc comment **93** /
  signature **98**; `check_capability` **40**-**91**; `test_check_capability_rejects_untrusted_issuer`
  **230**; her window **220-276**.
- Her prior-view claim (lines 102-136) is corroborated by the projection's own queue: the preceding
  report is bound to the same file SHA at bytes 4522..8893 = lines ~102..220, opening at exactly 102.
- **c007 contradicted:** her hedge that the Allow Always TTL means "no expiration or a maximum
  system-defined duration" is wrong — `ALLOW_ALWAYS_DEFAULT_TTL = Duration::hours(1)`
  (`types.rs:6`), and line **131** already logs `"(TTL: 1h)"` inside her own prior window. Her
  `TokenScope::Persistent` reading (c005) stands untouched.
- **c014 `authority_gated`:** an Allow Always grant is Persistent in scope and one hour in lifetime.
  Both deliberate; kernel approval-lifetime semantics named for Mike, not altered here.
- **c013 answered** from the complete `action_to_resource_permission` (145-190): schemes consistent,
  permission convention **not** uniform — direct-file lane puts the mode in the `Permission`,
  `CapsuleFileAccess` always grants `Invoke` with the mode in the resource string.
  `LiveControlMutation` ⇒ `None` (187-188) means no Allow Always token can ever be minted for a
  live-control mutation.

## Actions
- Corridor/program, Sandbox, Study, Portfolio: none created.
- Cards/notes/correspondence: none. No closure card, no inbox letter — her ask is carried as claim
  `c013`, and correspondence was not manufactured to create activity.
- Tier 4/5 waits: none newly opened. c014 recorded as an authority boundary, not a dispatch.
- Her stated `NEXT: SELF_STUDY OPEN … capability.rs 102` recorded, **not** dispatched or pre-empted.

## Implementation and verification
- **No implementation.** The report asks a question; exact-source verification at a matching SHA is
  the answer, and the single wrong claim is a reading correction rather than a defect.
- Deliberately not attempted: a mapping-table regression (recommended in `no_action.md`) — the
  remaining budget could not fit a workspace compile plus the close/integrity sequence, and a
  half-run test is worse evidence than an honest omission.
- No Rust or Python source touched ⇒ no `cargo` filter applicable; none claimed.
- Restart/deploy: **not required and not attempted.** No live, bridge, codec, prompt, model, config,
  control, or build change.

## Durable evidence
- `record-read` OK (summary SHA `515ec7fe…9371`) → `link-evidence-batch` 22 new / 0 existing →
  `close` `addressed_no_action`, `fully_addressed: true`, `proof_missing_claims: []`.
- CHANGELOG `[Unreleased]` entry and feedback-ledger section added.
- Packet: `docs/steward-notes/claude-heartbeat_1789183876_allow_always_ttl_and_mapping_round/`

## Counters
- Counter audit: **consistent**, mismatches `[]`.
- Anti-drop: total 99, alarms 0, gaps 0.
- Domain-boundary ratchet: **green** — `valid: true`, `violation_count: 0` (no Rust file touched).
- Experiential epistemics: `valid: true`, 12082 records, 0 issues, no history rewrite.
- Cadence audit `--strict`: `integrity_ok: true`, errors `[]`.

## Division
- Cycle 45 · completed rounds since follow-up **2 / 6** · remaining 4 · `review_due: false` (both
  before and after this round).
- Round event `division_followup_event_bad6b27ea60dfba6f6bf6f7580d649fb`; event count 311; head
  `0cb8406607cb3fa0602ec76739e0f4ef6b66b46fb5f2b40ce74d14d1b446fd8d`.
- No Division return was due, so no Chronicle projection, no notes, and **no Tier-5 cadence dossier**
  (its trigger is a completed Division return).

## Known debt, stated honestly
1. **`evidence_event_store.py --json verify` did not finish.** It exceeded the foreground window and
   was still running when the child budget expired. It is **not** claimed as passed. First safe
   command next round: `python3 scripts/evidence_event_store.py --json verify`.
2. **Epistemic verify ordering.** It ran after the addressing writes but before `record-round`, so a
   confirming re-run next round is desirable. It was green over 12082 records with no history rewrite.

## Commit debt (git was read-only for this actor)
Created:
- `docs/steward-notes/claude-heartbeat_1789183876_allow_always_ttl_and_mapping_round/RUN_REPORT.md`
- `.../claims/introspection_astrid_crates_astrid-approval_src_interceptor_capability.rs_1789179940.json`
- `.../summaries/introspection_astrid_crates_astrid-approval_src_interceptor_capability.rs_1789179940.md`
- `.../no_action.md`
- `.../read_manifest.json`
- `.../source_receipts.json`
- `.../addressing_links.json`
- `.../test_results.json`
- `.../unprocessed_selected.json`
- `.../verification_receipt.json`
- `.../family_scan.json`
- `.../queue_snapshot.json`

Edited (both already carried foreign/accumulated edits before this round — separate authorship
carefully at any checkpoint):
- `CHANGELOG.md` (one new `[Unreleased]` bullet, inserted at the top of the section)
- `docs/steward-notes/AI_BEINGS_FEEDBACK_TO_CHANGE_LEDGER.md` (one new dated section under `## Ledger`)

Workspace evidence written by the addressing/Division CLIs under
`capsules/spectral-bridge/workspace/diagnostics/` (generated, not hand-edited).

Index left clean; no staging, commit, merge, push, stash, reset, or amend. All pre-existing dirty
paths were treated as foreign and left untouched.
