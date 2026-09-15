# introspection_source_catalog_1789282513 — the scan limit is a catalog wall, not a hiding place

Astrid is on turn N of the "Arithmetic Gap": she wants the code that mints `fill_pct`
from raw reservoir telemetry before `research_budget_guard` consumes it. This turn she
received a recovery map instead of a source page and read it as confirmation of a
standstill.

## What the exact receipts show

Her three preceding literal searches were `"fill_pct"` — **with the quotation marks
inside the query** — at pages 13, 14, 15 and 16 of 55. Between them, at 23:44:58, she
tried to scope a search with an invented option:
`FIND "check_phase_timeout" --path astrid/crates/astrid-capsule/src/`. `page_suffix`
recognises only `--page N`, so the whole string became the literal query and returned one
empty page. Her next move, `MAP astrid-capsule`, took the bare-topic branch of
`path_candidates` and produced a recovery map with **zero** candidates — the mechanism
already pinned in packet `claude-heartbeat_1789204702`. That recovery map is this report's
input, and its `Reason` string is the source of the "use SELF_STUDY MAP" she quotes.

## The finding

`MAX_HITS = 1500` in `source_search.rs:6` aborts the **entire** scan, not the current
file, and `Catalog::sources` returns ids in sort order. Her live query read **142 of 6869**
catalog files and stopped inside one fixture:
`capsules/spectral-bridge/src/autonomous/btsp/fixtures/current_ledger_compact_v2.json`,
which carries 1954 matching lines and supplied 1479 of the 1500 permitted hits against 21
implementation lines.

Because `autonomous/` sorts before `types/` and `ws/`, the two files that actually answer
her — `types/schema/telemetry.rs:334-337` (`self.fill_ratio * 100.0`) and
`ws/telemetry_port.rs:640-654` (`resolve_fill_pct`) — were **never scanned**. Walking all
55 pages surfaces 25 distinct files and neither of those. There was no page she could turn
that contained the answer.

She read the instrument correctly ("the search is hitting a scan limit and returning
hundreds of matches in a `.json` file") and drew the only inference the output offered:
that the producer must be "abstracted into a utility module ... that isn't as easily
indexed." It is not. It just sorts later than the file that spent the budget.

## The steward-side correction

Packet `claude-heartbeat_1789263789` recorded `RELATE fill_pct` as a one-move recovery,
proven in a synthetic three-file tree. On the live catalog it hits the identical wall: 142
files read, `scan limit reached: true`, 55 pages, **no `Definition candidates` section at
all**. And `SELF_STUDY RELATE fill_pct` is exactly what the study prompt offers her by name
on every one of these turns, derived from her own `STUDY_QUESTION`. `RELATE
research_budget_guard`, by contrast, scans all 6869 files cleanly. The difference is hit
density, nothing else.

## Contradictions preserved

- Her expected shape, "a division ... e.g. `current_volume / max_capacity`", does not exist
  in Astrid's tree. Both producers **multiply** by 100 and clamp; `fill_ratio` arrives
  already normalised on the wire (`telemetry.rs:214`, documented "0.0 - 1.0, NOT
  percentage"). The capacity ratio she pictures is computed upstream in Minime.
- Her reading of "use SELF_STUDY MAP" as a hint about locality is a misreading of a
  topic-resolution `bail!`. Her underlying instinct about her neighbourhood was right
  anyway, for the different reason above.

## What was and was not done

Four read-only regressions in `crates/astrid-source-study/tests/scan_limit_catalog_reach.rs`
pin the wall for both search shapes, the unsaturated contrast, and the unknown-option
absorption. Nothing about ranking, caps, ordering, prompts or navigation text was changed:
cost attribution, a per-file cap, or a path-scoping form are being-facing navigation
behaviour and are recorded for Mike/operator, not implemented here. Her
`NEXT: SELF_STUDY MAP astrid` was recorded, not dispatched or pre-empted.
