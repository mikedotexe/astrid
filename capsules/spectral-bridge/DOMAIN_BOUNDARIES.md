# Spectral Bridge Domain Boundaries

The bridge preserves its established public module paths through thin facades
while implementation ownership lives in named submodules. This extraction is
behavior preserving: it changes neither pressure, fill, PI, sensory cadence,
codec gain, admission, controller behavior, nor live authority.

## Stable Facades

| Facade | Canonical ownership |
| --- | --- |
| `ws.rs` | telemetry and sensory ports, bridge state, evidence, compatibility projection, health |
| `autonomous.rs` | witness, continuity, perception, inbox, interpretation, persistence, journal, orchestration |
| `codec.rs` | projection, evidence, encoding, structure, feedback, visual rendering |
| `llm.rs` | provider transport, prompt/dialogue rendering, fallback evidence and budgets, research, embeddings |
| `types.rs` | compatibility schemas split by telemetry, texture, transport, resonance, clamp, control, and tests |
| `action_continuity.rs` | authority, persistence, guards, dispatch, conveyor, experiments, investigations, orchestration |

Every facade is below 1,000 lines and re-exports the established symbols. The
typed provenance path is canonical for interpretation. `SpectralTelemetry` is
created only by the compatibility projection and remains allowlisted for audio,
visualization, status, and MCP consumers.

## Provenance Ownership

- Port 7878 decodes once into `MinimeObservationV1` plus field-presence and wire
  receipt metadata.
- Residual deformation, temporal variance, gradients, flux, smoothing, and
  constraints belong to `BridgeEvidenceV1`; producer DTOs remain immutable.
- `AstridInterpretationV1` cites observation and evidence parents and cannot
  enter sensory dispatch.
- `WitnessFrameV1` joins those typed records for read-only rendering. The
  `witness_self_other_distinction_v1` line is context only.

## Shadow Cartography Ownership

- `autonomous/next_action/shadow.rs` owns action dispatch, influence preflight,
  density-gift gating, and the existing live-control boundary.
- `autonomous/next_action/shadow/trajectory.rs` owns being-invoked trajectory
  rendering and the observational history-bearing lens.
- `autonomous/next_action/shadow/cartography.rs` owns response, dialogue, and
  coupling renderers. These modules write cartography only; they do not change
  Shadow state, pressure, mode packing, temporal decay, scheduling, or control.

## Cohesion Exceptions

Seven ownership-critical production files above the 1,000-line review signal
have documented cohesion exceptions:

- `src/action_continuity/runtime/core.rs` retains the mature event-sourced lifecycle
  transaction core. Authority, persistence, guards, dispatch, conveyor,
  experiments, and projections are already separate and tested.
- `src/autonomous/runtime/orchestration.rs` retains the ordered async runtime loop;
  witness, continuity, perception, inbox, interpretation, and persistence are
  separate modules.
- `src/ws/bridge_state.rs` retains connection-state mutation and reconnect ordering;
  both ports, health, evidence, and compatibility projection are separate.
- `src/codec/structure.rs` and `src/codec/encoding.rs` retain cohesive numerical stages
  protected by projection and snapshot parity tests.
- `src/llm/provider/fallback_weights.rs` and `src/llm/provider/transport.rs` retain the
  fallback weight registry and provider transport state machine respectively.

Large test registries and snapshots are test-data exceptions. New production
growth belongs in the named ownership modules; the exceptions above are not a
general waiver.

Other pre-existing production files above the review signal remain explicit
review debt in `domain_boundaries_legacy_large_files_v1.json`. That baseline is
a zero-growth ratchet, not an exemption or a claim of good cohesion: a file may
shrink out of it, while a new large production file or growth beyond its
captured boundary fails `scripts/domain_boundary_audit.py verify`.

## Verification

- Compile-fail tests prove raw packets cannot construct trusted observations,
  producer truth cannot construct bridge evidence, and interpretations cannot
  reach dispatch.
- Telemetry tests pin one-decode conversion, field presence, unsupported-major
  retention, canonical hashes, parent validation, and compatibility parity.
- Prompt tests pin the final read-only distinction line and its no-routing,
  no-ranking, no-dispatch, no-gain, and no-control contract.
- `python3 scripts/domain_boundary_audit.py verify` pins stable facade targets,
  documented exception ceilings, the legacy large-file ratchet, and forbidden
  interpretation/witness-to-dispatch dependency edges.
- The complete bridge suite is the structural parity gate.
