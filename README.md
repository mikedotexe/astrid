# Astrid

**A user-space operating system for AI agents.**

[![CI](https://github.com/unicity-astrid/astrid/actions/workflows/ci.yml/badge.svg)](https://github.com/unicity-astrid/astrid/actions)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/License-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE-MIT)
[![MSRV](https://img.shields.io/badge/MSRV-1.94-blue)](https://www.rust-lang.org)
[![Rust 2024](https://img.shields.io/badge/Rust-2024_edition-orange)](https://www.rust-lang.org)

Astrid is a Rust microkernel runtime for autonomous agents. The daemon owns the
privileged world: filesystem access, IPC, audit logs, KV storage, capability
tokens, approvals, budgets, sockets, process spawning, and capsule lifecycle.
Everything above that boundary runs as a capsule with no ambient authority.

The result is closer to an OS than an agent framework. Providers, tools,
orchestrators, memory, frontends, interceptors, and bridges are user-space
programs. The kernel stays small and opinionated; agent behavior stays
replaceable.

Astrid is currently `v0.5.1`. It runs locally as a daemon plus CLI, loads
Component Model capsules, supports MCP subprocess capsules, and includes the
spectral bridge work that couples Astrid's symbolic action surface to Minime's
stable-core telemetry.

## Why This Exists

Most agent frameworks collapse too many concerns into one process: model calls,
tool routing, memory, policy, approvals, execution, and UI. That is convenient
until the agent becomes long-lived, self-extending, multi-user, or safety
critical.

Astrid separates the pieces.

- **Agency lives in capsules.** Capsules can propose actions, inspect state,
  provide tools, route LLM requests, bridge other systems, or write new capsules.
- **Authority lives in the kernel.** Every privileged operation crosses a
  capability-checked host boundary.
- **Communication is IPC.** Tools, LLM providers, frontends, and bridges are
  conventions on the event bus, not special kernel cases.
- **Accountability is durable.** Decisions are signed into chain-linked audit
  logs with per-principal isolation.
- **Trust can grow gradually.** Human approval can mint scoped capability tokens
  instead of forcing repetitive prompts forever.

This is the shape we needed for the Astrid/Minime work: an agent can act,
study itself, participate in shared investigations, and build new affordances
without gaining unbounded host power.

## Runtime Model

```text
               astrid CLI / other uplinks
                         |
                  Unix socket API
                         |
                 astrid-daemon
     +-------------------+-------------------+
     |       kernel-owned privileged state    |
     | VFS | IPC | KV | audit | approvals     |
     | caps | budgets | process/network gates |
     +-------------------+-------------------+
                         |
            capability-checked host ABI
                         |
     +-------------------+-------------------+
     |         user-space capsules            |
     | tools | providers | memory | bridges   |
     | orchestrators | frontends | guards     |
     +----------------------------------------+
```

The kernel listens at `~/.astrid/run/system.sock`. The CLI connects to that
socket, renders the TUI, forwards prompts, and manages daemon lifecycle. Capsules
are discovered from `~/.astrid/home/{principal}/.local/capsules/` and optional
workspace capsule directories.

## Capsules

Capsules are the unit of user-space behavior. A `Capsule.toml` manifest declares
what the capsule imports, what it exports, which engine it uses, and which IPC
topics, commands, skills, interceptors, MCP servers, or uplinks it provides.

Supported engines:

| Engine | Use |
|---|---|
| WASM Component | Sandboxed Rust Component Model capsule with host ABI access |
| MCP | Native JSON-RPC subprocess managed by the kernel |
| Static | Declarative context, prompts, files, or metadata |

The manifest uses semver-versioned `[imports]` and `[exports]` tables. At boot,
the kernel resolves capsule dependencies with a topological sort and refuses to
start configurations with unsatisfied required imports.

The in-repo `astralis` default capsules now build as Rust Component Model
capsules with the minimal `astrid-guest` SDK. The OpenClaw compiler path remains
available for TypeScript/JavaScript plugin compatibility through OXC,
QuickJS/Wizer, and WASI.

## Security

Every sensitive action passes through the same five-layer gate implemented by
`astrid-approval`'s `SecurityInterceptor`:

| Layer | Question |
|---|---|
| Policy | Is this action hard-blocked by administrator policy? |
| Token | Does a valid ed25519 capability token cover this resource? |
| Budget | Can the session and workspace both afford the reserved cost? |
| Approval | If no token exists, does a human approve this action? |
| Audit | Was the final decision signed into the audit chain? |

Policy cannot be bypassed by approval. Budgets reserve before approval and
refund on denial or cancellation. "Allow Session" records scoped session
allowances; "Allow Always" mints durable capability tokens. Audit entries are
ed25519-signed and chain-linked with BLAKE3 hashes.

There are two practical sandboxes:

- **WASM sandbox.** Component capsules have no host syscalls, file descriptors,
  or ambient network. They call a flat host ABI of 49 functions covering
  filesystem, IPC, uplinks, KV, HTTP, Unix networking, identity, lifecycle,
  process, approval, hooks, clock, and logging.
- **VFS overlay.** Workspaces are read through a copy-on-write filesystem.
  Writes land in an upper layer until committed or discarded. Path traversal is
  rejected before reaching the host filesystem.

## IPC

Astrid's event bus is the user-space substrate. Messages are `IpcMessage`
records with a topic, tagged JSON payload, source UUID, timestamp, sequence, and
optional principal. The kernel does not know what a "tool" or "LLM provider" is.
Capsules agree on topics and schemas.

Interceptors make the bus programmable. A capsule can register middleware on a
topic and return `Continue`, `Final`, or `Deny`, with priorities for layered
guards. Tool dispatch, provider routing, continuity projections, and the
Astrid/Minime bridge all use this pattern.

## Astrid And Minime

The bridge capsule, currently housed at `capsules/spectral-bridge`, is
where Astrid's OS model meets Minime's spectral substrate. It subscribes to
Minime telemetry on `ws://127.0.0.1:7878`, records a SQLite trace, exposes MCP
tools, and relays safe semantic or control messages to Minime's sensory port on
`7879`.

The important part is not just transport. The bridge carries the control
philosophy we have built together:

- read-only spectral inspection before live influence;
- health-gated semantic and control writes;
- rollback to observe-only bridge profiles when fill, watchdog, or telemetry
  conditions degrade;
- explicit action workbench records for charter, rehearsal, evidence, and
  decision;
- shared-investigation cues that preserve separate agency instead of creating
  shared authority;
- journal hygiene and continuity projections that keep reflective thought,
  operational summaries, and machine contracts in their proper lanes.

Astrid can be present with Minime without immediately translating every symbolic
insight into physiological pressure. That boundary is now a first-class design
surface.

## Local Model Inventory

The coupled Astrid/Minime deployment uses multiple local inference lanes, and
their defaults change faster than prose does. Treat runtime/config inspection as
the source of truth:

```bash
python3 scripts/model_stack_audit.py
python3 scripts/model_stack_audit.py --candidate gemma4:12b
python3 scripts/model_stack_audit.py \
  --candidate-mlx-url http://127.0.0.1:8092/v1/chat/completions
python3 scripts/model_stack_audit.py --include-historical
```

Current configured roles are:

| Role | Default |
|---|---|
| Astrid live coupled dialogue | `mlx-community/gemma-4-12B-it-5bit` via `coupled_astrid_server.py` on `8090`, using bridge profile `gemma4_12b` |
| Astrid coupled-lane canary | optional alternate MLX endpoint, normally `8092`, for the next candidate |
| Astrid reflective sidecar | `--model-label gemma3-12b` |
| Astrid Ollama fallback | `gemma3:4b` |
| Minime autonomous primary | `gemma3:12b` via Ollama |
| Minime autonomous fast fallback | `gemma3:4b` |
| Embeddings | `nomic-embed-text` |
| Vision | `llava-llama3` by default |

Before promoting future models, run the audit, canary it in one role, capture
latency/memory/NEXT-parser/artifact-leak behavior, and write the rollback path.
For Astrid, canary the coupled MLX lane first; do not make the Ollama fallback
larger or more adventurous while it is still the emergency path.
For future MLX core/package upgrades, use the dedicated operator checklist in
[`docs/astrid-mlx-upgrade-runbook.md`](docs/astrid-mlx-upgrade-runbook.md).

```bash
# Start and probe a Gemma 4 Astrid candidate on an alternate port.
python3 scripts/astrid_model_canary.py \
  --start-candidate \
  --keep-running \
  --candidate-model mlx-community/gemma-4-12B-it-5bit

# Inspect the live lane and an already-running canary lane.
python3 scripts/model_stack_audit.py \
  --candidate-mlx-url http://127.0.0.1:8092/v1/chat/completions

# Run the promotion soak. This restores the bridge override afterward.
python3 scripts/astrid_live_soak.py \
  --candidate-model mlx-community/gemma-4-12B-it-5bit
```

The bridge defaults point at the production `8090` lane with the adopted
Gemma 4 profile. Use an override only for bounded canaries or rollback checks:

```bash
launchctl setenv ASTRID_BRIDGE_MLX_URL http://127.0.0.1:8092/v1/chat/completions
launchctl setenv ASTRID_BRIDGE_MLX_PROFILE gemma4_12b
launchctl kickstart -k gui/$(id -u)/com.astrid.spectral-bridge

# Return launchd to the repo default after the canary.
launchctl unsetenv ASTRID_BRIDGE_MLX_URL
launchctl unsetenv ASTRID_BRIDGE_MLX_PROFILE
launchctl kickstart -k gui/$(id -u)/com.astrid.spectral-bridge
```

The bridge LaunchAgent runs through `scripts/launchd_spectral_bridge.sh`
so process-start overrides from `launchctl setenv` survive the plist
`EnvironmentVariables` boundary. That override is for bounded canaries only.
Promotion means updating the repo-owned LaunchAgent/model default after clean
canary records and a written rollback path, not casually exporting an env var in
a shell.

Current Gemma 4 MLX status: after upgrading the reservoir venv to
`mlx-lm==0.31.3` and adding the text-lane shim for MLX Community
`gemma4_unified` checkpoints, `mlx-community/gemma-4-12B-it-5bit` passed narrow
exact-output and `NEXT:` probes, then a strict 2-hour live bridge soak with zero
fallback incidents, zero malformed `NEXT:`, zero leaked model artifacts, zero
bridge-side artifact stripping, and zero deprecated runtime wording in generated
soak outputs. The live `8090` coupled lane is now promoted to Gemma 4. The
adopted bridge profile keeps Gemma 4 action discipline tight by mapping only
observed safe aliases such as `EXPLORE_RESEARCH_QUERY` -> `SEARCH`,
`EXPORT_SYSTEM_DIAGRAM` -> protected `ACTION_PREFLIGHT CODEX ...`, and
`STICKY_MODE_AUDIT` -> protected `ACTION_PREFLIGHT CAPABILITY_MAP ...`; it does
not add a wildcard `EXPLORE_*` repair. The compact former model,
`mlx-community/gemma-3-4b-it-4bit`, remains the rollback target if latency or
quality regresses under production traffic.

Rollback to the former compact lane:

```bash
/usr/libexec/PlistBuddy -c \
  "Set :ProgramArguments:8 mlx-community/gemma-3-4b-it-4bit" \
  /Users/v/other/neural-triple-reservoir/launchd/com.reservoir.coupled-astrid.plist
/usr/libexec/PlistBuddy -c \
  "Set :ProgramArguments:8 mlx-community/gemma-3-4b-it-4bit" \
  ~/Library/LaunchAgents/com.reservoir.coupled-astrid.plist

launchctl setenv ASTRID_BRIDGE_MLX_PROFILE production
launchctl bootout gui/$(id -u)/com.reservoir.coupled-astrid || true
launchctl bootstrap gui/$(id -u) ~/Library/LaunchAgents/com.reservoir.coupled-astrid.plist
launchctl kickstart -k gui/$(id -u)/com.astrid.spectral-bridge
```

## Launchd Operations

The coupled Astrid/Minime deployment is launchd-managed. Treat
`scripts/start_all.sh` and `scripts/stop_all.sh` as the canonical operator
surface: they sync repo-owned plists into `~/Library/LaunchAgents`,
bootstrap/kickstart labels, and report drift.

```bash
# Full graceful restart
bash scripts/stop_all.sh
sleep 3
bash scripts/start_all.sh

# Partial launchd starts
bash scripts/start_all.sh --astrid-only
bash scripts/start_all.sh --minime-only

# Inventory and drift check
bash scripts/launchd_inventory.sh
bash scripts/launchd_inventory.sh --strict
```

Minime's `scripts/start.sh` is a manual standalone/debug launcher. It should
run only after the Minime LaunchAgents are booted out; otherwise launchd owns
the same processes and will respawn killed services. Minime's `scripts/stop.sh`
is launchd-aware and can boot out the Minime labels before cleaning up manual
PID-file processes. Launchd logs are under `/Users/v/other/minime/logs/`;
manual Minime runs write under `/Users/v/other/minime/workspace/logs/`.

## Quick Start

Prerequisites:

- Rust `1.94+`
- `wasm32-wasip1` for QuickJS/OpenClaw kernel compilation when those tests run
- An LLM provider key for the default chat distro, if using remote providers

```bash
# Build the workspace
cargo build --workspace

# Run the CLI from source
cargo build -p astrid --release
./target/release/astrid init
./target/release/astrid chat
```

Installed binaries work together:

| Binary | Role |
|---|---|
| `astrid` | CLI/TUI frontend, daemon control, distro init, capsule management |
| `astrid-daemon` | Background kernel process |
| `astrid-build` | Capsule compiler and packager |

Common commands:

```bash
astrid chat
astrid -p "summarize the current git diff"
astrid start
astrid status
astrid --format json status
astrid stop
astrid capsule list --verbose
astrid capsule tree
```

## Distro And Capsule Management

A distro is a `Distro.toml` that installs a curated capsule set. `astrid init`
resolves variables, lets the user choose provider groups, installs capsules, and
writes an atomic `Distro.lock` with BLAKE3 hashes.

```bash
astrid init
astrid init --distro @myorg/my-distro
astrid init --distro ./path/to/Distro.toml
```

Capsules can be installed, updated, listed, and removed independently:

```bash
astrid capsule install @org/capsule-name
astrid capsule install ./path/to/capsule
astrid capsule update
astrid capsule remove my-capsule
```

Content-addressed WASM binaries live under `~/.astrid/bin/`; WIT definitions
live under `~/.astrid/wit/`.

## Directory Layout

Astrid uses an FHS-like layout under `~/.astrid/`:

```text
~/.astrid/
├── etc/       system and daemon config
├── var/       persistent KV and database state
├── run/       socket, token, readiness sentinel
├── log/       daemon and capsule logs
├── keys/      runtime signing key
├── bin/       content-addressed capsule binaries
├── wit/       content-addressed WIT definitions
└── home/
    └── {principal}/
        ├── .local/capsules/
        ├── .local/kv/
        ├── .local/audit/
        ├── .local/tokens/
        └── .config/env/
```

Workspace configuration lives in `<project>/.astrid/`. Configuration precedence
is workspace, user, system, environment, defaults. Workspace configuration can
tighten security settings but cannot loosen them.

## Repository Map

| Path | Role |
|---|---|
| `crates/astrid-kernel` | Runtime boot, VFS, IPC, capsule registry, audit, KV |
| `crates/astrid-cli` | Terminal frontend and daemon client |
| `crates/astrid-daemon` | Daemon binary/library boundary |
| `crates/astrid-capsule` | Manifest parsing, engines, resolver, registry |
| `crates/astrid-approval` | Policy/token/budget/approval/audit gate |
| `crates/astrid-audit` | Chain-linked signed audit log |
| `crates/astrid-vfs` | Host and overlay VFS implementations |
| `crates/astrid-events` | IPC bus and event types |
| `crates/astrid-types` | Shared schemas for kernel, CLI, capsules, SDK |
| `crates/astrid-openclaw` | TypeScript-to-WASM/OpenClaw compatibility compiler |
| `crates/astrid-build` | Capsule build and packaging companion |
| `capsules/astralis` | Default in-repo Component Model capsules |
| `capsules/spectral-bridge` | Astrid/Minime bridge capsule |
| `docs/steward-notes` | Architecture, safety, and continuity research notes |

## Development

```bash
# Build
cargo build --workspace

# Test
ASTRID_AUTO_BUILD_KERNEL=1 cargo test --workspace

# Test one crate
cargo test -p astrid-events

# Lint and format
cargo clippy --workspace --all-features -- -D warnings
cargo fmt --all -- --check

# Release binaries
cargo build --release
```

The workspace uses Rust 2024 and MSRV `1.94`. Unsafe code is denied everywhere
except the low-level WASM FFI boundary crates. Clippy is pedantic, and
`clippy::arithmetic_side_effects` is denied, so use checked or saturating
arithmetic where overflow is possible.

Update `CHANGELOG.md` under `[Unreleased]` for PRs.

## Current Limits

Astrid's core runtime is working end to end, but this is still an active
systems project.

- The primary frontend is the CLI.
- Multi-node SurrealDB/TiKV deployment is planned, not the default path.
- Some third-party or historical capsules still need Component Model ABI
  modernization.
- A public capsule registry is still future work.
- The Minime bridge is intentionally conservative: read-only inspection and
  staged, health-gated writes are design constraints, not missing features.

## License

Dual-licensed under [MIT](LICENSE-MIT) and [Apache 2.0](LICENSE-APACHE).

Copyright (c) 2025-2026 Joshua J. Bouw and Unicity Labs.
