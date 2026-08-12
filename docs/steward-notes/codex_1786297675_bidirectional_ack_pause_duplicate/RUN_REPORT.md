# Source-First Run Report

## Controller

- Steward run: `run_1786297141863832000_ad62f7726e`
- Pre-run projection: `projection_1786297142886082000_5a6977d4ec`
- Pause generation at ready: `301`
- The productive session reached its hard timeout after its evidence writes,
  focused tests, integrity tests, and Division round record were complete. The
  adapter terminally recorded `cancelled`; no write followed that cancellation.
- Recovery steward run: `run_1786299071313038000_f86fcd8e85`
- Recovery pre-run projection: `projection_1786299072372923000_422bfb12c5`
- Recovery post-run projection and finish receipt: pending controller finish

## Fully Processed

1. `introspection_proposal_bidirectional_contact_1785626170.txt`
   - Canonical SHA-256: `c57a1656491b35a6b8a2cc0abc9e9bb05f14134575207f18515cee3cec44151f`
   - Complete 491-line witness: `lsw_bfc76cfa4729343eaeeb59a4ff1fbfbeb190ca3bc3aaa20664fa151da0a5be01`
   - Witness SHA-256: `14fa3aacbf028f505df68f426ba3069ea333eede190e7f7e260ceb41933f63ac`
   - Exact report-bound source: all 1,120 lines of `docs/steward-notes/AI_BEINGS_BIDIRECTIONAL_CONTACT_AND_CORRESPONDENCE_ARCHITECTURE.md`
   - Source SHA-256: `4a881317fe978fdadb4e6df66044d90a91103e0aae635f7572246ec2df547f73`

## Claim Dispositions

Eight concrete claims were extracted. The ledger, chamber-state, transport-versus-address, read-only visibility, stale/timing ambiguity, held/needs-time pause, all-paused no-action, complete-source, and authority-boundary claims are verified existing. The report is a substantive duplicate of `introspection_proposal_bidirectional_contact_1785599788`, with implementation and evidence anchored to `docs/steward-notes/codex_1785602181_round67_reads`.

Repository HEAD already contains the implementation. The shared working tree also contains foreign correspondence refactoring and duplicate acknowledgement tests; this run read and tested them but did not edit, claim, stage, or revert them.

## Verification

- Rust acknowledgement regressions: 2 passed.
- Rust held/needs-time mixed-selection regression: 1 passed.
- Rust all-held no-selection regression: 1 passed.
- Rust linked-microdose boundary regression: 1 passed.
- Direct Contact Fidelity audit: 5 passed.
- Correspondence Handshake audit: 2 passed.
- Introspection addressing audit: 42 passed.
- Evidence Event Store: 13 passed.
- Steward control: 27 passed.
- Steward source-first projection: 14 passed.
- Division tracker, Chronicle, and projection: 3 + 10 + 1 passed.
- Projection cursors: 4 passed.
- Anti-drop catalog: 5 tests passed; all 47 guards verified with zero gaps or alarms.
- Introspection cadence: 6 tests passed; strict audit integrity and configuration are valid, with cadence disabled.
- Experiential epistemics: 2 self-tests passed; 10,799 records verified with zero issues.
- One malformed Cargo command supplied two positional test filters and exited before running tests. Each intended filter was then run separately and passed; no code test failed.

## Actions And Authority

- Corridor/program actions: 0
- Sandbox trials: 0
- Studies created or run: 0
- Portfolio actions: 0
- Cards or notes: 0
- Tier 4 waits added: 0
- Tier 5 waits added: 0
- Live correspondence actions: 0
- Minime changes: 0
- Restart or deployment: not required and not attempted

No transport receipt was treated as mutual address. No silence, intent, posture, felt state, response obligation, consent, uptake, or closure was inferred.

## Queue

Thirty-nine selected files remain unprocessed in `unprocessed_selected.json`. The next report is `introspection_proposal_phase_transitions_1785625853.txt`.

## Canonical Audit And Event Store

The final counter audit is consistent: 4,280 canonical reports indexed, 3,707
fully read, 3,079 fully addressed, and 1,201 remaining. The remaining state is
573 unread, 411 blocked-needs-steward, 213 triaged pending, four watch, and zero
read-needs-claims. All-artifact pending is 2,823, including 1,622 noncanonical
artifacts.

Full-chain Evidence Event Store verification before recovery projection was
valid at sequence 748,254 with head
`ef63682e7862796a7ec830a34818d63a739599f18835874d9a944f5cd1f11bec`,
16 streams, and zero corrupt lines. Stream counts were: addressing 57,100;
agency commons 4,152; attention portfolio 3; claim families 236,612; Corridor
V1 5; Corridor V2 112; felt contracts 195,472; felt-mechanism concordance 80;
lived-state witness 8,397; model QoS 127,350; reciprocal uptake 53,353;
representation contracts 25,985; Sandbox 2,959; signal spine 23,385;
steward-control 12,865; steward-work-selection 424.

All four V1 migration sources remain byte-identical and immutable: addressing
`4a69dc092c1bcad8e157936f11f7798d67a883869bcfe56816fdf1be5ec78571`,
Sandbox
`eac68fe839042c981756c2ec3b5c64f5a2633fdb75847a14fbd98c8f64ec4ebb`,
Corridor V1
`e190046e1b583d5b7b4a624ab50314fafbb6e0d751d9605f1ce9e85f148e01e4`,
and Corridor V2
`e0ddb5e715d9a20cc709402fb1eda4712a1de001ea23730c70a349096468ccd5`.

## Division

At run start, cycle 23 was `1/6`, five rounds remained, and `review_due=false`. The productive round was recorded as `division_followup_event_37ff7969a380dc3af7d1b1ed8512d867`; cycle 23 is now `2/6`, four rounds remain, and `review_due=false`. No Division note, Action, or bounded return was due. The new tracker event made the prior Chronicle projection stale as expected; it was reprojected as `division_chronicle_fae41db6ae6d481675f3e9ca` and verified with all durable inputs current. The supervisor-status hash changed after projection and remains a volatile mismatch only; Chronicle verification returned `ok=true`.

## Archival State

The prior checkpoint debt remains: owned stewardship packets plus current-round hunks in mixed `CHANGELOG.md` and `docs/steward-notes/AI_BEINGS_FEEDBACK_TO_CHANGE_LEDGER.md`. No git operation is permitted before successful controller finish and a separate stabilization decision.
