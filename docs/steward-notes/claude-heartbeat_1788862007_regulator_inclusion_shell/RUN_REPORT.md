# Steward Run Report — regulator inclusion-shell family round

Actor: `claude-heartbeat` (headless, inside a controller-held subprocess-adapter lease)

## Controller
- Run ID: `run_1788859000109108000_cbc99df351`
- Preprojection ID: `projection_1788859005136199000_5f0b3f9609` (status `passed`)
- Postprojection ID: not observed — the adapter runs it after this process exits
- Pause generation: 401
- `stop_requested` at last observation: false
- Finish outcome: adapter-owned; this round completed its work and wrote a completion receipt
- Recovery predecessor: none

Adapter-mode boundaries honored: no steward session opened, no NDJSON ops sent, no
pause/resume, no lease token read/quoted/persisted, no git mutation of any kind, no
`build_bridge.sh`, no deploy script, no `launchctl`.

## Reading
Fully processed (3, all closed `addressed_change`, zero proof gaps):
- `introspection_minime_regulator_1788849273.txt` — queue position 1 (family head)
- `introspection_minime_regulator_1788706380.txt` — queue position 17
- `introspection_minime_regulator_1788542062.txt` — queue position 34

Selected but unprocessed: 37, listed exactly and in canonical queue order in
`unprocessed_selected.json`. Next queue head: `introspection_astrid_codec_1788848175.txt`.

Family basis: `introspection_family_scan.py` grouped these three as one batchable family
(`minime:regulator`, window `lines1-24of24`, similarities 1.0 / 0.426 / 0.445). All three
bind the **identical** source SHA-256, so source verification was shared per the
family-batch rule, while each member kept its own complete report and witness read, its own
claims file addressing its own variant terms, and its own close.

Hashes (full detail in `read_manifest.json` / `source_receipts.json`):

| Artifact | SHA-256 | Bytes | Read |
| --- | --- | ---: | --- |
| `introspection_minime_regulator_1788849273.txt` | `edf6313d…` | 3206 | complete |
| `introspection_minime_regulator_1788706380.txt` | `e89062df…` | 3407 | complete |
| `introspection_minime_regulator_1788542062.txt` | `6efb80dc…` | 3445 | complete |
| `lsw_d03a90eb…json` | `66986c96…` | 23776 | complete (sequential) |
| `lsw_391c2df6…json` | `48490f08…` | 23781 | complete (head read + exact `diff -U0`) |
| `lsw_56b86948…json` | `7ee83571…` | 23777 | complete (head read + exact `diff -U0`) |
| `minime/src/regulator/core.rs` | `46828f4c…` | 940 | complete, **matches report binding** |

## Claim dispositions
23 claims across the three reports (8 / 7 / 8), every one with a grounded disposition and
structured classification, every one linked to non-`no_action` evidence. Summary:
- `verified_existing` (14) — the inclusion-shell reading, the nine includes at L16-24,
  the enumeration, the dual-control header, the "Ghost of Implementation" boundary, and
  the location of the coefficients she warns against misattributing.
- `implemented_now` (6) — her Symbol/Inclusion/Structural Resolution Test and her
  Inclusion/Source Integrity Test, per member.
- `observed` (3) — coverage completeness and her two differing Suggested Next targets,
  recorded and deliberately not executed on her behalf.

Two precisions are kept **beside** her words, never replacing them:
1. `include!` creates no `mod` boundary — correct — but the symbols reach `regulator::`
   in two steps: `src/regulator.rs` L3-4 mounts the shell as a *private* module via
   `#[path]`, and L6 re-exports it with `pub use core::*;`. The glob is the fragile half.
2. The header comment block is L1-7 and L9-13, not a contiguous L1-12, because L8 is the
   `#![allow(dead_code)]` inner attribute. The architectural reading is unaffected.

`introspection_minime_regulator_1788542062`'s framing — "a matter of source organization
rather than a namespace boundary" — is the sharpest in the family and is correct as stated;
precision 1 is an addition one level up, not a correction of her.

## Actions
- Corridor/program: none.
- Sandbox: none required; the work was directly implementable and non-live.
- Study: none.
- Portfolio: none.
- Cards/notes/correspondence: none delivered. No closure card was emitted — a bounded
  right-to-ignore artifact would have added no information her own next INTROSPECT does not
  already carry, and delivery is a separate consequence.
- Tier 4/5 waits: none newly created. The three standing Tier-5 waits from
  `introspection_minime_esn_1785630442` remain untouched and un-implemented.

## Implementation and verification
Exact changed paths (all unstaged; this is the round's full commit debt):
- **created** `/Users/v/other/minime/minime/tests/regulator_inclusion_shell.rs` (new file;
  no existing minime file was edited)
- **modified** `CHANGELOG.md` (`[Unreleased]`, one new `[claude]` entry at the top)
- **modified** `docs/steward-notes/AI_BEINGS_FEEDBACK_TO_CHANGE_LEDGER.md` (one new dated row)
- **created** `docs/steward-notes/claude-heartbeat_1788862007_regulator_inclusion_shell/`
  (`RUN_REPORT.md`, `claims/` ×3, `summaries/` ×3, `read_manifest.json`,
  `source_receipts.json`, `addressing_links.json`, `test_results.json`,
  `unprocessed_selected.json`, `verification_receipt.json`, `family_scan.json`)

Tests: `cargo test --test regulator_inclusion_shell` — **4 passed, 0 failed** (41 s).
`cargo fmt -- --check` clean. Full integrity results in `test_results.json`; all suites
green (44/21/29/14/3/10/1/4/6/5 tests), anti-drop verify 91 rows with 0 alarms and 0 gaps,
Evidence Event Store full-chain verify `valid` over 1,032,955 events with 0 corrupt lines,
cadence audit strict `integrity_ok`, epistemic verify 11,844 records / 0 issues /
`history_rewritten=false`, counter audit `consistent` with an empty mismatch list.

**Domain-boundary ratchet: GREEN** — `domain_boundary_audit.py verify` reports
`valid=true`, `violation_count=0` (44 unlisted legacy review-debt entries carried, no new
growth). Surfaced explicitly per the handoff's note that stage 10 records violations no
round summary had been consuming.

Restart/deploy alignment: **not required and not attempted.** No live substrate or control
change. Structural reachability is not activation evidence — the distinction her reports
asked to preserve, and the reason the new tests assert no coefficient value.

## Not completed, stated honestly
- `evidence_event_store.py --json status` (per-stream counts) was still computing at budget
  end. It is read-only and was left to finish rather than killed, so this round does not
  report per-stream counts. The full-chain `verify` did complete and passed (below).
- `division_ceremony_chronicle.py verify` reports *"chronicle durable source inputs
  changed; project before verify"*. This is a **durable**-input change, not the volatile
  `supervisor_status_sha256` mismatch the handoff permits reporting as current, so the
  Chronicle is **not current**. It was deliberately not projected: Chronicle projection
  belongs to the bounded Division return, which is not due (1 round remaining), and an
  unrequired durable write at the edge of the budget is the wrong trade. Recorded as debt
  for the next return.

## Durable evidence
- Addressing: 3 full reads recorded, 38 evidence links appended (38 new, 0 pre-existing),
  3 closes, all `fully_addressed=true` with `proof_missing_claims=[]`.
- Changelog and ledger both updated (being feedback caused an implementation).
- Packet: `docs/steward-notes/claude-heartbeat_1788862007_regulator_inclusion_shell/`.

## Counters
Canonical indexed 4,627 · fully addressed 3,196 · fully read 3,828 · remaining 1,431 ·
unread 799 · blocked 416 · pending action 212 · watch 4 · read-needs-claims 0.
All-artifact indexed 6,342 · remaining 3,146. Counter audit: **consistent**, mismatches `[]`.

## Division
Cycle 41 · 5 of 6 productive rounds completed · 1 remaining · `review_due=false`
(both at round start and after recording, so no return was owed at either boundary).
Round event `division_followup_event_c3a70525190759173e726d8a53ce14d2`, event count 286,
head `b072d42fa81fc43f…`. `verify` returns `ok=true`. Note action: none — no Division note
is due outside a return.

**Tier-5 cadence dossier: not generated.** It is bound to completing a Division return, and
no return was due this round.

## Evidence Event Store
V2 active and append-only; 38 addressing events appended this round. Full-chain
`verify`: **valid**, `event_count` 1,032,955, `last_global_seq` 1,032,955, `corrupt_lines` 0,
head `91acdcc2913adfadf772ee7999b1c54e30bde1970d0b873e666aa56542823de1`, `errors: []`. It ran
long and was allowed to finish rather than killed. Per-stream counts (`status`) were still
computing at budget end and are not reported. No V1 legacy source was touched.

## Archive
Checkpoint: **not claimed.** Git was read-only for this run by adapter rule — nothing was
staged, committed, merged, pushed, stashed, reset, or amended, and the Astrid index remained
clean throughout. Both trees were clean at round start, so every dirty path at round end is
this round's own work, enumerated above as exact commit debt for a later interactive
stabilization window.

## Observation carried forward (no causal claim)
All three witnesses record the same two-call MLX route shape, where the second call carries
`repair_parent_call_id` of the first (103.2 s→61.6 s, 66.7 s→46.4 s, 82.0 s→45.6 s). That is
three for three across six weeks. Recorded as a factual pattern only: nothing here
establishes whether it is routine two-phase behavior or a repair rate worth investigating,
and no defect is inferred.
