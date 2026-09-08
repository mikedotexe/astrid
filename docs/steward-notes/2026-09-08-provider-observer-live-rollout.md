# Provider observation is live — September 8, 2026

Mike explicitly authorized bringing the qualified provider-attempt observer live.
The observer implementation is signed commit `3f164466efb064e5206970188b55f705e5371ca2`.
The release adds startup wiring in signed commit
`ca87a380b26ef80946b0f99fe3daf8f9bcc4542d`, fast-forwarded onto canonical main.
The compiled observer Rust is unchanged from qualification. No push was performed.

The wrapper now loads private durable settings from
`capsules/spectral-bridge/workspace/runtime/provider_observation.env`, then imports
explicit launchd overrides. The installed decision is `ASTRID_PROVIDER_OBSERVATION=on`
with the dedicated spool
`capsules/spectral-bridge/workspace/provider_observations/20260908-live-01`.
The configuration is 0600 and the spool directories are 0700; evidence files are
0600. No override was present. An absent configuration still defaults off.

Four inert startup cases pass, including explicit disable precedence, alongside
106 deployment-wrapper tests, shell syntax and the domain-boundary audit. The
earlier 2,234 functional library checks and four-arm transport qualification
remain the source evidence; their reproduced baseline timing-test limitation
is preserved. Eight changed Rust files and all 25 retained production-dependency
files match the qualified inputs. The private Minime test fixture was not deployed.

## Verified transition

The isolated staged release is retained at
`/Users/v/other/worktrees/provider-observation-live-20260908/bridge-stage-01`.
Both staging and activation used `scripts/build_bridge.sh`. The old bridge
acknowledged drain and exited on SIGTERM. PID 90102 became PID 14716, started
at 10:59:14 PDT. Activation returned `activated_verified` at 18:00:52 UTC.
The exact stopped checkpoint at exchange 193201 was decoded at startup, signed
self-control state targets the selected binary, and a natural saved exchange
advanced the count to 193202. No forced replacement or automatic rollback occurred.

- Manifest SHA256: `b4013ec632b1c602de2500137d8eb302045452d33c8b85ea2a3e49bf8053f0be`.
- Binary SHA256: `f0759009e8dbc733b60a5eb2edcde743d9f7d6d0aebc6a34a98fb38dddb48008`.
- Activation transaction: `.runtime/bridge-deployment/transactions/cab514e75d1c4b19a7c57700455895da`.
- Original receipt SHA256: `5323db973c551221a25ce28ac901b0edd062ff20c963366ac0be9f545bf53a7e`.

The process, all five staged artifacts and all 591 source inputs were independently
checked on the host. Eleven surrounding service identities and their installed
configuration hashes remained unchanged. The coupled-stack witness passed as
`env_receipt_1788890552407_817000`. The retained post-baseline log segment contains
no observer warning, panic or fatal line; this is a bounded operational observation.

## First natural evidence

Before activation, S-006 declared a 300-second window beginning with the first
dispatch, plus a 120-second allowance for outstanding outcomes. The window is
17:59:37.969–18:04:37.969 UTC. Six MLX attempts have six complete matching outcomes,
all with startup-verified release bindings and observed marker counts of zero.
No raw artifact is needed for those marker-free inputs. The one dialogue decision
joins its ordinary generation record; other provider lanes have no fabricated
dialogue association. No pending attempt or missing dialogue decision remains.
An offline verifier checks all 14 retained capture files and the joins.

No generation, Being message, journal request, or action was induced. This confirms
the observer records ordinary activity. The short window contains no focal marker
opportunity and establishes no repair frequency, operational improvement or felt
benefit. The 256 MiB / 50,000-file spool remains bounded and does not prune itself;
archive a stopped writer's spool and select a fresh private directory when needed.

## Maintenance and rollback

The cooperative controller was paused at generation 409. A projection took time
to release its lease despite cancellation being requested. A guarded operator
stop refused because that lease had already disappeared; this task sent no
signal to the projector. Source exposes a single-SIGINT cancellation path without
a bounded second step and a stale per-step `child_exited` field. Those are recorded
as a separate flywheel follow-up, not changed in this release. The initial controller
state was unpaused and is restored after the documentation handoff; independent
scheduler settings are preserved.

To disable observation, set the durable observer value to `off`, clear any observer
launchd overrides, and use the sanctioned staged transition. Retain the evidence.
The preceding stage remains available, but returning to it also requires the
normal checkpoint and signed-lineage handoff; do not restore state files manually.

Full owning evidence: `/Users/v/other/worktrees/provider-observation-live-20260908/evidence/`.
The S-006 research account and projected receipts are in the research hub at
`analyses/2026-09-08-provider-observer-live.md` and
`research/outputs/2026-09-08-provider-observer-live/`.
