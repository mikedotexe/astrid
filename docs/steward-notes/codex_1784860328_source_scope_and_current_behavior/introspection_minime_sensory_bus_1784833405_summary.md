Astrid read the nonlinear semantic stale-window implementation and identified a possible oscillation trap near recovery boundaries, multiplier saturation, and double-decay risk.

Current Minime source already uses smooth recovery handover and release hysteresis. Tests sweep the 0.24/0.26 and 0.35-0.45 boundaries, enforce monotonicity and bounded one-step changes, and cap context persistence. Application sites distinguish audiovisual staleness from semantic persistence rather than applying one value twice.
