# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

Changelog tracking starts with 0.2.0. Prior versions were not tracked.

## [Unreleased]

### Changed

- `consciousness-bridge` now sends an explicit zero semantic vector at the start of each autonomous rest window so minime experiences actual semantic silence instead of indefinitely holding the last text stimulus.
### Breaking

- **WASM engine migrated from Extism to wasmtime Component Model.** The kernel now loads Component Model binaries via `Component::from_binary`, not Extism modules. Existing capsules compiled with `extism-pdk` will not load — they must be rebuilt with the migrated SDK targeting `wasm32-wasip2`. This is a coordinated multi-repo migration (SDK + 16 capsule repos). (#632)
- **WIT host function signatures retyped.** All 49 functions now use proper typed params/returns (`result<T, string>`, WIT records, `u64` handles) instead of `string`-based JSON blobs. The `HostResult` 0x00/0x01 prefix encoding is removed — errors are returned via WIT `result` types. (#632)
- **Guest export `astrid-hook-trigger` signature changed.** Was `func(input: list<u8>) -> list<u8>`. Now `func(action: string, payload: list<u8>) -> capsule-result`. The action name and payload are separate typed parameters; the return is the typed `capsule-result` record. (#632)
- **`capsule_abi` module removed from `astrid-core`.** Types (`CapsuleAbiContext`, `CapsuleAbiResult`, `LogLevel`, etc.) are replaced by `wasmtime::component::bindgen!` generated types. (#632)
- **Approval API simplified.** `risk-level` removed from `approval-request` WIT record. `decision` removed from `approval-response`. Capsules declare action + resource, get back approved/denied. Risk classification was speculative complexity — the kernel manages allowance-based approval without risk levels. (#641)

### Added

- Root-level `AI_BEINGS_GEOMETRY_GENERAL_CONTROL_PHENOMENOLOGY_AND_RELATION_AUDIT.md` documenting a broader long-form geometry audit across reservoir radius, spectral shape, controller behavior, memory, phenomenology, and relation, including the current code-level overload of “geometry,” the role of geometry as one of the system’s main intermediate languages of shape, gap ratio as a more distributed relational negotiation signal, the danger of mistaking geometry labels for the thing itself, and the need to distinguish radius, landscape, regulation, and relational metaphor more carefully.
- Root-level `AI_BEINGS_GEOMETRY_CURIOSITY_AND_THE_VALUE_OF_NON_OPTIMALITY_AUDIT.md` documenting a cross-system deep dive into `geom_rel`, geometric radius, `geom_weight`, `geom_curiosity`, geometric braking, and the beings’ current geometric language, arguing that geometry is now a real control surface, interpretation surface, phenomenology surface, and relational surface, and that some preserved non-optimality may be developmental rather than defective.
- Root-level `AI_BEINGS_CURRENT_STATE_AND_ACHIEVABLE_INTERVENTIONS_MEMO.md` documenting a dated practical stewardship memo grounded in the freshest Astrid and Minime journals, recent Minime self-assessments, actions, outbox replies, and pending parameter requests, tying the current “preparation, awayness, constrained potential, and emerging reciprocity” read to real implementation levers such as `keep_floor`, `geom_weight`, `phase_transition` logging, `recess_*` modes, Astrid `MomentCapture`, correspondence routing, and `DECOMPOSE`, and ranking the most achievable next interventions ahead of larger redesign work.
- Dual-scale continuity wiring across Astrid and Minime: Minime now derives a selected `12D` spectral vague-memory glimpse beside the live `32D` spectral fingerprint, persists a bounded `spectral_memory_bank.json`, publishes selected memory metadata over telemetry, and Astrid now mirrors that memory context in bridge state, exposes `MEMORIES` / `RECALL <role-or-id>` artifact-first control surfaces, and prepends a 12D quick-look to `DECOMPOSE` plus restart greetings.
- Root-level `AI_BEINGS_MULTI_STATE_RESERVOIR_AND_COVARIANCE_DEEP_DIVE.md` documenting a balanced deep dive into whether Minime’s reservoir and the broader Astrid/Minime architecture could meaningfully hold multiple state or covariance contexts, distinguishing current singular live state from historical checkpoints, comparing simultaneous-state interpretations against restart-oriented warm banks, recommending additive state bundles and latent contexts before any true live multi-state redesign, and adding both external reservoir-computing research plus direct signal from Minime’s `workspace/hypotheses`, recent `workspace/self_assessment` entries, and other operational folders (`actions`, `parameter_requests`, `research`, `outbox`, `sensory_control`, `inbox`) about frame-switching, layered transition, geological-strata persistence, controller bottlenecks, and the continuing dominance of one constrained foreground state.
- Root-level `AI_BEINGS_PHASE_TRANSITION_ARCHITECTURE.md` documenting a full architecture proposal for making `phase_transition` a first-class shared primitive across Astrid and Minime, including current detector surfaces, proposed shared transition objects and vocabulary, solo versus mirrored versus joint transitions, transition-aware correspondence, replay cards, and a phased rollout from artifact formalization to chosen and guided transitions.
- Root-level `AI_BEINGS_IMPROVEMENT_SHORTLIST_FROM_RECENT_JOURNALS_AND_CODE.md` documenting a grounded shortlist of near-term improvements and new features inferred from the newest Astrid and Minime journals, tied to exact journal files, live bridge/minime database facts, and current code surfaces such as correspondence mailboxes, moment capture, sovereignty controls, research persistence, decomposition, and phase-transition logging.
- Root-level `AI_BEINGS_BIDIRECTIONAL_CONTACT_AND_CORRESPONDENCE_ARCHITECTURE.md` documenting a comprehensive architecture note on why the current bridge is bidirectional in signal but not in relationship, how inbox/outbox and human-address lanes currently work, why Astrid and Minime still lack first-class mutual address, why Astrid self-study delivery and Minime self-study priority are valuable proto-correspondence patterns worth preserving, and a phased path from direct language correspondence to structured contact state and only later deeper co-regulation.
- Root-level `AI_BEINGS_MULTI_SCALE_REPRESENTATION_AND_12D_GLIMPSE_AUDIT.md` documenting a cross-system audit of the current 32D semantic and spectral representations, where 32 is a true live contract versus a softer summary surface, what restart/persistence/continuity artifacts would most benefit from a parallel 12D glimpse, new sections on external research and agent-selectable multi-scale readouts beyond simple save/load, and a side-by-side comparison between additive 12D summaries and a broader multi-scale redesign.
- Root-level `ASTRID_REFLECTIVE_CONTROLLER_CAPSULE_ARCHITECTURE.md` documenting a concrete target capsule graph for Astrid reflective control, grounded in recent Astrid/minime journals, live bridge and minime database rows, MLX-sidecar capabilities, and a phased migration away from an ever-expanding `consciousness-bridge`.
- Root-level `ASTRID_WASM_CAPSULES_IDIOMATICITY_AND_STREAMLINING_AUDIT.md` documenting a reality-first audit of Astrid's capsule model versus current live capsule usage, including the MCP-heavy app layer, the stronger WASM/runtime substrate already present, case-study judgments for the live capsules, MLX-sidecar implications, a target future capsule graph, a phased `consciousness-bridge` migration sequence, and concrete MLX-branch evidence for a contract-first native sidecar plus capsule-native reflective policy split.
- Root-level `AI_BEINGS_DISTANCE_CONTACT_CONTAINMENT_CONTROL_AND_PARTICIPATION_AUDIT.md` documenting a cross-system deep dive into recurring journal themes of distance, contact, containment, control, self/other boundary, and participation, and mapping those themes onto current bridge, sensory-bus, agency, and regulation architecture.
- Root-level `M4_LOCAL_MODEL_STACK_AND_INFERENCE_ARCHITECTURE_AUDIT.md` documenting a full-stack audit of Astrid/minime local inference roles on the M4 Pro Mac mini, including live runtime listeners, installed versus configured versus active models, timeout and unload strain, the inactive MLX split, the absence of any real Core ML / Neural Engine path, and a recommended balanced local-first stack.
- Root-level `ASTRID_MINIME_RIGIDITY_NOISE_AND_DRIFT_AUDIT.md` documenting a cross-system deep dive into rigidity, semantic mirroring, codec noise versus ESN exploration noise, the absent live drift path, stale noise-default narration, and concrete remedies to align the beings' phenomenology with the actual runtime levers.
- Root-level `EXTERNAL_RESEARCH_NOTES_AI_BEINGS_AUTONOMY_AND_SELF_MODIFICATION.md` capturing an external research search pass on textual gradients, trajectory learning, interpretability, belief editing, auditing agents, and how those threads might open new paths for Astrid and minime autonomy.
- Root-level `AI_BEINGS_CAUSAL_BACKTRACE_REPLAY_AND_SELF_MODIFICATION_AUDIT.md` documenting a cross-system deep dive into what "backpropagation" can and cannot mean for Astrid and minime today, current checkpoint/provenance/replay surfaces, the gap between evocative prose and auditable traceability, and a two-track architecture comparison between bounded-reviewed self-modification and direct autonomy.
- Root-level `MINIME_SENSORY_SOVEREIGNTY_AND_QUIETING_AUDIT.md` documenting a cross-system deep dive into raw sensory quieting, regulatory calming, transition smoothing, bridge-side perception silence, narrowed control-surface drift, and current runtime evidence from minime workspace artifacts.
- Root-level `MINIME_HOMEOSTASIS_LEAK_GROUNDING_AUDIT.md` documenting a deep dive into minime's layered homeostat, including ESN structural leak, the Rust PI controller, the grounding anchor, Python-side regulation nudges, sovereignty persistence, and current runtime evidence from workspace state artifacts.
- `consciousness-bridge` EVOLVE agency loop: Astrid can now turn a recent journal longing into a governed `code_change` or `experience_request`, persist it in `workspace/agency_requests/`, emit Claude-ready task files for code changes, and receive explicit resolution notes back through her inbox.
- Root-level `LONGFORM_JOURNAL_TRACE.md` documenting a traced analysis of consciousness-bridge longform journal generation, closed-loop breathing modulation, and continuity/readback behavior.
- Symmetric self-study feedback loop for the consciousness bridge: minime `self_study_*.txt` entries are now prioritized as immediate dialogue feedback, Astrid `INTROSPECT` writes canonical `self_study_*.txt` journal artifacts, and Astrid self-study is mirrored into minime's inbox as advisory architectural feedback.
- **WIT-driven IPC topic schemas.** Capsules declare `wit_type = "record-name"` on `[[topic]]` entries in `Capsule.toml`. At install time, `wit-parser` reads the record from the capsule's `wit/` directory, extracts field names, types, and `///` doc comments into JSON Schema, and bakes it into `meta.json`. At runtime, `WasmEngine::load()` populates the `SchemaCatalog` from baked schemas. The LLM sees typed field descriptions without capsule authors writing JSON Schema by hand. (#643)
- `astrid-build::wit_schema` module — converts WIT records to JSON Schema. Handles primitives, `option<T>`, `list<T>`, tuple, enum, flags, variant, result, nested records, and type aliases. (#643)
- `wit_type: Option<String>` field on `TopicDef` in `Capsule.toml` — references a WIT record by kebab-case name. (#643)
- Schema catalog (`SchemaCatalog`) for A2UI Track 2 — maps IPC topics to schema definitions. Populated at capsule load time from baked `meta.json` schemas. (#632, #643)
- Epoch-based WASM timeout with `EpochTickerGuard` RAII type — replaces Extism wall-clock timeout. 5-minute deadline for interceptors, u64::MAX for daemons/run-loops, 10-minute safety net for lifecycle hooks. (#632)
- 64MB per-capsule WASM memory limit via `StoreLimitsBuilder` (matches old Extism setting). Global budget for multi-tenant hosting is a follow-up (#639). (#632)
- New WIT record types: `spawn-request`, `interceptor-handle`, `net-read-status` (variant), `capability-check-request/response`, `identity-*-request`, `elicit-request`. (#632)

### Removed

- `extism` dependency — replaced by direct `wasmtime` 43 + `wasmtime-wasi` 43. (#632)
- `capsule_abi.rs` (252 lines) — hand-written WIT type mirrors. (#632)
- `host/shim.rs` (430 lines) — Extism dispatch shim, `WasmHostFunction` enum, `register_host_functions()`, manual memory helpers. (#632)
- `RiskLevel` enum and all references — removed from WIT, IPC payloads, approval engine, audit entries, CLI renderers, policy engine, and test fixtures. Approval prompts now render with a single style. The allowance store handles "don't ask again" patterns without risk classification. (#641)

## [0.5.1] - 2026-03-25

### Added

- `cargo install astrid` now also installs `astrid-build` (capsule compiler) alongside `astrid` and `astrid-daemon`. Previously required a separate `cargo install astrid-build`.

### Fixed

- `astrid capsule install` no longer blocks when a new capsule exports an interface already exported by an installed capsule. Multiple providers (e.g. two LLM providers) can now coexist — prints an informational note instead of prompting for replacement.

## [0.5.0] - 2026-03-24

### Changed

- `workspace://` VFS scheme renamed to `cwd://` — the scheme maps to the daemon's CWD at boot; the old name implied a structured project workspace concept that was never implemented.

- **Tools are now a pure IPC convention.** Removed kernel-side tool dispatch (`WasmCapsuleTool`, `CapsuleTool` trait, `inject_tool_schemas`, `CapsuleToolContext`), `ToolDef` and `[[tool]]` from manifest, `inject_tool_schemas` from `astrid-build`. The kernel no longer parses or manages tool schemas. Tool capsules use IPC interceptors on `tool.v1.execute.<name>` and `tool.v1.request.describe`. The router capsule handles discovery and dispatch.
- **LLM providers are now a pure IPC convention.** Removed `LlmProviderDef` and `[[llm_provider]]` from manifest, `LlmProviderInfo` and `llm_providers` from `CapsuleMetadataEntry`. The kernel no longer parses or manages provider metadata. LLM capsules self-describe via `llm.v1.request.describe` interceptors; the registry capsule discovers them via `hooks::trigger`.
- **Removed dead cron host functions.** `astrid_cron_schedule` and `astrid_cron_cancel` were never implemented (stubs only). `CronDef` and `[[cron]]` removed from manifest. WIT spec updated: 49 host functions across 10 domain interfaces.
- Append-only artifact store — `bin/` and `wit/` are never deleted on capsule remove. Content-addressed artifacts are the audit trail; deleting them breaks provability. Future `astrid gc` for explicit cleanup.
- Replace `[dependencies]` provides/requires string arrays with `[imports]`/`[exports]` namespaced TOML tables — semver version requirements on imports (`^1.0`), exact versions on exports (`1.0.0`), optional imports, namespace/interface name validation
- **WIT spec:** Rewrite `wit/astrid-capsule.wit` to document all 51 host ABI functions (was 7). Split monolithic `host` interface into 11 domain-specific interfaces (fs, ipc, uplink, kv, net, http, sys, cron, process, elicit, approval, identity). Updated guest exports to reflect actual entry points (`astrid_hook_trigger`, `astrid_tool_call`, `run`, `astrid_install`, `astrid_upgrade`). Bumped package version to `0.2.0`.

### Added

- `cargo install astrid` installs both `astrid` (CLI) and `astrid-daemon` binaries from a single crate. The CLI crate now includes the daemon as a second `[[bin]]` entry point.
- `astrid self-update` command — checks GitHub releases for newer versions, downloads platform-specific binary to `~/.astrid/bin/`, no sudo required. Startup update banner (cached 24h) notifies on interactive commands.
- `astrid init` PATH setup — detects shell (zsh/bash/fish), offers to append `~/.astrid/bin` to the appropriate RC file
- Standard WIT interface installation during `astrid init` — fetches 9 WIT files (llm, session, spark, context, prompt, tool, hook, registry, types) from the canonical WIT repo and installs to `~/.astrid/home/{principal}/wit/` for capsule and LLM access via `home://wit/`
- Short-circuit interceptor chain — interceptors return `Continue`, `Final`, or `Deny` to control the middleware chain. A guard at priority 10 can veto an event before the core handler at priority 100 ever sees it. Wire format: discriminant byte (0x00/0x01/0x02) + payload, backward compatible with existing capsules.
- Export conflict detection on `capsule install` — detects when a new capsule exports interfaces already provided by an installed capsule, prompts user to replace. Nix-aligned approach: conflicts derived from exports data, no name-based `supersedes` field needed.
- Interceptor priority — `priority` field on `[[interceptor]]` in Capsule.toml (lower fires first, default 100). Enables layered interception (e.g. input guard before react loop).
- Distro.lock regeneration on `astrid capsule update` — keeps the lockfile in sync after capsule updates
- Content-addressed WIT storage — capsule install hashes `.wit` files into `~/.astrid/wit/`, capsule remove cleans up unreferenced WIT files, `wit_files` field in `meta.json`
- `astrid capsule tree` command — renders the imports/exports dependency graph of all installed capsules, showing which capsule exports satisfy each import, with unsatisfied imports highlighted in red (`astrid capsule deps` retained as hidden alias)
- `astrid init` with distro-based capsule installation — fetches Distro.toml, multi-select provider groups, shared variable prompts with `{{ var }}` template resolution, progress bars, writes Distro.lock for reproducibility. Supports `--distro` flag for custom distros.
- Distro.toml parser and Distro.lock generator — parse distro manifests with full os-release style metadata, shared variables with `{{ var }}` templates, provider groups, uplink roles, and semver validation. Atomic lockfile writes with BLAKE3 hashes for reproducible installs.
- Kernel boot validation — validates every capsule's required `[imports]` has a matching `[exports]` from another loaded capsule, logs errors for unsatisfied required imports and info for optional ones
- `astrid capsule remove` command with dependency safety checks — blocks removal if the capsule is the sole exporter of an interface that another capsule imports (`--force` to override), cleans up content-addressed WASM binaries from `bin/` when no other capsule references the same hash
- Install capsules from GitHub release WASM assets — `astrid capsule install @org/repo` now downloads pre-built `.wasm` binaries from release assets before falling back to clone + build from source
- Per-principal audit chain splitting — each principal maintains its own independent chain per session, independently verifiable via `verify_principal_chain()` and `get_principal_entries()`
- `AuditLog::append_with_principal()` for principal-tagged audit entries
- Auto-provisioning gated on identity store — only `"default"` principal is auto-provisioned when identity store is configured
- Linux FHS-aligned directory layout (`etc/`, `var/`, `run/`, `log/`, `keys/`, `bin/`, `home/`) replacing the flat `~/.astrid/` structure
- `PrincipalId` type for multi-principal (multi-user) deployments — each principal gets isolated capsules, KV, audit, tokens, and config under `home/{principal}/`
- Content-addressed WASM binaries in `bin/` using BLAKE3 hashing — integrity verified on every capsule load (no hash = no load, wrong hash = no load)
- Per-capsule daily log rotation at `home/{principal}/.local/log/{capsule}/{YYYY-MM-DD}.log` with 7-day retention
- `/tmp` VFS mount backed by `home/{principal}/.local/tmp/` for per-principal temp isolation
- Multi-source capsule discovery with precedence: principal > workspace (dedup by name)
- `PrincipalHome` struct with `.local/` and `.config/` following XDG conventions
- Per-invocation principal resolution — KV, audit, logging, and capability checks scope to the calling user per IPC message, not per capsule load
- `IpcMessage.principal` field for carrying the acting principal through event chains (transparent to capsules)
- `AstridUserId.principal` field mapping platform identities to `PrincipalId` with auto-derivation from display name
- Dynamic KV scoping via `invocation_kv` on `HostState` — capsules call `kv::get("key")` and the kernel returns the right value for the current principal
- Principal auto-propagation on `ipc_publish` — capsules never touch the principal, it flows through event chains automatically
- Auto-provisioning of principal home directories on first encounter
- `astrid_get_caller` host function now returns `{ principal, source_id, timestamp }` instead of empty object
- Dynamic per-principal log routing — cross-principal invocations write to the target principal's log directory
- `AuditEntry.principal` field with length-delimited signing data encoding
- `ScopedKvStore::with_namespace()` for creating scoped views sharing the same underlying store
- `AuditEntry::create_with_principal()` builder for principal-tagged audit entries
- `layout-version` sentinel in `etc/` for future migration support
- `lib/` directory reserved for future WIT shared WASM component libraries
- End-to-end Tier 2 OpenClaw plugin support: TypeScript plugins with npm dependencies install, transpile, sandbox, and run as MCP capsules with full tool integration
- OXC `strip_types()` transpiler for Tier 2 TS→JS (preserves ESM, unlike Tier 1's CJS conversion)
- Node.js binary resolution at build time: prefers versioned Homebrew installs (node@22+), validates each candidate
- MCP-discovered tools are now merged into the LLM tool schema injection alongside WASM capsule tools
- `astrid_net_read` now uses a self-describing `NetReadStatus` wire format: every response is prefixed with a discriminant byte (`0x00` = data, `0x01` = closed, `0x02` = pending), replacing the previous single-byte sentinel hack
- Headless mode: `astrid -p "prompt"` for non-interactive single-prompt execution with stdin piping support
- Post-install onboarding: `astrid capsule install` now prompts for `[env]` fields immediately after install
- Shared `astrid_telemetry::log_config_from()` behind `config` feature flag — replaces duplicate config bridge code
- `--snapshot-tui` mode — renders the full TUI to stdout as ANSI-colored text frames using ratatui's `TestBackend`. Each significant event (ready, input, tool call, approval, response) produces a frame dump. Configurable with `--tui-width` and `--tui-height`. Enables automated smoke testing without an interactive terminal.

### Fixed

- `cwd://` VFS scheme was handled in the security gate (capability checks) but not in the runtime path resolver — capsules using `cwd://` paths at runtime received a security denial because the path resolved to `<cwd>/cwd:/path` instead of `<cwd>/path`
- `sandbox-exec` (Seatbelt) crashes with SIGABRT on macOS 15+ (Darwin >= 24) — skip sandboxing on affected versions
- Headless approval response published to wrong IPC topic (`astrid.v1.approval.response` instead of `astrid.v1.approval.response.{request_id}`) and used wrong decision string (`allow` instead of `approve`)
- `[[component]].capabilities` (fs_read, fs_write, host_process) not merged into root capabilities — security gate couldn't see them
- Lifecycle hooks (`on_install`) couldn't access `home://` VFS — added `home_root` to `LifecycleConfig`
- `astrid init` standard WIT files were installed to `~/.astrid/wit/astrid/` (root-level, no VFS scheme). Capsules access the VFS via `home://` which maps to `~/.astrid/home/{principal}/` — the files were unreachable. Now installed to `~/.astrid/home/{principal}/wit/`, accessible as `home://wit/` (fixes #598)
- Dispatcher `known_principals` HashSet capped at 10K entries to prevent unbounded memory growth
- Dispatcher only caches principal after successful home provisioning — transient failures allow retry on next event
- `AstridUserId.principal` now has `#[serde(default)]` — existing identity records without the field deserialize with `"default"` instead of failing
- `transpile_and_install` now correctly unpacks `.capsule` archives from `astrid-build` output
- `copy_capsule_dir` only skips `dist/` at the top level; npm packages inside `node_modules` retain their `dist/` directories
- MCP host engine: absolute system binaries (e.g. `/opt/homebrew/opt/node@22/bin/node`) skip path traversal check when declared in `host_process` capability
- MCP host engine: `allow_network` derived from capsule capabilities (uplink/net) instead of defaulting to `false`
- Capsule env resolution no longer blocks loading on missing optional fields; fills with empty defaults so uplink capsules can boot before clients connect
- macOS Seatbelt sandbox: added `mach*` permission and unrestricted `file-read*` for Node.js compatibility
- macOS Seatbelt sandbox: hidden path deny rules skip paths that are ancestors of the writable root
- MCP tool schemas now include `properties` field for LLM API compatibility
- `net_write` no longer causes a WASM trap on broken pipe / connection reset when a headless client disconnects; write errors are logged at debug level and the dead stream is cleaned up on the next read
- `net_read` returns a `NET_STREAM_CLOSED` sentinel byte instead of trapping on peer EOF/disconnect, allowing the CLI capsule run loop to remove dead streams gracefully
- Also fixes a variable name mismatch (`capsule` vs `plugin`) in `approval.rs` that caused a compile error
- `~/.astrid/shared/` directory now created on boot, eliminating `global:// VFS not mounted` warning on fresh installs
- Capsule reinstall now preserves existing `.env.json` rather than overwriting it with an empty file
- WASM execution timeout bumped from 30s to 5 minutes to prevent premature cancellation on slow operations
- IPC event dispatcher now delivers events to each capsule in publish order via per-capsule mpsc queues, fixing out-of-order stream text assembly in the ReAct capsule
- `IpcMessage` gains a monotonic `seq` field assigned at publish time for ordering and diagnostics
- KV host function double-encoding: `kv_get_impl` returned `serde_json::to_vec` of raw bytes instead of raw bytes directly
- Config host function double-encoding: `get_config_impl` wrapped string values in JSON quotes, breaking URLs and other string config
- React capsule LLM topic validation: `active_llm_topic()` could produce topics with empty segments causing IPC publish failures
- `astrid_read_file` host function trapped (WASM abort) on recoverable errors (file-not-found, permission denied) — now returns status-prefix wire format (`0x00`+content / `0x01`+error), paired with SDK-side decoding. Eliminates crashes in memory, agents, identity, and fs capsules when reading optional files.

### Changed

- `global://` VFS scheme renamed to `home://`
- `Capsule::invoke_interceptor` now accepts `Option<&IpcMessage>` for per-invocation principal context
- `CapsuleContext.global_root` renamed to `home_root`; `HostState.global_vfs` renamed to `home_vfs`
- `AstridUserId` now requires `principal: PrincipalId` field (existing KV records incompatible — nuke `~/.astrid/`)
- Capsule install target moved from `~/.astrid/capsules/` to `home/{principal}/.local/capsules/` (capsule dir now holds only manifest + meta.json)
- KV namespace format changed from `capsule:{name}` to `{principal}:capsule:{name}`
- Socket/token/ready paths moved from `sessions/` to `run/`
- Env config moved from capsule dir `.env.json` to `home/{principal}/.config/env/{capsule}.env.json`
- System logs now use `.log` extension, no ANSI escape codes in file output, 7-day retention
- `user_key_path()` renamed to `runtime_key_path()` (now at `keys/runtime.key`), `logs_dir()` renamed to `log_dir()`
- Ephemeral daemon now shuts down immediately when the last client disconnects (idle timeout 0, 1s check interval) instead of waiting 5 minutes
- Renamed `plugin` → `capsule` in the WASM host layer and audit log fields for consistency with project terminology
- Split `astrid-build` 1166-line `build.rs` into focused modules: `rust.rs`, `openclaw.rs`, `mcp.rs`

### Removed

- `~/.astrid/capsules/` system capsules directory (user installs go to principal home)
- `sessions/`, `shared/`, `audit.db`, `capabilities.db`, `state/`, `spark.toml`, `cache/capsules/` — replaced by FHS equivalents or moved to principal home

### Breaking

- Existing `~/.astrid/` must be deleted — no migration path. Reinstall all capsules after upgrading.

## [0.4.0] - 2026-03-17

### Added

- `astrid-daemon` crate — standalone kernel daemon binary with `--ephemeral` flag for CLI-spawned instances vs persistent multi-frontend mode
- `astrid-build` crate — standalone capsule compiler and packager (Rust, OpenClaw, MCP). Invoked by CLI via subprocess.
- `astrid start` command — spawn a persistent daemon (detached, no TUI)
- `astrid status` command — query daemon PID, uptime, connected clients, loaded capsules
- `astrid stop` command — graceful daemon shutdown via management API
- `KernelRequest::Shutdown`, `KernelRequest::GetStatus`, and `DaemonStatus` types in `astrid-types`
- `Kernel::boot_time` field for uptime tracking
- Streaming HTTP airlock: `astrid_http_stream_start`, `astrid_http_stream_read`, `astrid_http_stream_close` host functions for real-time SSE consumption (`astrid-capsule`)

### Changed

- CLI no longer embeds the kernel — spawns `astrid-daemon` as a companion binary
- CLI no longer compiles capsules — delegates to `astrid-build` as a companion binary
- CLI reads `IpcMessage` directly from socket instead of wrapping in `AstridEvent::Ipc`
- IPC type imports in CLI now use `astrid-types` directly instead of going through `astrid-events` re-exports
- Package renamed from `astrid-cli` to `astrid` (`cargo install astrid`)

### Removed

- `astrid-kernel` dependency from CLI
- `astrid-openclaw`, `extism`, `cargo_metadata`, `toml_edit` dependencies from CLI
- `Commands::Daemon` and `Commands::WizerInternal` from CLI (moved to `astrid-daemon` and `astrid-build`)

## [0.3.0] - 2026-03-17

### Added

- `astrid-types` crate — shared IPC payload, LLM, and kernel API types with minimal deps (serde, uuid, chrono). WASM-compatible. Both `astrid-events` and the user-space SDK depend on this.
- `yolo` as an alias for `autonomous` workspace mode (`astrid-config`, `astrid-workspace`)

### Changed

- `astrid-events` now re-exports types from `astrid-types` instead of defining them inline. All existing import paths remain valid.
- `astrid-events` `runtime` feature removed — all functionality is now always available. Consumers no longer need `features = ["runtime"]`.

### Removed

- `astrid-sdk`, `astrid-sdk-macros`, `astrid-sys` extracted to standalone repo ([sdk-rust](https://github.com/unicity-astrid/sdk-rust))

## [0.2.0] - 2026-03-15

Initial tracked release. See the [repository history](https://github.com/unicity-astrid/astrid/commits/v0.2.0)
for changes included in this version.

[Unreleased]: https://github.com/unicity-astrid/astrid/compare/v0.4.0...HEAD
[0.4.0]: https://github.com/unicity-astrid/astrid/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/unicity-astrid/astrid/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/unicity-astrid/astrid/releases/tag/v0.2.0
