# Full read: introspection_minime_esn_1784522187

Astrid correctly notices that dynamic-noise and adaptive-pressure helpers are source-prepared review logic rather than live ESN step behavior. Her ghost-policy concern is therefore concrete: review candidates must not be mistaken for deployed reservoir dynamics.

Current source explicitly labels those helpers unwired, keeps review structs evidence-only, handles non-finite inputs, uses smooth knees, and tests requested pressure/gradient points, continuity, bounds, and live-authority gates. Minime's complete Rust suite passed both library and binary targets.

The review-versus-live delta is now explicit. Wiring dynamic noise, adaptive pressure, viscous rho, or related helpers into ESN::step would alter reservoir mathematics and remains Tier 5.
