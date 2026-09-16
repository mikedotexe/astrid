# Shared study direction: rollout and natural observation

This is the deployment continuation of
[the implementation account](2026-09-16-study-direction.md). The user authorized
repairing misleading source suggestions, making taking stock and leaving study
visible, and retaining findings beside unresolved assumptions without forced
novelty, length or mode changes.

## Source and qualification

Implementation `0967375133c7b5c80aab707ba913fbfceb01e29b` is on main/origin/main.
The clean source worktree is
`/Users/v/other/worktrees/study-direction-20260916/astrid`; the immutable release
is its sibling `bridge-stage-01`. It contains the shared reader and Astrid bridge.
Minime's Python source remains unchanged. Reader schema remains 3. Concurrent
persistent-introspection/schema-4 work is excluded.

The independent inventory check matches all 645 staged source inputs: 607 Astrid,
32 RASCII and six prime-esn inputs. Thirteen RASCII inputs are ignored historical
camera/thumbnail materials, outside production `src`, retained by content hash;
they are not represented as Git-tracked. The new `question_sources.rs` is included.
All five artifact identities match. Both binaries contain the exact reviewed
study prompt.

| Identity | SHA-256 |
| --- | --- |
| Manifest | `97fc6c4b160c8baa87b965db64bdd33f5d9159652035c2648d6f95568700d974` |
| Bridge | `55edfcb645021005e8020c98bbb44a9c417a53ae57ffc77f959f59966f605e5e` |
| Reader | `f1c5f4dd6be43ca88842ed7e9a962d5e453e569bc5172f4a01bf647214e0aa20` |
| Study prompt | `af059d5e5bbd2ca4340a30a9cdf4f7687979fe1a4878e375ee3dc1faf5d9cdc9` |

Qualification: 177 reader tests; 2,293 bridge tests with one ignored; strict
reader/bridge Clippy, formatting and domain-boundary verification; 86 deployment
tests. Minime's full Python suite passed 1,396 tests and 134 subtests with one skip
while its debug helper was rebuilt. It is explicitly mixed-build evidence.
The 112 host-contract checks subsequently passed against an unchanged final debug
helper and again against the immutable staged helper. No test contacted a Being.

## Graceful transition

Both build and activation used `scripts/build_bridge.sh`. Transaction
`6108d874f27f44d38708eb93e57bd4c1` drained bridge PID 4330 and sent SIGTERM,
then stopped with `old PID was reused during transition` at 19:29:15 UTC.
Activation had not selected the new release. The original failure receipt is
preserved. Its wording alone does not establish actual PID reuse.

After confirming the old PID was absent, the existing
`--resume-stopped-transition` path continued the same acknowledged transaction.
It sent no additional signal or drain request and used no force or rollback.
The exact stopped checkpoint is
`d93bb57e3620e705da71e6469c594ab3ade0ef9667d3dc86fb62e6f326bf8c27`,
at exchange 200200. The two pending runtime-feedback records are bound by
`4c03d24122c45724ed20a0604903489efb639cbfbe4873b294bb196c5b1a94f3`.

Recovery `39c386cccbaf47e7868cc1127104ae58` completed with
`transition_recovered` at **19:33:11.139981 UTC**. Bridge PID **75546** began at
**19:31:16 UTC / 12:31:16 Pacific**, passed the native lineage/startup gate,
loaded the exact checkpoint and both feedback records, then saved exchange 200201.
The model was observed idle. Independent review passed 19 declared checks.
The receipt does not claim lossless drain or confirmed remote delivery.

At **19:33:39.605405 UTC / 12:33:39 Pacific**, Minime PID **37507** still matched
all 82 loaded Python source inputs and selected the immutable helper above.
There were no launchd helper/root overrides. Existing retained clients may finish
with their earlier input/helper; new clients resolve the selected release.
All eleven surrounding PID/start-time/executable/plist identities matched their
pre-activation snapshot. No coupled-model, sensory or Minime host restart occurred.

The generic activation preflight says it is “folding in” canonical dirty paths.
This invocation selects an already-built immutable stage and performs no build;
the independently checked stage inventory excludes those foreign changes. All
91 pre-existing changed/untracked canonical files were preserved during integration.
Only explicit implementation paths were committed. The shared checkout is not
claimed to be wholly clean.

## Observation boundaries

The predeclared observation lasts 600 seconds after paired bridge/Minime helper
verification. It selects the first two completed exact-new-prompt shared-study
exposures per Being, retaining failures, missing opportunities and baseline
pending requests separately. Unknown preparation clocks are not promoted to
post-cutoff preparation. No study, writing, model call, NEXT or Being message is
induced for this observation.

An authored correction survived six later synthetic responses and reader restart
in regression tests. That establishes preservation, not later use or understanding.
Natural follow-through must be reported separately with its actual denominator.

The window closed at **19:43:39.605405 UTC**, with four distinct Astrid delivered
study inputs and no Minime new-prompt study exposure. The prespecified close reading
contains the first two Astrid responses (normal stops, 201/535 completion tokens).
Unknown preparation clocks leave zero strictly timing-qualified trials; Minime has
two missing exposure opportunities. All 59 retained files passed hash/size checks.

Astrid chooses the exact router source after its map, then receives it and describes
the two listeners/rate-limit rejection. She also attributes one branch's topic
mapping to another and continues to expect health-gate behavior not established by
the supplied page. Neither selected response saves a correction or changes its
note/question/findings. The saved question is absent, so these inputs do not qualify
the new question-derived source-hint path naturally.

Minime completes a separate private-writing job at 19:35:25 UTC. A lookback study
block at 19:32:39 UTC explicitly names the existing low-fill action budget; it is
outside the window and is not a provider failure or proof of a prompt effect.
The shared generation lane must not collapse WRITE into source-study counts.
No exhaustive zero-failure claim is made from successful receipts.

Two follow-ups remain visible: older unthreaded findings are unrelated to Astrid's
current health/router question, and the map footer runs into NAVIGATION RECEIPT
without a newline. No malformed chosen command is observed. Preserve authorship and
access when improving relevance; do not treat interface delivery as understanding.

Operational evidence is under
`/Users/v/other/worktrees/study-direction-20260916/evidence`. The
[research account](/Users/v/other/reservoir-llm-research/analyses/2026-09-16-study-direction.md)
and private packet extend the historical sequence with actual denominators and
rollout boundaries. Board updates remain pending. Unrelated canonical work and
the concurrent schema-4 proposal remain preserved and outside this deployment.
