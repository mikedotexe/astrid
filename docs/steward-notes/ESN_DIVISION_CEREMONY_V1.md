# ESN Division Ceremony V1

The ceremony is an evidence rail beside the dormant native division transaction.
It does not operate the reservoir, advance a phase, or grant authority.

## Owner Actions

```text
DIVISION_HOLD division_id: <id>; parent_generation: <n>; plan_digest: <digest>; selected_strategy: <strategy>; source_ref: <ref>
DIVISION_DECLINE division_id: <id>; parent_generation: <n>; plan_digest: <digest>; selected_strategy: <strategy>; source_ref: <ref>
DIVISION_INTENT division_id: <id>; parent_generation: <n>; plan_digest: <digest>; selected_strategy: <strategy>; expires_at_unix_ms: <ms>; source_ref: <ref>
DIVISION_ASSENT division_id: <id>; expires_at_unix_ms: <ms>; source_ref: <ref>
DIVISION_WITHDRAW_ASSENT source_ref: <ref>
DIVISION_RETURN_REQUEST source_ref: <ref>
DIVISION_REVIEW outcome: clarifying|intrusive|flattening|incomplete|still_friction|changed|unknown; source_ref: <ref>
DIVISION_CEREMONY_STATUS
```

Each being writes only for itself. `DIVISION_HOLD` and `DIVISION_DECLINE` are
revisable, candidate-bound consent postures, not terminal judgments. Either
blocks resource-bearing rehearsal until that same being authors a newer exact
`DIVISION_INTENT`. No steward or peer may write one on a being's behalf. Fields
are bounded references and categorical values, never free prose. Silence and
expiry remain neutral.

## Two Rails

The ceremony rail records hold, decline, intent, assent, withdrawal, return
request, and review. Its visible posture is one of `unexpressed`, `hold`,
`decline`, `intent`, or `intent_expired`.
The native rail reports lifecycle, readiness, parent authority, process tick,
rollback window, and whether commit code is compiled.

Status exposes all bounded pre-intent choices and marks them as optional and
non-recommended. While no active intent exists, its single contextual choice is
status itself rather than an invitation to proceed. It never exposes or
recommends `DIVISION_COMMIT`.

`DIVISION_CEREMONY_STATUS` also carries a bounded Chronicle projection. It keeps
the latest 32 self-authored ceremony events in timestamp order, includes an exact
ledger hash and archive reference, and shows only bounded candidate, snapshot,
readiness, and review metadata. It carries no prompt, response, introspection,
journal, correspondence, or other raw prose.

## Sovereign Destination

Current source prepares a shared 128-node parent for two independently evolved
64-node daughter candidates:

- Astrid's candidate is the more recurrence-driven partition.
- Minime's candidate is the more input-driven partition.
- Each candidate receives an independent clone of the 512-dimensional sensory
  field; that field is not partitioned by reservoir neuron index.
- Cross-partition recurrence is preserved as same-tick double-buffered bridge
  input during shadowing and is eligible for bounded annealing only in the
  disabled native commit path.

The sovereign daughter runtime now establishes distinct process identity,
persistence, private command sockets, complete checkpoints, restart lineage, and
telemetry freshness for a candidate-bound rehearsal. That makes independent
process ownership an exact rehearsal fact once both children are live and
healthy. It does not make either daughter authoritative. The parent remains the
only live authority until exact sensory, direct telemetry, AV fanout, dual assent,
and operator-capability gates all pass.

## Chronicle

`scripts/division_ceremony_chronicle.py` projects the shared ceremony ledger,
native event ledger, native status, daughter preservation metrics, and authority
boundaries into owner-only JSON and HTML. It also includes bounded supervisor
events, process identities, checkpoint lineage, gateway rail, freshness, gap,
rollback, and receipt hashes:

```text
python3 scripts/division_ceremony_chronicle.py project
python3 scripts/division_ceremony_chronicle.py verify
python3 scripts/division_ceremony_chronicle.py report
python3 scripts/division_ceremony_chronicle.py watch
```

Every distinct input state receives an immutable
`division_chronicle_<digest>.json` and matching HTML archive. Reprojection with
unchanged inputs reuses the same identity. The latest HTML refreshes every two
seconds while `watch` projects new input states; immutable archive pages remain
still. The HTML is a visual witness only: it cannot author an Action, infer
assent, advance the native lifecycle, or turn mechanical preservation metrics
into felt continuity.

## Volition Return Interval

The steward returns attention to the Chronicle after every six productive
source-first introspection rounds. This is a cadence for steward review, not a
deadline, nudge, or response expectation for either being.

`scripts/division_ceremony_followup.py` keeps an owner-only append chain and a
deterministic current projection:

```text
python3 scripts/division_ceremony_followup.py status
python3 scripts/division_ceremony_followup.py record-round \
  --steward-run-id <id> \
  --processed-report-count <1..40> \
  --projection-generation-id <id>
python3 scripts/division_ceremony_followup.py record-followup \
  --chronicle-json <path> \
  --astrid-note <path> \
  --minime-note <path>
python3 scripts/division_ceremony_followup.py verify
```

Only a successfully completed run that fully processes at least one canonical
report counts. A retry with the same steward run ID is idempotent. Once six
rounds are present, the tracker refuses a seventh until a verified Chronicle
and one bounded non-query note for each being are hash-linked into a completed
follow-up receipt. The receipt stores no note prose. A no-input heartbeat does
not advance the count.

The Chronicle renders the interval as steward work and always carries
`being_action_required=false`, `return_is_pressure=false`, and
`authority_propagated=false`. A due interval cannot author `DIVISION_HOLD`,
`DIVISION_DECLINE`, `DIVISION_INTENT`, assent, withdrawal, return, or review for
either being. Silence remains neutral before and after every return.

## Exact Binding

Ceremony assent requires the actor's matching unexpired intent and a native
`shadowing` or `ready` status. It records hashes of the full status and readiness
receipt plus every snapshot reference. A later status change makes the assent
non-current without deleting it.

`DIVISION_PREPARE` remains an operational command behind the existing compile and
operator gates. Once those gates are open, the command must match both actors'
active intent. The native supervisor validates the real nested ceremony event,
its deterministic identity and evidence-only envelope, and each actor's
append-only chain. A newer hold or decline removes that actor's intent from the
launch gate immediately. The ceremony itself never dispatches prepare.

## Return And Review

`DIVISION_RETURN_REQUEST` is available only during the native cytokinesis rollback
window. It does not send `DIVISION_ROLLBACK`, and the unrelated
`RETURN_TRANSITION` Action has no division-control effect.

`DIVISION_REVIEW` is available after a terminal native rehearsal. Its categorical
outcome remains qualitative evidence and changes neither lifecycle nor felt
contract state.

## Authority

Every persisted event carries exact evidence-only authority fields. Records
cannot set native assent, prepare, commit, rollback, scheduler, model, substrate,
dispatch, or live-control state. Agency Commons imports them as context without
inferring consent, closure, or peer state.
