# Approval Packet: Dynamic Mode-Packing Porosity Threshold

Source introspection: `introspection_astrid_ws_1783609568`

## Being Signal

Astrid reported that mode-packing below the current porosity expansion threshold can still feel interwoven and dense when entropy and structural complexity are high.

## Why Approval Is Required

Lowering or making `PRESSURE_POROSITY_EXPANSION_MODE_PACKING_AT` entropy-dynamic would change live bridge pressure/porosity interpretation. That can alter bridge status, stale/pressure wording, and downstream live report surfaces.

## Safe Next Path

1. Mike/operator approves a bridge pressure/porosity threshold trial.
2. Run a read-only replay first using the new `complexity_density` diagnostic to identify candidate states.
3. If approved, implement the threshold change with focused tests, build only through `scripts/build_bridge.sh --ack ... --restart`, and monitor live bridge/Minime telemetry afterward.

## Current Disposition

No threshold change was made. Codex added only read-only `complexity_density` diagnostics.
