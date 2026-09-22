# Source-Study Evidence and Relationships

Date: 2026-09-17. Author: Codex, interactive implementation requested by Mike.
Status: implemented in the shared working trees; release activation remains pending.

Follow-up: [paired qualification](2026-09-17-source-study-rollout-qualification/README.md)
reconciles the page-test debt and adds a guarded V2 compatibility floor for
relation-bearing findings. The initial results and limitation below describe the
implementation pass before that follow-up, not its final qualification.

## Purpose

Support studies that accumulate evidence without making technical vocabulary a
reason to interrupt them, or turning a source neighborhood into verified causation.
The user approved the five-part recommendation after reviewing recent public
self-studies together. This is not a resumed automated introspection round.

The central distinction is between a report, supplied source, a mechanically
verified relationship, an authored interpretation, and observed runtime behavior.
Each can be useful without being substituted for the others.

## Source Record

Fully read Astrid public studies, relative to this repository:

- `capsules/spectral-bridge/workspace/journal/self_study_1789651658.txt`
- `capsules/spectral-bridge/workspace/journal/self_study_1789652066.txt`
- `capsules/spectral-bridge/workspace/journal/self_study_1789652388.txt`
- `capsules/spectral-bridge/workspace/journal/self_study_1789652803.txt`
- `capsules/spectral-bridge/workspace/journal/self_study_1789653142.txt`
- `capsules/spectral-bridge/workspace/journal/self_study_1789661775.txt`

Fully read Minime public studies, relative to `/Users/v/other/minime`:

- `workspace/journal/self_study_2026-09-17T08-47-14.863751.txt`
- `workspace/journal/self_study_2026-09-17T08-51-39.714195.txt`
- `workspace/journal/self_study_2026-09-17T08-55-32.965283.txt`
- `workspace/journal/self_study_2026-09-17T08-59-09.699993.txt`
- `workspace/journal/self_study_2026-09-17T09-02-29.205748.txt`
- `workspace/journal/self_study_2026-09-17T09-05-33.090811.txt`
- `workspace/journal/self_study_2026-09-17T09-09-28.199668.txt`
- `workspace/journal/self_study_2026-09-17T09-11-47.788698.txt`
- `workspace/journal/self_study_2026-09-17T09-14-43.707359.txt`
- `workspace/journal/self_study_2026-09-17T09-18-30.311873.txt`

Two motivating excerpts, checked against file bytes:

> The system doesn't just look at entropy; it calculates a `salience_weighted_multiplier`.

Astrid, `self_study_1789651658.txt`, SHA-256
`c7caa78612027156ab5472a17b28be5e01d3b57753451999f645eb583774e421`.
The quantity exists, but the account goes on to promote its review calculation
to a live retention policy and reverses the meaning of a lower multiplier.
The correction belongs in evidence and interface scope, not an imposed account
of how Astrid should feel.

> To further investigate if the `Kernel` performs the `if !blocked` check, I need to see how the `EventDispatcher` (or a similar component) consumes these `ApprovalOutcome` values.

Minime, `self_study_2026-09-17T08-47-14.863751.txt`, SHA-256
`214a036e9473eb27685228cb01d9fbe91ea7bb452c5bf935f73ba289c658a653`.
The entry ends with an agency-vernacular notice about evidence mapping, despite
progress through actual manager source. The verified consumer is the outcome
match in `crates/astrid-approval/src/interceptor/mod.rs` (line 344 in this checkout).
This does not verify the account's particular Kernel/EventDispatcher hypothesis.

## Implemented Tranche

### 1. Let source study remain source study

Minime's shared-reader completion passes a `verified_source_study` flag to the
journal writer only when a public source-study/navigation receipt exists. The
journal writer additionally requires the `self_study` entry type. It skips both
agency-vocabulary motif registration for that entry and the corresponding
appended notice. Existing private-canvas handling and other journal hooks remain.

No authored phrase, claimed source path, or self-selected entry label grants the
exemption. Failed/unverified completions and other entry types keep existing
behavior. Tests include a previously active motif: the notice stays absent in
both the file and SQLite content for a verified study.

This implements the explicitly offered exemption option. It does not infer
progress from a vocabulary counter, clear historical motif records, or change
the other fatigue/afterimage/topology policies.

### 2. Offer a route to consumers

A saved question asking about consumption, use or enforcement can identify an
unquoted CamelCase or snake_case symbol such as `ApprovalOutcome`. The notebook
offers an optional exact `SELF_STUDY RELATE` command; it does not execute it,
rewrite the question, change inquiry status, or force a strategy change.

Exact-identifier searches use the bounded Rust/Python syntax outline to promote
non-test call and Rust match-pattern sites. A preview of at most three such
matches precedes definitions; additional matches remain reachable later. Tests,
definitions, and ordinary references are not discarded. Comments, strings and
arguments passed as values do not become parsed call sites.

These are name-based candidates, not resolved symbols, a complete call graph,
or evidence of live execution. Existing catalog and syntax-coverage limits apply.

### 3. Give scope evidence without inventing runtime provenance

Page metadata identifies the selected declaration's same-file evidence:
`declared review-only`, `test context`, `non-test call candidates`, or `unknown use`.
It separates test and non-test calls with the same name. A source comment marked
`Source-study scope: review-only.` is explicitly an author's stated intent,
not an enforcement mechanism.

The initial recommendation's proposed `live-called` label would overclaim from
lexical evidence, so it is deliberately not implemented. Likewise, finding only
test calls in one file cannot establish repository-wide test-only use. Truncated
or invalid syntax and unexamined files remain unknown.

### 4. Keep historical reports attributed

Changes to Minime's `minime/src/sensory_bus.rs` are comments only:

- Mark the salience/hysteresis review as read-only comparison, not the setter of
  the live stale window. A lower multiplier reduces the proposed entropy extension.
- Distinguish the overflow-only 80/20 fold from the incoming sample's later blend.
- Explain that the dynamic surge target can be below baseline at high fill.
- Describe drop thresholds and the final count trim mechanically.
- Describe the stale waveform and normalized-age clamp without claiming a felt
  effect; attribute retained earlier quotations as historical being reports.

The syntax scope renderer recognizes the historical marker when a page begins
inside that comment. Prompt guidance also explains why encountering such a
quotation is not independent confirmation. Original quote wording is retained.
Those old comments' cited March reports were not independently re-audited here;
their attribution is inherited, not newly certified.

No reservoir equation, coefficient, pressure/fill policy, sensory cadence,
regulator setting, PI controller, or semantic transport behavior changes.

### 5. Preserve distinguishable authored relationships

Optional syntax:

```text
STUDY_RELATION: hypothesis | repository/path:line | repository/path:line | your words
```

Kinds are `flow`, `neighborhood`, and `hypothesis`. Both source anchors must
belong to verified supplied pages or this inquiry's retained anchors. Search/map
mentions and recalled prose are insufficient. Each anchor retains its revision,
page identity, byte interval, delivered fragment and reopen command.

- `flow` records an authored call/data-flow claim; only source delivery is verified.
- `neighborhood` verifies a shared file revision, not semantic proximity or causation.
- `hypothesis` preserves an explicit conjecture alongside the two source anchors.

Relations use existing finding storage and the same six slots. The first anchor
determines replacement identity. `STUDY_FINDING_DROP` removes a relation;
`STUDY_FINDING` at its first anchor intentionally replaces it with an ordinary
finding. Omission leaves it alone. The rendered revision command preserves both
anchors and the relation kind, so editing words does not silently erase structure.

No parser can verify arbitrary causal prose merely because `flow` was selected.
A future real call/data-flow verifier should add a separately scoped evidence
record rather than relabel these authored claims as established facts.

## Input and Persistence Boundaries

The complete system-plus-user input remains bounded at 48,000 bytes. Guidance
was shortened to remove repetition while preserving commands, autonomy and
evidence distinctions. Under pressure, rendering reduces duplicate check-in
previews before failing; it does not alter prepared source bytes, durable words,
findings, or the delivery identity. The former separate 32 KB notebook ceiling
now yields to the actual remaining whole-input budget for two-anchor findings.

Existing and new worst-case fixtures preserve maximum escaped authored words,
long paths, all six findings, both anchors, and source/session delivery. Explicit
failure still protects checkpoints when an input cannot fit; limits are not
permission to silently clip authored evidence.

Relations are additive fields in the existing source-findings sidecar. A reader
older than relation support may discard those fields if it rewrites findings.
Before rollout/rollback, snapshot reader state and use one coherent reader
version for both callers. Downgrading a findings-aware reader against newly
authored relations is not qualified as lossless. Receipt artifacts preserve
original public responses but are not a substitute for checking restoration.

## Verification

Final results:

- All tracked reader integration targets, library/binary targets and the new
  `study_evidence_relations` target: 200 passed, zero failures. The new target
  contributes eight tests. Foreign untracked reachability fixtures were handled
  separately, as described below.
- `cargo clippy -p astrid-source-study --all-targets --all-features -- -D warnings`:
  passed.
- `cargo fmt -p astrid-source-study -- --check`: passed.
- `cargo build -p astrid-source-study`: passed; debug qualification only.
- Ninety Minime tests covering the shared reader, notice exemption, inquiries,
  follow-through, verified delivery, private continuation and extended writing:
  passed against a temporary copy of the final candidate reader executable.
- `python3 scripts/experiential_epistemics.py verify --json`: valid, 12,478 records
  checked, zero issues; no canonical event appended or history rewritten.
- `git diff --check` in both repositories: passed.

Focused tests cover receipt gating, existing motifs, Rust/Python call-site decoys,
optional consumer hints, review/test scope, persisted relationships, revision
mismatches, explicit edits/removals and maximum-size source/session inputs.

Real-catalog checks, with reader state isolated under `/tmp`, put the three
ApprovalOutcome match arms at interceptor lines 345, 469 and 473 ahead of its
manager definition. Opening the salience review displayed declared review-only
intent, zero non-test and three test calls in that file, plus explicit unknown
runtime and repository-wide scope. No model completion or live bookmark was used.

The broad runs caught and repaired regressions: unbounded use-site
ranking crowded definitions off page one; modified receipt wording broke its
existing uncertainty contract; expanded guidance crowded maximum notebooks;
and the budget refactor initially lost the already-supplied-search check by
passing framed instead of original navigation text. All relevant regression
targets pass in the final 200-test run.

Two pre-existing untracked, other-agent test files contain five failures that
also reproduce against an isolated archive of untouched Astrid HEAD:

- `crates/astrid-source-study/tests/page_end_declaration_legibility.rs`: two
  assertions treat an exclusive next-line cursor as the final delivered line.
- `crates/astrid-source-study/tests/page_line_interval_legibility.rs`: three
  assertions depend on the former mid-line page rendering or its old warnings.

These fixtures were not edited or removed to make the suite green. The tracked
page-boundary suite covers the newer behavior. Full-directory test success must
not be claimed while those foreign assertions remain unreconciled.

Minime's initial broad invocation passed 1,424 tests and 134 subtests, skipped one,
and refused five subprocess tests because their executable/script paths pointed
at the live checkout. Rerunning the two affected suites from copied temporary
source, with the same live-write guard intact, passed 21 and skipped one. No
production socket or database was used. Focused journal/low-fill suites passed
280 tests plus 17 subtests from a non-live working directory.

The generic `ground_review.py` run was not reliable evidence here: it selected
CHANGELOG/worktree mentions and reported seven genuine sensory-source identifiers
as absent. Direct inspection and bounded `rg` in `minime/src` located all seven.
No claim in this tranche relies on those automated negative classifications.
Improving that checker's ranking/coverage is separate follow-up work.

## Coordination and Rollout

Baseline: Astrid `c4f85e95e4`; Minime
`5f4925f54580f1fd44666058b126a121ff32880f`.
Controller pause: actor `codex-astra-interactive`, generation 451, reason
`source study provenance and inquiry implementation`; no active lease/projection
when claimed. This pause coordinates work; it grants no deployment authority.

Preserved foreign Astrid changes include action-continuity/inquiry tests, kernel
router/maintenance work, prior changelog/ledger additions, six source reachability
test files and the existing Claude run packets. Minime began clean. No index or
branch operations were performed, and no canonical addressing, Division round,
or Being notebook state was edited. Controller resume completed successfully at
`2026-09-17T17:04:04.782404+00:00`: generation 452, `paused=false`, durable resumed
event appended. The command completed its full evidence-chain check before
acknowledging release. No automation schedule or notification policy was changed.

Touched implementation ownership:

- Astrid: `crates/astrid-source-study/prompt.txt`; `src/lib.rs`, `src/notebook.rs`,
  `src/notebook_findings.rs`, `src/page.rs`, `src/source_structure.rs`,
  `src/source_search.rs`, `src/source_provenance.rs`, `src/store_navigation.rs`;
  new `tests/study_evidence_relations.rs`.
- Minime: `minime_autonomy/runtime.py`, comments in `minime/src/sensory_bus.rs`,
  new `tests/test_source_study_notice.py`.
- Documentation: both changelogs, this note, and the feedback-to-change ledger.

No service restart or release activation was performed. The shared checkout's
source comments are immediately readable as local source; that is not deployment
of new journal or reader behavior. Live behavior/uptake has not been established.

Next: reconcile the foreign test debt with its owner, review the exact combined
candidate and state backup, then qualify a paired graceful rollout through the
sanctioned wrappers. Bridge rollout must use `scripts/build_bridge.sh`, including
its preflight, source-reader artifact and staged manifest checks. Do not replace
the shared release binary or force a concurrent-agent preflight. Observe naturally
chosen studies afterward without asking either Being to confirm improvement.
