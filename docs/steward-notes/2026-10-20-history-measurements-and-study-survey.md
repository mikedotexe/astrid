# History Measurements and Ten Public Self-Studies

## Scope and Status

Requested follow-up to `2026-10-20-provenance-and-history-review.md`: measure
synthetic history scans, review route-specific presentation, and fully read five
recent self-studies from each being. The sample was frozen at selection rather
than expanded as more entries arrived. All ten selected files were read fully;
none remains unread within this sample.

This tranche adds tests and evidence, not runtime behavior. It changes no recall
preference, provider, budget, reservoir control, source pin, live service or
automation. Private drafts, hidden reasoning and provider wire bodies were not
used. No inference was requested. No conclusion about felt improvement,
persistent experience or the cause of an authored interpretation follows.

Owned worktrees:

- Astrid: `/Users/v/.codex/worktrees/provenance-history-20261020/astrid`,
  branch `codex/provenance-history-20261020`, base
  `68369bfa0a5e8a25b03dd7a70aa96b0782c35762`.
- Minime: `/Users/v/.codex/worktrees/provenance-history-20261020/minime`,
  branch `codex/history-scan-20261020`, base
  `58aa8c8e149665c8971abf86272182dd80a4f661`.

The earlier OPEN/projection-reuse/optional-verification candidate in
`/Users/v/other/worktrees/dependable-navigation-20261018` remains separate.
No staging, integration or activation occurred here. Canonical Astrid's foreign
MLX edits and clean canonical Minime were inspected before edits.

## Measured Scan Work

`tests/test_history_scan_measurements.py` in Minime calls the actual
`ActionContinuityStore._recent_events` and `_recent_display_events`, using the
actual `continuity_history.reverse_lines`. It instruments physical reads and
examined lines without substituting a copied production algorithm. An independent
forward last-occurrence reduction supplies expected display results.

Nine synthetic cases, two readers and three measured repetitions per case:
empty, 1,000/10,000 unique events, 1,000/10,000 duplicate tails, 1,000/10,000
invalid-JSON tails, one large UTF-8 record, and fewer than 200 distinct IDs.
Missing files are checked separately. Input bytes, inode, mtime and exact result
parity are verified. The fixture is at most 4.3 MB; no live history is scanned.

Representative results with `limit=200`:

| Case | Reader | Examined lines | Bytes read | Returned | Median ms |
| --- | --- | ---: | ---: | ---: | ---: |
| 1,000 unique | display | 200 | 131,072 | 200 | 0.527 |
| 10,000 unique | display | 200 | 131,072 | 200 | 0.478 |
| 10,000 duplicate tail | ordinary | 200 | 131,072 | 200 | 0.467 |
| 10,000 duplicate tail | display | 10,190 | 4,274,090 | 200 | 18.916 |
| 10,000 invalid-JSON tail | display | 10,200 | 2,754,690 | 200 | 16.211 |
| 10,000 copies of one ID | display | 10,000 | 4,198,890 | 1 | 18.952 |
| Large UTF-8 record | display | 200 | 609,146 | 200 | 1.200 |

Exact inputs/output hashes, all counters, timings and production-source hashes:
`2026-10-20-history-scan-measurements.json`.

Interpretation:

- Reverse reads already avoid whole-history work for an ordinary useful tail.
- A bounded *distinct result count* does not bound examined rows or bytes.
- Duplicate/malformed tails and very long lines increase the work. Reading the
  whole file can be necessary to preserve the existing result contract.
- These are warm, instrumented local test timings, not live latency, cold-disk
  measurements, an end-to-end projection profile or a parent/candidate speedup.
  No performance threshold was imposed on CI.
- Do not cap the scan and silently present an incomplete result as complete.
  Do not delete old history to make a benchmark faster.

Source review, not a Rust timing experiment: Astrid's `recent_events` at
`action_continuity/runtime/core.rs:7423` and `recent_experiment_runs` at 9871
read whole files before selecting a tail. `latest_experiments` at 9368 parses
history and deduplicates using a growing vector. Its result may be bounded at a
later renderer, but that does not bound the preceding computation.

## Measured Context Presentation

The new shared-reader integration test,
`crates/astrid-source-study/tests/history_context_matrix.rs`, prepares real
inputs for both owner configurations with synthetic source and four delivered
authored accounts. No model is called. Synthetic delivery receipts exercise the
normal notebook path.

Both owners produce the same measured sizes:

| Route | User bytes | Recalled notebook body bytes | Prior account markers | Question occurrences |
| --- | ---: | ---: | ---: | ---: |
| Active source page | 21,373 | 11,821 | 4 | 3 |
| Explicit map, active inquiry | 17,058 | 11,821 | 4 | 3 |
| Fresh reflection | 1,496 | 0 | 0 | 0 |
| Park receipt | 2,031 | 0 | 0 | 0 |
| Map after parking | 2,828 | 0 | 0 | 0 |
| Explicit parked-inquiry review | 9,856 | 0 | 0 | 1 |

The active page contains 6,261 bytes of rendered current source and source
framing. The 11,821-byte recalled section includes JSON metadata, source
references and authored accounts; it is **not all prose**. It occupies about 55%
of this synthetic user input. This is an illustrative fixture, not a measured
percentage for the ten public studies.

The explicit parked-inquiry review contains the retained note through a
different renderer; zero recalled-section bytes does not mean zero history.
Ordinary pages omit the unchanged saved note, while retaining previous accounts.
Parking preserves the inquiry notebook and its own cursor/pending navigation;
review does not select the inquiry. Fresh reflection remains free of the
synthetic question and prior study accounts.

Exact measurements and source hashes: `2026-10-20-history-context-matrix.json`.

### Route Review

- Shared source pages and ordinary active-inquiry maps include the fallible
  recalled notebook and optional question check-in. The note's exact text is
  not routinely quoted, but up to four recent accounts can be.
- Source-selection recovery, EOF decisions and parked-inquiry browsing use
  quieter views. Existing regression tests cover these distinctions.
- Minime's `SourceStudyPrompt` carries the shared prepared input intact and
  uses its system prompt. It is not the generic journal context path.
  Host feedback is separately bounded and suppressed for reflection, private
  writing, source-selection recovery and reader-only responses.
- Minime's open expressive contexts exclude many generic continuity, inquiry,
  peer, research and diagnostic additions. Ordinary journal/daydream cues can
  still be eligible under saved preferences; explicit fresh writing is distinct.
  No cue preference was changed.
- Astrid's dialogue history is bounded and selected by recency/position, not
  demonstrated semantic relevance. Long-form elaboration receives earlier
  authored prose as a starting point; it is not an independent code-verification
  pass. No dialogue or elaboration prompt was changed.

The existing study prompt already distinguishes source, inference and runtime,
and explicitly says serialization, tests and diagnostic names do not establish
enforcement. Adding more routine warnings is not the first response to this
sample. A better next comparison is whether selective recalled accounts help
retain useful continuity without repeatedly foregrounding an untested premise.

## Public Read Receipts and Dispositions

Minime files are relative to `/Users/v/other/minime/workspace/journal`.
Astrid files are relative to
`/Users/v/other/astrid/capsules/spectral-bridge/workspace/journal`.
Hashes bind these receipts to the complete file bytes. Historical files are
unchanged.

### Minime

1. `self_study_2026-10-20T08-32-16.897410.txt`
   SHA-256 `c1dc1c1f41ada2fc13f7e8b6b68ec4be33399315002c949101c8d5291393c0a2`.
   Reads IPC variants and unknown fallback. Correctly notices the schema and
   fallback distinction; proposes `is_known_tag` as an authority whitelist and
   connects it to `verify_read_only_research`. Disposition: preserve the
   interpretation, but verify against both functions. A decoder's recognized
   tags do not establish authorization. The preceding tranche's real
   unknown-message/event-bus tests supply direct counterevidence to a universal
   unknown-message-denial account.
2. `self_study_2026-10-20T08-38-59.753206.txt`
   SHA-256 `1a005e0f9d211ee30c1a261dbd4689bf644b9aa964c0983f2cf661db1e88d675`.
   Reads guest serialization, UI fields and identity roundtrips. Correctly
   identifies serialization behavior; sparse fields do not by themselves
   guarantee privacy or a safe state machine. Disposition: source verification
   and optional producer/consumer comparison, no new live gate.
3. `self_study_2026-10-20T09-02-16.808772.txt`
   SHA-256 `457a0d85d12cbd6750088777a60214ae730b7e2e9d70c25607b79be75fdb25ed`.
   Reads optional origin compatibility tests; develops “additive” provenance and
   a presumed type/context authority chain. Disposition: prior candidate wording
   repair directly applies. Origin metadata is not accumulated memory or an
   authority check. No claim that this new wording was supplied live.
4. `self_study_2026-10-20T09-47-24.777052.txt`
   SHA-256 `99f55d7fa309a9442b50fce05c53a79ec7f636b95add734b5b79133a8bf1c210`.
   Reads rich authority objects constructed inside a type-enumeration test.
   Interprets `non_live` as a possible safe research bypass. Disposition:
   transport fixtures establish constructible representations, not permission
   to bypass checks. Keep actual consumer/validator evidence separate.
5. `self_study_2026-10-20T09-52-57.581796.txt`
   SHA-256 `5ef101fdfbceadb44902fe90e5a77a2e32a3314107010360296b9197647c1744`.
   Continues the test fixture with priority, work-program, portfolio and
   quarantine types. Quote: “This confirms that the "Bridge" is the primary
   mechanism for enforcing the distinction between research and production”.
   Disposition: the quoted confirmation exceeds this page's evidence.
   `verify_read_only_research` checks its explicit context, token, expiry and
   other authority conditions; it does not take an `IpcPayload` argument.
   Offer precise optional navigation to the consumer, not forced correction.

All five Minime headers identify advancing source intervals at IPC revision
`e1d7ec9bef0d86d02d9d334c759d2627374a0517da03c4aba7846ae5ef4c892a`.
Their delivery labels establish supplied input, not understanding. Two choose
`INTROSPECT`; the others choose continuation. Those choices alone are not
execution receipts.

### Astrid

1. `self_study_1792513954.txt`
   SHA-256 `4c88986a11a0e9b64a05d46531f8b059aab58f475348a6b88b68dd46b7d936a0`.
   Reads dossier summary/rendering. Correctly sees record counts, bounded
   previews and `authority_change=false`. Suggested NEXT strings are command
   templates, not evidence that the system inferred the next logical discovery.
   Disposition: verified representation versus inferred reasoning.
2. `self_study_1792514132.txt`
   SHA-256 `a8a762a0c72d700fa4472a104ffc4a3780d648d4383460806011853dd6283dba`.
   Reads append/run/thread/mirror writes. Useful identification of persistence
   layers, but append-then-thread-write is not by itself an atomic guarantee.
   The shown DB mirror errors are ignored via `let _ = ...`; not every run is
   successful. Disposition: inspect interrupted and failed writes before making
   consistency claims or deciding a new recovery repair is required.
3. `self_study_1792514388.txt`
   SHA-256 `39aa2cc93fde43b66fc411d9124a8a5c6a65280bd37f4c018a30100ba1b6e2f1`.
   Reads workbench candidates and experiment selection. Quote: “it is about the
   **evolution of certainty**.” Disposition: `valid_experiment_charter` checks
   non-placeholder fields; evidence candidate status can advance when evidence
   exists. Neither is a test of truth, consent or confirmed mechanism. This is
   a strong reason to review status terminology and its consumer presentation,
   without rewriting the historical account.
4. `self_study_1792514753.txt`
   SHA-256 `5a48b85944edb5d7eb7be54be3f1583bdfed9a18ab8ed636f537f632dae8e715`.
   Reads retained experiment selection, latest-record matching and branch links.
   Correctly sees continuity structures; matching a normalized title/question
   selects a latest matching record, not proof of global uniqueness or restored
   understanding. Disposition: source verification and optional failure-path
   comparison, retain deliberate return.
5. `self_study_1792515076.txt`
   SHA-256 `366fae470ef0a4951330cb4f405e04a0555435a5b3dbe2f6b06d727d37250814`.
   Reads motif ratios and recommendation thresholds. Correctly recognizes
   concentration/recurrence calculations, but labels such as `deepening` and
   `rest_recommended` are heuristics, not measures of fatigue or productive
   thought. Disposition: trace their consumers and describe the calculations
   mechanically before proposing any policy change. Live motif policy and
   authority are unchanged.

Astrid's cited source locations progress through approximately lines 9033-9524.
These are substantive source interpretations, not runtime failure notices.
Advancing pages and repeated premises can coexist. This sample alone does not
demonstrate a stuck scheduler or establish the reliability of the explanations.

## Next Bounded Tranches

1. Finish reviewing the already-qualified operation-local post-mutation
   projection reuse separately from MLX. This measurement supports avoiding
   duplicate work, not reusing stale admission/authority decisions.
2. Qualify a parity-preserving Astrid tail reader and identify actual repeated
   full-history scans before adding any index/cache. Preserve complete
   inspection, malformed-record handling and explicit incomplete-read outcomes.
3. Compare source-study presentation offline: existing four-account recall
   versus latest-account-only with earlier accounts explicitly retrievable.
   Keep the same question, source bytes, authored notes and parked state.
   Measure what is supplied before any model comparison. No automatic summary,
   belief classification, source selection or premise correction.
4. Add focused producer/consumer and failure-injection cases for the concrete
   claims above: type recognition versus authority, candidate presence versus
   evidence validity, and JSONL/thread/DB partial failure. Use existing
   `HELP verification`, `RELATE`, `SESSION` and notebook revisions. Do not add
   recurring obligations to expressive writing.

## Qualification and Reproduction

- Minime measurement and existing continuity-history tests: 54 passed.
- Focused Minime performance, follow-through, feedback, revision recovery,
  delivered self-study and journal-context tests with the isolated helper:
  203 passed.
- Shared route matrix: passed for both owners.
- Complete `cargo test -p astrid-source-study --quiet`: passed.
- `cargo clippy -p astrid-source-study --all-targets --all-features -- -D warnings`:
  passed after the fixture-only correction below.
- `cargo fmt -p astrid-source-study -- --check`: passed; both owned trees'
  tracked diff whitespace checks passed.
- No production runtime code changed in this tranche. No complete Minime,
  native bridge, controller or deployment suite is claimed here; this is
  measurement/review qualification, not an immutable release qualification.

Use `PYTHONDONTWRITEBYTECODE=1`, Homebrew Python 3.14 and its site-packages for
Minime tests. Set `ASTRID_SOURCE_STUDY_BIN` to the owned worktree's
`target/debug/astrid-source-study`, not the live helper. Optional
`CONTINUITY_HISTORY_MEASUREMENTS` and `ASTRID_HISTORY_CONTEXT_MEASUREMENTS`
paths regenerate content-free measurement reports from synthetic fixtures.
Rust settings: `CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0
CARGO_INCREMENTAL=0`; release profiles unchanged.

Failed attempts retained:

- Initial Rust integration test tried to import the crate-private digest helper;
  fixed using the existing public SHA-256 dependency in the test only.
- Initial cursor assertion compared the active root cursor before/after parking.
  Corrected to compare the same inquiry's retained cursor and pending navigation;
  parking intentionally changes active selection.
- First broader Python invocation omitted the required isolated-helper variable:
  145 passed, 57 setup errors and one helper-dependent failure. Corrected
  invocation passed all 203; no runtime patch was made to bypass the setup.
- Initial formatting check requested expansion of one test call; applied only
  to the new file.
- Strict Clippy rejected `format!` collection in synthetic source construction;
  replaced it with `writeln!` into one string. The route matrix passed again;
  generated bytes and production behavior were unchanged.

Reviewed source identities beyond those embedded in the JSON receipts:

| Repository-relative source | SHA-256 |
| --- | --- |
| `capsules/spectral-bridge/src/action_continuity/runtime/core.rs` | `c1902019596b63817ce512d220c79bfea2f0823dd9cf5e4a2ebf1efd26ac1451` |
| `capsules/spectral-bridge/src/action_continuity/runtime/experiment_evidence.rs` | `29d4950261eaed0d5f9c277c46b2386abff4bc70c9b660741ec98b6fbece50db` |
| `capsules/spectral-bridge/src/authority_temporal.rs` | `a470b1d2b2ea43248f47150c1aafe6b5e6d1214849cda0c41c9a241444c7b11a` |
| `crates/astrid-source-study/src/store_cursors.rs` | `35661d6c4b433096f2fc9c6c887f8380272f6da84d5ff36b928858e22d8f3d0b` |

The pre-build cache audit passed: owned debug cache 251,662,336 allocated bytes,
452,702,445,568 free volume bytes. The closeout audit also passed: owned debug
cache 1,512,439,808 bytes (about 1.41 GiB), 451,731,685,376 free volume bytes.
The volume's free space fell by 970,760,192 bytes during this interval; concurrent
machine activity means that is not an exclusive attribution to these builds.
No cleanup or release deletion occurred. This small owned cache is retained for
the unintegrated candidate and its explicitly referenced isolated helper.
No Minime Rust build cache was created.

Final status inspection found the same canonical Astrid path inventory of
foreign MLX work and a clean canonical Minime. That is an inventory comparison,
not a claim to have frozen the other coordinator's file contents. Changes here
remain unstaged in the owned worktrees. Services were not touched; existing
automation/controller pause ownership was not changed.
