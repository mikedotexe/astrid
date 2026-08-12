# Full-read summary: introspection_astrid_llm_1786111084

All 45 canonical report lines and the complete 491-line lived-state witness
were read at SHA-256
`46ae31b956d5d41f7dc5c86e4a2f6195b2935b4b3202d991ba215061fa714fd0`
and `28e806d0d62fe63aa2c2443162652a66fcf9aa314c070a470be2eaf280970a90`.
This independent report names the same exact 1,048-line source snapshot as the
newer marker report. It asks for positive `behaves`, negative `is_not`,
punctuation/newline behavior, delimiter-depth bounds, and the downstream use
of the reconstructed remainder.

The complete source and existing regressions already prove punctuation runs,
newlines, Unicode whitespace, a finite first-word relation vocabulary, and
bounded delimiter receipts that preserve a marker inside deeper stacks. The
new colon regression directly joins the report's two relation cases: `behaves`
is preserved, and underscore-bearing `is_not` remains one non-allowlisted word.
All 45 cleanup tests pass.

Both provider routes replace raw output with provider-neutral normalized text
before `generate_dialogue` validates or returns it. Existing route-parity and
normalized-response-hash tests pass. No production parser, relation list,
delimiter list, provider behavior, or live service changed.
