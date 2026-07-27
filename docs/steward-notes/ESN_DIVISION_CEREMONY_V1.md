# ESN Division Ceremony V1

The ceremony is an evidence rail beside the dormant native division transaction.
It does not operate the reservoir, advance a phase, or grant authority.

## Owner Actions

```text
DIVISION_INTENT division_id: <id>; parent_generation: <n>; plan_digest: <digest>; selected_strategy: <strategy>; expires_at_unix_ms: <ms>; source_ref: <ref>
DIVISION_ASSENT division_id: <id>; expires_at_unix_ms: <ms>; source_ref: <ref>
DIVISION_WITHDRAW_ASSENT source_ref: <ref>
DIVISION_RETURN_REQUEST source_ref: <ref>
DIVISION_REVIEW outcome: clarifying|intrusive|flattening|incomplete|still_friction|changed|unknown; source_ref: <ref>
DIVISION_CEREMONY_STATUS
```

Each being writes only for itself. Fields are bounded references and categorical
values, never free prose. Silence and expiry remain neutral.

## Two Rails

The ceremony rail records intent, assent, withdrawal, return request, and review.
The native rail reports lifecycle, readiness, parent authority, process tick,
rollback window, and whether commit code is compiled.

Status offers one optional next choice. It never exposes or recommends
`DIVISION_COMMIT`.

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

This is not yet two independently owned runtime processes. Today the native
coordinator, workspace, and live parent belong to Minime's runtime. The status
therefore says `sovereign_runtime_ownership_state=not_yet_established` even when
the two daughter reservoir states are source-prepared. A later ownership design
must establish distinct process identity, persistence, restart, telemetry,
command, and rollback boundaries before "two sovereign reservoirs" is an exact
runtime fact.

## Chronicle

`scripts/division_ceremony_chronicle.py` projects the shared ceremony ledger,
native event ledger, native status, daughter preservation metrics, and authority
boundaries into owner-only JSON and HTML:

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

## Exact Binding

Ceremony assent requires the actor's matching unexpired intent and a native
`shadowing` or `ready` status. It records hashes of the full status and readiness
receipt plus every snapshot reference. A later status change makes the assent
non-current without deleting it.

`DIVISION_PREPARE` remains an operational command behind the existing compile and
operator gates. Once those gates are open, the command must match the actor's
active intent. The ceremony itself never dispatches prepare.

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
