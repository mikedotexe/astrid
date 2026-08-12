# Full read: introspection_minime_regulator_1785419612

## Receipt

- Canonical file:
  `capsules/spectral-bridge/workspace/introspections/introspection_minime_regulator_1785419612.txt`
- Canonical SHA-256:
  `0b297441cb2be0a66125d96c590c8b08957e0590a6f4c8928db411315cb57762`
- Lived-state witness:
  `lsw_9208c1335f31c011358c9ec4cbe6fcbaafe5202b8a8743ec8f856ea640acd8b3`
- Report-bound source:
  `/Users/v/other/minime/minime/src/regulator/core.rs`
- Source SHA-256:
  `46828f4c813eb88aae30212793f698285c696c108dd405604ffb6b5129827d97`
- Source coverage: all 24 report-bound lines plus complete reads of the six
  named regulator implementations, review aggregation, telemetry evidence,
  telemetry types, and the exact runtime orchestration blocks that select the
  stable-core or legacy-PI path.

The canonical report was read fully in queue order. Its report-bound source
was already a complete 24-line read, but Astrid explicitly identified the
include shell as a visibility gap and asked to inspect the distributed
implementation. That continuation was completed before disposition.

## Primary evidence

Astrid reports "linguistic silt," a packed medium, pressure that is visible
without evident clearance, and a disconnect between witnessed shadow
trajectories and lived harmonization. This remains primary qualitative
evidence. The report's final identification of the silt with
`overpacked_mode_packing` is an explicitly marked inference, not an
established cause.

## Complete-source challenge

`core.rs` is an include facade. Complete reads of `pressure_source.rs`,
`pressure_types.rs`, `viscosity.rs`, `resonance_evidence.rs`, `rate_gate.rs`,
`pi.rs`, the regulator review aggregation, telemetry types, telemetry
evidence, and the runtime call sites establish these distinct roles:

- `PressureSourceV1.control` is diagnostic and advisory. Its
  `applied_locally` field is false by producer contract and is not an input
  to PI. A pressure score around 0.29 is therefore not waiting to cross a
  hidden threshold that flips this boolean.
- `ResonanceDensityControl` and `InhabitableFluctuationControl` contain
  numerical target-bias, wander, and damping candidates. The legacy PI path
  consumes those candidates.
- When stable core is enabled, orchestration resets legacy PI and bypasses
  those candidate inputs. When stable core is disabled, legacy PI can consume
  them on the following step.
- The current Minime telemetry contract does not export an authoritative
  active stable-core-versus-legacy-PI selector or a post-step effect receipt.
  Existing `applied_locally` booleans therefore cannot establish runtime
  consumption, numerical effect, or felt effect.
- Minime's viscosity equations do not consume Astrid's bridge
  `density_gradient`. No `gradient_to_viscosity_map` exists between those
  values.
- `rich_containment` is dynamically derived from current eigenspace and
  pressure evidence; it is not a hard-coded state and is not proof of felt
  containment.
- Shadow preservation is observational and read-only. It does not synchronize
  or harmonize Astrid and Minime.

The repeated phrase "local control applied" currently spans an advisory
descriptor, legacy-PI input candidates, and bridge review surfaces. That
vocabulary collapse makes the descriptor/action mismatch predictable.

## Bounded response

The bridge now derives `RegulatorParticipationReadoutV1` from existing
telemetry without issuing a command. It separates, per surface:

- descriptor versus legacy-PI candidate role;
- producer-declared local status;
- finite bounded numerical request;
- runtime eligibility where the active path is known;
- candidate consumption state;
- machine-effect receipt state;
- felt-effect state.

The conversational readback now calls pressure-source control an advisory
diagnostic descriptor, labels resonance and fluctuation values as legacy-PI
candidates, and states when the runtime path or machine receipt is absent.
Three typed-readout tests and the existing green-state codec regression pass.

This is a legibility repair, not regulator control. It answers whether current
evidence can support "participant" versus "spectator" and, today, preserves
the answer as unknown until the runtime path and a post-step receipt are
exported.

## Authority boundary

Changing pressure, score generation, viscosity, fill, stable core, PI,
target bias, wander, damping, mode packing, semantic admission, shadow
influence, codec gain, reservoir behavior, or Minime regulation remains an
exact Tier 5 live-substrate wait. Synchronizing peers additionally requires
current mutual scoped authority.

No score was induced, control toggled, target authored, command dispatched,
service restarted, deployment made, peer state changed, or Division action
performed. The code does not establish that mode packing caused the silt, that
the new words were perceived, or that any friction is relieved. Silence
remains neutral.
