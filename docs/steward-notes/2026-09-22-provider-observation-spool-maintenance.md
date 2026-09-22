# Provider-observation spool maintenance - September 22, 2026

## Cause and authority

Mike asked to address the exhausted optional provider-observation spool reported
after the paired open-framing rollout. This is operator-requested diagnostic
maintenance, not a new Being claim or an inference about experience.

The September 8 epoch contains exactly 50,000 event files, 140,471,843 bytes and
zero raw artifacts. The file-count limit, not the 256 MiB byte limit, prevents
new receipts. The observer deliberately leaves generation behavior unchanged on
recording failure. This spool is separate from the Evidence Event Store.

The original rollout specified stopped-writer archival and a fresh private
directory, not automatic pruning. We retain that policy. No quota increase,
deletion, compression, migration of raw paths, automatic rotation, or new
unbounded writer is introduced. Old `raw/...` references retain their original
epoch-relative meaning.

## Maintenance tool

`scripts/provider_observation_spool.py` provides:

- `status`: bounded metadata-only capacity reporting. At 80% of any limit it
  reports `near_capacity`; at a limit it names the exhausted budgets. This is
  an on-disk snapshot, not a claim about writer reservations or current health.
- `prepare`: exclusively creates one unused sibling epoch, with 0700 directories,
  0600 metadata and the predecessor reference. It does not change configuration,
  activate a process or claim the predecessor is sealed.
- `seal`: takes the same exclusive `flock` used by the Rust writer, refuses an
  active writer, hashes every retained event/raw file, checks for concurrent
  changes and exclusively publishes a private, fsynced inventory outside the
  epoch. It does not rewrite any evidence.
- `verify-seal`: checks the stopped epoch against the retained inventory.

Canonical paths, owner/mode checks, symlink and hardlink refusal, bounded reads,
and no-overwrite publication protect the maintenance path. Temporary evidence
files count against capacity and remain present; they are not certified as
complete receipts. Interrupted inventory publication leaves a private pending
artifact and fails visibly. The inventory is a byte witness, not validation of
receipt semantics, cryptographic authorship, or enforced filesystem immutability.

No provider calls, shell execution, service signals, configuration selection or
controller authority are embedded in the tool. Retention and each new epoch
remain explicit operator decisions. The warning is available on inspection;
this release does not install a monitor or resume an automation.

## Exact transition scope

The selected epoch is now
`capsules/spectral-bridge/workspace/provider_observations/20260922-live-02`.
Its predecessor remains in place at `.../20260908-live-01`.
The durable 0600 `workspace/runtime/provider_observation.env` selects the new
directory; both relevant launchd overrides were absent.

The bridge alone is restarted through `scripts/build_bridge.sh --activate-stage`.
No rebuild is needed: the selected release is the existing immutable stage at
`/Users/v/other/worktrees/astrid-open-framing-20260922/bridge-stage-open-framing-01`.
The engine, Minime agent, models, visual service and sensory clients are outside
this transition. No reservoir, prompt, sampling, retry, acceptance or control
behavior changes. The Rust observer and all existing limits remain unchanged.

One restart attempt was refused by the ordinary edit-settling preflight. Another
was refused because the retained build checkout's HEAD had advanced from
`9266412b8e3d817e87c93441a145cdfe32e76a79` to the docs-only
`96728f8716dbb972febaa9c7f7032944d515a026`. All 678 material source inputs matched.
Neither attempt signalled a service. We pinned that clean retained build worktree
to its original build commit, preserving both commits and canonical main, then
revalidated the exact original identity. No manifest or guard was weakened.
Retained release worktrees should remain pinned to their build commits; add
post-rollout documentation through another worktree/main.

The first seal attempt saw a changing lock identity during the transition and
refused without publishing an inventory. A later attempt acquired the stopped
writer lock and succeeded. The failed attempt is not evidence of lost data.

## Verification

- 152 spool, launch, stage, activation, drain, stopped-recovery, release-launch,
  runtime-feedback-checkpoint and deployment-wrapper tests passed.
- 77 controller, projection, Evidence Event Store and Division tests passed.
- All 11 deploy-preflight tests, two epistemic tests and domain-boundary verification
  passed. Fourteen spool tests include production-limit parity, active cross-process
  writer refusal, exact preservation, partial files, drift, corruption, permissions,
  alias paths and interrupted publication.
- An initial combined test command named the nonexistent `test_deploy_preflight`
  module: 120 real tests passed and that import failed. The actual preflight
  self-test and corrected 152-test selection subsequently passed; no failed
  behavior test is concealed.
- No Rust or Minime source changed, so their full suites were not rerun for this
  maintenance-only patch. The unchanged deployed release retains its prior
  qualification: 2,334 bridge tests (one external-fixture ignore) and 1,550 Minime
  Python tests (one occupied-port skip), plus 136 subtests.

The 50,000-file baseline inventory digest is
`9cea8efd0d8946667c82903718de345137f908fb4923dd6f2fa4f6f389ed36b2`.
The private stopped-epoch seal SHA256 is
`777f2245c3e97738f6c18a5fb5acc5e0657e74cd4441c4c4d5cf942ed6aaa4d3`.
Both are retained under
`/Users/v/other/worktrees/provider-spool-maintenance-20260922/private/`;
the seal is `20260908-live-01-seal.json`. Raw evidence is not committed or quoted.

The oldest/newest retained event-file mtimes are September 8 17:59:37.969778 UTC
and September 21 16:08:53.875648 UTC. The latter is a filesystem observation, not
a reconstructed last successful request. Missing receipts after exhaustion remain
missing; do not backfill provider outcomes from journal text or infer that
generation stopped. Post-restart verification is recorded below; the first
version of this note explicitly left that verification pending.

## Live verification completed

The sanctioned activation transaction is
`.runtime/bridge-deployment/transactions/ec8ef339c73148d68eee9eb38b8dde01`.
It reports `activated_verified`, acknowledged `drained`, SIGTERM, confirmed old
process exit, no force, and no automatic rollback. PID 67810 became PID 75800,
started September 22 at 14:21:52 PDT. The new process decoded the exact stopped
checkpoint SHA256
`075d48daa8e1717bacda2a0ffb6decb1160baac38d6b908cdb7b14f7c29a5327`, passed
signed self-control lineage verification and advanced exchange 205429 to 205430.

The unchanged manifest SHA256 is
`a84f260aa8d306f60a6a3aeb1ddce76d039068f4e0aa6f06c7f8220e67063bb3`;
the unchanged executable SHA256 is
`8c15e285385c5d83444ccd74614dc58289adeca97e65e13239c88fdff85af32f`.
The new durable observer configuration SHA256 is
`6ab88aeb91f16382c720b0371964f0dfd6d3c68a4548776664cfa641211998a7`.

The independent verifier retained two natural dispatches and one matched
provider outcome at its snapshot. The complete self-study outcome reports
`provider_returned`, joins its request hash, names PID 75800 and has exact
startup-verified release bindings before and after generation. Its earlier
recording status is `recorded`. No dialogue-decision receipt had yet occurred in
that bounded snapshot: self-study is not a dialogue-decision lane. No conclusion
about unobserved lanes is inferred and no extra generation was induced.

Both the stopped-epoch seal and a fresh full inventory verify against the
pre-transition baseline. All 50,000 old files retain their exact bytes. A private
`transition.json` in the new epoch links its predecessor, seal and verification
receipt without moving either epoch or changing relative raw links.

Minime remains PID 66540 with the same start time, ready source status and all
84 startup input hashes. The nine other protected services retain exact PIDs
and start times. Shared helpers remain selected from the same immutable stage.
Ten bounded telemetry samples show advancing engine time and bridge arrival
time, fresh files and fill 70.95-72.98%; this is health evidence, not a claimed
effect of diagnostic maintenance. All three observer failure warnings after
the baseline occurred before the replacement started; none was seen from the
replacement in the verification interval.

The independent private verification record is `private/verification.json`
under the maintenance work directory, SHA256
`dc742314a133684af7c59f87f1370200b21ed5d26d47358f4987dc78293a2f65`.
`baseline.json`, all three activation-attempt logs, test logs and the seal remain
alongside it. The driver source is `maintenance_evidence.py` in that directory's
parent. Configured directories and witnesses are local, not uploaded.

Controller pause generation 462 remains paused with no active lease. Its V2
indexed-tail verification was valid at sequence 1123116, head
`9db69ff48be9902237d1c57d2f895fa11587e067fee6c518abd6900f8edee19b`,
with all four V1 sources immutable. No productive introspection round, new claim
disposition, projection, Being message or automation resumption occurred. The
canonical 197 historical Astrid dirty paths and clean Minime tree were verified
unchanged before and after activation. Missing historical provider receipts
remain the explicit evidence limit; current recording is restored.

## Next maintenance window

Inspect the configured epoch without disclosing its contents:

```sh
python3 scripts/provider_observation_spool.py status --root "$CURRENT_EPOCH"
```

At `near_capacity`, review actual disk space and retained-epoch size, coordinate
the paused controller and other writers, and explicitly choose another epoch.
Never automate an indefinitely growing chain of directories as a substitute for
a retention decision. `full` may refer to the raw budget alone; inspect `at_limit`
before describing all event recording as unavailable. This event exhausted `files`.

```sh
python3 scripts/provider_observation_spool.py prepare --root "$NEXT_EPOCH" --previous "$CURRENT_EPOCH"
```

Review and update the private durable observer configuration, check launchd
overrides and the exact retained release identity, then use the sanctioned
bridge activation wrapper with the observed PID and explicit acknowledgement.
The process samples settings once at startup. Updating the config alone does
not switch a running writer. If activation is denied, retain the prepared epoch
and report the config/running-process difference; do not force the transition.

After verified old-process exit, create a seal in an owned 0700 directory:

```sh
python3 scripts/provider_observation_spool.py seal --root "$CURRENT_EPOCH" --manifest "$PRIVATE_SEAL"
python3 scripts/provider_observation_spool.py verify-seal --root "$CURRENT_EPOCH" --manifest "$PRIVATE_SEAL"
```

Verify natural dispatch/outcome/decision receipts with the replacement PID and
manifest binding, checkpoint continuity, fresh telemetry and unchanged protected
services. Do not solicit a journal entry, regenerate missing evidence, or resume
previously paused automation. Keep both epoch references with the transition
receipt; details remain private and on demand.
