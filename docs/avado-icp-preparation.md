# Avado and ICP Preparation

Status: source preparation and local rehearsal only; no deployment authorized.
Research baseline: `introspection`, commit
`f258d38fe99690a34ee53bb88e8fca2d2cbbe722` (workspace version 0.5.6).

This work targets the research fork. Contributions to `astrid-runtime/astrid`
remain a separate stream, based on current upstream, without importing the
research fork's main branch, device state, or release tags.

## Branch Boundary

Use `codex/avado-icp-preparation` in a separate checkout. This preparation
checkpoint contains no bridge, Minime, kernel, capsule, or device-service code
changes. Follow-up device repairs should use focused branches and separate PRs.
Merging preparation documents does not approve an appliance rollout.

The source candidate stays pinned to `introspection`, even as the preparation
branch gains documentation and tooling. A PR branch tip is not automatically
the appliance's candidate source revision. Record the exact source, build,
capsule, and test identities for each later candidate.

## Readiness Checkpoint

Read-only observations were taken September 5, 2026, approximately 19:42-20:17
America/Los_Angeles. These are a dated baseline, not a live status dashboard.

| Observation | Avado | ICP |
| --- | --- | --- |
| Reported daemon version | 0.5.1 | 0.5.1 |
| Loaded and installed capsule count | 19 | 19 |
| Reported capsule runtime health | OK | OK |
| Daemon and edge process images | Match installed bytes and inodes | Match installed bytes and inodes |
| Exact native source revision | Unproven; embedded revision unrecorded | Unproven; embedded revision unrecorded |
| Current native install receipts | Not found at inspected paths | Match installed/running hashes |
| Edge-spectral capsule | Absent | Absent |
| Thread schema | v5 | v6 |
| Recent workflow | Varied activity | Repeated failed action loop |
| Current full restore point | Not demonstrated | Not demonstrated |

The checkpoint's CPU-edge configuration contains 20 capsules, including
edge-spectral, and newer native runtime/recovery machinery. Installed payloads
match across devices for 16 capsules. React, edge-context, and edge-introspector
differ despite equal package version strings. Loaded names do not attest the
hashes of in-memory WASM modules, and cross-device equality does not prove
equality with a rebuilt checkpoint artifact.

Nearby source directories and older staged bundles cannot identify live source
commits by their names. In particular, ICP's older source-labeled bundle has
different native hashes from its running installation. Retain that distinction
instead of assigning a convenient but unsupported revision.

### Continuity and Resource Gates

In the sampled last 24 hours, Avado recorded 92 completed run receipts with
distinct response hashes. Its action receipts included executed, honored, and
repaired outcomes. ICP recorded 60 completed run receipts with one repeated
response hash and 60 action failures classified as missing owned artifacts;
its thread projection last advanced August 12. This does not prove that model
generation stopped or establish why the referenced artifact is missing.

ICP's current boot has four Ollama automatic restarts preceded by `oom-kill`
service-manager results; five more appear in the preceding boot's August
history. The service was running at observation time. Qualify sustained memory
headroom and failure recovery before adding build or runtime load. The audit
does not establish a leak, identify the peak-memory cause, or causally connect
OOM events to the artifact failure.

Both hindsight projections report valid current continuity with zero
current-epoch integrity violations, but retain historical legacy-race
exceptions. These operator projections and older authorship labels are not a
new independent cryptographic verification of full history. Do not rewrite
responses, create replacement artifacts, clear exceptions, or reset sessions
and counters merely to improve a readiness display.

Free capacity was ample on both devices. ICP's mounted SSD identity/options and
existing read-only guard passed inspection. This is not media-health
certification. Three retained ICP backup manifests verified all 747 listed
files, without skips or mismatches. Those July/August copies are not current
restore points, and checksum agreement does not prove complete coverage,
application consistency, metadata restoration, or bootability. Avado's located
July backup was partial and had no checksum manifest at the inspected location.

## Source Candidate

`scripts/prepare_edge_source_candidate.py` is the portable form of the local
candidate-freezing experiment. It performs no SSH, fetch, build, signing,
runtime invocation, service operation, or state migration. Python 3.11+ and
Git are required on the operator machine, not on the appliances.

Supply a clean source checkout whose HEAD matches the expected full commit and
an explicit ref that resolves to the same commit. Use a new output directory
outside that source checkout; its parent must already exist:

```bash
python3 scripts/prepare_edge_source_candidate.py \
  --repo /path/to/isolated-source-checkout \
  --source-ref refs/tags/introspection \
  --expected-commit f258d38fe99690a34ee53bb88e8fca2d2cbbe722 \
  --output-dir /path/to/private-parent/new-candidate
```

The tool archives immutable Git content, not ignored local state. It records
the commit/tree, workspace version, source archive digest, tracked build-input
digests, external capsule specifications, and required embedded edge source
identity. It creates a private output directory and refuses reuse. A failed
attempt may leave an incomplete directory, but never a success receipt.

The result is **source-pinned, not Linux-release-qualified**. Recorded external
inputs are not downloaded or independently validated. The result is not the
signed offline build closure or installable bootstrap produced by the existing
[source-bundle builder](../scripts/build_edge_self_change_source_bundle.py) and
[bootstrap packager](../scripts/package_edge_self_evolution_bootstrap.py).

## Rehearsal

The unchanged introspection source passed these local checks on Darwin arm64:

- Edge runtime: 239 tests, Rust 1.94.1 with locked dependencies.
- Checkpoint helper: 14 tests, including snapshot/restore, tamper rejection,
  interrupted staging/root-swap recovery, and restore replay/idempotence.
- Python state-store, legacy migrations, action reconciliation, bootstrap
  packaging, and release verification: 45 tests.
- Root-bootstrap dry-run, bundle-installer fixtures, and ICP SSD-guard fixtures.

The initial Python test command required `PYTHONPATH=scripts` for two modules'
sibling imports; the corrected invocation passed without source changes.
These 298 tests and three shell suites are historical candidate evidence,
not tests of this PR's new helper. Test the helper separately:

```bash
python3 -m unittest scripts/test_prepare_edge_source_candidate.py
```

The local rehearsal used synthetic fixtures, not copied appliance histories.
It did not qualify Linux/systemd/cgroups/mounts, a full 20-capsule release,
private production database migration, or backup restoration. Legacy v5 thread
migration tests preserve old snapshots as unscoped archives rather than
asserting old claims as newly verified beliefs. Full workspace tests and Clippy
were not run during that rehearsal.

## Next Gates

1. Qualify the exact candidate in a dedicated baseline x86-64 Linux environment:
   Rust 1.94.1, wasm32-wasip2 capsules, glibc no newer than ICP's 2.35, and no
   AVX2 requirement. Follow the existing CPU-edge build and bootstrap workflow.
2. Obtain maintenance approval for a fresh, application-consistent Avado backup.
   Quiesce actual writers; preserve private keys, audit/KV, workspace history,
   capsule objects/manifests/config, service units/drop-ins, launchers, and
   operator hindsight as explicit recovery scopes. Hot database-file copying
   is not sufficient proof of consistency.
3. Restore in isolated Linux state first; then rehearse migration, interrupted
   migration, failed activation, and exact rollback. The post-bootstrap runtime
   snapshot alone is not a complete pre-bootstrap host/config backup.
4. Investigate ICP's missing-artifact loop and OOM pressure separately. Preserve
   history and test proposed repairs in isolation. An upgrade is not presumed
   to repair either problem.
5. Request separate live rollout approval. Avado goes first under the
   [existing rollout gates](cpu-edge-self-evolution.md#rollout-rule); ICP follows
   only after those gates and its own readiness issues are resolved.

## Evidence Handling

Raw receipts, journal extracts, local paths, device identifiers, response
hashes, source archives, private collector scripts, and backup inventories
remain operator-local under the ignored `.device-local/` directory. They are
not public fixtures or backed up by a normal Git push. Preserve an appropriate
private backup separately. Do not force-add them to this PR or an upstream PR.

The public report intentionally includes aggregate operational findings and
their limits, not raw device evidence. It is an operator summary, not an
independent release attestation. The private collectors remain provisional
local investigation tools; they are not silently promoted into production
deployment or public diagnostic interfaces by this preparation checkpoint.
