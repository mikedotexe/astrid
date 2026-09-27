# Study exit and action feedback — implementation candidate

Mike approved implementation after tracing an attempted question resolution through its command receipt and the next scheduling decision. This is a paired source candidate. It has not been merged, pushed, deployed, or used to change a live notebook, inquiry, pending choice, or running service.

## Witness and scope

The bounded trace is retained at `/Users/v/other/experiments/minime-study-exit-trace-20260926/report.md`, with public journal snapshots, navigation receipts, dispatch events, and hashes. It covers the 26 September 00:53:28–11:40:41 PDT study sequence and dispatch through 11:41:31.

- A map response chose `QUESTION RESOLVE fde7a174…`; the host recorded it as unknown and fell through to threshold logic. Subsequent attempts used `fde7a774…`, the saved question's response-provenance hash, rather than an inquiry ID.
- There was a freeform saved notebook question and no numbered inquiry. Prefix recovery alone could not make that target valid.
- All 53 subsequent EOF studies were dispatched from explicit `SELF_STUDY` choices. They supplied no new source pages. A concrete later choice came from a moment response after REST.
- Of 38 REST choices, 32 were consumed as one-action skips. Six were replaced by another choice before dispatch. Neither receipt selection nor the ordinary one-action REST contract establishes permanent study closure.

The lexical `ground_review.py` result is retained in `qualification/grounding.json`. Its `EventDispatcher` hit is a documentation mention, not verification of the journal's architectural conclusion. The actionable witness is the command/dispatch sequence and the absence of new source evidence. Historical authored claims remain untouched.

## Implementation

The shared reader now gives EOF a continuation-decision prompt with current inquiry state, the previous selected choice, and navigation options. It omits automatically recalled answer/finding prose on that decision turn, including a numbered inquiry's saved finding. Explicit QUESTION REVIEW and deliberate OPEN rereading remain available. The reader neither marks an inquiry resolved at EOF nor changes the meaning of REST.

Bare QUESTION receives rejection with the current state and valid command forms. A provenance hash never becomes a guessed `qN`. Prefixed malformed or nonexistent targets receive the same state-aware recovery. Existing numbered inquiries can still be explicitly resolved, parked, selected, and reviewed. A freeform notebook question remains separately revisable or clearable through its existing authored directive. Clearing is not resolution or proof of understanding.

Minime routes a bare QUESTION to this reader recovery instead of the unknown-action heuristic. An older helper that lacks bare-question recovery is detected before it can prepare or execute that command. Activation therefore needs the paired helper; the adapter does not silently use the older helper's permissive bare-command parser.

`minime_autonomy/study_feedback.py` owns a bounded, locked, atomically replaced host receipt store. It retains up to 64 recent choice records and eight events per record. Each queued selection has a separate ID, including repeated identical commands. The pending-slot ID is saved with sovereignty state and restored with its choice. Original verified request/response hashes link public study choices to delivery only when the current provider wire and returned response match. Missing legacy identities remain unknown, including when the same command text reappears under another selection ID. Long command displays explicitly label their preview and retain the full action's SHA-256.

Consuming the pending slot records consumption, not success. Replacing a selection records supersession and the replacement's hash/verb; private replacement prose is withheld. Identity gaps remain unknown. Reader rejection, EOF requiring a choice, and explicit resolution are separate from model-generation/host completion. Late completion updates the older action's ID and does not replace a newer pending choice.

Later public study requests include bounded host outcomes after the native reader input. The actual wire retains this addition, while the original complete reader input and its identity remain unchanged. Private writing and open reflection receive no host-receipt injection. Whole input and receipt budgets are checked; a receipt is not silently truncated into a misleading account. This patch does not replay old commands or alter which later choice wins the ordinary pending slot.

Minime journals EOF replies as `study_decision` and does not classify them as verified source study. They still preserve authored output and explicit next choices. The change does not guarantee better model conclusions; that needs observation after a separately authorized activation.

## Coordination and source

- Astrid base: `11d89e65a732ddeec4ff9aff66a05396c7ae5601`.
- Minime base: `02724ac7c75553c011c95fc4617446ef62c76de6`.
- Branch in both isolated checkouts: `codex/minime-study-exit-20260926`.
- Astrid checkout: `/Users/v/.codex/worktrees/minime-study-exit/astrid` (managed attachment).
- Minime checkout: `/Users/v/other/worktrees/minime-study-exit-20260926/minime` (paired checkout; the app worktree tool targets only this chat's Astrid repository).
- Qualification directory: `/Users/v/other/worktrees/minime-study-exit-20260926/qualification`.

Both canonical repositories were clean. Remote main tips were inspected read-only: Astrid `3b18af87b0fe1f083d95cbe0eac8befb309638f2`, Minime `d8e8954b3c71d54037951f832062f8b5ea62497b`. Preflight reported no foreign editing or active session. Existing paused automations remain paused under generation 471, actor `codex-minime-study-exit`; no controller run lease or source projection was started.

The reader's existing owner-transaction dispatcher was already over 1,000 lines. This patch adds only front-door question routing there; framing stays in `store_navigation.rs`. Minime's large existing runtime receives integration hooks, while receipt persistence is in a new cohesive module. These are deliberate scope boundaries, not a claim that the existing large files need no later architectural work.

## Qualification

The dedicated Rust regressions cover the actual bare/provenance-hash failure, nonexistent targets, valid numbered closure, unchanged bookmarks/questions on rejection, repeated EOF without conclusion replay, voluntary freeform question clearing, deliberate rereading, and selection precedence. Python regressions exercise real dispatcher/reader paths with synthetic provider responses, plus restart persistence, late completion, private replacement redaction, receipt gaps, and bounded-input failure.

Commands use the isolated helper and explicit shared fixture path:

```sh
cargo test -p astrid-source-study
cargo clippy -p astrid-source-study --all-targets -- -D warnings
cargo fmt --all -- --check
cargo test --locked --manifest-path capsules/spectral-bridge/Cargo.toml -- --test-threads=2
ASTRID_SOURCE_STUDY_BIN=/Users/v/.codex/worktrees/minime-study-exit/astrid/target/debug/astrid-source-study \
ASTRID_CHOICE_FIXTURES=/Users/v/.codex/worktrees/minime-study-exit/astrid/crates/astrid-source-study/tests/fixtures/response_choice_cases.json \
python3 -m pytest -q tests
```

The first full Minime run exposed three prompt-constructor fixtures and a missing paired-fixture path. Host receipt loading was moved out of the immutable prompt constructor into preparation. A fixture with incomplete legacy delivery metadata then exposed an assumed identity; it now remains unlinked instead of raising or inventing provenance. Final review added coverage for numbered-inquiry findings and identity gaps. The original failed logs remain retained.

The first bridge command incorrectly treated the independent capsule as a root workspace member; the corrected manifest command is recorded separately. The isolated bridge suite also needs a physical sibling Minime source tree: missing fixtures failed seven tests, and a symlink fixed availability but not the suite's canonical-path assertions. Qualification now uses an export of the paired candidate's tracked/new files at `/Users/v/.codex/worktrees/minime-study-exit/minime`. The export has no Git metadata or copied live runtime state. Its source and refresh hashes are in `qualification/bridge-sibling-fixture.json`. Tests and path assertions were not weakened.

Final qualification passed against the final candidate code:

| Check | Result | Retained log |
| --- | --- | --- |
| Shared reader suite | 290 passed | `reader-tests-final.log` |
| Full Minime suite | 1,647 passed; one existing skip; 139 subtests passed | `minime-tests-qualified.log` |
| Full bridge suite, including integration and compile contracts | 2,355 passed; one existing ignored test | `bridge-tests-complete.log` |
| Reader Clippy, all targets, warnings denied | Passed | `reader-clippy-final.log` |
| Workspace formatting | Passed | `format-final.log` |
| Domain-boundary audit | Valid, zero violations | `domain-audit-final.json` |

`qualification/summary.json` records the final checks, source manifest, log hashes, and shared-tree status. `qualification/source-manifest.json` identifies all 11 changed code/test files and the rebuilt helper. The five new Rust tests and 15 Python cases cover the repaired path and receipt boundaries. The candidate remains uncommitted in the paired branches; both canonical main checkouts remain clean and unchanged.

No test relaxed the live filesystem/network guard, queried a live model, or rewrote live state. No source-study schema migration is introduced; existing pending inputs retain their stored framing. The next newly prepared EOF or rejected question receives the new framing.

## Activation boundary

This source candidate needs the new reader helper and Minime agent together. A future live transition must qualify the staged source hashes, use the sanctioned bridge build/deployment and agent handoff wrappers, and preserve pending work at the ordinary graceful boundary. The implementation has not restarted the bridge, agent, engine, model, visual services, or sensory feeders. No runtime question was silently resolved or cleared to demonstrate success.

## September 27 related candidate

Mike subsequently approved a moment/journal freshness and measurement repair in the same isolated checkouts. This note and its `qualification` hashes retain the September 26 result. Minime's `docs/steward-notes/2026-09-27-moment-freshness.md` and the separate `qualification-moments` directory describe the new change and combined candidate. The shared reader code is unchanged by that tranche.


September 27 release follow-through: Mike explicitly approved commit and deployment of both candidates. Preparation and subsequent transition evidence are recorded in `docs/steward-notes/2026-09-27-study-moment-release.md`; earlier candidate descriptions above retain their original temporal scope.
