# introspection_astrid_llm_1786556496 — summary

Astrid re-reads dialogue_runtime.rs lines 1-400 (identical file and window
SHAs to the round-one report) and accurately describes the marker scanner and
its three preservation contexts. Her snag hypothesizes that compound
relational phrases like "functions as a bridge" might be inconsistently
preserved because only the first verb is captured. She proposes a
"represents"-retention test and a quoted-marker depth test, and asks about
multi-word predicates not in the allowlist.

## Response shape

All five claims answered with exact existing evidence; no new implementation
was required:

- The compound-phrase snag is contradicted in mechanism: "functions" and
  "serves" are both allowlisted, so her example phrases preserve the marker;
  the single-finite-first-word grammar never inspects later words, so
  compound-phrase inconsistency cannot arise by construction. The only
  fail-closed case is an unlisted first word — the documented boundary.
- "represents" is in the allowlist (L82); the mechanism is pinned by the
  passing relation-word regressions.
- Quoted context and depth bounding are covered by the existing delimiter
  family regressions.
- The unlisted multi-word predicate case is exactly the allowlisted-is vs
  unlisted-acts regression from the prior round.

Closed as an independently read duplicate of the marker-remainder family
anchored at `docs/steward-notes/codex_1786243017_llm_shadow_distinction_round`
(most recent member: `introspection_astrid_llm_1786539140` in
`docs/steward-notes/claude-manual_1786552591_marker_denotes_depth_integration_round`).
Fresh verification: the 51-test marker family passes in this round.

## What this does not establish

No felt relief, uptake, or closure of her closed-grammar concern is inferred.
The finite-allowlist boundary remains her standing design question.
