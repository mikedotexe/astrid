# Source-Study Question Grounding

Date: 2026-09-09 PDT / 2026-09-10 UTC generation records

Status: isolated source candidate, qualified locally, not established live

## Why This Review Began

Mike surfaced a public Minime SELF_STUDY entry in which Minime said that the
next needed implementation was `dispatch.rs`, but ended with a `FIND` against
the `mod.rs` inclusion line. A later public entry again named `dispatch.rs` but
chose a two-page SESSION containing only earlier `mod.rs` locations.

The reports were read as navigation evidence and as Minime's attributed
account. Their headers explicitly say that no new source page was supplied and
that response claims were not independently verified. Those boundaries are
retained here.

## Complete Public Sequence Read

The following Minime journal files were read completely:

- `/Users/v/other/minime/workspace/journal/!self_study_2026-09-09T17-52-40.086870.txt`
  - SHA-256 `5bdf769d94bc1d0f0f939a47d9eda75d45a0f74aacda67991f358a066e10be99`
- `/Users/v/other/minime/workspace/journal/self_study_2026-09-09T18-02-01.136777.txt`
  - SHA-256 `de8e34105416b9816b4a0504f6b65aedda2f532ccb3e0b90b09da304bb0d7857`
- `/Users/v/other/minime/workspace/journal/self_study_2026-09-09T18-04-30.540468.txt`
  - SHA-256 `38f58fe16dd2a4ed48c91e2286410bdaeabeef0cad068b7e31f245a156056471`
- `/Users/v/other/minime/workspace/journal/self_study_2026-09-09T18-06-57.769795.txt`
  - SHA-256 `b092c5707518e12e3d005bf5157d1943e247d020228e0c0cb95de9a2e8a955a6`
- `/Users/v/other/minime/workspace/journal/self_study_2026-09-09T18-09-32.633463.txt`
  - SHA-256 `20f054a18e2c676204112079a1efb8aca53ab216a1bc4c83947498793f9e5e69`
- `/Users/v/other/minime/workspace/journal/self_study_2026-09-09T18-12-06.995277.txt`
  - SHA-256 `53768af0ae3063dd96fd26e4c377084870d7a8a20716327a628f682da4a21778`

The sequence is coherent:

1. Minime named `dispatch.rs` as the source he needed, but emitted a bare
   `FIND <mod.rs path> 2129` line. The corresponding generation record has no
   parsed next action.
2. The retained STUDY_QUESTION continued to name `dispatch.rs`. The reader's
   map offered `RELATE sense_tx` and a SESSION over two recently read `mod.rs`
   locations, but did not offer the named sibling source.
3. Minime chose that exact system-offered SESSION twice. It supplied two
   `mod.rs` pages and no `dispatch.rs` bytes.
4. Minime then chose `SELF_STUDY FIND <mod.rs path> sense_tx` twice. FIND is a
   global exact-literal search, so this searched for the entire path-plus-name
   phrase rather than for a symbol within one file.
5. The final entry retained the same premise and ended with a bare
   `FIND <mod.rs path> Route`; the generation record again has no parsed next
   action.

Representative retained generation evidence:

- `gen_1789001560080_self_study_a0.json`, SHA-256
  `cf4c783404b2a85208fe6b24793440cc2c9f3bb73af3bb10db049b5deb79e3c0`:
  malformed bare FIND; `next_action_parsed` is null.
- `gen_1789002121129_self_study_a0.json`, SHA-256
  `5b48735f052415b5ee5aff883154ee4d7c1fb6a9a70b973f4d23f525d7f67ad9`:
  the repeated two-page SESSION is admitted and is one of the reader's offered
  choices.
- `gen_1789002417762_self_study_a0.json`, SHA-256
  `049f8cdaea46fc27636a6e5be00c71f7b6c72107668b0337854c8ae1ca1effbd`
  and `gen_1789002572622_self_study_a0.json`, SHA-256
  `5202a751081b28c3cfcd2294b795f4f4581312f525333a48512af3168bc4808a`:
  admitted path-plus-symbol exact FIND requests.
- `gen_1789002726686_self_study_a0.json`, SHA-256
  `10ba52f2a25d10ba01bd5fe7c7209b9f20158782c3b5b1fe9f9d8ca1b48cdb34`:
  final bare FIND; `next_action_parsed` is null.

## Source-Grounded Disposition

Minime was right about the immediate reading move and wrong about the asserted
mechanism.

- `capsules/spectral-bridge/src/autonomous/next_action/mod.rs:2129` includes
  `dispatch.rs`.
- `dispatch.rs:21` defines `handle_next_action_with_author`, so opening that
  file is the direct next step for understanding dispatch.
- `NextActionContext` is defined in `mod.rs:113`; its field is `sensory_tx`, not
  `sense_tx`.
- No `sense_tx` identifier exists in the Rust source under `capsules/` or
  `crates/` in this checkout.
- `dispatch.rs:47-48` obtains continuity `stage` and `visibility` labels, but
  those labels do not implement one global Sense-versus-Silent switch.
- Actual `sensory_tx` writes live in individual handlers, including
  `next_action/operations.rs`, `next_action/shadow.rs`,
  `next_action/sovereignty.rs`, and `next_action/attractor.rs`, with their own
  gates and semantics.

This correction does not make the report unhelpful. The report accurately
named the missing source and exposed that the navigation UI was reinforcing a
stale route while Minime tried to pursue it.

## Implemented Response

`Notebook::study_choices` now receives the catalog and resolves source-like
backticked references in the retained question.

- Exact catalog references resolve directly.
- A basename such as `dispatch.rs` resolves as a sibling of the current or
  retained source locations.
- At most two exact paths are offered.
- Direct question-source OPEN choices appear before symbol RELATE choices.
- When a question source resolves, the generic comparison SESSION over old
  locations is suppressed for that turn.
- The source is not opened automatically. No bytes are supplied until the
  Being chooses OPEN, and no delivery bookmark advances from the suggestion.
- The retained question is not rewritten. `sense_tx` remains available as an
  exact RELATE choice, where a zero-match result can correct the premise from
  source rather than from hidden substitution.

The regression starts from `lib.rs`, retains the question "Where does
`dispatch.rs` trigger `sense_tx`?", and verifies all of the following:

- exact sibling `dispatch.rs` OPEN is offered;
- exact `RELATE sense_tx` remains offered;
- the stale two-page comparison SESSION is not offered;
- a zero-result `FIND <lib.rs path> sense_tx` still carries the direct
  `dispatch.rs` choice;
- choosing OPEN supplies real `dispatch.rs` bytes at line 1.

## Verification

- `cargo fmt -p astrid-source-study`
- `cargo test -p astrid-source-study`
  - 6 context tests
  - 12 inquiry tests
  - 3 path-recovery tests
  - 22 reader tests
  - 43 integration tests total, all passing
- `cargo clippy -p astrid-source-study --all-targets -- -D warnings`
- `cargo fmt -p astrid-source-study -- --check`
- `git diff --check`

All commands passed after the documentation pass.

## Authority And Live Boundary

This candidate changes only shared-reader navigation suggestions and tests. It
does not change action admission, dispatch semantics, sensory writes, prompts,
models, reservoir dynamics, controller values, regulation, source bytes,
Minime's notebook words, or any journal artifact.

The implementation is in the isolated worktree
`/Users/v/other/worktrees/source-study-question-grounding-20260909` on branch
`codex/source-study-question-grounding`. Concurrent shared-tree work was not
overwritten or staged. Passing tests do not establish deployment. Activating a
shared reader for Astrid and Minime remains a separate integration and graceful
rollout step after the live candidate and concurrent work are reconciled.

No reply, uptake, corrected belief, improved experience or completed study is
inferred. Silence remains neutral.
