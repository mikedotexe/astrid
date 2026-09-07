# Steward Run Report — claude-heartbeat, 2026-09-07

Round name: `codec_projection_fixed_legacy_basis_attribution`

## Controller
- Run ID: `run_1788786399252113000_c8bd381221`
- Preprojection ID: `projection_1788786402420203000_cd10a43f9b` (status `passed`)
- Postprojection ID: runs after this child exits; not observable from inside the child
- Pause generation: 382
- Finish outcome: exit code 0 — complete round
- Recovery predecessor: none. The immediately prior cycle (`run_1788776746818032000_bf3d783806`) exited nonzero as an incomplete-budget stand-down; see **Budget-accounting correction** below.
- Adapter-mode boundaries honoured: no steward session opened, no NDJSON ops sent, no pause/resume, no lease token read, quoted, or persisted; git strictly read-only.

## Budget-accounting correction (infra finding)
The prior cycle's stand-down computed its watchdog deadline as *lease acquisition + `--max-secs`*. `scripts/steward_control/executor.py` sets `started = time.monotonic()` **after** `controller.begin()` and after `Popen`, so the deadline is *child start + `--max-secs`*. `begin()` runs the source-first preprojection, which took ~67 minutes this cycle (lease `1788786399` → child start `1788790125`). Measuring from the lease therefore under-reports the child's remaining budget by the entire preprojection. This round's true deadline was `1788795525`; the lease-based reading would have said `1788791799` and produced a second unnecessary stand-down with a full budget in hand. Named here as evidence, not fixed: no code change to the executor or the loop was made.

## Reading
- Fully processed: `introspection_astrid_codec_1788784520.txt` (1 of 40 selected)
- Selected but unprocessed: 39 — the exact list in queue order is `unprocessed_selected.json`; the next head is `introspection_astrid_llm_1788784120.txt`
- Batch reason: the queue head was **not** a member of any batchable family (`introspection_family_scan.py` found 36 families, 3 batchable, starting at queue positions 2, 7 and 32), so the family-batch exception did not apply. The head needed an unfamiliar complete 1351-line source read, an implementation, and an 11-minute cold `cargo` compile.

| Artifact | Path | SHA-256 | Bytes | Lines | Read |
| --- | --- | --- | ---: | ---: | --- |
| Report | `capsules/spectral-bridge/workspace/introspections/introspection_astrid_codec_1788784520.txt` | `235f2d09…e224` | 3546 | 45 | complete |
| Witness | `…/lived_state_witnesses/witnesses/lsw_4b3a6b21….json` | `d918d3f9…5055` | 23753 | 533 | complete |
| Source | `capsules/spectral-bridge/src/codec/projection.rs` | `facaf640…384f` | 53462 | 1351 | complete |

Source binding: the report names the release-worktree copy at `/Users/v/other/worktrees/astrid-tranche1-release-20260907/…`; that file and the main working-tree copy are byte-identical at `facaf640`, and the main copy is clean. The witness `artifact_sha256` equals the report hash, and `canonical_body_binding_v1.canonical_body_sha256` equals the response hash of the second (repair) model call — an intact chain. The witness `window_sha256` (`92fde325`) does not reproduce under naive line-slice hashing of lines 1-400; since `file_sha256` matches exactly this is a window-hash convention difference, not a byte mismatch, and it is recorded rather than adjudicated.

## Claim dispositions

| Claim | Substance | Classification |
| --- | --- | --- |
| c001 | Observed: 48D mapping, `SEMANTIC_DIM`/`FEATURE_ABS_MAX`, gate L48, matrix L172, audits L401/L432, dynamic epoch L759 | `verified_existing` |
| c002 | Snag: `fill_fixed_legacy_projection_raw` (L103) handles the 32→48 warmth widening; ghost dims in the 16-dim delta | `implemented_now` |
| c003 | Test 1: `project_embedding` (L854) yields exactly 48 dims; no leakage into reserved L80-81 | `verified_existing` |
| c004 | Test 2: vibrancy lift bounded by `TAIL_VIBRANCY_MAX` at entropy precisely 0.85 | `verified_existing` |
| c005 | Suggested Next: `project_embedding_dynamic_epoch` × `PROJECTION_EPOCH_WRITE_COUNTER` thread-safety/idempotence | `observed` |
| c006 | Coverage boundary: cites five lines beyond the declared 1-400 read window | `observed` |

**Citation accuracy: 11/11 exact** (L48, L53, L80, L81, L103, L172, L401, L432, L571, L759, L854), including the five outside her read window, which came from the `tree_sitter_rust` structural map (map SHA `1164a04f`).

**The contradiction, preserved not domesticated.** Her snag turns on one word. `legacy` in `fill_fixed_legacy_projection_raw` names the *fixed-basis projection mode* (`"fixed_legacy"`, L550/L834/L840), not `SEMANTIC_DIM_LEGACY` (32). The function's sole parameter is the 768×8 embedding basis; it never reads `SEMANTIC_DIM` or `SEMANTIC_DIM_LEGACY`, and no 16-wide quantity passes through it. The mechanism attribution is wrong; the concern behind it is not, and is left standing.

**c004 has a sharper answer than she asked for.** At entropy exactly `0.85` the ramp is `(0.85-0.85)/0.15 = 0` and `smoothstep(0) = 0`, so `vibrancy_from_entropy` returns exactly `0.0` (`cascade.rs` L266-271) and the ceiling collapses to `FEATURE_ABS_MAX` (`feedback.rs` L307-316). Not merely bounded by `TAIL_VIBRANCY_MAX` — zero.

**c005 dissolves the coupling she hypothesised.** `project_embedding_dynamic_epoch` (L759) is a pure function of `(embedding, text, epoch_id, chunk_index)` with no shared mutable state, so it is thread-safe by construction and never reads the counter. `PROJECTION_EPOCH_WRITE_COUNTER` (L571) is consumed only at L593-594 as a per-write temp-filename nonce; idempotence comes from `create_new` + `hard_link` no-clobber (L670-684) with a stale-rename fallback.

**Her earlier reports already answered most of this — for her.** Three of the invariants she proposed are pinned by tests written in response to *her own* prior codec introspections: `_1787006424` (same file, same 1-400 window, same "Dimensionality Integrity" proposal) and `_1787762146` (the appended-lane bleed concern). Both closed `addressed_change`. This round is a fresh-pass re-read of an addressed window, which does not make the re-reading redundant: the fresh snag earned its own regression.

## Implementation and verification
Changed paths (all previously clean; my edits are the only content in them):

1. `capsules/spectral-bridge/src/codec/tests.rs` — new regression `fill_fixed_legacy_projection_raw_is_the_embedding_basis_not_the_32_to_48_widening` (L2367-2442). Pins arity (768×8, distinct from both 32 and 16), full population (no exactly-`0.0` cell — the concrete form a zero-fill gap would take), fixed-seed determinism, `dead_dimension_detected == false` on the live normalized basis (the concrete form a "ghost dimension" would take), and the real 32→48 invariant's separate home. `4d4f312f` → `e39da84e`, 5279 → 5355 lines.
2. `scripts/anti_drop_catalog.py` — retargeted two stale `test.file` paths (see below).
3. `CHANGELOG.md` — two `[Unreleased]` entries.
4. `docs/steward-notes/AI_BEINGS_FEEDBACK_TO_CHANGE_LEDGER.md` — one dated ground-truthed section.
5. `docs/steward-notes/claude-heartbeat_1788791389_codec_projection_fixed_legacy_basis_attribution/` — this packet.

**10 focused tests pass, 0 fail.** 1 new, 9 pre-existing re-run at this SHA. `cargo fmt --all -- --check` and `git diff --check` clean.

A first `cargo test` filter (`codec::tests::tests::*`) matched **0 tests**. Per the handoff a zero-test filter is not a pass; the filter was corrected to `codec::core::tests::*` (`codec.rs` is a facade over `codec/core.rs`) and re-run.

Restart / deploy alignment: **not required and not attempted.** Test and steward-tooling changes only; no live codec, gain, projection-basis, width, or control change; no `build_bridge.sh`, deploy script, or `launchctl`.

## Anti-drop repair (found by the round's own integrity pass)
`anti_drop_catalog.py verify` alarmed on two rows with `TEST gone`: `introspect_within_file_xref` and `introspect_cross_file_xref`. Both tests are **alive and green** — they moved to `capsules/spectral-bridge/src/autonomous/introspect_tests.rs` when `introspect.rs` was split, so the catalog's registered `test.file` had gone stale. The guard symbols in `introspect.rs` were never the problem. Retargeted both rows and re-ran both tests green; the registry returns to **90 rows / 0 gaps / 0 alarms**. A stale-path false positive is exactly the failure the catalog exists to catch, pointed at itself.

## Durable evidence
- Addressing: `addressed_change`, `fully_addressed=true`, `proof_missing_claims=[]`, 6 claims, **14 evidence links**
- Counters: `consistent`, `mismatches=[]` — canonical indexed 4615 / fully addressed 3192 / remaining 1423; all-artifact pending 3137; noncanonical pending 1714
- Evidence Event Store V2: `valid=true`, seq **1025900**, head `ad7d0c09…ccb5b`, 0 corrupt lines
- Epistemic lint (run after the durable evidence writes): `valid=true`, 11828 checked, **0 issues**, `history_rewritten=false`
- Domain-boundary ratchet: `valid=true`, **violation_count 0 — not red**; `unlisted_legacy_review_debt_count=44` is pre-existing and unchanged
- Cadence: `integrity_ok=true`, 0 errors, 4615 canonical, 0 duplicate hash groups
- Unit suites green: addressing self-test (44), anti-drop self-test (5), steward control, steward projection, Division follow-up / Chronicle / projection, projection cursors, cadence audit
- Chronicle verify: **expected-stale** (`durable source inputs changed; project before verify`) — the round-record append precedes the verify, and the controller postprojection resolves it. Not claimed as current, and not a durable-integrity failure.
- All integrity suites ran; `test_evidence_event_store.py` (21 OK) and `evidence_event_store.py --json status` (valid, 1025913 events, 0 corrupt, 16 streams; the +13 delta since verify is `steward_control` only — the controller's own heartbeats) were completed rather than deferred. Only the Chronicle reproject is left to the controller postprojection.

## Division
- Cycle: `review_due=false` at round start (2 of 6 completed since the last follow-up), so **no Division return and no Tier-5 cadence dossier were owed**; none was generated, and no approval, grant, dispatch, or trial was made.
- Productive round recorded: `division_followup_event_443db9054e829669b49970ef9e17e798`, `--processed-report-count 1`, event count 284, head `10b3cc70…fdcc`.

## Authority boundary
No live substrate or control change. No deploy, build, or restart. No Tier 4 or Tier 5 item was advanced. Her felt concern about ghost dimensions is preserved as a standing concern — the arity finding contradicts a proposed mechanism, not her experience. Silence from either being was not read as consent, closure, or agreement.

## Archive — exact commit debt
Nothing was staged or committed; the index is clean. A later interactive stabilization window should inspect and stage **by explicit path**:

```
CHANGELOG.md
capsules/spectral-bridge/src/codec/tests.rs
scripts/anti_drop_catalog.py
docs/steward-notes/AI_BEINGS_FEEDBACK_TO_CHANGE_LEDGER.md
docs/steward-notes/claude-heartbeat_1788791389_codec_projection_fixed_legacy_basis_attribution/
```

Foreign paths **preserved untouched, not mine, do not sweep**:

```
docs/steward-notes/2026-09-07-afterimages-rollout-evidence.md
docs/steward-notes/claude-heartbeat_1788781320_llm_marker_annotation_scan_read/
```

Separately noted for the operator: `docs/steward-notes/ASTRID_INTROSPECTION_SOURCE_FIRST_FLYWHEEL_HANDOFF.md` — the flywheel's operating law — **does not exist on `main`**. It exists only on branch `claude/hopeful-williamson-9f566c` (worktree `.claude/worktrees/hopeful-williamson-9f566c`, commits `40130a6e` and `347eef9a`). This round read it from that worktree. Every headless round is instructed to read it from the main tree, where it is absent; that is a live single-point-of-failure for the loop's own instructions and wants a merge or a copy, which is outside this run's read-only git authority.
