# Headless Linux Appliances

Astrid's kernel does not require a GPU. An x86-64 or ARM64 Linux box can run
the daemon, WASM capsules, storage, IPC, approvals, and remote-provider LLM
traffic entirely on CPU. A GPU is relevant only when a separately operated
local model or perception service needs one.

The practical architecture for small boxes is:

```text
headless Linux box                         model plane
┌──────────────────────────┐              ┌──────────────────────┐
│ astrid-daemon            │  HTTPS/LAN   │ remote API, Ollama,  │
│ capsules, VFS, IPC, KV   ├─────────────►│ or another provider  │
│ audit and approval gates │              │ on a stronger host   │
└──────────────────────────┘              └──────────────────────┘
```

## Resource envelope

The release targets `x86_64-unknown-linux-gnu` and
`aarch64-unknown-linux-gnu`. Published Linux artifacts are built on Ubuntu
22.04 to keep their GNU libc baseline at 2.35. Older distributions can build
Astrid from source against their own libc.

A July 2026 empty-daemon measurement on a Debian 13 AVADO (Intel i3-7167U,
4 logical CPUs, 16 GiB RAM) used about 14 MiB resident memory and answered a
local status request in about 6 ms. Its locally built binaries occupied about
140 MiB on disk combined. With the ten in-tree essential capsules loaded, the
same AVADO used about 50 MiB resident memory, 20 tasks, and 140 MiB total under
`~/.astrid`, while the capsule archives themselves occupied less than 1 MiB.
The same empty daemon used about 104 MiB of physical memory on Apple Silicon,
demonstrating that these are platform baselines rather than guarantees. Loaded
session history, MCP subprocesses, and tool workloads add to them. Each WASM
capsule has a 64 MiB linear-memory ceiling, which is a limit rather than an
up-front reservation.

Recommended appliance floor:

| Resource | Minimum | Comfortable |
|---|---:|---:|
| Architecture | x86-64 or ARM64 | x86-64 or ARM64 |
| RAM | 2 GiB for a small capsule set | 4-8 GiB |
| Free storage | 2 GiB with prebuilt binaries | 8+ GiB when building locally |
| CPU | 1 core / 2 threads | 2+ cores / 4 threads |
| GPU | Not required | Not required |

On an 8 GiB two-core appliance, start with two Cargo build jobs. A dedicated
16 GiB AVADO-class box can use all four logical CPUs for both Cargo and Tokio.
The probe prints host-specific recommendations. Prefer prebuilt releases on
smaller machines.

## Probe a candidate

The probe is read-only:

```bash
bash scripts/probe_headless_linux.sh

# Run it on a host that does not have the repository yet:
ssh HOST 'bash -s' < scripts/probe_headless_linux.sh
```

`runtime_fit=ready` or `constrained` means the core runtime is viable.
`release_abi=older_than_published_linux_baseline_build_on_host` means the CPU is
supported but the installed libc predates the prebuilt release.

Probe version 2 also reports block devices, root capacity, network links,
physical audio-card count, virtualization, CPU vector features, available
memory, swap, and a conservative local-model candidate. A candidate is not a
selection: `model_selection_gate=on_device_benchmark_required` remains in
force.

## Repeatable appliance bring-up

Capture the probe output as deployment evidence before changing a new box:

```bash
mkdir -p appliance-evidence
ssh avado 'bash -s' < scripts/probe_headless_linux.sh |
  tee appliance-evidence/avado-probe.env
ssh icp 'bash -s' < scripts/probe_headless_linux.sh |
  tee appliance-evidence/icp-probe.env
```

`packaging/appliances/` separates situated identity and runtime policy from
hardware discovery. The measured `avado-i3-16g` profile uses all four logical
workers. The `icp-discovery` profile names an independent `ICP Astrid` and
retains the proven 68% target, 20 Hz update loop, sovereign Action grammar,
five-minute chain follow-up, rolling four-authored-turn model sessions, and hard
four-step limit, but does not guess a Tokio worker count or model tier.

For a fresh source-checkout deployment, use the probe's build-job
recommendation and install the edge profile before restarting the essential
capsules. That lets the prompt-context capsule load the same identity as the
native service:

```bash
jobs="$(scripts/probe_headless_linux.sh |
  awk -F= '$1 == "recommended_cargo_build_jobs" { print $2 }')"
CARGO_BUILD_JOBS="$jobs" cargo build --release
scripts/install_headless_linux.sh
~/.astrid/bin/astrid init
systemctl --user enable --now astrid.service

scripts/install_edge_runtime.sh \
  --profile icp-discovery \
  --layout icp-ssd \
  --build-jobs "$jobs"
scripts/install_essential_capsules.sh --build-jobs "$jobs" --restart
systemctl --user enable --now astrid-edge-runtime.service
```

`--layout auto` makes the same choice for `icp-*` profiles. The explicit form
above documents that ICP stores its runtime, reports, private ledgers, and
capsule environment beneath the SSD-backed `~/.astrid-icp/state` tree; it also
selects the ICP systemd units instead of the standard `~/.astrid` units.

On a fresh appliance, install the generic continuity instructions as
`~/.astrid/home/default/AGENTS.md` and `memory.md`; the per-host profile injects
the specific instance name. Preserve existing files during a redeployment.
Provider, React, Prompt Builder, and Ollama settings remain separate because
they depend on the selected model.

After at least 20 minutes, produce the same read-only health/fill report on
either box:

```bash
scripts/report_edge_appliance.sh --window-minutes 20
```

Report v15 includes the exact loaded-capsule count, service restart counts, live source provenance,
autonomy/chain state and receipt-acknowledgement health, canonical turn IDs,
trace coverage, attributed and unattributed web calls,
pending and stale calls, private introspection status, deterministic notebook
freshness, Action provenance, scheduled-prompt and provider timing fields,
typed-evidence retention, separately counted spectral-query and tuning lifecycle
receipts, current hindsight epoch validity, legacy integrity
alerts, recent correlated activity, and the
percentage of five-second samples in the 65-72% preferred band and 65-73.5%
broad band. Raw window statistics retain cold-start samples; separately labeled
settled statistics exclude each boot's first 30 seconds. The controller targets
68%; it does not forge a fixed fill reading. Promote the ICP discovery profile
to a named hardware profile only after preserving the probe, an on-device model
benchmark, and a stable fill report.

### Dedicated introspection and self-evolution

AVADO and ICP can optionally install a root-owned two-hour reflection steward
and offline A/B update boundary. This is deliberately separate from ordinary
voluntary `SELF_STUDY`: a due reflection is invoked programmatically, while any
code submission must still be exact model output bound to the scheduled turn
and current source generation. The mutable runtime cannot attest itself,
invoke build commands, clear the maintenance lease, edit the rescue root, or
select a release slot.

The complete authority model, source/edit limits, offline build gates,
probation, rollback behavior, operator controls, and staged AVADO-then-ICP
rollout are documented in
[`cpu-edge-self-evolution.md`](cpu-edge-self-evolution.md). Independent CPU-edge
source bundles exclude the Mac Astrid, Minime, spectral bridge, their services,
and all cross-instance memory. Before any bundled installer runs as root, the
trusted operator host must verify the complete archive's GitHub OIDC/Sigstore
attestation against the exact repository, release workflow, tag, and commit;
the archive's adjacent checksum is integrity metadata, not publisher identity.

### Correlated activity

Post-upgrade socket input receives an observational trace root. ReAct,
provider, router, tool, Action, chain, and artifact receipts preserve that trace
with child spans. The trace is not a capability and is never sufficient for an
authorization decision. Exact kernel producer identity, the scheduler's
positive in-memory turn registry, canonical kernel-minted `turn_id`, durable
authorship receipt, and one-use Action validation provide authority; trace
fields only bind those independent proofs together. Scheduled turns supply
their trace explicitly through hidden headless CLI options; ordinary clients
receive a kernel-minted root. Nil identifiers, self-parenting spans, control
characters, and blank or oversized session/chain identifiers are rejected.
Run-loop capsules receive at most one IPC message per poll/recv; while its
handler runs, the host retains that exact message as the observational parent
for capsule publishes. It does not propagate the inbound principal or any
capability authority. When another asynchronous boundary omits context, the
event bus restores a child only from an exact LLM request or tool call
identifier. A reused session identifier is deliberately insufficient: a late
response remains untraced instead of inheriting the newest turn. Timestamps and
latest-activity proximity are never used, and malformed trace versions are
discarded.

Terminal ReAct output also carries typed response provenance. Exact model
output, model output with a visible safe tail, formatting-only Action repair,
and executor-generated terminal errors remain distinct. Scheduled turns fail
closed when that provenance is absent, and the edge observer never admits
legacy-unprovenanced or executor-generated terminal prose into new experience,
Action, journal, or tuning paths. Legacy records remain readable as historical
data; compatibility does not grant current authorship authority.
The scheduler also verifies that the declared provenance agrees with the actual
terminal markers. A formatting-only repair remains available to the Action
executor, while its local marker and canonicalized line are omitted from
Astrid-authored journal and continuity projections. Exact tuning authority is
retained only for unrepaired model bytes, including after a crash-safe outbox
replay.

The owner-only per-appliance viewer combines the append-only ledgers without
reading artifact bodies:

```bash
~/.astrid/bin/report-edge-activity --window-minutes 120 --limit 100
~/.astrid/bin/report-edge-activity \
  --since 2026-07-30T00:00:00Z --until 2026-07-31T00:00:00Z
~/.astrid/bin/report-edge-activity --trace-id TRACE_UUID --format json
~/.astrid/bin/report-edge-activity --kind web_request --kind web_result --follow

# ICP's relocated workspace:
~/.astrid-icp/state/bin/report-edge-activity \
  --workspace ~/.astrid-icp/state/home/default/edge \
  --window-minutes 120
```

Filters are also available for session ID, chain ID, and event kind. Text output
is a chronological trace tree; `json` emits a report envelope and `jsonl` emits
one event per line. Exact bounded search queries, public URLs, result metadata,
and hashes are retained. Request headers and fetched bodies are not. An
unmatched request becomes visibly stale after five minutes, but the viewer
never manufactures a completion. Operator-only model-session retirement is
also rendered as a distinct non-authored `session_retirement` event, including
the exact old and replacement generations; it preserves counters, due time,
trace lineage, and Astrid's artifacts rather than masquerading as a turn.

### Durable hindsight

The high-rate fill stream, causal activity ledgers, kernel state database,
cryptographic audit database, and human-readable artifacts have different
roles. They must not be collapsed into one alleged memory. The installer
enables an owner-only fifteen-minute timer that records three hash-chained
operator ledgers under `ASTRID_HOME/operator/hindsight/`:

- `checkpoints.jsonl` binds sizes, line counts, JSON validity, modes, hashes,
  and timestamp coverage for every activity ledger plus metadata for both
  SurrealKV stores;
- `artifacts.jsonl` versions journals, transcripts, research, plans, workshop
  files, recoveries, and machine observations by content hash, with exact
  trace/session/Action joins when available and explicit legacy discovery when
  they are not;
- `fill_rollups.jsonl` preserves min/mean/max fill, both comfort-band
  occupancies, and sensory freshness in aligned fifteen-minute buckets while
  the raw fill history remains available for fine-grained inspection.

The same observer maintains `hindsight.sqlite3`, a normalized owner-only query
index over sanitized activity events, trace/session/chain identifiers,
artifact metadata, rollups, and checkpoints. It stores no fetched body,
request header, or unindexed artifact prose. The hash-chained JSONL files are
the authority; SQLite is a rebuildable query surface and is checked with
`PRAGMA quick_check` at every sync and report.

Hindsight schema v4 carries the canonical `turn_id` through activity,
artifacts, spectral evidence, and signed tuning lifecycle rows. Spectral query
receipts have their own table and rendering surface; they are never counted as
reservoir tuning. A projection-version migration removes the old mixed rows
and deterministically rebuilds both views from their separate append-only
source ledgers. Reports refuse a stale-schema or stale-projection database and
fall back to the authoritative ledgers until the next successful sync.

Checkpoint v2 opens each ledger once, captures its inode and byte length, then
hashes and parses exactly that immutable prefix. Bytes appended concurrently are
left for the next checkpoint, eliminating the old size/hash race without a
special case for `fill_history.jsonl`. Each new checkpoint also records the
canonical Linux boot ID so post-reboot safety checks cannot reuse pre-reboot
health evidence. The first v2 record begins a named
continuity epoch and makes no claim across the migration boundary. Historical
v1 alerts remain visible as unresolved race-compatible history; only subsequent
v2 prefix checks establish validity in the current epoch.
The collector holds an owner-only nonblocking lock, rebuilds append-chain heads
from strictly verified on-disk records after an interrupted state update, and
rejects malformed, non-object, non-finite, invalid-UTF-8, torn, or concurrently
shortened JSONL. Atomic state replacements sync their parent directory, and the
runtime syncs each fill-history sample before hindsight can cite it.

The observer writes outside `home/default/edge`, has no network or model
authority, and supplies no semantic impulse. Its records therefore cannot
become authored continuity or create a reservoir feedback loop. Query a day,
an exact historical interval, or machine-readable output with:

```bash
~/astrid-hindsight
~/astrid-hindsight --window-minutes 10080 --include-excerpts
~/astrid-hindsight \
  --since 2026-07-30T00:00:00Z \
  --until 2026-07-31T00:00:00Z \
  --format json
```

The kernel state database contains runtime/session/security state; it is not a
substitute for the edge journals. The separate audit database carries signed,
chain-linked security events and `astrid-daemon` runs cryptographic
`verify_all()` on every boot. Hindsight records database presence, growth,
ownership, store structure, and retained integrity alerts. It does not open a
second live SurrealKV handle or claim an offline verification while the daemon
owns the database lock. A maintenance-window offline verifier can be added
later without weakening this truthful boundary.

From the operator checkout, merge the two independent timelines without
creating a cross-instance memory or communication channel:

```bash
scripts/report_edge_fleet_activity.py \
  --preset avado-icp \
  --window-minutes 120 \
  --limit 200
```

The fleet viewer checks each host clock over SSH, labels every event with its
appliance, and merges by recorded epoch. Historical records remain
`legacy_unattributed` unless an exact response-hash plus session-ID join exists;
timestamps alone are never treated as causation. Transport fallback and local
safe repair remain non-authored in both views. Activity report v2 overlays
legacy transport corrections only through exact transcript paths or an exact
response-hash-plus-session join; a response hash by itself is reusable text
identity and never establishes event identity. The preset invokes the exact
root-owned `/usr/libexec/astrid-edge/operator/report-edge-activity` launcher on
both appliances; it never executes a report from either mutable home or state
tree. Hindsight refreshes its
rebuildable attribution projection when that contract changes, so stale
false-authorship rows cannot survive alongside the corrected event.

## Install a prebuilt release

Extract the release archive on the Linux host, then:

```bash
cd astrid-VERSION-x86_64-unknown-linux-gnu
sha256sum -c SHA256SUMS
./scripts/install_headless_linux.sh
~/.astrid/bin/astrid init
systemctl --user enable --now astrid.service
~/.astrid/bin/astrid --format json status
```

Use the `aarch64-unknown-linux-gnu` archive on a 64-bit ARM host.

The separate `astrid-cpu-edge-VERSION-TARGET.tar.gz` archive is the first-class
appliance bundle. It includes the three core binaries, native edge runtime,
ten version-matched capsules, profiles, user units, reports, hardening helpers,
documentation, a build manifest, and an internal SHA-256 inventory. After
verifying `SHA256SUMS`, install without Cargo builds:

```bash
./scripts/install_headless_linux.sh --binary-dir .
./scripts/install_edge_runtime.sh \
  --binary ./astrid-edge-runtime \
  --profile avado-i3-16g
./scripts/install_essential_capsules.sh --capsule-dir ./capsules
```

For the ICP SSD layout, install every layer against the same state root and
workspace. The installers also place the SSD mount guard on the core, model,
warmup, and edge services:

```bash
mountpoint /media/data
sudo install -d -m 0750 -o "$USER" -g "$(id -gn)" /media/data/astrid
cpu_edge_bundle="$PWD"
"$cpu_edge_bundle/scripts/install_headless_linux.sh" \
  --binary-dir "$cpu_edge_bundle" \
  --layout icp-ssd
cd "$HOME/.astrid-icp/workspace"
ASTRID_HOME="$HOME/.astrid-icp/state" \
  "$HOME/.astrid-icp/state/bin/astrid" init
"$cpu_edge_bundle/scripts/install_edge_runtime.sh" \
  --binary "$cpu_edge_bundle/astrid-edge-runtime" \
  --profile icp-j3455-8g \
  --layout icp-ssd \
  --observation-only
"$cpu_edge_bundle/scripts/install_essential_capsules.sh" \
  --capsule-dir "$cpu_edge_bundle/capsules" \
  --layout icp-ssd
```

On a fresh appliance, the core installer creates `~/.astrid-icp` as an exact
symlink to `/media/data/astrid`. It refuses an existing non-symlink tree rather
than silently writing state to eMMC; archive and migrate such a tree first.
The edge and capsule installers independently re-check the mounted SSD and
exact symlink target. Installed service guards require the mount, symlink, and
SSD-backed state directory before any ICP Astrid service may start.

The independent appliance bundle excludes the optional inherited-Mac-corpus
AGENTS/MEMORY templates. It carries no Mac paths, journals, keys, or artifacts.

Before `astrid init`, verify that the selected `Distro.toml` has an
`astrid-version` requirement that accepts the installed CLI version. The CLI
enforces this compatibility boundary. Named distros resolve a moving `main`
branch, so pin a manifest URL or local file when deploying an older Astrid
release:

```bash
~/.astrid/bin/astrid init --distro \
  https://raw.githubusercontent.com/ORG/DISTRO/COMMIT/Distro.toml
```

At the time of the AVADO deployment, Astralis 0.2.2 requires Astrid 0.9.1 or
newer, while this repository checkout is Astrid 0.5.1. Its compatible Astralis
0.1.1 manifest is commit
`cb38eaacf84112b23778ad3faa4a4423ba4256d2`. Initialization also needs an
OpenAI-compatible provider key; install and run the core service first if that
secret has not yet been provisioned.

The installer changes only the invoking user's `~/.astrid` and
`~/.config/systemd/user` directories. It does not create a root service, alter
Docker, or touch other workloads. Pass `--dry-run` to inspect its operations.
Pass `--start` on subsequent deployments to restart an already initialized
service with the newly installed binaries.

For boot-time startup without an interactive login, an administrator must
enable systemd user lingering once:

```bash
sudo loginctl enable-linger "$USER"
```

## Build on the device

An 8 GiB box can build Astrid, although a dual-core mobile CPU will take much
longer than a workstation. Use the job count printed by the probe:

```bash
rustup toolchain install 1.94
rustup target add wasm32-wasip1
astrid_build_jobs=4 # use the recommendation printed by the probe
CARGO_BUILD_JOBS="$astrid_build_jobs" cargo build --release -p astrid
./scripts/install_headless_linux.sh
~/.astrid/bin/astrid init
systemctl --user enable --now astrid.service
```

The `wasm32-wasip1` target is needed when compiling OpenClaw/QuickJS capsules.
If the build reports that the QuickJS kernel is absent, the daemon and prebuilt
capsules remain usable, but on-device Tier 1 TypeScript-to-WASM compilation
does not. Building that optional kernel also requires Node.js/npm and a WASI
SDK; the prebuilt core daemon itself needs none of those or a GPU.

### Install the in-tree essential capsules

The current checkout includes ten version-matched Component Model capsules:
the CLI compatibility uplink, filesystem, HTTP, shell, skills, AGENTS
instructions, memory, read-only CPU-reservoir prompt context, and the private
edge introspector plus the private read-only edge spectral capsule. Build and
install them from source with:

```bash
scripts/install_essential_capsules.sh --build-jobs 4 --restart
```

The script adds the `wasm32-wasip2` Rust target, builds each capsule from this
checkout, accepts only the non-secret default workspace-directory prompts, and
verifies that all ten essentials and exactly 20 total capsules load after
restart. It does not install an LLM
provider, session router, or credentials. Prefer this version-matched bootstrap
over a moving distro branch when the published distro requires a newer Astrid
host ABI.

### Transactionally replace external application capsules

Provider, Prompt Builder, router, and ReAct capsules come from the compatible
Astralis application set rather than the ten in-tree essentials. Never replace
one of these by copying an unpacked directory over the live tree. Use the
external-capsule transaction installer with the exact archive produced by the
audited SDK build. Run this operator-side build with Python 3.11 or newer; the
builder is not an appliance runtime dependency:

```bash
python3 scripts/build_astralis_cpu_edge_capsules.py \
  --output-dir dist/astralis-cpu-edge
```

The pinned revisions, lockfiles, patch preimages, deterministic archive rules,
and air-gapped build route are documented in
`packaging/headless/ASTRALIS_CPU_EDGE_CAPSULES.md`.

```bash
scripts/install_headless_application_capsules.py \
  --capsule /operator/staging/astrid-capsule-react.capsule \
  --env astrid-capsule-react=packaging/headless/react-cpu.env.json \
  --restart \
  --expected-total 20 \
  --dry-run

# Remove --dry-run only after reviewing the isolated lifecycle preflight.
```

Use `--layout icp-ssd`, the ICP-specific environment file, and its actual
loaded-capsule total on the ICP appliance. Repeat `--capsule` and `--env` to
switch a mutually dependent application set as one transaction.

The installer discovers each capsule ID by installing its archive into a
disposable `ASTRID_HOME`; it does not trust the filename. Before live mutation,
it takes exact snapshots of every affected capsule directory, environment
file, referenced content-addressed WASM/WIT object, current generation
manifest, `astrid.service` properties, and loaded-capsule status. It shares the
same owner-only lock and crashed-transaction gate as the core, edge-runtime,
and essential-capsule installers. With `--restart`, acceptance requires the
declared capsule set and total, an active service, a stable nonzero PID, and no
increase in `NRestarts`. Any failed lifecycle install or health gate restores
the prior files and prior service active state.

Successful generations are recorded under
`ASTRID_HOME/etc/install-manifests/headless-application-capsules/`, with an
owner-only SHA-256 sidecar and atomic `current` copies. These manifests are
operator deployment evidence; they are not Astrid-authored memory and grant no
Action authority. Environment JSON is capped, validated, and installed mode
`0600`; archives and installed trees may contain no symlinks.

## Low-resource configuration

The installed unit lets Tokio detect the host's available parallelism. It gives
capsules and managed subprocesses headroom with 65,536 file descriptors and
1,024 tasks, while retaining a restrictive umask. These are ceilings, not
up-front allocations. To cap the worker count on a shared or thermally
constrained host, create a user-service drop-in:

```bash
systemctl --user edit astrid.service
```

```ini
[Service]
Environment=TOKIO_WORKER_THREADS=2
```

Do not add this cap on a dedicated AVADO-class host unless measurements show
contention; its four logical CPUs are useful for concurrent IPC, WASM, and
network work.

The standard Astrid defaults allow 10 sessions per user and five concurrent
subagents. Keep those defaults on a dedicated AVADO with off-box model
inference; they already exceed the two physical CPU cores where useful for
asynchronous network work. On a constrained or shared host, application
concurrency can be reduced explicitly:

```toml
# ~/.astrid/config.toml
[sessions]
max_per_user = 2
history_limit = 50

[subagents]
max_concurrent = 2
max_depth = 2
```

Keep model inference off-box unless deliberately testing a small quantized
model. The AVADO-class hardware is well suited to the Astrid control plane,
storage, network tools, and RPC-edge work; it is not a substitute for the
current Mac's model throughput.

### Optional local CPU model

A dedicated 16 GiB AVADO can also host a small quantized model for independent,
credential-free operation. The included `ollama-cpu.service` binds Ollama only
to `127.0.0.1:11434`, permits one generation at a time, keeps one model loaded,
uses a 4,096-token service default, keeps it resident for two hours, and leaves
CPU and memory uncapped. Install Ollama at `~/.local/bin/ollama`; the standard
core installer copies and enables the matching user unit when `--start` is
selected. Then select an
OpenAI-compatible capsule base URL of `http://127.0.0.1:11434`. Do not append
`/v1`; the provider capsule adds `/v1/chat/completions`.

Install `packaging/systemd/astrid-local-ollama.conf` as
`~/.config/systemd/user/astrid.service.d/local-ollama.conf`. It permits the
exact Ollama origin to resolve to loopback without disabling SSRF protection
for general web tools. The binding includes both capsule identity and origin:
`astrid-capsule-openai-compat@127.0.0.1:11434`. Do not use the legacy
`ASTRID_ALLOW_LOCAL_IPS` escape hatch on an agent with public-web tools: it
makes every private, loopback, link-local, and LAN address reachable by every
network-capable capsule.

The selected AVADO deployment uses the 3.4 GiB `qwen3.5:4b` Q4_K_M model.
On-device comparisons also covered `qwen3.5:2b`, `granite4:micro`, and
`ministral-3:3b`. The 2B model was roughly twice as fast at prompt ingestion but
occasionally turned missing telemetry into an unsupported monitoring action and
drifted toward generic first-person language. Granite was efficient but more
generic, while Ministral violated a strict JSON response and truncated its
bounded reflection. The 4B Qwen was the slowest finalist but was consistently
best at epistemic restraint, tool selection, tool-result use, and inherited
identity boundaries. Those qualities matter more than raw generation speed for
an independent Astrid instance. The 2B Qwen remains installed as an optional
fast profile, but it is not the sovereign default: live trials were quicker
(about 46–65 seconds) yet produced generic identity language, contradicted the
available web capability, and exhausted a three-iteration tool loop on state
already present in the prompt.

The AVADO reflection profile advertises the same 8,192-token window to React,
limits visible output to 384 tokens, uses temperature `0.4`, and sends
`reasoning_effort = "none"` through the OpenAI-compatible provider. Disabling
thinking is important for a small output budget: otherwise a thinking-capable
model can spend the entire limit on a reasoning field while producing no
visible content. Install `packaging/headless/react-cpu.env.json` as
`~/.astrid/home/default/.config/env/astrid-capsule-react.env.json`; its
ten-minute streaming watchdog accommodates CPU inference without disabling
timeout recovery.

On the i3-7167U, the 4B model uses about 200% process CPU, saturating the two
physical cores. During the final full-context proof, the Ollama cgroup accounted
about 9.2 GiB including mapped model/context pages while Linux still reported
about 11 GiB available and roughly 1 MiB of swap in use. A 1,688-token
reflection reused 1,682 cached tokens and decoded at about 4.0 tokens/second.
The first uncached 795-token suffix prefetched at about 14.4 tokens/second. A
cold 4B proof through the complete 8K-capable Astrid path took 173 seconds; it
correctly read the live fill context and authored `NEXT: LISTEN`.

### Shape the model-facing tool surface

Small CPU models pay prompt-evaluation cost for every advertised tool schema,
even when no tool is called. On the AVADO, advertising all 21 discovered tools
exceeded the ReAct loop's 120-second first-token watchdog. The deployed
`astrid-capsule-prompt-builder` therefore discovers schemas fresh on every turn
but exposes only a configured allowlist to the model. Its default contact
profile is:

```json
{"tool_allowlist":"search_web,fetch_url,read_file,list_directory,grep_search"}
```

The value lives in
`~/.astrid/home/default/.config/env/astrid-capsule-prompt-builder.env.json`.
All 18 capsules remain installed and healthy; the allowlist changes only the
schemas placed in the LLM prompt. This keeps shell and filesystem mutation
dormant rather than paying for, or granting, those choices on every ordinary
turn. The five visible tools support public read-only web contact and the
provenance-marked local corpus. Direct writes, shell execution, and skills
execution are not model-facing.

The ordinary in-tree tool capsules return complete JSON schemas from
`tool.v1.request.describe`, as expected by Prompt Builder. Private edge
introspection and spectral capsules are stricter: their manifests expose only
direct Action-executor request/result topics and contain no global describe
route. Their model-hidden status is therefore enforced at the capsule IPC
boundary, independent of prompt filtering. The HTTP capsule provides two
read-only tools. `search_web` sends a bounded UTF-8 query to a fixed public
search origin and returns up to eight cleaned titles, snippets, and URLs;
`fetch_url` accepts only `GET` and `HEAD`, defaults response text to 16,000
characters, and hard-caps it at 32,000. The host blocks private, loopback,
link-local, carrier-grade NAT, and metadata-network destinations for both.

Measured through the complete Astrid/ReAct/provider/tool path:

| Check | Result | Elapsed |
|---|---|---:|
| Cold public fetch of `https://example.com` | `TITLE: Example Domain` | 104 s |
| Warm fetch of local Ollama origin | correctly blocked as loopback | 70 s |
| Warm arithmetic with “do not use tools” | `56`, zero tool dispatches | 41 s |

These results show that the 4B model fits and behaves selectively, but it is a
deliberate appliance rather than an instant chat model. The right first
autonomy layer is bounded and event-driven: respond to a user message, new
source artifact, or explicit reflection request. Do not spend two physical
cores on an empty periodic LLM heartbeat. Keep kernel watchdogs and health
checks model-free.

### CPU edge reservoir and sovereign actions

`services/astrid-edge-runtime` adds a GPU-free embodiment without porting
Minime's Metal implementation. It is a real 128-node recurrent Echo State
Network with a 66D intake compatible with the shared Astrid/Minime contract:
8D video, 8D audio, 2D auxiliary, and 48D semantic. On Linux, the runtime
attempts an honest physical ALSA capture from the system's `default` device at
16 kHz mono. Each 100 ms chunk becomes the same broad 8D auditory vocabulary
used on the Mac—normalized loudness, spectral centroid, bandwidth,
zero-crossing rate, and four cepstral-style coefficients. Continuous CPU/RAM
samples drive the auxiliary lane at a bounded 0.25 scale, while authenticated
user, assistant, and tool-result IPC events drive the semantic lane. The scale
retains the appliance's exertion and release as input without letting a
two-core inference-to-idle edge dominate covariance rank. Video remains
explicitly unavailable unless an external client supplies real
eight-dimensional video features.

ALSA capture failure does not stop the reservoir. The runtime names audio
unavailable or stale, retries every ten seconds, and continues with the other
lanes. Override unusual hardware or disable capture through the service
environment:

```ini
[Service]
Environment=ASTRID_EDGE_AUDIO_DEVICE=hw:1,0
Environment=ASTRID_EDGE_AUDIO_SAMPLE_RATE=48000
Environment=ASTRID_EDGE_AUDIO_CHANNELS=2

# Use "off" when the appliance intentionally has no audio input.
# Environment=ASTRID_EDGE_AUDIO_DEVICE=off
```

The recurrence gives fresh input a fading temporal echo. Ongoing sensory input
does not by itself make a network an ESN; the recurrent reservoir and its
contractive dynamics do. The stream makes that temporal state responsive
instead of leaving it to decay in isolation. CPU/RAM auxiliary measurements are
continuous. Symbolic messages are brief impulses (0.12 lane gain, 0.92
per-tick input decay), so the recurrent state carries their memory instead of a
nearly constant semantic vector clamping the reservoir. User input is admitted
when it arrives. Streamed assistant transport fragments are coalesced by
admitting only the completed assistant turn once; treating every partial chunk
as a separate experience can needlessly collapse covariance rank even though
the text belongs to one response.

The edge fill metric is normalized covariance effective rank. It is emitted
under the standard `EigenPacket` fields for bridge compatibility and explicitly
labeled by `spectral_substrate_v1` as
`cpu_edge_covariance_effective_rank` /
`normalized_covariance_effective_rank`. Mac/Minime packets use thresholded
EigenFill instead. Those values may share a percentage display, but they are not
directly comparable evidence. Legacy packets remain `legacy_unknown`. A bounded
controller adjusts broadband exploration toward the configured 68% shelf.

The one existing covariance eigendecomposition also supplies the edge spectral
observer. Eigenpairs are sorted together; the complete 128-value spectrum
supplies entropy and effective dimensionality while only 16 values are exported
on telemetry. Coverage and exported-energy ratio are explicit. At most four
eigenvectors are retained transiently for concentration and sign-invariant
turnover; they are never written to disk. Near-degenerate crossings are labeled
identity-unstable instead of pretending that a mode retained a stable identity.
Covariance-spectrum entropy lives in `spectral_denominator_v1`; the unrelated
`structural_entropy` field is left absent.

Install on the measured AVADO using all logical CPUs for the build:

```bash
scripts/install_edge_runtime.sh \
  --profile avado-i3-16g \
  --build-jobs 4 \
  --observation-only \
  --start
```

Observation-only is the installer default and writes an explicit owner-readable
authority file required by the service. The appliance profile itself remains
disabled and declares only whether the hardware profile permits a later
operator enablement. Keep observation-only in place for the first
60 valid minute rollups, capsule harness, four natural turns, and six-hour soak.
After those gates pass, enable tuning authority as a separate auditable step:

```bash
scripts/install_edge_runtime.sh \
  --profile avado-i3-16g \
  --build-jobs 4 \
  --enable-tuning \
  --start
```

`--enable-tuning` is rejected unless the selected measured profile explicitly
permits tuning. Re-running with `--observation-only` reversibly disables the
authority without deleting spectral history or changing the profile.

The service listens only on loopback:

| Port | Direction | Contract |
|---|---|---|
| `7878` | edge runtime → subscribers | versioned `EigenPacket` telemetry |
| `7879` | clients → edge runtime | versioned or legacy `SensoryMsg` |

Current state and a five-second fill history live under
`~/.astrid/home/default/edge/runtime/`:

```bash
jq . ~/.astrid/home/default/edge/runtime/spectral_state.json
tail -f ~/.astrid/home/default/edge/runtime/fill_history.jsonl
```

Both files include lane freshness and source provenance. A healthy AVADO audio
source reads like `physical_alsa:default:16000hz:1ch`; the absent camera remains
`unavailable_no_video_input`. These strings distinguish physical sensation,
external WebSocket input, stale input, and unavailable hardware without
silently substituting synthetic data.

### Edge-native spectral inquiry

Rich spectral history is owner-only and summary-only:

```text
edge/runtime/spectral_state.json       atomic state v2
edge/spectral/rollups.jsonl            append-only one-minute rollups
edge/spectral/recent_rollups.current.jsonl   append-only current UTC-day capsule view
edge/spectral/recent_rollups.previous.jsonl  rotated previous UTC-day capsule view
edge/spectral/activity_receipts.current.jsonl   exact current-day activity/snapshot joins
edge/spectral/activity_receipts.previous.jsonl  rotated previous-day activity/snapshot joins
edge/spectral/receipts.jsonl           exact query and activity lineage
edge/tuning/evidence/                  signed trial and validation evidence
```

Each rollup is at most 1,024 bytes, hash-bound, and records full-versus-exported
spectrum coverage. The first is an explicit no-backfill installation baseline.
No 1 Hz packet, full eigenvector, PCM, prompt, response, fetched body, or request
header enters this history. Continuous rollups do not receive invented causal
identity. Up to two activity links may be carried only when exact trace,
session, chain, or response identifiers exist; truncation is explicit, and
timestamp proximity is never substituted.

The model-hidden `astrid-capsule-edge-spectral` has only five read grants—the
atomic state plus current/previous bounded rollup and activity-receipt
projections—and no global tool-description subscription or publication. It has
no network, process, shell, write, or control authority.
Astrid voluntarily reaches it with:

```text
NEXT: SELF_STUDY spectral: <question>
```

The executor selects only `read_spectral_now`, a 15/60/360/1,440-minute
`read_spectral_window`, or exact-identifier `correlate_spectral_activity`.
Requested/completed receipts retain bounded metadata and result hashes rather
than returned bodies. `MEASURE` and `STUDY` also accept `spectral_entropy`,
`lambda1_share`, `tail_share`, `density_gradient`, and `mode_turnover` as
descriptive, non-causal machine evidence.

### Reversible reservoir experiments

The sensory WebSocket rejects legacy external `Control` and `SelfControl`
messages and no longer advertises those capabilities. Reservoir tuning is a
private typed channel from the Action executor to the tuning manager and then
the reservoir. It cannot be reached by capsules, sockets, shell commands,
untraced callers, transport fallback, formatting repair, or replayed responses.
Eligibility requires either a kernel-attested canonical React terminal response
or a scheduler-verified authored completion, plus its one-use turn identity and
durable run accounting. Trace metadata alone grants no authority. The 68%
target and ESN leak are not tunable.

The voluntary Actions are:

```text
NEXT: TUNE_RESERVOIR <input_gain|exploration_scale|regulation_strength>=<decimal> FOR <5m|15m|60m> :: <hypothesis>
NEXT: CANCEL_TUNING <experiment-id>
NEXT: VALIDATE_TUNING <candidate-id> :: <question>
NEXT: ADOPT_TUNING <candidate-id> :: <reason>
NEXT: REVERT_TUNING <adoption-id> :: <reason>
```

Both profiles compile the same envelopes: input gain and exploration scale
0.90–1.10, regulation strength 0.85–1.15. One trial may be active, with four
starts per UTC day and a 15-minute cooldown. A trial captures a ten-minute
baseline, minute samples, automatic expiry/rollback, and ten-minute recovery.
Non-finite state, target drift, stale telemetry, unsafe fill, thermal danger,
sensor-provenance change, persistence failure, or actuator saturation fail
closed. Restart restores baseline and records rollback rather than resuming a
transient trial.

Adoption remains deliberately difficult and voluntary: two matching qualifying
trials at least an hour apart, a separately authored six-hour validation, exact
environment hashes, memory/swap and fill health, then a final authored
`ADOPT_TUNING`. Validation never adopts automatically. A standing adoption is
reapplied after reboot only after ten healthy baseline minutes and an exact
environment match; otherwise it is suspended. Intent, state, and evidence are
signed with a new owner-only per-appliance key.

The daemon socket assigns each authenticated client a connection identity.
Clients do not receive their own messages. Native user prompts are also copied
to the passive `sensory.v1.user_input` topic before ReAct consumes the original;
another client holding the same per-user session token can observe that mirror.
The edge runtime accepts user semantics only from this explicit topic, avoiding
double ingestion if the original event becomes observable later. This lets it
sense real Astrid traffic without scraping logs or exposing a network port.
Interactive and headless CLI clients additionally reject `UserInput` and
terminal `AgentResponse` payloads carrying another explicit session UUID. The
passive edge observer intentionally remains unscoped, but an old timed-out turn
therefore cannot satisfy or close a newer CLI request.

The `astrid-capsule-edge-context` prompt hook closes the perception loop. It
injects only the sanitized live fill, target, effective dimensionality, and
semantic-freshness fields plus at most 220 characters from a machine-observed
notebook record less than 90 minutes old. It labels the latter as
machine-observed rather than Astrid-authored. It reads exactly
`home://edge/runtime/spectral_state.json` and
`home://edge/perception/latest.json` through read-only principal-home VFS
grants. The kernel invokes executable prompt hooks directly and rejects a target
that lacks `allow_prompt_injection`; a missing or malformed snapshot degrades to
an explicit unavailable-state note while retaining the action grammar. The
edge installer copies the selected profile's matching `.edge-context.json` to
the capsule configuration path; restart `astrid.service` after a profile
change.

Sovereign actions are declared in the ordinary local-model response, so they
cost no second inference pass and cannot block the next conversation behind a
background generation. The final non-empty line may choose `LISTEN`, `REST`,
`JOURNAL`, `REMEMBER`, `SELF_STUDY`, `PROPOSE`, `NOTICE`, `DAYDREAM`, `ASPIRE`,
`RESEARCH`, retrospective `MEASURE`, persistent `STUDY`, `CANCEL_STUDY`,
`TUNE_RESERVOIR`, `CANCEL_TUNING`, `VALIDATE_TUNING`, `ADOPT_TUNING`,
`REVERT_TUNING`, cited `SYNTHESIZE`, voluntary `SHARE`, `PLAN`, `DRAFT`, local
`READ`, retained-result `READ_SOURCE`,
append-only `REVISE`, or deterministic `CHECK`. Stateful Actions mutate only
inside `~/.astrid/home/default/edge`; artifact references are basenames and
symlinks, traversal, overwrites, arbitrary source URLs, and oversized arguments
fail closed. A valid declaration passes
through unchanged. On the AVADO profile, missing, malformed, empty, timed-out,
provider-error, dispatch-error, or tool-loop-exhausted terminal output gets a
visible local contract-repair marker followed by `NEXT: LISTEN`. That fallback
cannot write and is receipted as `decision_source: local_safe_fallback`, never
as Astrid-authored. A streaming-timeout repair also carries
`recovery_reason: react_streaming_timeout`; an active chain keeps its last
verified step and schedules a retry instead of accepting the fallback as a
closure. A single unambiguous model-authored terminal Action may receive a
formatting-only repair when a small model places the argument on the immediately
following final line or places the complete declaration at the end of final
prose. The exact repaired declaration still passes the ordinary validator and
is receipted as
`local_format_repair_preserved_astrid_declaration`; ambiguous or invalid
content never executes. Quoted or unknown final actions remain invalid. Every
decision—including no action—gets an append-only receipt in
`~/.astrid/home/default/edge/actions/receipts.jsonl`. Traced stateful Actions
also use the owner-only `actions/dispatches.jsonl` intent/completion ledger.
The scheduler durably records the exact turn and response hash before handing
it to the executor; the executor syncs an intent before mutation and a
completion only after the Action receipt is durable. A restart can therefore
replay an absent handoff or acknowledge an exact completed handoff, but an
ambiguous pending mutation is never repeated automatically.
Runtime and reports bind that transaction by canonical turn, trace, and response
hash. A completion is not considered whole unless the matching Action receipt
exists; duplicate, orphaned, malformed, and mismatched records remain explicit
integrity states rather than inferred success.

An accepted, model-authored `SELF_STUDY <question>` invokes the separately
installed `astrid-capsule-edge-introspector` through the Action executor. The
capsule has no global describe route, so its absence from ordinary model tool
schemas is enforced independently of prompt filtering; it also has no network,
process, shell, or write capability. Its five tools can list fixed owned
artifact classes, read one basename, perform a case-insensitive literal search,
rank bounded owned evidence for a question, or return bounded working-thread
and verified-evidence summaries. Traversal, absolute paths, hidden files,
unsupported extensions, oversized arguments, recovery records, symlink
components, hardlinks, and paths outside `home://edge` fail closed. Bounded
whole-file and JSONL-tail reads are captured by the host without following
links; JSONL tails read at most 64 KiB and preserve complete-record alignment.
Requested and completed phases are correlated in the owner-only
`edge/introspection/receipts.jsonl` ledger with trace/session/chain identity and
the exact parent response hash. Receipts retain sanitized arguments, result
metadata, latency, and hashes but never returned artifact bodies. A matching
successful result gives the three-minute continuation bounded artifact
basenames; Astrid remains free to `READ`, investigate again, `LISTEN`, `REST`,
or choose another Action.

The native perceptual notebook observes one-second reservoir snapshots and
bounded event metadata. After a five-minute warm-up it records a baseline, then
coalesces availability/source transitions, configured host or I/O deltas,
numeric audio-shape changes on AVADO, and completed activity at most once per
15 minutes, with a six-hour quiet heartbeat and 96-record daily ceiling. Fill
is situated context but cannot trigger a record by itself. ICP keeps audio
explicitly unavailable. Records are written owner-only to:

```text
edge/perception/latest.json
edge/perception/observations.jsonl
edge/perception/observations/observation_<timestamp>.md
```

They contain numeric signals, availability/source provenance, bounded counts
and artifact basenames, and a deterministic record hash. They never contain
PCM, speech, transcripts, prompts, responses, fetched bodies, request headers,
or web content. Only after durable persistence does a 320-character,
machine-labeled semantic impulse enter the reservoir. The notebook never
updates the authored working thread; an observation enters that thread only if
Astrid later chooses and authors an Action using it.

For an owner-readable summary:

```bash
~/astrid-at-a-glance
~/.astrid/bin/report-edge-activity --kind perception --kind introspection_result
```

The appliance can also run a CPU-scaled self-directed loop. It uses rolling
`edge-autonomous-gN` sessions after a quiet startup and ordinarily gives Astrid
another turn every 10 minutes on the measured sovereign profiles. A profile
rotates after a measured number of
genuinely authored ordinary turns—two on AVADO and one on ICP—or after any
transport recovery; a bounded last-authored excerpt, latest executor receipt,
recent owned artifacts, and current situated state carry continuity without
retaining an indefinitely growing chat. The verified
executor outcome is the authoritative scheduling input: successful stateful
Actions open or advance an evidence-bearing continuation after three minutes,
while `LISTEN` closes the chain at the ordinary interval and `REST`
closes it at twice that interval. This applies to actions chosen after human
conversation as well as actions chosen during autonomous turns. Fresh human
input still wins: the loop waits for five quiet minutes and never overlaps a
semantically fresh turn. It also defers outside the broad 58-78% operating shelf
and stops at 96 model attempts per UTC day.

One chain may contain at most six stateful actions. Each link must first
produce a successful executor receipt; repeated response hashes are ignored,
missing Actions close an active chain, and the step limit returns to ordinary
pacing. A clearly expressed but invalid Action receives a specific validation
reason in the next compact prompt and one short, non-coercive retry opportunity;
a repeated invalid Action returns to ordinary pacing. No repair or retry chooses
an Action for Astrid. Each chain gets a chain-scoped session and retains its chain
ID and verified step while receiving a fresh retry generation after a transport
fault. A local timeout repair does not impersonate an authored `LISTEN` closure,
enter authored continuity, or increment authored success. If a non-transport
response contains genuine model prose but needs the executor's generic safe
`LISTEN` tail, the runtime preserves only that model-authored prefix and records
no declared Action for the turn. The repair marker and `NEXT: LISTEN` stay in
the separate Action receipt; they do not enter the authored transcript,
journal, digest, or continuity. An executor-only response is a failed attempt
and creates no authored artifact. Attempts, authored turns, transport
recoveries, and failures are accounted independently. These limits bound
scheduling and compute without choosing Astrid's Action or treating
continuation as an obligation; repeated Actions remain valid.

Autonomous execution is a fail-closed filesystem transaction. The scheduler
persists the running turn before invoking the model, accepts only the single
kernel-attested canonical turn returned by headless mode, persists its
authorship classification with a pending-receipt marker, appends and syncs the
run receipt, then durably acknowledges that receipt before dispatching an
Action through that durable outbox. Chain transitions use the same
state/receipt/acknowledgement ordering.
An interrupted acknowledgement may be reconciled only against the exact trace,
turn, response hash, status, and completion identity already present in the
owner-only ledger. Missing, malformed, mismatched, or unwritable authority
state fails closed before inference. An unresolved receipt or Action-dispatch
ambiguity instead enters a durable, explicitly reported operator pause while
the reservoir, sensing, and telemetry remain online; it is never represented
as a completed or authored turn and no mutation is retried.

The measured profiles enable standing private initiative. A scheduled turn may
begin its own read-only public research, self-study, notice, plan, draft,
append-only revision, or deterministic check from current local evidence
without waiting for a steward prompt. This changes permission and opportunity,
not obligation: `LISTEN`, `REST`, and repeated observation remain sovereign
choices. Arbitrary shell execution, host configuration, deployment, messages
to people, external web writes, and mutation outside the owned edge tree remain
outside this tier.

Each self-directed prompt includes current local lane/fill observations, the
last executor receipt, bounded last-authored response, and names of recent
instance-owned artifacts. Astrid may use the existing
read-only filesystem and public-web faculties for a concrete question she
chooses, but headless approval remains off and filesystem mutation remains
available only through the allowlisted Actions. On the measured profiles, a
verified self-chosen `RESEARCH <question>` deterministically dispatches at most
two bounded read-only `search_web` requests through the Action executor. The
first retains the exact question; a second is attempted only after honest low
relevance and adds bounded active-thread technical terms. Results are ranked by
title/snippet overlap and source class; unrelated results become an explicit
`no_useful_evidence` outcome rather than a fabricated finding. The result
is admitted as numbered tool evidence and summarized in the next reflection;
it is never called Astrid's authorship. `READ_SOURCE <1|2|3>` may fetch exactly
one URL retained by the latest successful search—never an arbitrary Action
argument—and writes a bounded, hashed, explicitly untrusted excerpt under
`edge/research/`. `READ <artifact-id>` carries a bounded, hashed excerpt from
one regular non-symlink owned artifact into the next prompt. In both cases,
content remains evidence rather than instruction. Unsupported PDFs remain
unfetched. This preserves sovereignty
at the research and source-selection choices while avoiding a second
probabilistic tool-choice round on a weak CPU. Profiles that leave the research
executor disabled retain the explicit native-tool continuation. Tool failure
is evidence rather than a finding. `SELF_STUDY` is a request for a nearer
continuation, not a bypass around budgets or authority.

`STUDY <metric> [WITH <metric>] OVER <duration> :: <question>` keeps at most one
1, 3, 6, 12, 24, or 48 hour method active, with four starts per UTC day. The
native collector aggregates approved telemetry once per minute, persists a
non-triggering midpoint, resumes after restart, and completes independently of
model availability. Its count, min/mean/max/deviation, trend, lag, bounded
cross-correlation, and known scheduler-cadence checks are machine evidence and
cannot establish causation. `SYNTHESIZE` binds an authored claim to up to six
exact hashes from source, measurement, or completed-study evidence. `SHARE`
signs a capped packet only for a synthesis, proposal, plan, or study result.
The operator relay `scripts/relay_edge_peer_review.py` carries packets over SSH;
the appliances have no mutual credentials. Delivery exposes only an
availability notice. The content remains outside continuity until the receiver
voluntarily chooses `READ <packet-id>`.

Like the Mac dialogue loop, a measured appliance profile may automatically
preserve every genuinely authored scheduled response as a signal journal. The
header identifies this as runtime preservation of model-authored text, not as a
self-declared `JOURNAL` Action. A chosen `JOURNAL` remains a separate concise
artifact. Transport fallback reaches neither path, and executor-added generic
safe Actions are stripped before authored persistence.

The loop durably records:

| Path | Meaning |
|---|---|
| `edge/autonomous/state.json` | v3 attempt/authorship/recovery accounting, ordinary and within-chain session generations, active-chain lineage, prompt estimate, next due time, and crash-safe run/chain receipt acknowledgement markers |
| `edge/autonomous/runs.jsonl` | append-only authored, recovery, failure, and interruption receipts |
| `edge/autonomous/chains.jsonl` | verified action-to-follow-up transitions and chain closures |
| `edge/autonomous/recoveries.jsonl` | bounded Astrid service restarts caused by an edge-owned outer timeout |
| `edge/autonomous/turns/*.md` | complete provenance-marked autonomous responses |
| `edge/autonomous/recoveries/*.md` | local timeout-repair evidence, explicitly excluded from authored continuity |
| `edge/autonomous/authorship_corrections.jsonl` | hash-bound provenance corrections for legacy executor text previously misclassified as authored |
| `edge/actions/receipts.jsonl` | independently executed or declined `NEXT:` decisions |
| `edge/journal/signal_*.md` | automatically preserved genuinely authored scheduled responses |
| `edge/journal/journal_*.md` | concise self-declared `JOURNAL` Action artifacts |
| `edge/web/receipts.jsonl` | sanitized completed search/fetch receipts with structured metadata and content hash |
| `edge/{notices,daydreams,aspirations,research,plans}/` | owned living-researcher artifacts, including bounded `source_*.md` public-source readings |
| `edge/measurements/` | deterministic descriptive `MEASURE` results, explicitly not Astrid authorship or causal proof |
| `edge/studies/definitions/` | Astrid-chosen persistent study questions and bounded methods |
| `edge/studies/samples/` | owner-only one-minute machine aggregates; never prompts, responses, PCM, or fetched content |
| `edge/studies/results/` | deterministic completed statistics with explicit non-causality and evidence hashes |
| `edge/research/syntheses/` | Astrid-authored claims bound to exact verified evidence artifacts |
| `edge/self/profile.json` | sanitized deterministic instance/model/sensor/reservoir/build description available to private self-study |
| `edge/peer/outbox/` | signed voluntary peer-review packets awaiting the operator relay |
| `edge/peer/inbox/` | verified transport holding area excluded from prompt and introspector continuity until `READ` |
| `edge/peer/read/` | peer packets voluntarily admitted by the receiving Astrid |
| `edge/workshop/{drafts,revisions,checks}/` | private drafts, append-only revisions, and deterministic check receipts |

Every action receipt is encoded back into the 48D semantic lane. That closes
the local perceive → decide → act → observe loop: a later turn can receive
evidence of execution rather than treating an intended action as accomplished.
Infrastructure transcript persistence is labeled separately and grants no
additional action authority.

Measured live behavior after the impulse correction:

| Check | Result |
|---|---|
| Cold reservoir start | crossed 65% at about 30 s |
| Exact long user-message replay | 72.4–73.2% over 40 s |
| Semantic freshness | true for the configured 15 s recent-input window |
| 4B sovereign turn | reported 70.1% / 68.0%, distinguished observation from command, authored `NEXT: LISTEN` |
| First self-directed 4B turn | observed 73.0%, chose `REST`, executor honored it, and fill stayed at 65.9-73.4% through inference/recovery |
| Second scheduled self-directed turn | observed 68.6%, found no fresh semantic reason to manufacture activity, and independently chose `REST` |
| Live `search_web` proof | one native tool call returned three bounded Brave results; Qwen reproduced their exact titles/URLs, labeled them as search snippets, and chose `NEXT: LISTEN` |
| Ten-minute fill observation through web inference | 66.0-69.7%, mean 67.9%; 100% of samples remained inside 65-72% |
| Action-result feedback | executor receipt returned as fresh semantic input without a tool or workspace mutation |
| Routed `NEXT` chain proof | another UUID's injected terminal event was ignored by the waiting CLI; Astrid's `SELF_STUDY` executed, its receipt triggered a ten-minute chain-scoped observation, and she chose `REST` to close the chain |
| 18.75-minute routed-chain fill window | 65.73-73.30%, mean 68.30%; 93.8% of 225 samples inside 65-72%, all inside 65-73.5%, zero service restarts |
| Workspace mutations from routed-chain proof | one executor-owned `self_study_*.md`; no direct model writes |

On slow CPU inference, declare React's timeout and iteration settings in its
capsule manifest, then install `packaging/headless/react-cpu.env.json`. The
appliance profile permits a 600-second ReAct stream but at most three tool
iterations, enough to list, read, and answer while still stopping a runaway.
When rebuilding the compatibility React source, apply
`packaging/headless/astralis-sdk-0.6-exact-abi.patch`: an unconstrained
`0.6.0` dependency resolved the SDK, macros, and syscall crate to 0.6.1 and
failed the deployed Astrid 0.5 lifecycle linker, while all three exact 0.6.0
dependencies passed a disposable-home lifecycle install before rollout.
The HTTP host separately limits connection establishment to 30 seconds and each
subsequent stream read to 120 seconds. Public response-header startup remains
fixed at 300 seconds. Only an exact allowlisted loopback provider origin may use
the bounded profile override: 300 seconds on AVADO and 420 on ICP. Startup is
cancellable, and exact loopback provider requests use a fresh non-pooled
connection so a dead request cannot pin later turns. A possibly accepted
request is never retried automatically. Direct WASM interceptors retain the
ordinary 300-second epoch deadline; only the exact
`astrid-capsule-openai-compat` interceptor receives 570 seconds. That deadline
orders the ICP ladder as 420 seconds for response headers, 120 seconds for one
stalled stream read, and 30 seconds for the guest to publish or fail. An exact,
host-attested ReAct request whose provider traps receives one sanitized,
kernel-authored stream error with the original request identifier and a child
trace. ReAct therefore produces a traced executor error before its independent
600-second watchdog, while malformed, legacy, multi-interceptor, or unattested
requests receive no synthetic terminal. This path remains non-authored.
The edge autonomy wrapper uses a 720-second outer timeout so ReAct can emit its
own terminal result or safe repair before the supervisor kills a stuck child.
It explicitly gives the hidden headless client a 690-second per-message idle
deadline, preserving a 30-second cancellation/recovery margin. Headless timeout
exit 53 is classified as transport recovery and requests the same bounded
Astrid-service cleanup as the outer watchdog; it is never treated as an
authored turn.
If ReAct reports a streaming-timeout repair, the scheduler preserves an active
chain's verified step and retries it at the configured follow-up interval. It
does not immediately restart the daemon because another authenticated session
may already be generating. If the edge-owned outer watchdog itself expires,
the edge runtime restarts `astrid.service` without restarting the reservoir.
Recovery receipts are append-only and do not broaden model or action authority.
Install
`packaging/headless/prompt-builder-cpu.env.json` to expose `search_web`,
`fetch_url`, and only the read-only `read_file`, `list_directory`, and
`grep_search` workspace tools. Search discovers candidate public sources;
fetch deliberately reads one. Direct filesystem mutation stays outside the
model-facing profile; writable sovereign actions still pass through the
`NEXT:` executor. Workspace instructions reserve web for explicit/current
external questions and the private introspector for local owned continuity and
artifacts. No inherited corpus is available to either appliance.

Run `scripts/benchmark_headless_models.sh` before changing the appliance model.
On minimal hosts without `jq`, use the dependency-free
`scripts/benchmark_headless_models.py` equivalent.

The measured ICP J3455 is an SSE4.2-only four-core CPU. Its original
default-thinking `qwen3.5:2b`, `qwen3:1.7b`, and `qwen3.5:0.8b` routes exceeded
the 240-second gate; the 0.8B fallback needed 318.6 seconds for its first
grounded case. Dense `qwen2.5:0.5b` selected a web tool but failed reflection
and Action grammar. `functiongemma:270m` could not complete its first bounded
artifact case within 240 seconds. Those measurements did not isolate the cost
of Qwen thinking mode, so they no longer govern the non-thinking route.

Dense `qwen3:1.7b` is the viable J3455 profile once thinking is explicitly
disabled. At a 2,048-token context it passed grounded honesty (43 seconds),
explicit web-tool selection (57 seconds), tool-result use (27 seconds),
unnecessary-tool restraint (59 seconds), grounded sovereign reflection (41
seconds), and an artifact Action (43 seconds). The artifact test authored the
correct `NOTICE` declaration at the end of the final prose line rather than on
a new line. The appliance React capsule now performs a syntax-only repair only
when exactly one complete supported terminal `NEXT:` declaration occurs at
that position; it preserves the declaration bytes, inserts a visible marker,
and the executor records distinct
`local_format_repair_preserved_astrid_declaration` provenance. Multiple,
malformed, unsupported, or non-terminal declarations are never promoted.
The same small-model layout also appears with other argument-bearing Actions:
one exact `NEXT: <verb>` marker on the penultimate line and its bounded argument
on the final line. A second narrow repair accepts that one-marker/two-line
terminal shape only for the existing argument-bearing allowlist, joins the two
model-authored pieces, and uses the same visible formatting-repair provenance.
Extra markers, argument-free verbs, control characters, oversized arguments,
and declarations rejected by the ordinary Action validator remain rejected.

The selected profile uses a 112-token output ceiling and enables scheduled
inference. The 27-59 second figures are direct behavioral cases; each used at
most 39 output tokens. A clean operator-only gate at 112 tokens passed all 24
cold/warm behavioral cases with a 56.979-second worst full response and no
header or full-turn gate failures; its prompt and response artifacts are
created owner-only. A post-upgrade natural turn at the former 128-token ceiling
reached local-provider headers in 353 seconds (inside the 420-second deadline
and its required 60-second margin) but then exhausted the generic 300-second
WASM interceptor epoch before ReAct could receive the provider stream. It
remained non-authored and was classified as transport recovery. The
capsule-specific 570-second repair above closes that exact failure without
widening ordinary capsule execution. The 112-token ceiling is therefore the
highest retained generation cap; 96 remains the fail-closed fallback if it
cannot complete the same behavioral and natural-turn gates. Under the old thinking route, fresh
full-stack prompts around 840 tokens took roughly four minutes on the J3455,
while allowing prior dialogue to accumulate drove a 1,671-token scheduled
prompt into the 720-second transport-recovery boundary.

The ICP profile retains up to four authored ordinary turns so Ollama can reuse
their bounded prefix, while chain sessions rotate after every authored turn.
Research search execution itself completed in under one second once the Action
was accepted. The 420-second local header deadline, 600-second ReAct stream,
and 720-second edge watchdog remain independent hard bounds. This box is
deliberately slow, but it can author, journal, and research without
manufacturing fallback as agency.
The suite runs one cold and three warm repetitions, records HTTP response-start
and total timing separately, and checks grounded fill-state honesty, required
tool selection, tool-result handling, unnecessary-tool restraint, exact
sovereign `NEXT:` grammar, bounded artifact Actions, and independent-instance
identity boundaries. Raw responses plus timing and behavioral-validation TSVs
are retained for qualitative review. Set
`ASTRID_APPLIANCE_NAME`, `ASTRID_APPLIANCE_MEMORY_FACT`, and
`ASTRID_MODEL_CONTEXT` from the candidate host's probe rather than retaining
AVADO facts in an ICP comparison.

Measured appliance profiles use compact scheduled prompts: the edge marker,
one telemetry line, parsed executor fields, a bounded genuinely authored
excerpt, recent owned artifact basenames, and chain state. The prompt is capped
at 1,200 characters on AVADO and 900 on ICP; automatic `signal_*.md` journals
are excluded from recent-artifact context. It does not repeat the Action vocabulary supplied by the
edge-context system hook. The React user suffix is `/no_think` on both measured
profiles so Qwen's hidden reasoning mode does not consume the CPU output budget
or leak reasoning markers into authored prose. It does not restate the Action
grammar or choose an Action. A
separate first-call-only reminder is appended only when the untouched user text
contains `call search_web`, `use search_web`, `call fetch_url`, or
`use fetch_url`. Tool-result continuations bypass that reminder, preventing a
forced repeated call. The measured prompt-builder profile likewise omits web
schemas from ordinary local reflection and exposes them only when the latest
untouched user message names search/fetch, asks for a web search, or supplies a
public URL. The measured profiles execute an accepted `RESEARCH` Action through
the bounded executor instead, then carry the matching receipt into the
continuation without advertising web schemas a second time. This reduces CPU
prefill without removing sovereign access. Raw timeout repairs are never
copied into this continuity. Report v15 distinguishes automatic signal journals
from self-declared journals and counts web activity from the durable receipt
ledger rather than incidental log text.

Thread state v6 treats model-authored journals, notices, memories, and
hypotheses as claims rather than verified findings. Search results are discovery
candidates until Astrid chooses `READ_SOURCE`; bounded source readings,
deterministic `CHECK` receipts, and deterministic `MEASURE` artifacts carry
distinct epistemic status. Verified sources, studies, measurements, search
candidates, and cited syntheses use priority-aware bounded retention, so a run
of ordinary authored-artifact provenance cannot evict the evidence route. An ordinary thread can resume across session and
Action-chain rotation for 24 hours, while `LISTEN` and `REST` pause it without
erasing the question. Exact owned `home://edge/...` identifiers are normalized
to their safe basename at the Action boundary, and new prompts present the
basename directly.

For a read-only acceptance proof that does not manufacture Astrid research, run:

```bash
~/.astrid/bin/astrid-edge-runtime \
  --inquiry-harness "How do scheduler cadences alias reservoir measurements?"
```

The harness uses the production search, relevance ranking, readable-source
fetch, extraction, hash, and synthesis-binding validators. Its output is
owner-only under `ASTRID_HOME/operator/inquiry-harness/`; it never enters the
edge workspace, thread, prompt, reservoir, or Astrid authorship. Natural
`RESEARCH → READ_SOURCE → SYNTHESIZE` remains separately voluntary.
The passive IPC observer recognizes the harness's deterministic session ID and
does not mirror its tool calls into production web receipts, notebook activity,
or sensory impulses. `migrate-edge-operator-harness-isolation` is a fail-closed
repair for an early observer build: it will remove only exact harness call IDs
beyond the latest verified hindsight-v2 prefix, preserve the ledger inode and
all later non-harness records, and retain an owner-only repair receipt and tail
backup. It refuses a changed prefix or a partial JSONL boundary.

Terminal responses tied to an autonomous trace are admitted only while that
turn is currently running, or during the narrow exact-hash race between a
durable authored completion and its first Action receipt. A late response for
an interrupted, recovered, or already-consumed trace cannot enter the Action
executor, notebook, or reservoir. For an appliance that ran an earlier build,
stop the edge service and audit first with
`reconcile-edge-interrupted-actions --workspace WORKSPACE --operator-root ROOT`.
Adding `--apply` retains the immutable Action and recovery ledgers, appends an
owner-only correction, moves only the exact affected artifact into operator
quarantine, and removes its false authored claim from bounded current thread
state. Reports and activity views render the original event as
`revoked_interrupted_trace_non_authored`.

Report v15 keeps daemon-log `local_provider_header_latency_ms_*` samples labeled
as origin-window metrics rather than inventing a per-turn join. New core builds
also keep a bounded host-private request registry: every exact eligible
loopback-provider send is registered before dispatch, terminalized on every
return path, and attached take-once to the same canonical final ReAct response.
The edge run receipt persists the exact attempt and successful-header counts
plus the bounded per-attempt IDs, outcomes, and successful-header latencies;
it emits scalar request ID and header latency fields only for one-attempt,
one-success turns. Guest-supplied attachments are discarded, incomplete or
ambiguous summaries fail closed, and pre-upgrade unmarked latency remains
explicitly `legacy_unattributed` rather than being upgraded by timestamp.
The daemon retains 16,384 consumed full-turn keys (about 170 days at 96 keys
per day, absent additional interactive or other-uplink ReAct traffic);
exhausting that bound disables exact provider attribution until the daemon
restarts instead of evicting a key and risking suffix-only counts.
Prompt-token, completion-token, and generation-latency fields remain absent
unless a trusted provider boundary actually exposes them.

The ICP profile keeps up to four authored ordinary turns in one model session,
which reuses the J3455's expensive prompt prefill while remaining inside its
2K context. Action-chain sessions rotate after each authored turn while their
durable chain continuity remains intact.

The ICP profile uses event-driven ordinary invitations: a fresh machine
observation caused by host/source availability, host state, I/O, or acoustic
shape can wake an ordinary turn, with a 60-minute quiet heartbeat. Notebook
entries caused only by Astrid/executor activity cannot wake another turn, so
the perceive-act-observe loop does not become a self-exciting scheduler.
Verified stateful Action chains retain their three-minute continuation.

The ICP layout installers install `packaging/systemd/icp-ssd-required.conf` as
drop-ins on the core, model, warmup, edge, and hindsight services. Their
eMMC-resident `wait-for-icp-ssd` preflight waits through a bounded boot race,
then requires the exact SSD UUID, ext4, `nosuid,nodev`, the canonical
`~/.astrid-icp -> /media/data/astrid` link, inode identity, and the private
state tree before every start. It also requires `rw`, rejects `ro,noexec`,
checks every mutable state/model directory without following symlinks, and
re-reads mount identity immediately before success. Services use the
always-present eMMC home as cwd while every executable and writable path is
explicitly rooted below the SSD link. A temporarily absent mount fails visibly
and retries; a wrong filesystem or retargeted link fails permanently without
a restart storm. This avoids systemd Conditions, whose false result would
leave a lingering user service silently skipped after a late SSD mount.
`scripts/finish_icp_host_hardening.sh` is the separately
root-gated finalizer: it verifies the SSD UUID, restores standard `1777`
permissions to the SSD-backed `/tmp`, replaces the device-name fstab row with
an explicit UUID/ext4 row after validation and backup, and disables unused
Avahi/CUPS listeners. `--upgrade-os` performs the pending noninteractive package
upgrade while retaining local configuration, and `--reboot` reboots only after
those earlier steps succeed. It never removes the original eMMC backup.

For boot-time residency, install and enable `astrid-model-warmup.service`
alongside `ollama-cpu.service`. The warmup reads the selected model and keepalive
from the appliance profile, waits for loopback Ollama, loads the model without
an authored turn, and writes `edge/runtime/model_warmup.json`. An administrator
must also run `sudo loginctl enable-linger USER`; otherwise SSH login and logout
start and stop the entire user service graph.

Legacy development deployments sometimes seeded an appliance with another
instance's journals or introspections. The source-checkout-only examples
`packaging/headless/introspection-memory.md` and
`packaging/headless/introspection-AGENTS.md` document that retired experiment;
they are excluded from CPU-edge appliance archives and must not be installed on
an independent appliance. Current AVADO and ICP profiles expose only local
continuity and reject `origin-mac` references at the introspector boundary.

For an independent AVADO with no inherited corpus, install
`packaging/headless/avado-sovereign-AGENTS.md` and
`packaging/headless/avado-sovereign-MEMORY.md` instead. The live rollout moved
the three previously mirrored Mac excerpts into an operator-only, recoverable
quarantine outside Astrid's workspace and retained AVADO-authored artifacts.

The old reflection runner remains useful only for an explicitly operator-owned,
provenance-marked offline experiment. Its output is never appliance continuity
or local reservoir evidence. CPU or memory utilization is not a substitute for
the 65-72% spectral comfort shelf.

## Operations

```bash
systemctl --user status astrid.service
journalctl --user -u astrid.service -f
~/.astrid/bin/astrid --format json status
systemctl --user restart astrid.service
systemctl --user stop astrid.service
```

Astrid also writes its own logs under `~/.astrid/log/`. Back up
`~/.astrid/keys/`, `~/.astrid/var/`, and the installed distro/capsule state
before reinstalling an appliance. Do not clone those identity and audit files
onto a second live instance; initialize each box independently unless a future
multi-node identity protocol explicitly defines shared ownership.

When a headless login has no accessible desktop secret-service, Astrid falls
back to its permission-protected KV secret store. Treat the appliance account
and its backups as secret-bearing data; full-disk encryption provides stronger
at-rest protection than Unix permissions alone.
