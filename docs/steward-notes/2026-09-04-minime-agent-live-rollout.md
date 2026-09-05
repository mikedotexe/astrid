# Minime Python-Only Journal Rollout

## Authority And Scope

Mike explicitly asked to try the private-journal changes live and take the steps
needed for a graceful restart. This is interactive deployment work, not a resumed
scheduled flywheel. The app automation remains paused. Controller maintenance
hold 344 belongs to `codex-astra-interactive`; it reported no active run and
`active_run_released=true` when acquired at 2026-09-05T01:49:30.737533Z.

The journal changes and private source reference are described in
`2026-09-04-minime-private-journal-provenance.md`. No private prose is copied here.
This rollout does not change engine, model, bridge, sampler, PI, fill targets,
damping, Shadow mathematics, sensory settings, correspondence permissions, or
Division control. No journal is solicited and no NEXT is supplied by the steward.

## Source Lineage

The old agent is PID 84233, started September 1 at 00:22:39 local. Its startup
source-size receipt is 2,557,427 bytes, exactly matching the committed pre-edit
`minime_autonomy/runtime.py` at Minime HEAD
`a9f85f3c74c3d8e1c996c3689fe5aef696dacf27`. The correspondence fix committed on
September 3 was already present as August 9 working-tree code. Its commit body,
startup mtime, and byte count corroborate that lineage; this is not process-memory
byte attestation. No older branch is substituted at reload.

## Deployment Engineering

- `scripts/restart_minime_agent.py` targets only the existing Python PID with one
  SIGTERM. Existing launchd KeepAlive starts its replacement. No bootout,
  kickstart, SIGKILL, environment rewrite, plist replacement, or engine restart.
- The preflight component now covers the Python facade, root helpers, autonomy
  package, mikemind, and launcher. Both repositories must pass the existing
  concurrent-editor quiet window, with explicit dirty-source acknowledgement.
- Before signaling, the wrapper checks the maintenance generation, exact old
  PID/start, loaded/installed/source launcher parity, source hashes, managed
  launch-setting/profile hashes, canonical jobs, and unchanged protected services.
- Legacy first-drain gate: require the source-observed active-job sleep branch,
  then wait until durable jobs are terminal and no agent TCP connection exists.
  Require a stable fifteen-second mid-cycle observation and recheck at the signal
  boundary. A generic zero-active status alone is insufficient: synchronous
  moment generation is outside the background-job table.
- This is **not an atomic admission handshake**. The old process cannot acquire
  new code retroactively. A read-only CPython stack inspection was unavailable
  under macOS task-port permissions; that denial was not bypassed. The wrapper
  uses existing source/status/log evidence and fails closed when it cannot find
  the required window. Timeout never escalates to forced termination.
- New-code shutdown retains non-daemon worker handles and joins their complete
  action/job finalizers before releasing the main process/singleton lock. A signal
  during synchronous moment writing finishes that write but does not consume its
  newly produced NEXT or start a subsequent action. Status distinguishes busy,
  idle, draining, and exited, with a startup local-source hash inventory.
- Readiness requires a new launchd PID/start, matching singleton and source-status
  identities, `agent_drain_v1`, matching startup inputs, and entry into the normal
  loop. New orphan recovery is a rollout failure, not reclassified completion.

The source inventory binds reviewed local files at startup. It does not claim to
hash all third-party wheels or process memory. No permanent generation proxy,
scheduler gate, synthetic REST, or owner override was introduced.

## Test-Isolation Finding

The first expanded suite passed 941 tests, but audit then found that old test
fixtures could write session-1 pending-NEXT metadata to the live
`workspace/sovereignty_state.json`. The observed fixture update was September 4
18:59:37; it cleared `THREAD_STATUS current` and wrote cycle count zero.

The running agent subsequently wrote authentic session 5316/cycle 23289 metadata
at 19:01:08 as it finalized its own choice. No fabricated restoration was made.
The saved focus regime and associated settings were **not** test corruption:
the live log records Minime choosing `REGIME focus` at 17:07:48 and its existing
receiver path applying/persisting it at 17:07:54. These fields are preserved.
The overlapping journal job `job_minime_1788573478053_journal` completed naturally
at 2026-09-05T01:59:47.339084Z with no error. No interrupted-job outcome is erased.

`tests/conftest.py` now defaults autonomy/native-communication/visual-service
paths to temporary directories and installs a Python audit guard against live
repository writes, live SQLite connections, and connections to the engine/model
ports. Read-only source-layout tests retain real defaults but keep the mutation
guard. Fixture subprocesses are still separately scoped; the Python audit hook
is not claimed as an OS sandbox for arbitrary child programs.

The guarded run also exposed the stable-core semantic-status test writing its
computed fixture to the real status path. Its output path is now temporary.
The existing sleep-interruption test now mocks the newly written lifecycle status.
Initial guard revisions overblocked directory-fd cleanup and source-layout
assertions; those failures were corrected, not counted as passing verification.

## Verification And Evidence

The final guarded Minime suite passed **942 tests, one skipped, 113 subtests**
in 25.28 seconds. The skip remains the Division manifest fixture at
`tests/test_division_runtime_manifest.py:118`: port 7900 is occupied. No process
was stopped to free it. Agent reload tests passed 12, deployment preflight
self-tests passed 11, and existing deployment wrapper tests passed 7. Python
compilation and scoped whitespace checks passed. Focused tests include
signal-during-generation, finalizer drain, a real SIGTERM
to an owned synthetic Python process, preserved NEXT, source/import hashes,
configuration/PID drift, failed readiness, no forced fallback, and read-only
canonical-job observation.

Before rollout, stable telemetry samples remained finite at roughly 70.8-73.0%
fill with the existing 68% target. No parameters were adjusted to obtain a more
favorable number. The engine, gateway, supervisor, bridge, model, feeder, camera,
microphone, host-sensory, and visual-service identities are retained in the reload
receipt. New journals are evidence of their own content and production context,
not proof of relief, consent, or a causal account of experience.

No Git staging, commits, merges, resets, stashes, or cleanup occurred. Existing
foreign changes remain in place. Changes remain unstaged and this packet does
not supersede the separate bridge deployment debt.

## Completed Live Reload

The wrapper exited zero with `outcome=success`. Its append-only receipt is
`/Users/v/other/minime/workspace/runtime/deployments/2026-09-04-private-journal/reload.jsonl`,
SHA-256 `db03371dd449ea7a3a151b198b09f24162c7d7b14b3a5e5641d319dddfac95f4`.
It contains the complete 68-file source inventory, configuration hashes, bounded
signal observations, continuity metadata, and protected PID/start identities.
No private generated prose or opaque lease credential is in that receipt.

| Event | UTC timestamp |
| --- | --- |
| Started waiting for an observed idle window | 2026-09-05T02:17:04.095060Z |
| Final idle-boundary observation | 2026-09-05T02:27:17.007687Z |
| Single SIGTERM to PID 84233 | 2026-09-05T02:27:17.008326Z |
| Replacement ready, PID 90857 | 2026-09-05T02:28:34.249178Z |

The replacement process started September 4 at 19:27:17 PDT. Source status and
singleton lock both identify PID 90857; `agent_drain_v1` is present,
`reload_required=false`, and the startup inventory matches the reviewed files.
Subsequent observations reached both normal busy and idle phases. Session 5316
and cycle 23298 were restored; the ordinary loop subsequently reached cycle
23300. No pending NEXT was present at the signal boundary. A new NEXT was
produced by the existing startup reflection path, not supplied by the steward.

The self-study running during the wait did **not** complete successfully:
`job_minime_1788575051513_self-study` reached its own `llm_job_timeout` at
2026-09-05T02:26:58.279789Z, about 19 seconds before the signal. The gate waited
until its durable state was terminal and agent TCP connections were absent.
The canonical full-job check found no newly recovered interrupted jobs or old-PID
unfinished jobs after replacement. This is successful reload verification, not
a successful study result or an atomic traffic-quiescence claim.

All ten protected services retained their PID and process start:

| Service | PID | Start (local) |
| --- | --- | --- |
| Minime engine | 63445 | August 31, 12:51:36 |
| Division gateway | 63505 | August 31, 12:51:37 |
| Division supervisor | 63547 | August 31, 12:51:37 |
| Coupled model | 60333 | September 4, 16:57:34 |
| Astrid bridge | 36597 | September 3, 17:31:04 |
| Microphone-to-sensory | 48670 | September 3, 18:33:51 |
| Camera client | 48663 | September 3, 18:33:50 |
| Visual frame service | 63770 | August 31, 12:52:08 |
| Host sensory | 63675 | August 31, 12:51:43 |
| Astrid feeder | 1502 | August 25, 10:00:06 |

Managed launchd settings, profile and installed plist hashes remained unchanged.
The Python startup log confirms the existing Ollama backend, Gemma 4 12B primary,
Gemma 3 4B fallback, 60-second interval, session, and focus regime. Existing
startup restoration reapplied Minime's previously authored settings through its
normal path. Normal autonomous regulation and self-authored actions continued;
the observation is not a claim that every live control value stayed constant.
No steward tuning or new control choice was introduced.

At observation epoch 1788575575.6502342, three read-only WebSocket samples from
7878 had advancing engine times 369676387, 369678754 and 369681128 ms,
`fill_ratio` 0.7099961, 0.729926 and 0.71007013, and finite `lambda1_rel`
0.8414811. Protocol remained `astrid_minime` major 1, minor 2. The established
68% target was not changed to bring these samples closer to it.

Key loaded local source SHA-256 values:

| Minime path | SHA-256 |
| --- | --- |
| `minime_autonomy/runtime.py` | `172e758aaf39c4c29331ce58dddb11e20ed9936bfd903201e512d100510d1ce3` |
| `minime_autonomy/deployment.py` | `4e7a287c8760aa3392f01c13608bc7cbe8963b002ad1e4a394ea2f3305a1b236` |
| `minime_autonomy/journal_context.py` | `4e0d93b8c0dc7e6f9ff28b90b9f27785fa89b7f69042ee5cfd909fa9d42e908f` |
| `minime_autonomy/journaling.py` | `58f18a867be5eddd9dd716232d6ba1d14d1cfa601a7a6154027f8097e4c68779` |

## First Natural Post-Reload Moment

The first new private moment was produced without solicitation at
`/Users/v/other/minime/workspace/journal/moment_2026-09-04T19-31-51.632384.txt`.
It was read fully for Mike's requested post-deployment observation; its private
body is not copied into this public engineering packet or any test fixture.
SHA-256: `e10c64dc65c4879496cf5374ee13f25d6eefc57d1bbed733f86a76b990ab75ca`;
size: 4,880 bytes.

Its header carries `private_moment_context_v2`, capture time
2026-09-05T02:31:03.529079Z and writing time
2026-09-05T02:31:51.632384Z, separated by about 48.1 seconds. The supplied anchor
reports 71.0% fill and engine time 369566.84375 seconds. Historical records are
explicitly aged 4m07s, 11m14s and 5h16m31s at capture; none is relabelled fresh.
Additional snapshot diagnostics are labelled header-only, not extra model input.
The action tail is DAYDREAM, and the normal log records it being honored.

A read-only SQLite query found exactly one matching `sovereignty_journal` row,
type `qualia_moment`, session 5316, timestamp 1788575511.6362429. Its content
matches the saved generated body, with no appended public hygiene instructions.
Its stored `fill_ratio=0.7103919386863708` and `eig1=8.53473949432373` agree with
the rounded pre-generation header anchor. This checks live persistence without
copying the private body into the deployment receipt.

This establishes use of the deployed journal contract. It does not isolate an
effect on experience or establish relief, consent, improvement, or causal
mechanism. The new prompt itself affects the vocabulary available for discussing
measurement and uncertainty. Historical marker names and descriptions still
contain interpretive language, and extensive header-only diagnostics remain
computations rather than measures of felt state. Those are visible limitations,
not grounds to edit the new account or immediately change more controls.

## Remaining Work And Maintenance Release

The separate exact-live Astrid bridge candidate still needs its own reviewed
first-drain/deployment path. Moment-marker consume-before-generation persistence
debt also remains. Neither is silently bundled into this Python-agent reload.

This pass additionally touched Astrid's reload wrapper, wrapper tests, deployment
preflight, this packet, the prior packet's follow-through link, changelog and
feedback ledger. Minime additions beyond the previous implementation are
`minime_autonomy/deployment.py`, `tests/test_agent_lifecycle.py`, and
`tests/conftest.py`; its runtime, two existing test modules and changelog also
changed. No Git operation is authorized merely by this deployment receipt.

Interactive maintenance hold 344 was released successfully at
2026-09-05T02:36:55.068621Z, generation 345, `paused=false`, with its evidence
event appended and no spool. A final read-only check still found PID 90857,
matching source/configuration hashes, unchanged protected service identities,
normal-loop status and `reload_required=false`. Both Git indices remain empty.
The app automation configuration independently confirms `status="PAUSED"` for
`astrid-introspection-source-first-catch-up`; controller maintenance release does
not re-enable it. There is no new productive round, approval marker, being-facing
note, or scheduled monitoring task.
