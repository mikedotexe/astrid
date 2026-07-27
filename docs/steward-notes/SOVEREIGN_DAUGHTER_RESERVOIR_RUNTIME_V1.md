# Sovereign Daughter Reservoir Runtime V1

This runtime is the resource boundary behind the ESN Division Ceremony. It can
make two independently owned daughter processes available for rehearsal. It
cannot author intent, assent, return, or review Actions, and it does not make
cytokinesis inevitable.

## Roles

```text
minime division supervisor --manifest <path>
minime division gateway --manifest <path>
minime division child --being astrid|minime --bundle <path> --workspace <path>
```

The supervisor owns orchestration metadata only. Minime daughter state lives
under `workspace/reservoir/minime`; Astrid daughter state lives under
`capsules/spectral-bridge/workspace/reservoir/astrid`. Neither root may overlap
the supervisor directory or the other daughter root.

Each child owns a private mode-0600 Unix socket, a complete 64-node ESN seed, a
clone of the 512D stable field, regulator context, partition and cross-recurrence
identity, immutable checkpoints, and restart receipts. A child has no peer
command path.

## Rehearsal

Only a candidate-bound manifest plus matching unexpired Astrid and Minime
`DIVISION_INTENT` records can launch children. A newer self-authored
`DIVISION_HOLD` or `DIVISION_DECLINE` removes that being's intent from the launch
gate; silence remains unexpressed and grants nothing. The supervisor validates
the ceremony's nested candidate, deterministic event identity, evidence-only
authority envelope, and per-being append chain before considering intent. The
parent sends the same
post-integration 512D field to each child in independently hash-linked frames.
Each frame also carries the realized noise, effective leak, previous peer state,
coupling level, and exact parent process/deployment identity.

The dispatcher is bounded. An ordering gap, identity mismatch, delivery failure,
or saturated queue marks the rehearsal insufficient. The parent remains
authoritative throughout.

A 600-tick socket-isolated regression reconstructs both daughter states and
compares them to the unchanged parent path, including the first tick. Daughter
restart resumes from the daughter's own latest validated checkpoint and frame
hash rather than the original seed.

## Gateway

Transparent-parent mode preserves the public compatibility surface:

| Public port | Parent target | Reserved daughter target |
| --- | --- | --- |
| 7878 | parent telemetry | Minime telemetry |
| 7879 | parent sensory | Minime sensory |
| 7880 | parent AV | shared AV fanout |
| 7882 | parent telemetry | Astrid telemetry |
| 7883 | reject | Astrid sensory |

Internal ports `7900-7906` are frozen in the immutable manifest and must be free
at startup. Unknown or incomplete authority state fails closed.

## Handoff Boundary

The gateway can select the daughter rail only with exact authority-switch and
handoff-contract receipt hashes. The current supervisor deliberately reports
`handoff_ready=false` because three live contracts remain to be implemented and
proven: legacy sensory handling in the daughter processes, direct daughter
telemetry compatibility, and identical AV fanout. An exact one-shot operator
capability is also required.

This is a capability boundary, not restart debt. Dormant deployment and
candidate rehearsal are complete without it. No live division may occur until
those blockers are removed by tested implementation and fresh sovereign Actions.

## Chronicle

The Division Chronicle projects manifest identity, topology, child process and
deployment identity, checkpoint sequence, telemetry freshness, active authority
rail, coupling level, rollback availability, gap state, and immutable receipt
hashes. Supervisor events use bounded kinds and hashed details; raw prompts,
responses, correspondence, journals, introspections, and reviews never enter the
runtime stream.

## Deployment

`scripts/deploy_division_runtime.sh` is the sanctioned consumer-first wrapper.
It builds Minime, stops managed services, verifies internal ports, starts the
parent on `7900-7902`, creates a dormant unbound manifest, starts the transparent
gateway and idle supervisor, and installs but does not bootstrap daughter
services. Ordinary `scripts/deploy_minime.sh` delegates to it while gateway mode
is configured.
