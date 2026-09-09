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
| `SELF_STUDY RESUME astrid/Cargo.toml` | Resume this exact source at its existing bookmark |
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

## Input kind and recalled accounts

Every new offered input starts with `THIS TURN`, identifying source page, map,
search, EOF or recovery. The shared output JSON includes `input_kind` and
`evidence_scope`; older retained navigation outputs default to `legacy` without
rewriting their bound text. A map's saved bookmark is not a code page shown now.
Source pages retain exact repository identities, revisions and numbered spans;
search results require OPEN for full context. Empty source pages are labeled EOF.

Notebook entries are earlier responses or explicitly marked excerpts, not verified claims. New entries
carry exact reopen and resume commands alongside their original source revision.
Both adapters put input scope and delivery limitations in the journal. Both
peer-study readers preserve attribution and distinguish a peer account from code
the receiving Being read. Source delivery verifies input, not the response's
symbols, line claims, inference, understanding or deployed behavior.

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

## Study continuity

Maps show delivered byte ranges, partial or complete delivery, deliberate reread,
and changed-source status. Ranges are merged only within the same source revision;
opening the last page alone does not mean the whole file was delivered. A reread
does not erase earlier coverage. Changed files require a new explicit OPEN. The
current source is visible on maps, and incomplete files offer RESUME. Literal
searches with no matches say so and explain that punctuation belongs to the query.

Each Being also has a bounded notebook carried as reference text in source and
navigation requests. It contains up to four recent substantive visible responses,
an optional lasting note, and an optional current question, with response hashes
and source/revision references where applicable. A Being can write one-line
`STUDY_NOTE: <words>` and `STUDY_QUESTION: <question>` fields in a response, or use
`-` to clear either field. Omission and continuation-only replies preserve existing
notes. No extra generation, compulsory format or minimum response is required.
Notes stay in the user reference material; they are never promoted to system
instructions or interpreted as verified code facts.

The note and question have 1,600- and 500-byte limits. Each retained response has
a 16,000-byte storage limit; the rendered notebook JSON has a 9,000-byte budget.
Rendering drops the oldest whole account first. An oversized latest response
keeps its beginning and ending, with a middle-omission marker and `complete=false`.
Legacy excerpts also default to `complete=false`; they are never relabeled full.
Source pages retain their separate 7,000-byte limit. The complete system/reference
input is protected within 24,000 bytes; SELF_STUDY Ollama requests reserve 32,768
context tokens alongside the existing 4,096 output ceiling. MLX and fallback
admission use the same complete input budget. Other journal modes keep their budgets.
Provider reasoning fields and tagged reasoning blocks are not carried as notes.
Both adapters preserve the entire combined input and reject shortened or failed
delivery. Navigation responses have their own retained wire receipts and can update
the notebook, but never advance source coverage or a source bookmark. Stale offers
and conflicting replay evidence are rejected; retained navigation receipts also
recover a checkpoint-write interruption.

Existing shared-reader checkpoints migrate their coverage from verified retained
receipts and seed the previous excerpt from the current bookmark's completed
response. Pending pages, bookmarks and receipts are preserved. The migration does
not import preparation-only coverage or infer understanding. Current checkpoint
version 3 reads versions 1 and 2; older readers reject version 3 rather than erase
recent context on their next write. Rollback requires deliberate state reconciliation,
not restoring an old checkpoint over later deliveries. Original source and
navigation receipt artifacts remain available.

Beside a saved question, valid backticked symbols offer exact RELATE commands.
Distinct recent source locations offer an optional two-page SESSION comparison.
These reuse existing navigation; no automatic search, generation, selected Action,
or compulsory synthesis is introduced. The Being can revise the finding/question,
write freely, continue or stop. A question's source link identifies where it was
asked, not where its answer is known to be.

Astrid retains its existing provider artifacts and lived-state witness capture.
Minime retains its generation and action-continuity records. Neither reading a
file nor recording a receipt changes controls, grants authority, or proves that
the local source matches the running process.

## Entry and follow-through

A successful Astrid study response is eligible for the existing exact-response
attestation and ordinary NEXT dispatch. The shared reader uses this mode only
after verified provider delivery and a successful artifact write. The legacy
workspace-review producer also returns model-authored text in this mode; its
runtime-generated preparation, thin-output and carriage notices use separate,
ineligible modes. Authorship does not grant delegated, mutual or operator authority.

Minime classifies source INTROSPECT before research-budget admission and executes
it as a shared-reader job. Original action wording stays in continuity records;
the chosen source command travels with the job context. An explicit legacy offset
is converted from zero-based to one-based. Workspace artifact requests retain the
existing introspection and experiment-budget policy. SELF_STUDY and source
INTROSPECT share the same read-only exception above the recovery release shelf.

Unusable source targets, map topics and Action syntax return a labelled recovery
map in both adapters. The bounded notice preserves the requested target and makes
clear that no requested source bytes were delivered. This is a navigation offer:
only verified delivery can record its response, and it cannot move source
bookmarks or coverage. Source revision, checkpoint-integrity and storage failures
remain failures. No fuzzy search is presented as an exact source match.

Minime retains a short MAP/FIND/OPEN/RESUME/CONTINUE reference when ambient system
context is compacted, reserving room within the existing prompt limit. Full source
pages continue through the separate intact-delivery path. No extra generation,
service, research budget grant or forced study is needed.

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

## Inquiry and session extension (checkpoint version 2)

The [September 9 inquiry contract](../steward-notes/2026-09-09-study-inquiries.md)
adds Being-owned questions, bounded exact-symbol relationships, explicit 2–3-page
sessions and read-only delivery/execution views in the same shared reader. Session
receipts represent several pages of **one generation**, not several independent
studies. Inquiry IDs bind input to its notebook despite later focus changes.
Existing source catalog and revision identities remain authoritative for access;
lexical relationships and authored findings do not establish runtime behavior.
The file remains reader-v1.json for migration; schema 2 intentionally prevents
an older executable from silently discarding new inquiry/session state.
