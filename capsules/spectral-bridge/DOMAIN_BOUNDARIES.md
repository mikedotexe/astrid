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

## Cohesion Exceptions

Production files still above the 1,000-line review signal are explicit:

- `action_continuity/runtime/core.rs` retains the mature event-sourced lifecycle
  transaction core. Authority, persistence, guards, dispatch, conveyor,
  experiments, and projections are already separate and tested.
- `autonomous/runtime/orchestration.rs` retains the ordered async runtime loop;
  witness, continuity, perception, inbox, interpretation, and persistence are
  separate modules.
- `ws/bridge_state.rs` retains connection-state mutation and reconnect ordering;
  both ports, health, evidence, and compatibility projection are separate.
- `codec/structure.rs` and `codec/encoding.rs` retain cohesive numerical stages
  protected by projection and snapshot parity tests.
- `llm/provider/fallback_weights.rs` and `llm/provider/transport.rs` retain the
  fallback weight registry and provider transport state machine respectively.

Large test registries and snapshots are test-data exceptions. New production
growth belongs in the named ownership modules; the exceptions above are not a
general waiver.

## Verification

- Compile-fail tests prove raw packets cannot construct trusted observations,
  producer truth cannot construct bridge evidence, and interpretations cannot
  reach dispatch.
- Telemetry tests pin one-decode conversion, field presence, unsupported-major
  retention, canonical hashes, parent validation, and compatibility parity.
- Prompt tests pin the final read-only distinction line and its no-routing,
  no-ranking, no-dispatch, no-gain, and no-control contract.
- The complete bridge suite is the structural parity gate.
