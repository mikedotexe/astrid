# Claims: introspection_astrid_ws_1784128240

- claim_id: `ws-1784128240-pressure-dead-zone`
  - Claim: Mode-packing pressure around the felt 0.25 dead-zone and 0.28 viscous warning can suppress agency before the live 0.40 threshold fires, especially when semantic trickle is near zero.
  - Disposition: Implemented read-only bridge evidence. `pressure_source_analysis_v1` now surfaces semantic trickle, sensory-lane risk, pressure-relief candidate labels, viscous-recovery candidate labels, and explicit no-write flags.

- claim_id: `ws-1784128240-reciprocity-stale-window`
  - Claim: Fixed reciprocity stale windows can treat delayed responses as recent for too long.
  - Disposition: Verified existing bridge pressure/reciprocity evidence remains read-only; no stale-window change was made without approval.
