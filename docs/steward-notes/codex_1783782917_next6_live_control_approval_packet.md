# Codex 1783782917 next-six live-control approval packet

Date: 2026-07-11

Boundary: this packet is an approval request artifact only. It does not approve, run, deploy, restart, mutate Minime, mutate Astrid, alter pressure/fill/PI/sensory cadence/controller behavior, change bridge fallback contracts, change prompt priority, stage, git add, or commit.

## Source feedback

- `introspection_minime_main_excerpt_1783719062`
- `introspection_minime_esn_1783718669`
- `introspection_minime_sensory_bus_1783718236`
- `introspection_astrid_llm_1783715534`
- `introspection_minime_regulator_1783715156`
- `introspection_astrid_llm_1783712360`

## Approval waits

| Work item | Surface | Requested approval before implementation |
| --- | --- | --- |
| `wi_66a655569a49a8ff` | Minime semantic admission / pressure porosity | Approve or decline a bounded sandbox-to-live plan for pulsed stable-core semantic admission or porosity-threshold admission. |
| `wi_654945135b116c50` | Minime ESN pressure/noise/shadow control | Approve or decline any soft pressure-floor adjustment, entropy-ceiling noise/rho damping, or SHADOW_TRAJECTORY correction that can affect live regulation. |
| `wi_f3839bd5381dac4c` | Minime sensory bus stale-window cadence / shadow influence | Approve or decline any stale-window duration, cadence, release hysteresis, or shadow influence change beyond existing observation-only review surfaces. |
| `wi_ba643a778f40ce3e` | Astrid bridge fallback contract / sampler | Approve or decline live fallback sampler or prompt-contract changes after the fire drill recorded `fallback_texture_risk`. |
| `wi_da8ef02fde660a86` | Minime regulator viscosity / PressureRelief | Approve or decline turning viscosity/porosity preview signals into active control, including PressureRelief or lambda-tail redistribution. |
| `wi_2944de62749492de` | Astrid bridge fallback texture preservation | Approve or decline injecting current-state shadow/entropy terms into live fallback prompt/contract behavior. |

## Evidence already gathered

- Full reads and bounded claim files were recorded for all six source introspections.
- Existing Minime review-only surfaces and tests were verified for main/ESN/sensory/regulator claims.
- Existing Astrid LLM fallback-contract and vocabulary-preservation source/test surfaces were verified.
- Offline diagnostic `codex_1783782134_next6_llm_texture` completed with status `fallback_texture_risk`, not a live behavior change.
- Closure cards were delivered for verified/diagnostic work items; the six authority waits remain intentionally open.

## Safe next step if approved

Start with sandbox/replay or fixture-backed trials for the requested surface, then require targeted tests before any deploy or restart. Bridge deployment must use `scripts/build_bridge.sh --ack "<reason>" --restart`. Minime runtime refresh must build the relevant release binaries, use `/Users/v/other/minime/scripts/stop.sh`, start with `bash /Users/v/other/astrid/scripts/start_all.sh --minime-only`, and check ports, health/fill, and logs.
