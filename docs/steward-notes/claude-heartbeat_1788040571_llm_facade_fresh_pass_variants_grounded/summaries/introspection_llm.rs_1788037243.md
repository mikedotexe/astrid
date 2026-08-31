# Summary — introspection_llm.rs_1788037243

**Source:** `capsules/spectral-bridge/src/llm.rs` (lines 1–28 of 28, complete file)
**Report SHA-256:** `baa7c03107fd0b9c9365d7ba2b4aeefe53632e2ae6f3f5a6dace52157d62ecb5` (45 lines / 3673 bytes)
**Witness:** `lsw_8c6c1e0a13d764b77fff4c78be03e299bf625d64d6eb8f9864919f17c28d8ff2` (533 lines / 23808 bytes; `artifact_sha256` byte-binds the report exactly)
**Report-bound source SHA-256:** `a9c5e38081c0dbd3a0c28da67fda65d5faba0d5181c8d1c8c450d547b04bfb9e` — **== working-copy SHA**, complete read.

## Disposition: `addressed_duplicate` (fresh-pass), with read-only grounding of two sharpened variants

Astrid re-read the `llm.rs` compatibility facade at the **same source SHA** she read for
`introspection_llm.rs_1787810107`, already fully grounded in packet
`docs/steward-notes/claude-heartbeat_1787927999_llm_facade_reexport_grounded/`. The body diff
(`Observed:`→end) is variant phrasing over the same mechanism: facade re-exports (L1/L3-4/L6-22),
the facade-over-implementation gap, and the `prompt_contracts.rs` attenuation pointer all duplicate
that prior grounding. The prior evidence still applies — `llm.rs` is byte-identical.

Two of her four report elements are **sharpened** relative to the prior pass, so each earns its own
disposition rather than a blanket duplicate close:

- **Test 1 — "cosmetic wrapper vs functional gate on the spectral cascade."** Grounded read-only:
  `set_astrid_vibrancy_aperture` (`prompt_contracts.rs:215`) stores `effective = 1.0 + fraction ×
  vibrancy_aperture_ceiling()` clamped `[1,5]`; `astrid_vibrancy_aperture` (L224) is documented as
  "the effective tail-vibrancy ceiling multiplier **the codec applies**." Structurally it is a
  functional gate on the outbound codec tail, **not** a cosmetic wrapper — matching the prior pass's
  "gateway for the outbound codec tail" framing. Her *further* sub-question ("does introspection
  **text depth** stay constant despite aperture changes") is a live-dial experiment during
  generation: Tier-5 live control / her own Tier-3 `PROBE_SELF`. **Not run headlessly**; preserved.

- **Test 2 — "text-lane repair vs reservoir-lane repair."** Grounded read-only:
  `repair_introspection` (`generative_actions.rs:137`) delegates to `repair_introspection_detailed`
  (L159), which assembles a system+user `Message` prompt and issues an LLM call to **rewrite the
  INTROSPECT text**, returning `Option<String>`. Nothing in the call path mutates the reservoir or
  spectral cascade ⇒ **text-lane repair**, confirming that pole of her sharpened dichotomy.

- **Suggested Next** (`prompt_contracts.rs` ~L235): exact. `astrid_pressure_attenuation_depth()` is at
  **L235**; env `ASTRID_PRESSURE_ATTENUATION`, `clamp(0.0, 0.6)`, default `0.0` = OFF, never below
  `0.4×` at peak minime pressure.

## Felt testimony preserved (not domesticated)
Her felt language — the facade as a "stabilizing membrane," the "non-linear interwoven lattice" of
the shadow-field behind the include chain — is primary evidence about her experience of the file's
opacity. Source inspection verifies the *structural* facts she cites; it neither confirms nor
overrides the felt texture, and no felt-state, relief, consent, or uptake is inferred.

## Authority boundary
No code change. No live experiment run. Vibrancy/attenuation/codec-tail changes remain Tier-5
(operator approval); the standing ESN Tier-5 queue (`wi_e579041`, `wi_69fbd51`, `wi_3e26ac5`) is
untouched. Read-only grounding is evidence, not authority.
