Private qualification harness. Not deployed; real cases have not been executed by the implementing agent. All files and state belong beneath this directory. The reviewed candidate source remains unchanged.

`driver.py prepare` only reads/copies the pinned source closure. `driver.py run` launches one actual NumPy service child and runs real local WebSocket clients. Do not invoke `run` until root explicitly releases it after the observers finish. The service child and protocol client contain lazy real-package imports; stdlib tests do not import either real service or network/numeric packages.

Run the low-cost tests with:

```bash
python3 -I -B /Users/v/other/astrid/.runtime/mlx-upgrade/reservoir-snapshot-candidate/qualification-harness/test_harness.py
```

After source review/freeze, prepare private immutable inputs with:

```bash
python3 -I -B /Users/v/other/astrid/.runtime/mlx-upgrade/reservoir-snapshot-candidate/qualification-harness/driver.py prepare \
  --dependency-root /Users/v/other/neural-triple-reservoir/workspace/model-environments/20261015-mlx-upgrade/core/source \
  --preparation /Users/v/other/astrid/docs/steward-notes/2026-10-15-mlx-upgrade/preparation.json \
  --output /Users/v/other/astrid/.runtime/mlx-upgrade/reservoir-snapshot-candidate/qualification-harness/prepared-01
```

The first real case, only after release, is:

```bash
/Users/v/other/neural-triple-reservoir/.venv/bin/python -I -B \
  /Users/v/other/astrid/.runtime/mlx-upgrade/reservoir-snapshot-candidate/qualification-harness/driver.py run \
  --manifest /Users/v/other/astrid/.runtime/mlx-upgrade/reservoir-snapshot-candidate/qualification-harness/prepared-01/sources.json \
  --variant original --case normal --execute-real-cpu \
  --output /Users/v/other/astrid/.runtime/mlx-upgrade/reservoir-snapshot-candidate/qualification-harness/runs/01-original-normal
```

Review `result.json`, `child.json`, `service.log` and retained snapshot evidence before advancing. A single case waits for the unchanged 60-second periodic timer, then checks final shutdown and two restore cycles. Each constructs the actual 192-node NumPy backend and CPU readouts; these are not free background observations. One actual service/listener runs at a time. Client and child PID, OS start time, full process argv and exact interpreter launcher and macOS framework runtime paths/hashes are retained and checked across the listener lifecycle; no check requires an exited child to remain alive. Explicit child shutdown uses a private pipe; no signals or forced termination are used. A watchdog failure retains the owned child and an unresolved-run guard instead of allowing another run. Never manually overwrite `active-run.json` to hide an unresolved child.

Subsequent invocations use the same command, changing only the following arguments and output suffix. There is no automatic matrix runner or automatic retry:

| Order | Variant | Case | Output suffix |
| --- | --- | --- | --- |
| 2 | candidate | normal | 02-candidate-normal |
| 3 | original | aggregate | 03-original-aggregate |
| 4 | candidate | aggregate | 04-candidate-aggregate |
| 5 | candidate | aggregate | 05-candidate-aggregate-reverse |
| 6 | original | aggregate | 06-original-aggregate-reverse |
| 7 | original | single | 07-original-single |
| 8 | candidate | single | 08-candidate-single |
| 9 | original | writer-error | 09-original-writer-error |
| 10 | candidate | writer-error | 10-candidate-writer-error |
| 11 | original | cancel | 11-original-cancel |
| 12 | candidate | cancel | 12-candidate-cancel |
| 13 | original | cancel-delayed-final | 13-original-cancel-delayed-final |
| 14 | candidate | cancel-delayed-final | 14-candidate-cancel-delayed-final |

Every case starts with 102 fresh synthetic NumPy handles. Aggregate delay is blocking `20/102` seconds at each real writer boundary; the single-save negative control is 3.2 seconds. Native client deadlines remain `open_timeout=2` and `recv(timeout=5)`; proxy routing is explicitly disabled for this private loopback fixture. The external client must start within 0.5 seconds of its trigger (a test-alignment requirement), and successful aggregate-case acknowledgements must occur during the batch. Original aggregate handshake failures and both single-save handshake failures are expected characterization, never upgrade acceptance. No timeout is changed to make them pass.

The oracle checks every NPZ key, array descriptor and thermostat payload against pre-write state; only timestamps have explicit bounds. The fixture holds 102 × 3 × 48 × 192 float32 thermostat values: 11,280,384 bytes (10.76 MiB), plus 235,008 bytes of hidden states and small metadata. The backend weights are shared across handles. Expected evidence retains only field descriptors and thermostat hashes; full arrays/JSON are compared one handle at a time and raw copies stream to disk. Restore cycles start only after the private event loop closes, and each restored service object is released before constructing the next, avoiding three simultaneous full handle sets. These figures describe fixture arrays, not the total process: actual NumPy/Torch/sklearn imports, readout training and allocator peaks remain unmeasured until released execution.

Raw files are retained under `snapshots/seed`, `periodic`, `final` and `roundtrip`, with hashes in the child receipt. Instrumentation performs additional reads and copies on the service loop equally for both variants; its timings characterize this instrumented fixture, not production disk throughput. Restore checks use actual backend/persistence code and confirm event-sequence continuity. Logs are retained without a blanket warning-string rejection rule; exact complete state and thermostat comparisons catch missing/quarantined restores. A caught writer exception cannot become success simply because the service continues: the receipt checks the exact intended error and prior-file preservation.

When all fourteen cases are complete, `review_matrix.py --receipt <absolute result.json> ... --output <new absolute review.json>` rechecks the raw artifact hashes, complete case coverage, no overlap, same source/package/fixture/harness identities and original/candidate then reversed aggregate order. Its acceptance applies only to the private NumPy fixture. Any harness change requires a new prepared source manifest and fresh cases; keep old evidence.

Pending until real execution/review: actual package API behavior, fixture cost, trigger alignment, NumPy round-trip equality, and measured network responsiveness. The CPU fixture does not cover MLX-backed handles, current production history, safe production replacement, or the MLX upgrade. A single long save remains a known limitation. No missing generation state is recovered.
