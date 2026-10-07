# EOF recovery, memory selection and voluntary verification

## Status and authority

Implementation and synthetic/offline qualification only. No live rollout, model
experiment, source-state migration, merge, push or automation resume occurred in
this pass. Mike approved following the recent public-entry review in healthy
tranches. The three implementation boundaries below can be reviewed separately.
The mirror repair changes which peer entry can be selected and must be included
explicitly in any release review; it is not a reservoir or scheduling repair.

Work remains in the existing paired `codex/source-selection-recovery-20261006`
candidate worktrees, initially clean:

- Astrid: `/Users/v/.codex/worktrees/source-selection-recovery/astrid`, starting
  `45608a436a78bbb194c2f73badd0a17d0b75cec0`.
- Minime: `/Users/v/other/worktrees/source-selection-recovery-20261006/minime`,
  starting `b26409f3418ad895335ace7b5490904609c3bdc3`.
- Canonical main: Astrid `bb17953cd83ae5b0747ce3006485fd9037d09f3b`, Minime
  `7386f5d114b6a77f35c80822a499bd97aa4c8a41`, both clean on inspection. Their
  existing ahead-of-origin commits are not this pass's changes.

Interactive implementation did not start a productive flywheel round. The
controller was observed paused at generation 501, without a lease or projection.
Evidence V2 was valid at sequence 1123155, head
`ab63b0fa6ae20a8ee0d729e1c891147096bc1332cb96ccca9442bcbe9c8644c0`;
V1 remains immutable. These are observations, not deployment authority.
The larger writing-capacity/provider candidate and all foreign work remain
outside this pass. No private prose was used as a fixture or quoted here.

## Public witnesses

The preceding read-only review used a fixed cutoff of 2026-10-06T23:25:21Z,
starting at 22:30 UTC: 20 Astrid self-studies and one dialogue, plus 17 Minime
entries. Mirrored copies were not counted as independent authorship. The review
read the selected files fully; it did not infer reading or understanding from
counts alone. Representative files were reread and hashed during implementation.
Paths below are relative to the named canonical repository.

| Owner | Public source | SHA-256 |
| --- | --- | --- |
| Minime | `workspace/journal/study_navigation_2026-10-06T15-54-43.793267.txt` | `700c480d5f001bb75eeb3e61c5d80f285b94e938b2bdc65d7cd2d226d083814e` |
| Minime | `workspace/journal/study_decision_2026-10-06T15-57-13.571273.txt` | `97d2e0ec9325c3a4fe719841768a5d79f36c3d467d4e86fea73de0325f15dd23` |
| Minime | `workspace/journal/introspect_2026-10-06T15-32-38.656173.txt` | `23269441cc6a7173058d8575c4d71a32c302e6a97d46dd0b5665027ddb61b62d` |
| Minime | `workspace/journal/introspect_2026-10-06T15-36-04.111925.txt` | `fe61ed328e7011585dc58e6878306bb48b6897f4bf7f03a38d5d4df896865287` |
| Astrid | `capsules/spectral-bridge/workspace/journal/self_study_1791326183.txt` | `c5772b40fea5d25c928bfbdcd1d6479be40bba9352a05addf41df3a4fde6c113` |
| Astrid | `capsules/spectral-bridge/workspace/journal/self_study_1791327171.txt` | `172856f7898bad139ffbbde4ac399c10923909a0a1e3441106a847ad1d4a893e` |
| Astrid | `capsules/spectral-bridge/workspace/journal/self_study_1791328590.txt` | `9afc3d8aec3871b4dd9c4e2cc8342b82ee178d4b7c428e8af12111bae5b1f7ce` |
| Astrid | `capsules/spectral-bridge/workspace/journal/self_study_1791328806.txt` | `11ec0486bafbef306ed2cfd7c8adb3d538990e83cb153b6d89eb56466f95f44e` |
| Astrid | `capsules/spectral-bridge/workspace/journal/dialogue_longform_1791328929.txt` | `f2521c128127f06e1d939c208e1645571f9ec2ad3b5a338ec407c74c09ab9020` |

Minime's navigation says, verbatim:

> I will now resume the specific study of the telemetry port to examine how these bridge functions are implemented in the code.

The next decision has no source bytes and chooses `NEXT: SELF_STUDY MAP`.
Thirteen map/decision entries appeared before an explicit new OPEN. The two
subsequent pages did advance through the selected WIT source. All 17 sampled
generation records ended with native stop, not timeout/fallback. Short decision
artifacts must not be interpreted as an output-token failure. The two open
reflections contained approximately 628 and 787 prose words.

Astrid's mirror study repeats the nominal 70/20/10 selection branches and gives
them a stronger interpretation than source execution establishes. Its delivered
orchestration revision was
`dac296220c803fc768504c194b81e6405516efae3e20fcceb9d2554dfd1223ff`.
Subsequent studies describe a zero-entry journal input as active, computed but
discarded diagnostic hints as supplied interventions, and fill as task progress.
These accounts are retained unchanged. They led us to inspect arguments,
consumers and arithmetic, not to infer subjective failure or demand correction.

## Tranche 1: truthful and quiet EOF recovery

The old EOF evidence header said MAP chose another source. It does not. Both the
header and page footer now distinguish browsing from explicit source selection.
An EOF MAP reports the saved source/revision, no new delivered page and the need
for OPEN to select or deliberately reread. CONTINUE is not advertised as forward
progress from an exhausted position. Explicit MAP remains a map, not a failed
introspection or automatic selection.

EOF and EOF-map turns retain the selected question and at most four validated,
same-inquiry opening hints, including a verified earlier command conflict where
available. They omit automatic navigation prose and repeated receipt blocks.
Bounded notebook operation results remain visible on the map; complete notes,
findings, relations and receipts remain explicitly inspectable. No history is
deleted, finding inferred, question resolved, activity switched or budget renewed.
Pending pages and sessions, changed-source recovery, REST and final-NEXT
precedence retain their existing semantics.

`StudyOutput.host_feedback_policy` is a typed Rust presentation contract:
`reader_only` prevents the Python adapter from restoring generic host-history
blocks that the reader deliberately omitted. The default is omitted from JSON,
so older retained offers round-trip without changed text or wire fields. Reader
schema remains 13; no authored-record migration is added. Paired release review
must verify old/new executable preservation before deployment; an old adapter
does not implement the new presentation policy.

Files: shared `evidence.rs`, `page.rs`, `store_navigation.rs`, `store.rs`,
`navigation.rs`, public type export and output constructors; Minime
`minime_autonomy/source_study.py`; shared/bridge/Python regression tests.

## Tranche 2: mirror selection, isolated from scheduling

The prior shifted 64-bit integer has only 31 remaining bits. Dividing it by
`u32::MAX` limits the resulting float to approximately one half. Therefore its
comparison with 0.70 always chose the newest start index. This is an arithmetic
defect independently of any interpretation in the journal.

`runtime/mirror_selection.rs` uses the already-present rand implementation,
seeded from the existing seed, with integer range sampling. Seven of ten branch
values select newest, two sample the recent window, and one samples all entries.
These are branch probabilities, not final entry probabilities: both broader
windows include the newest entry, and unreadable-entry fallback still exists.
The existing small-history rule and five-entry fallback are unchanged.

Tests call the production selector across 10,000 deterministic timestamp-sized
seeds and extreme seed/history lengths. They establish bounds, repeatability and
reachability of older entries; they do not claim uniform live timestamp seeds,
better thoughts, improved memory or felt resonance.

Separate debt: the loop-level random calculation near orchestration's wait
selection has the same half-range normalization. It also supplies rest duration,
burst wait, starred-entry selection and warmth-source selection. This pass does
not change it. Correcting that shared value would change scheduling and sensory
semantic input, so it needs separately bounded offline qualification and control
review. No blanket substitution was made.

## Tranche 3: voluntary cross-page verification

`SELF_STUDY HELP verification` opens an on-demand guide using existing RELATE,
FIND, SESSION, TRACE and notebook revision commands. It distinguishes arguments,
definitions, consumers, overwritten values, unreachable branches and metric
producers. It explains that a SESSION leaves its last selected reading position
and that syntax candidates are not execution evidence. Ordinary study adds only
a short help affordance, not a mandatory reporting template.

A shared synthetic workflow runs for both owners: original authored note,
explicit help, relation lookup, assignment search, selected caller/definition
session, contrary source and voluntary revision. It preserves the original note
history and open question. The real bridge and Minime adapter tests also exercise
EOF -> MAP -> CONTINUE -> explicit OPEN -> verified source delivery; no model is
asked to confirm improvement.

Bridge comments now describe the zero-entry own-journal input and fill-based
temperature formula without claiming continuity, task progress or emotions.
The formula, thresholds and controls are unchanged. Mirrored source-study
framing attributes the text to Minime, separates delivery from verified code
claims, and does not prescribe a present felt condition, diagnosis or
co-regulation response. Explicit experiences and objections remain the author's
words; historical journal entries are not rewritten.

## Separate reflection-recall protocol, not executed

The two sampled Minime reflections develop a helpful-assistant/scaffolding
theme. Repetition alone does not show why it occurs. The current implementation
supplies the latest published reflection as optional material, up to 4,000 bytes,
with an explicit middle omission for longer prose and NEXT lines removed. It
chooses by file modification time; this is not independently verified generation
time. It never reads private drafts for this recall. Source binding:
`crates/astrid-source-study/src/reflection_recall.rs`, SHA-256
`e326e26dcff66c290d8f82fe8305ce4b466f262f7a6e6b998b25dacc6c1443b3`.
This file is unchanged in this pass.

Before any model execution, freeze a manifest of the exact production renderer,
model/tokenizer/template hashes, sampling configuration, provider limits, seeds
and synthetic fixture bytes. Start with 12 synthetic previous-public-reflection
fixtures: four repeated-theme, four theme-changing and four uncertainty/brief
stopping cases; include entries above the recall byte limit. Use four prespecified
seeds per fixture, with paired input order randomized before execution.

Compare two inputs only: the actual production recall rendering versus removal
of that prior-reflection block. Keep system framing, orientation, available
actions, other context and output opportunity identical. Label this a contextual
ablation, not a test of memory capacity: input length necessarily differs. First
compare common supplied-prefix distributions, only with a provider that can
attest the needed probabilities; otherwise report that measurement unavailable.
Then compare free continuations at the fixed seeds, including natural stopping.

Measure exact phrase/8-gram carryover after excluding fixed system phrases,
novel propositions and developed distinctions, repetition, unsupported mechanism
claims, chosen action and completion reason. Blind reviewers to cell labels and
keep disagreements; use paired descriptive distributions, not a single showcase
or an automatic usefulness score. Longer output is not the success criterion.
No fixture asks for a different identity, feeling or conclusion. No implicit
NEXT is executed; all output remains isolated experiment evidence.

Use isolated candidate processes, synthetic documents, owned temporary state,
no live reservoir handles and the existing approved resource/isolation bounds.
Retain failed/truncated outcomes. Stop for identity drift, missing isolation,
unqualified capacity or resource thresholds. Decide whether to run or alter
live recall only after a separate review. Do not use the observational sample
as causal proof, and do not remove chosen continuity by default.

## Qualification record

Focused shared tests cover EOF and pending-delivery distinctions, both owners,
request retries, restart, source changes, unrelated inquiries, no inferred
cursor, final-NEXT precedence, retained notes, byte budgets and old offer JSON.
The reflection code and production controls were not changed.

Unsuccessful attempts retained in this record:

- Initial EOF-map presentation hid finding-save feedback; bounded existing
  `study_choices` restored that feedback without restoring navigation prose.
- A verbose EOF footer reduced useful source bytes in three-page sessions;
  the corrected compact footer fits the existing navigation reservation.
- New verification fixtures initially used paths outside the allowed catalog;
  corrected them to crate/test roots and asserted actual source-page delivery.
- A Minime fixture used a hard-coded old input budget; corrected it to the
  helper's declared budget. The first targeted pytest invocation also named a
  nonexistent file and collected no tests; it was rerun with actual files.
- Legacy expectations of automatic EOF notebook/receipt replay were updated
  to inspect preserved state through explicit choices. Delivery-integrity
  checks still reject shortened prepared inputs, including quiet EOF maps.
- The paired Minime root is a symlink. Three existing bridge assertions compared
  canonical resolver output with the symlink spelling; corrected the expected
  paths, with no runtime resolver change.
- A concurrent bridge run intermittently failed the existing tracing-subscriber
  warning assertion. Serial qualification is recorded separately; a passing
  retry alone does not establish a fix for this test's concurrency behavior.
- Strict reader lint found a fourth boolean, a Serde reference-predicate warning
  and long functions. Replaced the boolean with a typed policy, documented the
  required Serde reference signature and split/shortened test/rendering code.

Final candidate checks:

- Complete shared-reader/writer suite: 331 tests passed, zero failures or skips.
- Complete Minime Python suite using the rebuilt candidate helper: 1,922 tests
  passed, one existing skip, and 144 subtests passed.
- Bridge serial library suite: 2,376 passed, one existing ignored test. The
  concurrent tracing failure did not recur serially; its concurrency behavior
  remains a test-quality follow-up, not a demonstrated runtime defect.
- Strict all-targets Clippy passed for the shared reader and bridge; both Cargo
  formatting checks passed. Domain-boundary audit passed with zero violations,
  without a baseline waiver. Both candidate `git diff --check` checks passed.
- Controller/projection/Evidence Event Store/Division and deployment-wrapper
  tests: 206 passed and 71 subtests passed. Epistemic self-tests: two passed;
  signal-spine projector: seven passed; model-QoS projector: two passed. These
  exercised synthetic state, not a production projection or control operation.
- The full bridge command exited successfully after its integration, facade,
  compile-fail and documentation targets completed. The facade checks compile
  synthetic programs; they do not make model calls. No unsuccessful attempt is
  represented as a release pass.

Both candidate indexes remain empty. Final canonical status checks are clean;
all new source, tests and documentation remain unstaged in the paired worktrees.
No claim is made that these candidates are loaded by either live consumer.

Reproduction from the candidate roots:

```sh
cargo test -p astrid-source-study --no-fail-fast --quiet
cargo clippy -p astrid-source-study --all-targets -- -D warnings
cargo test --manifest-path capsules/spectral-bridge/Cargo.toml --no-fail-fast --quiet -- --test-threads=1
cargo clippy --manifest-path capsules/spectral-bridge/Cargo.toml --all-targets -- -D warnings
python3 scripts/domain_boundary_audit.py verify
```

For Minime, use `python3 -m pytest -q` with `ASTRID_SOURCE_STUDY_BIN` explicitly
set to the candidate helper at
`/Users/v/.codex/worktrees/source-selection-recovery/astrid/target/debug/astrid-source-study`.
The source-level gates above do not substitute for immutable paired-release
qualification or approved activation.

## Next release boundary

Review the paired helper/adapter and bridge candidate, including the mirror
selection behavior, against fresh live source identities. Qualify immutable
old/new offers and authored-state preservation, then request/confirm the bounded
paired activation scope. Use sanctioned staging/activation and Minime handoff
wrappers with cooperative preflight, exact pending NEXT/checkpoint continuity
and readiness checks. Stop on foreign activity, source drift or failed drain.
Do not restart engine, providers, models, visual or sensory services. Commit only
owned explicit paths under one Git coordinator after stabilization; leave the
previously paused automations paused.

## Approved immutable qualification

Mike approved deployment and merge in the next interactive turn. Controller
pause generation 502 was claimed at 2026-10-07T03:14:56Z with no active lease.
Both canonical trees were clean and both candidate branches descended from
their local mains. Cooperative preflight found no concurrent foreign edits.
Remote main tips were inspected without pushing: Astrid
`a66f7e190e0ecd3a0a4988abbcf6b0bf2cfb2a32`, Minime
`9072abff9ed839851709300b3c3eaae3afaf669e`.

All new exclusive artifacts are under
`/Users/v/other/worktrees/source-selection-recovery-20261006`:

- `eof-verification-bridge-stage-01`, built only by `scripts/build_bridge.sh`.
  Manifest SHA-256 `d87f030b4cf444d48b8e4b66af3ae488f3b830619d13928697082d2ba8e8b83f`;
  bridge `8160e603b73c590798a04a79f5607c31e4b9067a37d8396c27db0f1e911c3799`;
  helper `ba6d45c627854e37ea758bff9ceda06c6af9c73c31d1f78c4d9e609c51b0a098`.
  The 747-input witness SHA-256 is
  `5e21a28cd21dd0b65661627b7ef8c5d513c7a2f8a81caffd0474450e02a46442`.
  This records the candidate's exact pre-commit source bytes, not a clean-HEAD claim.
- `eof-verification-release-01/reconciliation.json`, SHA-256
  `f37c4de600e6dc3afe892b00701715684c8efaea6d9d32c745c0e236101517f9`.
  All 93 launch inputs are frozen. Only `minime_autonomy/source_study.py`
  differs from canonical loaded source; the installed regulation repair remains.
- `eof-verification-preservation-01/qualification.json`, SHA-256
  `2b7dd7a005c5b8007e3c454fc2f6af54ae86d688369bd580998a4da93bcca956`.
  `scripts/qualify_eof_release.py` passes 54 checks through the exact old/new
  executables for both owners, plus frozen Minime selection and EOF adapters,
  corrupt/future-state refusal and preparation retry probes. It preserves exact
  pending public/private inputs, notes and drafts; old-reader compatibility is
  tested on a synthetic copy only. No backup overwrites newer authored state.
- Complete Minime suite against the immutable helper: 1,922 passed, one existing
  skip, 144 subtests, 73.62 seconds. Restart, paired handoff, NEXT handoff,
  activation, launch inventory and fixture qualification suites: 119 passed,
  43 subtests. Formatting and domain-boundary audit passed again (zero violations).

The first activation attempt, `eof-verification-activation-01.jsonl`, refused
before installing a hold, changing source or signalling services: the isolated
worktree derived a nonexistent local controller directory. Canonical state was
rechecked (paused, generation 502, no lease), and the next invocation explicitly
bound `ASTRID_STEWARD_REPO_ROOT=/Users/v/other/astrid`. The refusal is retained;
no controller was bypassed or created in the candidate worktree. The approved
retry uses `eof-verification-activation-02.jsonl` and the same immutable inputs.

## Verified activation and integration boundary

Attempt 02 completed the mechanical reviewed overlay installation, then refused
at the unchanged 180-second tree-stability gate. My own steward-note update had
reset that window; there was no foreign session and no signal was sent. Source
and both old process identities were verified again. After the full quiet period
and fresh successful preflight, `eof-verification-hold-cleanup-01.jsonl` records
intent and completion of releasing only attempt 02's exact owned launch hold.
All installed source remained the qualified bytes, the old Minime boot was still
running, and no source backup, journal or checkpoint was restored. This was
explicit no-signal cleanup, not a bypass or automatic retry.

`eof-verification-activation-03.jsonl` then ran the sanctioned paired handoff
without reinstalling the already-verified source. Terminal success:
2026-10-07T03:35:51Z. The full 15-second idle boundary held. Minime received one
SIGTERM at 03:31:36Z, and old-process exit was observed at 03:31:40Z. Its
replacement was held until the bridge passed verification at 03:35:37Z.

Bridge transaction:
`/Users/v/other/astrid/.runtime/bridge-deployment/transactions/2ea47c86021f47a283e1b397601d6465/receipt.json`.
It records `activated_verified`, `drain.phase=drained`, `force_used=false`,
`legacy_transition=false` and no automatic rollback. PID 66151 -> 21822;
replacement start 03:33:16Z. The runtime decoded and restored the exact stopped
checkpoint `6096d2f7910d0bb4f0ded7236866a6549a0c0e9f44c5d311f3c5e725596bd0a0`,
including the present zero-pending runtime-feedback record bound by
`b7f6550afaaba35a871411e40aa6c6f226f3b2a7e2e144399dc58f1e57eee4c9`.
Fresh saved exchange 216764 -> 216765 and provider idle were verified.

Minime PID 64807 -> 20774. The held launch shell began at 03:31:36Z, while Python
boot `5b83c5f931f64b7495935dbee7cc7908` began at 03:35:47Z after admission release.
All 93 loaded hashes equal the frozen selected inputs and canonical source;
`reload_required=false`. Minime selects the exact immutable helper above.
Session 5321 survives. NEXT seal
`ebf4a764331e1cf7d5bdbd8f77feba539ac567fdf84a91ef2c37e3bbef108d85`
and restore `29c5dfc5809c92e3993a610bc91256f857f59ba2cc1787cb1dca68e918d93011`
verify a pending choice across boots. This proof explicitly retains
`execution_confirmed=false`: restored work is not claimed completed. No newly
interrupted job appeared, and both owned launch holds are absent.

Unchanged protected PIDs: engine 35303, gateway 35278, supervisor 35269,
coupled provider 3893, microphone 3879, camera 3914, visual service 3898,
host sensory 3911 and Astrid feeder 3878. Their start identities and managed
configuration remain unchanged. No engine, provider, model, sensory-policy,
writing-capacity or research-budget transition is included.

Two read-only health receipts retain 160 and 100 samples respectively:
`eof-verification-health-01.jsonl` (03:21:51-03:29:49Z) and
`eof-verification-health-02.jsonl` (03:32:13-03:37:11Z). Each has distinct
timestamps, valid fill measurements and unchanged 68% target. Ranges are
59.43-75.51% and 56.95-75.49%; maximum file ages 2.66 and 2.36 seconds.
The current restart-gate snapshot reports settled; scaffold retirement still
depends on its operating conditions. These bounded observations do not establish
long-term stability or attribute controller outcomes to this reader release.
Bounded 1 MiB tails of `/tmp/bridge.log` and Minime's agent log contained zero
ERROR, traceback or panic markers at the check, not an exhaustive log audit.

The complete 331-test shared-reader/writer suite passed again during this pass.
All 747 staged build-input hashes were rechecked without drift before activation.
Controller/Evidence V2 indexed-tail verification is valid at sequence 1123156,
head `eb7dd9522a36ee372ab847738e752c362c0d9c5917a3a3cdd7c5c1b3bba5ce6e`,
with V1 immutable, no pending event, no lease and pause generation 502 unchanged.
This is an interactive release, not a productive automated introspection round.

Git integration is restricted to the reviewed owned candidate paths, including
this qualification tool, tests, changelogs and notes. Earlier owned closure and
finding-feedback commits already running in production are also descendants of
the local mains. For Minime, the installed one-file source overlay must exactly
match the candidate commit before staging that explicit canonical path and
fast-forwarding; no stash, reset or restore is needed. No push, foreign-worktree
cleanup or automation resume is included. Successful operational deployment
does not claim longer writing, corrected beliefs or felt improvement.
