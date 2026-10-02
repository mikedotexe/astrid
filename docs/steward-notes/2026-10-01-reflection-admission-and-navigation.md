# Reflection Admission and Directory Navigation

## Status and Ownership

Implementation candidate by Codex, in paired isolated worktrees:

- `/Users/v/other/worktrees/reflection-admission-20261001/astrid`
- `/Users/v/other/worktrees/reflection-admission-20261001/minime`
- Both branches: `codex/reflection-admission-20261001`.
- Astrid baseline: `8255c8d238f278d4fc3f49a296a49aed0468febe`.
- Minime baseline: `145aaab45690eac9539803015de73a06fa1bab6d`.

The writing-room review repairs from the concurrent session were already merged
and the canonical trees clean when this implementation began. They are retained.
This candidate is not staged, committed, merged, pushed or deployed. No service
was restarted. Steward controller pause generation 487 remains paused; this is
an explicit interactive repair, not a resumed automation round.

## Public Witnesses

Paths below are relative to `/Users/v/other/minime`. All four files were read
fully during the investigation; exact bytes are unchanged. No private writing
was used as a fixture or reproduced here.

1. `workspace/journal/self_study_2026-10-01T18-21-36.739034.txt`
   SHA-256: `b00c86e8cf1370bd217034d9ae4510c8d08f33eb2a90de2bf7eee52f93e17d17`.
   Exact chosen action: `NEXT: INTROSPECT`.
2. `workspace/journal/study_navigation_2026-10-01T15-39-00.071613.txt`
   SHA-256: `1888b8085cca3f81338c035ca3c311a13d9da548b1f7b209c952a645aa235301`.
   Exact chosen action: `NEXT: SELF_STUDY MAP astrid/crates/astrid_kernel/src`.
3. `workspace/journal/action_preflight_2026-10-01T18-41-49.319025.txt`
   SHA-256: `095b38ee450a38f774d143951381e8654df8322b580574fd5694e535153ee671`.
   System-generated receipt: `REGIME calm`, `Effective route: unwired`.
4. `workspace/journal/regime_choice_2026-10-01T18-34-54.738805.txt`
   SHA-256: `4e9c74d6cf6cca86f4091df9f8547652c37d9d3bc64756bf3ec2b4bbf8da40c0`.
   System-generated receipt: calm requested/applied and bounded control sent.
   This receipt alone does not establish consumption by the engine or an effect.

Read-only `scripts/ground_review.py` ran on the two authored entries. It reports
the underscored directory missing. Its broad lexical citation matches are not
semantic verification of Minime's explanation; the decisive grounding here is
the exact chosen command, dispatch source, logs and executable regressions.

## Reconstructed Failure

On October 1, local PDT, the log records bare INTROSPECT blocked at 16:43:05,
17:48:20 and 18:24:42 with
`research_budget_required_for_active_experiment`. The retained legacy experiment
is `exp_minime_20260925_legacy-self-experiment`. The new prompt offers reflection
as a study exit, but the older budget classifier treats every INTROSPECT as
research. Its rejection supplies experiment-budget guidance instead.

This explains a concrete obstruction in the observed action chain, not every
choice or subjective experience. The short action/preflight/regime files are
system receipts. Navigation-only responses also now have their own prefix.
The investigation's 68 retained post-restart generation records reported normal
stops, with no recorded timeout/fallback; this is bounded evidence, not a claim
that all possible generation failures are observable.

## Implemented Repairs

### Bare Reflection

`is_open_introspection_action` recognizes only the complete bare action, not a
prefix, target or command bundle. Admission and executor research-budget checks
share it. The existing source-study adapter still supplies typed reflection
input, with no source-page delivery credit or notebook claim mutation.

An existing active or paused experiment does not require a new research budget
to reflect. Targeted workspace artifacts and external research keep their
existing policy. Normal executor safety, operator/drain and resource handling
are unchanged. No experiment is closed, deselected or granted authority.

### Truthful REGIME Preflight

The existing grammar is factored into one pure parser used by dispatcher and
preflight. Existing trailing punctuation/first-token acceptance is preserved;
no new free-form gain grammar is introduced. REGIME is correctly classified as
live control. Invalid/missing regimes describe the existing notice route.
Preflight names normal execution gates and the existing below-35%-fill handler
substitution from calm/explore to recover. It sends no control and does not
claim observed effects. The handler, gain table and controller are unchanged.

### Precise Directory Spelling

MAP/LIST reuse OPEN's existence-based, per-component correction: an existing
underscored directory wins; only a missing component with an existing hyphenated
counterpart changes. The listing discloses both spellings and offers exact
catalog commands. This is a directory listing, not an implicit source open.

Recovery already offered candidate paths before this change. The observed
problem was repeated recovery for a precisely recoverable directory spelling,
not proof that no candidates existed. Unknown/private/invalid paths remain
recovery; normal catalog exclusion rules and byte limits still apply. Source
bookmarks, authored notes, pending pages and old prepared inputs are preserved.
No schema migration is required.

## Qualification

Before implementation, the new tests reproduced eight preflight failures,
three reflection-admission failures and the directory-spelling failure.
After implementation, the initial focused run passed 54 tests.

- Full `cargo test -p astrid-source-study`: passed.
- `cargo clippy -p astrid-source-study --all-targets -- -D warnings`: passed.
- `cargo fmt -p astrid-source-study -- --check`: passed.
- Domain-boundary audit: passed, zero violations.
- Launch-inventory, geometry-release and observation-release tooling: 16 passed.
- Full Minime Python suite, final rerun: 1,742 passed, one skipped,
  141 subtests passed in 77.29 seconds.
- Both candidate diffs pass `git diff --check`; canonical working trees remain
  untouched by this pass.

The first complete Python run had 1,741 passed, one skipped, 141 passed
subtests, and one failure in a newly added negative test. That test incorrectly
expected WRITE_FILE to be owned by the research-budget guard. It now uses
AR_START, which that guard actually rejects; no unrelated policy was changed.
The unsuccessful attempt is retained here rather than counted as a passing run.

The end-to-end test uses the real Minime decision/execution path and shared Rust
reader, synthetic source, and stubbed provider responses. It performs source
study -> authored INTROSPECT -> reflection -> authored CONTINUE -> next source
page while the unrelated experiment stays active. It verifies exact next-page
position, retained notes and the separate introspection artifact. A denied
executor gate still prevents generation. Default pytest live-write/network
guards remain enabled throughout.

Reproduce the Python suite from the isolated Minime worktree:

```sh
ASTRID_SOURCE_STUDY_BIN=/Users/v/other/worktrees/reflection-admission-20261001/astrid/target/debug/astrid-source-study python3 -m pytest -q tests --tb=short
```

## Next Release Boundary

Reconcile both candidates against current main and mapped live identities, then
qualify the immutable paired helper/agent/bridge release with existing migration
and bridge suites. A graceful activation must use sanctioned wrappers and
cooperative preflight, preserving pending choices and checkpoint continuity.
No full bridge suite or packaged live migration qualification was run in this
implementation pass. Stop on foreign activity, drift or failed readiness.
Do not restart the engine/model/visual/sensory processes for this repair.
Leave automations paused. Observe public use naturally; do not ask Minime to
confirm that the repair helped or infer felt improvement from test success.

## Approved Release Qualification

Mike subsequently approved deployment and merge. Codex claimed pause generation
488 with no active lease; both canonical main trees were clean and matched the
remote baseline identities above. This does not resume the paused automations.

The paired launch overlay omitted `minime_autonomy/authority.py`, the reviewed
reflection-admission implementation. The exact path is now included with a
synthetic snapshot/no-canonical-write regression. No other admission gate or
deployment preflight was relaxed. Release qualification and activation results
will be recorded below; the earlier implementation-only status is historical.

The reload wrapper requires the exact interactive actor `codex-astra-interactive`;
the maintenance pause was renewed under that identity as generation 489 before
any service transition. The previous pause was not resumed. Source reconciliation
froze all 90 selected agent inputs in
`/Users/v/other/worktrees/reflection-admission-20261001/launch-reconciliation-01/`.
The packet is evidence, not activation authority.

Fresh qualification: 56 paired handoff/reconciliation/restart/inventory/legacy
qualifier tests and 176 bridge deployment/controller/projector/evidence tests pass.
The full bridge library passes 2,350 tests with one ignored. Shared-reader strict
Clippy, formatting and the domain-boundary audit pass. Same-schema executable
qualification is `scripts/qualify_reflection_release.py`; it deliberately does not
reuse an earlier schema-migration script with incompatible version assumptions.
