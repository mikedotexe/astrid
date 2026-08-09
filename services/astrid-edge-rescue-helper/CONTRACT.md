# CPU-edge rescue-helper contract

`astrid-edge-rescue-helper` is a native member of the immutable rescue root. It
is never a capsule or model tool and accepts no shell text, commands,
environment overrides, network input, prompts, responses, or arbitrary service
names. The root supervisor invokes only these digest-pinned public profiles:

```text
/usr/libexec/astrid/astrid-edge-rescue-helper --config /etc/astrid/edge-rescue-helper.json verify-install
/usr/libexec/astrid/astrid-edge-rescue-helper --config ... build --candidate-manifest ABS --intent-envelope ABS --model-handoff ABS --build-manifest ABS
/usr/libexec/astrid/astrid-edge-rescue-helper --config ... install --build-manifest ABS
/usr/libexec/astrid/astrid-edge-rescue-helper --config ... activate --generation-dir ABS --previous-generation-dir ABS
/usr/libexec/astrid/astrid-edge-rescue-helper --config ... rollback --generation-dir ABS
/usr/libexec/astrid/astrid-edge-rescue-helper --config ... health
/usr/libexec/astrid/astrid-edge-rescue-helper --config ... reflection-prepare
/usr/libexec/astrid/astrid-edge-rescue-helper --config ... reflection-cleanup
/usr/libexec/astrid/astrid-edge-rescue-helper --config ... reflection-reconcile
```

`verify-install` first reconciles an interrupted active-link/generation-binding
switch from the immutable transition journal and only then verifies the
installation. The independently pinned checkpoint helper has these exact
profiles:

```text
/usr/libexec/astrid/astrid-edge-checkpoint flush --workspace ABS --output ABS --generation-id ID
/usr/libexec/astrid/astrid-edge-checkpoint checkpoint --workspace ABS --output ABS --generation-id ID --reason REASON --maximum-age-seconds N
/usr/libexec/astrid/astrid-edge-checkpoint verify-health --workspace ABS --generation-id ID --maximum-age-seconds N
/usr/libexec/astrid/astrid-edge-checkpoint snapshot --workspace ABS --output ABS --generation-id ID --require-dual-readable
/usr/libexec/astrid/astrid-edge-checkpoint verify-snapshot --snapshot ABS --generation-id ID
```

The flush, checkpoint, and snapshot profiles require root. The health and
snapshot verification profiles are read-only. They accept no command text,
environment overrides, network input, or paths outside their compiled state
layout.

`build`, `install`, `activate`, and `rollback` are root orchestration
operations. The build helper copies and applies source as root, makes source
files/directories `0444`/`0555`, and then drops each candidate-controlled
Cargo, Python, and capsule-build child to the configured non-root builder
UID/GID. Only build targets, Cargo home, and capsule archive output beneath the
disposable build target are builder-writable. Children receive a cleared,
fixed environment, enter a new process group, and cannot outlive the fixed
command boundary. On Linux the helper first enables and verifies child-subreaper
mode and refuses to spawn when its child boundary is already occupied. After
every success, failure, timeout, or capture-setup failure it kills the original
process group, repeatedly kills every direct child listed by
`/proc/self/task/*/children`, and reaps until `waitpid` reports that no child
remains. Subreaper reparenting turns deeper descendants into direct children on
the next pass without requiring cross-UID `/proc` visibility. A candidate using
`setsid`, `setpgid`, or a double fork therefore cannot survive into artifact
verification or packaging. Any inability to prove an empty boundary aborts the transaction.
The helper also runs inside the root-owned build/update unit's namespace,
cgroup, no-network, filesystem, device, and syscall envelope.

`install` does not materialize a release directly as root. It invokes the same digest-pinned
helper under the distinct updater UID/GID with only a fixed source directory, exact updater-owned
`generation-staging` destination, and expected bundle hash. The updater can write only that
private root and cannot write releases, supervisor state, snapshots, units, or builder state. Root
then re-hashes and validates every updater-owned byte, copies it into a root-owned partial release,
seals the tree, and alone performs the final release rename and later pointer switch. Runtime,
steward, and builder identities cannot read or write the updater staging root.

Before materializing source, `build` independently verifies the exact processed
scheduled-intent envelope and joins its attestor signature, trace/session/turn,
response hash, terminal declaration hash, candidate digest, appliance identity,
and base generation to the separately signed model-unload handoff. The
supervisor's orchestration order is not treated as authorization evidence.

Two additional subcommands, `verify-candidate` and `verify-package`, are
internal fixed replays called only by this same pinned helper during `build`.
They are not supervisor profiles. A byte-identical helper copy named `rustup`
implements only `rustup target list --installed` and reports the already
attested `wasm32-wasip2` target so the fixed `astrid-build` can operate with a
standalone signed toolchain. Every other invocation fails.

## Exact root-owned configuration

Unknown fields are denied. This AVADO example shows every field; ICP uses the
same schema with its managed state/source/build/release roots beneath the
guarded SSD. In both cases `generation_binding` is exactly
`supervisor_state/current-generation`. It is mutable supervisor state and must
not be placed statically under `/etc`.

```json
{
  "schema": "astrid.edge_rescue_helper.config.v1",
  "appliance_id": "avado-astrid",
  "target": "x86_64-unknown-linux-gnu",
  "model": "qwen3.5:4b",
  "ollama_origin": "http://127.0.0.1:11434",
  "source": {
    "root": "/opt/astrid-edge/trusted/source/generation-1/astrid-edge-self-change-source",
    "manifest": "/opt/astrid-edge/trusted/source/generation-1/astrid-edge-self-change-source/MANIFEST.json",
    "signature": "/opt/astrid-edge/trusted/source/generation-1/astrid-edge-self-change-source/MANIFEST.signature.json",
    "signing_key": "/etc/astrid-edge-self-change/source-signing.key",
    "intent_attestation_key": "/etc/astrid-edge-self-change/intent-attestation.key",
    "vendor": "/opt/astrid-edge/trusted/source/generation-1/astrid-edge-self-change-source/vendor"
  },
  "roots": {
    "supervisor_state": "/var/lib/astrid-edge-self-change",
    "candidate_store": "/var/lib/astrid-edge-steward-helper/candidate-outbox",
    "model_handoff_root": "/var/lib/astrid-edge-steward-helper/model-handoff",
    "model_handoff_ledger": "/var/lib/astrid-edge-steward-helper/model-unload-receipts.jsonl",
    "candidate_work": "/var/lib/astrid-edge-builder/work",
    "build_store": "/var/lib/astrid-edge-builder/builds",
    "releases": "/opt/astrid-edge/releases",
    "active_link": "/opt/astrid-edge/current",
    "generation_binding": "/var/lib/astrid-edge-self-change/current-generation",
    "maintenance_lease": "/var/lib/astrid-edge-self-change/maintenance.json",
    "maintenance_mutex": "/var/lib/astrid-edge-self-change/maintenance.lock",
    "state_snapshots": "/var/lib/astrid-edge-updater/snapshots",
    "workspace": "/home/avado/.astrid/home/default/edge",
    "system_unit_root": "/etc/systemd/system",
    "unit_policy": "/var/lib/astrid-edge-self-change/unit-policy.json",
    "unit_transactions": "/var/lib/astrid-edge-updater/snapshots/unit-transactions"
  },
  "identities": {
    "steward_uid": 980,
    "steward_gid": 980,
    "builder_uid": 981,
    "builder_gid": 981,
    "updater_uid": 982,
    "updater_gid": 982,
    "runtime_uid": 1000,
    "runtime_gid": 1000
  },
  "executables": {
    "cargo": {"path": "/opt/astrid-edge/toolchain/bin/cargo", "sha256": "HEX64"},
    "rustc": {"path": "/opt/astrid-edge/toolchain/bin/rustc", "sha256": "HEX64"},
    "rustfmt": {"path": "/opt/astrid-edge/toolchain/bin/rustfmt", "sha256": "HEX64"},
    "python": {"path": "/usr/bin/python3", "sha256": "HEX64"},
    "systemctl": {"path": "/usr/bin/systemctl", "sha256": "HEX64"},
    "systemd_analyze": {"path": "/usr/bin/systemd-analyze", "sha256": "HEX64"},
    "checkpoint": {"path": "/usr/libexec/astrid/astrid-edge-checkpoint", "sha256": "HEX64"},
    "capsule_builder": {"path": "/usr/libexec/astrid/astrid-build", "sha256": "HEX64"},
    "invariant_runner": {"path": "/usr/libexec/astrid/astrid-edge-rescue-helper", "sha256": "HEX64"},
    "package_verifier": {"path": "/usr/libexec/astrid/astrid-edge-rescue-helper", "sha256": "HEX64"}
  },
  "services": {
    "core": "astrid.service",
    "warmup": "astrid-model-warmup.service",
    "edge": "astrid-edge-runtime.service"
  },
  "drain": {
    "autonomy_state": "/home/avado/.astrid/home/default/edge/autonomous/state.json",
    "model_lock": "/var/lib/astrid-edge-self-change/model.lock",
    "model_lock_gid": 983,
    "maintenance_edge_acknowledgement": "/home/avado/.astrid/home/default/edge/runtime/maintenance-edge-ack.json",
    "maintenance_core_acknowledgement": "/home/avado/.astrid/run/maintenance-core-ack.json",
    "activity_ledgers": [
      "/home/avado/.astrid/home/default/edge/actions/receipts.jsonl",
      "/home/avado/.astrid/home/default/edge/web/receipts.jsonl",
      "/home/avado/.astrid/home/default/edge/introspection/receipts.jsonl"
    ],
    "maximum_wait_seconds": 900,
    "poll_milliseconds": 500
  },
  "policy": {
    "maximum_files": 25,
    "maximum_changed_lines": 4000,
    "build_workers": 4,
    "command_timeout_seconds": 1800,
    "pipeline_timeout_seconds": 86400,
    "maximum_candidate_bytes": 16777216,
    "minimum_free_disk_bytes": 4294967296,
    "network_policy": "private-network-none:v1",
    "dependency_policy": "signed-vendor-offline-locked:v1"
  },
  "health": {
    "sensor_state": "/home/avado/.astrid/home/default/edge/runtime/spectral_state.json",
    "hindsight_state": "/home/avado/.astrid/home/default/edge/operator/hindsight/latest.json",
    "fill_history": "/home/avado/.astrid/home/default/edge/runtime/fill_history.jsonl",
    "meminfo": "/proc/meminfo",
    "swaps": "/proc/swaps",
    "thermal_celsius": "/sys/devices/virtual/thermal/thermal_zone0/temp",
    "maximum_age_seconds": 1200,
    "maximum_thermal_celsius": 85.0,
    "minimum_available_ram_bytes": 2147483648,
    "maximum_swap_bytes": 134217728,
    "minimum_fill_samples": 10
  }
}
```

The installer substitutes actual numeric identities, hashes, thermal path, and
workspace paths. Every trusted executable is absolute, root-owned, regular,
single-link, non-group/world-writable, and digest-matched. `invariant_runner`
and `package_verifier` must be the same rescue-helper path and digest. The
source HMAC key remains separate. The supervisor, source, candidate, work,
build, release, snapshot, and workspace roots are disjoint. Steward, builder,
updater, and runtime are distinct non-root identities. The only service names
accepted are the three shown above.

`model_lock_gid` identifies a fifth, dedicated
`astrid-edge-model-lock` group. It is nonzero and distinct from all four
primary role groups; only runtime and steward are members. The persistent lock
is root-owned, single-link, exactly `0640`, and its parent is root-owned and
not group/world writable. Runtime and steward hold shared `flock` locks while
using the model. Transition code opens it read-only with `O_NOFOLLOW` and must
obtain an exclusive lock before trusting a drain. No mutable identity can
replace the lock or its parent.

The root maintenance lease is schema
`astrid.edge_self_change.maintenance_lease.v2`. It carries a random
`lease-` plus 24 lowercase-hex identifier, a 64-lowercase-hex nonce, exact
creation/expiry times, reason, and owner
`immutable_astrid_edge_rescue_helper`. Mutable services acknowledge that
exact lease with schema `astrid.edge.maintenance_ack.v2`,
`lease_schema=astrid.edge_self_change.maintenance_lease.v2`, and
`lease_kind=generation_transition`. The acknowledgement binds `lease_id`,
SHA-256 of the nonce, SHA-256 of the canonical lease payload, generation,
blocked/acknowledged times, PID, process-start ticks, and authority
`mutable_runtime_acknowledgement_subject_to_immutable_verification`.

The edge acknowledgement additionally proves `new_work_blocked=true`, an
exact hash/size index of `autonomous/state.json`, and exact full-file indexes
for Action, web, and introspection ledgers. Immutable code independently
requires autonomy v3, no running turn, false Action/run/chain pending flags,
and JSON `null` (not Boolean false) for `thread_projection_pending`. It opens
each ledger once, rechecks inode/size/mtime, hashes every byte, and derives a
zero pending count. V2 web and v1 introspection requests must have one ordered,
unique completion. Historical completion-only receipts are accepted only for
the exact allowlisted legacy schemas with a terminal status, valid result
hash, and explicit non-authorship authority; they never join by timestamp.
The core acknowledgement separately proves `ipc_user_input_blocked=true` and
zero active conversations, sessions, tools, and local-provider LLM requests.
The helper rechecks both live PIDs, UIDs, start ticks, active-generation
executables, file ownership, and
freshness after acquiring the exclusive model lock. It binds that exact
drain-barrier generation, lease, nonce/payload hashes, PID/start identities,
autonomy state, full ledger indexes, and barrier sequence. The binding is
revalidated after flush, after checkpoint, immediately before stop, after the
stopped-state snapshot, and immediately before either generation switch. Only
the periodically refreshed `acknowledged_at_unix_ms` may differ; disappearance
or any substantive change aborts and restores the prior selection.

## Scheduled-reflection admission

Scheduled reflection uses a separate root protocol and cannot borrow
generation-transition authority. `astrid-edge-steward.service` first invokes
the root helper's fixed `reflection-prepare` profile. When the private steward
schedule is due, the helper serializes on the same root-owned maintenance
mutex as activation, rejects either generation-transition lease or any prior
reflection artifact, and creates
`/run/astrid-edge-self-change/reflection.json` with schema
`astrid.edge_scheduled_reflection.lease.v1`,
`lease_kind=scheduled_reflection`, the current boot, systemd invocation, and
generation, and a nonce-bound identifier. The lease is root/runtime-group
`0440`; it blocks new core and edge work but grants no activation authority.

Core and edge acknowledge the reflection lease with the same exact
`astrid.edge.maintenance_ack.v2` envelope, but with the reflection lease schema
and kind. The IPC barrier itself is independently and exactly
`astrid.edge.maintenance_barrier.v2`. Root requires equal barrier sequences,
zero conversations, sessions, tools, local-provider requests, scheduled work,
Actions, and continuations, plus exact clear edge indexes. It then acquires the
model lock and revalidates both ACKs, live runtime UIDs/start ticks,
active-generation executables, and the unchanged lease. Only then may it write
the distinct root/steward-group `0440` admission marker
`reflection-admission.json` with schema
`astrid.edge_scheduled_reflection.admission.v2`. The unprivileged steward
accepts only that exact current-boot, current-invocation, current-generation
lease/marker pair and rechecks it before and after every provider call.

The shared runtime parent is deliberately `root:root 0755`. This grants only
directory traversal: neither the runtime nor steward UID can create, unlink,
or replace an entry. Root creates the lease `root:<runtime-group> 0440` and the
marker `root:<steward-group> 0440`; those groups provide read access only. The
core and edge unit namespaces bind their persistent maintenance state
read-only, and no mutable service receives a writable bind or DAC capability
for this root-owned runtime parent. The steward's systemd `RuntimeDirectory`
is distinct and does not confer authority over these proofs.

`reflection-cleanup` is an `ExecStopPost=+` operation in the same systemd
invocation, so normal completion, main-process failure, and SIGKILL remove only
the byte-identical reflection lease and marker belonging to that invocation.
The generation guard runs `reflection-reconcile` before mutable services and
removes only exact prior-boot reflection artifacts. It refuses to remove a
current-boot admission. Generation transitions reject both reflection files
while holding the shared mutex, recheck after generation-lease creation, and
recheck immediately before a pointer switch. Neither protocol parses, removes,
or treats the other's lease as its own.

`verify-install` also proves exact Rust 1.94.1 release/commit/host identity,
the signed source inventory, root/role directory ownership, and that all three
root service `ExecStart` values pass through `/opt/astrid-edge/current` (or the
configured active-link equivalent). The current generation contains
`astrid`, `astrid-daemon`, `astrid-build`, and `astrid-edge-runtime` at its
root, plus `capsules/`, `scripts/`, and `packaging/`.

The checkpoint helper independently validates hindsight checkpoint v2 and its
hash chain, the exact captured prefixes of every compiled allowlisted ledger,
the required core ledger subset, `runtime/fill_history.jsonl`, current-epoch
continuity, owner-only modes, host-boot identity, and SQLite `quick_check` plus
schema. It opens each prefix once and rejects replacement or mutation of any
captured byte while deliberately ignoring append bytes beyond the captured
size until the next checkpoint. Existing v2 checkpoints without a boot ID are
accepted only when their timestamp is after the kernel boot; this is a bounded
legacy migration rule, not a continuity rewrite. Root checkpoint receipts and
rollback fixtures are private, content-inventoried, hash-bound to the
generation, and contain no prompt, fetched body, model output, or secret.
Root evidence writes use random collision-resistant temporary names, durable
file and parent-directory sync, and Linux `renameat2(RENAME_NOREPLACE)` where
replacement is forbidden. A stale temporary file or competing destination
therefore cannot make immutable evidence overwrite an earlier record.

## Build and package transaction

The exact candidate is `astrid.edge_self_change.candidate.v1`. Its canonical
full-replacement patch is derived only as
`candidate-patch-<patch_sha256>.json` beneath `candidate_store` and binds the
signed source ID, active base generation, candidate, ordered paths, original
hashes, and replacement hashes. The root generation binding, active symlink,
and validated base manifest must agree before any candidate command starts.
The helper also requires the exact per-envelope
`astrid.edge.steward_helper.model_unload_handoff_envelope.v1` file and its
identical signed JSONL record. It verifies the separate intent-attestation HMAC,
appliance, envelope, intent, candidate ID and digest, configured model and
loopback origin, canonical `{model,keep_alive:0}` request digest, provider
result binding, `unload_confirmed`, `build_ready=true`, and
`attempt_count=1`. A ledger record without the per-envelope file is never
sufficient.

Allowed mutable source covers the CPU-edge core crates and root manifest,
`astrid-edge-runtime`, all ten essential Astralis capsule sources, edge reports,
appliance profiles, and the six exact AVADO/ICP base fragments carrying signed
origin `mutable_astrid_service_template`. Every other service, timer, drop-in,
and host unit remains inspect-only, build-required, or excluded. Reviewed
rescue/steward/broker/checkpoint source and root policy are signed as
`inspect_only_immutable_boundary`: the model may read them but neither broker nor this independent
verifier accepts them as candidate paths. Mac/Minime/spectral-bridge paths, hidden or arbitrary
paths, links, devices, binaries, stale hashes, privilege-growing units, and unvendored dependency
changes are rejected. Limits are exactly 25 files and 4,000 deterministic
insertion/deletion line edits.

Cargo receives `--offline --locked`, signed-vendor replacement, exact
Rust/Rustfmt, private Cargo home and targets, fixed jobs, and no inherited
environment. Candidate commands cannot modify the root-owned source; source
membership and every declared/undeclared byte are reverified before immutable
replay. The fixed gates include metadata, formatting, strict Clippy, focused
and workspace tests, report tests, systemd verification, native release builds,
deterministic shadow-reservoir replay, migration policy checks, and package
replay.

Changed essential capsules are tested and rebuilt as Component Model
`wasm32-wasip2` archives by the digest-pinned `astrid-build`. A core change
rebuilds all ten. Unchanged capsule archives are copied byte-for-byte from the
exact validated base generation. The final package must contain exactly these
ten `.capsule` archives:

```text
astrid-capsule-cli                 astrid-capsule-fs
astrid-capsule-http                astrid-capsule-shell
astrid-capsule-skills              astrid-capsule-agents
astrid-capsule-memory              astrid-capsule-edge-context
astrid-capsule-edge-introspector   astrid-capsule-edge-spectral
```

Package replay validates four target-architecture ELF64 binaries, every safe
capsule archive and Component Model header, exact capsule membership, and the
install layout. Success emits exact supervisor Build v1, hashed command
evidence, and a deterministic payload inventory. It emits no source body,
diff, prompt, response, fetched page, or build log into public telemetry.

Temporary infrastructure refusal is retryable only before the first
candidate-controlled command starts. It exits 75 and writes exactly this
canonical JSON class to stdout:

```json
{"reason":"BOUNDED_REASON","retry_authority":"immutable_supervisor_may_retry_after_condition_clears","schema":"astrid.edge_rescue_helper.result.v1","status":"deferred_infrastructure"}
```

After the first candidate command begins, every error is terminal exit 1. The
supervisor records the failed candidate hash and never retries that hash
automatically.

## Install, activation, rollback, and health

`install` revalidates Build v1, stored evidence, package digest, and exact
payload; creates a new root-owned read-only generation; and refuses replay.
Both candidate generation manifests and the operator-packaged initial
`astrid.edge_self_change.initial_generation.v1` manifest are validated down to
their complete inventories. The manifest itself is excluded from the payload
digest and verified separately.

Activation obtains the v2 maintenance lease, waits for the exact edge/core
acknowledgements, and holds the exclusive model lock for the complete
transaction. It records `planned` and `drained_and_model_locked`, flushes
durable state, and records a valid hindsight checkpoint while services still
run. Before stopping, it seals both prior and target forms of exactly
`ollama-cpu.service`, `astrid-model-warmup.service`, `astrid.service`,
`astrid-edge-runtime.service`, `astrid-edge-hindsight.service`, and
`astrid-edge-hindsight.timer` beneath the private root transaction directory.
It verifies the target set with `systemd-analyze verify`, the immutable
operator policy, and the exact effective drop-in membership. It then stops
edge followed by core and records `runtime_stopped`.
Because those services continuously own databases and append-only state, the
full rollback-compatible snapshot and its independent verification occur only
after both services have stopped. Snapshot/verification commands may use the
configured timeout up to 3,600 seconds; every other transition command is
capped at 300 seconds.

The helper atomically replaces only those six base fragments, durably syncs
each replacement, runs `daemon-reload`, and verifies systemd's effective
`FragmentPath` and `DropInPaths` before the generation pointer can move. The
root-owned policy pins every immutable drop-in by path, size, and SHA-256 and
requires each `90-root-runtime-boundary.conf` plus the edge runtime's
`60-self-evolution-root.conf`. Rescue, steward, broker, guard, SSH, sudo, host,
and arbitrary units are never transaction targets. An uncommitted failure
restores the sealed prior fragment set; the pending transaction is cleared
only after prior restoration or the outer generation journal's durable commit.

The active symlink and root generation binding are two separately durable
objects and are never described as atomic together. Before changing either,
the helper appends `switch_intent_recorded` to a root-owned, hash-chained phase
journal. It then switches the symlink, writes the binding, validates both, and
appends `pointer_and_binding_switched`. It reloads only validated Astrid units,
clears failed/start-limit state only for the three fixed Astrid units, starts
core (which loads capsules), warmup, then edge, and records a
post-switch hindsight checkpoint before creating one-hour probation and
appending `probation_started`. Any activation failure after stop—including
snapshot, switch, startup, binding, checkpoint, probation, or phase-journal
failure—stops mutable services as needed, restores both prior pointers, starts
the prior generation once, and validates it.

Rollback uses the same exact drain, flush, checkpoint, stop, journaled switch,
start, binding, and post-checkpoint boundary. It appends
`rollback_target_validated` only after the rollback target has started and its
post-switch checkpoint has passed. That is the rollback commit point. The
helper then closes the old probation and appends `completed`. If this final
evidence write fails, it stops mutable services rather than returning to the
failed candidate; boot reconciliation retains the already validated rollback
target and retries the idempotent closure.

The generation guard invokes reconciliation before any mutable unit starts.
For an interrupted uncommitted activation or rollback it durably restores the
prior generation. A committed activation at `probation_started` retains its
target. A rollback at `rollback_target_validated` or `completed` retains its
target. Link and binding are rewritten and revalidated before the journal is
advanced, so repeating reconciliation after power loss is safe. A malformed
or partially appended journal fails closed to operator rescue rather than
guessing. If the probation `started` record survived but the activation commit
phase did not, reconciliation closes that ledger-only orphan before restoring
the prior generation. The same outer-journal selection also completes any
partially installed six-fragment transaction, reloads systemd, and verifies
the effective fragments and immutable drop-ins. With no pending transaction,
boot reconciliation still refuses to proceed unless the active generation's
six normalized fragments exactly match the live manager. The maintenance
lease is removed on every normal path and expires if its process is lost.

`health` requires the fixed services healthy at zero restarts, fresh sensing,
valid independently checked hindsight/database state with zero current-epoch
violations, at least 2 GiB available RAM, at most 128 MiB swap, safe
temperature, and bounded fill mean 67–70% with at least 90% occupancy in
65–73.5%. Sensor v2 requires its exact record hash and reservoir generation;
the installed v1 compatibility path is explicitly reported as legacy
unhashed. Audio may be unavailable on ICP, but auxiliary sensing must remain
fresh and its source is reported.

Probation evidence is a private root-owned hash-chained ledger with an atomic
state cache. The ledger is authoritative, including if power fails after an
append but before cache replacement. Health sampling must cover at least 57
minutes, contain at least 648 five-second samples, have no gap over 20 seconds,
and meet the fill/resource/service/hindsight gates throughout one elapsed
hour. At least seven immutable health evaluations are required with no gap
over ten minutes, and swap growth from the recorded baseline may not exceed
128 MiB. The immutable supervisor owns promotion or rollback; `health` never
promotes a generation itself.

### Required aggregate-storage boundary before live self-change is enabled

`LimitFSIZE` bounds one file, not the sum of daemon and edge writes. The
existing exact-workspace `statvfs` gate detects low host free space and causes
probation failure, but it is not a pre-exhaustion containment boundary and it
does not reserve room for rollback. Live autonomous activation therefore
remains fail-closed until the installer and rescue configuration implement the
following immutable topology:

- A fully allocated 32 GiB ext4 image is mounted directly at the appliance's
  existing runtime state root (`/home/avado/.astrid` on AVADO and
  `/media/data/astrid/state` on ICP). This preserves all existing paths and
  operator UX. The exact required options are
  `rw,nodev,nosuid,noexec,noatime`; mutable units may write only this mount or
  their bounded private tmpfs paths.
- A separate fully allocated, root-owned 32 GiB ext4 image is mounted at the
  root snapshot directory. Candidate, builder, updater, daemon, and edge
  identities cannot write this mount. The ICP eMMC backup remains a sibling
  outside both images.
- The immutable config pins, independently for both images: canonical backing
  path, backing-filesystem device and inode, exact 34,359,738,368-byte length,
  ext4 UUID, canonical mountpoint, and expected mount device. The two UUIDs,
  inodes, loop devices, and mounted devices must differ.
- Root validation opens each backing file without following links and requires
  a root-owned single-link regular file, mode `0600` or stricter, exact size,
  and allocated blocks covering the complete length. It resolves the unique
  `/proc/self/mountinfo` record, requires ext4 plus the exact option set, binds
  mount `st_dev` to the mount record and `/dev/disk/by-uuid`, then binds the
  loop device's sysfs `backing_file` to the configured file identity. A loop
  number by itself is never a durable identity.
- Both backing files live outside either mounted tree. After allocation their
  backing filesystem must retain at least 64 GiB available. Before every
  build, activation, snapshot, periodic health sample, and rollback, runtime
  free space must remain at least 4 GiB and rollback free space must exceed
  the exact runtime bytes to copy plus 4 GiB. Failure blocks a new activation;
  during probation it triggers automatic rollback while the reserved root
  volume remains writable.
- State-preserving migration is stop-and-copy into a freshly formatted image,
  followed by an exact inventory/hash/database check before the mount replaces
  the old path. The old directory is retained read-only until reboot and
  post-migration acceptance. Power-loss reconciliation must select either the
  complete old directory or complete mounted image, never a partially copied
  mixture.
- Immutable service drop-ins additionally require `LimitCORE=0` and bounded
  journald rate limits. These controls complement the volume cap; they do not
  replace it.

Until that topology is attested, the storage status must report
`aggregate_capacity_unbounded` and autonomous build/activation authority must
remain disabled. This requirement is intentionally appliance-local: AVADO and
ICP use separate images, UUIDs, loop bindings, and keys.

## Architecture-health note for large modules

Several rescue modules intentionally exceed the repository's 1,000-line review signal. They are
kept cohesive because each is one fail-closed transaction or one independently replayed policy
gate; splitting it across mutually callable modules would duplicate authority-bearing parsing or
make crash-boundary review harder:

- `transition.rs` owns the single journaled drain, switch, rollback, and boot-reconciliation state
  machine, including all power-loss phase tests.
- `invariant.rs` owns the complete immutable release/capsule/service semantic verifier and its
  adversarial fixtures.
- `manifest.rs` owns one signed-source/candidate/dependency lineage gate; source IDs, vendor
  membership, changed-line accounting, and derived-tree verification are reviewed together.
- `unit_transaction.rs` owns the six-fragment systemd transaction, effective-unit replay, and
  every partial-install recovery case.
- `build.rs` owns the fixed offline build command graph, child-containment boundary, evidence
  aggregation, and terminal no-retry classification.
- `reflection.rs` owns the exact root-issued reflection lease/admission protocol and reboot
  reconciliation.

These files contain no ambient command dispatch and have focused module test suites. Future splits
should extract only pure, authority-free helpers; the authenticated state transition and its crash
tests should remain reviewable in one place.
