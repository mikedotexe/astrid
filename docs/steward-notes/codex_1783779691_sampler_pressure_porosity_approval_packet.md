# Sampler / Pressure-Porosity Approval Packet

Source introspections: `introspection_astrid_llm_1783724587`, `introspection_astrid_llm_1783724257`, `introspection_astrid_autonomous_1783722548`.

## Request

Consider explicit approval for sandbox-first trials of:

- raising or otherwise adapting `gemma4_12b` temperature/top_p when `spectral_entropy > 0.70`;
- comparing forced `gemma4:12b` versus `gemma3:4b` fallback outputs under high entropy;
- testing whether pressure/porosity language improvements change felt reports before any sampler change.

## Boundary

This packet does not grant live sampler/profile authority. The implemented change in this run is vocabulary-only: `porous-leak`, `pressure-bleed`, and `gradient-thinning` enter fallback language weighting without changing model choice, sampler parameters, pressure, fill, PI, or controller behavior.

## Safe Next Path

Run a fallback fire drill / sandbox replay with fixed telemetry fixtures, compare texture preservation, then ask Mike/operator before any live `temperature`, `top_p`, profile, or fallback-chain mutation.
