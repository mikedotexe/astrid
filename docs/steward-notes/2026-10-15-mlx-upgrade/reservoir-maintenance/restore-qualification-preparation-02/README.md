# Restore qualification preparation 02

This archive preserves the reviewed per-child admission preparation. It has not run the actual numerical qualification and does not authorize production maintenance.

`candidate/` contains exact source, input manifests, the two guarded CPU test receipts/logs, and the independent source review. Python files have a `.py.txt` suffix so this public evidence is not an executable package. The final guarded admission suite passed 17 tests; the copied earlier qualification suite was adapted but was not rerun. The review records that limit.

`candidate/superseded-cpu-01/` preserves the first passing source and evidence before the narrow initial-release reference fix. These historical files and their manifest remain unchanged; they are not the final accepted source.

To reconstruct for review, copy `candidate/` into a new private directory, excluding `superseded-cpu-01/` from the active import path, and remove only the final `.txt` suffix from names ending in `.py.txt`. Verify every reconstructed file against `archive-manifest.json`. Do not rewrite the preserved manifests or receipts to make their absolute paths match a reconstruction. Their original paths and hashes are historical provenance. A relocated execution would require separately reviewed bindings and authorization.

External interpreters, packages, model payloads, supervisor files, and runtime state are referenced by the preserved manifests but are intentionally not copied. No actual payload or production state is included. The archive alone is insufficient for execution. A future run requires a genuine accepted host window, the exact reviewed supervisor, one immutable run release, and a separate one-use root permit for the seed and each of nine children. Failure or uncertainty stops advancement; no in-flight child is signalled or retried by this qualifier.

The production controller's compatibility with this new admission schema remains separate work. This archive changes no production service, frozen controller, or prior evidence.
