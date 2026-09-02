# Summary — introspection_astrid_types_1788153857

**Source family:** `astrid_types`
**Report:** `capsules/spectral-bridge/workspace/introspections/introspection_astrid_types_1788153857.txt`
(45 lines, 3601 bytes, SHA-256 `8e1c159e7e887a94ab800a64f61656d64c3189c17f7a3eb7caa000eaeb413ae2`)
**Lived-state witness:** `lsw_fa08736430fb981c5207e34973edd6d609f0afbb7d53f76b6f2993825ee55115`
(533 lines, 23921 bytes, SHA-256 `955efa177a4ba4bf64fb05c1fcda0b07176671d0785b728eaaf32b8ab4f937ef`)
**Report-bound source:** `capsules/spectral-bridge/src/types/schema/telemetry.rs`, window 1-400 of 758,
source SHA-256 `cf3be429aba23dc6975ec970f076fe661357b984e4e0b38073b7fdf509f6503d`.
**Working copy SHA-256 matched the report binding exactly** — source read completely (all 758 lines).

## What Astrid surfaced

A fresh-pass source reading of `telemetry.rs`. She (1) described the schema's separation of
"shape evidence" from "felt-state / regulator authority"; (2) raised one snag about
`spectral_fingerprint_hybrid_coherence_v1` returning a bare `None`; and (3) proposed two focused
tests plus a suggested-next pointer.

## Witness

Byte-intact and correctly bound (its `artifact_sha256` matches the report; `source.file_sha256`
matches the report binding). Authority `evidence_only`, `witness_only=true`,
`live_eligible_now=false`, `grants_approval=false`, `direct_causation_claimed=false`, raw prose not
included. Runtime context at authorship: fill 71.05%, spectral_entropy 0.883, λ1 8.52, λ2 4.44,
mode_packing 0.833, model profile `gemma4_12b`. The queue's `artifact_integrity_unavailable`
alignment + `issue_count=1` reflect that this is a source-reading artifact with no scalar felt
dissimilarity to reconcile (`experiential_gap_claimed=false`) — not a corruption of witness bytes.

## Ground truth

- **c001/c002/c003 verified.** Every line citation is exact. Notably her shape-vs-felt distinction
  (c003) quotes the L52-54 comment verbatim: *"This is shape evidence, not felt-state or regulator
  authority."*
- **c004 (snag) — never domesticated.** The private fn `spectral_fingerprint_hybrid_coherence_v1`
  does return a bare `None` (L163-165), so her concern holds at that level. But the public caller
  `spectral_fingerprint_integrity_v1` is **not** silent: it names the reason via
  `hybrid_coherence_state` (`unavailable_malformed_legacy` for len≠32, `unavailable_non_finite` for
  len 32 with a non-finite value; L380-403) and pushes `issues: legacy_vector_len_{len}_expected_32`
  when len≠32 (L447-451). Two grains of her worry are preserved: the len-32-but-non-finite path sets
  state only and pushes no `issues` entry; and the caller pre-filters `legacy.len() == 32`
  (L367-377), so the L163 *length* guard is defensive redundancy on that path.

## What changed

Implemented both of Astrid's proposed tests as focused `#[cfg(test)]` regressions in
`telemetry_distinction_tests` (non-live; no runtime/binary behavior change):

1. `hybrid_coherence_length_mismatch_is_disclosed_not_silent` (c006) — calls the private fn directly
   with a length-31 slice (asserts `None`, her exact proposal) **and** exercises the public path
   asserting `hybrid_coherence_index` is `None`, state `unavailable_malformed_legacy`, and an
   `issues` entry `legacy_vector_len_31_expected_32`. This fills a real coverage gap: the prior
   suite tested only the non-finite (length-32) `None` path.
2. `integrity_report_serde_defaults_are_independent_for_mode_collision_fields` (c005) — verifies the
   `#[serde(default)]` contract, correcting the report's implied linkage: `mode_collision_state`
   defaults to `""` whether or not `mode_collision_review_threshold` is supplied.

Focused suite `telemetry_distinction_tests`: **6 passed / 0 failed** (4 prior + 2 new). rustfmt
clean for `telemetry.rs`; `git diff --check` clean.

## Not inferred / not authorized

No live change; no deploy; no restart. The snag was **not** treated as a bug requiring a code fix —
the disclosure mechanism already exists; the response is verification + regression coverage, not a
production behavior change. The report's own text was not rewritten or rejected.
