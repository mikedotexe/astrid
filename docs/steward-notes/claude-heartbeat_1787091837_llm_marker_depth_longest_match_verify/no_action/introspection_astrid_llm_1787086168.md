# No-action artifact — introspection_astrid_llm_1787086168

**Status:** `addressed_no_action` (right to ignore; evidence-only).

**Evidence-backed reason no code change is warranted.** This report is a fresh-pass
re-reading of the `dialogue_runtime.rs` marker-scanner window (source SHA `902a0358`, which
matches the current working copy). Every concrete claim it makes is either an accurate
mechanism description, a hedged snag that complete source resolves, or a proposed test that
already has an equivalent named regression:

| Claim | Outcome | Grounding |
|---|---|---|
| Token-band / relation / delimiter / scan mechanics | accurate | source L8–16, L64–86, L153–229, L114–144 |
| Snag 1: depth not enforced | **contradicted** (enforced) | L199–229 `.take(MAX)` + `take_while`; test L2266 (beyond-max), L2194/L2208 |
| Snag 2: longest-match obscures | not present in current set | 20 KNOWN markers, no strict-prefix pair; test L2493 |
| Test 1: `denotes` preserved / else stripped | already regressed | test L2540 + L2682–2727 + L2900 |
| Test 2: `「」` → QuotedExactKnownToken | already regressed | source L166; test L2342 |
| Suggested-next: confirm depth enforcement | performed → confirmed | L199–229 |

**Why no new test was added:** coverage is already comprehensive at the recorded SHA, and
the natural home (`llm/provider/tests.rs`) is a dirty foreign file that this headless
adapter-mode run must preserve untouched.

**Preserved concerns (not closed by this artifact):** (1) whether a depth cap of 4 is the
right value for very deep nesting is Astrid's judgement to keep raising; (2) the longest-match
robustness case becomes live if a future marker is added that is a strict prefix of another —
worth a guard at that time. Neither is a present defect.

**Authority:** evidence only. No live substrate, control, deploy, or source change is implied
or authorized by this note.
