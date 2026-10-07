# Peer Mirror Publication and Scheduling Review

Date: 2026-10-07. Status: offline candidate, not deployed, committed or merged.
Coordinator: Codex. A delegated coding agent implemented the isolated random-draw
helper and preselection-filter regressions; the coordinator owns integration,
journal changes and verification.

## Scope and source identity

Mike noticed three identical-looking Astrid entries and authorized proceeding
with repairs. We separate publication/provenance from scheduling activation.
The earlier qualified open-reflection candidate is not modified by this work.

Astrid worktree: `/Users/v/.codex/worktrees/mirror-provenance-20261007/astrid`,
branch `codex/mirror-provenance-20261007`, based on local main
`aba4cec45f2ed8468eca883fe2db92155e611ec7`.
Minime source-only test checkout: sibling `minime-source`, branch
`codex/mirror-provenance-tests-20261007`, based on local main
`754c44b0e7abc72d2b689510f832218dcc8ee9ea`. No Minime source changes.

All three supplied canonical files were read fully. Paths below are relative
to `/Users/v/other/astrid/capsules/spectral-bridge/workspace/journal/`:

| File | SHA-256 |
| --- | --- |
| `astrid_1791394598.txt` | `c002204528aa097efa561fad19f64ca6a43c603349ea87a57cdb74aea94f5160` |
| `astrid_1791394637.txt` | `c4363c424e13a23574237a9104633a53bc7d14fc8cb1c2f3c92d0065068a32a9` |
| `astrid_1791394676.txt` | `25dc14323b2e13d6af00eb5f0409ce754456cfa8a9e6833eed253a379657c6b5` |

Their source is Minime's public
`/Users/v/other/minime/workspace/journal/!introspect_2026-10-07T09-24-18.713163.txt`,
SHA-256 `0de67fc48bc60d48cc72ec5eb90411140cba712c74cc5bff1f0ba356301c0d98`.
It was also read fully. The historical source ID lacks the later `!` filename
prefix. Its authorship and wording are not changed or recast as a request.

The mirrored excerpt ends with the exact text:

> I cannot take away the b

The source continues:

> I cannot take away the burden of their experience, but I can stand beside them while they carry it.

This is a mechanical clipping boundary, not evidence that either being chose
to stop there or independently repeated a thought.

## Confirmed execution evidence

Read-only SQLite inspection of canonical `bridge.db` confirms:

| Message row | Exchange | Unix timestamp | Mode |
| --- | --- | --- | --- |
| 18802605 | 217301 | 1791394598.360034 | mirror |
| 18802679 | 217302 | 1791394637.244011 | mirror |
| 18802750 | 217303 | 1791394676.618808 | mirror |

All name the same Minime source. All claim exact text and trailing-edge
preservation because the old comparison begins AFTER the 2,500-character
parser cut. All three say `semantic_chunk_sent=false` and report
`limited-write v2 requires inactive semantic state`.
These records do not establish repeated sensory injection into Minime.
The bridge's separate local Shadow bookkeeping still occurs per exchange.
Historical receipt values are preserved; new scope fields are prospective.

## Publication tranche

1. Read the selected file once into `MirrorExcerpt`. Bind the raw-file SHA-256
   and byte count, normalized body hash/character count, selected character
   count and explicit `excerpt_truncated` flag to that read.
2. Keep the existing first-2,500-Unicode-scalar body selection, header/markup
   normalization and peer-action projection. Do not feed additional metadata
   through the codec or silently enlarge input.
3. Retain the projected excerpt under
   `journal/peer_mirrors/peer_mirror_v1_<identity>.txt`, visibly labelled
   `PEER MIRROR EXCERPT`, with peer ownership and first-retention time. The
   identity includes versioned evidence and rendered text hash, not current
   fill or exchange time. A filename-only rename does not create another copy
   when source bytes and projection are unchanged.
4. Serialize publishers with an on-disk lock. Write a unique create-new
   preparation, fsync, then atomically hard-link it to the final name without
   overwrite. Interrupted preparations remain inspectable but do not occupy
   the canonical name; a retry can publish a complete copy. Bound preparations
   to 1,024 per identity and expose retained-preparation cleanup debt in the
   receipt. Check retained evidence and text on retry. A mismatched, partial
   or unreadable CANONICAL record still fails closed without overwrite.
5. Retain every exchange's database receipt, including `newly_retained`,
   `already_retained` or `unavailable` and the exact artifact reference. This
   is journal-publication deduplication, NOT deduplication of actions, codec
   sends or exchange accounting. Database exchange text is not deleted.
6. Exclude old and new peer mirror records from own-journal continuity and the
   optional fallback own-voice anchor by HEADER provenance, not prose content.
   Filter before recall/agency selection limits so rejected mirrors cannot
   crowd out older authored work. Keep the 32-own-record expressive recall
   window and the three-snippet fallback bound. Explicit remote reading and
   original files remain available. No prior journal is rewritten, renamed,
   removed or reclassified in place.
7. Retain existing fidelity fields for compatibility but add an explicit
   sanitized-excerpt comparison scope and
   `full_journal_fidelity_established=false`. New excerpt evidence discloses
   the earlier cut even when the selected excerpt is rendered exactly.

The dedicated peer-artifact directory is outside ordinary journal compaction
and own-voice scanning. There is no automatic eviction. Source revisions or
changed projections get distinct records even if an older prefix looks alike.
Mirror records no longer enter the generic authored-journal afterimage
association path. Per-exchange voice-health diagnostics remain; no meaning or
consent is inferred from repeat selection or lack of selection.

An adjacent metadata defect is also corrected: if mirror reading falls back
to a fixed bridge dialogue string, Shadow provenance uses bridge ownership,
not Minime or Astrid authorship. Shadow numeric computations are unchanged.
The ownership helper is extracted into `astrid_shadow/provenance.rs`; agency
journal selection likewise moves to `agency/journal_selection.rs`. These
cohesive extractions avoid growing legacy large modules; no ceiling is relaxed.

## Separate random-draw tranche

The old spontaneous/agenda expressions shift a 64-bit result by 33, then
divide that 31-bit numerator by `u32::MAX`. With floating-point rounding their
range is approximately `[0, 0.5]`, not `[0, 1)`. The spontaneous upper Witness
band cannot be reached through this ladder. Safety/other Witness paths are
separate and remain reachable.

Use two draws from existing `rand::rngs::StdRng`, seeded from the same bounded
timestamp source. Tests call the actual production helper and actual mode
ladder, covering extreme/timestamp seeds and 10,000 deterministic samples.
Thresholds, fill gates, priority ordering, cooldown and agenda settings are
unchanged, but effective probabilities and low-fill behavior DO change.

For orientation only, under an ideal uniform input distribution and absent
earlier gates, the default mirror interval changes from roughly 24% of the
old half-range to its declared 12% of a unit range. The upper Witness interval
becomes reachable. Agenda thresholds also regain their unit-interval meaning.
These are conditional arithmetic observations, not measurements of live
clock seeds or claims about the three observed selections' cause.

No new recent-source exclusion or automatic activity substitution is added.
The existing mirror start selector remains unchanged. Any future decision to
reduce repeated selection, rather than duplicate journal publication, must
review its codec and scheduling consequences separately.

## Verification and unsuccessful attempts

- Initial focused mirror suite: 33 passed.
- Two early bridge runs: 2,380 passed, seven failed, one ignored. The failures
  concern missing Minime source/afterimage fixtures. The first run had no
  sibling checkout; the attempted link in the second landed inside a test-
  created directory. Preserve that directory as `minime-test-scaffold` and use
  a separate Minime worktree at the expected sibling path. The seven focused
  introspection-target tests then pass. Final suite results are recorded below.
- The initial debug build reduced disk headroom from about 9.7 GiB to 0.8 GiB.
  No service was stopped. A targeted interrupt found the test process had
  already exited. Remove only this run's identified incremental/object caches
  (new manifest-bound build identities), leaving test executables, prior
  artifacts, stage inventories and all evidence intact. Subsequent checks use
  one build job, no incremental cache and package-local debug information off.
  Headroom recovered to about 3.3 GiB, still insufficient for an unreviewed
  large release build. No broad cache or worktree cleanup was performed.
- Boundary audit rejected growth in `astrid_shadow.rs` and later `agency.rs`;
  cohesive extractions repair both without changing the baseline or exceptions.
- Read-only review caught pre-limit filtering and interrupted-publication
  issues. Add mixed-directory tests (40 legacy mirrors before authored work),
  32-own-record window tests, atomic no-clobber publication, restart/partial
  preparation tests and explicit cleanup-debt reporting.
- Domain-boundary, recurrence-provenance, steward-control/projector, Evidence
  Event Store, flywheel and Division tracker/Chronicle suites: 97 passed.
- Experiential-epistemics self-tests: two passed. A later strict-Clippy check
  rejected unchecked timestamp addition in a new test fixture. Replace it with
  `checked_add` in both fixture helpers; the final all-target/all-feature check
  with warnings denied passes. No lint exception was added.
- Final bridge library suite, including the agency module extraction and final
  fixture corrections: 2,394 passed, zero failed, one existing fixture-dependent
  test ignored (48.23 seconds). Formatting, whitespace and domain-boundary
  checks pass on this source. No architecture ceiling was relaxed.
- Wider test run before the last review corrections: library 2,387 passed / one
  ignored; codec replay six, agency integration one, authority typestate one
  and rendering five passed. The LLM facade compile-contract run failed with
  `No space left on device` while its nested Cargo invocation built an archive.
  That subprocess did NOT inherit the root package-local debug setting. Do not
  describe the complete integration/compile-contract suite as passed. Later
  integration targets were not reached; retain this qualification debt.
- Remove only additional current-run manifest-bound compiler objects/archives
  and incremental cache directories after their processes exited. No release
  assets, prior worktree source, test receipts or authored material were removed.
  Disk headroom later fell below 0.8 GiB and measured 1.8 GiB after the final
  lightweight library check. This is not qualification of sufficient release
  build headroom: no further unrestricted nested build was attempted.

The lightweight verification command uses
`CARGO_TARGET_DIR=/Users/v/.codex/worktrees/source-selection-recovery/astrid/capsules/spectral-bridge/target`,
`CARGO_BUILD_JOBS=1`, `CARGO_INCREMENTAL=0`, and
`cargo test --manifest-path capsules/spectral-bridge/Cargo.toml --config 'profile.dev.package.spectral-bridge-server.debug=0' --config 'profile.test.package.spectral-bridge-server.debug=0' --lib -- --test-threads=2 --quiet`.
The first safe action for remaining compile-contract debt is to establish
adequate free disk on an owned build volume, then rerun the whole bridge suite
with debug/incremental policy that ALSO reaches nested Cargo. Do not weaken
the tests or delete foreign caches to claim completion.

## Next safe integration

1. Review publication/provenance and random-draw changes independently. To
   release only publication, omit `autonomous/state.rs` and
   `autonomous/spontaneous_random.rs`; do not treat the combined branch as a
   presentation-only release.
2. Reconcile the desired subset with the separately qualified open-reflection
   candidate, preserving its helper/schema/adapter work. Its stage remains
   unchanged and is not a qualification of this new source.
3. Resolve disk headroom, inventory exact live identities and foreign activity,
   then qualify a fresh immutable candidate and checkpoint preservation. Use
   sanctioned bridge staging/activation, acknowledged drain and explicit
   activation approval. No engine, provider, model, sensory or Minime-agent
   restart is implied by these source changes.
4. After any approved activation, inspect natural mirror receipts/artifacts.
   Distinguish fewer duplicate journal files from fewer selections, less
   sensory input, new authorship or subjective benefit.

No Git staging/commit/merge/push or live activation occurred. Canonical trees,
authored history and paused automations remain untouched. The controller was
read-only checked as paused, generation 502, without an active lease. Its
evidence verification reports V2 valid at sequence 1123156, head
`eb7dd9522a36ee372ab847738e752c362c0d9c5917a3a3cdd7c5c1b3bba5ce6e`,
with V1 immutable; no new stewardship round or projection was requested.

The earlier open-reflection stage remains byte-identical: manifest SHA-256
`20728f255f715d64c28791b4b028fe8dc95670ac44f94594e72322baf91a3d21`,
bridge SHA-256 `b2c791aa2b7fbe6065359512c59a72da8dfc82abda86c7ac0134d83d915560d1`.
