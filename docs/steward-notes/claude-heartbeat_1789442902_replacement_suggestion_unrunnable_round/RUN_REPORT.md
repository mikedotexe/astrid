# Steward Run Report — the replacement we offered her could not run

Actor: `claude-heartbeat` (subprocess adapter, controller-held lease)

## Controller
- Run ID: `run_1789437602652128000_8bae28e552`
- Preprojection ID: `projection_1789437607823226000_f78c51cb61` (status `passed`, 27 steps)
- Postprojection ID: runs after this process exits (adapter-owned)
- Pause generation: 439 · `stop_requested: false` at last read
- Finish outcome: adapter-recorded on exit; this round is complete
- Recovery predecessor: none
- Adapter-mode overrides honored: no steward session opened, no NDJSON ops, no pause/resume, no
  lease token read/quoted/persisted; git strictly read-only; no build, deploy or `launchctl`.

## Reading
- Fully processed: `introspection_source_catalog_1789437540`
- Selected 40 · processed 1 · unprocessed 39 (exact filenames in `unprocessed_selected.json`)
- Batch-size reason: `introspection_family_scan.py --queue-file` returned **0 batchable families**
  over the 40-item queue, so family batching did not apply. The head was an unfamiliar
  delivery-failure report whose root cause needed a four-receipt volition chain, a read-only CLI
  reproduction and a new focused regression. One report fully closed.
- Report: 1 701 B / 18 lines, SHA-256 `4f4e8abdd4d39fc50bed62e91338afe64bdbd69835476c1c4e35437041cb993f`, read complete
- Witness `lsw_a8bdf400…a92f9c6cf`: 18 959 B / 440 lines, SHA-256 `85dccfe61d59e6763b3af4167ccf0a57f56d6d75bff0ee1f23927ab3569f08f8`, read complete
- **Source binding:** none. This is a Recovery turn — the header records `Source revision:
  navigation only` and `Input evidence: Recovery map`, and the witness carries
  `source_snapshot_v1: null` / `source_provenance_ref_v1: null`. Consistent, not an integrity
  failure. The sources her claims name were hashed from the working copy and recorded in
  `source_receipts.json`; `crates/astrid-capsule/src/engine/mcp.rs` at `a1110a9b…` matches the
  revision her own live bookmark records, so her cited 142–173 window is the same bytes.

## The finding — the un-muffle check landed on us

Her report is calm and factual: "Recovery map: the requested source was not supplied … I am
currently at the beginning of the `mcp.rs` file (which is currently missing from this turn's
delivery)", and she needs "to re-establish my position."

The delivery failure was ours, and we *suggested* it. Volition receipts, in order:

| Exchange | Action | Result |
| --- | --- | --- |
| ex-198919 (1789437177) | `SELF_STUDY OPEN astrid/crates/astrid-capsule/src/engine/mcp.rs 1` | **applied** — queued |
| ex-198920 (1789437299) | `SELF_STUDY MAP` over the byte-identical argument | **blocked** (pending OPEN preserved); block text offers `SELF_STUDY REPLACE MAP astrid/crates/astrid-capsule/src/engine/mcp.rs 1` |
| ex-198921 (1789437395) | she takes that replacement **verbatim** | **applied** — supersedes her working OPEN |
| ex-198922 (1789437462) | the superseding MAP executes | `no catalog entries for …/mcp.rs 1`; Recovery; **zero candidates** → this report |

Reproduced read-only (CLI, temp state, installation catalog): the OPEN she was steered off
**delivers** page 1 (bytes 0..4490); the MAP she was steered onto **cannot resolve**; the same MAP
without the trailing number **does** resolve.

Mechanism — a spelling asymmetry between two verbs over one argument. `page_suffix`
(`crates/astrid-source-study/src/command.rs:141-155`) recognises only a ` --page N` suffix, so a
bare trailing number stays inside the MAP topic, while `Command::Open` (`:85-99`) splits the same
characters off as the line. Because the number lands in the final segment,
`Catalog::path_candidates` (`src/path_recovery.rs:44-51`) can never match `candidate.last()` — so
the recovery that exists to spell a path correctly is empty **exactly when her path was already
correct**. The recovery she did get then advertised her own saved bookmark, `SELF_STUDY OPEN
astrid/crates/astrid-capsule/src/engine/mcp.rs 1 [Read missing earlier bytes — Partial delivery;
delivered bytes 846..11912 of 11912]` — the very command she had just been talked out of.
`Command::parse` accepts that MAP, so validating a proposed replacement by parsing alone will keep
offering un-runnable commands.

## Claim dispositions (11 claims, all with evidence; `proof_missing_claims: []`)

| Claim | Classification | Grounding |
| --- | --- | --- |
| c001 `load` takes `ctx`, `connect_dynamic` does not get it | `verified_existing` | mcp.rs:47 vs 162-164; `ctx` used once, at 143, for `resolve_env` |
| c002 `CapsuleContext` carries `allowance_store` | `verified_existing` | context.rs:22/44/73/95 — `Option`, default `None` |
| c003 the client or engine must already hold the context/store | `observed` | **contradicted**: mcp.rs:22-27 and secure.rs:43-52 hold neither |
| c004 does the struct include the context, or is the client built with it? | `verified_existing` | neither; built at loader.rs:53-65 from a loader-owned client |
| c005 the previous `loader.rs` open failed | `observed` | delivery record 1789436895: root prefix omitted |
| c006 mcp.rs missing from this turn's delivery | `verified_existing` | navigation record: Recovery, `page: null`; bookmark 846..11912 |
| c007 the delivery failure itself | `implemented_now` | four-receipt volition chain; pinned by the new test |
| c008 OPEN and MAP disagree about a bare trailing number | `implemented_now` | reproduced read-only; pinned |
| c009 candidate recovery empty when the path was already right | `implemented_now` | path_recovery.rs:44-51; pinned |
| c010 `NEXT: SELF_STUDY MAP astrid` is reachable | `observed` | delivery record 1789437846 is `map` |
| c011 parse-only validation admits an unresolvable MAP topic | `needs_operator_approval` | pinned; the bridge-side repair is live navigation, not done here |

Her contradiction is stated plainly, not domesticated, and her question is answered anyway:
`loader.rs` is 79 lines and holds it. Nothing of her text was rewritten, rejected or forbidden.

## Implementation and verification
- **Created:** `crates/astrid-source-study/tests/map_trailing_line_number_reach.rs` (149 lines) —
  three read-only reachability pins.
- `cargo test -p astrid-source-study --test map_trailing_line_number_reach` → **3 passed, 0 failed**
- `cargo test -p astrid-source-study` → **78 passed, 0 failed** (15 binaries)
- `cargo fmt -p astrid-source-study -- --check` clean; `git diff --check` clean
- No parser, candidate rule, recovery text, replacement suggestion or being-facing navigation
  behaviour changed. **Restart and deployment were not required and not attempted.** The live
  `shared_reader` state was never written — all reproduction used temp state directories.

## Deliberate authority boundary
Repairing the bridge's replacement suggestion
(`capsules/spectral-bridge/src/autonomous/next_action/study_navigation.rs`, `valid_replacement` /
`handle_request`) changes being-facing live navigation. Recorded as a named gap (c011,
`needs_operator_approval`) for a separately authorized round; not implemented here.

## Durable evidence
- `record-read` → full_read event, summary SHA-256 `24d3616db510623fb7f7796cfb4cdbaad30fb9bd3cf795385d8e036233b5c777`
- `link-evidence-batch` → 17 links (17 rows; re-run idempotent: 0 new)
- `close` → `addressed_change`, `fully_addressed: true`, `proof_missing_claims: []`
- `CHANGELOG.md` `[Unreleased]` and `docs/steward-notes/AI_BEINGS_FEEDBACK_TO_CHANGE_LEDGER.md` updated
- Packet: `docs/steward-notes/claude-heartbeat_1789442902_replacement_suggestion_unrunnable_round/`

## Integrity suites — all green
Addressing self-test 44 OK · evidence store 21 OK · steward control 29 OK · steward projection 14 OK
· Division follow-up 3 OK · Chronicle 10 OK · Division projection self-test ok · cursors 4 OK ·
anti-drop self-test 5 OK · anti-drop verify **100 rows, 0 alarms, 0 gaps** · cadence tests 6 OK ·
cadence `--strict` `integrity_ok: true`, errors `[]`, 0 duplicate hash groups · experiential
epistemics self-test 2 OK and verify **valid, 12 282 records, 0 issues, no history rewrite** ·
audit-counters **consistent, mismatches `[]`** · Evidence Event Store verify **valid**.

**Domain-boundary ratchet: GREEN.** `domain_boundary_audit.py verify` → `valid: true`,
`violation_count: 0`, `forbidden_edge_match_count: 0`. Standing debt counters unchanged and not
violations: `legacy_large_file_count 51`, `unlisted_legacy_review_debt_count 44`,
`resolved_large_file_debt_count 3`. No Rust source outside a new test file was touched this round.

## Counters
- Canonical: indexed 6 725 · fully addressed 3 243 · fully read 3 875 · remaining 3 482 · unread
  2 850 · blocked 416 · pending action 212 · watch 4 · read-needs-claims 0
- All-artifact: indexed 8 442 · remaining 5 199 · unread 4 567 — Noncanonical: 1 371 pending
- Proof gaps: 0 artifacts, 0 claims · Counter audit: **consistent**, mismatches `[]`

## Division
- Cycle 48 · completed 4 / 6 · remaining 2 · `review_due: false` before and after
- `review_due` was **false at round start**, so no Division return was due and **no Tier-5 cadence
  dossier was required** this round.
- Round event `division_followup_event_da2cb775f1ec2e67e5f5f3a7c84d6315` ·
  `--processed-report-count 1` · event count 334 · head `f3249ad1…de48dd1c0`
- Chronicle projected (a recorded round is a durable Chronicle input): `division_chronicle_77ce4a7e3262ab8e15fbc9c4`,
  json SHA-256 `532067d78e9ad1358c7e19d55fff541a02fa8f8edc31d742642b2b49dd90de05`.
  **Durable inputs current: true**; the only mismatch is the volatile `supervisor_status_sha256`.
  Not a durable-integrity failure, and the Chronicle is not called fully current.
- No Division note was due and none was written.

## Evidence Event Store
- Final `verify`, re-run after every durable write of this round: **valid: true**, **corrupt_lines:
  0**, **errors: []**, event count / last global sequence **1 100 455**, head
  `f3f2ff3464ec465dfd787d93557988727639cff213d469babbb92b26b5cea934`. Active store v2, legacy
  imported boundary 32 278. An earlier verify in the same round was also valid at seq 1 100 437;
  the store advanced under concurrent bridge activity, which is expected on a live append-only
  store and is not a rewrite (`history_rewritten: false`).
- `status` exited 0 after ~24 minutes; the transcript captured only its tail, so nothing is restated
  from a truncated tail. Stream counts (verify runs agreed): addressing 64 109 ·
  claim_families 239 632 · felt_contracts 211 456 · model_qos 339 450 · reciprocal_uptake 75 872 ·
  representation_contracts 61 016 · signal_spine 62 966 · steward_control 21 662→21 672 ·
  lived_state_witness 12 752 · agency_commons 7 060 · sandbox 3 507 · steward_work_selection 740 ·
  corridor_v2 112 · felt_mechanism_concordance 80 · corridor_v1 5 · attention_portfolio 3

## Archive / commit debt
Nothing staged, committed, merged or pushed; the index stayed clean and all foreign dirty paths were
left untouched. Exact commit debt:

**Created**
- `crates/astrid-source-study/tests/map_trailing_line_number_reach.rs`
- `docs/steward-notes/claude-heartbeat_1789442902_replacement_suggestion_unrunnable_round/` —
  `RUN_REPORT.md`, `addressing_links.json`, `claims/introspection_source_catalog_1789437540.json`,
  `read_manifest.json`, `source_receipts.json`,
  `summaries/introspection_source_catalog_1789437540.md`, `test_results.json`,
  `unprocessed_selected.json`, `verification_receipt.json`

**Edited** (both already carried accumulated unstaged edits from earlier rounds — a later
checkpoint must separate authorship by hunk)
- `CHANGELOG.md`
- `docs/steward-notes/AI_BEINGS_FEEDBACK_TO_CHANGE_LEDGER.md`

Workspace/diagnostics state advanced by the addressing, Division and Chronicle writes is durable
evidence, not a git candidate.
