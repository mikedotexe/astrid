# PR Draft: Portable Cooperative Steward Lifecycle

Status: blocked on maintainer triage and assignment of
[astrid-runtime/astrid#1271](https://github.com/astrid-runtime/astrid/issues/1271).

This draft is a preparation aid. It is not a pull request and must be refreshed
against upstream `main` after assignment.

## Linked Issue

Closes #1271

## Summary

Add a scheduler-neutral cooperative lifecycle for external maintenance
sessions. The helper starts paused, allows one opaque-token lease, supports
durable pause and cooperative stop requests, records bounded owner-only
receipts, and never mutates git or acquires runtime authority.

## Proposed Changes

- Add a Python 3.12+ standard-library package under the maintainer-approved
  location.
- Resolve configuration relative to the script or explicit TOML path.
- Persist owner-only pause, lease, and run state with atomic replace.
- Persist only SHA-256 lease-token hashes.
- Add status, pause, resume, begin, heartbeat, finish, run, and reconcile
  commands.
- Interrupt wrapped subprocesses cooperatively with `SIGINT`.
- Capture repository identity read-only and report prohibited mutations.
- Expose injected verifier, receipt-sink, projection-hook, and source-lag
  protocols without enabling a project-specific projector.

## Explicit Exclusions

- Evidence Event Store V2 and its stream names
- introspection, Sandbox, Corridor, Signal Spine, claim-family, dossier, or
  felt-contract projectors
- spectral bridge or Minime paths
- launchd/systemd/cron installation
- subprocess output, prompts, private workspace content, credentials, or raw
  token persistence
- git writes, deployment, approval, capability, capsule, or live-control
  authority

## Verification

- [ ] Configuration precedence and relative-path tests
- [ ] Owner-only state and spool permission tests
- [ ] Lease exclusivity, stale recovery, pause race, and watchdog tests
- [ ] Raw-token absence from files and receipts
- [ ] Interruption, crash, missing-heartbeat, duplicate-finish, and retry tests
- [ ] Shell-free subprocess exit propagation
- [ ] Temporary git repository remains unchanged in HEAD, branches, index,
      worktree baseline, and remotes
- [ ] Portability scan rejects personal paths, provider hooks, credentials,
      and shell interpolation
- [ ] All source files remain below 1,000 lines
- [ ] `cargo fmt --all -- --check`
- [ ] `cargo clippy --workspace --all-features -- -D warnings`
- [ ] `ASTRID_AUTO_BUILD_KERNEL=1 cargo test --workspace`

## Maintainer Decisions Needed Before Coding

- Repository-owned helper versus separate operations package
- Python helper versus another implementation language
- Default receipt behavior: injected sink, no persistence, or local JSON
- Whether projector hooks belong in the first issue
- Whether scheduler examples belong in-tree

## Checklist

- [ ] Issue assigned to the contributor
- [ ] Maintainer confirms accepted scope
- [ ] Fresh branch created from the then-current upstream `main`
- [ ] `[Unreleased]` changelog entry added
- [ ] Pull request template completed
- [ ] Required newcomer approval label applied
