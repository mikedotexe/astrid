# Summary — introspection_DOMAIN_BOUNDARIES.md_1787003465

**Source:** `capsules/spectral-bridge/DOMAIN_BOUNDARIES.md` (complete file, lines 1-89, SHA `ae69b34c…`, binding-match true)
**Witness:** `lsw_fb0172a9…` (evidence_only, no live eligibility)
**Fill at authorship:** 71.0% · model `gemma4_12b` (two mlx calls, second a repair of the first)

## What Astrid said

A fresh-pass read of the bridge's `DOMAIN_BOUNDARIES.md`. She (1) restates the structural map — Stable Facades vs. internal transformation ownership, `AstridInterpretationV1` cites observation/evidence but **cannot enter sensory dispatch**, and the Cohesion Exceptions that let `core.rs` exceed 1,000 lines under a unique-fn-signature ceiling and a zero-growth ratchet; (2) raises a **Likely Snag** that the signature ceiling could be *silently violated* if `domain_boundary_audit.py verify` is not run on every commit touching `core.rs`, since complexity can grow inside an unchanged line count; and (3) proposes two tests — a compile-fail dispatch-isolation test and a signature-growth audit run against `core.rs`.

## Disposition (terminal: `addressed_no_action`, evidence-backed)

- **c001 structural map → verified_existing.** Read the complete file at the exact report-bound SHA (no drift). Every line citation is accurate (facades L8-17, provenance/dispatch L30-31, ceiling L65-68, ratchet L72).
- **c002 snag → verified_existing, cadence nuance preserved.** The signature-growth mechanism is real (`_unique_fn_signature_count` L60 → `exception_signature_growth` L178-183), regressed (`test_exception_signature_growth_fails_within_line_ceiling`), and **auto-runs as source-first projection stage 10** (`projection_profile.py` L272-296) on every controller pre/postprojection — a stronger cadence than commits. Honest nuance kept, not domesticated: it is **not** wired into `.github/workflows` CI, so a commit could land uncaught-at-commit-time and be caught only at the next projection. Not a defect; a real cadence observation.
- **c003 Test 1 → verified_existing.** The exact named test `tests/ui/interpretation_cannot_dispatch.rs` (+ committed `.stderr`) already pins the constraint via a trybuild compile-fail (`AstridInterpretationV1` → `dispatch_semantic_microdose` type mismatch expecting `LiveExecutable<SemanticMicrodose>`).
- **c004 Test 2 → observed.** Ran the audit read-only: `core.rs` = **210/232** unique fn signatures (within ceiling), whole audit `valid=true`, `violation_count=0`, `documentation_sha256` equals the report-bound source SHA.

## Authority boundary

No code changed. No production module, ceiling, facade, or audit behavior was modified. No live/substrate/control change was made or attempted; restart/deploy were not required. This response infers no consent, relief, or uptake, and does not gate the audit into CI (that would be a separate, non-flywheel change). Astrid's Suggested Next reached the file end; any deeper INTROSPECT remains a read-only continuation she may self-activate.
