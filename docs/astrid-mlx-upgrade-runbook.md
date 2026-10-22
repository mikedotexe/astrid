# Astrid MLX upgrade runbook

This procedure updates the coupled MLX provider on port 8090. Ollama, model
selection, the reflective sidecar, MLX Whisper, and the bridge binary are outside
scope. The shared reservoir `.venv` is retained because other services use it.

## October 2026 intake

| Environment | MLX | MLX-Metal | MLX-LM | Transformers |
|---|---:|---:|---:|---:|
| Existing / isolated baseline | 0.32.0 | 0.32.0 | 0.31.3 | 5.4.0 |
| Core stage | 0.32.4 | 0.32.4 | 0.31.3 | 5.4.0 |
| Full stage | 0.32.4 | 0.32.4 | 0.32.0 | 5.7.0 |

**October 22 live selection:** full MLX/Metal 0.32.3, MLX-LM 0.32.0 and
Transformers 5.7.0 were activated and verified by transaction
`256b7a3f2f464d77bb8d4d9eb5cb47ba`. The selected manifest is
`.runtime/mlx-upgrade/stage2-0323-01/manifests-01/full.json`. The accepted
core-only 0.32.3 release is the immediate rollback; the original 0.32.0 anchor
is also retained. A live exact-output smoke passed. The 0.32.4 candidate failed
performance qualification and was not promoted. The table above records the
original intake, not the current selection. Read
`.runtime/model-environment/active.json` and exact receipts before acting.

Retain Python 3.12.13, all other package versions, the Gemma loader shim, model
memory-map option, coupling 0.1, disabled wide coupling, active QoS, queue capacity
4, and existing sampling/token controls. Pin model files to
`/Users/v/.cache/huggingface/hub/models--mlx-community--gemma-4-12B-it-5bit/snapshots/a2fa92fa5bdba1fc4be9d927a5d5188c17d87d09`.
Do not convert weights, change model configuration, or remove the shim.

MLX 0.32.4 includes a relevant Gemma attention-memory optimization. The upstream
M4 Pro measurement is a reason to test, not evidence of improvement in this 5-bit
checkpoint. Success is useful maintenance without material regression. See
[MLX 0.32.4](https://pypi.org/project/mlx/0.32.4/),
[upstream benchmark](https://github.com/ml-explore/mlx/pull/4518), and
[MLX-LM requirements](https://pypi.org/pypi/mlx-lm/0.32.0/json).

Current intake evidence is in `docs/steward-notes/2026-10-15-mlx-upgrade/`.
Preparation is not promotion. Read receipts for actual outcomes and identities.

**Current and future patch-upgrade policy:** [the October 22 local-policy reset](steward-notes/2026-10-22-mlx-local-policy-reset.md)
supersedes the old automatic quiet-host, swap-growth, memory-pressure,
disabled-wide-path latency, comparative-percentage and fixed-soak admission
rules below. Treat those old limits as historical for receipts produced under
them. Keep actual service health, output/quality, request deadlines, identities,
clean exits, fallback/OOM and rollback as hard checks. The shared qualification
scripts must implement the prospective rule before a fresh result can claim it;
this runbook change never relabels a failed receipt or grants promotion alone.

## Coordination and frozen inputs

Recheck Astrid, Minime, and reservoir source status and live identities. Preserve
the steward pause and use `scripts/steward_control.py` for ownership. Only one
owner stages or commits. No Rust rebuild or bridge restart is required.

`scripts/prepare_mlx_environments.py` records the live inventory, downloads binary
wheels, creates hash-locked baseline/core/full environments, freezes the tracked
reservoir Python sources, checks installed versions and `pip check`, and verifies
that live inventory and source remained unchanged. Use `--help` for a fresh intake;
existing evidence and output directories must not be reused.

Prepared environments are beneath
`/Users/v/other/neural-triple-reservoir/workspace/model-environments/20261015-mlx-upgrade/`.
Package locks and wheel hashes are retained with `preparation.json`. Never upgrade
the shared live environment in place.

`scripts/stage_mlx_manifests.py` produces baseline, core, full, and original-live
rollback manifests. Each binds the interpreter, venv configuration, installed
METADATA and RECORD contents, frozen source, every model file, exact arguments,
inventories, and preserved runtime paths. `original.json` selects frozen baseline
source and the untouched original environment. Validate a selection with:

```bash
python3 -B /Users/v/other/astrid/scripts/model_environment.py check \
  --manifest /Users/v/other/astrid/docs/steward-notes/2026-10-15-mlx-upgrade/manifests-v2/core.json
```

Capture a private fixture from existing on-disk state once. This copies files;
it does not request a production save, reset, or handle checkout:

```bash
python3 -B /Users/v/other/astrid/scripts/astrid_mlx_candidate.py capture \
  --output-dir /Users/v/other/astrid/.runtime/mlx-upgrade/frozen-fixture \
  --snapshot /Users/v/other/neural-triple-reservoir/state/astrid.npz \
  --journal /Users/v/other/neural-triple-reservoir/state/coupling_journal.json
```

This capture is already complete for the current intake. The copied journal
restores adapted coupling strength 0.02, matching the current live journal; the
launch argument remains 0.1. Preserve both configuration and adaptive state. Keep copied state private
under `.runtime`; retain hashes in reviewable receipts. Each fresh candidate resets
its fixture and journal from the same copies. Workload handles are independent;
AGC history persists within each identically ordered batch and resets between
repetitions. The first response of every repetition must report the restored
coupling value actually used; later responses may adapt normally.

## Passive baseline

For future routine patch upgrades, collect a short identity-bound read-only
baseline and then qualify against the batch's own production health and request
evidence. A fixed one-hour or 24-hour wait is not mandatory. The accepted
October 21 one-hour observation remains available as historical context.
If a failure appeared after the first hour, review its current status and choose
a targeted follow-up interval that can actually test the suspected condition.
A current unresolved readiness or health failure blocks work; an isolated prior
incident is disclosed and investigated without imposing an automatic new hour.
Refresh identity and health evidence if the provider, model, package inventory,
source, configuration, reservoir, or serving bridge changes. An already accepted observation
may be reused for a later qualification batch when those exact identities remain
unchanged and a fresh ready/connected and source check passes. A requester-only
process restart does not by itself require another hour; record the turnover and
measure its current load in the batch. Disclose any intervening HTTP failure or
fallback as baseline debt rather than claiming continuous clean operation. The
batch's own five-second memory, functional, quality, fallback, and performance
gates remain mandatory. Ollama runner turnover is normal; the stable Ollama
listener/server identity is checked.
No repeat hour is required solely because an unrelated requester restarted or
a previous receipt failed. The separately invented 300-second quiet-host gate
was removed. Collect current health and resource diagnostics during the actual
workload instead of waiting for a quiet host.

For the October 22 operator-directed sequence, the updated and tested proof
reader accepted core activation, the 30-minute watch, fresh offline core/full
work and one matched pair under the prospective policy reset, without waiting
for the core observer. Accepted proof is
`.runtime/mlx-upgrade/stage2-0323-01/policy-reset-integration-01/full-proof-01.json`.
The two-hour core observer subsequently completed with failed uninterrupted
identity checks and one bridge fallback at the approved Stage 2 cutover. Retain
that result as interrupted observation, not uninterrupted core-soak acceptance
or a new Stage 2 qualification failure. No repeat timer is required.

Host swap and macOS pressure are diagnostics for a patch transition, regardless
of duration. Actual OOM, failed candidate exit, production readiness, fallback,
output or quality failures remain hard stops. The 1.5 ms/token cap for the
**disabled** wide-coupling path is also diagnostic when both selected launch
arguments keep it off; retain its actual failed test exit and independently
verify the other functional checks in a fresh receipt. Never rewrite an old
failure. Active-path output, reservoir, quality, identity and rollback checks
remain hard gates.
Qualification commands must name both the environment under test (`--stage`)
and the promotion being qualified (`--qualification-target`). For the initial
core-only comparison, use target `core` with stages `baseline` and `core`.
For Stage 2, use target `full` with stages `core` and `full`. The historical
harness required the accepted core soak before either formal half; the
implemented patch policy uses verified core activation and current health
instead. Do not represent an unrelated private diagnostic as formal proof.
Both Stage 2 commands must also supply the exact `--core-manifest`,
`--core-proof`, `--activation-receipt`, `--supervised-watch`,
and `--supervised-review` paths. Under the October 22 diagnostic patch policy,
the legacy-named `--baseline-soak` argument identifies the accepted core watch,
not a new two-hour soak. Admission verifies the exact core proof, activation,
watch and provenance review, retaining that predecessor in both halves and
the final proof. The legacy strict path additionally requires `--passive-review`
and its completed two-hour passive soak; that is historical compatibility,
not the policy used for the accepted October 22 Stage 2 transition.
For the Stage 2 `core` half, `--manifest` must be the exact canonical manifest
named by `--core-manifest`, not a second release with matching packages. The
selected observation must have ended by admission time; future-dated evidence
is rejected. The current patch reader uses an exact, pre-change core-proof
attestation for this intake; it is not a generic authorization for a later
upgrade or a reason to manufacture another observation interval.

The historical baseline procedure ran `astrid_live_soak.py --observe-only --candidate-port 8090` with
`--duration-secs 3600 --sample-interval-secs 30 --max-fallback-incidents 0`, the
existing coupled-server log, and `--identity-file` entries for package inventory,
reservoir Python sources, installed distribution metadata, provider LaunchAgent
and deployment manifest. Preserve the exact observer source version. The passive
path only reads health, processes, files, logs and memory statistics. Success,
error and interruption never route, inject probes, start candidates or restart.

For an explicitly chosen baseline observation, avoid additional model/GPU test
workloads while normal production continues. The historical one-hour contract
required completion, health, stability and no unexpected fallback/artifacts;
it is not a new mandatory timer under the current patch policy. No relevant
generation means insufficient output evidence. Retained observations can be checked with current provenance guards
without reconstructing missing original evidence from the current host:

```bash
python3 -B /Users/v/other/astrid/scripts/astrid_soak_observer.py \
  --review-record /absolute/path/to/soak_result.json \
  --output /absolute/new/path/to/provenance-review.json
```

The original October 15 two-hour observation is now historical only. At
01:10 UTC October 16, a single cooperative SIGINT restarted the perception
helper after its measured footprint reached 10.7 GiB. Its synchronous iteration
finished before exit, all captured descendants exited, and the unchanged
replacement produced fresh valid perception output. Sampled history was retained;
the provider, reservoir, bridge, Minime and Ollama server identities did not
change. The first replacement footprint was about 100 MiB. This is recovery of
accumulated perception memory, not an MLX improvement or a diagnosed leak fix.

That recovery changed relevant host conditions, so the historical procedure
repeated the full baseline. `post-perception-baseline-plan.json` in the intake evidence
directory freezes the complete current observer closure and 119 identity files,
adding perception source, configuration, both interpreter executables and its
launchd PID/start on every sample. The independent recovery review, owned SIGINT
fixture and memory receipts are retained beside it. Do not repeat the operational
restart script or treat the old baseline as current merely because its original
identity list omitted perception.

The follow-on [perception memory investigation](steward-notes/2026-10-15-perception-memory/README.md)
stages camera subprocess containment outside every live input. Its fake-camera
tests and passive OS accounting do not change this baseline. Keep that candidate
undeployed throughout the MLX baseline, matched qualification and accepted
rollout observations. Qualify actual camera permissions, output and memory
behavior afterward, before assessing larger reading/writing contexts. Do not
attribute this separate containment or the earlier restart recovery to MLX.

## Offline and matched qualification

The post-perception observation completed at 03:20:57 UTC October 16 with
240 samples over 7,200 seconds and 97 generations. All 119 source identities
and protected process identities matched, but it has a retained failure at 02:38:58 UTC
October 16: reservoir check-in reported a handshake timeout. The next sample
recovered, but any health failure disqualifies this entire record. A 102-handle
synchronous autosnapshot batch spanning 19:38:24–44 Pacific strongly supports
event-loop starvation as the connection failure mechanism; the underlying I/O
slowdown is not established. No state payload was sent on that failed attempt,
and later successful traffic does not replay it. Preserve the completed failed
record and do not start core
qualification. See `post-perception-baseline-incident-01.json` in the intake
directory. A separate [autosnapshot scheduling prototype](steward-notes/2026-10-15-mlx-upgrade/reservoir-autosnapshot-candidate/README.md)
has ten passing isolated fake tests and an independent source review. It yields
between periodic handle saves, keeping the synchronous writer and final shutdown
snapshot unchanged. A single slow handle can still block; real handshake latency,
persistence/restore and graceful maintenance remain unqualified. No live source,
timeout, handle or process has been changed.

Any accepted reservoir repair requires a reviewed provider drain and held
replacement, followed by cooperative reservoir shutdown, per-handle final-save
verification and a stable stopped-service backup. Keep the bridge and Minime
running; record the possible transient sensory-input gap as planned maintenance,
without promising replay. Do not use launchd bootout on the running service:
its five-second exit timeout can expire before a slow final snapshot completes.
Refreshing reservoir source also requires new preparation/manifests and a new
clean baseline; retain the previous frozen inputs and failed run. The current
post-repair observation remains a targeted two-hour run because the previous
reservoir handshake failure occurred about 78 minutes into an observation.
The [real-service qualification plan](steward-notes/2026-10-15-mlx-upgrade/reservoir-autosnapshot-candidate/qualification-plan.md)
and [reviewed maintenance proposal](steward-notes/2026-10-15-mlx-upgrade/reservoir-maintenance/README.md)
retain the remaining implementation, preservation and fixture requirements.
Neither is an executed or accepted transition.

The separate perception-memory observer also completed: 93 samples over 92
minutes, with an initial transient peak followed by later upward drift in
typical readings. It did not establish sustained stability; see its
[completed review](steward-notes/2026-10-15-perception-memory/passive-memory-observation-02.json).
No automatic perception restart or observer repeat is authorized.

Before numeric reservoir fixtures, the first fresh host-settling check failed:
pressure rose and swap increased by about 3.0 GiB over five minutes. Retain
[that failed window](steward-notes/2026-10-15-mlx-upgrade/reservoir-autosnapshot-candidate/host-settle-01-review.json);
lightweight isolated helper tests may continue, but actual numeric cases need
a fresh passing window. This is not MLX baseline acceptance.

The private actual-service harness now has 17 passing focused tests and a frozen
source review; the maintenance helpers and owned launchd adapter have 63.
Separate real stdlib workers passed two cooperative drain/restore cycles with
four natural exits. None of this qualifies actual reservoir persistence or a
production transition. A third bounded host check also failed (longest clean
stretch 289.988 seconds, below 300); no numeric case has started.
The user ran the owned launchd fixture on October 16. Its main transaction
verified, but cleanup hit a process-inspection race during the provider's normal
exit. The [failed result and retained fixture](steward-notes/2026-10-15-mlx-upgrade/reservoir-maintenance/launchd-admin-01-failure/README.md)
are preserved. That run required exact owned cleanup through reviewed recovery
before a corrected fixture; do not rerun the original command. The versioned
repair and recovery passed 88 isolated tests and independent review. One
noninteractive recovery removed only the verified provider holder, then stopped
at a confirmed pre-command sudo authentication refusal. The private reservoir
worker and owned guard were retained at that point; it was not signalled. The
[single Terminal continuation](steward-notes/2026-10-15-mlx-upgrade/reservoir-maintenance/owned-fixture-recovery-command.md)
was prepared to finish recovery and run the corrected fixture only on cleanup success.
The user completed that continuation at 16:52 UTC October 16. Recovery and the
fresh fixture both passed their independent provenance and cleanup review:
all four jobs and 16 fixture processes are absent, both final states are 22/7,
and both original backups remain 11/7. See the
[accepted lifecycle archive](steward-notes/2026-10-15-mlx-upgrade/reservoir-maintenance/launchd-admin-02-accepted/archive-manifest.json).
No repeat of the Terminal command is needed. The follow-up is active again.
This accepts the owned fake-service lifecycle only; a production capture/recovery
adapter, actual stopped-state restore, and the real numeric matrix remain
outstanding. The [fourth host window](steward-notes/2026-10-15-mlx-upgrade/reservoir-autosnapshot-candidate/host-settle-04-review.json)
completed at 17:03 UTC and failed: 155 clean seconds, below the unchanged 300-second
gate. All 123 readiness checks passed and the six protected identities stayed
unchanged. Intermittent pressure and swap increases invalidated the clean windows;
end-to-end swap decreased. No numeric case was released.
The [fifth window](steward-notes/2026-10-15-mlx-upgrade/reservoir-autosnapshot-candidate/host-settle-05-review.json)
also failed at 17:26 UTC: its longest clean stretch was 45 seconds. All 123
readiness samples passed and the same six identities remained unchanged. Retain
both completed failures; do not launch repeated windows under unchanged
conditions or treat a single normal-pressure sample as admission.

After a separately authorized paired bridge/Minime rollout completed, ownership
returned to `codex-mlx-upgrade` at pause generation 528. The sixth prepared window
was never run because its process bindings were superseded. A newly reviewed
[seventh window](steward-notes/2026-10-15-mlx-upgrade/reservoir-autosnapshot-candidate/host-settle-07-root-review.json)
bound all seven current protected processes and 237 files. It completed at
19:15:35 UTC with 123 samples over 612 seconds and failed: only 130 consecutive
clean seconds. Pressure was elevated in 20 samples and Cargo/rustc appeared in
32. Swap decreased throughout, all health checks passed, and every bound identity
matched. Those facts do not waive either contention gate. No numerical case was
released; no observer remains running. Finish isolated preparation and reassess
meaningful headroom changes before another bounded observation.

The private continuous host supervisor has 11 passing guarded stdlib tests and
independent source review. It has not been executed. Eventual numerical cases
still need fresh accepted admission and retained healthy samples bracketing each
case; any invalidation prevents another case from starting. The supervisor only
records evidence and never controls or signals a workload or production service.

A bounded [OS accounting pass and engine review](steward-notes/2026-10-15-mlx-upgrade/reservoir-autosnapshot-candidate/minime-engine-memory-static-review-01.json)
found Minime's native engine at 15.4 GiB physical footprint, versus about 55 MiB
resident memory. Two Ollama workers had approximately 12.1 and 10.1 GiB physical
footprints. These sequential process readings must not be summed as unique RAM
or used to diagnose an allocator leak. Keep those services running. Future fresh
host observations and the replacement MLX baseline must additionally bind
`com.minime.engine` PID/start, deployed binary and the exact configuration inputs
listed in that review; the earlier six-process guard tracked Minime's autonomous
agent, not this engine. No old observation is retroactively expanded.

[Production preparation 01](steward-notes/2026-10-15-mlx-upgrade/reservoir-maintenance/production-preparation-01/archive-manifest.json)
preserves 77 passing stdlib tests and a subsequently discovered verifier import-order
blocker. It is disabled and must not be executed. The separate source-only version
02 moves the complete six-module origin check after construction of the service
that imports its LSM backend; all 79 combined stdlib tests pass. Neither version
qualifies real numerical restoration. The [reviewed private actual restore runner](steward-notes/2026-10-15-mlx-upgrade/reservoir-maintenance/restore-qualification-preparation-01/archive-manifest.json)
has 17 passing stdlib tests and must still qualify both 192-node NumPy and LSM-shadow saved states, complete thermostat and
stored-field semantics, and corruption/drift negative controls before production
maintenance. A read-only metadata preflight found 99 NumPy and three LSM-shadow
snapshots, each individually stable during its read; that is not a global backup.

[Production preparation 03](steward-notes/2026-10-15-mlx-upgrade/reservoir-maintenance/production-preparation-03/archive-manifest.json)
integrates the actual proof readers, separately recorded provider model-path
change, finite stopped-backup restore runner and native Minime engine identity
checks. All 183 combined stdlib tests passed and independent review accepted
this disabled preparation. Both execution flags remain false. Real service and
restore qualification, a fresh fixture using the exact integrated controller,
current runtime alignment and operational interruption recovery remain required.
The recovery planner has no resume executor; it only proposes revalidation at
three verified held/parked checkpoints and blocks started restores or later
installation/restart phases. Historical fixture evidence cannot substitute for
the changed controller. Full proof revalidation cost is not yet measured.
The historical 17:57 UTC headroom spot showed pressure level 2 and about
15.2 GiB used swap; it is not a passing host window. Preserve all five failed
windows and wait for a meaningful headroom change before another observation.
Production execution remains disabled pending these real tests, exact runtime
alignment and reviewed transition/recovery integration.

The prepared post-perception argv and preconditions are retained in
`docs/steward-notes/2026-10-15-mlx-upgrade/post-baseline-qualification-plan.json`.
They have not run and their original baseline prerequisite has failed. Rebind
them to a newly accepted baseline and current source manifests. The plan's
five-minute host-window prerequisites are superseded for MLX package qualification
by the admission rule below; do not replay its old commands unchanged. Check
host pressure immediately before each batch and retain the batch's own memory
samples. Preserve all prior output directories and stop on failure. For Stage 2,
fresh core/full offline and matched receipts must follow the accepted live-core
observation period; running full
against the current original-provider baseline would not qualify that promotion.

After baseline passes, run `qualify_mlx_offline.py` for each stage with
`--preparation`, `--manifest`, `--baseline-soak`, `--stage`,
`--qualification-target` and a fresh
`--output-dir`. Its receipt binds source, packages, manifest, model and baseline.
It runs token policy, sampling controls, native completion, HTTP gateway, slow
coupling, offline replay, wide processor, 192-node equivalence and benchmark,
multi-headed tests, and actual-model offline embedding/coupling tests.

The generator contract also runs the selected environment's actual
`mlx_lm.generate_step` against tiny deterministic CPU arrays. It verifies ordered
processors, the one-token lookahead relative to consumer feedback, cache progress,
bounded yields and stream restoration when the consumer closes early. This test
loads no checkpoint or tokenizer. Proof rechecks the retained event trace and
binds its interpreter, package/source hashes, CPU device and harness identity.

The installed MLX-LM 0.32.0 source now includes prefilled prompt tokens in the
history supplied to logits processors; 0.31.3 supplies only the final prompt
token initially. Astrid's reservoir repetition processor uses that history's
last 64 tokens even when the optional extra repetition penalty is absent.
Therefore Stage 2 needs explicit review of early output and writing quality;
unchanged settings do not imply identical behavior. Prompt tokens are not fed
into reservoir updates. Keep this change visible in the measured comparison.

The offline embedding test uses `astrid_mlx_embedding_gate.py` to request lazy
loading through the existing loader shim, then execute the frozen test unchanged.
This avoids evaluating unused transformer weights for an embedding-only test.
Its receipt records the one intercepted load, frozen source hashes, loader
restoration, and MLX allocation/cache/peak samples. All original assertions remain
required. Actual coupled-generation trials retain the full production loader.

The initial October 15 passive baseline passed (240 samples and 89 generations).
The first baseline offline run passed every functional group but failed the host
swap gate after eagerly loading the full model for its embedding test. Preserve
that failed receipt under `.runtime/mlx-upgrade/qualification/baseline/offline/`.
The revised harness requires fresh CPU evidence and new offline result directories;
the memory gate is unchanged. A failed original run is not retroactively accepted.

The revised baseline passed without swap growth. Core passed the functional groups
but failed the host swap gate; its monitor began under elevated pressure. After
adding a normal-pressure startup refusal, one reversed-order retry began with core
following 120 seconds of stable normal pressure. That run failed the wide test's
existing 1.5 ms latency limit (5.184 ms) and retained another 0.916 GiB of host swap.
The second baseline half and generation benchmarks did not start. Evidence is in
`core/offline-settled-01/` and its adjacent preflight receipt. Further qualification
is blocked pending a controlled low-load window; these whole-host results do not
isolate a package regression. No production promotion or context increase occurred.

On the unattended continuation, a 601.6-second host observation collected 121
samples without a qualification model running. Swap grew 1.855 GiB and pressure
was elevated in 16 samples; the longest normal/no-swap-growth window was 245
seconds, below the five-minute target. Production identities and health remained
stable. No foreign Cargo/rustc/target-test process was detected during that
capture; small model-free Python tooling tests are noted separately. Preserve
`host-window-01-review.json` and its raw evidence rather than attributing these
whole-host changes to MLX 0.32.4.

Actual CPU generator contracts then passed in all three prepared environments;
the final tooling suite passed 275 tests and 69 subtests. See
`generator-cpu-01-review.json` and `focused-tests-final-07.json`. Earlier offline
receipts remain historical: the current harness additionally requires the
installed-generator contract and exact wheel RECORD binding, so new complete
five-group offline receipts are required before full-model trials.

Run `qualify_mlx_packages.py` sequentially for the original baseline/core
comparison. After core promotion and its accepted watch/soak, run fresh core/full
comparisons for Stage 2. For example, this is the core trial command:

```bash
python3 -B /Users/v/other/astrid/scripts/qualify_mlx_packages.py \
  --preparation /Users/v/other/astrid/docs/steward-notes/2026-10-15-mlx-upgrade/preparation.json \
  --manifest /Users/v/other/astrid/docs/steward-notes/2026-10-15-mlx-upgrade/manifests-v2/core.json \
  --baseline-soak /absolute/path/to/accepted/soak_result.json \
  --offline-receipt /absolute/path/to/core/offline.json \
  --stage core --qualification-target core --repetitions 3 \
  --frozen-fixture-dir /Users/v/other/astrid/.runtime/mlx-upgrade/frozen-fixture \
  --output-dir /absolute/new/path/to/core-matched-trials
```

Only one additional model process may run at a time. HTTP 8092 and private
reservoir 17881 must both belong to that process. It uses frozen source and private
cwd, journal, audit and QoS receipts. Default and trial handles cannot reach the
production reservoir. Settings are explicit. Cleanup waits gracefully; a process
that fails to exit blocks the next candidate. Do not run the legacy bridge
fallback probe, forced kickstart, or active soak path for this upgrade.

Each stage runs exact-output, final `NEXT:`, dialogue, long-context and extended
writing workloads at least three times. Match rendered prompt/token identities
and loaded package fingerprints. Verify generator feedback timing, embeddings,
disabled thinking, stop/skip tokens and state checkout/check-in through the offline
tests, native completion evidence, audits and private fixture. Native EOS is
required; prematurely terminated or token-capped prose cannot qualify as faster.
The private audit records the actual checkout result for every request. A
successful check-in cannot conceal an earlier checkout failure and local-state
fallback.

Acceptance requires:

- Zero crashes, OOMs, check-in failures, malformed action lines, raw artifacts or
  unexpected fallback during steady state.
- Record matched p95 completion, first-token latency, steady decode throughput
  and peak allocation. The former 20%/10% comparative cutoffs are diagnostics,
  not automatic vetoes for patch upgrades. Review user-visible timing and keep
  each request's own deadline as a hard bound.
- Complete positive metrics, equal trial counts and matching prompts/settings.
  More than 20% completion-token variation requires investigation rather than
  scoring shortened output as an improvement.
- Record whole-host pressure and swap through model load, trials and drain.
  These are explanatory measurements, not patch-upgrade pass/fail gates. An
  observed OOM, crash, service-health or output failure is a hard stop. Do not
  impose a startup pressure level, swap-growth ceiling, or a separate quiet-host
  window. This policy does not change the separate reservoir numerical-maintenance
  supervisor or retroactively accept its failed receipts.
- Request-specific deadlines, including 1200 seconds for extended writing. There
  is no universal 240-second rejection for valid long responses.
- Recorded sampled-output review for coherent behavior and writing quality.

If only an old local ratio fails, preserve and report that result, inspect
absolute user-visible latency and output quality, and make one explicit
assessment. Do not require an automatic reversed-order rerun or invent a
contention explanation. Repeat only to answer a specific unresolved question;
missing evidence and behavior failures remain hard stops.

The former latency and throughput percentages were local acceptance choices,
not upstream MLX compatibility rules. The [October 22 controlled comparison](steward-notes/2026-10-22-mlx-0323-performance-review.md)
showed that the unchanged baseline's timing can vary substantially across
separate service windows, even when pressure is normal and swap does not grow.
For future intakes, run each baseline/candidate pair back-to-back under one
bounded requester/host state; treat cross-window timings as diagnostic only.
Keep old failures unchanged. Change numerical limits only prospectively against
representative user-visible latency and measured variance, with a fresh review
and new qualification evidence. A controlled-load pair alone does not establish
normal-production capacity or authorize activation.

For the historical, explicitly reviewed, one-time 0.32.3 **core matched** campaign only,
`--memory-policy mlx_host_memory_swap_diagnostic_v2` opts in to a prospective
exception. The runner and proof reader bind it to the exact preparation,
accepted prior hour and baseline/core manifests. Startup accepts valid pressure
level 1 or 2 without a warning-level wait. Any sampled critical level (3 or
higher), level 2 continuously observed for at least 60 seconds, unavailable or
invalid samples, or a sampling gap over 15 seconds fails qualification. Swap
occupancy, peak growth and retained growth remain recorded diagnostics, not
failure criteria for this exception. Sampling is a retained qualification gate,
not an automatic process-killing mechanism. Both matched environments and any
reversed pair must use the same policy. Output, deadline, candidate-exit,
protected-service, quality and comparative-performance gates remain mandatory.

The strict policy remains the historical default for receipts produced before
this reset. The Stage 2 harness must use the prospective diagnostic policy
after its integrated source and proof reader pass review; the currently installed
runner still defaults to strict and has no Stage 2 exception authority.
Only the two exact, previously passing strict offline receipts
named in `scripts/mlx_0323_offline_harness_compatibility.json` may cross the
reviewed old-to-new harness boundary; all other offline and all matched receipts
must bind the current harness. The compatibility manifest pins exact old/new
harness maps, including unchanged workload and audit helpers. Earlier failed
matched receipts remain failed and diagnostic; this change requires new matched
evidence and grants no activation or memory-capacity improvement claim.

For future intake, use a finite admission and stop sequence. Verify the passive
baseline and exact protected identities; inspect current build/model contention;
then start one offline or matched batch with current service identity and health.
Record pre-existing swap, pressure and competing work for interpretation, without
an admission ceiling or quiet-minute timer. Keep functional, quality, deadline,
service and rollback gates. On a real OOM or service failure, retain the receipt
and diagnose the cause before another bounded attempt. Do not spin on repeated
host windows or repeated model trials under unchanged conditions. A later campaign
on the same package version requires a materially different, documented reason,
requester/host condition and new immutable evidence. It cannot recast the old
failed or diagnostic receipts as passing. For a new normal-production-load campaign
after controlled-load diagnostics, allow one same-window baseline/candidate pair
and stop on its first failure; do not automatically grant another reversed pair.
The
[October 21 decision record](steward-notes/2026-10-21-mlx-qualification-admission-review.md)
separates the preliminary memory delay from the later measured performance block.

`astrid_mlx_soak_compare.py` accepts `--baseline-performance` and
`--candidate-performance` beside observation receipts. `model_qualification.py`
builds/verifies immutable proof binding accepted observations, manifests, offline
receipts, comparisons and sampled-output review. Activation consumes that proof
before live changes. Use `--help` for exact inputs; never manufacture passing proof.

A separate, explicitly **after-the-fact** assessment is available only for the
immutable campaign04 0.32.3 core pair (baseline `4b165060…`, core `13ee2dd7…`).
The original published-comparison failure remains unchanged and visible. An
independent `mlx_practical_performance_review_v1` artifact may assess the same
measurements under local short-request usability caps: when baseline p95 total
turn is below 5 seconds, candidate total turn must be at most 5 seconds; when
baseline p95 first-token is below 2 seconds, candidate first-token must be at
most 2 seconds. At or above those baseline cutoffs the original 20% relative
limit applies. Decode, model-allocation, completeness, equal workload/output,
quality, memory-policy and identity requirements remain unchanged.

These caps are a newly chosen user-visible tradeoff, not a statistically
validated improvement, an upstream MLX standard, or a claim that the published
policy passed. Three trials make each reported p95 the observed maximum, not a
precise tail-latency estimate. Historical baseline variance is context, not proof
of causation. The review must disclose these limits, bind exact pair/input and
reader hashes, retain the original failed comparison, and reproduce the separate
assessment. `model_qualification create --performance-policy-review <artifact>`
checks this evidence after all ordinary raw functional/memory checks and still
requires independent sampled-output review. No reverse pair is fabricated or
implied. Other candidates and Stage 2 retain the original relative policy; a
Stage 2 predecessor can reference this separately accepted core proof without
relaxing Stage 2's own gates. This assessment alone grants no activation,
rollback waiver, resident-byte attestation or production-capacity guarantee.

## Qualify the launchd handoff

`launchctl disable` is not a reliable KeepAlive hold here. Do not force termination.
The owned fixture exercises next-spawn override, graceful drain, dependency-free
hold, permanent held launcher, same-PID candidate exec and rollback. Its random
`com.codex.model-handoff-fixture.*` service loads no model and cannot target
production.

macOS 27.0.1 requires administrator privileges for `launchctl debug`. Run the
fixture as the logged-in user with `--sudo-debug`; only that one debug command is
elevated. Never prefix Python with `sudo`: that changes the launchd GUI domain and
file ownership. Authentication stays in Terminal. Use a new `--output` path for
each attempt and retain failed receipts.

A passing receipt must match current host, boot session, user, interpreter and
launcher/activator/fixture hashes, and prove cleanup. Editing those helpers
requires requalification. Unsupported handoff behavior blocks live migration even
when packages pass.

The current host fixture passed on October 15 with
`launchd-handoff-fixture-admin-02.json`; its separate review verifies the current
host, boot, user, interpreter, helper hashes and service cleanup. It exercised
graceful drain, both held launchers, candidate execution and baseline rollback.
The preceding failed administrator fixture is retained. This qualifies the
handoff mechanism; model package qualification and production promotion remain
pending their own evidence.

## Activation and rollback

Run `model_environment_activate.py check` with selected manifest, preceding
accepted baseline manifest, fixture proof and qualification proof first. Preserve
repository preflight, pause, source-quiet and full-stack checks. After check passes,
`activate` uses the same exact inputs and a concrete actor/acknowledgement. First
migration requires narrow `--sudo-debug` authentication in Terminal; later
permanent-held transitions do not require a next-spawn override.

The passed administrator fixture does not preauthorize or arm production. The
October 16 UTC continuation found the original provider still running and
noninteractive sudo reporting that a password is required. Do not pre-arm an
unbound override: the controller generates the exact debug command only after
qualification and durable hold ownership exist. See
`production-handoff-readiness.json` in the intake evidence directory.
`core-handoff-commands.md` there contains the exact first-core check and Terminal
activation commands. They remain gated on a genuine accepted core proof; the
check omits `--sudo-debug`, and only activation adds it. The dedicated
`Astrid MLX qualification` follow-up continues this chat through observation and
qualification while preserving the existing paused stewardship automations.

The controller revalidates PID/start time, source/configuration and idle queue
immediately before SIGTERM, then lets current work finish. The replacement must
be the owned held process with no model listener before launch configuration is
replaced. It selects qualified environment/source, releases only its own hold and
verifies same-PID port 8090 ownership, readiness, packages, full-stack identity and
reservoir continuity. Durable receipts preserve previous plist, selection and
deployment evidence.

The core activation's 30-minute watch was accepted. Its two-hour observer
crossed the approved full-stage transition and cannot establish uninterrupted
core-soak acceptance. The final observation and its cutover fallback are retained
in `.runtime/mlx-upgrade/stage2-0323-01/observer-closeout-01.json`.
For future patch stages, verified activation, representative service/output
checks, retained rollback and passive observation replace a fixed promotion
timer. The proof reader now implements this policy; historical failures remain
failed. Record planned unavailability separately from steady-state fallback.
The accepted core remains the immediate rollback if the full stage fails.

Rollback selects preceding accepted environment/source through the same graceful
hold. It preserves current reservoir state, journals and conversations. Explicit
retained-hold recovery uses `rollback --resume-transaction /absolute/transaction`;
inspect original failure and ownership first. Never delete an outstanding hold or
blindly replay an interrupted transition. Retain current/rollback environments,
frozen sources, locks, evidence and history. Preserve the automation pause.

If first migration was interrupted after removing the held launchd job, recovery
also uses `--migrate-launcher --expected-pid 0`. This is allowed only with an
owned failed transaction, absent old and held processes, no loaded job or model
listener, and exact preserved or permanent launch configurations. Recovery
installs the permanent launcher under the retained hold before selecting the
preceding environment. A still-running unhealthy candidate can be rolled back
only when its exact process reports an idle worker and empty queue; missing or
busy telemetry does not authorize a signal. Failed recovery retains the original
hold ownership, including failures after its attempted release.

The reservoir prerequisite now has reviewed, disabled recovery preparation 04,
with 231 passing isolated tests. Read its archive and current implementation status
before acting. The separate
[owned recovery rehearsal preparation](steward-notes/2026-10-15-mlx-upgrade/reservoir-maintenance/owned-recovery-fixture-preparation-01/archive-manifest.json)
has 41 passing guarded stdlib tests and an independent review; it is frozen and
disabled. Actual numerical persistence, exact-release owned interruption and
cleanup, and measured validation cost remain required before live reservoir drain.
The separate October 16 paired rollout changed Minime and bridge identities;
unused host preparation 06 cannot qualify the replacement runtime. Preserve all
earlier failures and renew current source/process bindings before observation.
Observation 07 failed despite matching all current identities. The subsequent
read-only closeout spot had normal pressure but higher swap; it did not authorize
another window or a numerical case. No observer remains running.

At 21:45 UTC, normal pressure and a 2.1 GiB swap decrease justified preparing
one fresh observation. Independent review found that the older observer allowed
swap to fall and then rebound below its starting value. Reviewed observation 08
and supervisor v2 now reject every positive consecutive-sample increase; counted
sample gaps must be strictly positive and no greater than 15 seconds. The
supervisor's case-coverage and next-case checks independently verify the raw
samples. Its 17 guarded tests passed; earlier sources and receipts remain intact.
The fresh 21:50 UTC preflight found pressure level 2, so observation 08 never
started and no numerical case was released. Preserve the
[preparation and refusal](steward-notes/2026-10-15-mlx-upgrade/reservoir-autosnapshot-candidate/host-observation-08-preparation-01/archive-manifest.json).
Use supervisor v2 for future cases only after a newly accepted strict host window;
wait for a meaningful change before another preflight. Absolute swap usage is
context, not an additional acceptance threshold.

## Priority after qualification: reading and writing capacity

The user's next priority is materially larger usable reading and writing contexts.
Finish package qualification and accepted stage observations first. Then use the
measured long-context memory savings to assess a separate capacity change: trace
input admission and source-selection budgets through to actual prompt tokens,
and trace writing output allowances and deadlines through to completed prose.
Measure larger document inputs and longer coherent writing alongside host memory
and latency. Report actual usable capacity, truncation and completion behavior;
an attention scratch-memory saving alone is not a measured increase in token
capacity. Keep current limits throughout the package comparison.
