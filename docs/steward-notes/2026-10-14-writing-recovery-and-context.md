# Exact Writing Recovery and Bounded Context

## Scope and Ownership

Mike requested safe recovery of Astrid's blocked job first, then correction of
parking status, historical excerpts and repeated context. Larger writing capacity
remains separate. This is an interactive repair, not an automation round.

Canonical Astrid starts at `4207858de948ded2705bd74feaeb0128c62e964a`, Minime at
`68ae81722079bac0c4470a117a8d6271ed5e6c08`; both trees were clean and matched their
remote main tips at entry. Candidate changes are confined to branch
`codex/writing-recovery-context-20261014` in
`/Users/v/.codex/worktrees/writing-recovery-context-20261014/astrid`.
No Minime source changes, staging, commits, merge or push occurred in the initial repair turn.
Controller pause generation 521 has no active lease; automations remain paused.
An unchanged Minime test worktree at the same baseline is paired under
`/Users/v/.codex/worktrees/writing-recovery-context-20261014/minime`; its branch
also uses `codex/writing-recovery-context-20261014`. It contains no candidate
Minime edits and exists to qualify adapter compatibility without live paths.

The exact-job recovery below is live and used an existing qualified operation.
New code remains a candidate. No new-code activation, provider/engine/sensory
restart, budget increase, inferred outcome or private-prose publication occurs.

## Survey and Source Witnesses

The preceding survey fully read 21 public entries. Its bounded metadata window
ran from October 13 at 18:05:45 through October 14 at 11:07 PDT. Astrid had 376
verified writing deliveries (22 FROM_JOURNAL, 10 START, 344 CONTINUE), covering
32 drafts d874-d905 with roughly 10-14 passages and 41-49 KB each. Minime had
19 verified deliveries (four FROM_JOURNAL, 15 CONTINUE), covering five drafts.
No explicit PARK, RESUME, BRANCH or FINISH was observed in that window. These
are operation counts, not private-content judgments or a general claim about
the authors' preferences. Failed/uncertain attempts were not counted as delivered.

Public witnesses (Astrid paths are relative to
`capsules/spectral-bridge/workspace/journal/`; Minime paths are relative to its
`workspace/journal/`):

| Owner | File | SHA-256 |
|---|---|---|
| Astrid | `aspiration_1791977867.txt` | `fb1790e2dff79a407247d3215cdcfd3de320f7d04513fe9bf4ba0562d1623dc6` |
| Astrid | `aspiration_longform_1791975197.txt` | `81b1f938d6954553c5c047e17ee94f14487d2a3f90eaa7c0b0da76530cb6634e` |
| Astrid | `astrid_1791977787.txt` | `9720f238718a3075c77a3c009f1c47a526d21d613feaba9b929ed250094e2925` |
| Astrid | `astrid_1791980463.txt` | `87306350988b1efeb787d46d5cf66b99afaec6fe54b959a113ad45acc49ce815` |
| Astrid | `astrid_1791980580.txt` | `0097628a89e31e40755dbf9d78868239f916662e6405b6745f08644acf98c8d4` |
| Minime | `journal_2026-10-13T17-38-25.898106.txt` | `bf5d106a8c1cb2a08051f1c2899149215d04505d9d49f4703728becad51b86b5` |
| Minime | `journal_2026-10-14T10-02-51.396492.txt` | `1a305f20cd88441c7af44edc143a9608b2a6afeea583197063bc0e3c7f65fedf` |

Astrid's aspiration interprets the supplied excerpt's unfinished final word as
significant. The source continues normally beyond the 500-character cap. Her
interpretation remains intact; the supplied boundary should not masquerade as
an authored stopping point. `ground_review.py` reports zero code citations and
six felt observations, not a source-verifiable claim about reservoir mechanics.

## Exact Live Recovery

At 2026-10-14T13:05:53Z, accepted-artifact retention failed with StorageFull.
Clearing disk space did not itself retire the already-claimed job. Fresh checks
found available disk space, an idle ready provider, no matching accepted-delivery
partition, no writer receipt and no retained completion for that dispatch.
The dispatch's cancelled-or-abandoned sentinel was not treated as cancellation
proof. No result could safely be reconstructed or blindly regenerated.

Exact bindings:

- Job: `study-job-97db78e03ea3f35705b9895b2e421ad87dd42c393beb2caec3410610ab640713`.
- Activity selection revision before review: 38793.
- Intent hash: `eaaebe2e5bc85961875241f02558f3728e7e737d3be62f4ed614be8d87166638`.
- Prepared-input hash: `913bc0795d9fb7fa6b674eef8aea75ea4e98973988383d3b93f70c4392cce8c7`.
- Input: `writing-7924`; draft d905 revision 12. No private text is copied here.
- Accepted partition: `ba0c8d8d923ddb5533a66af510fd40d14b2e08381230b2500372fe607e3ea098`, absent.
- Provider dispatch hash: `bf0f1ecde4576cc303435e17fcd4acdad18b1a2a758c290f9397e55869063e30`.
- Provider request hash: `82864833b367841be22a5878bc236b44f650fa87223252cfc85b006f2474dcd1`.

Evidence packet directory:
`/Users/v/other/worktrees/writing-recovery-context-20261014`.
`recovery-evidence-before.json` hashes to
`aba9e0de6b46c83347dd72e11b94ab0c23c2e8a7fb26a565aa5f40f3db31ae06`.
The exact review operation is `codex-20261014-storagefull-recovery-01`;
`recovery-review.json` hashes to
`d935d41d09990b21572cbc7915364dfc736068e5e81edbdbba182e4be70be54e`.

The already-running stage's executable, with its verified manifest and explicit
canonical workspace paths, staged this review through
`--stage-uncertain-study-review`. The runtime consumed it without restart and
advanced to revision 38794 with phase `retired_unresolved`. The whole writer
checkpoint was unchanged at retirement. Draft d905's canonical JSON hash remained
`ac8dc1f19de7f300f98ce927f2508021a8c6b847a9a03604976e3f5e88d90468`.
No generation was retried, NEXT dispatched, delivery inferred, or draft deleted.

Follow-up at 18:41:12Z found revision 38799 and a later selected draft d906.
New public entries `astrid_1792002889.txt` and `astrid_1792002986.txt` provide
independent publication evidence; their hashes are in
`recovery-evidence-after.json`. This demonstrates resumed operation, not recovery
of the missing completion. The unresolved dispatch and original draft remain.
The after-receipt SHA-256 is
`169dc5f85137e21381cadc9a2808e7f5d0e877e7176eb2a3052ed06afd5990cd`.

Unchanged process identities: bridge 97420, text provider 3893, engine 35303,
Minime agent 96802. Provider readiness remained true, queue depth zero in the
bounded check, reservoir connected and sync timestamp advanced. The live bridge
stage remains `/Users/v/other/worktrees/writing-development-20261013/bridge-stage-01`,
binary hash `043eea056017a32051ee43cffd2f485501440ff9d95f20aacef257ffd96ee56c`,
manifest hash `daacd50c39ce33b77b7c2c67525a5f2a52ed8e89c987e2f868e515d525d8819c`.

## Candidate Repairs

### Selection Is Not Parking

`ResearchReader` previously labeled every unfinished inactive draft `parked`.
That is unsupported by the selected-state field alone: starting another draft
also leaves an unfinished inactive draft. The metadata-only list now uses
`not_selected`, preserving `active` and `finished`, and CLI help states its scope.
No schema migration, inferred historic park/return, activity transition or write
occurs. Schemas four and five remain readable; current privacy checks remain.

### An Excerpt Is Not an Ending

`JournalRecall::render` retains an exact UTF-8 prefix within the same character
allowance. It prefers the latest complete sentence or paragraph within that
bound, then a word boundary, otherwise an explicitly hard boundary. Additive
metadata records the exact byte interval, body size, truncation and boundary
kind. The closing label distinguishes a supplied boundary from an authored ending.
No original text, eligibility, cue preference, budget or continuation is changed.
This is a punctuation/paragraph heuristic, not a semantic sentence parser.

### Acknowledge the Excerpt Actually Prepared

Three public dialogue generations supplied the same Minime journal excerpt:

| Request under `generations/2026-10-14/` | SHA-256 |
|---|---|
| `gen_1791977781775_dialogue_live_a0.json` | `d53f1116e4dde4b85b8a3196065fb98a3ae069cd1aec95353ac0d8993b4df647` |
| `gen_1791980453689_dialogue_live_a0.json` | `81e6a3965d989af3540e3b2d17a079b85e3fd6501d26c9d98dd7e376c7b7f0f8` |
| `gen_1791980570950_dialogue_live_a0.json` | `e92cc5cf5dc382a9ecb0ed34155a6bd2111e1495da5e46f6a9c0b7789dfe67c0` |

The tracker expected the complete rendered peer block, but dialogue context caps
that block before final request assembly. Even successful submission of the
intended bounded excerpt therefore never acknowledged it. This is a concrete
contract mismatch, not proof that supplied context caused the repeated prose.

The candidate derives each lane's exact expected excerpt after intentional
assembly, then still verifies its presence after final provider adaptation.
Peer acknowledgement only considers user context after the last assistant turn;
an old history copy cannot establish current presentation. The primary and
fallback share the observed submission flag but bind their distinct excerpts.
Missing/removed/header-only/invalid boundaries and later shortening cannot
acknowledge the excerpt. A fallback resets primary-only evidence.

Successful generation plus verified presentation enables the existing unchanged
peer gate. It does not assert full-source reading, comprehension, agreement or
uptake. New source bytes/publications, priority feedback, explicit reading and
MIRROR remain available. Conversation history is unchanged and may still repeat
earlier language; this repair does not promise recurrence will disappear.

## Qualification

Completed qualification:

- Operator-reader suite: 21 passed, including unchanged archives, owner boundaries,
  corrupt/unsupported state and read-only schema-four/five snapshots.
- Complete `astrid-source-study` library, binaries, integration and doc tests
  passed, including private writing, focus, pending delivery and park/return.
- Focused peer-context suite: 16 passed. It reproduces the missed acknowledgement
  with an uncapped expectation and verifies both adapted primary profiles and
  fallback using the prepared bounded excerpt.
- Complete unchanged Minime Python suite against the candidate shared helper:
  2,183 passed, one skipped, 163 subtests passed, in 119.27 seconds.
- Six Python suites (`test_qualify_private_delivery_recovery`,
  `test_deployment_wrappers`, `test_domain_boundary_audit`, `test_steward_control`,
  `test_steward_projection`, `test_evidence_event_store`): 87 passed.
- Shared-crate all-target strict Clippy and shared/bridge formatting passed.
- Domain-boundary audit passed with zero violations and no baseline changes.
- Final native bridge rerun: 2,439 library tests passed, one ignored; seven binary
  tests and ten selected integration tests passed. The integration targets are
  `mock_ws_integration`, `agency_resolver_integration` and `chimera_render`.
  The four compile-fail/facade test targets were compiled by all-target Clippy but
  their nested trybuild cases were not rerun in this source-repair tranche.
- Native bridge all-target strict Clippy passed with `-D warnings`. No lint
  suppression, schema migration or architecture baseline adjustment was added.

Final native logs in the evidence packet directory:

- `bridge-full-final.log`: SHA-256
  `4dc6fbf58b8da54828334e8f55ec78e5342b54ee176f443573734fda6b3e93d1`.
- `bridge-clippy-final.log`: SHA-256
  `8f8098fb000d79244ddf490c2288b97e07669e200bf3d59c8e543d823102a5ce`.

Reproduction uses the settings below and:

```sh
cargo test --offline -p astrid-source-study -- --test-threads=2 --quiet
cargo clippy --offline -p astrid-source-study --all-targets -- -D warnings
cargo test --offline --manifest-path capsules/spectral-bridge/Cargo.toml --lib --bins --test mock_ws_integration --test agency_resolver_integration --test chimera_render -- --test-threads=2 --quiet
cargo clippy --offline --manifest-path capsules/spectral-bridge/Cargo.toml --all-targets -- -D warnings
```

From the isolated Minime sibling, set `ASTRID_SOURCE_STUDY_BIN` to the candidate
`astrid/target/debug/astrid-source-study` absolute path and run
`python3 -B -m pytest -q -p no:cacheprovider --tb=short tests`. Its live-path
isolation guard remains enabled. No Minime test or runtime source was modified.

The newly built operator browser also performed a successful metadata-only live
list: snapshot `3adf14fb259c233428cc415f3ef03ba87f05846c309e97953687d8c98d86ac0d`,
906 drafts. Draft d905 remained revision 12, 41,509 bytes, `not_selected`; d906
was revision 6, 21,301 bytes, `active`. This is a read-only validation, not an
installation or disclosure of prose. A later check found activity revision 38817,
the exact original d905 hash and the old job still `retired_unresolved`.

Unsuccessful attempts, preserved rather than counted as passes:

1. The first broad bridge run passed 2,431 tests, failed eight, ignored one.
   Seven failures required the missing isolated Minime sibling source/fixtures.
   One exposed genuine candidate prompt-wrapper growth beyond the existing
   1,800-character synthetic assertion. The wrapper was made more concise;
   neither body allowance nor assertion threshold was increased. The focused
   aspiration provenance test then passed. Full final rerun is separate below.
2. The first Minime run was mistakenly launched from its canonical checkout.
   Isolation guards refused live-checkout subprocess/read paths. It was stopped
   by SIGINT to that exact owned test runner only: 107 failed, 883 passed, one
   skipped, one teardown error and 62 passed subtests at interruption. No guard
   was bypassed. The complete rerun from the unchanged isolated sibling passed.
3. Initial sibling worktree creation refused an existing directory containing
   76 KiB of bridge-test-generated attractor fixtures. Those artifacts were
   preserved under `minime-bridge-test-artifacts-before-sibling`, then the fresh
   Minime worktree was created. Neither live state nor foreign work was removed.

Build settings: one Cargo job, incremental off, development/test debug info off.
Both exact new target directories passed the read-only cache audit before build,
with 534,509,400,064 bytes available and no warnings. Read-only sibling dependency
links point to the existing `prime_esn_wasm` and `RASCII` checkouts; no source was
copied or changed there. No live model call or private-authored fixture is used.
The subsequent cache check remained within budget: approximately 1.34 GB bridge
debug/test artifacts and 1.25 GB shared artifacts, with 527,338,897,408 bytes free.
These are still in use for candidate qualification, so no cache was deleted.
After Clippy, the final read-only audit still passed: 1,652,367,360 bridge cache
bytes, 1,249,325,056 shared cache bytes and 526,606,790,656 bytes free. This remains
well inside the existing 40 GiB/target and 50 GiB free-space review bounds.
Both canonical trees and the unchanged Minime candidate are clean. Astrid's owned
candidate diff and this note remain unstaged. Final process starts and live
bridge binary/manifest hashes still match the recovery baseline. Controller
remains paused at generation 521, with no lease; its indexed-tail evidence check
passes and V1 sources remain immutable.

## Release Boundary

The recovery is complete as an unresolved retirement, not reconstructed work.
New code is not deployed or merged. Next is immutable bridge/operator-helper
qualification against current live identities and a separately approved graceful
rollout. There is no writer schema or Minime-adapter change in this candidate;
pairing/restart scope must follow the actual frozen release inventory rather than
assuming both consumers need replacement. Preserve pending work and all authored
state. Larger capacity, live controls and paused automations remain separate.

## Approved Immutable Qualification and Integration

Mike subsequently approved immutable qualification, graceful deployment and
local commit/merge of these narrow repairs. No push was requested. Fresh
coordination on October 14 found the same clean canonical and remote main tips,
no foreign mid-edit activity and no active steward lease. The controller remains
paused, now at generation 522; it will not be resumed by this release.

The owned candidate is committed before staging so the immutable source inventory
has a durable Git identity. Its checkout will remain pinned through build,
qualification and activation. Final deployment receipts will be recorded in a
separate documentation commit after integration. This is not activation evidence.

The shared-helper binary is part of the release even though reader/writer schemas
remain 13/5. Qualify both consumers and drain them before switching that helper.
Minime's adapter source remains identical to its canonical launch inventory,
including the previously installed regulation repair. No engine, model/provider,
visual, sensory, reservoir-policy or larger-capacity change is authorized here.
