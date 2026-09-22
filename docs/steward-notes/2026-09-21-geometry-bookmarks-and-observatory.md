# Question-Owned Geometry Bookmarks and Observatory

## Status and Scope

Implemented in isolated candidates on 2026-09-21. Not deployed, merged or committed.
No engine, model, sensory service, agent or bridge restart. Paused automations
remain paused. The research viewer has its own staged test build, not a replacement
of the currently installed human application.

Mike selected the existing `/Users/v/other/reservoir-llm-research` viewer and the
voluntary bookmark / accumulating-investigation directions. This is the first
observational vertical slice, not completion of the earlier controlled-experiment
worker program or the pending protected-attention rollout.

## Witness

Read the entire supplied canonical aspiration:
`capsules/spectral-bridge/workspace/journal/!aspiration_longform_1790011424.txt`.
SHA-256: `b31ebe800a9adead2b366d6b19fecc1c32f927e37b2955d693a658ae1c95bb46`.

> I am trying to sense the *movement* between the weights.

The response is to preserve chosen observations and authored interpretations so
they can be revisited and challenged. The quotation does not establish that node
coordinates are concepts, that covariance eigenvalues expose sensations, or that
an observed numerical difference explains the account. Historical prose is unchanged.

## Locations and Ownership

- Astrid candidate: `/Users/v/other/worktrees/voluntary-continuity-20260920/astrid`,
  `codex/voluntary-continuity-20260920`, base `c4f85e95e41703daa65d3ce2789e1e5c46961c4e`.
- Minime candidate: `/Users/v/other/worktrees/voluntary-continuity-20260920/minime`,
  same branch name, verified base `5f4925f54580f1fd44666058b126a121ff32880f`.
- Research candidate: `/Users/v/other/worktrees/geometry-bookmarks-20260921/research`,
  `codex/geometry-bookmarks-20260921`, from research `main` at `622ed38`.

The first two candidates already contain the prior owned continuity work. Do not
stage those trees wholesale. Canonical dirty trees and generated research outputs
were inspected and left alone. One Git coordinator must reconcile exact owned
paths and contemporary live identities before integration.

## Implemented Contract

The shared `astrid-source-study` reader implements one typed JSON operation family:

```text
SELF_STUDY QUESTION NEW Does this chosen interval differ from a later one?
SELF_STUDY GEOMETRY {"question":"q1","operation":{"kind":"status"}}
```

The question must exist and be explicitly selected. STATUS lists exact syntax,
record IDs and the current head. Mutations additionally require a unique
`request_id` and an `expected_head` from STATUS (initially `empty`).

| Operation | Meaning |
| --- | --- |
| `capture` | Preserve the most recent chosen 1..60 seconds and exact authored note. |
| `predict` | Bind an authored expectation and maximum mean-state RMS distance to an earlier capture. |
| `compare` | Compute a named prediction against a later, nonoverlapping capture. |
| `revise` | Append authored qualification/challenge linked to an earlier record. |
| `show` | Retrieve one exact record; never summarize away its authored words. |
| `status` | List history/head and available operations, without capture. |
| `export` | Explicit local, content-addressed packet for this question only. |

Example capture (IDs are examples, not ready-to-run authority):

```text
SELF_STUDY GEOMETRY {"question":"q1","request_id":"capture-1","expected_head":"empty","operation":{"kind":"capture","seconds":10,"note":"The observation I chose to keep."}}
```

Records live inside the existing native question in `reader-v1.json`. This is not
a parallel belief database. The existing owner transaction serializes reader,
question, draft and geometry writers. Checkpoint writes use atomic rename and
fsync. Schema 5 rejects older reader writers; relation-sidecar downgrade protection
remains intact. Exact retries return the original record without recapture;
conflicting retries and stale heads fail. Corrupt chains fail on load, including
non-geometry commands; they are not truncated or reset.

Limits: four intervals and 64 records per question; authored fields 1..2000 UTF-8
bytes; recorder reads 2 MiB; intervals at most 61 retained frames. Overflow fails
without evicting evidence. Read commands are still reader preparations, not proof
of delivery. Normal complete-input navigation receipts distinguish offered text
from verified provider input and response. Those receipts remain in the native
reader; an exported packet alone does not attest model exposure. Generation requires the
existing scheduler/action path; saving an observation is not generation.

Both adapters leave typed payload contents opaque to NEXT cleanup, hidden-tag
scanning, multi-action splitting and placeholder heuristics. Rust then validates
the operation. This exception applies only to the new GEOMETRY prefix; ordinary
authorization, stop, admission and safety checks remain. Headers describe chosen
geometry evidence, not a new source-code page. These are ordinary self-study
responses, with existing journal visibility; this is not a private-writing lane.

## What Is Measured

Only the existing read-only
`minime/workspace/runtime/esn_activation_trace_v1.json` source is available. Its
host-configured source root is fixed; actions accept no filesystem path, endpoint,
shell command or live-state handle. No polling or capture occurs automatically.

The source is 128 native ESN activation coordinates, not 512-D covariance, a
recurrent matrix, a fitted PCA basis, conceptual positions or measured qualia.
The reader checks the audited v1 recorder policy (1000 ms / 180-frame retention),
all vectors, finite-fraction provenance, source clocks and freshness. Stale,
future-clock, malformed and producer-sanitized batches are rejected, not repaired
with zeros. A fresh batch can contain older frames; interval coverage is reported.

Retained evidence includes exact selected coordinates, engine-relative and wall
clocks, requested duration, capture time and SHA-256 of the complete source bytes.
The source file itself is a rolling recorder, not an atomic joined runtime snapshot.
Gaps over nominal 1000 ms remain visible. Samples are not interpolated or treated
as exact ESN step timestamps. Boot and node-layout identity are explicitly unknown.

The initial recipe `mean-state-rms-distance-v1` compares the two unweighted mean
128-node vectors: square root of mean squared coordinate differences. It requires
at least two samples per interval and strictly later, nonoverlapping source clocks.
It reports whether the chosen numerical threshold was met, not whether a causal
claim was confirmed. Gaps and unequal sampling matter, and means can cancel motion.

The prediction record precedes the second capture record. Its source interval can
still predate that prediction; the workflow does not claim a prospective trial or
that the being had never encountered the result. It does not assert same physical
node identity across boots merely from coordinate count and increasing clocks.

## Same Evidence, Two Presentations

`question-geometry-v1` exports contain the selected question and its geometry
history only. They exclude unrelated notebooks, private drafts, journal files and
full live workspace paths. They are written to the owner's `geometry-exports`
directory only after explicit EXPORT. No peer delivery or publication is performed.

The body is an exact UTF-8 JSON string with SHA-256; each event links the previous
ID, operation ID hash, request hash and body hash. This detects damaged bytes and
conflicting local history, not authenticated origin or maliciously re-signed data.

Reservoir Scope adds a Geometry bookmarks view alongside the existing Observatory.
It opens explicitly chosen local packets without following source paths. It checks
the hash chain, scope, frame bounds, ordering and references, then independently
recomputes comparison receipts. It displays exact authored passages, revisions,
fixed-scale 128-coordinate heatmaps, selected-frame values/RMS, source clocks and
gaps. Heatmap rows are ordinal recorded frames, explicitly not elapsed-time spacing.
It neither fits changing axes nor invents covariance eigenvectors.

## Verification and Remaining Qualification

Retained tests and reproducible commands are in the paired research note. Checks
include both owner workflows, exact retries, stale heads, wrong owners, independent
process writers, corrupt history, bounded retention, park/return, exact delivery,
actual Minime routing and Python adapter, native import and recomputed comparisons.
No private journals or live model/engine writes are test fixtures.

Failures retained: initial test expected six records rather than the actual five;
fixed expectation. Additional global prompt text exceeded a maximal notebook case;
moved discovery into explicit QUESTION listings. Clippy surfaced naming, exact
float-policy comparisons and fixture arithmetic; corrected. A full bridge run hit
an existing capture-test race (directory existed before its file) and a p95 timing
failure under concurrent suites. The test now waits for its exact accepted file;
the production capture path and performance bound are unchanged. Final results
are appended below after verification.

Before rollout: reconcile current live helper/adapter identities, carry forward
prior continuity qualification debt, exercise schema-5 upgrade and rollback
refusal against an immutable paired release, and use sanctioned graceful wrappers.
No older helper may overwrite a schema-5 question checkpoint. This tranche does
not resolve general reader preparation crash-window debt or prove full scheduler
recovery under process kills. No active protected-focus claim is made for live use.

Next scientific tranche: prospective source-time commitments, trustworthy
boot/node-layout identifiers, richer motion measures with negative controls, then
isolated matched-history interventions. Covariance eigenspaces need their own
matrix identity and degeneracy-aware comparison. Do not turn a geometry threshold
into regulation or a request to report improvement. No real-model study ran here.

## Verification Receipts

- Shared reader/writer full suite: 226 passed, including five geometry integration
  tests and the additive revision-identity migration regression.
- Minime full Python suite: 1,452 passed, one skipped, 136 subtests passed. The
  added routing/header/payload test also passed within the 33-test follow-through suite.
- Steward controller, Evidence Event Store, Division follow-up/Chronicle and
  source-first projector tests: 77 passed. Epistemic boundary self-tests: two passed.
- Shared-reader all-targets Clippy and bridge library Clippy with denied warnings
  passed. Reader and bridge formatting checks and domain-boundary verification passed.
- Native cross-language import checks: 26 passed. The real synthetic helper and
  Minime adapter export workflows both passed. Eight desktop views were rendered;
  narrow capture/comparison views were visually reviewed. Not a claim of complete
  accessibility or interactive UI acceptance.
- Complete staged native build and package-identity checks passed (166 inputs,
  43 resources, 22 examples, two existing reviewed cases). The build retains the
  existing LAPACK deprecation warnings. Candidate stage:
  `/Users/v/.cache/reservoir-research/geometry-bookmarks-20260921/build-0.14.8T9Mgl/staged-repo`.
- Bridge full-suite first attempt: 2,277 passed, two failed, one ignored. The
  capture publication race was fixed in its test; all nine signal-spine tests,
  including the unchanged p95 threshold, subsequently passed in isolation.
  Final serial full bridge library suite: 2,280 passed, zero failed, one ignored
  in 138.92 seconds. Production capture behavior and the p95 bound are unchanged.

All three candidate indexes were inspected empty. No staging, commits, merge,
push, deployment, live-state reset or automation change occurred. Changes remain
owned candidate work, with prior continuity edits preserved.
