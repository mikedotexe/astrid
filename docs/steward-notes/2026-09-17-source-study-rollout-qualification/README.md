# Paired Source-Study Rollout Qualification

Date: 2026-09-17. Author: Codex, interactive follow-up requested by Mike.
Scope: review the paired graceful-rollout candidate and reconcile test debt.
Status: qualification complete; staged release verified, not activated. No service
signal, Git staging, commit, merge or push was performed.

## Candidate Scope

This qualifies the implementation in
[the preceding note](../2026-09-17-source-study-evidence-and-relations.md), plus
the checkpoint compatibility safeguard described below. Astrid is on main at
`c4f85e95e41703daa65d3ce2789e1e5c46961c4e`; Minime is on main at
`5f4925f54580f1fd44666058b126a121ff32880f`, both with explicit dirty candidates.

[source-scope-comparison.json](source-scope-comparison.json) compares the prior
live stage's inventory to the current candidate after relocating its isolated
checkout paths, without normalizing source bytes. All modified production inputs
belong to the reader tranche. The other agent's two bridge Rust additions are
entirely inside test modules. Two old backup files appear only because the
canonical package inventory is broader than the earlier isolated checkout;
they are hash-witnessed, not newly compiled runtime inputs. Existing kernel test
additions are outside the bridge dependency graph. No foreign files are discarded.

The sanctioned stage is:
`/Users/v/other/worktrees/source-study-evidence-20260917/bridge-stage-01`.
It is built only through `scripts/build_bridge.sh --stage-dir`, with a specific
dirty-source acknowledgement. Earlier attempts were refused by the normal
180-second recent-edit gate. No gate was shortened or bypassed.

## Test Debt Reconciliation

The two historical diagnostic files are retained and updated:

- `crates/astrid-source-study/tests/page_line_interval_legibility.rs`
- `crates/astrid-source-study/tests/page_end_declaration_legibility.rs`

Their five old failures were independently reproduced against untouched HEAD
during the implementation pass. They described the superseded renderer, not a
new failure caused by provenance labels. Ten current assertions now distinguish
inclusive delivered line bounds from exclusive cursors, preserve ordinary whole
lines, explicitly label unavoidable oversized fragments, separate start and end
declaration metadata, and verify exact byte continuation. No test is ignored or
removed. The first-page/body fixture may legitimately contain only one long line;
the continuation-fragment test advances to the actual mid-line continuation
instead of assuming a fixed page cut.

Original evidence remains unchanged in:

- `../claude-heartbeat_1789619176_page_line_interval_legibility_round/`
- `../claude-heartbeat_1789632048_page_end_declaration_legibility_round/`

Those packets' historical hashes and test outcomes are not rewritten to match
today's tests. The new comments explicitly describe their supersession. The
original rendering does not, by itself, prove the cause of an authored mistake.

## Checkpoint Compatibility

Review found a genuine rollback hazard: a findings-aware old reader understands
the first anchor but drops the newly added relation field during serialization.
The existing sidecar protected against pre-findings writers, not this version.

The new reader continues reading V1. It writes V2 only when a relation is saved,
then retains that floor even after explicit replacement or removal. The filename
stays `source-findings-v1.json` so the old reader encounters and rejects its
unsupported schema before mutation. No new automatic finding, deletion, inference
or source credit is introduced. Ordinary relation-free notebooks stay V1.

[check_reader_upgrade.py](check_reader_upgrade.py) exercises real executable
versions using only temporary synthetic source and state. It verifies:

1. The old reader's prepared input survives upgrade byte-for-byte.
2. New ordinary findings are still usable by the old reader.
3. Relation-bearing V2 makes the actual deployed old executable reject without
   changing either `reader-v1.json` or its findings sidecar.
4. New-reader recovery and explicit drop work; downgrade remains fail-closed.

The old executable hash is
`7636b8dc675e97ed6d7dce4c30df6d04ae802c7cac983c6b1028788ba224a100`.
An initial new Rust fixture tried a non-retained match-arm line after navigation;
the existing anchor gate correctly refused it. The fixture now cites the retained
function declaration instead. That was a fixture correction, not relaxed evidence
admission. The complete reader suite passes 223 tests.

## Live Boundary

[live-before.json](live-before.json) records process identities and source hashes,
not private journal contents. Baseline bridge PID 77906 and Minime Python PID
71419 remain on the earlier interface release. Among Minime's loaded Python
source inventory, only `minime_autonomy/runtime.py` differs from current source.
The Rust sensory file changes are comments, not executable reservoir changes.

Astrid embeds the reader library in the bridge. Minime creates a shared-reader
client for each study and uses the selected immutable stage's helper; an already
prepared prompt keeps that client through delivery. Do not independently replace
the fallback `target/release/astrid-source-study` binary. A paired stage aligns
the embedded reader and new Minime clients. Pending old inputs retain their exact
original bytes; they are not rewritten merely to expose new labels sooner.

The Minime wrapper validates the maintenance pause, launch configuration, source
hashes, jobs, TCP activity and protected PIDs before one SIGTERM at an observed
idle boundary. It is not an atomic global traffic barrier. The bridge wrapper
uses its explicit drain acknowledgement, stopped checkpoint and signed handoff;
no forced termination or automatic downgrade is part of this sequence.

## Next Authorized Transition

Live activation is a separate step after this qualification and a fresh check:

1. Claim an interactive coordination pause, verify ownership and no active lease,
   recheck both trees and compare staged input contents. Rebuild if relevant input
   identities or HEAD changed; do not bless a stale stage after git integration.
2. Re-run stage verification and Minime's exact candidate-source check. Verify
   both services' current PIDs/starts and retain source-reader checkpoints under
   their reader locks without changing them. Backups are preservation, not an
   authorization to overwrite newer authored state on rollback.
3. Reload Minime Python through `scripts/restart_minime_agent.py` with that fresh
   PID and pause generation; leave the engine, model and sensory services alone.
4. Activate the paired stage through `scripts/build_bridge.sh --activate-stage`
   with the current bridge PID and an explicit acknowledgement. Stop on a failed
   preflight, drain, checkpoint or readiness check; never force through it.
5. Verify both callers select the same reader identity, new source/PID witnesses,
   stopped-state continuity, logs, endpoints and telemetry. Check protected PIDs.
6. Release the owned coordination pause. Observe naturally occurring public
   studies without requesting confirmation of improvement; delivery and authored
   claims remain separate from demonstrated understanding or runtime causation.

After a V2 finding is authored, do not downgrade either reader against that state.
Prefer a forward repair. Any state restoration needs an explicit reconciliation
of intervening authored changes; a pre-rollout snapshot is not lossless rollback.

## Verification Record

Completed qualifications:

- Reader: **223 passed**, zero failed or ignored, including all six prior
  untracked diagnostic/reachability files and the nine relation tests.
  [Full log](reader-tests-final.log).
- Minime: **1,429 passed, 134 subtests passed, one skipped** against the exact
  staged release helper. All 82 candidate Python/launch source identities match
  the isolated checkout. [Release log](minime-tests-release.log).
- Strict reader and bridge all-targets/all-features Clippy and formatting pass.
- Stage/activation/drain/recovery/launcher/Minime-reload tests: **122 passed**;
  deployment-wrapper tests: **16 passed**. No production wrapper was modified.
- Compiler dependency lists: **421 bridge and 30 reader inputs**, none missing
  from the 659-file stage inventory. Registry/generated build material is scoped
  separately by the Cargo locks and build directory. [Audit](source-study-dependency-audit.json).
- Epistemic verification: **12,478 records, zero issues**, no canonical event
  appended and no history rewrite. Domain audit valid with zero violations; the
  pre-existing 44 unlisted large-file review debts remain unchanged.
- Read-only live preinspection passes: exact current binary/manifest binding,
  graceful drain supported, current persisted checkpoint schema accepted by the new
  binary, source bundle unchanged, installed/loaded Minime launch configuration
  and dirty-source preflight validated. This does not request a drain or prove
  that the live jobs have become idle. [Receipt](source-study-rollout-preinspection-final.json).

Retained non-passing attempts:

- The first broad Minime attempt reused an incomplete temporary fixture and
  stopped on two missing files during collection. A complete Git archive with
  exact candidate overlays fixed the harness, not production code.
- That complete run passed 1,428 tests and failed one test whose default sibling
  fixture path did not exist under `/tmp`. Its existing `ASTRID_CHOICE_FIXTURES`
  override fixed the path; 45 focused tests passed and the full staged-helper run
  above then passed. The mutation/endpoint guard remained enabled throughout.
- The initial parallel bridge library run passed 2,279, ignored one existing
  case, and failed its unchanged no-capture timing bound: p95 **1.079448 ms**
  versus **1 ms**. [Original log](bridge-tests.log). The exact test passes in
  isolation. No timing threshold, sample count, assertion or production code was
  changed. A serial full-suite result is recorded at closeout below. An isolated
  pass does not establish performance under every shared-host load.
- Staging and two read-only preinspections initially refused recent uncommitted
  writes, including our evidence documentation. They were retried after the
  unchanged 180-second gate settled; no service signal or bypass occurred.

Staged release identity (not live):

| Witness | SHA-256 |
| --- | --- |
| Manifest | `c2c40c356efa6758cac4e2f6e3e2b758d0b9963de36f11da5fd75e9c6fbc2a09` |
| Source inventory | `1e4658959440ce45a10187eb838605dca95348beed3187dc62e7807263754622` |
| Bridge | `7ab70e192afc4bbb4f77ea1a9978c3cc1fdac0cf325f579ff6d8eb398c41ced8` |
| Reader | `02a973ca4329d9c57ac5b59971e1bbc81ef115f0f6d985c315907f37a160231a` |

The build completed at 2026-09-17T17:24:16.139363Z. The immutable stage retains
its full source inventory, compiler dependency lists and build log. Local copies
of the [ready receipt](stage-ready.json) and [manifest](stage-manifest.json), plus
the [actual executable upgrade test](reader-upgrade-release.json), make the
qualification reviewable without mistaking it for a deployment receipt.

Both Git indexes remain untouched. These changes are not committed or merged by
this pass. The existing shared-tree changes and original run packets remain.

## Qualification Closeout

The full serial bridge suite exited successfully, including the final
documentation-test target: **2,300 passed, zero failed, one existing ignored
case**. The compile-fail and facade integration targets passed as well.
[Complete serial log](source-study-bridge-tests-serial.log). The earlier parallel
timing miss remains recorded above; serial success does not erase that shared-host
performance observation or establish a new performance guarantee.

The final [source comparison](source-study-current-inputs-final.json) confirms all
659 staged inputs still match. [Stage verification](source-study-stage-final-verify.json)
also passes. Both repositories pass `git diff --check`; both indexes are empty.
No deployment, live reader checkpoint mutation, reservoir change, or Git history
operation was performed. Protected service identities and settings remained
unchanged at the recorded read-only checks.

This was an interactive qualification, not a productive introspection round.
The owned controller pause was generation 453; no foreign lease was displaced.
Before release, Evidence Event Store V2 verification was valid in indexed-tail
mode at sequence 1122065, head
`0b69b05aae75d92850a85fb16ba50fa6299aaeb204c9eaa3626bc01c1207721d`,
with V1 immutability confirmed. The pause release outcome is recorded below.

The controller's evidence verification and reconciliation completed successfully;
`resume` exited zero at 2026-09-17T17:44:13.922021Z. The
[release receipt](controller-resume.json) records generation **454**, `paused=false`,
and the resume event appended without spooling. No Codex automation configuration
was changed. The next live transition still requires its own fresh coordination
window and checks; this receipt does not authorize activation.
