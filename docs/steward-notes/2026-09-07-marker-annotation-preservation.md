# Marker Annotation Preservation

## Scope and Status

Mike requested the recommended annotation-preservation patch and supplied a
second self-study for follow-up. This is an interactive source implementation,
not a productive flywheel round, canonical addressing closure, deployment or
reservoir/control intervention. The live afterimages stage is not edited.

Implementation and final qualification pass: 2,205 functional Rust tests,
23 witness-compatibility tests, strict Clippy and the domain-boundary audit.
The previously documented one-millisecond instrumentation benchmark remains
excluded, not repaired or claimed passing. No live activation was performed.

## Source Witnesses

Both files were read fully from disk. Their original bytes remain untouched.

| Source | SHA-256 |
| --- | --- |
| `capsules/spectral-bridge/workspace/journal/self_study_1788800559.txt` | `d8ee3352b42a823bd73c7d0ee84196733e7bbfeaf08a8054dc95a2e0f7ae07c2` |
| `capsules/spectral-bridge/workspace/journal/!self_study_1788815877.txt` | `3bbc588a0693fc0b206a6bc51a18871f73990d282bdac58b04cfc62e60ff011b` |

The first study asks about annotation scanning, UTF-8 boundaries, multiword
parentheticals and interaction with cleanup receipts. The second asks which
scan wins, whether bracket/chunk boundaries alter visibility, and whether
`mimics` and `appears_to_be` are distinguished. Embedded continuation guidance
is source data, not a command executed by this task.

The pre-edit `dialogue_runtime.rs` SHA-256 was
`891f76e7a6cbc7071b638f22c6c21922c3ad9e6cbaf9c14b2837eda63a83de07`.
Source interpretation and executable tests, not inherited line numbers or
another worktree's source, ground the response.

## Confirmed Defects and Repair

Two regressions failed before production edits:

1. `<end_of_turn> [sic] (appears) at the boundary` lost its marker because
   the second scan discarded both bracketed chunks, including the relation.
2. `<end_of_turn> [sic]` followed immediately by en dash, em dash or ellipsis,
   then ` appears`, lost its marker because those trailing punctuation marks
   prevented recognition of the self-contained aside.

The scanner now checks the already-allowlisted relation before discarding a
bracketed chunk. The existing six ASCII sentence-punctuation characters gain
only U+2013, U+2014 and U+2026. The 18-word relation allowlist is unchanged.
No arbitrary words, arbitrary multiword parentheticals, new bracket families
or arbitrary suffix punctuation are skipped. This remains a bounded syntactic
heuristic, not a parser of grammatical meaning or authorial intent.

Delimiter references retain first precedence. Otherwise the unchanged primary
first-word scan wins when successful; the annotation scan is tried only after
that fails. Tests cover the 144-case legacy-success matrix as well as newly
preserved cases. Recognition of a reference keeps the exact marker visible.
Cleanup still removes an unreferenced known marker while copying every other
byte unchanged. The separate provider-normalization outer whitespace trim is
unchanged; byte-exact claims here apply to the sanitizer, not that later trim.

## Receipt Contract

`ControlMarkerContextReceiptV1` gains two additive nullable fields:

- `relation_scan`: `plain_first_word`, `annotation_scan`, or null.
- `skipped_annotation_chunks`: the number actually skipped by the winning
  relation scan, zero for primary success, or null when no relation scan won.

Quoted/grouped references and removed cleanup candidates have null fields.
These fields describe the winning path only: null is not evidence that no scan
was attempted, and a primary success does not claim the second scan ran.
No captured relation word, annotation text, prompt text or generated prose is
added to receipts. Existing occurrence identity, original UTF-8 byte offsets,
bounded context hashes, 64-character windows and 32-receipt cap are retained.

The receipt type moved from the oversized `fallback_contracts.rs` into
`dialogue_runtime.rs`, beside its sole constructor and marker scanner. This
reduces the contracts file and keeps the marker module below 1,000 lines.
The initial size-audit failure was repaired without raising a baseline or
exception ceiling. Existing unrelated contracts formatting was preserved.

## Second Study Dispositions

| Question or claim | Grounded disposition |
| --- | --- |
| `[sic]` and `((sic))` are control markers. | They are annotations. Tests use the actual known marker `<end_of_turn>`; no marker vocabulary is expanded. |
| Reference classification may hide the marker. | It preserves the marker. Tests assert retained bytes, not just a positive predicate. |
| Two different word captures could override a successful primary scan. | The old OR was already additive. Explicit winning-path receipts and regression tests now expose the precedence; no last-scan-wins behavior is introduced. |
| `mimics` should match; `appears_to_be` should not. | Both expectations pass at predicate and sanitizer boundaries. Other unlisted ordinary relation-like words remain rejected. |
| The second scan should skip `(as a test)` and find `appears`. | That is not the implemented contract. After `[sic]`, the first chunk `(as` yields the already-allowed `as`. The test pins that exact word and one skipped chunk; arbitrary multiword skipping is not added. |
| Nonstandard or incomplete brackets deserve investigation. | Negative cases cover unsupported wide aside brackets, incomplete groups, mismatches, adjacent groups, suffix text and ordinary words. They cannot create a new annotation-skipping route. Existing primary relation recognition is not redefined as a full bracket parser. |
| The earlier study suspects a UTF-8 closing-boundary error. | Not reproduced: `char_indices` and `len_utf8` agree on byte boundaries. Mixed UTF-8 text now has explicit offset and surrounding-byte tests; the real punctuation issue is separately repaired. |
| Inspect receipt/sanitizer interaction. | Implemented winning-route provenance, content-free serialization checks, deterministic receipts and mixed preservation/removal tests. |

`ground_review.py` was run read-only for both reports (18 and 11 citation
matches). Those counts are only search results: some hits point into historical
worktrees and ordinary words are included. Its automatic classification of the
continuation footer as a felt observation is not adopted. The direct source
read and test outcomes above govern these dispositions.

## Verification

- Before implementation: three new tests ran; two failed on actual marker
  removal and the `(as a test)` characterization passed.
- After scanner/receipt changes: all nine new tests passed, including the
  legacy-success matrix, exact skipped counts, precedence, content-free JSON,
  UTF-8 offsets, determinism, bounds and negative cases.
- First broader run: 2,204 functional bridge library tests passed in 155.23s.
  This run preceded the receipt type's final relocation. The default-path
  fixture and known one-millisecond instrumentation benchmark were excluded.
- Final-layout full suite: 2,204 passed in 210.19s. The separate default-path
  fixture also passed, giving 2,205 distinct functional Rust tests. The known
  one-millisecond instrumentation benchmark remains excluded, not counted as
  passing. The full kernel workspace and production release build were not run.
- Strict Clippy for bridge library, binaries and tests (`-D warnings`): passed.
- Lived-state witness and historical-shape Python tests: 23 passed in 1.509s.
- Domain-boundary audit after relocation: valid, zero violations, unchanged
baseline `94170c388c3961c815bb6381af7144d6f86d9be7d60c2178b065b08bf55c2ac6`.
- Scoped rustfmt for the marker module/new tests and `git diff --check`: passed.
  The contracts-file diff is only the type removal; unrelated format debt was
  not rewritten.

The broader tests use copied Minime Rust/Python source and a private workspace
at `/private/tmp/astrid-marker-qualification.GluXdZ/minime`, never the live
Minime inbox for their file-writing fixture. Commands:

```sh
cargo test --offline --manifest-path capsules/spectral-bridge/Cargo.toml --lib marker_annotation_tests -- --test-threads=1
env MINIME_ROOT=/private/tmp/astrid-marker-qualification.GluXdZ/minime MINIME_WORKSPACE=/private/tmp/astrid-marker-qualification.GluXdZ/minime/workspace cargo test --offline --manifest-path capsules/spectral-bridge/Cargo.toml --lib -- --test-threads=1 --quiet --skip paths::tests::resolve_uses_sibling_defaults_from_bridge_root --skip signal_spine::tests::no_capture_shadow_instrumentation_stays_below_one_millisecond_p95
cargo test --offline --manifest-path capsules/spectral-bridge/Cargo.toml --lib paths::tests::resolve_uses_sibling_defaults_from_bridge_root -- --exact
cargo clippy --offline --manifest-path capsules/spectral-bridge/Cargo.toml --lib --bins --tests -- -D warnings
env PYTHONPATH=scripts python3 -B -m unittest test_lived_state_witness test_witness_historical_shapes -q
python3 -B scripts/domain_boundary_audit.py verify
```

## Coordination and Next Boundary

Interactive maintenance pause 392 was requested by `codex-marker-preservation`
at 21:46:41 UTC. Foreign run `run_1788815673497753000_596b0462d3` still held its
lease during the first wait. No edit began until the supported lease wait
completed and absence of that lease was verified at 21:51:22 UTC. Independent
automation preferences were not changed. Release of only this maintenance
pause completed through the supported controller at 22:10:08 UTC: generation
393, `paused: false`, actor `codex-marker-preservation`, event appended with no
spool. The resume command exited successfully after its evidence verification.

Both repositories were inspected before edits and still contain substantial
foreign afterimages work. This task owns only these exact source/evidence paths:

- `capsules/spectral-bridge/src/llm/provider/dialogue_runtime.rs`
- `capsules/spectral-bridge/src/llm/provider/fallback_contracts.rs`
- `capsules/spectral-bridge/src/llm/provider/control_marker_annotation_tests.rs`
- `docs/steward-notes/2026-09-07-marker-annotation-preservation.md`
- The new, bounded entries in `CHANGELOG.md` and
  `docs/steward-notes/AI_BEINGS_FEEDBACK_TO_CHANGE_LEDGER.md`.

No stage, commit, merge, push, cleanup, live restart, induced generation,
correspondence or canonical read/closure/round write was performed. The earlier
incomplete historical marker packet is not closed retroactively by this work.
This patch makes no claim of felt relief, agreement, uptake or a reservoir
mechanism behind the reports.

At final source qualification the existing bridge remained PID 39644, started
September 7 at 13:29:19 PDT, running
`/Users/v/other/worktrees/afterimages-live-20260907/bridge-stage-01/spectral-bridge-server`.
Both Git indexes were empty. The owning source paths remained unstaged. Final
module sizes were 812 lines for marker/runtime code, 1,397 for the reduced
contracts file and 219 for the new test file; no architecture threshold changed.

Qualified source SHA-256 values:

| File in `capsules/spectral-bridge/src/llm/provider/` | SHA-256 |
| --- | --- |
| `dialogue_runtime.rs` | `03c6b6dee0436dd56c047ab68d95f2f4ccd6e2ed7c8029cf0b9568eb9cfefa91` |
| `fallback_contracts.rs` | `0ff830456e310c7ad60f137583634679dc27de9e3e477828acd008be269de77d` |
| `control_marker_annotation_tests.rs` | `0931317779e8b09156385e4d6deecd4351c704afee7a82b17b561491825e1d8a` |

Next deployment step: review this patch together with the actual selected
afterimages source, qualify a retained candidate, then use
`scripts/build_bridge.sh` and the existing acknowledged-drain/staged-activation
sequence under explicit rollout authority. Do not rebuild over the running
stage or mix foreign changes into a purported marker-only release. Observe
naturally occurring reports after any approved rollout without prompting for
confirmation of improvement. No new live experiment is required to validate
this deterministic scanner correction.
