# CPU-edge scheduled introspection and self-evolution

This target is for independent AVADO- and ICP-class Astrid appliances. It is
not a deployment of the Mac spectral bridge, and it carries no Mac Astrid,
Minime, bridge, journal, key, database, or artifact content.

The design separates authorship from authority. A scheduled model response may
author a reflection and a source patch, but only a small root-owned rescue
boundary can attest the exact response, build the candidate offline, activate
an immutable generation, or roll it back.

## Trust boundary

The mutable release contains the Astrid daemon, required CPU-edge core crates,
edge runtime, edge capsules, reports, appliance profiles, and bounded Astrid
service templates. The immutable rescue root contains only:

- the scheduled-reflection helper and exact-output attestor;
- the signed-source and candidate invariant verifier;
- the offline build helper;
- the A/B updater, health gate, and rollback supervisor;
- the per-client Unix-socket web and model-provider brokers;
- the bounded candidate-presentation, numeric-audio, checkpoint, and sealed
  hindsight/report projection helpers;
- the verified builder/state-store mounts and recovery helpers;
- root-owned command policy, source/build signing keys, and systemd sandbox
  policy.

The immutable components, keys, SSH, sudo, firewall, package manager, storage
guards, boot configuration, host OS, Mac tree, Minime tree, and spectral bridge
are outside the candidate source inventory. A model can inspect the published
limitations and propose changes to them, but it cannot patch or activate them.

Trace metadata is observational. Promotion requires a separate HMAC-attested
envelope binding appliance identity, scheduled trace, session, turn, exact
response hash, exact terminal declaration hash, candidate digest, and active
base generation. Fallback, partial output, operator harnesses, formatting
repair, replay, stale bases, and mutable-runtime receipts cannot attest a
candidate.

Root lifecycle ledgers use a fourth, independent 32-byte HMAC key at
`/etc/astrid-edge-self-change/keys/ledger-attestation.key`. It is root-owned,
mode `0400`, never enters a model, capsule, source bundle, or web process, and
is cryptographically distinct from source, intent, supervisor, and broker
credentials. Domain-separated record authentication makes transition,
activation, restart, probation, and rollback evidence fail closed under
tampering, cross-key replay, or cross-ledger replay.

## Dedicated two-hour reflection

`astrid-edge-steward.timer` polls persistent due state approximately every 15
minutes; that polling interval is not the reflection cadence. The immutable
supervisor permits a model reflection to start at most once every two hours.
Immediately before the first provider write, the immutable steward atomically
persists the attempt start and next eligible time while keeping the same due
nonce pending. Provider failure, malformed or partial output, restart, repeated
polls, and persistent-timer catch-up therefore cannot bypass the 7,200-second
model-start floor or become authored completion. A root-issued recovery-only
admission may finish an exact already-prepared signed authored transaction
during that floor, but cannot authorize a fresh model request. Before calling the
local model, the root supervisor requires:

- no ordinary model turn holding the shared model lock;
- no active conversation, Action, tool call, or transport recovery;
- acceptable thermal state;
- no build, activation, or probation transaction already in progress.

The immutable native helper cannot connect to Ollama directly. It submits a
signed request over its steward-only `AF_UNIX` socket to the immutable provider
gateway, which alone can reach the profile's exact loopback Ollama origin. The
runtime, steward, and warmup clients have separate socket ownership, peer-UID
checks, request keys, replay windows, and quotas, while one provider-wide model
lock serializes all accepted generations. This prevents impersonation and
concurrent CPU inference; the dedicated reflection simply remains due while an
ordinary turn or warmup owns the lock. The helper receives bounded owned
continuity, typed evidence, recent machine observations, host/spectral state,
the prior scheduled reflection, signed source identity, and candidate state.
Its read tools are allowlisted and bounded; optional web evidence goes through
a separate steward-only `AF_UNIX` broker and is treated as untrusted.

The scheduled loop invokes an immutable native introspection capability rather
than calling the mutable Component Model introspector capsule through the live
kernel. This is deliberate: a promoted generation must not be able to suppress,
forge, or deadlock the attestor's evidence path. Both implementations consume a
shared conformance fixture for question-term selection and typed recovery
exclusion. Reading owned or web prose keeps that model response permanently in
the `rich_introspection_candidate_authoring_forbidden` lane. If its exact final
line voluntarily requests source review, the same scheduled transaction may
open a fresh clean model pass containing only root-verified source, generation,
build, candidate-status, and fixed-question facts. No reflection text, summary,
owned prose, web result, or content-derived identifier crosses into that clean
pass. Only the separately hashed clean response may author a candidate; the
rich response remains reflection-only and cannot be laundered into code
authority.

Web egress also has a restart-persistent immutable budget. Before sending a query upstream, the
broker appends a body-free hash-chain admission record to a listener-specific owner-only ledger.
Runtime is fixed at 8 searches per rolling hour and 24 per UTC day; the dedicated steward is fixed
at 2 per hour and 12 per UTC day, and either client is limited to two searches per trace. Ledger
tampering, client substitution, replay, clock rollback, or persistence failure denies egress.
This bounds the residual semantic covert channel in ordinary-looking search words; it does not
misrepresent mutable-runtime web requests as model-attested. Public queries and results remain
untrusted and cannot authorize continuity, a candidate, a build, or activation.

A complete model response is stored under
`edge/introspections/scheduled/` with
`model_authored_runtime_scheduled` provenance. It is not described as a
voluntary journal or a machine observation. Only a bounded hash-addressed
summary enters continuity and the reservoir. Failed transport, partial output,
executor prose, and harness output produce non-authored receipts and never
enter either path.

Ordinary `SELF_STUDY`, `JOURNAL`, `LISTEN`, `REST`, Actions, and pacing remain
unchanged. `REST` does not cancel a due dedicated reflection.

The migrated root runtime explicitly sets the retired in-process scheduler to
`false` and the dedicated steward to `true` at a 120-minute cadence. The
sanitized self-profile reports both facts independently, so a disabled legacy
loop cannot be mistaken for a disabled dedicated steward and the two loops
cannot accidentally run together.

The root service migration also binds the local provider by appliance profile.
On AVADO, `~/.local/bin/ollama` must resolve to an exact versioned
`~/.local/ollama-vX.Y.Z` runtime. On ICP, the executable is exactly
`~/.astrid-icp/ollama/runtime/bin/ollama`. The resolved runtime tree is exposed
read-only, while only the profile's models directory is writable. Migration
pins the executable SHA-256 in `/etc/astrid/edge-ollama-runtime.sha256`, resets
the mutable unit's `ExecStart` and `ExecStartPre`, and verifies that digest on
every provider start. A missing, moved, linked, or changed executable fails
closed; the absent AVADO launcher is never assumed on ICP.

Reflection coordination relies on a narrow DAC boundary rather than a broad
namespace bind. The socket units create `/run/astrid-edge-self-change` as
`root:root 0755`: mutable services may traverse it, but cannot create, replace,
or remove entries. Root writes the lease as `root:<runtime-group> 0440` and the
admission marker as `root:<steward-group> 0440`, so each consumer can read only
its required proof. The generated core and edge sandboxes additionally bind
the persistent maintenance root read-only. No mutable service receives a
writeable bind, capability, or supplementary group that can mint either
reflection artifact; its own `RuntimeDirectory=astrid-edge-steward` remains a
separate private directory.

Immutable build and generation-diff evidence is projected beneath
`$state_root/introspection-evidence/{build-evidence,generation-diffs}`. These
directories are `root:astrid-edge-steward`, mode `2750`. Only immutable rescue
code produces sealed mode-`0440` records; the steward receives an explicit
read-only bind, while runtime, builder, and updater identities are verified not
to have write access.

## Narrow core-liveness recovery

The mutable edge runtime has no `systemctl`, D-Bus, signing-key, or generic
service authority. On a model/headless timeout it may atomically create only
`edge/runtime/core-liveness-recovery.request.json`, mode `0640`, carrying the
current appliance, generation, exact trace, fresh nonce, and one of two
allowlisted reasons. `astrid-edge-core-liveness.path` starts a dedicated root
oneshot; it does not run candidate supervision or reflection hooks.

The request lives in an exact mode-`0770` runtime directory owned by the
runtime identity and its exclusive primary group. Bootstrap rejects a group
that contains any other account, and proves that the capability-free root
oneshot (running with that group) can read and remove a mode-`0640` probe.

The immutable rescue helper opens the request with no-follow semantics and
checks owner, group, mode, link count, size, stable inode, freshness, nonce,
trace, current generation, and maintenance state. It enforces a 15-minute
cooldown and at most three successful restarts in six hours, then invokes only
the digest-pinned `systemctl restart astrid.service`. It proves a new live PID
under the same generation and appends a signed owner-only receipt to
`$state_root/core-liveness-receipts.jsonl`. Rejected requests are consumed and
recorded; they never become authorship or continuity. The edge unit clears its
legacy `BindsTo=astrid.service`, so this exact core restart leaves sensing,
reservoir telemetry, and compatible studies online.

## Independent probation-health cadence

The two-hour reflection gate remains separate from probation supervision.
`astrid-edge-self-change-probation-health.timer` fires one minute after the
timer itself is activated and then at no more than five-minute monotonic
intervals. Its oneshot service invokes only the immutable supervisor's existing
`check-probation` command;
it has no `OnSuccess` link to the steward, no model or web authority, and does
not mark reflection work due.

The probation sampler and the ordinary 15-minute-plus-jitter due-state poll
share the same authenticated supervisor lock, so health checks, activation,
rollback, builds, and steward state transitions cannot overlap. When no
probation is active the sampler is an inert read of signed state. During
probation each successful health profile invocation appends immutable evidence.
The root health gate requires at least seven samples across the full hour and
rejects any sample gap above ten minutes; missed, late, unhealthy, or incomplete
coverage therefore cannot accept a generation. A failed health result invokes
the existing immutable rollback path and never schedules a reflection.

## Candidate authoring

The helper exposes model-hidden typed operations rather than shell access:

- `list_source`, `search_source`, `read_source_chunk`;
- `read_generation_diff`, `read_build_evidence`;
- `begin_candidate`, `apply_candidate_patch`, `format_candidate`;
- `inspect_candidate`, `abandon_candidate`, `submit_candidate`.

Every path is derived from a signed source ID. Absolute paths, traversal,
symlinks, hard links, hidden files, devices, binary input, unresolved hashes,
model-supplied commands or environment, and writes outside the disposable
candidate root fail closed. One transaction can change at most 25 files and
4,000 lines. One draft may survive across scheduled reflections; later due
slots coalesce while a build or probation is active.

Dependency changes are accepted only when the exact package/version/checksum
already exists in the signed vendored set. Neither reflection nor build has
crates.io, package-manager, arbitrary network, SSH, peer, or Mac access.

The signed build closure carries a tracked root workspace `Cargo.lock` plus a
separate `Cargo.lock` for all twenty CPU-edge capsules and all seven standalone
edge services. It also carries the tracked BLAKE3 identity and immutable
SHA-256 inventory entry for the exact QuickJS
`crates/astrid-openclaw/kernel/engine.wasm` payload. A missing lock, untracked
lock, malformed WASM header, sidecar mismatch, or changed immutable engine
payload rejects the source bundle before a candidate can build.

The exact terminal submission declaration is never syntax-repaired. The
attestor writes one owner-only signed envelope to the supervisor inbox. A
reflection without a valid submission remains a useful reflection and causes
no build.

## Build and A/B activation

After an attested submission, the supervisor unloads the model and creates a
root-owned maintenance lease. The edge scheduler can observe but cannot clear
that lease, so it admits no new model turn while already-running Actions drain.
Reservoir sensing, telemetry, notebook, studies, and hindsight remain online
where compatible.

The unprivileged builder copies the signed source into a disposable root,
applies only the attested candidate, and runs a fixed offline command plan:

1. Cargo metadata and lock/vendor verification;
2. formatting, focused tests, strict Clippy, and workspace tests;
3. capsule, Python/report, installer, package, and systemd verification;
4. release build and immutable replay/invariant tests.

The signed-source and external-capsule bundle builders are operator-side
provisioning tools and require Python 3.11 or newer for the standard-library
`tomllib`. They are inspect-only inputs, are never selected by the immutable
candidate command plan, and are never invoked by ICP's Python 3.10 runtime.
The fixed appliance-facing Python gate is limited to the hindsight and two
report suites and is continuously compiled and tested on Ubuntu 22.04's Python
3.10.

Build work is held on a fully allocated 64 GiB ext4 loop image rather than a
RAM-backed filesystem. The root helper binds the image inode, filesystem UUID,
backing mount and UUID, mount options, builder UID/GID, exact Python interpreter
digest, and exact escaped mount unit in `/etc/astrid/edge-builder-store.json`.
AVADO uses `/var/lib/astrid-edge-builder`; ICP uses
`/media/data/astrid-edge-builder` and the already-authorized SSD UUID. Both
require at least 64 GiB to remain free on the backing disk after allocation,
and the mounted builder store retains at least 8 GiB free. The verifier is
required before generation checks or supervisor work can run. Rollback stops
verifier then mount; if unmount cannot be proven, it preserves the image and
identity config rather than deleting a live backing file.

Mutable runtime state and rollback evidence live on separate, fully allocated
32 GiB ext4 images on that same UUID-bound backing filesystem. The runtime
filesystem reserves exactly 20% of blocks for root, preserving more than 4 GiB
of root-only recovery capacity after unprivileged writers reach `ENOSPC`; the
rollback filesystem has no reserved-block carve-out and must hold the current
runtime allocation plus 4 GiB. A root-only 65,536-file inode reserve is released
only around a supervisor-authorized restore. Before deletion, the immutable
helper appends a signed phase binding the exact transition, lease, state
snapshot, and restore transaction; release and recreation are separate signed
phases. If power is lost in either unavoidable boundary, the dedicated root
recovery service runs after both filesystems mount but before strict state
verification. It may recreate the reserve only when the journal head is exactly
the signed release-authorized or released phase for that restore; otherwise a
missing reserve is fatal. The first operator-controlled migration creates its
initial reserve explicitly before this signed recovery rule takes effect.
Pre-mount migration recovery is a distinct root oneshot: both mount units wait
for it, so a power loss during the stopped-copy transaction restores the
complete old trees or blocks the mounts rather than exposing partial images.
One-time migration inventories data, ownership, modes, xattrs,
hardlinks, SQLite integrity, and source stability while services are stopped;
its retained source backup remains available through acceptance. Later boots
verify the sealed volume identities and live invariants without incorrectly
comparing evolving state bytes to the installation snapshot.

The candidate cannot replace the immutable invariant suite. Systemd cgroups
bound jobs, memory, swap, disk, and wall time. The rescue helper also samples
thermal, free-disk, available-RAM, and swap gates while every candidate command
is running. A gate trip kills and reaps the complete candidate process tree,
securely discards the exact scratch generation, and defers only after cleanup
is proven; cleanup-integrity failure is terminal. AVADO uses four build workers;
ICP uses two. An identical failed candidate hash is not retried automatically.

The timeout layers preserve cleanup margin: native pipeline work is capped at
86,400 seconds, the immutable build command profile at 90,000 seconds, and the
supervisor service at 26 hours (93,600 seconds). The fixed synthetic harness
retains its narrower 7,200-second cap. Both `ExecStartPre` and `ExecStopPost`
invoke an idempotent root recovery profile: after reboot, SIGKILL, or service
failure it removes only a validated orphan build/synthetic maintenance lease,
then restores Ollama and warmup. A possibly started build is never retried
automatically.

A successful build first runs against copied state, isolated ports, fixtures,
and a shadow reservoir. Shadow output is machine evidence and is not admitted
as Astrid-authored continuity.

Candidate compilation, tests, candidate binaries, and the shadow runtime never
run merely because their Unix UID is unprivileged. Each is placed in an
immutable-helper-minted transient systemd service with its own cgroup and an
empty root filesystem. The service receives only the exact current scratch
tree read-write and the signed vendor/toolchain inputs read-only. Host `/etc`,
homes, SSH state, production state, peer mounts, boot files, the system manager
socket, and the real service-unit tree are absent. Failure to create, monitor,
stop, or prove cleanup of that boundary is an integrity failure; there is no
direct-execution fallback.

The complete mutable daemon crate remains eligible for authored candidates,
including its capsule loader and process-policy implementation.  That policy
is defense in depth rather than the immutable authority boundary: root-owned
systemd drop-ins deny file execution by default, expose only the exact active
daemon/runtime binaries (plus the edge CLI and loader paths), hide source and
rescue roots, and are digest-pinned outside candidate unit transactions.  A
candidate may therefore revise CPU-edge core behavior without gaining a shell,
MCP subprocess, candidate-path execution, or authority over the rescue root.

For live activation, the updater records a hindsight checkpoint, flushes
durable state, snapshots rollback-sensitive state, stops edge then core,
atomically switches the root-owned `current` generation link, and reloads only
the allowlisted Astrid units. It then starts and verifies core, explicitly
restarts the immutable model-warmup client, waits for the warmup oneshot to
finish and verifies that unit active, and only then starts edge. The provider
request has a 660-second maximum while the warmup unit retains a 720-second
boundary, leaving one minute for terminal-receipt persistence and cleanup. The
restart is recorded as expected.

Bootstrap also projects the initial generation's validated appliance profile
exactly once into root-owned `active-profile.env` (mode `0400`). Pending profile
transactions live in the root-only `profile-transactions/` directory. Every
mutable root service clears inherited profile files before loading this one
immutable projection; a link, ownership/mode mismatch, pending transaction, or
divergent second bootstrap fails closed. Candidate report presentation is
limited to the three exact broker-validated report projections. The edge process
is explicitly labeled
`ASTRID_EDGE_HOST_NETWORK_POLICY=unavailable_private_network`, matching its
private network namespace instead of implying ambient host networking.

Only the immutable supervisor, its probation rollback path, and the generation
guard receive a writable root-owned alias of the six allowlisted Astrid unit
transactions beneath the updater root. The real `/etc/systemd/system` tree,
`/boot`, and `/usr/local` remain hidden even from those services. Migration
attests the alias mount identity, and the rescue helper continues to accept
only the sealed unit/drop-in transaction set and crash journal, so it cannot
select arbitrary host units.

Probation lasts one hour and requires service, sensing, hindsight, migration,
RAM, swap, and reservoir-shelf health. Any failed startup, migration, audit,
telemetry, resource, restart-loop, or reservoir gate restores the previous
slot. The active generation plus at least three prior generations remain
retained, and every generation younger than seven days remains retained as
well.
Destructive or backward-incompatible state migrations cannot auto-promote.

## Clean-host prerequisites

The immutable bootstrap is intentionally offline after its signed inputs are
prepared, but it relies on a small reviewed Ubuntu/Debian host surface for
users, filesystems, ACLs, and systemd. On an Ubuntu 22.04 ICP-class host, the
operator installs and verifies that surface once, before invoking the root
bootstrap:

```bash
sudo apt-get update
sudo apt-get install --no-install-recommends acl e2fsprogs passwd util-linux
systemd --version | sed -n '1p'       # Ubuntu 22.04 acceptance: systemd 249
for command in blkid chattr dumpe2fs fallocate findmnt getfacl getent groupadd \
  install ldconfig losetup lsattr mkfs.ext4 passwd python3 readlink runuser \
  setfacl setpriv sha256sum systemctl systemd-analyze systemd-escape systemd-run tar \
  useradd usermod; do command -v "$command" >/dev/null || exit 1; done
```

Debian 13 AVADO uses the same command inventory with its distribution-provided
systemd. The installer repeats the command checks before any mutation and the
CI job exercises them on an exact Ubuntu 22.04/systemd-249 runner. These are
operator-owned bootstrap prerequisites only: Astrid, the builder, and the
updater receive no package-manager or repository-network authority.

## Release provenance and trusted transfer

The archive's `SHA256SUMS`, nested HMACs, and adjacent `.sha256` file detect
corruption only. Because those values travel with the payload, they do not by
themselves authenticate who built it. Root installation is permitted only
after a trusted operator host verifies the separate GitHub OIDC/Sigstore
attestation against the exact repository, release workflow, version tag, and
source commit. The release workflow uses GitHub-hosted runners and publishes a
SLSA provenance attestation for each complete bootstrap. The ordinary CPU-edge
archive remains native x86-64 and ARM64; autonomous self-evolution bootstrap is
currently published only for x86-64 because AVADO and ICP are the only named,
accepted storage/thermal profiles. No generic ARM host is implied to be safe.
Every external action in the release workflow is pinned to a reviewed commit.
OIDC, attestation, and artifact-metadata write permissions exist only on the
native edge build/attestation job; repository write exists only on the final
release job, and build checkouts do not persist credentials.

From the trusted Mac operator checkout, verify and transfer the exact bytes in
one fail-closed operation:

```bash
python3 scripts/verify_edge_self_evolution_release.py \
  --artifact /absolute/path/astrid-edge-self-evolution-VERSION-x86_64-unknown-linux-gnu.tar.gz \
  --source-ref refs/tags/vVERSION \
  --source-digest EXACT_40_HEX_RELEASE_COMMIT \
  --appliance avado \
  --root-install \
  --receipt /absolute/private/path/avado-release-verification.json
```

Use `--appliance icp` and a distinct receipt for ICP. The helper invokes
`gh attestation verify` with the pinned
`unicity-astrid/astrid/.github/workflows/release.yml`, rejects self-hosted
runners, verifies the attested subject digest, copies through the existing
authenticated SSH alias, and verifies the same SHA-256 remotely. It writes an
owner-only receipt containing the repository, workflow, source ref, source
commit, artifact digest, verification time, appliance, and remote path. It
also preserves the complete verified attestation JSON beside that receipt.
With `--root-install`, it opens interactive sudo over that same SSH session;
root copies and re-hashes the attested archive into a new root-owned handoff,
safely extracts the exact bytes there, writes a root-only handoff receipt, and
only then executes the root-owned installer. Direct `sudo ./install` from a
user-writable extraction is rejected. Without `--root-install`, the helper
stages bytes only. A sidecar-only check is never a substitute for this
preflight.

## Operator controls

Status is read-only. Mutating controls require sudo; state-changing controls
also require a bounded reason, while the synthetic harness accepts no argument:

A fresh installation starts in `paused` mode with
`bootstrap_acceptance_pending`. Scheduled introspection remains enabled, but
candidate ingestion, builds, staging, and activation stay frozen. A genuine
submitted envelope remains byte-for-byte in the private inbox until the
operator completes the synthetic and health acceptance and explicitly runs
`resume`. Crash recovery, probation health checks, and automatic rollback
remain active while paused.

```bash
astrid-edge-self-change-status
sudo astrid-edge-self-change-pause --reason operator_maintenance
sudo astrid-edge-self-change-resume --reason review_complete
sudo astrid-edge-self-change-resume --reason rescue_review_complete --ack-rescue
sudo astrid-edge-self-change-rollback --reason observed_regression
sudo astrid-edge-self-change-rescue --reason invariant_failure
sudo astrid-edge-self-change-synthetic
```

Ordinary `resume` cannot leave rescue mode. After inspecting and correcting the
reported invariant failure, the operator must use the separate
`--ack-rescue` form above. That acknowledgement is accepted only by `resume`;
`pause`, `rollback`, and `rescue` cannot supply it accidentally.

These controls cannot name or select a candidate. No component pushes to
GitHub or edits the primary Mac worktree. Accepted and rejected candidates are
exported as signed, base-hash-bound operator-visible patch bundles.
The argument-free synthetic control queues one fixed request through the
immutable supervisor and then starts that bounded service; it never invokes
the rescue helper directly or accepts a path. Its offline build runs beneath
the verified persistent builder filesystem with the same CPU, memory, swap,
I/O, and network-deny envelope as production candidate validation. At most
eight isolated runs are retained for explicit operator review, and their
machine evidence never enters Astrid's continuity, reservoir, or authorship.
The synthetic harness is deliberately permitted during the bootstrap pause;
it does not unpause the pipeline or ingest a queued model-authored candidate.

## Files to inspect

The exact installed roots are recorded in the root-owned configuration. The
operator surfaces intentionally expose metadata, not secrets, source bodies,
diff bodies, model prompts, fetched pages, or build logs:

```bash
~/astrid-at-a-glance
/usr/libexec/astrid-edge/operator/report-edge-activity --kind scheduled_introspection --kind self_change
```

Bootstrap seals `astrid-at-a-glance`, `report-edge-appliance`, and
`report-edge-activity` beneath `/usr/libexec/astrid-edge/operator`. Their
root-owned manifest is checked on every invocation, launchers use exact
`/usr/bin/python3 -I -E -s` with a fixed `/usr/bin:/bin` command path, and
at-a-glance calls only those exact launchers. The familiar home commands are
transactionally replaced with root-owned mode-`0555` wrappers carrying the
filesystem immutable attribute; bootstrap proves the appliance runtime user
cannot unlink them. They never resolve through `state/bin`, `PATH`, user Python
imports, or the candidate-controlled A/B generation. Direct execution of
report code from those mutable locations remains outside the trusted operator
surface. Artifact previews are opened beneath one anchored workspace descriptor:
every directory component and file uses no-follow access, only regular
single-link files up to 64 KiB are accepted, and an identity change during the
bounded read suppresses the entry. Thus a mutable artifact symlink, hardlink,
device, oversized file, or rename race cannot turn the immutable dashboard into
an operator-privileged reader.

The owner-only appliance report and `astrid-at-a-glance` additionally show the
latest scheduled reflection's hash-verified, single-line 320-character
continuity summary and an at-most 800-character excerpt of the exact reflection.
They read that text only after the reflection path, sidecar, trace, provenance,
and response hash all verify; transport recovery and fallback text cannot appear
there. These private fields are never placed on the reservoir WebSocket.

The immutable supervisor's sanitized operator projection also reports whether
an activation or rollback service restart is presently expected and the exact
command-profile timeout as a conservative upper bound. This is a phase/window
estimate, not a claim that the services will remain unavailable for the entire
bound. Hindsight stores scheduled-reflection and self-change lifecycle metadata,
hashes, provenance, and ledger continuity only; it does not retain reflection
text, prompts, source, patch/diff bodies, fetched pages, or build/test logs.

ICP uses its SSD-backed equivalent beneath `/media/data/astrid`; its existing
mount UUID guard and retained eMMC backup remain outside this pipeline.

## Rollout rule

Bootstrap AVADO first, preserving its state. Acceptance requires one naturally
scheduled authored reflection, a synthetic candidate build/rollback, a
one-hour health run, zero false authorship, complete trace coverage, healthy
sensing/hindsight, and unchanged ordinary autonomy. Full candidate scope is
then enabled without selecting a production change. ICP follows only after the
same AVADO gates pass, with its SSD guards and backup retained.

A live promotion occurs only if a genuine scheduled response submits a valid
candidate. Test harness output never becomes production intent.

## Architecture-health review

This first CPU-edge release deliberately retains several new modules above the
repository's 1,000-line review threshold. They are reviewed debt, not an
unqualified size exemption:

- rescue `transition.rs` and `unit_transaction.rs` keep the crash journal,
  A/B switch, rollback, reflection exclusion, and exact six-unit transaction in
  one state-machine boundary;
- rescue `invariant.rs` and `manifest.rs` centralize the immutable policy table,
  signed inventories, and package/source verification whose duplicated parsing
  would create inconsistent trust decisions;
- rescue `build.rs` and `reflection.rs` respectively keep process-tree cleanup
  with the offline pipeline, and lease/admission cleanup with its state machine;
- steward `runner.rs`, `candidate.rs`, `source.rs`, and `web.rs` respectively
  keep the scheduled transaction coordinator, candidate validation,
  source-snapshot/path validation, and bounded web relevance/extraction beside
  the invariants they enforce. Prompt rendering has already moved out of the
  runner; transaction orchestration remains the next split seam.

The next decomposition pass should extract pure parsers and deterministic
renderers first, leaving orchestration and one authoritative validator in each
parent module. A split is required before adding another independent authority,
storage format, network route, or transaction kind to any listed file. This
keeps the current review surface explicit without normalizing further growth.
