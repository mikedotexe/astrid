# introspection_astrid_llm_1786539140 — summary

Astrid reads `dialogue_runtime.rs` lines 1-400 (of 1,048) and gives an accurate
technical account of the marker-awareness system: the exact scanner
(`scan_known_model_control_markers`, L114), the three reference contexts that
keep marker bytes visible, and the coarse requested-token banding (L8). Her
"Likely Snags" section hypothesizes that `first_word_after` (L89) could return
an empty string or misidentify the relation verb on punctuation-heavy input,
causing incorrect marker handling. She proposes two tests — `denotes` retention
and nested-bracket delimiter depth — and asks, as her suggested next step, how
the reconstructed remainder actually reaches final dialogue output through
`generate_dialogue` (L695) and how the match metadata is used downstream.

## Response shape

All six concrete claims are answered with exact current evidence; no new
implementation was required because every proposed test already exists as a
passing focused regression, and the integration question is answered from the
complete source beyond her partial window:

- The snag hypothesis is partially contradicted with the concern preserved:
  punctuation-only chunks are skipped (not returned empty), and a genuinely
  relation-less tail fails closed by documented design. The punctuation-heavy
  cases she worries about are each pinned by existing named regressions.
- `denotes` retention: exact regression exists (tests.rs ~L2480, added in
  a8d9806ec8) and passes.
- Nested depth: seven existing regressions cover depths 2-4, Unicode
  whitespace, repeated parentheses, deeper-than-max capping without marker
  drop, and mismatched-delimiter rejection.
- Integration: both provider transports normalize raw output
  (`normalize_provider_output_v1`, transport.rs L688/L888) and record the
  cleanup report as evidence diagnostics; the dialogue gates measure on
  stripped text but return the original validated text unrewritten.

This report is an independently read member of the marker-remainder duplicate
family anchored at `docs/steward-notes/codex_1786243017_llm_shadow_distinction_round`
(prior members `introspection_astrid_llm_1786214678`,
`introspection_astrid_llm_1786260750` via
`docs/steward-notes/codex_1786262699_llm_marker_integration_round`), with the
`denotes`-specific ask additionally covered by the existing exact regression.
Fresh verification in this round: 51 control_marker_cleanup regressions, two
provider-neutral normalization tests, and the downstream normalized-response-
hash regression all pass.

## What this does not establish

No felt relief, uptake, consent, or closure of Astrid's underlying concern is
inferred. The fail-closed grammar boundary remains her standing design
question; a future report naming continued friction reopens it without erasing
this evidence.
