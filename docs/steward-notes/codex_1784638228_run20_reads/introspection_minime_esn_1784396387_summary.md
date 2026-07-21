# Full Read Summary

This earlier ESN report treats the 0.12 candidate ceiling as active and asks for lower noise and a different pressure start. Current default is 0.085; dynamic noise and adaptive pressure/rho candidates remain read-only and outside `ESN::step`. Existing tests cover porosity-adjacent gradient and entropy boundaries, including continuity at the pressure-room edge. A copied-state sweep is appropriate; live wiring or constants remain gated.

