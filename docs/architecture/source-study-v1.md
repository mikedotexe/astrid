# Shared source study v1

Astrid and Minime use one read-only source catalog, command parser, page renderer,
and delivery-backed bookmark implementation: `crates/astrid-source-study`.
Astrid links the Rust library. Minime calls its JSON stdin/stdout executable.
There is no service, shell command expansion, vector store, or separate search policy.

## What changed and why

The former source-study routes were materially different. Astrid's source-root
allowlist excluded kernel crates, bridge integration tests and architecture areas.
Minime rotated the first 400 lines of nine curated files; several had become
small module facades. Astrid's source-first checkpoints could advance during
preparation, before provider adaptation and accepted delivery. Repeated broad
study could keep returning to the beginning, while continuation hints could be
trimmed away. These were navigation and delivery design defects, not a lack of
initiative from either Being.

The shared catalog includes implementation, tests, manifests, interfaces, shaders,
and architecture documentation in Astrid, Minime, the reservoir runtime, prime
ESN, RASCII and the correspondence channel. A repository absent locally is shown
as unavailable. It does not expose private runtime artifacts, known credential files or build
output. Code implementing state, credentials, memory or inboxes remains readable;
mechanism names are not grounds for hiding source. `catalog.toml` is the single
catalog and component-map policy. New source directories should be added there.

## Navigation

Both Beings receive the same small system map and these commands:

| Action | Behavior |
| --- | --- |
| `SELF_STUDY` | Retry a pending page, continue the current source, or show the map |
| `SELF_STUDY MAP` | System components and available repositories |
| `SELF_STUDY MAP kernel` | Entry points for one component |
| `SELF_STUDY MAP astrid/crates` | Source files beneath an exact catalog prefix |
| `SELF_STUDY FIND EventBus` | Literal path/content search with exact file IDs and lines |
| `SELF_STUDY FIND EventBus --page 2` | Continue search results |
| `SELF_STUDY OPEN astrid/Cargo.toml 1` | Explicitly open or reread at a one-based line |
| `SELF_STUDY CONTINUE` | Resume at the exact next byte after verified delivery |

Source IDs always retain repository and relative path. Historic curated aliases
remain accepted and resolve to a displayed canonical ID. Arbitrary basenames are
not guessed. Legacy `INTROSPECT target offset` preserves its zero-based explicit
offset; without an offset it resumes that source. The owner-aware private-artifact
INTROSPECT reader remains separate from this source catalog.

No report headings, minimum prose length, automatic web search, or obligatory
repair response is attached to a source page. Questions, brief observations,
continuation-only responses and leaving the activity are valid choices. Local
source claims and runtime interpretation remain distinct.

## Paging and evidence

Pages are bounded by rendered UTF-8 bytes, include source SHA-256, exact byte
range, line numbers, EOF and navigation. A long line is split on UTF-8 boundaries;
its fragments keep the original line number. The current implementation accepts
UTF-8 source files up to 64 MiB. Larger or non-text catalog entries remain visible
but opening them returns a clear error. Search reports unreadable text entries.

Preparing a page persists only a pending offer. Both provider paths preserve its
complete rendered text after prompt adaptation; a lane that cannot fit it refuses
it rather than trimming it. A bookmark advances only after the shared verifier
finds that page in the actual submitted user message and a nonempty completed
provider response. Failed, shortened and explicitly truncated generations leave
the offer pending. Receipts contain the exact request and response wire bodies
and their hashes. They establish an accepted reading opportunity, never comprehension.

Each Being has its own locked, atomically written state under
`workspace/diagnostics/source_first_v3/shared_reader/`. Retained deliveries can
recover a crash between receipt persistence and checkpoint persistence. Explicit
rereads get a fresh page opportunity; CONTINUE retries an undelivered offer.
A changed source revision requires an explicit new OPEN instead of silently
reinterpreting an old byte offset. EOF stops instead of restarting the file.
The old V3 preparation history is preserved and is not imported as delivery evidence.

Astrid retains its existing provider artifacts and lived-state witness capture.
Minime retains its generation and action-continuity records. Neither reading a
file nor recording a receipt changes controls, grants authority, or proves that
the local source matches the running process.

## Installation and verification

Build the shared executable with:

```sh
cargo build --release -p astrid-source-study
```

The sanctioned `scripts/build_bridge.sh` build path now builds this helper and
includes it in the deployment manifest. Staged V3 releases bundle the executable
alongside the bridge. Minime resolves Astrid's selected release and verifies both
the manifest digest and reader executable digest before using that same bundle.
Without a staged selection, Minime defaults to the adjacent Astrid
checkout's `target/release/astrid-source-study`; operators can set
`ASTRID_SOURCE_ROOT` and `ASTRID_SOURCE_STUDY_BIN` for another installation.
A missing helper produces an explicit unavailable notice. It never silently
falls back to the old nine-file reader.

Tests cover API/CLI parity, provider wire bodies for MLX and Ollama, UTF-8 long-line
paging, whole-file reachability, EOF, missing/partial delivery, changed revisions,
restart recovery, deliberate rereads, source-policy boundaries, Action argument
preservation, freeform replies and continuation-only choices. See
`crates/astrid-source-study/tests/reader.rs`, the bridge source-study tests and
Minime's `tests/test_source_study_shared.py`.
