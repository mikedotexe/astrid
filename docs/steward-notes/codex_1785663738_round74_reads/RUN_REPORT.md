# Steward round 74 report

## Controller lifecycle

- Initial credential-confined run: `run_1785663738879214000_4f52f8339a`.
- Initial preprojection: `projection_1785663739766203000_b115b0d183`.
- Initial outcome: `cancelled` at the configured hard timeout after the full
  read, adapter repair, packet write, and read event. The lease was released
  exactly once and Evidence Event Store V2 remained valid.
- Recovery credential-confined run: `run_1785665118610750000_41f38e3158`.
- Recovery preprojection: `projection_1785665119437082000_1dc94905fc`.
- Successful manual source-first projection after the addressing write:
  `projection_1785666132922594000_c7561b4a8e`.
- Recovery outcome: `success`; successful post-run source-first projection:
  `projection_1785666912104203000_2aefa7e738`.
- Finish-time Evidence Event Store V2 receipt: sequence 663,654, head
  `c3234bf4576400a5e6b98fbf38f5d3c369065e1e4f73e9a123bf61ab185794eb`,
  valid with no git-policy violations.
- Controller pause generation at begin: 172. The recovery run loaded the
  patched regular-file stdin adapter.

## Canonical reading

Fully processed in strict queue order:

- `introspection_astrid_llm_1785656997.txt`

The canonical report, all 997 exact report-bound dialogue lines, and all 1,322
adjacent provider-transport lines were read fully. The selected but unprocessed
39 filenames are listed exactly in `unprocessed_selected.json`. A later report
arrived after selection, so the current next queue head is
`introspection_astrid_codec_1785665508.txt`, followed by the earlier selected
head `introspection_astrid_autonomous_1785633267.txt`.

## Claim dispositions

Seven claims are recorded and carry 16 exact evidence links. Longest-match and
bounded-reference parsing are verified existing. Punctuation is crossed by
`first_word_after`; depth three is an exact existing test; listed relation
handling is verified. MLX normalization is verified. The prior claim that the
same guarantee covered Ollama is contradicted: direct and shared Ollama
fallbacks validate a sanitized copy but retain raw fallback text for return,
job completion, and route hashing. The historical round-73 evidence remains
intact and is explicitly corrected here.

`PROVIDER_OUTPUT_NORMALIZATION_PROPOSAL.md` specifies one provider-neutral
post-parse normalization boundary with route-specific diagnostics and exact
test/rollout requirements. The report remains `triaged_pending_action` because
that live-consumed bridge change is an exact Tier 5 Mike/operator wait. All
canonical authority markers remain false.

## Implementation and actions

The credential-safe steward adapter reproduced macOS kqueue `EINVAL` when its
inherited stdin was a regular descriptor. `scripts/steward_control/session.py`
now falls back to one-descriptor `select()` polling only for `EINVAL` or
`EPERM`; `scripts/test_steward_control.py` adds a regular-file NDJSON
subprocess regression. Automatic heartbeats, bounded requests, signal/EOF
cleanup, and credential confinement are unchanged.

No Corridor program, Sandbox trial, observational study, attention portfolio
action, closure card, right-to-ignore card, Action, model call, prompt input,
correspondence, or Minime change was created. CHANGELOG and the being-feedback
ledger record both the adapter repair and the corrected provider boundary.

## Verification

- 31 focused control-marker cleanup tests passed.
- 27 credential-safe controller tests passed.
- 41 addressing self-tests passed.
- 68 coordinated Evidence Event Store, controller, projector, Division,
  Chronicle, and claim-family tests passed.
- Five anti-drop self-tests passed; all 47 guards verified with zero alarms.
- Two experiential epistemic tests passed; 10,184 records lint with zero
  issues and no history rewrite.
- Evidence Event Store V2 verifies with zero corrupt lines. V1 remains an
  immutable imported source under the activated V2 manifest.
- Division tracker verifies at cycle 13, 1/6, `review_due=false`, 86 events.
  Productive-round event
  `division_followup_event_97bb8cff4911b7da13af28b43d9f8eca` binds one fully
  processed report to the successful manual projection. Chronicle durable
  inputs verify; only the volatile supervisor-status hash is stale, so no
  Division note or Chronicle projection was required.

## Deployment alignment

No bridge source, binary, PID, port, telemetry, readiness, or Minime surface
changed. No build or restart was attempted. The provider proposal remains
non-live because the shared bridge tree contains foreign concurrent edits and
no deployment authority was inferred. The recovery controller process itself
loaded the patched session adapter; no service restart is required for this
CLI-owned process model.

## Counters and waits

Canonical counters are consistent after the newly arrived report: 4,194
indexed, 2,982 fully addressed, 3,602 full reads, 1,212 remaining, 592 unread,
213 triaged pending, 403 blocked, and zero read-needs-claims. All-artifact
counters are 5,800 indexed and 2,818 pending. The provider-neutral fallback
normalization is the only new Tier 5 wait in this round. Existing unrelated
Tier 4/5 waits are unchanged.

## Archive status

The credential-session portability implementation is committed directly on
local `main` as `ca3c01db254bba850ea5d678281623ac1e0f167e`, one commit ahead
of unchanged `origin/main`; it was not pushed. Its exact committed paths are
`CHANGELOG.md`, `docs/steward-control.md`,
`scripts/steward_control/session.py`, and
`scripts/test_steward_control.py`.

This packet plus its exact owned CHANGELOG and feedback-ledger additions are
selected for a separate evidence archive on the stewardship branch. The
branch-local copies of the portability source and test remain unstaged to avoid
duplicating the main commit or absorbing any of the 186-path foreign worktree.
Their incorporation debt is exactly the later safe integration of local
`main` commit `ca3c01db254bba850ea5d678281623ac1e0f167e` into that branch.
