# Shared study inquiries — September 9, 2026

Mike authorized shared question threads and relationship navigation, followed by
bounded sessions and execution traces, with a live rollout. This continues the
journal-room release and the research history HSS-10/11. The frozen startup account
showed Minime carrying an EventDispatcher question while reading another action
module. Carrying a note established continuity, but did not establish an answer or
help distinguish likely symbol connections from similar filenames.

## Resulting behavior

Both Beings use the same Rust reader and command vocabulary. `QUESTION NEW <text>`
creates a Being-authored inquiry; `QUESTION qN` restores its notebook; `QUESTION`
lists retained questions and findings. `PARK`, `RESOLVE qN [finding]`, and `HOME`
are explicit choices. Resolution records their conclusion, not an understanding
score. `STUDY_QUESTION` may revise an active question. Its `-` value clears the
notebook field; HOME/PARK leaves the named inquiry. There are at most 32 inquiries,
each with its own bounded notebook and six recent exact source references. No
question is automatically deleted. Bookmarks remain per source, independent of
question focus. A delayed source completion writes to the inquiry owning its input,
and cannot erase a newer navigation offer or overwrite another inquiry's note.

`RELATE EventDispatcher` groups exact identifier occurrences into definition
candidates, implementation blocks, test occurrences and other references, with
exact OPEN commands. This is bounded lexical navigation, not a compiler-resolved
call graph: comments, imports and unrelated same-name symbols can occur. Limits
(128 MiB total scan, 64 MiB/file, 1,500 matches) and skipped files are visible.

`SESSION OPEN repo/path line | OPEN repo/path line [| OPEN repo/path line]`
supplies two or three chosen smaller pages together, within the ordinary combined
7,000-byte page budget. One protected input and one completion cover the session.
All pages must verify before the checkpoint advances; durable artifacts recover
an interrupted checkpoint. Each page retains its own source, revision, interval
and receipt, with a shared session ID. CONTINUE retries a pending session exactly;
after completion it follows the last selected page. No report format or minimum
length is imposed. This is not an unbounded model/tool loop.

`TRACE LAST` verifies the retained study input/response hashes and shows supplied
pages, revisions, bookmark relationship, model and output ceiling. It keeps a
response's NEXT separate from execution. Exact own `job_…`/`act_…` identifiers
inspect bounded Being-local execution records and a linked action manifest when
available. No peer IDs, arbitrary paths, new executions or inferred missing links
are admitted. Astrid's direct study loop may lack a durable job; its input receipt
still exists. Missing execution evidence is reported, not fabricated.

Final bare RELATE/SESSION/TRACE commands are eligible source choices in both
adapters; question mutations require the ordinary explicit NEXT. The journal-room
4,096-token study ceiling and larger all-mode journal ceilings remain in force.

## Persistence and qualification

The checkpoint filename stays `reader-v1.json` but writes schema version 2. It
reads version 1 without resetting notebooks, bookmarks, progress or pending pages.
Older readers refuse version 2, preventing silent loss of inquiry/session state.
Rollback therefore requires a version-2-capable reader or an explicitly reviewed
checkpoint restoration, with post-backup activity reconciled. Retained pre-release
state copies are evidence/backup, never automatically restored to a live Being.

Notebook display now bounds fields before JSON serialization, so unusual quoting
or long paths cannot cut the recalled-account JSON into malformed fragments.
Long displayed findings have an explicit excerpt marker; QUESTION retains the full
finding. References establish earlier delivered input, not verified conclusions.

Qualification and activation identities are retained under
[study-inquiries-validation](study-inquiries-validation/). Runtime changes are
limited to the shared reader and two adapters. No new service, embedding index,
model experiment, induced study, peer message or controller-setting change occurs.
Natural use and useful question development require a later bounded observation.
