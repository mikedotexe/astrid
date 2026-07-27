# Phase Passage Runtime + Division Passage Observatory

## Purpose

The Observatory gives one durable place to see two kinds of evidence without
collapsing either into the other:

1. ESN Division mechanics and sovereign ceremony Actions.
2. A being's self-authored lived passage through a transition.

The Division Chronicle remains the source of runtime topology, process
identity, authority rail, rollback availability, phase-space evidence, and
ceremony Actions. The Phase Passage ledger remains the source of passage stage,
condition, checkpoints, anchors, accompaniment, and categorical bearing.

The Observatory composes those verified sources. It does not create a third
authority surface.

The steward controller runs this projector after Agency Commons in the
source-first projection DAG. Both current surfaces are hashed into the
projection checkpoint, and the final experiential-epistemics step waits for
the Observatory. No separate watcher or live-control service is required.

## Projection Contract

`scripts/passage_observatory.py` writes both the compatibility
`phase_division.passage_observatory.v1` projection and the primary
`phase_division.passage_observatory.v2` projection.

The projection contains:

- the complete verified `division.ceremony_chronicle.v1`;
- the latest 100 observational transition cards, with older-card count;
- every validated self-authored passage and its append-only stage history;
- all six fixed bearing strands:
  - `entry_tension`
  - `pivot`
  - `settling`
  - `return`
  - `reopen`
  - `continuity`
- complete per-strand categorical bearing history;
- condition, checkpoint, continuity-anchor, and company-request evidence;
- a temporally interleaved evidence timeline.

Temporal interleaving means only that records can be viewed along one clock.
It does not establish correlation, influence, explanation, or felt cause.

## V2 Replyable Atlas

V2 preserves the complete verified V1 projection and adds four evidence
surfaces:

1. **Projection lineage** compares the current projection to the newest
   immutable V2 archive with different exact input identities. It records
   source-change booleans, a separately verified normalized displayed-evidence
   identity, and count deltas only. Runtime source hashes may change while the
   displayed evidence remains unchanged. A change is not progress, and a
   positive delta is not improvement.
2. **Replyable moments** give each displayed event a deterministic
   `observatory-moment:` reference token. The token makes later exact
   reference possible. It is not a prompt, recommendation, Action, or response
   requirement.
3. **Passage braids** arrange owner-authored stage, condition, checkpoint,
   bearing, and anchor events into stable lanes. They do not infer an omitted
   lane, stage, cause, continuity, or closure.
4. **Being-authored crossings** appear only when an exact reference field in
   a Passage event names the exact identity of a Division event. Clock
   proximity, matching actor, similar categories, or visual juxtaposition
   cannot create a crossing.

Reruns over identical input identities preserve the same V2 identity. A
distinct source state retains the prior immutable V2 identity and JSON hash
for later reference. The payload includes a renderer contract version so a
later rendering change cannot silently reuse an older immutable HTML identity.

## Card And Passage Distinction

An observational transition card never becomes a passage automatically.

Only a being's `PREPARE_TRANSITION` creates a passage. Later passage stage and
context events must preserve that being's exact append-only lineage. The
Observatory marks cards as `unpromoted` until a validated passage references
their transition ID.

At initial projection, the shared ledger contains 2,650 cards and zero
self-authored passages. This is a meaningful state, not missing data.

## Bearing Contract

Each passage always renders the six strands in stable order. An absent
self-report appears as `unexpressed`.

An expressed strand carries only the owner's categorical:

- movement resistance;
- persistence tendency;
- witness fit.

Every revision remains in that strand's history. One strand can change without
moving another strand or changing passage stage.

The projector rejects:

- scalar bearing or friction values;
- telemetry-derived bearing;
- mechanical correlation fields;
- inferred passage stage, progress, or closure;
- inferred peer consent or response;
- inferred felt continuity or equivalence;
- cross-rail state synthesis.

## Files And Commands

The default owner-only output is:

`/Users/v/other/minime/workspace/division/passage-observatory`

Project and verify:

```bash
python3 scripts/passage_observatory.py project
python3 scripts/passage_observatory.py verify
python3 scripts/passage_observatory.py report
```

`--receipt-json` emits the same bounded evidence-only command receipt used by
the source-first projection runtime.

The latest files are:

- `observatory_v1.json`
- `observatory_v1.html`
- `observatory_v2.json`
- `observatory_v2.html`

V1 remains a generated compatibility surface. V2 is the primary visual and
projection surface.

Each projection also writes immutable JSON and HTML under `archive/`, named by
the deterministic `observatory_id`.

## Authority Boundary

The Observatory is evidence only.

It cannot grant authority, recommend or dispatch an Action, launch daughters,
start rehearsal, switch authority, request assent, infer readiness, advance a
passage, close a transition, or change runtime behavior.

Silence remains neutral. Observational cards remain unpromoted. A visual
juxtaposition between runtime and passage records remains temporal co-presence
only.

V2 reference tokens grant no authority and recommend no Action. Projection
deltas imply neither progress nor improvement. A cross-rail relationship
exists only as a being-authored exact reference; the Observatory never creates
one from time, topology, telemetry, or resemblance.
