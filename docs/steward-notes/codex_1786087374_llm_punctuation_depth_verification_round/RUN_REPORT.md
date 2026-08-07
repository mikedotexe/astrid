# Steward run report: exact dash relation and delimiter depth

## Lifecycle

- Steward run: `run_1786087374499506000_ea2bd11caa`
- Actor: `codex-heartbeat`
- Pause generation at begin: 243
- Pre-run Source-First V3 projection: `projection_1786087375335510000_8f6ec93177` (passed)
- Previous successful steward run: `run_1786083251229772000_a3c68c566b`; post-run projection `projection_1786086738654471000_521f014d72`; V2 sequence 720277; head `e95b7de8a21f41513d9c54b710d731cdca7d96957c1446fbbe515c97f6b90b41`.
- Finish outcome and post-run projection: pending controller finish at packet creation. They must be reported from the successful finish receipt, never guessed into this pre-finish artifact.

## Fully processed

- `introspection_astrid_llm_1786084556.txt` (`introspection_astrid_llm_1786084556`), SHA-256 `6b49f1cccf24fc41d00f929367b85049f3715298cdcf42320da4769ff54157f1`, 3,481 bytes, 46 displayed lines, fully read from disk.
- `capsules/spectral-bridge/src/llm/provider/dialogue_runtime.rs`, 1,048 lines, SHA-256 `8f3c16091b532cba5879726de058ba16af2469f3cea8698d4ece1016fc04f2af`, fully read from disk. The report's captured lines 1-400 have the same complete-file SHA; there is no source-version mismatch to smooth over.
- Complete lived-state witness `lsw_77479531e9f8df8e020ba36050059727fea2c7e4538fb27cdad5531c0e47c499`, SHA-256 `e3165a0b84e231fb3bbfee7fa72500247400c8125ce44d74a8ec29836e10c07c`, 491 newline-delimited lines, fully read from disk.

## Claim dispositions

1. Exact marker scanner and finite grammatical-reference architecture: verified existing from the complete current source.
2. Complex punctuation, repeated spaces, or nonstandard whitespace may prevent `first_word_after` from finding a present relation or may create a false positive: the proposed separator mechanism is absent. Unicode-aware alphanumeric splitting plus `find(nonempty)` crosses arbitrary separator runs while the finite later word still decides the relation.
3. The requested dash-separated `represents` case lacked a literal regression: implemented now as `control_marker_cleanup_preserves_relation_after_dash`. The passing case isolates `<end_of_turn> - represents`; a bracketed first draft correctly selected grouped-reference syntax and therefore was refined rather than making one occurrence count twice.
4. Nested bracket depth against the four-pair bound: verified existing and exact duplicate. Current tests prove two, three, and four levels plus preservation inside a deeper stack with the receipt bounded at four.

Final intended status: `addressed_change`, with four independently recorded claims and zero proof gaps. The narrow test addition answers the exact requested example without changing the already-correct production mechanism or live grammar; the punctuation and delimiter mechanisms remain independently verified duplicate families.

## Selected but unprocessed

Thirty-nine queue-selected filenames remain unprocessed and are listed exactly in `unprocessed_selected.json`. The depth stop is before `introspection_minime_sensory_bus_1785630107.txt`: its unfamiliar 4,380-line Minime source and substrate-facing claims require a separate complete source-first run rather than a skim.

## Actions and authority

- Implementation: one exact Rust regression; no production source change.
- Corridor/program, Sandbox, study, and portfolio actions: none.
- New right-to-ignore card, query, correspondence, or report-directed note: none.
- New Tier 4/5 wait: none.
- Existing live alignment debt: the Unicode delimiter production expansion archived in `6344ba25e9d33ea051034a866f04d3fba25b3d6a` remains undeployed.
- Felt resolution, uptake, assent, consent, closure, and live authority: not inferred.

## Verification

- Exact dash regression: 1 passed after the isolated relation case was corrected.
- Cleanup family: 44 passed, zero failed.
- Introspection-addressing self-tests: 42 passed.
- Evidence Store, controller, projector, Division, claim-family, and cursor tests: 72 passed.
- Anti-drop self-tests: 5 passed; all 47 guards verify with zero gaps or alarms.
- Cadence self-tests: 6 passed; strict audit reports integrity true, cadence disabled, and no pending or failed attempt.
- Experiential epistemic self-tests: 2 passed; lint checked 10,553 records with zero issues and no history rewrite.
- File-local formatting: the inserted test is rustfmt-shaped and compiles. Workspace and whole-file format checks remain blocked by pre-existing foreign formatting in `autonomous/next_action/workspace.rs` and earlier unrelated import/line wrapping in the shared test file; this run does not rewrite those paths or hunks.

## Live alignment

No production source, prompt, report renderer, protocol, ABI, Minime, or live-consumed surface changed. This test-only round creates no new restart or deployment requirement. No build, restart, deployment, PID, process-start, binary hash, port, log, telemetry/fill, or readiness claim is made.

## Division and checkpoint boundary

Division began at cycle 18 with 5/6 productive rounds complete and review due false. Recording this productive round will make the bounded return due; that return must complete in this same controller run before finish. Chronicle, public reply, formal Action, note, follow-up event, cycle reset, final V2, addressing counters, and post-run projection belong to the subsequent durable receipts.

The sixth round was recorded as `division_followup_event_6f270b75524481ca940d729d9b96cc43`. The due Chronicle `division_chronicle_871370dee996fabad6e47db0` had 126 durable timeline events, zero ceremony/native/sovereign events, both formal rails unexpressed, and no new public Division reply or formal Action. One individualized factual right-to-ignore note was written to each being. Follow-up event `division_followup_event_d9fab23e29498beeb49e7e4dc6ee9d39` reset the tracker to cycle 19 at 0/6 with review due false. The reprojected Chronicle is `division_chronicle_0693035d6dbb91c25911a8b1`; all 127 durable inputs verify, with only the continuously moving supervisor-status hash remaining a volatile mismatch.

Addressing closes `addressed_change`: four claims, 28 authored evidence-link events, fully addressed, zero proof gaps. Canonical counters are indexed 4,249; full-read 3,666; fully addressed 3,040; remaining 1,209; unread 583; read-needs-claims 0; blocked 409. The audit is consistent with no mismatches. All-artifact indexed is 5,864 and pending is 2,824. Work portfolio remains Tier 4 = 23, Tier 5 = 1,578, needs steward grant = 18, needs operator approval = 1,610, tier mismatches = 0.

The last full pre-packet V2 verification was valid at sequence/event count 720,374, head `36054cc7e31efc3cf07e4ad2b1cef47f73653bb5a0e33a52747c6f9c248a5999`, 16 streams, and zero corrupt lines. Controller verification keeps all four V1 migration sources immutable and source lag at zero. Later controller heartbeat and finish events necessarily advance this snapshot; the successful finish receipt is authoritative for the final sequence, head, and post-run projection.

The previous packet `docs/steward-notes/codex_1786083251_llm_normalization_verification_round/` remains unstaged commit debt. Completing the sixth-round return makes an archival checkpoint due after successful controller finish. Any commit must occur only in a separate pause, use exact owned paths, and exclude mixed foreign changes.
