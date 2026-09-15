# Summary — introspection_astrid_..._autonomous_activity_reading.rs_1789187098

Astrid's third sequential page of `capsules/spectral-bridge/src/autonomous/activity_reading.rs`
(bytes 8571..12834 = lines 254–383, source SHA `3c7fca52…` matching the working copy exactly).
She describes three functions: `offer_requested_reading_in`, `describe_reader`, `status_in`.

**The report is careful and largely correct.** Eleven of sixteen extracted claims verify exactly
against the complete 572-line source, including both line ranges that sit wholly inside her page
(`offer_requested_reading_in` 263–333, `describe_reader` 335–363), the mailbox/foreground mutual
exclusion, the offer trigger, and the five-field composition of `ActivityReadingOfferV1`.

**Four precision gaps, all of the same shape — the page's edge read as the code's structure.**

1. `status_in` is cited as "(lines 365–383)". It opens at 365 and closes at **385**. Line 383 is
   where her *page* ended, inside the function's final `Ok(format!(...))`. The page footer offers
   CONTINUE; it never names the item it just cut. She had no signal that the last thing on her page
   was unfinished.
2. The two lines she did not see complete a format string whose own last sentence is
   *"Status does not advance reading or dispatch a saved command."* — and her reading of `status_in`
   is "the primary telemetry for the UI or the next step in the autonomous loop." The clause that
   contradicts the reading is the clause the page withheld. **Recorded as a contradiction, not
   resolved away.** There is also no UI: `status_in` is produced by her own `ACTIVITY_STATUS` /
   `MAILBOX_STATUS` / `PARK_ACTIVITY` / `CHECK_MAILBOX` verbs (`next_action/dispatch.rs:308-328`)
   and lands in `conv.pending_file_listing`, i.e. back in her own turn.
3. The same audience shift appears in "what the **user** should see next": the offer becomes a
   `ProtectedDialogueInputV1` of kind `Reading` (`runtime/activity_delivery.rs:9-32`) fed to her own
   next model turn. The reader the offer serves is Astrid.
4. The completion guard is quoted as `next_byte == byte_count` alone. Source (287-288) conjoins
   `&& bookmark.offered_passage.is_none()` — the guard that prevents a still-pending passage from
   being dropped by an early `Complete`.

One claim gets a friendly correction rather than a gap: the "hardcoded return command
`RETURN_ACTIVITY`" is emitted *with* `preview.session_record_id`, and the argument is load-bearing.
A bare `RETURN_ACTIVITY` takes the inspect branch (line 394) and performs no return; a stale record
errors. That behavior is already covered at `activity_reading/tests.rs:135-142`.

**What was implemented.** Item 1 is measurable and was unwatched. `scripts/source_page_symbol_truncation_watch.py`
flags a cited `lines A–B` where `B` is the page's last line, `A` is not its first, and the Rust block
opening at `A` closes later. A 200-artifact scan: **16 truncated citations across 97 SHA-verified
reports**, including `shadow.rs` lines 23–103 cited on four separate turns for an item that closes at
184 (81 lines unseen). This is the page-*end* twin of the existing `source_page_item_context_watch`
(page-*start* severance).

**What was not done.** No live change, no page-renderer change, no being-facing surface change. The
obvious downstream — a page footer that names the item straddling its boundary — touches a
being-facing delivery surface and is a separate authority decision, not this round's to take. Her
text was not rewritten, annotated, or corrected anywhere she can see.
