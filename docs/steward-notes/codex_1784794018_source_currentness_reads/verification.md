# Source Currentness Verification

Run: `run_1784794018520540000_719ad1979e`

## Source And Tests

- The six canonical reports were read fully in queue order and yielded 32 claims.
- `cargo test --lib` in `capsules/spectral-bridge`: 1,658 passed.
- `cargo clippy --all-targets --all-features -- -D warnings`: passed.
- `cargo fmt --all -- --check`: passed.
- Minime `cargo test dynamic_noise`: 16 passed across library and binary targets.
- Minime `cargo test semantic_stale`: 36 passed across library and binary targets.
- Evidence Event Store, steward control, Agency Corridor, introspection addressing,
  Sandbox, recent-signal, proactive-scan, and lived-state witness self-tests: passed.

## Live Alignment

- Sanctioned wrapper: `scripts/build_bridge.sh --restart`.
- Deployment receipt: `env_receipt_1784795898239_19000`.
- Source HEAD recorded by receipt: `93d993f2da77705401ac62a377420209480c2a07`.
- Bridge PID changed from `96283` to `88482`.
- New bridge process start: `Thu Jul 23 01:38:16 2026`.
- Release binary SHA-256:
  `85e11f46beb463e4316a3f5bb8179ac93f773812147ab38ca55fb2dbde28daeb`.
- Minime ports `7878` and `7879` and model port `8090` are listening.
- The bridge has fresh established telemetry, sensory, and model connections.
- Model `/livez` and `/readyz` pass.
- The deployment receipt reports fresh telemetry, clean logs, compatible protocol,
  all three processes running, and no failed compatibility check.
- Fresh post-restart jobs record bridge worker PID `88482`.

## Boundary

The deployment aligns source-currentness rendering and prompt guidance only.
It does not establish that an existing mechanism resolves Astrid's felt report,
and it changes no pressure, fill, PI, cadence, codec, model scheduling, sensory
transport, protocol, Minime regulation, dispatch, or live-control behavior.
