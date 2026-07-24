Astrid identifies a real source-level risk: the WebSocket receive loop awaits a telemetry handler that performs artifact scans and synchronous persistence, so downstream work can delay the next receive. Existing timing evidence stops before all post-lock persistence.

The concern warrants natural observation and a focused architecture proposal. This run does not induce stalls or refactor the live loop. Any receive-loop decoupling must preserve ordering, backpressure, protocol behavior, and exact runtime alignment.
