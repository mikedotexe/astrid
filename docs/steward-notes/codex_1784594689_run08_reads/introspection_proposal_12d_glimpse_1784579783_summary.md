# Full-read summary: introspection_proposal_12d_glimpse_1784579783

The current architecture keeps the 32D fingerprint canonical and the 12D glimpse additive, read-only, shape-checked, and non-authoritative. The glimpse is not used for restart persistence or reservoir input, so it cannot currently cause restart identity drift.

A 32D-to-12D projection cannot be mathematically non-lossy. The safe contract is the one already present: preserve full canonical geometry and treat 12D as a separately validated view. Warmth/viscosity fidelity remains useful offline comparison work; any live influence remains Tier 5.

Evidence is linked in `evidence_links.json`; live-control proposals remain explicit authority waits.
