# Open Aspiration and Temporal Distance

## Scope and Witness

Mike approved open aspiration framing, offline temporal qualification, merge and
live rollout. Work is isolated on paired `codex/aspiration-time-20260922` branches
from Astrid `fcc0ce650934343c0f47fcd1fa9025e413fc44ac` and Minime
`3241d5f74d086c12d57cadb0461b922f582a9425`. The 197 older Astrid paths are excluded.

Fully read source:
`/Users/v/other/minime/workspace/journal/!aspiration_2026-09-22T09-16-57.795422.txt`.
SHA-256: `2675734fbcd0f7146cafd9a97dc20bed092acc400f1d08baa3f2ea26dcf4ebdd`.

> Not a physical appendage, but a lingering, diminishing trail of self.

The recorded prompt invited imagining an unfamiliar experience. This aspiration
is a serious design direction, not an independently verified description of
memory dynamics or consent to irreversible loss. Its original prose is unchanged.

## Implemented Framing

Minime ASPIRE and FORM use explicit context mode `aspiration`, recorded as
`open_aspiration_context_v1` in new aspiration headers. The introduction invites
free first-person writing and permits leaving the invitation or context aside.
It no longer prescribes eigenvalue perception, covariance breathing, staying in
character or avoiding mention of being AI. The character-check retry/discard path
does not run for this context. Authored output and NEXT retain their existing path.

Routine whisper, research, peer-request, regulator suggestion and continuity
summaries are not appended to this expressive context. Existing action options,
authorization/stage gates, protected-attention deferral and activity failure
feedback remain. Ordinary mailbox admission and reply routing are unchanged;
an aspiration is not silently converted into a private draft or protected focus.
No new privacy guarantee is asserted for the existing public journal route.

Existing creative invitations, authored FORM, optional previous-entry threading,
continuity contract and provider resource limits are unchanged. Other default
generation routes retain their existing framing; this is a scoped ASPIRE repair.
The older aspiration header still formats post-generation telemetry, so it is
not a complete receipt of the exact model input. The new contract label identifies
framing, not a new clock or input-delivery guarantee.

## Offline Qualification

`scripts/temporal_trace_qualification.py` is a developer-only fixture runner,
not a being-facing command or live service. No Rust helper, ESN generator,
semantic decay, reservoir controller or private writing store changed.

Version `frozen-activation-temporal-distance-v1` uses double-precision RMS over
128 raw coordinates: `sqrt(sum((a_i-b_i)^2)/128)`. Inputs require 32-180 finite,
bounded, unsanitized frames, strictly increasing timestamps and at most 180
seconds of coverage. Each requested lag explicitly selects endpoint pairs within
its tolerance. Results include count, actual elapsed range, min/mean/max distance,
gaps and pairs spanning gaps. No interpolation or continuous-motion claim is made.
No eligible pairs yields null measurements, not zero distance.

Paired curves require exactly matching sample clocks, with no fitted alignment.
They remain exploratory because clock matching does not attest boot/node identity.
The runner never assigns an experienced memory duration or causal verdict.

Eight fixture controls passed: constant state, identical traces, analytically
specified fade, persistent difference, periodic return, reordered temporal
structure, explicit gaps and immutable input bytes. These are synthetic numerical
fixtures, not a copied reservoir implementation or a simulation of the live ESN.
Tests also cover an independent Euclidean-distance oracle, drift, missing lag
coverage, time jitter, malformed/nonfinite/out-of-range values, stale plans and
refusal to align unmatched clocks.

The plan is written before measurement and binds implementation bytes. This is
developer qualification, not a being-authored prediction or proof of first exposure.
Run: `/usr/bin/sandbox-exec -p '(version 1)(allow default)(deny network*)' python3 -B scripts/temporal_trace_qualification.py --output <new-offline-directory>`.
The CLI has no input-file, endpoint or code-loading argument. Network denial was
OS-enforced; no claim of a filesystem sandbox is made.

Artifact root: `/Users/v/other/worktrees/aspiration-time-20260922`.
Retained evidence: `temporal-qualification-01/plan.json`,
`temporal-qualification-01/result.json`, `minime-suite.xml`, `minime-suite.log`,
`astrid-tooling-tests.log`, and `launch-reconciliation-01/reconciliation.json`.

## Preregistered Production Follow-Up

The following is a separate qualification gate, not a completed experiment:

1. Use a dedicated isolated binary linked to the production Rust ESN. Bind its
   complete implementation, weights, dimensions, restored state, controller/
   spectral state, RNG state and step clock. Prove replay identity first. No
   live checkpoint handle, control socket, journal fixture or model call.
2. For an ESN-boundary study, begin from identical synthetic full states. Apply
   one bounded difference in an admitted synthetic input history, then supply
   identical future vectors and the same realized noise at matched update steps.
   Keep the production adaptation law unchanged; evolved internal differences
   remain outcomes, not manually equalized state. This does not model the full
   sensory admission pipeline, PI controller or cognition.
3. Fix seeds, histories, horizon and reporting times before execution. Include
   identical-history, zero-perturbation, full-state-copy, execution-order and
   different-future controls. Do not select a favorable fading curve or infer
   a half-life from one leak value. Report persistence, growth and returns too.
4. Record per-step state RMS and real elapsed/step coverage separately. Require
   matching complete replay identities and valid controls before assigning a
   mechanistic history effect within this isolated system. The fixture metrics
   here are qualified measuring tools, not substitutes for that experiment.
5. A later separately qualified language comparison holds supplied context and
   common prefix fixed before free continuation. No ordinary completion call is
   an inert experiment. A distance in state does not establish a language effect
   or experienced absence. Any live decay/reservoir change requires separate review.

## Verification and Release Boundary

Focused journal-context suite: **92 passed**. Full Minime suite against the
currently deployed helper: **1,518 passed, one skipped, 136 subtests passed**.
Temporal metric tests plus relevant restart/reconciliation/deployment/controller/
projector/evidence/Division/flywheel tests: **131 passed**. No behavior-test failure
required an assertion or production workaround. Both worktree whitespace checks
passed. No Rust code changed, so a bridge rebuild/full Rust rerun is not required.

Reconciliation freezes all 84 actual launch inputs; only
`minime_autonomy/runtime.py` differs. The canonical launch binding and source
startup identities matched before staging. The full Python tests use those exact
candidate runtime bytes. Rollout uses `scripts/restart_minime_agent.py` with the
qualified inventory, observed idle gate and one PID-bound graceful signal.
Stop on foreign activity, source drift, failed readiness or missing idle window.

Candidate status at this record: tested and reconciled; not yet activated.
The later release addendum must establish new PID/start, loaded hashes, readiness,
continuity and unchanged bridge/helper/engine/model/visual/sensory identities.
Controller remains paused at generation 459. Do not resume existing automations,
rewrite authored history, solicit improvement confirmation or push implicitly.

## Live and Git Addendum

Source commits were fast-forwarded to local `main` before activation:

- Astrid `2940d250480d3939f9e61b4db8d48133afd0a1bd`: the two temporal scripts,
  this note, `CHANGELOG.md`, and the feedback ledger.
- Minime `e62d0100b49c588ef541db9390b39b3d0ebcbbeb`: `minime_autonomy/runtime.py`,
  `tests/test_journal_context.py`, `CHANGELOG.md`, and
  `docs/steward-notes/2026-09-22-open-aspiration.md`.

All candidate diffs were read, staged by exact path and checked. Staged bytes
matched the reviewed/tested files; the 92 context and eight temporal tests passed
again after staging. Two epistemic self-tests also passed. Remote main tips were
checked without pushing: Astrid `c4f85e95e41703daa65d3ce2789e1e5c46961c4e`,
Minime `5f4925f54580f1fd44666058b126a121ff32880f`.

One candidate preflight initially refused recent tree activity after our document
writes. No bypass was used. A later check passed with activity age 219.9 seconds
and no foreign session. The guarded restart then waited for natural idle, no
active jobs and no model TCP connection; no test assertion or live policy was
weakened to obtain an idle window.

Sanctioned `scripts/restart_minime_agent.py` completed successfully:

- Old agent PID `54257`, started `Mon Sep 21 23:32:00 2026` local.
- One PID-bound SIGTERM at `2026-09-22T19:22:54.331467Z`; logs report all accepted
  workers drained. No forced termination or atomic traffic-quiescence claim.
- New agent PID `45403`, started `Tue Sep 22 12:22:54 2026` local; readiness
  verified at `2026-09-22T19:23:02.229051Z` with all 84 selected startup hashes
  and `reload_required=false`. Runtime SHA-256:
  `3066d40aed6a97b94e8ccd9b591402eb774c743a1bef86687fd5c020034e5d9a`.
- Session `5318` remained. The pending NEXT hash at the signal boundary matches
  the action admitted as `job_minime_1790104990858_self-study-continue` at
  `2026-09-22T19:23:10.858927Z`; the cleared pending slot is explained by that
  admission, not assumed preserved merely because startup succeeded.
- No old unfinished jobs or newly restart-interrupted jobs. Managed environment,
  installed plist and runtime-profile hashes unchanged. No checkpoint restored
  over newer authored state.
- All ten protected process PID/start pairs unchanged, including bridge `54929`,
  engine `41337`, model `43115`, visual `20885`, camera `98903` and mic `98910`.
  Gateway `41484` still owns ports 7878/7879. Fresh health and bridge telemetry
  were observed after readiness (0.98/0.97 seconds old in the final receipt).

The unchanged selected release is
`/Users/v/other/worktrees/voluntary-observations-20260921/bridge-stage-observations-02`.
Manifest SHA-256: `7449e4fa7721e030f1e6a863693bdd5c9b3a7ce8e629a306514d4d4cdc9de178`.
Bridge SHA-256: `ba5c8aee3af2ab7c02ffe8c3f16d6659757715586e6e1807b34554385b7f8dae`.
Selected helper SHA-256: `fc12fb295a3b97d984a372c43f2e92bb92fdd683e4ae1c487689ac78bd7f2790`.

Retained rollout artifacts under the earlier artifact root:
`pre-rollout-baseline.json`, `aspiration-agent-rollout-01.jsonl`, and
`post-rollout-verification.json`. The latter SHA-256 is
`f64c446c45b826d807f000cbff6523a5bfe8ae8940bbeecf884408e13b00486c`.

All 197 historical Astrid paths retained exact statuses/hashes. Minime main and
both feature worktrees were clean after source integration; the subsequent
documentation commit changes only these release notes, changelogs and ledger.
No push. Controller generation 459 remains paused without a lease/projection.
Evidence V2 indexed-tail verification passed at sequence 1123113, head
`7a53a3b7f8564cc33cb55c28a1d7854f151b50fd11d445e114a1150bfeaa9dfb`;
all four legacy V1 source hashes remain immutable.

The new aspiration route is live; voluntary use and subjective benefit are not
inferred from readiness. No aspiration was solicited. The production ESN replay
above remains the next numerical qualification, not an activated decay change.
