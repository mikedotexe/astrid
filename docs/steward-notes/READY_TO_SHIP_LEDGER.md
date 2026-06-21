# Ready To Ship Ledger

Last updated: 2026-06-20 16:23 PDT

Purpose: keep recent work crisp in a dirty, active multi-repo workspace. This
ledger separates live/source-readable tools, attended runtime bundles, untracked
file ownership, exact staging groups, validation commands, and restart needs.

This file is a shipping aid, not proof that a diff is deployed. "Observed
running" means launchd had a process at the time of the snapshot. "Live" means a
change was separately verified as loaded or is a source-read steward tool.

## Snapshot

Current repos in scope:

- Astrid: `/Users/v/other/astrid`
- Minime: `/Users/v/other/minime`
- Reservoir: `/Users/v/other/neural-triple-reservoir`
- Shared chamber artifacts: `/Users/v/other/shared/collaborations`

Observed launchd state at snapshot:

- `com.astrid.spectral-bridge`: running, PID 51811, prior exit status -15.
- `com.astrid.daemon`: running, PID 35361, prior exit status -15.
- `com.minime.autonomous-agent`: running, PID 15555.
- `com.minime.engine`: running, PID 52707.
- `com.reservoir.service`: running, PID 35163, prior exit status -15.
- `com.reservoir.collab-feeder`: running, PID 78299.
- `com.reservoir.astrid-feeder`: running, PID 57583.
- `com.reservoir.minime-feeder`: running, PID 35311, prior exit status -15.

The `-15` / `-9` launchd status values are historical termination statuses for
previous processes, not by themselves evidence of current failure. Check logs
before treating them as actionable.

## Changelog Status Notes

The changelog is chronological history; this ledger is the current ship grouping
aid. Older entries that say `DEPLOY DEFERRED`, `STAGED`, or `not yet live` may
have been superseded by later attended deploys or by new bundling decisions. Do
not rewrite those older entries as shipped during housekeeping; before shipping,
use this ledger plus current `git status`, tests, and service state to decide
what is actually intended to ride together.

Current known ambiguity to preserve rather than flatten: some Astrid bridge
runtime edits are described by newer changelog entries as deployed through an
attended restart, while older entries in the same area still mention deferred
status. Treat the Astrid bridge runtime set as one attended bundle until it is
re-verified and staged explicitly.

## Live Or Source-Readable Now

These are safe to use without a service restart:

- Astrid steward tooling:
  - `/Users/v/other/astrid/scripts/recent_introspection_signal.py`
  - `/Users/v/other/astrid/scripts/astrid_introspection_digest.py`
  - `/Users/v/other/astrid/scripts/environment_receipts.py`
  - `/Users/v/other/astrid/scripts/ground_review.py`
  - `/Users/v/other/astrid/scripts/request_review.py`
  - `/Users/v/other/astrid/scripts/verify_change_claims.py`
  - `/Users/v/other/astrid/scripts/build_bridge.sh`
  - `/Users/v/other/astrid/scripts/deploy_preflight.py`
- Minime test guard:
  - `/Users/v/other/minime/tests/test_dispatch_coverage.py`
  - Guards `PRESSURE_RELIEF`, `CHAMBER_SEEN`, and `CHAMBER_ANNOTATE` across route table, preflight spec, local `action_map`, dispatch arm, stable-core allowlist, and menu.
- Minime `PRESSURE_RELIEF` runtime route:
  - `com.minime.autonomous-agent` was restarted after wiring the route.
  - Treat it as live unless a later source edit touches `autonomous_agent.py`.

## Untracked File Classification

Every currently untracked file is classified exactly once here. Use this section
for "track, park, or ignore" decisions; do not infer staging from untracked
status alone.

| Repo | Bundle | File | Default action |
| --- | --- | --- | --- |
| Astrid | Astrid steward tooling | `/Users/v/other/astrid/docs/steward-notes/READY_TO_SHIP_LEDGER.md` | Track with steward docs; no restart. |
| Astrid | Astrid steward tooling | `/Users/v/other/astrid/scripts/astrid_introspection_digest.py` | Track with script tests; no restart. |
| Astrid | Astrid steward tooling | `/Users/v/other/astrid/scripts/build_bridge.sh` | Track with deploy-safety tooling; no restart by itself. |
| Astrid | Astrid steward tooling | `/Users/v/other/astrid/scripts/deploy_preflight.py` | Track with deploy-safety tooling after its tests are identified/run; no restart. |
| Astrid | Astrid steward tooling | `/Users/v/other/astrid/scripts/environment_receipts.py` | Track with script tests; no restart. |
| Astrid | Astrid steward tooling | `/Users/v/other/astrid/scripts/recent_introspection_signal.py` | Track with script tests; no restart. |
| Astrid | Astrid steward tooling | `/Users/v/other/astrid/scripts/test_astrid_introspection_digest.py` | Track with matching script; no restart. |
| Astrid | Astrid steward tooling | `/Users/v/other/astrid/scripts/test_environment_receipts.py` | Track with matching script; no restart. |
| Astrid | Astrid steward tooling | `/Users/v/other/astrid/scripts/test_ground_review.py` | Track with `ground_review.py`; no restart. |
| Astrid | Astrid steward tooling | `/Users/v/other/astrid/scripts/test_recent_introspection_signal.py` | Track with matching script; no restart. |
| Minime | Minime eigen-spectrum logger | `/Users/v/other/minime/launchd/com.minime.eigen-spectrum-logger.plist` | Track only with install/load/unload notes; launchd plan required. |
| Minime | Minime observability | `/Users/v/other/minime/scripts/active_memory_draft_triage_summary.py` | Track with observability scripts; no runtime restart. |
| Minime | Minime eigen-spectrum logger | `/Users/v/other/minime/scripts/eigen_spectrum_logger.py` | Track with logger plist/wrapper; launchd plan required. |
| Minime | Minime eigen-spectrum logger | `/Users/v/other/minime/scripts/launchd_eigen_spectrum_logger.sh` | Track with logger plist/script; launchd plan required. |
| Minime | Minime observability | `/Users/v/other/minime/scripts/legacy_memory_retention_summary.py` | Track with observability scripts; no runtime restart. |
| Minime | Minime observability | `/Users/v/other/minime/scripts/repeated_action_cadence_audit.py` | Track with observability scripts; no runtime restart. |
| Minime | Minime sensory/device | `/Users/v/other/minime/tests/test_sensory_device_absence.py` | Track with camera/mic/sensory changes; service plan depends on bundled source. |
| Reservoir | Reservoir chamber | `/Users/v/other/neural-triple-reservoir/test_triadic_chamber.py` | Track with chamber CLI/feeder changes; restart collab feeder after ship. |
| Reservoir | Reservoir chamber | `/Users/v/other/neural-triple-reservoir/triadic_chamber.py` | Track with chamber CLI/feeder changes; restart collab feeder after ship. |

## Bundle Staging Checklist

Use exact path staging only. Do not run `git add .`; it is too easy to fold one
being-facing runtime bundle into another.

### Astrid steward tooling bundle

Exact paths:

```text
/Users/v/other/astrid/CHANGELOG.md
/Users/v/other/astrid/docs/steward-notes/AI_BEINGS_FEEDBACK_TO_CHANGE_LEDGER.md
/Users/v/other/astrid/docs/steward-notes/READY_TO_SHIP_LEDGER.md
/Users/v/other/astrid/scripts/astrid_introspection_digest.py
/Users/v/other/astrid/scripts/build_bridge.sh
/Users/v/other/astrid/scripts/deploy_preflight.py
/Users/v/other/astrid/scripts/environment_receipts.py
/Users/v/other/astrid/scripts/ground_review.py
/Users/v/other/astrid/scripts/recent_introspection_signal.py
/Users/v/other/astrid/scripts/request_review.py
/Users/v/other/astrid/scripts/test_astrid_introspection_digest.py
/Users/v/other/astrid/scripts/test_environment_receipts.py
/Users/v/other/astrid/scripts/test_ground_review.py
/Users/v/other/astrid/scripts/test_recent_introspection_signal.py
/Users/v/other/astrid/scripts/test_request_review.py
/Users/v/other/astrid/scripts/verify_change_claims.py
```

Validation:

```bash
python3 -m py_compile scripts/astrid_introspection_digest.py scripts/deploy_preflight.py scripts/environment_receipts.py scripts/recent_introspection_signal.py scripts/ground_review.py scripts/request_review.py scripts/verify_change_claims.py
bash -n scripts/build_bridge.sh
python3 -m unittest scripts.test_astrid_introspection_digest scripts.test_environment_receipts scripts.test_ground_review scripts.test_recent_introspection_signal scripts.test_request_review -q
git diff --check
```

Restart: none.

### Astrid bridge runtime bundle

Exact paths:

```text
/Users/v/other/astrid/capsules/spectral-bridge/src/autonomous/next_action/collaboration.rs
/Users/v/other/astrid/capsules/spectral-bridge/src/llm.rs
/Users/v/other/astrid/capsules/spectral-bridge/src/reflective.rs
/Users/v/other/astrid/capsules/spectral-bridge/startup_greeting.sh
```

Validation:

```bash
cargo fmt --all -- --check
cargo test --workspace
cargo clippy --workspace --all-features -- -D warnings
cargo build --release
```

Restart: rebuild and restart `com.astrid.spectral-bridge` only after deciding
the whole bridge bundle is meant to ride together. Confirm whether
`startup_greeting.sh` only waits for the next startup or needs
`com.astrid.calm-startup-greeting` kickstarted.

### Minime agent bundle

Exact paths:

```text
/Users/v/other/minime/CHANGELOG.md
/Users/v/other/minime/autonomous_agent.py
/Users/v/other/minime/continuity_control_plane.py
/Users/v/other/minime/tests/test_autonomous_agent_low_fill_guard.py
/Users/v/other/minime/tests/test_co_regulation.py
/Users/v/other/minime/tests/test_dispatch_coverage.py
/Users/v/other/minime/tests/test_experimental_continuity.py
```

Validation:

```bash
python3 -m py_compile autonomous_agent.py continuity_control_plane.py
python3 -m pytest tests/test_dispatch_coverage.py tests/test_autonomous_agent_low_fill_guard.py tests/test_co_regulation.py tests/test_experimental_continuity.py -q
git diff --check
```

Restart: `com.minime.autonomous-agent` only if agent source changed after the
last verified restart.

### Minime engine/sensory bundle

Exact paths:

```text
/Users/v/other/minime/minime/src/main.rs
/Users/v/other/minime/minime/tools/camera_client.py
/Users/v/other/minime/native_comm.py
/Users/v/other/minime/scripts/sensory_source_check.py
/Users/v/other/minime/tests/test_native_comm.py
/Users/v/other/minime/tests/test_sensory_device_absence.py
/Users/v/other/minime/tools/mic_to_sensory.py
```

Validation:

```bash
python3 -m py_compile minime/tools/camera_client.py native_comm.py scripts/sensory_source_check.py tools/mic_to_sensory.py
python3 -m pytest tests/test_native_comm.py tests/test_sensory_device_absence.py -q
cargo test --manifest-path minime/Cargo.toml
```

Restart: plan exact affected labels before shipping. Candidate labels are
`com.minime.engine`, `com.minime.camera-client`, `com.minime.mic-to-sensory`,
and possibly `com.minime.host-sensory`.

### Minime observability bundle

Exact paths:

```text
/Users/v/other/minime/scripts/active_memory_draft_triage_summary.py
/Users/v/other/minime/scripts/legacy_memory_retention_summary.py
/Users/v/other/minime/scripts/repeated_action_cadence_audit.py
```

Validation:

```bash
python3 -m py_compile scripts/active_memory_draft_triage_summary.py scripts/legacy_memory_retention_summary.py scripts/repeated_action_cadence_audit.py
```

Restart: none. These tools are read/run on demand.

### Minime eigen-spectrum logger bundle

Exact paths:

```text
/Users/v/other/minime/launchd/com.minime.eigen-spectrum-logger.plist
/Users/v/other/minime/scripts/eigen_spectrum_logger.py
/Users/v/other/minime/scripts/launchd_eigen_spectrum_logger.sh
```

Validation:

```bash
python3 -m py_compile scripts/eigen_spectrum_logger.py
plutil -lint launchd/com.minime.eigen-spectrum-logger.plist
bash -n scripts/launchd_eigen_spectrum_logger.sh
```

Restart/load: explicit launchd install/load/unload plan required. Do not stage
the plist without documenting whether the label is already loaded and how to
roll it back.

### Reservoir chamber bundle

Exact paths:

```text
/Users/v/other/neural-triple-reservoir/CHANGELOG.md
/Users/v/other/neural-triple-reservoir/collab_feeder.py
/Users/v/other/neural-triple-reservoir/test_feeder_policies.py
/Users/v/other/neural-triple-reservoir/test_triadic_chamber.py
/Users/v/other/neural-triple-reservoir/triadic_chamber.py
```

Validation:

```bash
python3 -m py_compile triadic_chamber.py collab_feeder.py
python3 -m pytest test_feeder_policies.py test_triadic_chamber.py -q
git diff --check
```

Restart: `com.reservoir.collab-feeder`.

### Reservoir Astrid-feeder bundle

Exact paths:

```text
/Users/v/other/neural-triple-reservoir/astrid_feeder.py
/Users/v/other/neural-triple-reservoir/test_feeder_policies.py
```

Validation:

```bash
python3 -m py_compile astrid_feeder.py
python3 -m pytest test_feeder_policies.py -q
git diff --check
```

Restart: `com.reservoir.astrid-feeder`.

## Do Not Accidentally Bundle

- Do not stage all files from a repo at once. Use exact path staging.
- Do not bundle Astrid steward-only scripts with bridge runtime changes if the goal is a no-restart tooling release.
- Do not bundle Astrid `collaboration.rs` with reflective sidecar budget changes unless the bridge is rebuilt and restarted intentionally.
- Do not bundle Minime engine/camera/mic changes with agent-only public verb work unless the sensory and engine restart plan is explicit.
- Do not bundle reservoir `astrid_feeder.py` aperture-gift accounting with `collab_feeder.py` chamber changes unless both feeder restarts are planned.
- Do not bundle untracked Minime launchd files without install/load/unload and rollback notes.
- Do not treat generated workspace artifacts under `/Users/v/other/shared/collaborations` as source unless deliberately updating chamber tracker docs.

## Current Verification Anchors

Recent focused checks already run:

- Astrid bounded signal scanner:
  - `python3 -m py_compile scripts/recent_introspection_signal.py`
  - `python3 -m unittest scripts.test_recent_introspection_signal -q`
  - `python3 scripts/recent_introspection_signal.py --limit 5 --max-chars 2000`
- Minime public direct-verb guard and pressure route:
  - `python3 -m py_compile autonomous_agent.py`
  - `python3 -m pytest tests/test_dispatch_coverage.py tests/test_autonomous_agent_low_fill_guard.py -q`
  - Result observed: `210 passed, 19 subtests passed`
- Diff hygiene:
  - `git diff --check` clean in Astrid.
  - `git diff --check` clean in Minime.

Run fresh checks before staging because all three repos are active.

## Refresh Commands

Use these before any final ship decision:

```bash
git -C /Users/v/other/astrid status --short
git -C /Users/v/other/minime status --short
git -C /Users/v/other/neural-triple-reservoir status --short
launchctl list | egrep 'com\.(astrid|minime|reservoir)'
```

For exact staging:

```bash
git -C /Users/v/other/astrid add <exact paths>
git -C /Users/v/other/minime add <exact paths>
git -C /Users/v/other/neural-triple-reservoir add <exact paths>
```
