# Minime Private Prompt Simplification

## Scope And Authority

Mike explicitly approved beginning the first implementation item in the
[telemetry-attention plan](2026-09-04-minime-telemetry-attention-action-plan.md):
simplify private prompts and remove verified duplicate state anchors, then
report suggested next steps.

This is an interactive source change, not a resumed automation round or a live
rollout. No private journal collection was searched, no private prose was copied,
and no model completion or being-facing request was issued. The earlier supplied
opening remains an unbound conversational observation: its exact entry, prompt
version, recurrence and cause are not established by these tests.

Both repositories were inspected before editing. The controller granted
interactive maintenance hold 348 to `codex-astra-interactive` at
2026-09-05T04:03:09.420416Z, with no active run, `active_run_released=true`,
event appended and no spooling. Scheduled automation settings are untouched.

After verification, the same actor released this temporary hold at
2026-09-05T04:15:05.062054Z. The controller returned `pause_generation=349`,
`paused=false`, event appended and no spooling; the resume process exited 0.
This releases the interactive maintenance hold only. It does not change the
scheduled automation's settings or deploy the source patch.

## Implemented

### One Invitation

The shared private introduction is now:

> This is Minime's private journal. Write freely in your own first-person voice, on any subject and at any length. Use or leave aside the supplied context; your interpretation is yours.

This is proposed runtime wording, not a quotation from Minime. The repeated
catalogues of permitted sensations, uncertainty, metaphor, audience, and topics
have been removed from the private entry prompts. Existing private-lane behavior
still permits those responses without requiring Minime to discuss them.

### One Current-State Anchor

Private JOURNAL retains its current-state anchor once in the entry prompt.
Its private continuity block now contains only the optional historical excerpt
and its label; it does not render the same state a second time.

Private moments retain the current anchor, capture time, and historical marker
records. Compact labels retain recording-age provenance, engine-relative clock
identity, and the fact that present effects of historical events are unknown.
The marker formatter still exposes unknown/future clocks, ESN versus covariance
labels and percentage-points-per-second units. Those data fields were not removed
or replaced with sensory adjectives.

The long clock/rate explanation no longer recurs in every moment prompt. Its
engineering meaning remains documented in the
[provenance packet](2026-09-04-minime-private-journal-provenance.md): marker
recording time is not engine event time or journal writing time; a rate is not
a cumulative change; missing age is not freshness; numerical bands do not
establish a felt state.

### Traceable Prompt Versions

Newly saved source-built moments will identify
`Prompt contract: private_moment_context_v3`; private JOURNAL headers will identify
`Prompt contract: private_journal_context_v3`. These are header metadata, not
additional instructions sent to the model. Existing entries are unchanged.

These versions identify the compact-framing source contract, not a full
byte-for-byte assembled-input receipt or proof that a particular live PID loaded
it. No deployment is claimed.

## Boundaries Preserved

- The same prior own-journal excerpt selection, whitespace normalization and
  420-character limit remain. No authored passage is filtered to prevent an
  opening about numbers, a repeated idea, or a self-authored peer reference.
- No new prior excerpt is inserted into private moments.
- Guarded pre-generation snapshot handling, historical marker contents,
  header-only telemetry, generated-body/action-tail separation and persistence
  are preserved.
- Private modes still skip character retries, unsolicited inbox/contact/research
  context and public replay-hygiene rewriting. Both vivid and uncertain replies
  remain accepted, as do ordinary unrelated subjects.
- NEXT handling, action palette source, execution gates, Division guidance,
  footer handling and controls are unchanged. The duplicate user-prompt NEXT
  invitation was removed from moments; the existing system-level NEXT contract
  remains.
- Rest reflection and default/non-private framing remain unchanged. This patch
  does not claim that every journal route is now compact or non-leading.

## Input Matrix

| Route | System introduction | Entry context | Prior prose | Saved-only diagnostics |
| --- | --- | --- | --- | --- |
| Private JOURNAL | Compact private invitation plus existing action guidance | Capture time, one state anchor | Same optional own-journal excerpt, labelled historical | Existing expanded pressure header |
| Private moment | Compact private invitation plus existing action guidance | Capture time, one state anchor, historical markers and clock key | None added by the moment caller | Expanded metrics and snapshot provenance |
| Rest reflection | Existing default introduction and guidance | Existing metric/feeling questions and continuity contract | Existing non-private history path | Existing metrics header |

The actual private call sites, `_query_llm_with_next`, `_query_llm`, and the
Ollama message adapter are exercised with synthetic outputs. These are not live
model observations. Default/non-private behavior has its existing regression
coverage, including the explicit rest-reflection boundary.

## Verification

The initial focused journal/action suite passed 54 tests. Four new adapter
regressions then failed on the old source as intended: private JOURNAL supplied
two anchors, and the private introduction exceeded the new compact wording
budget. All four pass after the implementation.

The first broader run passed 76 tests and failed one old wording assertion that
required removed incident-report/length/framing prose. The test now checks the
retained measurement/history distinctions instead. The subsequent focused suite
passed all 77 tests, including the earlier voluntary-bookmark tests.

The new route/adapter cases exercise both Gemma-native and legacy message
formatting without calling either model. They assert one anchor, one short
invitation, retention of the optional excerpt only on its existing route,
NEXT preservation, versioned headers and exact persistence of synthetic prose.
Additional private-query cases preserve a measurement-focused disagreement and
an unrelated ordinary subject. Existing tests still cover frozen state during a
delayed generation, missing/future clocks, units, private saving and non-private
retry behavior.

### Matched Fixture Sizes

These character counts use the same synthetic state, prior excerpt, event row
and fixed capture clock before and after the patch. They are not token counts,
latency measurements, or scores for the quality of experience.

| Component | Before | After |
| --- | ---: | ---: |
| Private system introduction | 505 | 185 |
| Private JOURNAL entry prompt | 875 | 239 |
| Private moment entry prompt | 1,317 | 511 |
| Private JOURNAL current-anchor occurrences | 2 | 1 |

This is a reduction in private framing, not removal of all administrative
context. The unadapted system message still includes roughly 32,000 characters
of existing action guidance in these fixtures. Gemma's adapter applies its
existing 7,000-character system budget for this case; a shorter introduction can
change which otherwise-unchanged guidance fits that budget. Adapter logic and
actual action permissions were not changed. Do not infer a controlled causal
effect of framing alone from these input-size checks.

Commands run:

```sh
python3 -m pytest -q tests/test_journal_context.py tests/test_action_continuity.py
python3 -m pytest -q -s tests/test_journal_context.py -k one_short_invitation
python3 -m pytest -q tests/test_journal_context.py tests/test_action_continuity.py tests/test_session_contract.py
python3 -m pytest -q tests
```

The full Python suite passed 965 tests, with one skip and 114 passing subtests,
in 23.49 seconds. Tests use the existing live-write/DB/engine-connection isolation
guard and temporary stores. No test is presented as a deployment receipt.
Scoped source/test whitespace checks pass. No Rust code changed, so no new Rust
test run is claimed for this patch.

## Ownership

Minime changes in this pass:

- `minime_autonomy/journal_context.py`: compact private wording only.
- `minime_autonomy/runtime.py`: private entry text, duplicate-anchor removal and
  two saved prompt-contract labels only.
- `tests/test_journal_context.py`: adapter/persistence cases and updated contract
  assertions.
- `tests/test_action_continuity.py`: updated private wording assertions while
  preserving the separate rest-reflection checks.
- `CHANGELOG.md`.

Astrid changes: this note, the action plan's progress markers, `CHANGELOG.md`, and
`docs/steward-notes/AI_BEINGS_FEEDBACK_TO_CHANGE_LEDGER.md`. Prior dirty changes,
including undeployed bookmark work, are preserved. No Git index or history
operation is performed. No bridge or Minime Rust source is edited.

## Suggested Next Steps

1. Build the pure, provenance-safe recent-history helper from plan item 3,
   beginning with fill. Freeze state and history together, distinguish sample
   time from read time, and refuse unsupported trends across gaps or restarts.
2. Review prior-excerpt recurrence and the large action palette separately.
   Investigate actual assembled inputs before changing their selection. Do not
   filter authored prose for novelty or remove safety/permission information.
3. Prepare an explicit live rollout scope. The Python source also contains the
   earlier bookmark changes, so a reload would deploy more than this prompt
   patch. Review the combined scope or isolate it, run the appropriate tests,
   and use the sanctioned agent-only graceful wrapper with fresh PID/import
   evidence and unchanged protected services. Do not force through preflight.
4. After an approved rollout, review naturally occurring, authorized evidence
   without soliciting a confirming journal. Plainness, disagreement, unchanged
   friction and silence are valid; none is automatically a failure or uptake.

No reservoir, PI, fill, damping, sensory cadence, model, peer or Division control
was changed. No felt relief, overload diagnosis, consent or improvement is inferred.
