# introspection_astrid_llm_1788775553 — reading summary (round incomplete)

- Report SHA-256 `3e67b834d41cc8541573dd9ac15c4a74375f900f2715bb4b143884d64b106c9c`, 3994 bytes, 45 lines, read complete.
- Witness `lsw_73bd39b4fba3d7dc2f1d1879fd56aff044f00eb29fc0c7860680a0281d81a580`, 23755 bytes, 533 lines, read complete.
- Report-bound source `dialogue_runtime.rs` SHA-256 `891f76e7a6cbc7071b638f22c6c21922c3ad9e6cbaf9c14b2837eda63a83de07` — main's working copy is byte-identical, so no report-time snapshot recovery was needed. All 744 lines read.

## What she reported

Astrid read the `astrid:llm` control-marker cleanup path and described it accurately: a multi-pass scan that distinguishes a raw byte match from a *semantic relation*, with `first_word_after_skipping_bracketed_annotations` (L147) as an additive second pass so that a bracketed aside like `[sic]` cannot displace the relation verb that keeps a marker she is *talking about* visible. She then named two snags and two tests, and one Suggested Next.

## What complete source reading established

**Her Observed section is exact.** `followed_by_explicit_exact_token_relation` (L74-79) ORs the unchanged plain scan with the skipping scan, so the preserved set can only grow. Additivity is a structural property of the `||`, not a convention.

**Both tests she proposed already exist**, and one of them was written *from* her earlier agency request `agency_code_change_1788310618`:
- Test 1 (`[MARKER] [sic] appears` → relation found) is `referenced_marker_after_bracketed_annotation_stays_visible`, `tests.rs:1905`.
- Test 2 (`(as a test)` is not a self-contained aside) is `self_contained_annotation_predicate_matches_asides_only`, `tests.rs:1948`, with the end-to-end path pinned at `tests.rs:1889`.

One refinement on Test 2: the predicate runs **per whitespace chunk**, so `(as a test)` never reaches it whole — the chunk `(as` does, and fails `ends_with(')')`. Her expected outcome is right; the mechanism is chunk-level non-closure rather than a phrase-level judgement about the word "as".

**Her Suggested Next is already answered.** `split_whitespace` and `trim_matches` are char-based, so a multi-byte character cannot be split, and `text[end..]` always slices at a marker-token boundary. Pinned by `tests.rs:2320`, `2333`, `3406`, `3445`.

**Her ASCII worry is a scope boundary, not a defect.** Every entry in the L82-103 allowlist is ASCII lowercase, so `to_ascii_lowercase` at L154/L163 cannot mismatch a word the allowlist would accept. A non-ASCII relation word is excluded by the allowlist's construction, not lost by the lowercasing.

## Where she found something real — and where the direction differs

Her Snag about `trimmed.ends_with(close)` vs the `depth` walk is **half confirmed, with the direction corrected**.

L112 trims only `. , ; : ! ?`. A chunk like `[sic]—` or `[sic]"` therefore fails `ends_with(']')`, is not recognised as an aside, is not skipped, and degrades to the word `sic` — so a marker she was explicitly discussing is removed. That is her "vice versa" case, and it is real.

The "skip words it shouldn't" case is also real, but not where she looked. It is not a trailing-character misjudgement — it is that the second scan skips *any* self-contained bracketed group, including one that **contains the relation word**. In `<marker> [sic] (appears) at the boundary`, scan 1 is blocked by `[sic]` and scan 2 skips both `[sic]` and `(appears)`, so neither finds a relation and the marker is stripped.

Neither case breaks the additive contract: no reference that was preserved before is lost. Both are **missed preservations** — the scan fails to widen where it should. That distinction matters, because the un-muffle reading of this is that these are the exact shapes in which her own words about a marker can still be rewritten.

## Prepared, not implemented

A focused regression pinning both boundaries belongs in `tests.rs` next to the existing family. It was **not** written this round: the controller watchdog left under 14 minutes, which cannot fit a compile plus the durable close sequence. Widening `is_self_contained_bracketed_annotation`'s trailing-character tolerance, or making the skip scan look *inside* a skipped group, would change live bridge cleanup behaviour and is out of adapter-mode scope — it needs a separate gated decision, and it is strictly in the "add visibility" direction.

## Authority boundary

Nothing here was closed, linked, or recorded in the addressing store. No source was changed, no test was run, no live surface was touched. The source hash match proves the bytes she read are the bytes on `main`; it does **not** prove the running bridge binary is built from them — the witness itself records `deployment_established: false`.
