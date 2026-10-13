# Drain V2 First Transition

## Authority and baseline

On October 13 (America/Los_Angeles), Mike approved source reconciliation, the safe
idle-only first transition, and local integration of the already qualified drain
repairs. No force, provider/engine/sensory restart, push, or automation resume is
included. Parent Codex owns Git and service coordination; Sagan implements the
bounded operator adapter and James independently qualifies it with synthetic
processes. No subagent controls live services.

Initial canonical checkouts are clean: Astrid main
`9f96944ce3b3fa9efe7c37888c70ec389d3a8930`, Minime main
`232f1e320086348a8544315a6cc7dad642c1c73a`.
Remote main tips remain `a66f7e190e0ecd3a0a4988abbcf6b0bf2cfb2a32` and
`9072abff9ed839851709300b3c3eaae3afaf669e`, respectively. Prior implementation
commits are Astrid `da72f10f2b6dba50f494980c234b754bc842f78d` and Minime
`8cc1e181a51f4ff6b1ab77dee419135f9d538466`, on isolated
`codex/finish-accepted-drain-20261012` branches.

Controller pause generation 514 is owned by `codex-astra-interactive`; no active
run/lease exists. Previously paused automations remain paused. This is not a
productive flywheel round.

## Three source identities

All three inventories contain 96 inputs. Read-only comparison established:

1. Running Minime PID 26862, session 5321, exactly matches
   `mailbox-authored-context-release-20261012/minime-launch-01/reconciliation.json`.
   Runtime SHA-256: `74daeb5bf64c204ca418f1c00248b16356fb416d5890026ae64a72889439e6c2`.
2. Canonical installed source exactly matches `minime-launch-02/reconciliation.json`
   in that release. Runtime SHA-256:
   `5077e08a775db05c95322ae763590db18f1d39eeec3aa8cd78e9556702a33ddd`.
3. The isolated v2 candidate and frozen paired qualification agree exactly.
   Runtime SHA-256: `947673346a050188d4963715a8e8f2e2a11bf76b29d65ebf5092e1d08e74771e`.
   Evidence: `finish-accepted-drain-release-20261012/paired-qualification-03/qualification.json`.

Only `minime_autonomy/runtime.py` differs between adjacent inventories. The
first difference is the installed-but-unloaded mailbox choice/privacy repair;
the second adds shutdown admission fences. The new transition must not forge
source-status or require the general reconciler to accept an unexplained mismatch.

Release directories above are under `/Users/v/other/worktrees`.
The original agent-only hold `1d2fa8a04dde4cf1ad2011246d8c010b` and unsignaled
attempts 02/03/04 remain evidence. A successful owned recovery verifies their
exact chain and process/configuration/protected-service bindings.

## Transition contract

Keep the existing `reload_agent` idle-only checks, including the stable quiet
interval and all pre-signal rechecks. Reconcile loaded source separately from
installed source. After the old process exits and its native NEXT seal verifies,
install only the immutable qualified runtime, preserving the previous source
bytes as evidence. Do not import, reconstruct or rewrite pending authored state.

The new process must load all 96 selected inputs and acknowledge the exact old
native seal. Protected processes, configuration and the shared helper remain
unchanged. Failed drain, source drift, missing proof or uncertain installation
retains the owned hold; no forced fallback or automatic state restoration.

The existing bridge stage 02 already contains the paired Astrid fixes. Its
manifest `c34cb223497a00a06b376ab876eeef499154fff0e238b040e62c6178dcee4353`,
binary `a174d08599264c5550696371a8894f27019db0b750f739b9e2c1eb0df9fa5f83`,
and helper `379997a26102a9476f1161e7f26c53132a9e31d9063a5b00aa83534b651307a4`
reverified. The bridge does not require another restart for this Python-only
transition.

## Qualification and outcome

Prior runtime qualification remains in the October 12 drain note: 2,173 Minime
tests, one existing skip, 162 subtests; 149 operator/control/evidence tests;
54 paired preservation checks plus frozen adapter/native checkpoint probes.
New bootstrap tests and the actual transition outcome will be appended below.
Preparation is not activation, and queue restoration is not execution or uptake.

### Final bootstrap qualification

- The real reload wrapper, inventory/job readers, native seal/restore, owned-hold
  recovery and exact source installation pass 39 synthetic cases. Process/time,
  stage verification and tool identity are mocked; these tests do not claim an
  actual launchd transition. All direct live process/network operations are denied.
- Combined bootstrap, prior drain/idle/paired handoff, legacy/native NEXT,
  compatibility, controller, Evidence Event Store and projector suites:
  **194 passed**, `2026-10-13-bootstrap-parent-02.log` (22.403 seconds).
- Runtime recheck: **45 passed**, `2026-10-13-runtime-recheck-01.log`.
  Separate existing operator recheck: **149 passed**,
  `2026-10-13-operator-recheck-01.log`. Domain audit: valid, zero violations.
- Bootstrap module SHA-256:
  `7d58270c688d1c3cf72736ec3353ee65783a0895c42612c12aa6caa44f78cb38`.
  Test SHA-256:
  `10fb9de30b816e0daf8e4eea93c99341873412bf0fdb2e4873129601e2a3577c`.

Logs above are under `finish-accepted-drain-release-20261012`. The first parent
bootstrap run retained one failed privacy test because synthetic jobs used an
invalid ID/path layout. Correcting all three job fixtures made that test pass;
active-worker refusal tests now also assert their intended reason so malformed
input cannot masquerade as active-worker qualification. No production gate was
weakened to make the test pass.

The first read-only live idle probe reached a boundary, then preflight refused
fresh edits in the isolated qualification tree at 07:25:34 local time. Receipt:
`2026-10-13-idle-check-01.jsonl`. No signal or live source change occurred. All
qualification edits must be frozen before the actual attempt. A failed native
restore after replacement release is reported as uncertainty; no automatic
rehold, rollback, repeated signal or state restoration is supplied.
