# Full read: introspection_astrid_llm_1784515091

Astrid worries that artifact cleanup is a brittle sequence of string replacements and asks for token-aware parsing plus nested-quote tests. Current source has already replaced that failure mode with a single-pass longest-raw-marker scanner that cannot create second-order markers and preserves ordinary words containing artifact-like substrings.

Run 13 adds the missing nested-quote regression: a marker inside inner straight quotes and outer smart quotes is counted as contextual and quoted, with the semantic remainder preserved. Existing tests cover exact byte accounting, quoted and boundary placement, delicate context, structure-only remainders, and common-language preservation.

No prompt, model output, marker set, or runtime cleanup behavior changed.
