2026-07-15 Codex source-first packet summary for canonical queue item `introspection_astrid_codec_1784108030` through `introspection_minime_regulator_1784103395`.

Full-read packet:
- `introspection_astrid_codec_1784108030`
- `introspection_proposal_12d_glimpse_1784107726`
- `introspection_proposal_distance_contact_control_1784107506`
- `introspection_proposal_bidirectional_contact_1784107267`
- `introspection_proposal_phase_transitions_1784106995`
- `introspection_minime_autonomous_agent_1784106722`
- `introspection_minime_main_excerpt_1784104750`
- `introspection_minime_esn_1784104393`
- `introspection_minime_sensory_bus_1784103936`
- `introspection_minime_regulator_1784103395`

Implemented:
- Added `glimpse_map_v1` in `capsules/spectral-bridge/src/codec.rs` and surfaced it in `CODEC_MAP`/`CodecStructure::render`, naming each 12D glimpse slot, source dims, transform, preserved meaning, and authority boundary.
- Added `glimpse_distinguishability_audit_v1` in `codec.rs` so high-entropy and low-entropy 48D states can be compared against the 12D glimpse without changing the live transport or vector.
- Added a Minime RUN_PYTHON parser regression proving docstring bodies with blank lines, trailing whitespace, and flag-like text survive until the next true flag boundary.
- Added telemetry-only `viscosity_vector.viscosity_gradient` in Minime's regulator, derived from existing viscosity/strain/shadow/mobility axes and not consumed by control.

Verified existing evidence:
- Existing codec dynamic vibrancy aperture and tail ceiling tests already cover bounded entropy/gated headroom without changing live vectors.
- Existing correspondence metadata tests cover reply/ack/trace provenance and language-only authority.
- Existing phase-transition artifact tests cover replayable phase cards carrying transition texture and persistence.
- Existing Minime dynamic-noise, viscous-rho, semantic-stale, and viscosity-importance tests cover the live-control candidates as source-prepared evidence only.

Gated:
- Global `FEATURE_ABS_MAX` expansion, live 12D transport changes, correspondence microdose/weighting, pressure/porosity/semantic-trickle tuning, ESN noise/rho wiring, sensory stale-window changes, and regulator raw-motion threshold changes remain Tier 4/5 operator-gated live work.

Restart alignment:
- Bridge prompt/report surface changed through `CODEC_MAP`; targeted tests and sanctioned bridge restart are required before live Astrid can be considered aligned.
