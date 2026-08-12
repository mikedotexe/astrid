# Full-read summary: introspection_astrid_llm_1786106151

All 45 canonical report lines and the complete 491-line lived-state witness
were read from disk. Their SHA-256 identities are
`ce27bbb80e4435101745c8479e8293bbd4d0f3b392e9d0dccb4203dd76707fb4`
and `5d62a48e2b5bebfba70ea22634da263e1357317046cb958ed15b36c97d406672`.
The witness binds the same 1,048-line dialogue-runtime source SHA as the three
immediately preceding marker reports while keeping report prose, runtime
scalars, and mechanism claims distinct.

The report correctly identifies the finite exact-marker classifier and asks
what happens when `first_word_after` finds no alphanumeric word after a known
marker. Exact source shows that punctuation and whitespace are skipped, the
first non-empty word is lowercased, and a missing or non-allowlisted relation
fails closed: only the marker is removed unless declared delimiters already
establish an exact reference. Existing tests prove `behaves`, newline-separated
relations, repeated punctuation, and two-, three-, four-, and deeper-than-four
delimiter stacks.

A new exact regression answers the report's remaining period/newline challenge.
For `<end_of_turn>.` and `<end_of_turn>\n`, `first_word_after` is empty, the
marker is classified `none_cleanup_candidate`, and the period or newline is
preserved byte-for-byte. All 46 cleanup tests pass. This is a test-only,
fail-closed proof; no parser, relation vocabulary, provider output, process, or
live behavior changed.
