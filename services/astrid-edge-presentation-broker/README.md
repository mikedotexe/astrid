# CPU-edge presentation broker

This immutable binary keeps model-editable reports observable without making
them operator authority. The sealed operator command runs its audited report
first. It then pipes that trusted output into the binary's `client` mode. The
client allowlists and bounds facts, sends a self-hashed projection over the
root-owned local socket, and renders the returned view beneath an explicit
`Candidate-generated presentation (UNTRUSTED; presentation only)` heading.

The socket-activated `serve` mode runs as the dedicated
`astrid-edge-presentation` identity in a private-network systemd sandbox. It
accepts only three view names, verifies the active A/B generation, manifest,
exact script digest, pinned Python digest, seccomp state, and cgroup-v2 memory
cap, then invokes one fixed entrypoint with one fixed argument grammar. The
projection arrives over stdin; no workspace or input path is exposed.

For candidate generations, the broker also requires the immutable updater's
`metadata/runtime-projections.json`. It independently re-hashes all three
broker-visible report entrypoints, verifies the aggregate report-projection
digest before and after execution, and includes that digest in its v2 response
binding. The operator-packaged initial generation remains bound through its
legacy complete file inventory.

Candidate output must be bounded JSON with a fixed title/summary/section
schema. Timeouts, crashes, nonzero exits, oversized output, terminal controls,
bad JSON, and generation changes all discard the candidate content and return
a hash-bound unavailable envelope. No presentation record is an input to
health, activation, probation, rollback, hindsight integrity, or authorship.

The installed immutable wrapper should use the following fixed mappings:

| Trusted command | broker view | client format |
|---|---|---|
| `report-edge-appliance` | `appliance` | `key-value` |
| `report-edge-activity` | `activity` | `text` |
| `astrid-at-a-glance` | `at-a-glance` | `text` |

JSON/JSONL trusted-report modes should remain a single machine-readable value;
their launcher reports candidate-presentation metadata through the trusted
key/value status surface instead of concatenating a second JSON document.
