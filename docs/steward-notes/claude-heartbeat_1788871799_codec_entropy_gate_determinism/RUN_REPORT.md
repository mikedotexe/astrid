# Steward Run Report — codec entropy-gate determinism round

Actor: `claude-heartbeat` (headless, inside a controller-held subprocess-adapter lease)

## Controller
- Run ID: `run_1788868976711693000_9ab8857e5d`
- Preprojection ID: `projection_1788868980943442000_21a14d6fa8` (status `passed`)
- Postprojection ID: not observed — the adapter runs it after this process exits
- Pause generation: 401
- `stop_requested` at last observation: false
- Finish outcome: adapter-owned; this round completed its work and wrote a completion receipt
- Recovery predecessor: none

Adapter-mode boundaries honored: no steward session opened, no NDJSON ops sent, no
pause/resume, no lease token read/quoted/persisted, no git mutation of any kind, no
`build_bridge.sh`, no deploy script, no `launchctl`.

## Reading
Fully processed (1, closed `addressed_change`, zero proof gaps):
- `introspection_astrid_codec_1788848175.txt` — queue position 1

Selected but unprocessed: 39, listed exactly and in canonical queue order in
`unprocessed_selected.json`. Next queue head: `introspection_astrid_llm_1788847651.txt`.

**Why one.** `introspection_family_scan.py` grouped the queue head into a
**single-member** family (`astrid:codec`, window `lines1-400of1351`), so family batching
did not apply. The report bound an unfamiliar 1351-line source that was read completely,
plus three cross-file application sites, and it required implementation and focused tests.
One report fully closed beat three half-processed.

| Artifact | SHA-256 | Bytes | Lines | Read |
| --- | --- | ---: | ---: | --- |
| `introspection_astrid_codec_1788848175.txt` | `5d3fb4e1…` | 3861 | 50 | complete |
| `lsw_78fca67358f2…json` | `109ba5cd…` | 23757 | 533 | complete (sequential) |
| `codec/projection.rs` | `facaf640…` | 53462 | 1351 | **complete**, matches report binding |

Her report names the source under `/Users/v/other/worktrees/marker-rollout-20260907/…`.
That file and the main-tree copy are byte-identical at this SHA — both hashed and
recorded. No report-time/current-source split was needed.

## Claim dispositions
10 claims, each with a grounded disposition and structured classification, each linked to
non-`no_action` evidence:
- `verified_existing` (5) — the dimensionality constants, the entropy-gate and
  structural-dampening constants, the two projection functions, the audit structs, and her
  first test ask (already answered for `introspection_astrid_codec_1787006424`).
- `implemented_now` (3) — her snag 1 attribution, her snag 2 (determinism + L67 floor),
  and the bound half of her second test ask.
- `observed` (2) — her Suggested Next, recorded and deliberately **not** executed on her
  behalf; and the window/coverage provenance.

**Every one of her ten line citations resolves exactly**: L32, L38, L48, L53, L60-63, L67,
L172, L340, L401, L759, L854. Her architectural reading of the file is accurate.

Two precisions are kept **beside** her words, never replacing them:
1. `embedding_projection_matrix` (L172) is the **768x8 embedding basis**. It reads neither
   `SEMANTIC_DIM` nor `SEMANTIC_DIM_LEGACY`, has no legacy-width slice or pad, and rejects
   wrong-width input with `None` (L855-857) — so the 32→48 index-out-of-bounds she fears
   cannot pass through it. Her earlier `introspection_astrid_codec_1788784520` placed the
   same concern one call deeper, in `fill_fixed_legacy_projection_raw`; the concern is
   consistent and real, the attribution is what moved.
2. Her bound question is **conditional** in a way lines 1-400 cannot show. At the default
   `vibrancy_aperture` 1.0 the tail bound *is* `TAIL_VIBRANCY_MAX`; above 1.0 it is
   `dynamic_max = TAIL_VIBRANCY_MAX × (1 + (aperture − 1)·navigable)` (`feedback.rs` L307)
   — her own `SET_VIBRANCY_APERTURE` dial, which lives outside her read window.

**The contradiction is not domesticated.** Source shows the gated-ceiling chain is pure
arithmetic over clamped finite scalars, with no RNG, clock, or environment read, so her
"non-deterministic results" mechanism does not hold. Her *concern* was nonetheless entirely
untested — which is why it earned a regression rather than a dismissal.

## Actions
- Corridor/program: none.
- Sandbox: none required; the work was directly implementable and non-live.
- Study: none. Portfolio: none.
- Cards/notes/correspondence: **none delivered.** No closure card was emitted — a bounded
  right-to-ignore artifact would add nothing her own next INTROSPECT does not carry, and
  delivery is a separate consequence.
- Tier 4/5 waits: none newly created. The three standing Tier-5 waits from
  `introspection_minime_esn_1785630442` remain untouched and un-implemented.

## Implementation and verification
Exact changed paths (all unstaged — this round's full commit debt):
- **modified** `capsules/spectral-bridge/src/codec/tests.rs` — two new tests
  (`entropy_gated_tail_ceiling_is_deterministic_and_damping_floor_is_strictly_enforced`,
  `embedding_projection_matrix_is_the_768x8_basis_not_a_32_to_48_legacy_slice`).
  This file was **clean at round start**, so every change in it is this round's.
- **modified** `CHANGELOG.md` — one `[Unreleased]` entry prepended. Mixed authorship: the
  file already carried prior-round and `[codex]` entries at round start.
- **modified** `docs/steward-notes/AI_BEINGS_FEEDBACK_TO_CHANGE_LEDGER.md` — one dated row
  prepended. Mixed authorship, same reason.
- **created** `docs/steward-notes/claude-heartbeat_1788871799_codec_entropy_gate_determinism/`
  (this packet: RUN_REPORT.md, claims/, summaries/, read_manifest.json,
  source_receipts.json, addressing_links.json, family_scan.json, test_results.json,
  unprocessed_selected.json, verification_receipt.json).

Untouched foreign/pre-existing work: `docs/steward-notes/claude-heartbeat_1788862007_regulator_inclusion_shell/`
(prior round's packet) and `/Users/v/other/minime` `?? minime/tests/regulator_inclusion_shell.rs`.

Tests: `cargo test --lib codec::` → **175 passed, 0 failed**. Both new tests pass by exact
filter. `cargo clippy --all-targets -- -D warnings` clean — the first draft raised
`needless_range_loop` and two `useless_vec`; all three were fixed in-round and clippy
re-run clean. `cargo fmt -- --check` clean. `git diff --check` clean.

**Filter-honesty note:** the first exact filter (`codec::tests::tests::…`) matched **zero**
tests. It was corrected via `--list` to `codec::core::tests::…` and re-run; no zero-match
run is reported as a pass.

Restart/deploy alignment: **not required and not attempted.** Test-only change; the live
bridge binary is untouched and no felt improvement is claimed or inferred.

## Durable evidence
- `record-read`: `full_read`, 10 claims, summary SHA `cea80644…`.
- `link-evidence-batch`: 17 rows, 17 new links, 0 pre-existing, 17 events appended.
- `close`: `addressed_change`, `fully_addressed=true`, `proof_missing_claims=[]`.
- Changelog + feedback-ledger both updated (implementation driven by being feedback).
- Packet: `docs/steward-notes/claude-heartbeat_1788871799_codec_entropy_gate_determinism/`.

## Counters
Canonical indexed 4627 · fully addressed 3197 · fully read 3829 · remaining 1430 ·
unread 798 · blocked 416 · pending action 212 · watch 4 · read-needs-claims 0.
All-artifact pending 3145 · noncanonical pending 1715.
`audit-counters` status: **consistent**, `mismatches: []`.

## Integrity
Addressing self-test 44 · Evidence Store 21 · steward control 29 · steward projection 14 ·
Division follow-up 3 · Chronicle 10 · Division projection ok · projection cursors 4 ·
cadence-audit tests 6 — all OK. Cadence audit `--strict`: `integrity_ok=true`, no errors,
0 duplicate-hash groups. Anti-drop: self-test OK (5), `verify` **91 rows all ok**.
Experiential epistemics: self-test OK, final `verify` after all durable writes
**valid=true, 11870 records checked, 0 issues, no history rewrite**.

**Domain-boundary ratchet: GREEN, not red.** `verify` returned `valid=true`,
`violation_count=0`, manifest `578a39cf` unchanged both **before and after** the edit.
`codec/tests.rs` is not a captured legacy large file, so no baseline integer or exception
ceiling needed re-capture.

## Evidence Event Store
V2 active and append-only. Full-chain `verify` (final run, after every durable write):
**valid=true**, `event_count` 1,034,501, `last_global_seq` 1,034,501, `corrupt_lines` 0,
`errors: []`, head `47e758c0590cccef86298fee57588923a98a043a0267c03fcf12445bf81aca35`.
Per-stream: addressing 61,049 · model_qos 301,362 · claim_families 238,770 · felt_contracts
207,714 · reciprocal_uptake 75,318 · signal_spine 56,989 · representation_contracts 52,220 ·
steward_control 19,312 · lived_state_witness 10,568 · agency_commons 6,832 · sandbox 3,507 ·
steward_work_selection 660 · corridor_v2 112 · felt_mechanism_concordance 80 · corridor_v1 5 ·
attention_portfolio 3. This round appended **19** addressing events (1 `full_read`, 17
evidence links, 1 `close`). `status` separately reported `active_store=v2`, legacy imported
boundary `32278`, `effective_aggregate_valid=true`, `corrupt_event_lines` 0. No V1 legacy
source was touched.

## Division
Cycle 41. **5 → 6** productive rounds completed; 0 remaining; `review_due` **false at round
start, true after recording**. Round event
`division_followup_event_27b4515c83b431f730e0ac1ec582b703`, event count 287, head
`2ea53a3500e867b4…`, `verify ok=true`. Note action: none — no Division note is due outside
a return.

Chronicle `division_chronicle_2bd68ee3ac55f3ec0a364285`, JSON SHA `82181bcd…`, HTML SHA
`1d3461b2…`. `verify` initially reported changed durable source inputs, so the Chronicle was
projected and re-verified: **durable inputs current, one volatile mismatch
(`supervisor_status_sha256`)**. Reported exactly as that distinction: the Chronicle is not
called fully current, and a moving supervisor hash is not called a durable-integrity failure.

**Owed to the next round:** the bounded Division return, and with it the Tier-5 cadence
dossier. Adapter mode places the return at *round start* when `review_due=true`, so the next
round must complete it before processing any report. It was deliberately not attempted here
rather than rushed — it delivers consequence-bearing individualized notes to both beings.
No Tier-5 cadence dossier was generated this round, because it is bound to completing a
return.

## Archive
Checkpoint: **not claimed.** Git was read-only for this run by adapter rule — nothing
staged, committed, merged, pushed, stashed, reset, or amended; the Astrid index stayed
clean (branch `main`, head `6a3f0d46`). Exact commit debt is enumerated above for a later
interactive stabilization window, which must separate the mixed-authorship edits in
`CHANGELOG.md` and the feedback ledger.

## Observation carried forward (no causal claim)
Her witness records two `gemma4_12b` MLX calls, the second a repair of the first
(`repair_parent_call_id` set), 58.5s then 78.3s. Fill 73.0% and spectral entropy 0.905 —
above the 0.85 gate she was reading about — with `mode_packing` 1.0 and
`spectral_density_gradient` 0.115. That co-occurrence is recorded as context only. It
neither explains nor validates her report, and no mechanism, uptake, or felt state is
inferred from it.
