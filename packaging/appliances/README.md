# CPU appliance profiles

Release archives carry build-manifest format `cpu-edge.2`. Both x86-64 and
ARM64 archives enforce a 60 MiB compressed-bundle ceiling and a 20 MiB ceiling
for the edge runtime plus its three private edge capsules.

Each profile pairs a systemd `.env` file with an `.edge-context.json` capsule
configuration. Together they configure the native edge reservoir, autonomous
scheduler, bounded `NEXT:` executor, and model-facing local identity. They
contain no credentials.

`install_edge_runtime.sh` defaults to an explicit observation-only authority
file. Measured profiles remain disabled themselves and declare eventual
capability separately with `ASTRID_EDGE_RESERVOIR_TUNING_PROFILE_PERMITS`.
After observation acceptance, `--enable-tuning` is a separate reversible step;
it is rejected for profiles that do not explicitly permit tuning.

- `generic-cpu.env` keeps hardware concurrency automatic and requires a local
  model benchmark.
- `avado-i3-16g.env` records the measured four-thread AVADO configuration.
  Autonomous prompts are capped at 1,200 characters. It rotates both ordinary
  and in-chain model sessions after two authored turns: measured full-stack
  latency rose from 200 seconds fresh to 234 seconds on the second retained
  turn and about 287 seconds on the third, while compact verified continuity
  already crosses generations.
- `icp-j3455-8g.env` records the measured four-core, 8 GiB ICP configuration
  and its independent no-inherited-corpus identity. It selects dense Qwen3
  1.7B at a 2K/128-token tier after explicit non-thinking mode reduced its
  six behavioral cases to 27-59 seconds with all cases passing. An accepted `RESEARCH` choice
  executes one audited read-only search without asking the small model to emit
  a native tool call. `READ_SOURCE <1|2|3>` can then fetch one retained result,
  while `READ <artifact-id>` carries verified local evidence. Its one-turn
  session cap prevents prior dialogue from doubling the next 2K-context prefill.
  Its autonomous prompt is capped at 900 characters. The exact allowlisted
  loopback provider receives a 420-second response-header deadline; public web
  requests retain the fixed 300-second ceiling and existing SSRF policy.
  Ordinary turns wake on salient host/I/O/source observations or a 60-minute
  quiet heartbeat; activity-only notebook records cannot recursively wake it.
- `icp-discovery.env` gives the next box an independent `ICP Astrid` identity
  and the proven 68% reservoir/Action policy without guessing its CPU, RAM, or
model tier. The measured AVADO and ICP profiles use a 10-minute ordinary
cadence, three-minute Action continuation, six-step chain ceiling, five-minute
human-input quiet period, and 96-attempt daily ceiling; session rotation and
hardware concurrency remain profile-specific. They also enable standing
private initiative for self-directed research, self-study, noticing, planning,
and workshop work without making activity obligatory. The measured
AVADO and ICP profiles also preserve every genuinely authored scheduled turn
as a provenance-labeled signal journal. Self-declared `JOURNAL` artifacts and
executor fallback remain separate; an executor-added generic safe `LISTEN` is
stripped before authored persistence and is never attributed to Astrid. Their
Action boundary also preserves one unambiguous argument-bearing declaration
when a small model splits its verb and single-line argument. Truly invalid
intentions remain non-executing, receive a specific validation reason, and get
at most one short correction opportunity before ordinary pacing resumes.
The bounded research executor stores sanitized request and completion receipts
with observational trace/session/chain spans and exact parent response hashes.
Native model tool calls retain a null Action parent instead of inventing one.
Result titles/URLs enter the next reflection as numbered tool evidence rather
than model authorship. A
self-chosen source read stores only a bounded, hashed, explicitly untrusted
visible-text excerpt inside the private research tree. `MEASURE` produces a
deterministic descriptive artifact that names known scheduler cadences and
never claims causality or Astrid authorship. Working-thread v6 state separately
retains authored claims and evidence records with explicit epistemic status.
Verified sources, studies, measurements, checks, syntheses, and peer evidence
have bounded priority over ordinary authored-artifact provenance, so routine
writing cannot evict the evidence needed for a later cited synthesis.
Spectral observations and reservoir-tuning results remain separately typed,
machine-derived evidence rather than authored findings.

Activity ledgers are owner-only. `report-edge-activity` provides trace, session,
chain, kind, absolute/relative time, limit, follow, text, JSON, and JSONL views without reading
artifact bodies or exposing headers. `report_edge_fleet_activity.py` merges
AVADO and ICP read-only over SSH, checks clock skew, and always labels the host;
it does not copy continuity or enable appliance-to-appliance messages.

`astrid-edge-hindsight.timer` adds a separate fifteen-minute operator
checkpoint outside `home/default/edge`. Checkpoint v2 opens each ledger once,
captures its inode and byte length, and hashes and parses exactly that prefix;
concurrent append bytes are deferred to the next checkpoint. Each checkpoint
binds its evidence to the canonical Linux boot ID. It hash-chains
checkpoint, artifact, and fill-rollup ledgers; verifies exact append-only
prefixes; indexes every version of an owned artifact with exact causal
attribution where identifiers exist; and
records owner-only kernel/audit database health. A normalized owner-only
`hindsight.sqlite3` mirrors sanitized events and metadata for fast historical
queries while the chained JSONL remains authoritative. The index is observability,
not continuity: Astrid cannot read it and it is never admitted to the
reservoir. `~/astrid-hindsight` renders a retrospective text or JSON report.
Use `--include-excerpts` only when prose is desired on the operator terminal.
The first v2 checkpoint is an explicit migration baseline with no prior
continuity claim. Preserved v1 alerts remain visible as legacy race-compatible,
unresolved history and are never rewritten as valid.

Report v15 emits both the 65–72% diagnostic shelf and the 65–73.5% acceptance
shelf, exact-origin local-provider header latency, current hindsight epoch
validity, legacy-alert counts, the exact loaded-capsule count, and bounded
spectral/tuning lifecycle summaries. Late autonomous responses associated with an
interrupted/recovered trace are rejected before experience or Action admission.
The bundled `reconcile-edge-interrupted-actions` tool can audit older ledgers
and, only with `--apply`, append an owner correction and quarantine the exact
affected artifact without rewriting historical receipts. Correction v2 binds
the canonical trace/turn plus response hash; repeated response text on another
turn remains independent, and legacy hash-only corrections stay explicitly
unattributed.

Measured CPU profiles also select compact autonomous prompts and declare the
model, context, output ceiling, and keepalive consumed by
`astrid-model-warmup.service`. The warmup service loads the model before Astrid
starts; enabling login lingering is still required for operation without an
interactive SSH session.

Install a profile with:

```bash
scripts/install_edge_runtime.sh --profile avado-i3-16g --start
```

The default `--layout auto` selects the standard `~/.astrid` tree for AVADO
and generic profiles, and the SSD-backed `~/.astrid-icp/state` tree plus the
ICP-specific units for `icp-*` profiles. `--layout standard` and
`--layout icp-ssd` are available when an operator needs to make that choice
explicitly; the dry-run output always prints the selected layout and paths.
For `icp-ssd`, `/media/data` must be mounted and `/media/data/astrid` must be
owner-writable. The core installer creates only the exact
`~/.astrid-icp -> /media/data/astrid` symlink and refuses a pre-existing
non-symlink tree; every ICP service also receives mount, symlink, and state-tree
guards.

Both installers hold the shared CPU-edge transaction lock, stage and hash a
complete generation, and snapshot every managed user unit's enabled and active
state. If a file or service transition fails, rollback restores both the prior
bytes and those exact unit states. Owner-ledger permission normalization rejects
symlinks, nonregular files, and parents resolving outside the private edge tree
before applying mode `0600`.

Run `scripts/probe_headless_linux.sh` before promoting a discovery profile into
a hardware-specific profile. Model settings remain in the provider and capsule
configuration because the edge service must not imply that an unbenchmarked
model is safe merely because it fits in memory.
