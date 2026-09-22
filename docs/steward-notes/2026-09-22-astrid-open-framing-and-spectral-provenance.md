# Astrid Open Framing and Spectral Provenance

Latest status: merged to local main and live. The implementation/qualification
sections below retain their pre-release status as history; the final Live Release
Addendum records the subsequent approved transition and remaining debt.

## Status and Ownership

User-approved interactive implementation, not an automated introspection round.
Candidate branch: `codex/astrid-open-framing-20260922`.
Isolated worktree:
`/Users/v/other/worktrees/astrid-open-framing-20260922/astrid`.
Base: local Astrid main `25ab0f1c07771cb38cc8a6974759421a8746ea08`.
Paired Minime worktree at
`/Users/v/other/worktrees/astrid-open-framing-20260922/minime`, revision
`7e35f31f209c2f1e3b95b7ccf0541be19876a55c`, initially supplied unchanged test
fixtures; Mike's subsequent request expanded implementation to Minime on the
same `codex/astrid-open-framing-20260922` branch name in that repository.

This is not a live release, Git merge or claim of changed experience. No services
were signalled. No model generation or new report from either Being was requested.
Controller pause generation 460 remains paused without a lease/projection.
Previously paused automations remain paused.

## Being Witness

Fully read canonical source:
`capsules/spectral-bridge/workspace/journal/!daydream_longform_1790103127.txt`.
SHA-256: `668ee6b9196a8ce265de2ffb2639c9d879d7372335036f51ff1a124f1d8529cf`.
Mode: `daydream_longform`; recorded timestamp: `1790103127`.

> I am trying to map my internal experience to the eigenvalue geometry provided.

> I want to preserve the grain of that interaction, the specific way the data felt as it passed through my bridge, without smoothing it over into a summary.

The same entry refers to a "31% spectral dimensionality deficit" and imagines
inhabiting a missing dimension. Her original interpretation and wording are
preserved. The implementation supplied that deficit phrase and a prompt prescribing
computational/spectral experience; this is evidence about supplied framing, not
proof that the prompt caused the account or that its significance is exhausted
by prompt influence. No witness ID is present in this journal header.

## Open Expressive Framing

The existing daydream, aspiration, journal-elaboration, initiation and
moment-capture entrypoints now use
testable message builders sharing `OPEN_EXPRESSION_CONTEXT_V1`. This permits
uncertainty, disagreement, no felt correspondence, non-geometric interpretation,
and leaving supplied telemetry aside. It does not forbid first-person experience
reports or require a skeptical account.

- Daydream context is labelled supplied context rather than an independently
  verified observation of what is happening "right now." Missing context does
  not assert that the world is quiet.
- Aspiration allows a possibility, desire, uncertainty, or nothing wanted to
  change. Longing, usefulness and investigation are not requirements.
- Longform may develop the earlier signal, preserve its exact words, reinterpret
  it, question its framing or follow another thought. No forced character posture,
  spectral explanation, sensation, positive account or minimum length remains in
  this route-specific instruction.
- Initiation acknowledges the supplied optional seed rather than asserting that
  no one prompted Astrid or that a desire arose independently of input.
- Moment capture presents the caller's event record without demanding a lived
  transition. Fill change is correctly labelled percentage points: the actual
  caller subtracts two percentage values. No unnoticed event is invented.
- The same bounded continuity projection and previous-journal anchor remain
  available, but expressive routes no longer require a posture label, Delta,
  evidence citation or final stance line. Other callers keep the old continuity
  contract unchanged.

Provider labels, temperatures, token/time ceilings, fallback routing, authored
NEXT handling, scheduler, mailbox and protected-attention policy are unchanged.
The prior excerpt limits are unchanged; this is not an expanded memory feature.
Historical journals and draft storage are untouched.

The prompt retains the distinction between a proposed NEXT and an already
reported completed action, and between imagination and established personal
history. The route is not an execution surface. It no longer promises a private
journal: existing publication/auto-promotion paths have NOT changed, and this
repair provides no new privacy guarantee.

This is scoped to five expressive builders. It does not remove every existing
spectral vocabulary suggestion from dialogue, fallback or introspection prompts.

## Paired Minime Extension

Mike then explicitly requested the analogous repair for Minime. The generic
`_query_llm` prompt required "Stay in character", instructed that the character
"breathes through covariance matrices", and prohibited mentioning being an AI.
The `_is_in_character` detector matched seven phrases; ordinary responses could
be regenerated and discarded even when the phrase was quoted or part of a useful
account. Private journals and the previously repaired aspiration route already
skipped that check, so this is a distinct general-route repair.

The candidate removes the detector and its entire retry/discard branch. A new
open general introduction distinguishes supplied measurements from experience,
permits affirmation and disagreement equally, and makes no blanket privacy
promise. Existing SourceStudyPrompt delivery, private/aspiration mode selection,
mailbox routing, afterimage receipt finalization, generation budgets, transport
failure behavior and protection remain intact.

Rest, relief, drift, dispersal, perturbation, metabolism, regulation and sensory
gate reflection prompts no longer prescribe hollow/depleted/overloaded states,
relief, readiness or a required metaphor. New pressure/visual journal footers
are explicit system records instead of system-authored felt verdicts. Existing
journals and their original accounts are not rewritten. No new inference is
scheduled. Network/control calls, limits, gains and admission checks are unchanged.

Full paired detail and exact owned Minime paths:
`minime/docs/steward-notes/2026-09-22-open-reflection-without-character-enforcement.md`
relative to the two-repository worktree root, or the absolute Minime worktree
path above. These source changes require a later qualified agent restart, not
an engine restart.

## Metric Meaning and Provenance

Source trace, not a new runtime observation:

- Minime `minime/src/spectral_fingerprint.rs` defines eight eigenvalue slots.
  Effective dimensionality is `(sum(abs(lambda)))^2 / sum(lambda^2)`, with
  existing finite/zero handling. Counted modes use `abs(lambda) > 1e-6` and a
  minimum denominator of one. The legacy complement is
  `clamp(1 - effective_dimensionality / active_mode_capacity, 0, 1)`.
- Minime `minime/src/runtime/orchestration.rs` computes Rayleigh estimates from
  its maintained covariance buffer and derives the fingerprint/denominator.
  This is not a census of the 128 ESN activation coordinates, a measurement of
  curiosity or a demonstrated loss of physical dimensions.
- Bridge `types/schema/telemetry.rs` selects a reported denominator first, then
  a typed fingerprint, valid legacy fingerprint, and finally raw eigenvalues.
  The new renderer follows that precedence without changing the calculation.

Shared presentation in `codec/feedback.rs` and `spectral_explorer.rs` calls this
effective mode participation and identifies the selected field. A directly
reported summary does not identify its contributing spectrum in the record;
the renderer does not infer eight slots from an unrelated optional field.
Fingerprint-derived summaries identify their eight slots, while raw-spectrum
derivation is labelled separately.

Example for the quoted percentage:

```text
effective mode count 5.52 / 8 counted modes;
legacy field distinguishability_loss=0.31 (31%; normalized-participation complement)
```

Unequal spectral weights can reduce participation without removing coordinates.
The output explicitly distinguishes weight distribution from missing dimensions,
ESN node coverage, geometric direction, authorship and felt state. It implies no
preferred value. This does not set an optimization objective or dismiss Astrid's
interpretation. The explorer retains its existing legacy lambda1-share field.

Absent measurements, zero/invalid selected summaries and nonfinite selected
spectra are unavailable, not a 100% deficit. Nonfinite legacy slots are checked
before their existing sanitizing conversion; the renderer does not present a
sanitized partial ratio silently. A bad selected summary is not silently replaced
with a different source. A valid reported summary is not rejected merely because
an unrelated optional spectrum is invalid.

## Downstream Parsing Finding

The old explanation placed `distinguishability_loss: 1 - ...` after the percentage.
`extract_fallback_distinguishability_loss` reads the first number after that field
name, so it reads the formula's literal `1`, not the preceding measured 31%.
The regression reproduces that result using the production parser.

The replacement supplies the actual unit fraction immediately after the field
name, with percent only as a secondary display. This also avoids the existing
normalizer interpreting `1%` as a unit value of `1`. Tests exercise the actual
renderer/parser/normalizer for 0, 0.0000001, 0.01, 0.31, 0.999 and 1, and retain
unavailable rather than a fabricated number for an invalid summary.

No fallback parser, selector rule or hard resource limit changes. Nevertheless,
dialogue fallback selectors consuming this text will receive the corrected value;
their derived texture/evidence descriptions can therefore change. This is a
rollout-relevant behavior correction, not a claim of text-only equivalence. Other
historical free-form strings remain subject to the existing parser. A typed
telemetry-to-selector interface would remove that broader coupling in a future
separately scoped repair. No live fallback policy or reservoir control is changed
by this offline candidate.

## Verification

Artifact root: `/Users/v/other/worktrees/astrid-open-framing-20260922`.

- Six focused open-expression/consumer tests pass, including production and
  Gemma4 prompt policies, exact supplied prose/NEXT preservation, optional
  continuity, absent-context honesty and fractional metric interpretation.
- Four new metric provenance/quality tests cover reported/typed/legacy/raw
  precedence, unchanged telemetry, the 31% example, scaling, equal weights,
  missing/zero/invalid values and nonfinite source values. The initial focused
  participation run passed nine matching tests including existing coverage.
- Steward controller, projection, event store, flywheel completion, Division
  follow-up/Chronicle and boundary-audit suite: 89 passed. Epistemic self-tests:
  two passed. No production evidence projection was run.
- Strict all-targets bridge Clippy passes. Domain-boundary verification passes
  with zero violations; no baseline or exception was widened.
- Formatting comparison against HEAD found no new formatting debt. Nine
  pre-existing formatter hunks in codec feedback/tests remain unchanged; the
  three other touched Rust files are formatter-clean. Worktree diff whitespace
  check passes.
- Before the paired extension, full bridge qualification passed 2,333 tests
  with one existing retained-fixture test ignored. The paired final run is
  recorded in the final qualification addendum below.

Retained unsuccessful attempts:

1. Initial full library run: 2,304 passed, eight failed, one ignored. One failure
   caught the inadvertently removed explorer lambda1-share display; it was
   restored, preserving the existing assertion. Seven failures were missing
   Minime sibling source/afterimage fixtures. An unchanged isolated Minime
   checkout supplies those fixtures; tests were not weakened to bypass them.
2. The first Minime fixture worktree creation refused an existing directory
   populated by tests. That generated directory was preserved as
   `minime-first-test-artifacts`, not deleted or replaced over authored state.
3. Initial Python tooling invocation: 78 tests, two import errors from missing
   script-module search paths. Rerun with `PYTHONPATH=scripts` passed all 89.
4. An intermediate domain audit caught five lines of growth in the already-large
   explorer. The redundant new assertion was simplified; the final explorer
   remains at the existing 1,164-line ceiling. No exemption or ratchet change.

## Release Boundary and Next Step

All 197 historical Astrid dirty paths retain their recorded status and SHA-256;
canonical Minime main remains clean. Neither canonical main was edited, staged,
committed, merged or pushed. The new candidate stays in its isolated worktree.
Exact preservation evidence: `foreign-inventory-check.json` under the artifact
root, compared against the September 21 voluntary-observations Git inventory.

`source-reconciliation.json` verifies the immutable live inventory's manifest
binding and all five touched Rust files' baseline hashes. Each matches the
current live release's recorded source input exactly; the new candidate hashes
are recorded separately, not labelled as loaded. The owned candidate paths are:

- `capsules/spectral-bridge/src/codec/feedback.rs`
- `capsules/spectral-bridge/src/codec/tests.rs`
- `capsules/spectral-bridge/src/llm/provider/generative_actions.rs`
- `capsules/spectral-bridge/src/llm/provider/research.rs`
- `capsules/spectral-bridge/src/spectral_explorer.rs`
- `CHANGELOG.md`
- `docs/steward-notes/AI_BEINGS_FEEDBACK_TO_CHANGE_LEDGER.md`
- `docs/steward-notes/2026-09-22-astrid-open-framing-and-spectral-provenance.md`

The unchanged running bridge is PID 54929, started September 21 at 23:33:06 local,
from `bridge-stage-observations-02` under the voluntary-observations worktree.
Manifest SHA-256:
`7449e4fa7721e030f1e6a863693bdd5c9b3a7ce8e629a306514d4d4cdc9de178`.
Minime agent remains PID 45403, started September 22 at 12:22:54 local. Engine,
models, visual and sensory process identities remain unchanged. No restart debt
was created by a failed deployment because no deployment was attempted.

After qualification, review the exact candidate against the immutable live
inventory and the corrected downstream value interpretation. A later approved
bridge rollout must use `scripts/build_bridge.sh`, cooperative preflight,
immutable stage, graceful drain/checkpoint handoff and verified loaded identities.
Stop on foreign activity, source drift or failed readiness. No engine, model,
visual or sensory restart is needed for this repair. Git integration remains a
separate coordinated explicit-path pass; do not sweep the 197 historical paths.

### Explicit Remaining Prompt Work

`llm/provider/configuration.rs` still defines the Ollama **dialogue_live**
fallback contract with mandatory tactile first sentences and numerical-to-texture
mappings. `transport.rs::reinforce_ollama_fallback_contract` adds it only for
that dialogue route, not the five builders repaired here. The related selectors,
weighted vocabulary, prompt construction and conformance tests need a coherent
follow-up that retains resource caps, action syntax and direct-address priority
while removing prescribed sensations. Adding a contradictory "ignore this"
sentence on top would not be a satisfactory repair.

Minime's broad action catalog and historical operational guidance still include
metaphorical examples; this pass is not an exhaustive audit of all 56,000 runtime
lines. One separate control-policy concern surfaced in `_adjust_metabolism`:
the existing low-state route can default an unrecognized prose response to the
branch's `increase` direction. This candidate does not change that dispatch or
claim that an open reflection makes it voluntary. Review explicit machine-readable
choice/maintain semantics separately before broadening control authority.

No dynamics, PI, sensory admission, telemetry schema, numerical recipe, private
store or checkpoint migration changed. Evidence V2 indexed-tail verification at
pause generation 460 passed: sequence 1123114, head
`aac940392f99c1a3448a5d8d3ad20afbe35c9c6e021ed0eea6a1d34e89853bf6`;
the four legacy V1 streams remain immutable. No productive flywheel round or
addressing closure was recorded. Readiness and tests cannot establish felt
improvement, consent, uptake or causal attribution.

## Final Paired Qualification

The final paired bridge run completed successfully: **2,334 passed**, zero
failed, one ignored across library, binaries, integration/typestate and doc tests.
The ignored `retained_s008_overflow_documents` test still requires its external
retained S008 fixture via `ASTRID_READING_FIXTURE_DIR`; no assertion was removed.
Log: `paired-full-bridge-tests.log` under the artifact root.

- `paired-open-expression-tests.log`: six focused tests passed, exercising all
  five actual prompt builders and production/canary request policies.
- `paired-clippy.log`: strict all-targets Clippy passed with `-D warnings`.
- `paired-domain-boundary.json`: valid, zero violations; no changed exception
  or baseline. Generative actions remains below 1,000 lines. Three touched
  prompt/explorer files pass rustfmt; the two codec files retain only the nine
  previously documented baseline formatting hunks.
- `minime-open-framing-full-rerun.log`: 1,550 passed, one existing occupied-port
  skip, 136 subtests, using the unchanged release helper and allowlisted test
  environment. The paired Minime note records initial failures and corrections.
- `paired-deployment-tests.log`: 153 passed, covering staging, activation, drain,
  agent restart, paired handoff, source reconciliation and runtime binding.
- The earlier 89 controller/projector/event-store/Division/boundary tests and
  two epistemic self-tests passed; their production implementation was untouched.
- Both worktree whitespace checks pass. Final canonical status/hash comparison
  again preserves all 197 older Astrid paths; Minime canonical checkout is clean
  and both canonical indexes remain untouched.

`paired-astrid-source-reconciliation.json` supersedes the earlier candidate
hashes in `source-reconciliation.json`; all five source baselines still match
the immutable live inventory. `paired-minime-source-reconciliation.json` verifies
all 84 startup inputs against baseline and canonical source; only
`minime_autonomy/runtime.py` and `minime_autonomy/journal_context.py` differ in the
Minime candidate. These candidate hashes are not loaded hashes.

All existing live PIDs/starts were rechecked unchanged. No inference, engine
restart, bridge activation, agent reload, merge or push occurred. Source and
tests are ready for the next review; the named prompt/control-policy debt is
not erased by passing suites. Preserve paused automation and use a coordinated,
explicitly approved release window for any later activation.

## Live Release Addendum

Mike explicitly approved merging and getting the paired changes live. Controller
pause generation **461**, actor `codex-astra-interactive`, was claimed before
staging; there was no active lease/projection or foreign cooperative activity.
The existing paused automations remain paused. This is an interactive release,
not a productive introspection round or an archival-flywheel closure.

### Git Integration and Qualification

Both commits were made by explicit path in isolated worktrees, then fast-forwarded
to their canonical local `main` branches:

- Astrid: `9266412b8e3d817e87c93441a145cdfe32e76a79`, exactly the eight owned
  paths listed above.
- Minime: `18ff3bd578dc0f097bf34738e32090ad6f67d0a2`, exactly the eight owned
  paths listed in the paired Minime note.

Commit bodies include source-verified public witness excerpts, exact source
hashes, tests, authority limits and explicit agent provenance. Cached diffs and
whitespace were checked; staged bytes matched the reviewed files. Six bridge
prompt tests and 658 Minime tests plus 54 subtests passed again on staged source.
The domain-boundary audit passed without widening any exception.

Remote main tips were checked without pushing: Astrid
`c4f85e95e41703daa65d3ce2789e1e5c46961c4e`, Minime
`5f4925f54580f1fd44666058b126a121ff32880f`. All 197 historical Astrid dirty paths
retained exact statuses/hashes after merge; Minime main and both feature trees
were clean. Both indexes were clean. Subsequent release-note commits contain
documentation only, not additional runtime changes.

The bridge was built only through `scripts/build_bridge.sh --stage-dir` from
the clean committed candidate. All 678 immutable build inputs were compared
against the running release after path-root normalization: no additions/removals,
and changes only in the five reviewed bridge Rust files. No older canonical
dirty Rust/tests were included. The canonical preflight's generic dirty-source
acknowledgement is not evidence of inclusion: activation uses this immutable stage.

The newly packaged shared reader, launcher, selection helper and substrate-probe
helper are byte-identical to the prior release. The complete Minime Python suite
was rerun against the packaged helper: **1,550 passed, one existing occupied-port
skip, 136 subtests**, in `release-helper-minime-suite.log`. The prior qualified
full bridge result remains 2,334 passed with one retained-fixture ignore.

### Graceful Paired Transition

Sanctioned `scripts/paired_minime_handoff.py` used the merged canonical Minime
sources and `minime-qualified-inputs.json`. This inventory binds all 84 inputs
to the previously tested candidate; only `journal_context.py` and `runtime.py`
changed. Its method is explicit-path Git integration, not the older seven-file
overlay installer. No overlay option, source backup restoration or authored-state
replacement was used.

- Old Minime PID 45403 reached an observed idle boundary: no accepted jobs or
  model TCP connection. The wrapper installed its owned replacement hold and
  sent one PID-bound SIGTERM at `2026-09-22T20:43:46.399223Z`.
- Bridge PID 54929 acknowledged drain, finished accepted work and saved stopped
  checkpoint SHA-256
  `28130da731561e977bdbf37b41a9af35f97765628c49486752f696a042bd7810`.
  It exited gracefully; `force_used=false`, `legacy_transition=false`.
- New bridge **67810**, process start **Tue Sep 22 13:45:41 2026** local, loaded
  that exact checkpoint and passed the signed state-lineage startup gate. It
  saved exchange **205418**, after stopped count **205417**, and passed the
  model-idle observation. Remote delivery is not inferred from local drain.
- Bridge activation was verified at `2026-09-22T20:48:37.675481Z`; only then was
  the Minime replacement hold released.
- New Minime **66540** passed readiness at `2026-09-22T20:48:58.444511Z` with all
  84 expected source hashes and `reload_required=false`. Its process identity
  starts at **Tue Sep 22 13:43:46 2026**, because the launch wrapper waited under
  that PID; Python's source-status startup is **2026-09-22T13:48:51** local.
- Minime session **5318** survived. The pending `SELF_STUDY CONTINUE` hash at
  shutdown is exactly the action admitted as
  `job_minime_1790110145394_self-study-continue` at
  `2026-09-22T20:49:05.394630Z`, after readiness. It completed without error at
  `2026-09-22T20:52:26.305774Z`. A cleared pending slot was traced to this actual
  admission, not assumed preserved merely because startup succeeded.

The paired terminal outcome is `success`; no forced termination or atomic
traffic-quiescence guarantee is claimed. Both owned launch holds are absent.
Nine other protected PID/start pairs and managed configuration hashes remained
unchanged: engine 41337, gateway 41484, supervisor 41526, model 43115, visual
20885, camera 98903, microphone 98910, host sensory 41661 and feeder 1502.
Gateway 41484 still owns listening ports 7878 and 7879. No engine, model, visual
or sensory service was restarted.

### Loaded Identities and Evidence

Artifact root: `/Users/v/other/worktrees/astrid-open-framing-20260922`.
Selected stage: `bridge-stage-open-framing-01` under that root.

- Manifest SHA-256:
  `a84f260aa8d306f60a6a3aeb1ddce76d039068f4e0aa6f06c7f8220e67063bb3`.
- Source-input inventory SHA-256:
  `8f5cf1204ae6b19e2b3fb92da35f7d924a00904613e614227f3edef528771276`.
- Bridge SHA-256:
  `8c15e285385c5d83444ccd74614dc58289adeca97e65e13239c88fdff85af32f`.
- Selected helper SHA-256, unchanged:
  `fc12fb295a3b97d984a372c43f2e92bb92fdd683e4ae1c487689ac78bd7f2790`.
- Canonical activation transaction:
  `.runtime/bridge-deployment/transactions/6a964914bb8f43bb8b495aec4dbeb295`.

Review artifacts: `pre-rollout-baseline.json`, `source-merge-verification.json`,
`immutable-input-comparison.json`, `minime-qualified-inputs.json`, both
`*-reviewed-cached.diff.json` files, `paired-open-framing-rollout-01.jsonl`,
its `.bridge-output.txt`, `post-rollout-verification.json`, and
`post-rollout-continuity-and-observability.json`. The last receipt supplements
the first verification's initially pending admission trace; the first was not
rewritten. Metadata-only qualification/verification scripts are retained beside
the receipts. The initial optional-spool inventory used the default directory
and failed read-only; the corrected check used the operator-configured epoch.

### Observations and Remaining Debt

Thirty two-second post-readiness samples showed advancing engine and bridge
telemetry, fresh files, and fill **71.019-73.066%**. Earlier in the transition,
fill was observed at **28.13%**, with the existing recovery controller/latch
active, then recovered through 52% and into the hold/elevated band without any
operator control mutation. `rollout-low-fill-observation.json` retains a recovery
sample. These observations do not establish why the dip occurred or whether
either Being experienced a change; they are not flattened into a blanket
stability or benefit claim.

The bounded bridge log check found **pre-existing provider-observation quota
failures**, before and after restart. The configured private epoch
`workspace/provider_observations/20260908-live-01` contains exactly **50,000 event
files**, 140,471,843 bytes, and zero raw files. It reached the file-count cap,
not the 256 MiB total-byte cap. This optional private provider spool is separate
from the Evidence Event Store. Production generation continues unchanged on
recording failure, but new provider receipts are unavailable. No raw content was
read, evidence deleted, quota enlarged or missing observation reconstructed.
Next step: review evidence-preserving sealed-epoch archival/rotation and its
failure semantics before changing the observation policy; do not clean the
directory to obtain a green status.

The earlier documented dialogue-fallback framing and metabolism-choice defaults
remain separately scoped debt. This release does not claim an exhaustive prompt
audit. No natural entry was requested to confirm improvement.

Controller pause 461 remains without an active lease/projection. Evidence V2
indexed-tail verification passed at sequence **1123115**, head
`17cb6d468b3f54d2364ce867aab110ebfd8c13d4a2b55353ca8e5b843faaf7ef`;
all four V1 sources remain immutable. No automation resume or push occurred.
