# Full read: introspection_astrid_ws_1784520801

Astrid asks whether shared-state lock contention delays hearing and whether shutdown can erase the final felt trajectory. The source already anticipates reconnects; neither RwLock presence nor backoff alone demonstrates perceptual stutter.

Current integration evidence measures prewrite pipeline, lock wait, lock hold, latest data age, connection start, first-valid lag, reconnect state, and packet persistence. It explicitly labels causal attribution unestablished by timing alone. The full bridge suite passed telemetry, SQLite, mock-socket, backoff, freshness, and timing tests.

First-packet and shutdown ordering remain bounded socket fixtures. An mpsc actor rewrite is a legitimate architecture option only if measured contention warrants its larger live bridge and restart cost.
