# Cooperative reservoir autosnapshot prototype

This directory is an isolated prototype. It has not been deployed and is not deployment-ready. No live or baseline-bound file was changed, and no actual service module, persistence backend, NumPy, MLX, camera, microphone, or network service was imported or exercised.

`baseline/` preserves the exact original service, persistence writer, and applicable AGENTS instructions. `original-manifest.json` records their original hashes and metadata before editing the candidate.

The patch adds `snapshot_all_cooperative()` for periodic autosnapshot use only. It captures handle names in insertion order once, performs each save with the unchanged synchronous `snapshot_handle()` on the event-loop thread, preserves the ordinary per-handle error log and continuation behavior, and yields with `asyncio.sleep(0)` between handles. New handles are deferred to the next batch. A captured handle removed before its turn follows the existing per-handle exception path. Cancellation propagates at the yield; no worker threads, retry, timeout change, handle deletion, or payload change was added. `snapshot_handle()`, synchronous `snapshot_all()`, and `run()` (including its final shutdown snapshot) are byte-identical.

One slow per-handle save still blocks the event loop. Ready tasks can run between saves, but the prototype does not establish any real handshake latency bound. Handle state can change between snapshots; the batch is not an atomic cross-handle view. Cancellation may leave the periodic batch incomplete until the unchanged shutdown or next periodic path saves remaining handles. Existing background-task cancellation semantics are not improved by this patch.

`tests-01.json` and `tests-01.txt` record ten tiny AST-extracted/fake stdlib tests, run with the reservoir Python 3.12 interpreter and `-I -B`. They validate scheduling within a batch, insertion order, captured-list mutation behavior, error equivalence, cancellation, empty/single batches, unchanged timeout plumbing, and unchanged writer/final path. They do not qualify real persistence or recovery.

Deployment still requires a separately reviewed admission/drain and held-replacement lifecycle, complete preservation/restore qualification, source provenance reconciliation, refreshed preparation/manifests, and a fresh clean two-hour MLX passive baseline. Direct launchd bootout has a five-second termination timeout, shorter than observed approximately twenty-second final snapshot batches; this prototype supplies no safe live handoff mechanism.
