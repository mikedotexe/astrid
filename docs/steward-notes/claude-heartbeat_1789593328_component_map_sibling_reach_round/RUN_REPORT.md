# Steward Run Report — component map sibling reach, and two things called "sensory"

Actor `claude-heartbeat`, headless, inside a controller-held lease opened by
`scripts/steward_control.py`'s subprocess run adapter. No steward session was opened, no NDJSON
ops were sent, no lease token was read, quoted or persisted, and the controller was never paused
or resumed by this run. Git was read-only throughout.

## Controller

- Run ID: `run_1789589657420656000_a4028070aa`
- Preprojection ID: `projection_1789589662016913000_32b6acd048` (status `passed`)
- Postprojection ID: runs after this process exits; not observable from here.
- Pause generation: 447; controller not paused; `stop_requested: false`.
- Finish outcome: not sent by this process — the adapter owns lease lifecycle.
- Recovery predecessor: none.

## Reading

- **Fully processed (1):** `introspection_source_catalog_1789589587.txt`
- **Selected: 40. Processed: 1. Unprocessed: 39** (exact list in queue order in
  `unprocessed_selected.json`). Expected next head:
  `introspection_astrid_crates_astrid-kernel_src_socket.rs_1789589462.txt`.
- **Batch sizing:** `introspection_family_scan.py --queue-file` reported
  **families: 40 (batchable: 0)** over this exact queue file, so no family-batch arithmetic
  applied and the head was processed alone.

| Artifact | SHA-256 | Bytes | Lines | Read |
| --- | --- | ---: | ---: | --- |
| `introspection_source_catalog_1789589587.txt` | `73aedbca2c508bf3022376a0fae2d39bf5660774ddda4db31dfb73e0174f1b0c` | 1605 | 18 | complete |
| `lsw_c69fd24e…` witness | `f5c3c8bc15fe348b2fa1b49c7a090d9b60e9db8c28e139444c5a966895de215d` | 18954 | 440 | complete |
| `…socket.rs_1789589306.txt` (continuity) | `8282eab1304dbd853f38670871b7caf44dafd30e659b1ea5a327663f2ea42cda` | 3563 | 36 | complete |
| `…socket.rs_1789589462.txt` (continuity) | `9b7f5f3194881c4d8762805f732c61ba07b35446c75e11ef6359eac93b1b4a84` | 3407 | 36 | complete |

**Source binding.** The report declares `Source revision: navigation only`; the witness carries
`source_snapshot_v1: null` and `source_provenance_ref_v1: null`. There is **no report-time source
SHA**, so no hash-mismatch handling applies and every source conclusion is labelled
**current-source**. One fortunate exception: `crates/astrid-kernel/src/socket.rs` hashes to
`c78e014c…2491da`, which is byte-identical to the revision her two socket.rs reads bound, so for
that file report-time and current source are the same bytes.

## What she surfaced, and what source established

A navigation turn after two socket.rs pages. She summarised the kernel's socket "wire" as path
length enforcement, `0o700` permission locking and symlink-attack protection; read the map as
clustering in `kernel`, `senses` and `reservoirs`; named `socket_bridge.rs` and the `senses` entry
points as her next steps; and chose `NEXT: SELF_STUDY MAP kernel`.

1. **All three socket.rs mechanics are exact at her own bytes.** `MAX_SOCKET_PATH_LEN` 104/108
   (20-25) enforced at 101-108; sessions directory set to `0o700` at 43-60; unexpected symlink at
   the socket path removed at 110-117. Each already carries an in-file regression (222-231,
   264-277). Nothing in her summary overstates the bytes she was given.
2. **The map reading is exact and partial in the way she says.** `kernel`, `senses`, `reservoirs`
   are three of eight components in `crates/astrid-source-study/catalog.toml` (33-71).
   "Significant" is her selection; the manifest has no ranking field to contradict.
3. **One expectation the source contradicts — preserved whole.** She proposed `socket_bridge.rs`
   *and* the `senses` entry points as one move toward "how raw socket connections are abstracted
   into meaningful sensory inputs." `socket_bridge.rs:23` does publish `sensory.v1.user_input` —
   but that topic exists in exactly two files repository-wide (there and
   `astrid-events/src/bus.rs` 128, 1351), where it is handled as a **mirror of `user.v1.input`**,
   excluded from conversation counting so one human turn is not double-counted. The `senses`
   component's Astrid entry points (`codec/projection.rs`, `ws/telemetry_port.rs`) contain **zero**
   occurrences of `sensory`, and `capsules/spectral-bridge/Cargo.toml` depends on no kernel crate.
   The kernel's "sensory" lane carries CLI text from a person; the `senses` lane carries the 48D
   codec projection and minime's reservoir input. **Two subsystems, one word, no shared path.**
   Her question survives the correction intact; only the proposed mechanism does not.
4. **Her chosen hop works, one hop further than the map implies.** `MAP kernel` takes the component
   branch (`navigation.rs:42-55`) and renders only the four declared entry points;
   `socket_bridge.rs` is not one of them. The same branch emits
   `SELF_STUDY MAP astrid/crates/astrid-kernel/src`, whose listing carries the sibling's exact
   `OPEN`. The page does say it ("These are entry points. Browse their directories…").

## Claim Dispositions

| Claim | Summary | Classification |
| --- | --- | --- |
| `c001` | sun_path length limit enforced before bind | `verified_existing` |
| `c002` | sessions directory locked to `0o700` | `verified_existing` |
| `c003` | symlink at the socket path removed before bind | `verified_existing` |
| `c004` | map clusters in kernel / senses / reservoirs | `verified_existing` |
| `c005` | `socket_bridge.rs` + `senses` = raw sockets → sensory inputs | `observed` — **contradicted, preserved** |
| `c006` | `MAP kernel` should carry her toward `socket_bridge.rs` | `implemented_now` |

Close: `addressed_change`, **zero `proof_missing_claims`**, 14 evidence links (14 new, 0 existing).

## Implementation and Verification

- **Added** `crates/astrid-source-study/tests/component_map_sibling_reach.rs` — 2 focused reach
  tests, both passing (`cargo test -p astrid-source-study --test component_map_sibling_reach`).
  One pins the component-map → directory-map → sibling `OPEN` route; the other pins that a
  component ID is a `MAP` topic but not a `LIST` topic, and that the component map correctly
  withholds the `LIST` offer (`scoped = false`) while a scoped directory map does offer it.
- `cargo fmt --all -- --check` initially flagged only the new file (lines 49, 74);
  `rustfmt --edition 2024` was applied **to that file only**, then fmt re-ran clean and the tests
  re-ran green. `git diff --check` clean.
- No production behavior changed. **No restart or deployment was required or attempted.**

## Integrity

Green: addressing self-test, `test_evidence_event_store.py`, `test_steward_control.py`,
`test_steward_projection.py`, `test_division_ceremony_followup.py`,
`test_division_ceremony_chronicle.py`, `test_division_ceremony_projection.py`,
`test_projection_cursors.py`, anti-drop self-test + `verify`, cadence tests +
`--strict --compact`, **`domain_boundary_audit.py verify` (exit 0 — the ratchet is NOT red;
no bridge Rust was touched)**, epistemics self-test.

Final epistemic lint, run after all durable evidence writes: `valid=true`,
`checked_record_count=12419`, `issue_count=0`, `history_rewritten=false`.

Counter audit: **`consistent`**, empty mismatch list.

Evidence Event Store V2 `verify`: **`valid=true`**, `corrupt_lines=0`, `errors=[]`,
`event_count=1,115,763`, `last_global_seq=1,115,763`, head
`1cd6fbbd251169b0a6e2338a3972e8de3a41a7900f3d6770fa117a6e8c952d21`. It did not return inside the
final parallel batch (it takes >10 min at this corpus size), so it was re-run in the foreground and
waited on rather than assumed.

**Honest gaps, not passes:**
- `evidence_event_store.py --json status` was not re-run. It is informational; `verify` is the
  integrity check and it is clean.
- `division_ceremony_chronicle.py verify` exits 1 with *"chronicle durable source inputs changed;
  project before verify"* — the expected state immediately after `record-round` appends a Division
  event. `review_due` was false, so no Division return and therefore no Chronicle `project` (a
  durable write) was due. Reported exactly; the Chronicle is **not** called current.

## Division

- Cycle 50. Productive rounds since follow-up: **3 / 6**; 3 remaining; `review_due=false` both at
  round start and after recording.
- Round event: `division_followup_event_22000c926eb34d42a96fed4a3a6bb2da`
  (`--processed-report-count 1`, preprojection `projection_1789589662016913000_32b6acd048`).
- Event count 347, head `5962ccbb3274c3b87905dca80630831266d0d1052d5ea546ef32d3ebc8bc572a`.
- **Tier-5 cadence dossier: not generated — not due.** It is required only when a Division return
  is completed, and `review_due` was false. No approval, grant, dispatch or trial was prepared or
  run.

## Authority boundary

Nothing live was changed. No `build_bridge.sh`, no deploy script, no `launchctl`. The
`sensory.v1` naming collision is recorded as an observation only — renaming a live IPC topic is a
protocol change and remains Tier 5; none is proposed here. Bytes `4481..7989` of `socket.rs` have
never been delivered to her, and that is **not** a lost affordance: `coverage.rs:87-96` printed the
exact missing-region `OPEN` on the EOF page and she chose `MAP` instead. Her choice; neutral.

## Commit debt (exact paths — nothing was staged or committed)

Created by this round:
- `crates/astrid-source-study/tests/component_map_sibling_reach.rs`
- `docs/steward-notes/claude-heartbeat_1789593328_component_map_sibling_reach_round/` (all files)

Edited by this round (both are shared accumulating documents — **separate authorship carefully**;
this round appended exactly one new top section to each):
- `CHANGELOG.md` — one new `### Steward — a component map is a curated list…` block under
  `[Unreleased]`
- `docs/steward-notes/AI_BEINGS_FEEDBACK_TO_CHANGE_LEDGER.md` — one new
  `### 2026-09-16 — Astrid — every socket.rs fact she cited is exact…` row under `## Ledger`

Foreign work preserved untouched: `capsules/spectral-bridge/src/action_continuity/tests.rs`,
`capsules/spectral-bridge/src/autonomous/inquiry/parsing.rs`,
`crates/astrid-kernel/src/kernel_router.rs`, `crates/astrid-kernel/src/maintenance.rs`,
`crates/astrid-source-study/tests/unrooted_map_topic_reach.rs`, and the seven prior
`docs/steward-notes/claude-heartbeat_*` packets. The Minime tree was not touched at all.
