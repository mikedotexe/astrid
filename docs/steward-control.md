# Steward Control Plane

`scripts/steward_control.py` coordinates cooperative external sessions without
receiving git, deployment, approval, or live-control authority. It uses only the
Python standard library and supports Python 3.12 or newer.

## Safety Model

- New installations start paused.
- One opaque 256-bit token owns one live lease. Only its SHA-256 digest is
  persisted.
- The preferred session adapter retains that token inside one controller
  process. The token never enters argv, environment variables, NDJSON requests,
  stdout, or stderr.
- A pause atomically blocks new leases and causes the next heartbeat to return
  `stop_requested=true`.
- A live lease is never overwritten or force-killed. Expired or dead-process
  leases are reaped through reconciliation.
- Git inspection is read-only. Staging, branch, HEAD, or remote changes during
  a run become policy-violation evidence.
- Events carry `ArtifactAuthorityStateV1(evidence_only)`. They cannot approve,
  deploy, edit source, or make live work runnable.
- If the canonical evidence store is unavailable, pause still succeeds and its
  bounded event is written to an owner-only spool.

## Configuration

Copy `scripts/steward_control.example.toml` to an installation-owned location
and adjust relative paths. Resolution order is:

1. Explicit CLI path options.
2. `ASTRID_STEWARD_*` environment values.
3. `ASTRID_STEWARD_CONFIG` or `--config` TOML.
4. Script-relative defaults.

The example contains no machine-specific paths, credentials, provider names,
or executable command strings.

## Session Lifecycle

Start a credential-confined cooperative session:

```bash
python3 scripts/steward_control.py --json session --actor interactive-agent
```

The command speaks newline-delimited JSON over stdin/stdout. Its first `ready`
record includes the run ID, pre-projection generation ID, heartbeat interval,
and `lease_token_included=false`. The controller owns the lease credential and
renews it automatically while idle. Clients never receive or submit the token.

Send explicit operations as one JSON object per line:

```json
{"op":"status","request_id":"status-1"}
{"op":"heartbeat","request_id":"heartbeat-1"}
{"op":"project","request_id":"project-1","full_rebuild":false}
{"op":"finish","request_id":"finish-1","outcome":"success"}
```

Requests are bounded to 64 KiB. `status` is session-local and renews the lease;
`project` runs a manually requested source-first generation under the same
lease. `finish` is terminal and preserves the controller's normal post-run
projection and git-policy checks. The session cancels and releases its lease on
pause, clean EOF, timeout, `SIGINT`, `SIGTERM`, or `SIGHUP`. A hard kill remains
recoverable through TTL expiry and `reconcile`.

`run --actor NAME --max-secs N -- ARGV...` remains the shell-free adapter for
one child process. It also retains the token inside the controller process.

### Compatibility Credential Transport

`begin`, `heartbeat`, `project`, and `finish` remain available for older
clients. They are a compatibility path, not the preferred interactive
lifecycle. A client that already holds a token in private process memory should
send it as one line on stdin:

```bash
python3 scripts/steward_control.py --json heartbeat \
  --run-id RUN_ID \
  --lease-token-stdin
```

The legacy `--lease-token TOKEN` form is retained for compatibility but emits a
warning because process arguments can be observed by process listings and
execution transcripts. `begin` still returns a token for those clients, so it
must not be used on a surface that records command output. New interactive
clients should use `session` instead.

The session protocol cannot execute commands, edit files, stage git changes,
deploy services, grant approvals, or expand live-control authority. It only
confines lifecycle authentication and exposes the controller operations already
available to the lease owner.

## Pause And Recovery

`status` and `verify` are read-only and remain available while paused.

```bash
python3 scripts/steward_control.py --json pause \
  --actor interactive-agent \
  --reason "maintenance window" \
  --wait-secs 30

python3 scripts/steward_control.py --json reconcile

python3 scripts/steward_control.py --json resume \
  --actor interactive-agent \
  --ack "evidence and pending receipts verified"
```

Resume requires a valid active V2 store, immutable V1 source hashes, and a
reconciled local event spool.

## Projection Generations

The built-in `source-first` profile runs:

1. Evidence-store and V1-immutability verification.
2. Introspection addressing inventory.
3. Sandbox queue generation and report.
4. Corridor core, leases, queue, programs, and portfolio.
5. Causal Signal Spine.
6. Living claim families.
7. Experiment dossiers.
8. Final evidence verification, authority scan, and counter audit.

Commands are explicit argument arrays with timeouts and JSON validation.
Required descendants stop after a failed dependency. The last successful
generation manifest is replaced only after every step passes. Projector
checkpoints declare their exact input streams and external source hashes, so
unrelated stream activity does not make them stale.

Inspect the plan without acquiring a lease:

```bash
python3 scripts/steward_control.py --json project --dry-run
```

Scheduler examples live in `scripts/scheduler_examples/`. Scheduling and agent
selection remain outside the controller.

## Compatibility

The old loop and hook commands are warning-only for one architecture cycle.
`scripts/steward_mutex.py` keeps the deployment preflight import path but only
exposes read-only status and activity evidence; its former lock-mutation
commands are inert.
