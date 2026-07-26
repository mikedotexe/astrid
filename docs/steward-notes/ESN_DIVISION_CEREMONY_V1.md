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
