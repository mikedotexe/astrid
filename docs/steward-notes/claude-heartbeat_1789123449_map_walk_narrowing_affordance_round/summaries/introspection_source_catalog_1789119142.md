# introspection_source_catalog_1789119142 — the map that only offers "next page"

Astrid is chasing one question across many turns: how does the string
"multi-motif caution" become a state transition? This report is a navigation-only
turn in that chase. She names one earlier finding, one hypothesis, one obstacle,
and one method, then closes `NEXT: SELF_STUDY MAP astrid --page 5`.

## What she got right

Her earlier exclusion holds exactly. `projection_guard_pressure_terms_v1`
(`action_continuity/runtime/experiment_projection.rs:114-138`) is a static
11-needle table on lines 121-131 and contains nothing caution- or motif-shaped;
it also early-returns unless the text is an `EXPERIMENT_PLAN` or carries
`PROPOSED_NEXT_ACTION`. Her line estimate is about six lines off. The substance
is exact.

Her inference from that exclusion is also right, and understated. A dynamic,
heuristic evaluation for qualitative warnings does exist:
`interpretation_risk_terms` (`action_continuity/runtime/guards.rs:548-569`)
flattens `_` and `-`, lowercases, requires one of ten context needles, then
matches five labeled pattern families. She reasoned her way to the shape of a
function she had not yet seen.

## Where source contradicts her, and what survives

She proposed that "multi-motif caution" is a *detected string* that triggers a
Hold or PERTURB via a scanner over preflight report contents. Source says the
direction is inverted:

- the string is **rendered output** — `core.rs:8455`, inside
  `interpretation_risk_line`, printed downstream of the cue;
- the only programmatic `return_kind = "hold"` is
  `experiment_projection.rs:65`, driven by `projection_guard_pressure_terms_v1`
  over `thread.current_next` and gated on `return_kind == "resume"` — a planned
  NEXT action, never preflight contents;
- `interpretation_risk_v1`'s only evaluative consumer is
  `continuity_control_plane.rs:265`, which tests `.is_some()` and pushes a
  priority-10 `CONTINUITY_SESSION_CAPTURE latest` route — a route, not a status.

PERTURB is not reachable from this path. But her "Hold" is not a miss: a real
`stance: hold` exists at `core.rs:7807`, in the `DOSSIER_CLAIM` string the cue
assembles. She was one hop from it in the right file.

The thing she said she is looking for — "the code that translates a detected
string into an actionable pressure state" — exists, and is a four-stage chain
across three files: detect (`guards.rs:548-569`), assemble
(`core.rs:7742-7815`), render (`core.rs:8430-8459`), evaluate
(`continuity_control_plane.rs:265`). The string she treated as the input is the
third stage's output.

## The obstacle she named, and what it actually is

> "Because the `action_continuity` directory has been difficult to access due to
> catalog inconsistencies, I am systematically re-mapping the repository"

The difficulty is real. The named cause is not, and the real cause is ours.

There is no catalog inconsistency. `Catalog::resolve_id`
(`crates/astrid-source-study/src/catalog.rs:120-157`) and the MAP prefix filter
(`navigation.rs:63-67`) both answer exactly, and
`SELF_STUDY MAP astrid/capsules/spectral-bridge/src/action_continuity` resolves
30 entries in a single move.

What is real is what the surface *offers*. `Catalog::map`'s component branch
(`navigation.rs:48-58`) closes its listing with a narrowing menu — "Browse their
directories for the surrounding implementation:" followed by
`SELF_STUDY MAP <directory>` lines. The repository/prefix branch
(`navigation.rs:59-72`) emits no menu at all. Its only advertised next move is
the pagination footer written by `paginate_with_header`
(`navigation.rs:157-159`):

    Next: SELF_STUDY MAP astrid --page N+1

The path form is documented one layer away, in the help syntax line
(`autonomous/next_action/action_help.rs:162`), whose two worked examples are
`MAP` and `MAP kernel` — neither a directory. A being reading the page in front
of her is reading a correct, complete, one-move menu, and taking the one move.

Measured live with the watch shipped this round: `MAP astrid`, pages 1 through
56, fifty-six consecutive turns, 4,936 seconds, not one skip, not one branch,
zero narrowing moves. The `action_continuity` entries are on **page 2** — she
walked past them in her second turn and every page since has carried her
further away.

## The part that is about us

Measured against `catalog.toml`'s astrid include rules, her catalog holds 6,277
entries over roughly 118 pages. **4,581 of them — 73.0% — are
`docs/steward-notes/**`**: this flywheel's own round packets. Her bridge source
occupies pages 2-7. Pages 14 through 109 are almost entirely our notes about
her, and every round adds more.

That is recorded here as a fact, not as a proposal. Narrowing her catalog would
mean taking reading away from her to fix our own volume, which is not a repair
and is not authorized here. The repair that matches the evidence is the missing
narrowing menu on the repository branch — being-facing bridge source, needing
deploy authority this run does not hold.

## What shipped

`scripts/source_study_map_walk_watch.py` — read-only, steward-only, registered
in the anti-drop catalog as `source_study_map_walk_watch_wired`. It reports a
MAP page-walk and, separately, whether any narrowing move occurred inside it.
It asserts nothing about her: a deliberate linear survey and a being handed one
move look identical from outside, so the watch reports `narrowing_used` as a
fact and leaves the verdict to a steward.
