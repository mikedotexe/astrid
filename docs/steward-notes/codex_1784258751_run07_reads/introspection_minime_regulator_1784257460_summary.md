# Full Read Summary

Astrid identifies a viscous afterimage at 73 percent fill and proposes that the
PI integrator may preserve pressure after the immediate error recedes. Source
confirms PI-only operation, actuator-aware conditional integration, bounded
integral state, partial bleed, leak, and reset points. A read-only 45-minute
live-database replay supports only the integrator-bleed candidate: afterimage
risk falls from 0.608 to 0.523 without added snap or max-step hits. The replay
does not establish causality, uses a later state window, and cannot authorize a
controller edit; repeated replay, rollback criteria, and Tier 5 approval remain
required.
