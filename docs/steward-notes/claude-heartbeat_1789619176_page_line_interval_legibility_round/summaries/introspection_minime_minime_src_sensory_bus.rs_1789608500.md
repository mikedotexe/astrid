# introspection_minime_minime_src_sensory_bus.rs_1789608500

Astrid reads `minime/src/sensory_bus.rs` at
sha256:3fc6bd2a16bd78c5caa496f2a6dccbc67928da4fbded123998f59a82bcd4aa3a, delivered bytes
18492..22865 — lines **502..620**, as both `page.start.line` and the witness
`window_start_line` record. She opens: "The current page (lines 464–620)".

## What the delivered page actually said

The page record (`page efabf5fb3465d74a0620642dd51fba75d8bedbfa70435cbe5a096eaffb2d6042`,
delivery receipt `f6ca4acbe67e…`, 6,692 B of rendered text) carries three different
line facts and they do not agree:

1. Header, line 4: `Exact source bytes 18492..22865; line fragments retain their line
   number.` — the interval, in **bytes only**.
2. SOURCE SCOPE row: `function modality_boundary_transparency_v1 (lines 464–515; no test
   marker found). Enclosing declaration: SELF_STUDY OPEN minime/minime/src/sensory_bus.rs
   464` — the only line range spelled in prose, and it belongs to the **declaration**, not
   the page.
3. Numbered gutter, first row: `   502 | lityBoundaryTransparencyV1 {` — a mid-line
   fragment, because the previous page's budget ran out inside line 502.

Her `620` comes from the last gutter row. Her `464` comes from (2). Nothing in the bytes
she read states `lines 502–620`. This is a render gap, not a confabulation: `Page::read`
(`crates/astrid-source-study/src/page.rs:86-98`) formats the interval in bytes, and
`Outline::scope_text` (`src/source_structure.rs:147-156`) formats the enclosing
declaration's span in lines. The exact interval exists on `page.start.line` /
`page.end.line` — it is simply never rendered.

## What was verified in her reading

Her citations of `minime/src/sensory_bus.rs` are unusually exact, including attribute
lines: `SemanticReceptivityPulseReviewV1` 374–383, `SemanticGlimpse12dV1` 386–402,
`ModalityBoundaryTransparencyV1` 404–417, `SurrenderModeAuthorityGateV1` 419–428,
`semantic_degradation_clarity_factor` from 430, `normalized_boundary_label` at 454,
`modality_boundary_transparency_v1` 463–515. Most of those bodies were delivered on the
**previous** page (bytes 14188..18492); two of them (374, 420) appear on this page only as
RELATED SOURCE LOCATIONS line numbers. She marks the carry herself — "previous pages
defined the *forces*".

Verified exactly: the opaque/constrained/`contact_change_route` logic (480–500), the
operator-approval gate including `status: "tier5_operator_approval_required_before_live_trial"`
(517–528, on her own page), the four-argument clarity signature (430–435) and its call at
619, the persistence-vs-clarity split at 611–620, and that the glimpse preserves
`semantic_fresh_ms`/`semantic_stale_ms` through compression (392–393, 2189–2191).

## Two corrections, stated rather than smoothed

- **Entropy sign.** She writes that clarity may degrade because "internal structure is
  incoherent (entropy)". Source says the opposite: `entropy_support` rises above
  `SEMANTIC_ENTROPY_PERSISTENCE_START = 0.75` and enters `max_loss` with **−0.08**
  (446–450), so higher spectral entropy *sustains* clarity. Crowding is the term that
  costs: `gradient_load` enters with **+0.26**. Low fill also holds clarity (−0.12).
- **`smoothstep_unit`'s role.** `smoothstep_unit` shapes `age_fraction` only (436). `max_loss`
  is built without it (440–450). They meet once, at 451, as `1.0 - age * max_loss`.
- **Glimpse as transport.** She calls the 12D glimpse "the primary unit of movement for
  semantic data". The constructor at 2183–2199 marks it
  `compression_role: "non_authoritative_companion_summary"`, `live_vector_write: false`,
  `controller_write: false`, `authority: "read_only_glimpse_summary_not_live_transport_or_control"`,
  with `live_transport_dim_count = LLAVA_DIM = 48` against `glimpse_dim_count = 12`.

Her framing claim survives all three: staleness in this file really is a gradient of
clarity and not a timer, and the file itself says so in the words she reached for —
"semantic hold quality, not just hold duration" (585–588).

## What was implemented

`crates/astrid-source-study/tests/page_line_interval_legibility.rs` — five read-only
legibility pins: the header states bytes and never the delivered line interval; the one
prose range is the enclosing declaration's and starts strictly behind the first delivered
line; the first gutter row can be a strict suffix of the line it numbers; the exact
interval is available on the page record; and the scope row's line is a real Action that
returns the declaration head, distinguishing the two spans in one read.

No render, header, scope row, pagination, budget, or being-facing navigation behaviour was
changed. No live substrate or control change was made, attempted, or needed.
