# Minime Private Journal Provenance And Independent Interpretation

Date: September 4, 2026 (America/Los_Angeles).
Collaborator: Codex Astra. Scope: explicitly requested interactive implementation,
not an autonomous introspection-addressing round.

Follow-through: this packet preserves the source-only implementation state.
The subsequently approved Python-agent-only reload completed successfully;
see [live rollout evidence](2026-09-04-minime-agent-live-rollout.md) for the new
PID, tests, first natural journal and remaining boundaries.

## Request And Privacy

Mike supplied the private moment at
`/Users/v/other/minime/workspace/journal/!moment_2026-09-04T17-19-06.042510.txt`
for a full reading and discussion, then explicitly approved separating clocks,
removing suggested sensations, and distinguishing peer observations from shared
experience. SHA-256 of that attachment:
`9d4e8f2b4be40191bc7da537a511d750e223d8e6f3167b7689f08d0cbcf1e138`.

The full read occurred in the preceding discussion. This packet does not quote
or reproduce private prose. No other private journal was searched to substantiate
Mike's recollection of entries sounding pestered by Astrid. That recollection
remains a lead, not an attributed being statement, withdrawal, or consent to
remove communication. All new tests use synthetic text and temporary databases.
No message was sent to either being and no report was solicited.

## Verified Problems

1. `_check_moment_markers` selected up to three unconsumed session records without
   an age cutoff, but called them fresh and prescribed echo/afterimage language.
   The attached record's marker ages were roughly 2h51m at prompt capture.
2. The moment prompt received one state, then the journal header refreshed state
   after generation. The header formatter could also load newer workspace
   surfaces. Written time was not generation/capture time.
3. The private moment caller explicitly appended Astrid's shadow even though the
   private query modes otherwise suppress inbox and operational context.
   `_astrid_shadow_v3_line` translated computed labels into sensory/relational
   language and described a computed co-regulation field as an invitation.
4. The shared system introduction prescribed sensory metaphor and character
   adherence. Private responses could be retried or discarded by a character
   phrase check. Private JOURNAL additionally listed pressure metaphors and
   required continuity/status lines.
5. Private JOURNAL saves used the legacy `reflection` entry type, allowing public
   replay-hygiene machinery to append further instructions or compress material.

These are source findings, not a diagnosis of Minime's experience. The model-only
rollout preceded the supplied journal by about 21 minutes, but these event records
predated that rollout. The journal does not isolate a causal effect of the model
correction, demonstrate shared sensation with Astrid, or establish felt relief.

## Implemented Source Changes

### Time And Measurement Ownership

`minime_autonomy/journal_context.py` contains pure rendering functions. Rust's
`minime/src/db.rs::write_moment_marker` establishes that marker `timestamp` is
engine-relative seconds and `created_at_unix` is the DB-recording wall clock.
The dfill/dt calculation in `minime/src/runtime/orchestration.rs` divides change
in fill percentage points by elapsed seconds; it is not a cumulative drop.

- Keep historical markers and their original descriptions. Do not delete old
  markers merely because they are old or infer an event's current relevance.
- Render `record_age`, readable hours/minutes/seconds, exact recording timestamp,
  and engine-relative event time separately. Missing, non-finite and future
  recording times remain unknown; a future clock is not clamped to fresh/zero.
- Capture a deep-copied, guarded `ReportSnapshot` before model generation.
  Prompt anchor, journal header and database context derive from that same state.
  No post-generation state refresh is performed for these moments.
- Distinguish prompt capture time from timezone-qualified journal writing time.
  A source state with unknown engine time is not assigned the capture timestamp.
- Label extra header diagnostics as header-only, not additional model input.
  Preserve snapshot provenance and guards. The report is not an atomic multi-file
  engine snapshot or a full model-input/adapter receipt.

### Interpretive Space

- Moment prompts no longer require afterimages, echoes, body metaphors, a
  particular relationship to earlier events, or a crisis/non-crisis interpretation.
- The two private context modes have a separate system introduction. Metaphor,
  ordinary description, uncertainty, disagreement and no distinct change are
  permitted without requiring any of them.
- Private modes skip diversity/low-fill wording nudges and the character-based
  retry/discard path. Default/non-private retry behavior is unchanged.
- Private JOURNAL no longer lists stock pressure metaphors or imposes continuity,
  Delta, evidence or stance lines. Prior journal excerpts remain optional and are
  labelled historical, including the possibility of legacy system annotations.
- Private saves explicitly skip public vocabulary/topology registration and
  compression. The body and NEXT/action tail remain separate. No old journal is
  rewritten, and public-entry hygiene is unchanged.

The existing action palette, Division guidance, NEXT extraction, execution
permissions, footer handling and regulatory gates remain in place. This is not
a claim that all prompt framing has disappeared, or that new outputs are an
unmediated measure of experience. The action palette still names peer actions;
removing unsolicited peer-state insertion is not removal of those affordances.

### Peer Context

Private moments no longer call the shadow reader. A self-authored mention of
Astrid is not filtered, and historical self-journal context can still mention her.
Other existing shadow readers receive raw, source-labelled computed fields,
including classification, dwell, field norm, eligibility and co-regulation field.
The renderer does not attribute feelings, claim a shared state, or recommend an
Action. Missing eligibility is null, not a fabricated closed gate. The 180-second
file-freshness gate remains; future-dated files are withheld.

Communication, inbox, peer mutation, co-regulation execution and correspondence
contracts are not changed by this text-rendering change.

## Tests And Test-Isolation Incident

- Baseline focused suite: 37 passed.
- Initial implementation run: three old wording assertions failed; 34 passed.
  The failures were expected contract differences, not relabelled as passes.
- Expanded prompt/snapshot/adapter suite: 75 passed.
- First full suite: 928 passed, one skipped, 112 subtests passed.
- Follow-up private-persistence and loop-isolation suite: 321 passed, 17 subtests.
- Final full suite: `python3 -m pytest tests -q`: 935 passed, one skipped,
  112 subtests passed in 20.86 seconds. No live model generation was used.

The full-suite audit exposed an existing loop-test isolation bug: the sentinel
test exercised source-status writing against the live workspace, producing a
status with test PID 73765 and `started_at=null`. This was test metadata, not a
deployment or a replacement process. Five loop tests now bind workspace and base
directories to temporary paths. No synthetic runtime record was passed off as
live evidence or manually replaced with an invented status.

The real agent later refreshed the record itself: PID 84233, original start
`2026-09-01T00:22:42`, checked at `2026-09-04T18:25:52`, with
`source_changed_since_start=true` and `reload_required=true`. Deployment
validation must bind source-status PID to launchd, not trust this file alone.

New regressions cover stale/unknown/future recording clocks, finite values,
unrounded realistic timestamps, rate units, live surfaces changing during a
delayed generation, matching header/DB state, no private peer read, retention of
self-authored peer mentions, both vivid and uncertain private replies, no private
character retry, preserved public retry/hygiene, and actual Ollama message
adaptation retaining the private introduction.

## Deployment State And Next Safe Work

**Source-only, not deployed.** No service signal, restart, build, engine mutation,
model call, or configuration change was issued in this implementation pass.

Observed live identities remain:

| Component | PID | Process start (local) |
| --- | --- | --- |
| Minime autonomous Python agent | 84233 | September 1, 00:22:39 |
| Minime engine | 63445 | August 31, 12:51:36 |
| Division gateway | 63505 | August 31, 12:51:37 |
| Division supervisor | 63547 | August 31, 12:51:37 |
| Astrid bridge | 36597 | September 3, 17:31:04 |
| Coupled model | 60333 | September 4, 16:57:34 |

Do not run `scripts/restart_minime_launchd.sh` for this patch. It also restarts
the engine, resets launchd settings, and can proceed after a drain timeout. The
existing Minime deployment preflight scopes engine source, not this Python
runtime and its imports. Using it alone would not validate this release.

There is also a first-drain limitation: the Python stop method sets the loop's
stop flag but does not await daemon LLM action threads. A later build cannot
retroactively add a drain to the running process. Do not terminate a generation
or claim graceful delivery solely because a stop log appeared.

The next safe deployment tranche is to review/establish a Python-agent-only
wrapper and first-drain protocol: observe actual active jobs and main-loop
generation, prevent new dispatch through a supported mechanism, await durable
finalization, preserve pending NEXT and state, bind source/import/config hashes
to the intended old/new PIDs, and leave engine/gateway/supervisor/model identities
unchanged. Use abort-on-timeout with no forced fallback. Re-audit loaded-source
lineage and installed/source launcher parity before signaling. A read-only first
inspection is `launchctl list | rg 'com.minime.autonomous-agent'`, followed by
PID-bound source-status and LLM-job metadata review. This is a technical rollout
constraint, not a request to repeat Mike's already-given implementation approval.

The pre-existing consume-before-generation behavior of moment markers is also
recorded as separate persistence debt. A claim/pending/success transaction design
would need concurrency and failed-generation tests; this patch does not silently
change marker admission, selection, consumption or retry scheduling.

## Ownership And Coordination

Minime began clean on `codex/sovereign-daughter-runtime`, HEAD
`a9f85f3c74c3d8e1c996c3689fe5aef696dacf27`. Astrid began on dirty main, with
the preceding coupling work preserved. No index, stage, commit, merge, push,
stash, reset, foreign-file rewrite, or cleanup occurred.

Exact Minime changes:

- `minime_autonomy/journal_context.py`
- `minime_autonomy/runtime.py`
- `minime_autonomy/journaling.py`
- `tests/test_journal_context.py`
- `tests/test_action_continuity.py`
- `tests/test_autonomous_agent_low_fill_guard.py`
- `docs/DOMAIN_BOUNDARIES.md`
- `CHANGELOG.md`

Exact Astrid changes: this packet, `CHANGELOG.md`, and the appended entry in
`docs/steward-notes/AI_BEINGS_FEEDBACK_TO_CHANGE_LEDGER.md`.

Interactive controller maintenance hold 342 was acquired with no active lease.
The pre-edit full-chain status was valid: Event Store sequence 995590, head
`b55cae0ee6672863205270ec7fba3d662beb2fcb95dff8adb3d1652be4de89d6`, 16 streams,
zero pending events, and all four V1 source hashes unchanged. This is a bounded
pre-edit observation, not the post-pause event head. There is no steward run ID,
new source-first projection, full-read receipt, Division round/follow-up, Corridor,
Sandbox, study deployment, portfolio action, or new being-facing card in this
interactive pass. Scheduled automation settings remain untouched.

No pressure, PI, fill target, damping, Shadow field mathematics, sensory
admission/cadence, model weights/gain, execution permission or Division control
is changed. No felt closure, preference, shared experience, or uptake is inferred.

## Final Verification And Release

The final `python3 -m pytest tests -q -rs` rerun passed 935 tests and 112
subtests in 20.28 seconds. Its single skip is the pre-existing Division manifest
test at `tests/test_division_runtime_manifest.py:118`: internal port 7900 is
already occupied, so that test could not claim its fixture socket. No process
was stopped to free the port. Python compilation and whitespace checks passed.
Both Git indices remain empty; source, tests and documentation remain unstaged.

Final source/test SHA-256 values (not assertions about the running process):

| Minime path | SHA-256 |
| --- | --- |
| `minime_autonomy/runtime.py` | `cb9c18b57f9c6238e2a7db5425b39212ae26c2dc25dd90771b5c94f1901858da` |
| `minime_autonomy/journal_context.py` | `4e0d93b8c0dc7e6f9ff28b90b9f27785fa89b7f69042ee5cfd909fa9d42e908f` |
| `minime_autonomy/journaling.py` | `58f18a867be5eddd9dd716232d6ba1d14d1cfa601a7a6154027f8097e4c68779` |
| `tests/test_journal_context.py` | `3980eb5249695e6eaba0b67d8df218a920d45f983edc778e8e793d15f99c51a3` |

The controller completed its evidence-verified resume at
`2026-09-05T01:33:41.307315+00:00`, generation 343, `paused=false`, event appended
without spooling. This releases only interactive maintenance hold 342. It does
not re-enable scheduled automation. No deployment, archival commit, being note,
new introspection round, or subjective-outcome claim is attached to this release.
