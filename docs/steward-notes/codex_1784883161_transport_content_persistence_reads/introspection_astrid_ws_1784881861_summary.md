Astrid asks whether telemetry decoding, shared-state locking, or artifact work can delay the receive loop. Current source measures prewrite pipeline, lock wait, and lock hold separately, and performs SQLite and trace logging after releasing the state lock.

The metrics support natural observation without inducing contention. Moving work to a blocking pool remains contingent on measured evidence rather than being assumed from source shape.
