# Astrid Journal Write Audit

Original audit date: March 27, 2026
Storage refresh: June 8, 2026

This note documents how Astrid journals are actually written on the current checkout, using read-only evidence from:

- `capsules/spectral-bridge/src/autonomous.rs`
- `capsules/spectral-bridge/src/llm.rs`
- `capsules/spectral-bridge/src/journal.rs`

It also uses live workspace scans of `capsules/spectral-bridge/workspace/journal/` performed on March 27, 2026 and June 8, 2026.

## Executive Summary

Astrid does not currently write to one canonical journal stream. The runtime produces multiple journal-like artifact types:

- signal journals written through `save_astrid_journal()`
- asynchronous longform second-pass journals for some reflective modes
- inbox-triggered outbox reply copies
- self-study companion inbox files sent to minime
- existing bang-prefixed journal files such as `!astrid_*.txt`, `!daydream_*.txt`, and `!aspiration_*.txt` whose writer is not accounted for in the current runtime code

Operationally, the signal journal is the guaranteed write for a turn. Longform is a later, asynchronous secondary artifact for a small subset of modes. That split explains why some thoughts appear twice, why some entries are short even after the longform work, and why some files look like a short journal plus an appended section even though they are actually separate files.

Storage answer as of June 8, 2026:

- The active Astrid journal root is `capsules/spectral-bridge/workspace/journal/`.
- Older entries are not deleted by the bridge. They are moved as whole `.txt` files into `capsules/spectral-bridge/workspace/journal/archive/until_<local-mtime>/`.
- The archive directory name is a cutoff bucket based on file modified time, not a semantic topic, body section, or parsed timestamp from the filename.
- The direct live directory is capped at 6,000 `.txt` files. When it exceeds that cap, the oldest 3,000 direct files are moved into a new `archive/until_*` bucket.
- The current code reads recent local continuity from the direct journal directory only. Archive-aware tools must explicitly traverse `journal/archive/`.
- Paragraph/chunk-level longform retrieval is still a proposed follow-up, not the current journal storage layout.

## Write Paths

### 1. Signal journal write

Primary write path: `capsules/spectral-bridge/src/autonomous.rs:6534-6536` and `capsules/spectral-bridge/src/autonomous.rs:2883-2923`.

- Trigger: every normal Astrid response path that produces `response_text`
- Destination: `capsules/spectral-bridge/workspace/journal/`
- Naming:
  - `daydream_<ts>.txt`
  - `aspiration_<ts>.txt`
  - `moment_<ts>.txt`
  - `dialogue_longform_<ts>.txt`
  - `daydream_longform_<ts>.txt`
  - `aspiration_longform_<ts>.txt`
  - `witness_<ts>.txt`
  - `introspect_<ts>.txt`
  - `self_study_<ts>.txt`
  - fallback default: `astrid_<ts>.txt`
- Status: canonical in the current runtime, because it is the only guaranteed journal write for that turn

Current detail: `_longform` modes are now mapped to their own filename prefixes in `save_astrid_journal()`. Older audit notes that described `_longform` files falling through to `astrid_<ts>.txt` are historical.

### 2. Async longform second-pass journal

Secondary write path: `capsules/spectral-bridge/src/autonomous.rs:6601-6623` and `capsules/spectral-bridge/src/llm.rs:3184-3230`.

- Trigger: only when `mode_name` is `dialogue_live`, `daydream`, or `aspiration`
- Destination: `capsules/spectral-bridge/workspace/journal/`
- Naming: saved through the same `save_astrid_journal()` helper, with current prefixes `dialogue_longform_<ts>.txt`, `daydream_longform_<ts>.txt`, and `aspiration_longform_<ts>.txt`
- Body shape:
  - original compact signal
  - blank line
  - `--- JOURNAL ---`
  - elaborated reflective body
- Status: derived, asynchronous, and best-effort rather than guaranteed

This is not an in-place append to the original signal file. The code writes one journal file immediately, then later writes a second file from a spawned task. A corpus scan did not find journal files sharing the same timestamp suffix, which supports the interpretation that these are distinct artifacts rather than rewrites of the same file.

### 3. Inbox-triggered outbox reply

Trigger and duplication path: `capsules/spectral-bridge/src/autonomous.rs:6639-6643`, with outbox write in `capsules/spectral-bridge/src/autonomous.rs:3266-3277`.

- Trigger: a `.txt` file appears in `workspace/inbox/`
- Effect:
  - Astrid is forced into dialogue mode
  - the reply is saved to the normal journal path
  - the same reply body is also copied to `workspace/outbox/reply_<ts>.txt`
- Status: outbox copy is derived, not canonical

This is one confirmed reason the same response can appear in two places.

### 4. Astrid self-study companion message to minime

Write path: `capsules/spectral-bridge/src/autonomous.rs:6592-6599` and `capsules/spectral-bridge/src/autonomous.rs:3081-3144`.

- Trigger: when `mode_name == "self_study"`
- Canonical write: Astrid still saves the local journal entry through `save_astrid_journal()`
- Secondary write: companion file to `/Users/v/other/minime/workspace/inbox/astrid_self_study_<ts>.txt`
- Special detail: the companion inbox message is excerpted to 1800 chars
- Status: inbox companion is derived and delivery-oriented

### 5. Continuity readback path

Parser path: `capsules/spectral-bridge/src/journal.rs:79-84` and `capsules/spectral-bridge/src/journal.rs:160-205`.

- Local self-continuity uses `read_local_journal_body_for_continuity()`
- If `--- JOURNAL ---` exists, local continuity prefers that section
- Extracted body is still capped to 2500 chars

This means longform can matter for later self-continuity, but only within a bounded readback window.

## Current Corpus Snapshot

Live scan date: June 8, 2026

Observed from `capsules/spectral-bridge/workspace/journal/`:

- Total journal files under the journal root: 148,660
- Direct live `.txt` files: 4,660
- Archived `.txt` files under `journal/archive/`: 144,000
- Archive buckets: 48
- Oldest observed archive bucket: `archive/until_2026-03-28T10-29-48`
- Newest observed archive bucket: `archive/until_2026-06-05T06-56-00`
- Files containing `--- JOURNAL ---`: 48,459 total; 1,753 direct live; 46,706 archived
- Bang-prefixed journal files: 168 total; 9 direct live; 159 archived

Top direct-live filename prefixes by count:

| Prefix | Count |
| --- | ---: |
| `astrid` | 1834 |
| `dialogue_longform` | 1449 |
| `moment` | 600 |
| `daydream` | 174 |
| `daydream_longform` | 171 |
| `aspiration_longform` | 133 |
| `aspiration` | 133 |
| `witness` | 124 |
| `self_study` | 34 |
| `evolve` | 9 |

Direct-live bang-prefixed files observed on June 8, 2026:

- `!aspiration_1780803400.txt`
- `!astrid_1780803243.txt`
- `!astrid_1780803748.txt`
- `!dialogue_longform_1780803155.txt`
- `!dialogue_longform_1780803339.txt`
- `!dialogue_longform_1780803634.txt`
- `!dialogue_longform_1780803800.txt`
- `!self_study_1780802179.txt`
- `!self_study_1780802952.txt`

Broad conclusion: the missing-looking entries are mostly in archive buckets. Longform is now common in the corpus, but still represented as whole journal files with an internal `--- JOURNAL ---` section, not as paragraph-level retrieval chunks.

## Archive Mechanics

The compaction path is `save_astrid_journal()` -> `managed_dir::compact_text_directory(&journal_dir)`, plus startup compaction in `spectral-bridge-server`. The shared compactor lives in `capsules/shared/managed_dir.rs`.

Current defaults:

- `DEFAULT_LIVE_CAP = 6000`
- `DEFAULT_BUCKET_SIZE = 3000`
- extension: `.txt` for journals

The compactor only looks at direct files under `workspace/journal/`; it ignores the existing `archive/` subtree. It sorts direct files by modified time, moves the oldest bucket into `archive/until_<mtime-of-newest-moved-file>/`, and repeats until the live directory is under cap. This is why archive buckets usually contain exactly 3,000 files.

This is movement, not deletion. A file renamed with a leading `!` remains a normal `.txt` file for this compactor and will be archived by age like any other journal unless a future pinned-favorites rule teaches the bridge otherwise.

## Confirmed Artificial Limits

### Dialogue context is still finite and budgeted

From `capsules/spectral-bridge/src/llm.rs:188-195`, `capsules/spectral-bridge/src/llm.rs:1612-1640`, and `capsules/spectral-bridge/src/llm.rs:1684-1729`:

- current-turn journal context is capped by `DIALOGUE_JOURNAL_CAP = 2400`
- recent history is still excerpted by age, but the old 80/200-char binary compression has been replaced by a gradient
- prompt blocks are assembled within a profile-specific budget, with lower-priority overflow written aside when needed

This means even when a source journal is rich, the live dialogue model still sees a bounded slice rather than the full corpus.

### Live dialogue output is still bounded, but the old 800-char return cap is historical

The March 27 trace correctly identified an 800-character live-dialogue return cap for the artifact it examined. The current checkout no longer shows that exact `generate_dialogue()` return cap. The live dialogue path is still bounded by requested token caps, profile-specific prompt budgets, and downstream journal formatting, but this audit should not treat the old 800-character return cap as current behavior.

Historical example:

- `/Users/v/other/astrid/capsules/spectral-bridge/workspace/journal/astrid_1774637463.txt`

The file ends mid-thought and remains useful as evidence of the older cap, not as proof of the current return path.

### Longform exists only for three modes

From `capsules/spectral-bridge/src/autonomous.rs:6601-6623`:

- Stage B runs only for `dialogue_live`
- Stage B runs only for `daydream`
- Stage B runs only for `aspiration`

No equivalent longform second-pass exists in the current runtime for `mirror`, `witness`, `dialogue`, `moment_capture`, or `self_study`.

### Longform is written as a second file, not an upgrade of the first file

From `capsules/spectral-bridge/src/autonomous.rs:6534-6536` and `capsules/spectral-bridge/src/autonomous.rs:6601-6623`:

- the compact signal journal is written first
- a spawned task later writes a second journal file containing the original signal plus the `--- JOURNAL ---` section

This is why the system can feel like it creates a short journal and then “the next file is identical except appended.” In current behavior, that is not one file being rewritten. It is two separate artifacts.

### Stage B expands only signal text plus spectral summary

From `capsules/spectral-bridge/src/autonomous.rs:6604-6615` and `capsules/spectral-bridge/src/llm.rs:3187-3230`:

- Stage B receives:
  - `signal_text`
  - `spectral_summary`
  - `mode`
- It does not receive the richer original source journal entry that triggered the turn

So the longform pass is elaborating Astrid's own compact signal rather than re-reading the full incoming context.

### Local continuity still has a bounded readback cap

From `capsules/spectral-bridge/src/journal.rs:79-84` and `capsules/spectral-bridge/src/journal.rs:160-205`:

- local continuity prefers the `--- JOURNAL ---` body when present
- extracted continuity text is still capped to 2500 chars

This is better than header-only readback, but it is still not unlimited longform continuity.

## Why Responses Sometimes Appear In Two Places

There are three separate duplication patterns in current behavior.

### Inbox reply duplication

When a message is dropped in `workspace/inbox/`, Astrid is forced into dialogue mode. The response is then written both to:

- the normal journal stream
- `workspace/outbox/reply_<ts>.txt`

Example pair:

- `/Users/v/other/astrid/capsules/spectral-bridge/workspace/journal/astrid_1774636070.txt`
- `/Users/v/other/astrid/capsules/spectral-bridge/workspace/outbox/reply_1774636070.txt`

### Self-study dual delivery

When Astrid performs `self_study`, the system writes:

- a canonical local journal entry
- a delivery copy to minime's inbox

Those are intentionally different artifact roles even when they share the same core text.

### Signal plus longform second-pass

For `dialogue_live`, `daydream`, and `aspiration`, current behavior can produce:

- a short signal journal first
- a later longform second artifact that repeats the signal and appends `--- JOURNAL ---`

Representative observed sequence:

- `/Users/v/other/astrid/capsules/spectral-bridge/workspace/journal/daydream_1774637019.txt`
- `/Users/v/other/astrid/capsules/spectral-bridge/workspace/journal/astrid_1774637045.txt`

These are separate files, not one file being extended in place.

## Representative Examples

### Truncated live dialogue

`/Users/v/other/astrid/capsules/spectral-bridge/workspace/journal/astrid_1774637463.txt`

- Mode: `dialogue_live`
- Behavior: cut off mid-thought
- Interpretation: consistent with the 800-char hard cap in `generate_dialogue()`

### Longform second-pass journal

Either of these captures the Stage B pattern clearly:

- `/Users/v/other/astrid/capsules/spectral-bridge/workspace/journal/astrid_1774637045.txt`
- `/Users/v/other/astrid/capsules/spectral-bridge/workspace/journal/!astrid_1774632449.txt`

They contain:

- the original compact signal
- `--- JOURNAL ---`
- a much longer reflective body

### Inbox reply duplication

This pair demonstrates the same response body landing in two destinations:

- `/Users/v/other/astrid/capsules/spectral-bridge/workspace/journal/astrid_1774636070.txt`
- `/Users/v/other/astrid/capsules/spectral-bridge/workspace/outbox/reply_1774636070.txt`

## Mode Notes

### `dialogue_live`

`dialogue_live` is still bounded by prompt budgets, token caps, and journal formatting, but the old direct 800-character return cap should be treated as historical unless reintroduced by a future code path.

### `mirror`

`Mode::Mirror` in `capsules/spectral-bridge/src/autonomous.rs:1341-1388` reads a remote journal body and writes that text back as Astrid's mirror entry. It does not apply the 800-char `generate_dialogue()` cap, because it is not using the dialogue generation path. Mirror is often short because it inherits already-short remote journal content, not because mirror itself adds the live dialogue truncation limit.

Example:

- `/Users/v/other/astrid/capsules/spectral-bridge/workspace/journal/astrid_1774487045.txt`

That mirror entry ends mid-word, but the limiting factor is upstream content shape rather than the dialogue cap.

### `witness`

`witness` is frequently tiny because many recent witness turns are the fallback string produced by `witness_text()` in `capsules/spectral-bridge/src/autonomous.rs:801-803`:

- `[witness — LLM unavailable] fill=...`

That fallback dominates recent witness corpus size more than prompt design does.

### `dialogue_fallback`

`dialogue_fallback` is intentionally small because it comes from three fixed fallback strings in `capsules/spectral-bridge/src/autonomous.rs:440-450`. These are not longform paths.

## Bang-Prefixed Files

No current write path was found in:

- `capsules/spectral-bridge/src/autonomous.rs`
- `capsules/spectral-bridge/src/llm.rs`
- `capsules/spectral-bridge/src/journal.rs`

that writes bang-prefixed filenames such as:

- `!astrid_*.txt`
- `!daydream_*.txt`
- `!aspiration_*.txt`

Those files definitely exist in the journal workspace. On June 8, 2026, 9 were direct-live and 159 were under `journal/archive/`.

Current behavior:

- General recent-continuity reads include direct-live `!*.txt` files because they scan all direct `.txt` files by mtime.
- Prefix-filtered reads such as witness seeding do not match bang-prefixed files unless they are taught to strip or interpret the leading `!`.
- Archive traversal is not implicit in normal recent-continuity reads.
- The leading `!` is therefore curator-visible but not yet a formal pinned/favorite semantic in runtime code.

## Suggestions, Not Implemented

These are follow-up recommendations only. They are not part of the current behavior.

- Evaluate longform availability across all reflective modes, not only `dialogue_live`, `daydream`, and `aspiration`.
- If the system eventually changes, prefer one natural canonical record per thought, with delivery artifacts like outbox or inbox companions labeled explicitly as secondary.
- Keep dialogue budget limits visible in diagnostics so future truncation can be attributed to prompt budget, token budget, fallback behavior, or journal formatting instead of being guessed from file size.
- If dual artifacts remain, attach an explicit linkage id so signal and longform files can be paired deterministically instead of by timing and visual similarity.
- If longform remains a second pass, consider giving Stage B the richer triggering journal context rather than only `signal_text + spectral_summary`.
- If bang-prefixed files are meant to be pinned memories, implement that deliberately: scan `!*.txt` across both live and archive roots, strip the marker for mode classification, and surface them in a separate curated-memory lane rather than relying on mtime accidents.

## Bottom Line

Astrid is not blocked from longform in principle, and the corpus now contains many longform-section files. The most important remaining storage confusion is that only the newest direct files sit in `workspace/journal/`; the rest are whole-file archives under `workspace/journal/archive/until_*`. The most important semantic confusion is that one thought can still create multiple artifacts with different purposes, and the runtime does not yet present them as one explicitly linked record.
