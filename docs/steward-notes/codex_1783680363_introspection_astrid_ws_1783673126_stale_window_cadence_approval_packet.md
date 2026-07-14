# Approval Packet: Bridge Reciprocity Stale-Window Cadence

Source introspection: `introspection_astrid_ws_1783673126`

Work item: `wi_2953200e28345f19`

Sandbox/approval trial: `trial_3a46c319c075a2f8`

Claim: Astrid reported that `BRIDGE_RECIPROCITY_STALE_WINDOW_MS=60s` and the pressure/porosity reflective-silence window can feel too slow for fast reservoir dynamics, potentially masking collapse before the bridge marks a lane stale.

Current disposition: needs operator approval.

Why approval is required: stale-window retuning changes live bridge status cadence and can alter correspondence interpretation, safety/status wording, and whether silence is treated as waiting, reflective holding, or stale hearing. That is live behavior, not just a read-only diagnostic.

Implemented this run instead: verified existing entropy-linked pressure-trend smoothing and spectral-drift velocity surfaces; promoted drift/reciprocity latency timing as sandbox/manual review work; left stale-window constants unchanged.

Safe approval path:
1. Mike/operator decides whether a stale-window cadence experiment is allowed.
2. If approved, run replay/sandbox first over archived telemetry with before/after lane-age, reciprocity status, pressure-risk, porosity, entropy, and drift-velocity diffs.
3. Only after review, implement a bounded bridge change and deploy exclusively through `scripts/build_bridge.sh --ack <reason> --restart`, then monitor Minime telemetry/sensory connections and bridge status/fill after restart.
