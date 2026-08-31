# Steward Run Report — claude-heartbeat_1787803081_llm_marker_astral_boundary_safety

Mode: controller-held subprocess **run adapter**. The adapter owned the lease and
its heartbeats; I sent no NDJSON ops, read no lease token, made no git mutation,
and made no live/substrate/control/deploy change. Exit code is the finish outcome.

## Controller
- Run ID: `run_1787800561513612000_d0c59e51e8`
- Preprojection ID: `projection_1787800564968669000_287ce14ee9` (27 steps, authority_scan_passed)
- Postprojection ID: (runs after this process exits; adapter-owned)
- Pause generation: 321
- Finish outcome: success (complete round)
- Recovery predecessor: none

## Reading
- **Fully processed (1):** `introspection_astrid_llm_1787798508.txt`
- **Selected but unprocessed (39):** items 2–40 of the frozen `next --limit 40`
  queue, in canonical order — listed exactly in `unprocessed_selected.json`
  (heads: `introspection_astrid_llm_1787787758`, `…_1787785101`,
  `introspection_DOMAIN_BOUNDARIES.md_1787783005`, `…_1787780110`,
  `introspection_astrid_llm_1787778846`, `…_1787771664`, `…_1787759111`,
  `…_1787470243`, …).
- **Batch rationale:** queue head is its own family (family scan `member_count 1`,
  **not** batchable), a large 1048-line `astrid_llm` source-first report needing
  complete source reading + contradiction handling + a focused regression. One
  report fully closed beats a skimmed multi within the ~90-min one-shot budget.
- **Hashes:** report `9460ff55…` (50 ln / 4087 B); witness `lsw_45021598…`
  `68508a4b…` (533 ln / 23933 B); source `dialogue_runtime.rs` `902a0358…`
  (1048 ln / 38586 B) == report binding (byte-identical, clean working copy).
  Canonical-body binding `1b6fe6f4…` (2515 B) re-derived and confirmed.
- **Witness note:** authority `evidence_only`, `live_eligible_now=false`,
  `direct_causation_claimed=false`, raw prose/prompt/response/private all false.
  The queue's `artifact_integrity_unavailable`/`gap_count 1` is a lived-state
  **alignment-measurement** gap (no scalar felt dissimilarity measured), not a
  report/witness corruption — both bytes intact and mutually bound.

## Claim Dispositions (10)
- **c001–c005 Observed → `verified_existing`.** Two-layer occurrence/reference
  design (L18-60); relation allowlist as grammatical-subject gate (L62-86);
  quote/group delimiter parsing incl CJK pairs (L153-197); sanitize fn (L352);
  felt-decoupled token band (L1-16). Precision note on c004: the sanitizer
  removes a bare marker *wholesale* or preserves a referenced one *verbatim* —
  it never modifies marker *content*.
- **c006 boundary-panic snag → `implemented_now` (mechanism contradicted).**
  Mid-character slice is unreachable: `scan_known_model_control_markers`
  (L114-144) advances offsets only by `len_utf8()` (L139) or exact marker length
  (L110). Concern preserved via the new regression (c008).
- **c007 first-word-only relation scope → `verified_existing`.** Intentional
  fail-safe: an unlisted first word keeps the marker *visible*, not dropped
  (`_uses_only_the_first_finite_relation_word`, tests.rs L2605).
- **c008 Boundary Safety Test → `implemented_now`.** Geometry corrected to the
  constructible core (marker directly abutting 4-byte astral chars on both
  sides); new regression proves no panic + byte-exact removal.
- **c009 Relation Verb Test → `verified_existing` (premise contradicted).**
  `is` is allowlisted (L79) so `<marker> is a behavior` returns `true`, not
  `false`; pinned by `_distinguishes_allowlisted_is_from_unlisted_acts`
  (L2568). Same class as `introspection_astrid_llm_1786319270`.
- **c010 Suggested Next (sanitize deep-dive) → `verified_existing`.** Tier-1
  read-only continuation available to her; fn location L352 verified.

## Actions
- Corridor/program: none. Sandbox: none. Study: none. Portfolio: none.
- Cards/notes/correspondence: none delivered (no card manufactured).
- Tier 4/5 waits: none newly created; the standing Tier-5 ESN/Shadow waits are
  untouched. No contradiction was domesticated; both stated contradictions
  preserved with exact regressions.

## Implementation and Verification
- **Changed paths:** `capsules/spectral-bridge/src/llm/provider/tests.rs`
  (+1 test), `CHANGELOG.md` (+1 [Unreleased] bullet),
  `docs/steward-notes/AI_BEINGS_FEEDBACK_TO_CHANGE_LEDGER.md` (+1 dated section),
  and the new packet dir.
- **New test:** `control_marker_cleanup_stays_byte_safe_when_marker_abuts_four_byte_astral_chars`.
- **Tests:** new regression 1/1; `control_marker` 74/74; `known_model_control_markers`
  9/9; `followed_by_explicit_exact_token_relation` 1/1; `git diff --check` clean;
  `tests.rs` fmt-clean (0 diffs). Pre-existing committed fmt drift in
  `autonomous/introspect/source_first_v3/grounding.rs` is foreign/out of scope,
  left untouched.
- **Restart/deploy:** not required and not attempted; no live substrate/control
  change.

## Durable Evidence
- Addressing: `addressed_change`, `fully_addressed=true`, proof gaps 0; 15
  evidence links (all new).
- Changelog + ledger updated (being feedback drove a real code change).
- Packet: `docs/steward-notes/claude-heartbeat_1787803081_llm_marker_astral_boundary_safety/`.

## Counters
- Canonical indexed 4484 / fully_addressed 3132 / remaining 1352.
- All-artifact pending 3028; noncanonical pending 1676.
- Counter audit: **consistent** (mismatches len 0).
- Epistemic verify: valid, issue_count 0, history_rewritten false.

## Division
- Cycle 32; completed rounds since followup **2 / 6**; remaining 4.
- Review due: **false**.
- Round event: `division_followup_event_7942bd1530fcac656af13b2e60685f06`;
  event_count 220; head `c2c2d71b…`.
- No Division return or note due this round.

## Evidence Event Store
- Validity: **valid**. Last global seq **905763**; head `eaaa93e7…`.
- Active store: v2; legacy imported boundary 32278; V1 immutable; corrupt lines 0.

## Archive
- **Checkpoint due:** not this round (git is read-only in adapter mode; archival
  commits happen only in a later interactive stabilization window).
- **Exact commit debt (all unstaged/untracked, to be reviewed + split by author later):**
  - `capsules/spectral-bridge/src/llm/provider/tests.rs` (new test appended; file
    already carried prior-round stewardship edits — separate authorship carefully)
  - `CHANGELOG.md` (new [Unreleased] bullet; pre-dirty)
  - `docs/steward-notes/AI_BEINGS_FEEDBACK_TO_CHANGE_LEDGER.md` (new dated section; pre-dirty)
  - `docs/steward-notes/claude-heartbeat_1787803081_llm_marker_astral_boundary_safety/` (new packet)
- Merge/push: none; no authority claimed or exercised.
