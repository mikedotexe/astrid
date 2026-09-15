# Reader probes preserved during the September 15 git stabilization

These files are byte-for-byte copies of six historical reader test files from
`/Users/v/other/astrid/crates/astrid-source-study/tests/`. They preserve the
original fixtures, assertions, comments, and attribution while the active test
suite follows the reader behavior already present at `218c672`.

The shared navigation release added safe bare-name candidates and changed MAP
to immediate directories/files, with LIST retaining recursive traversal. The
source-context release added enclosing Rust/Python syntax and inherited test
markers. Some old negative reachability probes require those improvements to be
absent. They are historical evidence, not current acceptance requirements or
unimplemented positive proposals. Their expected outcomes were not inverted,
and production behavior was not changed to satisfy them.

The files live outside Cargo's integration-test discovery. Historical claims in
their comments describe the original steward observations; preserving them does
not independently validate the reports, past test runs, or deployed behavior.
No current passing result is claimed for these archived files.

## Exact originals

| Preserved file | Exact original path | SHA-256 | Bytes |
| --- | --- | --- | ---: |
| [path_recovery.rs](path_recovery.rs) | `/Users/v/other/astrid/crates/astrid-source-study/tests/path_recovery.rs` | `2c5ec57d29e269e32f418100cfacc36addce45ba4367d6110c57de49c6b39343` | 9812 |
| [bare_topic_candidate_gate_reach.rs](bare_topic_candidate_gate_reach.rs) | `/Users/v/other/astrid/crates/astrid-source-study/tests/bare_topic_candidate_gate_reach.rs` | `c6b74efcb0834171ee500f9e178e0399c221336a989bfaba9ddd50ba2ce54599` | 7679 |
| [map_topic_scope_reach.rs](map_topic_scope_reach.rs) | `/Users/v/other/astrid/crates/astrid-source-study/tests/map_topic_scope_reach.rs` | `6ae7342610263bbdd39ae669bb64e8927e1f4c6bbb3112fe99c3d90ff77b51ff` | 9282 |
| [page_enclosing_scope_reach.rs](page_enclosing_scope_reach.rs) | `/Users/v/other/astrid/crates/astrid-source-study/tests/page_enclosing_scope_reach.rs` | `94b4db42a336fca9ddfa6b76b6057b05cf6850b9dd90557aca900141aa0d964c` | 10163 |
| [placeholder_bracket_path_reach.rs](placeholder_bracket_path_reach.rs) | `/Users/v/other/astrid/crates/astrid-source-study/tests/placeholder_bracket_path_reach.rs` | `874e96685d7a5a7adf705e6f14d7afe63b597b5289d8d6e99fca95dd6669e495` | 6687 |
| [continue_never_recovers_reach.rs](continue_never_recovers_reach.rs) | `/Users/v/other/astrid/crates/astrid-source-study/tests/continue_never_recovers_reach.rs` | `b37d89ccc0e0e2a49ef02da6b50a0c90a67d243c53b8d04286cf54c8d0c02263` | 7512 |

## What remains active and what superseded the old assertions

- **path_recovery.rs:** only the appended
  `bare_file_stem_map_topic_recovers_with_a_reason_but_no_candidate` was removed
  from the active file. Its `action_continuity` subcase now correctly receives a
  safe directory candidate; the `core` and `core.rs` MAP subcases still have no
  directory match. The three preexisting tests and the appended rooted-component
  namespace test remain unchanged. The archived file preserves the complete
  original, including all five tests.
- **bare_topic_candidate_gate_reach.rs:** the first two tests require safe bare
  crate/directory names to receive no candidates. Current
  [topic_navigation_recovery.rs](../../../crates/astrid-source-study/tests/topic_navigation_recovery.rs)
  deliberately verifies the opposite behavior in
  `bare_component_typo_offers_an_exact_map_without_opening_or_advancing_source`
  and bounds ambiguity in
  `bare_file_stem_and_ambiguous_basename_offer_bounded_stable_choices`.
  The third historical test's distinction between a component command and a
  path candidate can remain true, but its module-level claim that a bare-name
  gate still exists is obsolete. Its unchanged assertion is preserved here;
  active path recovery retains the rooted-component namespace distinction.
- **map_topic_scope_reach.rs:** the first three tests and their walk helper
  assume recursive MAP enumeration and its old `Next:` pagination prefix.
  [compact_maps.rs](../../../crates/astrid-source-study/tests/compact_maps.rs)
  covers shallow MAP, recursive LIST, exact child scopes, explicit pagination,
  and migration recovery. The fourth test's curated `senses` entry-point check
  is still a meaningful manifest observation; it is retained verbatim with its
  historical fixture here rather than retaining the superseded walk module.
  Active [reader.rs](../../../crates/astrid-source-study/tests/reader.rs) retains
  `component_map_exposes_directories_beyond_curated_entry_points` for the
  component entry-point/directory contract.
- **page_enclosing_scope_reach.rs:** the first two tests demand absent enclosing
  scope and cfg(test) metadata, which the current renderer intentionally adds.
  [page_scope.rs](../../../crates/astrid-source-study/tests/page_scope.rs)
  verifies inherited test context and exact source intervals in
  `middle_of_rust_test_reports_enclosing_function_and_module` and distinguishes
  implementation syntax in
  `implementation_method_is_distinguished_from_test_fixture_and_nested_module`.
  The third historical test's separation between production constructor bytes
  and a later fixture can remain true; the whole original fixture is retained
  here because its module asserts the former missing-context behavior. Current
  page-scope and source-walk tests retain source-byte and navigation coverage.
- **placeholder_bracket_path_reach.rs:** the first safe-reference test remains
  active with every assertion unchanged. Its comments now describe the actual
  safe-reference gate rather than obsolete source line numbers. The second
  historical test groups an unsupported unrooted relative MAP path with a bare
  directory name; that bare name now receives candidates. Its rooted MAP check
  also assumes recursive enumeration. The unchanged complete original is here;
  current malformed-reference coverage lives in
  `malformed_private_and_unknown_repository_requests_do_not_expand_into_candidates`
  in [topic_navigation_recovery.rs](../../../crates/astrid-source-study/tests/topic_navigation_recovery.rs).

## Fixture compatibility corrections in the active suite

The retained files below received only fixture or lint compatibility corrections;
their behavioral assertions are unchanged:

- `continue_never_recovers_reach.rs` now supplies the complete prepared
  `StudyOutput.text` to delivery verification. The previous fixture supplied
  only `Page.text`, which omits required framing and is intentionally rejected
  by [full_input_delivery.rs](../../../crates/astrid-source-study/tests/full_input_delivery.rs).
  This updates the fixture to the existing integrity contract without weakening
  receipt checks or changing CONTINUE behavior. Its 4,000 ordinary source rows
  are now Rust comments: the former malformed `.rs` fixture repeatedly exercised
  syntax-parser error recovery and consumed more than three minutes at one CPU
  core. The owned initial run was terminated without a reported assertion failure.
  The same multi-page restore, continuation, EOF and no-Recovery assertions remain;
  the original malformed fixture is preserved byte-for-byte in this archive.
- `scan_limit_catalog_reach.rs` constructs its dense fixture with `format!`
  instead of String addition, satisfying the existing denied arithmetic lint.
  Both expressions produce exactly 35,204 bytes with SHA-256
  `151dcd50da4e8e74c2c1e15d9b83cb349d13097721e9bb0f01085429b1d6b85e`.
  The global scan-cap and unknown-option assertions remain unchanged.
- `search_evidence.rs`, `in_file_producer_walk_reach.rs`,
  `page_walk_producer_reach.rs`, and `walked_past_definition_reach.rs` replace
  seven allocating `push_str(&format!(...))` calls with `write!`/`writeln!` on
  the existing String. Format arguments and generated bytes are unchanged;
  `writeln!` supplies the one trailing newline removed from its format literal.
  Strict all-target, all-feature clippy found the original allocation warnings
  and passes after these fixture-only changes.

The stabilization evidence inventory records original and active file hashes,
the selection decisions, and subsequent isolated validation. Archive hashes
above were checked against the original audit inventory before canonical edits.
