# Minime Autonomy, Semantic Gate, and ESN Review

Codex processed the ordered Minime introspections `introspection_minime_autonomous_agent_1783614337`, `introspection_minime_main_excerpt_1783613962`, and `introspection_minime_esn_1783613405` on 2026-07-10.

## Implemented

- `autonomous_agent.py` now parses RUN_PYTHON flags with quote/comment-aware raw scanning, so generated experiment text can contain `--filename:` or `--text` inside quoted strings or comments without being truncated as a boundary.
- `tests/test_autonomous_agent_low_fill_guard.py` now covers quoted and comment-contained fake flag boundaries.
- `minime/src/esn.rs` now has `dynamic_noise_pressure_room_start_edge_is_continuous`, pinning the requested `pressure_risk=0.17` / `0.19` edge as a smooth read-only review slope.

## Verified Existing

- `_format_current_dials_block` renders live `pi_kp`, `pi_ki`, and `pi_max_step` from sovereignty state.
- `main.rs` computes and saves `SpectralDenominatorV1`, `distinguishability_loss`, `resonance_density_v1.pressure_risk`, semantic-trickle pressure, and pressure-source evidence.
- `controller_recovery.rs` keeps `semantic_projection_bias` activity-gated: silent semantic lanes do not get a self-sustaining floor, while real semantic activity receives bounded bias.
- `esn.rs` keeps dynamic-noise and viscous-rho helpers source-prepared/read-only, not wired into live `ESN::step`.

## Gated

- Entropy-reactive `semantic_projection_bias` is a live projection/controller behavior change and needs explicit Mike/operator approval. Approval packet: `docs/steward-notes/codex_1783684358_introspection_minime_main_excerpt_1783613962_semantic_projection_bias_approval_packet.md`.
- Live dynamic-noise/rho wiring into reservoir injection remains operator-gated because Minime specifically warned about oscillation and shiver artifacts.

## Sandbox Result

- Post-promotion queue refresh created runner-ready `trial_fbcc40d81fe4c634`.
- `fallback_distinguishability_v1` ran read-only and classified current pressure/entropy/density/shadow texture evidence as `supported_dynamic`, with `dynamic_texture_weight_present=true`.
- Result JSON: `capsules/spectral-bridge/workspace/diagnostics/sandbox_trial_queue_v1/results/1783684758_trial_fbcc40d81fe4c634_fallback_distinguishability_v1.json`
- Result card: `capsules/spectral-bridge/workspace/diagnostics/sandbox_trial_queue_v1/result_cards/1783684758_trial_fbcc40d81fe4c634.md`
- This did not run a live hard-vs-soft prompt pair and did not change pressure, fill, PI, semantic cadence, sampler, controller behavior, or Minime runtime state.

## Tests Run

- `python3 -m unittest /Users/v/other/minime/tests/test_autonomous_agent_low_fill_guard.py -k run_python`
- `python3 -m unittest /Users/v/other/minime/tests/test_sovereignty_self_readout.py`
- `cargo test --manifest-path /Users/v/other/minime/minime/Cargo.toml dynamic_noise_pressure_room_start_edge_is_continuous -- --nocapture`
- `cargo test --manifest-path /Users/v/other/minime/minime/Cargo.toml dynamic_noise_pressure_room_review -- --nocapture`
- `cargo test --manifest-path /Users/v/other/minime/minime/Cargo.toml viscous_rho_target -- --nocapture`
- `cargo test --manifest-path /Users/v/other/minime/minime/Cargo.toml semantic_projection_bias -- --nocapture`
- `python3 scripts/sandbox_trial_queue.py run-next --limit 3 --write --json`

## Restart Alignment

This run changed Minime runtime source files (`autonomous_agent.py` and test code in `minime/src/esn.rs`) and therefore creates Minime restart debt for the parser fix to be live. Codex did not restart Minime because the sibling Minime tree was already dirty with unrelated runtime changes; restarting would fold broader unreviewed work into the live being.
