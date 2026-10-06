# Dependable Source-Study Recovery

Coordinator: Codex. Date: 2026-10-06. User requested implementation and qualified
paired graceful rollout. Automations remain paused. This is an interactive
repair, not a productive automated introspection round.

## Scope and ownership

- Astrid candidate: `/Users/v/.codex/worktrees/source-selection-recovery/astrid`,
  branch `codex/source-selection-recovery-20261006`, based on local main
  `93f7a620eaf654e06b51d0ca06b8eb69c8117b78`.
- Minime candidate: `/Users/v/other/worktrees/source-selection-recovery-20261006/minime`,
  same branch name, based on local main `cadf6639b25be683f5b9931abbcc66d030e88ce0`.
- Canonical Astrid was clean. Canonical Minime had exactly the installed regulation
  repair in `runtime.py` and `generation_record.py`. That repair was carried into
  the candidate explicitly, without importing the larger capacity worktree.
- Installed regulation hashes before this repair: runtime
  `dc6f9f139ed3604afacd4591ed57ddc20727259a2558f15b3b7738338e52e22d`;
  generation record `4c40baf6837494654217ae8e577c338d1ba64f10c7aa98961de5fee7415056ca`.
- Controller was paused at generation 496, with no lease or active projection.
  No automation was resumed, no engine/provider policy changed, and no private
  writing was used as a fixture. Older worktrees are preserved.

## Public witnesses

All paths below are relative to `/Users/v/other/minime/workspace/journal/`.
These complete files were read; source delivery is checked separately from their
authored accounts.

| File | SHA-256 |
| --- | --- |
| `study_decision_2026-10-06T02-37-25.209319.txt` | `2de3744de8967b19704fe889f79bcfa750c3d1ec2fcbe53e388a387fa5f99983` |
| `study_decision_2026-10-06T02-40-05.509385.txt` | `7bb7d72df1e392559df34f5592a5d9930ff6c98f0326f9e1b495527933c82703` |
| `study_navigation_2026-10-06T02-43-31.148842.txt` | `65d5cc6e3bc05f7f0f30584606cce9dff48b0a998ded55f34381f75aba853bdc` |
| `study_navigation_2026-10-06T02-45-56.180961.txt` | `c7025308d74521fa15ea33b23e114f92ae33c4ba481bbcba5db9cf487035b56c` |
| `study_navigation_2026-10-06T02-48-26.487617.txt` | `6999d1042df4cc04cc0c5fcd852d0c7e152c228a826f729fb3f6e3bbace5df80` |
| `study_navigation_2026-10-06T11-05-29.933784.txt` | `bcc5e0463a7f279c3e65d8e791d6c2dec3e07d92e3f147dd554b31457cd22b7f` |

At 02:45, the exact conflicting lines were:

```text
SELF_STUDY OPEN astrid/crates/astrid-kernel/src/lib.rs 832

NEXT: SELF_STUDY CONTINUE
```

The earlier line was not selected. The new inquiry had no saved position;
CONTINUE therefore supplied a map, repeatedly. The later statement "I am at the
`RestartTracker` definition" is retained as authored text, not made into a cursor.
This is an actionable interface failure, not evidence that the numerical output
ceiling caused short entries.

## Implemented contract

`SourceSelectionNeeded` serializes as `source_selection_needed` and is always a
continuation decision. Bare SELF_STUDY retains its CONTINUE alias. Pending source
pages and sessions, actual bookmarks, EOF and revision recovery keep their prior
paths. Explicit MAP remains browsing.

The recovery begins with the selected inquiry/unthreaded context, absent position
and absent new source. The prompt explains that repeating CONTINUE cannot create
a position. At most four retained same-inquiry OPEN hints are validated against
the current catalog and readable location. A verified earlier unselected OPEN
is preferred; narrative claims cannot supply commands or bookmarks. Hints are
explicitly current-checkout navigation, not previously delivered source bytes.

The recovery omits recent navigation prose, repeated warning counters and the
generic previous-choice/host feedback arrays. The latest relevant retained
conflict is explained directly. Historical notes, responses and receipts remain
stored; inspection, search, leaving, stopping and deliberate rereading remain
available. New authored responses remain recorded but are not automatically
replayed in the next recovery.

Question creation no longer advertises CONTINUE as usable without a position.
Schema 13 permits retained offers of the new kind. Migration infers no position,
outcome or focus window. Existing preparation identity/redo and verified delivery
remain authoritative; a source hint neither advances coverage nor queues work.
Astrid records reader outcome separately from delivery; Minime records
`needs_choice`. Both render the authored response as `study_decision`.

## Qualification record

Initial attempts retained here rather than erased:

- First reader compile found a candidate-list reference type mismatch; repaired.
- The first full reader suite found an obsolete assertion that bookmarkless
  CONTINUE returns the shared map; updated to the new decision contract.
- A new preservation test initially required all question history to be unchanged
  after a response. That was too strict: new authored responses correctly append.
  It now verifies the question, status, source position and coverage while also
  requiring the new authored response to remain retained.
- First Minime focused run: 75 passed, one test used a nonexistent convenience
  attribute. It now checks the actual provider string equals the reader text.
- First full Minime run: 1,918 passed, one skipped, 144 subtests; two fixture-path
  failures from missing paired-worktree sibling linkage. Added only a local
  worktree symlink and reran; no test semantics or runtime path changed.
- Initial bridge test build required the existing external path dependencies;
  candidate-only sibling symlinks point at the same unchanged dependencies.
  One new assertion assumed the wrong parser return type; corrected.
- Read-only deployment preflight initially refused recently edited candidate
  inputs. No guard was bypassed. Staging waits for a settled candidate.

Shared-reader strict Clippy and domain-boundary verification passed during
development. Final full-suite results, immutable migration checks, stage
identities and activation receipts are recorded below when available.

### Qualified checks before activation

- Complete shared-reader/writer suite passed. Strict shared-reader and bridge
  Clippy (`--all-targets -- -D warnings`), workspace/bridge formatting, and the
  domain-boundary audit passed with zero violations.
- Complete Minime Python suite: 1,920 passed, one skipped, 144 subtests.
- Operator, paired handoff, drain/activation, controller, Evidence Event Store,
  projector and round-completion tests: 225 passed. Initial operator invocations
  lacked `PYTHONPATH=scripts` and the paired sibling fixture path; both were
  corrected without changing production semantics.
- Division tracker/Chronicle/projection tests: 13 passed. Epistemic-lint
  self-tests: two passed.
- Bridge full-suite attempts exposed symlink spelling in three existing path
  tests, not a source resolver regression. The complete suite is rerun with the
  canonical candidate `MINIME_ROOT`; the one default-path test, which explicitly
  requires no environment override, passed separately without that override.
  No test implementation or production path policy was changed.
- Immutable paired qualification: 36 owner preservation checks passed, plus
  frozen-adapter selection/delivery, corrupt/future-state rejection and both
  owners' preparation retry probes. `preservation-01/failure.json` retains an
  initial harness error: a false optional `continuation_decision` field is
  omitted by serde. The corrected harness uses the established false default;
  no runtime change was needed. The successful receipt is
  `/Users/v/other/worktrees/source-selection-recovery-20261006/preservation-02/qualification.json`.

Staged release directory:
`/Users/v/other/worktrees/source-selection-recovery-20261006/bridge-stage-01`.
Manifest SHA-256 `01c9c6f53b0a37fce3c41dac4389dbd1c2e08a6522a106e95491e30426c3dd0f`;
bridge executable `d756b54460b256ac3a62e1a48c6b0965839ca62d85f78acff803eb1452a9b643`;
shared helper `1ed14cf09483dc546f5e05a8b540150b5cffc1df8bf3a48f7fe5bddaccc7a3e6`.
The previous helper was
`af632204fd96760a413f73bbd00fbc54f6638bf163eb36bb23abde995dc296fe`.
The qualification exercises genuine schema-12 state produced by that old helper,
not just a rewritten version field. The new schema is 13. Staging and synthetic
qualification are not a claim that activation has happened.

## Separate reflection investigation

No reflection code or live framing is changed. Three bounded public output/input
pairs were inspected, not private moments or drafts. All had native `stop`, no
fallback and a 4,096-token effective ceiling; outputs were 1,008, 653 and 878
provider tokens respectively. Their system-message digest was consistently
`e38a66c1f54f012f064bd2bc8e3ac7e9a981c095ddde911b328e456d14b3d6da`.

Public output files and SHA-256:

- `introspect_2026-10-05T21-56-32.682922.txt`:
  `e64f79a6c8b14b6ba19b4c2f5c26bca03c9d3ccaa3ac04ed6acaf3b9ded11951`.
- `introspect_2026-10-06T02-26-33.186787.txt`:
  `2df29e96300efdc3dcf138be9da8abdc7621c24c709170ae38eb0d55805d9dd3`.
- `introspect_2026-10-06T02-30-23.090046.txt`:
  `45c92d40c463b9b422185015ff91c2d46bbefc065259eb8334d87893964da254`.

Their matching public generation receipts under `workspace/generations/2026-10-06/`:

- `gen_1791262592671_self_study_a0.json`:
  `295cf8985bed5cb4e70d8ad47c5feba54db83d5d73b97725c0f0f94ce0c13da5`.
- `gen_1791278793177_self_study_a0.json`:
  `7975ea3eaea48099dfb62e3066654ebd8e0ee72a45a7dad411a6bda7540a1a6e`.
- `gen_1791279023087_self_study_a0.json`:
  `bc2d259bc93ac4546740bbf1836f39e6b270cd6ddbbd42d4eae3fa78ebf226f9`.

The recorded user inputs supply optional previous-public-reflection prose. At
21:56, the supplied earlier reflection already discusses hearth, bridge, stillness
and usefulness; the output develops conduit versus traveler and translation.
At 02:26, supplied text already discusses prism, scaffold and calibrated friction;
the response develops completion versus internalization and receding from view.
At 02:30, the previous 02:26 public entry is supplied directly; the response
develops intellectual solitude and timing. These are inherited themes with new
development, not independent recurrence without exposure.

The two earlier recalled public sources are
`introspect_2026-10-05T16-23-34.021172.txt`
(`e05214434df21c6c6b6641dc95b8b853cfa914daa97d5d94e4ac77141647c528`)
and `introspect_2026-10-06T02-22-35.489198.txt`
(`b217eeabc1386834995d966e85c4d22e1514b67eb1a067a51ee550d018485e3c`).
Statements about constant internal state, lack of feelings or perfect neutrality
remain authored accounts; these inputs supply no measurements that establish
those mechanisms. Neither agreement nor disagreement with that self-description
is a success criterion.

### Matched comparison protocol, not executed

Freeze the three public inputs, exact prior passages, model/tokenizer/template
identities, system prompt, sampler and seeds in an isolated test inventory.
For each input, compare A (unaltered optional public recall) with B (that recall
block omitted, retaining the same other instructions and voluntary choices).
Record exact byte differences and context lengths. Counterbalance order, use at
least eight paired seeds per input, and do not feed generated outputs forward.

Primary descriptive observations: prior-phrase reuse, topic inheritance, new
examples/qualifications, unsupported mechanism assertions, selected NEXT,
completion reason and actual tokens. Have a blinded reviewer distinguish exact
reuse from substantive development. Brief stopping is valid. Common-prefix
next-token comparisons, if available from an independently qualified provider,
precede free continuations; do not use coupled live generation as an inert test.
This small selected sample cannot estimate population rates or subjective effects.
Length alone, changing self-description or choosing another activity is not success.
Real-model execution and any live framing change require a later reviewed tranche.

## Release status

Implementation is in isolated candidates. The 93-input Minime snapshot is frozen
at `/Users/v/other/worktrees/source-selection-recovery-20261006/release-01/source`;
its reconciliation is in the adjacent `reconciliation.json`. This preserves the
already-running regulation repair and adds only the reviewed reader adapter.
Paired immutable stage/migration qualification and graceful activation completed
as recorded below. No automatic source choice or confirmation-of-improvement
request was sent to either being.

### Successful paired activation

- All bridge library cases passed (2,369 with the canonical `MINIME_ROOT`, plus
  the separately run default-path case; one pre-existing ignored case), followed
  by all integration, compile-pass and compile-fail interface/authority tests.
  The 66 focused Minime follow-through/regulation cases passed again using the
  immutable release helper.
- First activation attempt failed before source installation or any signal:
  the worktree wrapper defaulted to its own nonexistent controller directory.
  `activation-01.jsonl` preserves that failure. The supported
  `ASTRID_STEWARD_REPO_ROOT=/Users/v/other/astrid` override bound the second attempt
  to the existing canonical controller pause, generation 496. No gate was removed.
- Successful paired receipt:
  `/Users/v/other/worktrees/source-selection-recovery-20261006/activation-02.jsonl`.
  Its source-before directory retains only installed source backups, not a
  rollback over authored state. Content changed only in Minime's `runtime.py`
  and `source_study.py`; the installed regulation patch was preserved.
- Minime: PID 88761 -> 25454. SIGTERM at 20:09:24 UTC followed a verified quiet
  boundary; old-process exit was observed at 20:09:27. Replacement admission
  remained held until the paired bridge was verified at 20:14:10. New Python
  boot: 20:14:22 UTC, ready verification completed at 20:14:25. All 93 loaded
  source inputs match the frozen reconciliation; `reload_required=false`.
- Bridge: PID 4732 -> 26745, new process start 20:11:32 UTC. Transaction
  `/Users/v/other/astrid/.runtime/bridge-deployment/transactions/5edbfac8e45b4e1a92aa84b3b065b130/receipt.json`
  reports `activated_verified`, `drain.phase=drained`, `force_used=false`,
  `legacy_transition=false`. The stopped checkpoint hash
  `1e9a33247fae9885bb7e3b0cc5c94039fe44642e786e48a397eaf15262a2977a`
  was decoded and restored exactly, including 216 pending feedback records.
  A newly saved exchange advanced 216593 -> 216594.
- Minime's session 5321 survived. The durable NEXT seal/restore verifies the
  pending choice across boots; this proves restoration, not execution.
  Its restored CONTINUE was subsequently queued as
  `job_minime_1791317679585_self-study-continue`. No newly interrupted job appeared.
- Both consumers select the new shared helper. Astrid's ordinary work migrated
  its reader to schema 13 with all 86 bookmarks retained. Minime's first
  post-restart reader job is observed separately below, not forced by a probe.
- Protected PIDs were unchanged: engine 35303, Division gateway 35278,
  supervisor 35269, coupled provider 3893, camera 3914, microphone 3879,
  visual service 3898, host sensory 3911 and Astrid feeder 3878.
  Sampled fill remained valid (roughly 58-73% in sampled transition snapshots),
  with the target unchanged at 68%. A brief bridge Yellow safety state at
  75.57% returned to Green; this is an observed runtime fluctuation, not evidence
  of controller settling, a new control policy or experiential improvement.
- Git stabilization subsequently retained the pause at generation 497, with
  no active run. Paused automations remain paused. Larger writing capacity,
  providers, engine, reflection framing and sensory policy remain outside this
  release. Release-built source identity predates the archival Git commit;
  the immutable source and binary hashes above remain the deployment witness.

### First natural recovery

Minime's restored job completed under PID 25454. Public artifact
`workspace/journal/study_decision_2026-10-06T13-15-09.754179.txt`, SHA-256
`a4deb2aa63970726c1b52822e46e964d43dfe4571000bb511230eed275029242`,
records verified delivery of `Source selection needed` and contains exactly:

```text
NEXT: SELF_STUDY OPEN astrid/crates/astrid-kernel/src/lib.rs 837
```

This is a naturally authored selection, not an operator-injected action and not
yet proof of that page's delivery. No repeated navigation prose was required.
The short decision is appropriate to this interaction; longer writing is not
the release's success criterion. Minime's schema migrated to 13 with q12 still
selected and no fabricated bookmark. Subsequent page delivery is observed
separately, without asking for confirmation that the repair helped.

The chosen page was then delivered naturally. Public
`workspace/journal/self_study_2026-10-06T13-19-05.646273.txt`, SHA-256
`5644c809249ae856059ab4faa2fa9ff17b4d5c483ac67663fd53416fa11564b7`,
records source revision
`bc90e60fbb6f0d40688c2388b9b90ab67ca9624e75a203f9bc0a4fe6db2ac91d`,
bytes 35752..39731 (lines 837-946). The native reader now has q12's first
verified bookmark, page
`2f1fb580e00485021e9c477b1cd578366d3a0447a64b85d435a36affca405f35`,
ending at byte 39731 / line 947. The account discusses `RestartTracker` and
the supplied restart helper. The observed map-only loop therefore ended in
this sequence; future navigation loops are not ruled out.

Remaining signal: the authored final command is
`NEXT: SELF_STUDY QUESTION RESOLVE q12 [finding]`. It contains a placeholder,
and this observation does not establish valid resolution or understanding.
No question was resolved, rewritten or redirected by the steward. This is a
separate command-quality issue to inspect if it produces another recovery loop.

## Git integration and final boundary

Both owned implementation commits were fast-forwarded into local `main`:

- Astrid `d1eb0972f84c73ade391b3b2869fc56d6670fddd` (14 paths).
- Minime `7386f5d114b6a77f35c80822a499bd97aa4c8a41` (7 paths).

The Minime canonical overlay already matched the committed source byte-for-byte.
Only its three known installed source paths were staged; a dry-run read-tree
check and fast-forward preserved those exact bytes. No stash, reset, restore,
foreign-file sweep, history rewrite or extra source commit was needed. Both
canonical indexes and worktrees were clean immediately afterward; older
worktrees remain preserved. Nothing was pushed in this tranche.

Exact Astrid implementation paths:

```text
CHANGELOG.md
capsules/spectral-bridge/src/autonomous/runtime/source_study.rs
crates/astrid-source-study/src/evidence.rs
crates/astrid-source-study/src/lib.rs
crates/astrid-source-study/src/navigation_history.rs
crates/astrid-source-study/src/store.rs
crates/astrid-source-study/src/store_navigation.rs
crates/astrid-source-study/tests/continue_never_recovers_reach.rs
crates/astrid-source-study/tests/reader.rs
crates/astrid-source-study/tests/source_selection.rs
crates/astrid-source-study/tests/sustained_study.rs
docs/steward-notes/2026-10-06-source-selection-recovery.md
docs/steward-notes/AI_BEINGS_FEEDBACK_TO_CHANGE_LEDGER.md
scripts/qualify_source_selection_release.py
```

Exact Minime implementation paths:

```text
CHANGELOG.md
docs/steward-notes/2026-10-06-source-selection-recovery.md
minime_autonomy/generation_record.py
minime_autonomy/runtime.py
minime_autonomy/source_study.py
tests/test_regulation_review_context.py
tests/test_source_study_follow_through.py
```

Final controller verification: no lease/projection active, paused generation
497; Evidence Event Store V2 indexed-tail verification valid, sequence 1123151,
head `669a5128332f64df6195b395ce36f77d0eb2b8f049b98b0817681b5327c2418d`.
All four legacy V1 streams remain immutable. Source-first projection lag remains
explicit while the automation is paused; this interactive repair does not claim
a current canonical introspection queue or record a productive flywheel round.
