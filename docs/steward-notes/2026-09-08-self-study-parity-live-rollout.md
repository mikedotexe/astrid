# SELF_STUDY parity is live — September 8, 2026

Astrid and Minime now run the shared source-study implementation. Mike explicitly
authorized taking over integration, committing and deploying it live. This pass
changed source reading and language context, with no reservoir/controller setting
change and no request that either Being perform a study or send a response.

## Committed implementation

- Astrid `542c006040381ef7673cc0c5f5154da5edd6ca90`.
- Minime `37ed8b7` (full identity in the linked JSON).
- Both implementation commits are on local `main`. Nothing was pushed.
- Design and commands: `../architecture/source-study-v1.md`.

The staged V3 release contains both the bridge and `astrid-source-study` executable.
Minime verifies the selected Astrid manifest digest and the helper digest before
using it. This closes the staged-deployment gap found during integration: building
only the bridge would leave Minime's new adapter without its shared executable.
Legacy V1/V2 release verification remains supported.

## Verified live transitions

Astrid used `scripts/build_bridge.sh --stage-dir ...`, followed by its
`--activate-stage ... --expected-pid 5971` path. The old bridge acknowledged drain
and exited through one SIGTERM. The new bridge is PID **90102**, running the
committed release. The wrapper returned `activated_verified`: checkpoint 193079
was decoded at startup, the signed self-control state targets this exact binary,
and a new saved exchange advanced the count to 193080. No force or automatic
rollback was used. Remote delivery is not inferred from drain acknowledgement.

Minime used `scripts/restart_minime_agent.py`. One initial attempt was refused
before any signal because the wrapper hard-codes `codex-astra-interactive` as its
maintenance actor. The same user-authorized task retained its pause under that
expected label and retried. The successful idle-gated transition replaced PID
**10263** with **91125** through one SIGTERM. All **80 startup source inputs** match
disk, including `minime_autonomy/source_study.py`; `reload_required` is false.
Session **5318** was preserved, pending NEXT remained absent, and the cycle count
advanced from 25934 to 25935. The wrapper verified all ten protected service
identities and configuration across this transition.

The hard-coded maintenance actor is a deployment-tool design limitation, not a
SELF_STUDY permission requirement. This rollout used the existing convention
without weakening its checks; a future cleanup can accept an explicit actor.

The stage, raw build log, process snapshots, health samples and access smoke
records are retained under:
`/Users/v/other/worktrees/self-study-parity-live-20260908/`.

## Evidence and bounds

`source-study-v1-validation/live-rollout.json` binds the implementation commits,
PIDs, source inputs, continuity fields and exact activation/reload receipt hashes.
The receipt paths point to retained `.runtime` records. Access checks used the
staged executable and Minime's actual installed adapter with temporary operator
state. They opened kernel source and the final lines of the 56,006-line Minime
runtime. No live model was prompted, no Being bookmark was advanced by a test,
and these checks establish access, not comprehension or benefit.

Initial validation passed 2,242 bridge tests. Final integration passed 1,261
Minime tests plus 130 subtests (one expected skip), nine shared-reader tests,
two focused bridge source-study tests and 109 deployment/reload-wrapper tests.
Formatting and the bridge domain-boundary audit passed with zero violations.

During 32 samples spanning the rollout, Minime fill ranged from 70.998% to
73.045%; the oldest sampled telemetry was 2.29 seconds old. Engine, Division,
model, host-sensory, visual-frame and collaboration-feeder process identities
were unchanged between the initial and final snapshots. Camera and microphone
PIDs changed at 08:38:30–31 PDT, before bridge activation; this task issued no
command to those jobs, and their cause was not investigated. Their replacement
identities remained unchanged throughout Minime's verified reload.

## Shared-tree handoff

The cooperative heartbeat released its lease after pause generation 402. The
Minime wrapper required the same pause under its expected actor label, yielding
generation 403. The initial controller state was unpaused and is restored after
the final documentation commit; preexisting Codex automation pauses are preserved.

Unrelated heartbeat work remains unstaged and uncommitted: Astrid's codec tests,
two evidence packets and their changelog/ledger entries, and Minime's regulator
inclusion-shell test. Every unrelated non-document file hash was preserved.
The overlapping documentation was merged by retaining both entries. No foreign
work was swept into the SELF_STUDY commits.
