# Full-read summary: introspection_minime_esn_1784577137

The 0.12 and 0.70 density-gradient values are endpoints of a smoothstep window, not an abrupt jump. Source-prepared dynamic noise is bounded and deliberately not wired into `ESN::step`; review packets expose linear versus smoothed room and tests cover continuity and boundedness.

A current-versus-candidate comparison is safe offline work. Changing thresholds, active exploration noise, or introspection scheduling remains Tier 5.

Evidence is linked in `evidence_links.json`; live-control proposals remain explicit authority waits.
