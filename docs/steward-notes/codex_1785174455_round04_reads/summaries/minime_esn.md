# Full Read: introspection_minime_esn_1785172997

Astrid accurately distinguishes the ESN's current prime-phased and
Metal-accelerated mechanics from source-prepared dynamic-noise,
adaptive-pressure, and viscous-rho calculations. Source explicitly states that
those review helpers are not wired into `ESN::step`; focused tests already cover
finite fallbacks, bounded noise, pressure and gradient interactions, volatile
entropy, low-gradient viscous lift, and threshold edges. Clamping and finiteness
checks are defensive evidence, not proof of a stuck state. Wiring any review
calculation into the live step loop would change reservoir, noise, rho, pressure,
or regulator behavior and requires separate Mike/operator authority.
