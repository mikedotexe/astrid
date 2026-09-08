# Provider-attempt evidence

This opt-in steward observer supports reservoir-llm-research S-006, following
the journal-led marker-preservation repairs. The research follow-up found that
ordinary generation records contain already-cleaned responses. They cannot
establish how often a repair had a natural opportunity to help. This is a
human-authorized observation extension, not a new fix attributed directly to
an authored journal passage, and not a claim of experienced benefit.

## Enablement and rollback

Source preparation does not enable the running bridge. A separately authorized
owning rollout can set `ASTRID_PROVIDER_OBSERVATION=on` (`1` or `true` also
accepted) before process startup. It defaults off. The setting is read once per
process. `ASTRID_PROVIDER_OBSERVATION_DIR` selects a dedicated private spool;
otherwise it is `workspace/provider_observations`. No prompt reads this spool.
Disable the setting through the owning release workflow to roll back; retain
all evidence. There is no observer-driven restart, schedule or live control.

The launch wrapper reads the operator-owned
`workspace/runtime/provider_observation.env` before importing launchd overrides.
For durable enablement, write `export ASTRID_PROVIDER_OBSERVATION=on` and an
exported `ASTRID_PROVIDER_OBSERVATION_DIR` pointing at a dedicated private spool.
Keep this configuration private (0600). An absent file retains the default-off
behavior. A launchd override takes precedence, including `off`; clear stale
overrides when changing the durable decision. Settings take effect only at a
sanctioned process transition. Rollback sets the durable value to `off`, clears
both launchd overrides, and uses the same reviewed release workflow.

Use a release launched with the existing verified `--deployment-manifest`
binding. The observer reuses that immutable startup verification of manifest
and executable. It records the same binding before and after each attempt.
Without it, activation is explicitly unknown: a Git revision or mutable build
manifest alone is insufficient. These fields identify the process build, not
current service health or authority to deploy it. Raw manifest and binary hashes
must still be independently checked when assembling a research window.

## What is recorded

Every entry into a provider `send` call gets a fresh attempt ID and an immutable
dispatch envelope. A dispatch receipt means the client began an attempt, not
that a server accepted it. Each Ollama fallback model call has its own identity;
retries with identical text remain distinct. Before dispatch is reached, request
preparation/admission failures are outside this denominator.

The outcome envelope records HTTP/transport/timeout/parse/missing-message
failures or provider output, including empty, degenerate, profile and incomplete
response rejection. Successfully parsed message content is observed before
cleanup and downstream rejection. The special protected Ollama incomplete path
is observed even though it returns before the usual cleanup hook. Normal
cleanup reports reuse the existing bounded counts, byte offsets and hashed
context receipts. No known markers produces an explicit zero; unavailable
input leaves the count null. Drop emits `cancelled_or_abandoned` on graceful
future cancellation. An abrupt process loss may leave only a dispatch receipt;
consumers must preserve that unknown rather than discard it.

Exact stages and hashes are separate:

1. HTTP body hash (when fully read), without retaining the wire body.
2. Parsed message-content hash before cleanup.
3. Marker-cleanup report, including its existing hash framing and untrimmed
   sanitized hash; do not conflate those with ordinary UTF-8 content hashes.
4. Normalized text hash after cleanup and trimming.
5. Provider-returned text hash after provider quality filters.
6. Dialogue decision and accepted-text hash after its later gates/repair.

Configured model and provider-reported model remain separate. Empty or missing
reported identity stays unknown. A reported name over 256 characters is hash-only,
with an explicit status; it is never presented as an exact truncated name.

Dialogue dispatches share a generation ID and logical attempt index with the
existing generation record. Its additive `provider_observation` field includes
all physical attempts, their recording statuses and a separate decision receipt.
The two legacy logical attempt slots remain unchanged. `response_text` retains
its existing semantics, explicitly labelled
`after_provider_cleanup_and_quality_filters_before_dialogue_gate` for primary,
or `after_provider_cleanup_and_fallback_next_repair_before_dialogue_gate` for
fallback. Historical records without this field are not retrospectively raw.
Other provider lanes receive the attempt envelopes but do not gain a fabricated
dialogue or journal association. Later action execution is outside this observer.

## Private retention and failure behavior

Only marker-bearing message content is retained, byte-exact, at
`raw/<sha256>.txt`. Exact duplicates share an artifact, not an attempt. This
uses the same private hash-addressed pattern as existing delivery artifacts,
but does not scan or rewrite their stores: those artifacts contain whole wire
responses and have a different hash scope.

Limits are fixed: 256 KiB per raw input, 64 MiB of raw artifacts, 256 MiB total
spool bytes and 50,000 files. One bounded inventory runs when the store first
opens. Subsequent writes use in-memory counters under a mutex and a process
lock. A second writer is refused. Bytes are charged before writing, including
interrupted temporary artifacts after restart. Exact existing files are checked
before deduplication; symlinks, altered content and broad permissions are refused.
Directories are created as 0700, files as 0600; existing paths must already have
those permissions. Atomic publication exposes only complete files as `.json`
or `.txt`. A completed immutable file is its write receipt, not a promise of
survival under every filesystem or power-loss failure.

Nothing is deleted automatically. Archive the stopped writer's complete spool
and choose a fresh private directory for a later observation window when needed.
Once limits are reached, raw evidence can be omitted while envelopes continue;
at the total/file limit, recording failures remain visible in the normal warning
channel and available generation joins. This is a bounded study spool, not an
unlimited historical log. Include its full coverage and failure history in any
rate calculation.

Oversize or quota-omitted raw text remains `input_not_retained_*`. Successfully
observed marker-free text is `observed_no_markers_raw_not_retained`, with its
exact hash and zero count. Recording faults are never turned into ineligibility.
Persistence errors are swallowed by the observer and cannot authorize a response,
change text, skip fallback or alter an action. The affected transport error logs
no longer quote response snippets. Raw text stays out of ordinary observer logs,
the research board and prompts.

## Qualification and interpretation

`scripts/qualify_provider_observation.py` drives the actual MLX and Ollama paths
using a local HTTP fixture and isolated test processes. It compares the exact
pre-change source, observation-disabled, enabled and recording-failure arms.
It verifies output, dialogue acceptance and fallback count parity, attempt joins,
raw artifact hashes, release binding and failure coverage. The test worker uses
an artificial label to keep timeout fixtures bounded; it invokes the actual
dialogue acceptance helpers. Full dialogue orchestration and action behavior
are additionally covered by the existing suite; this does not induce a live turn.

The qualification report retains all fixture cases, paths, executable hashes,
spool bytes and measured worker latency. Timings include request parsing and
synchronous writes on this test machine; they do not establish production
throughput. This observer uses bounded synchronous disk writes, so slow storage
can add latency even though returned content and acceptance logic are unchanged.

After a separately verified rollout, freeze a new natural observation window
before examining outcomes. Join the exact release and attempt to the episode's
specified old/new failure property. Report no opportunity, preserved/lost eligible
input, missing input, failed recording and unknown activation separately. Marker
counts alone do not prove that the focal repair was needed. Tests establish
software behavior; natural frequency, operational improvement and the beings'
experience remain separate research questions.
