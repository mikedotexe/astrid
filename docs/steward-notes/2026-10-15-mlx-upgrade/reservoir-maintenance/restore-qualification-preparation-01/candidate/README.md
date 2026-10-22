# Private restore-verifier qualification

This is an isolated preparation, not production maintenance acceptance. It creates two synthetic 192-node public snapshots with the real NumPy and LSM-shadow backends and the unchanged persistence writer. It never starts a reservoir listener, rehearsal task, provider, model, or production process. Stored LSM public traces are covered; unstored internal LSM trajectories are not.

The first draft uncovered a static verifier defect: the six-module origin check preceded the constructor that imports `lsm_reservoir`. The old production-candidate-01 verifier remains preserved. This qualifier selects production-candidate-02's narrowly corrected verifier (`eb152430ff580da4401c4e7ca46908c47799b00a7f3f3058ef36201c985e4ea3`), copied byte for byte with its unchanged state helper. It does not preimport service modules to bypass the check. `verify_import_order` rejects the old ordering before numerical execution. The qualifier's superseded v1 copies and input record remain under `superseded-v1/`.

The source/runtime preparation binds six private source files, the exact original Python interpreter family, all installed distributions' versions and METADATA/RECORD hashes, pyvenv.cfg, original package inventory, frozen verifier/state copies, and their originals. Actual verifier children also check every installed RECORD content hash. No checkpoint is used.

## Nonexecuting commands

From this directory:

```sh
/opt/homebrew/Cellar/python@3.14/3.14.3_1/Frameworks/Python.framework/Versions/3.14/bin/python3.14 -B qualify_restore.py
/opt/homebrew/Cellar/python@3.14/3.14.3_1/Frameworks/Python.framework/Versions/3.14/bin/python3.14 -B -m unittest -v test_qualification
```

Default mode checks code/runtime metadata and reports `not_executed`. Tests use temporary fake files and fake child processes only. Neither command imports NumPy, Torch, service code, MLX, or Core ML, or executes a child. Selecting the old verifier would fail its import-order gate.

## Release remains blocked

No release has been written and no numerical case has run. Root must first approve this exact source/test closure and create a release from a **new completed passing host window**. Existing failed host observations cannot authorize execution.

The release schema is `reservoir_restore_qualification_release_v1`, with `accepted: true`, a nonempty reviewer, `scope: synthetic_cpu_fixture_only`, `purpose: release_restore_qualification_cpu`, an exact `run_id`, finite `expires_wall`, `inputs: {path, sha256}`, `artifacts` equal to `harness_hashes()`, and `input_artifacts` equal to the complete prepared artifact map. It also references `host_gate_raw` and `host_gate_review` as `{path, sha256}` pairs. The raw gate must use `reservoir_cpu_fixture_host_window_v1`, be completed, and retain unchanged production identities. The independently reviewed record must use `reservoir_fixture_host_settle_review_v1`, have `passed: true`, and bind the same raw file.

The qualifier independently recomputes the latest continuous 300 seconds of pressure level 1, nonincreasing swap relative to the window's first sample, no build/test process, healthy provider readiness and connected reservoir, with sample gaps no greater than 15 seconds. Initial launch must be within 600 seconds of gate completion and before release expiry. The same release and inputs are revalidated before every child, with expiry still enforced. No new health probes are made by this runner.

The next reviewed host window and any later production preparation must also preserve Minime's native engine (`com.minime.engine`, currently PID 35303) as an explicit protected identity with its exact start and argv, in addition to the existing provider/reservoir/bridge/feeders/perception/Ollama/Minime identities. A process name alone is insufficient. This preparation does not query or control that engine.

This release is bounded admission evidence, not a continuous memory/contention monitor. Root must supervise any later actual execution and stop further cases if contention returns. Interrupting the qualifier retains its exact owned child and prevents another child from starting; it does not force the current child to stop. The immutable release hash is checked between cases, and no release change can silently extend this run's authority.

Only after that release exists, the ordinary nonprivileged execution command is:

```sh
/opt/homebrew/Cellar/python@3.14/3.14.3_1/Frameworks/Python.framework/Versions/3.14/bin/python3.14 -B qualify_restore.py --execute-cpu-qualification --release /absolute/root-reviewed-release.json --run-id reviewed-run-01
```

Do not run the command now, use sudo, invent a passing host receipt, or bypass the root review. The children use the exact original `/Users/v/other/neural-triple-reservoir/.venv/bin/python`, CPU device, and one numerical thread. Python path/home overrides are removed. Network bind/connect/send and GPU/model imports are refused.

## Finite matrix and evidence

One seed child creates both backends with nontrivial hidden state, last input, event sequence maxima, nested provenance, old wall age, oversized stored histories, three complete thermostat sidecars, floor adjustments, and retained synthetic quarantine data. It exits before any verifier child starts.

Nine sequential verifier cases cover complete restoration, missing sidecar, malformed sidecar, corrupt NPZ, incompatible schema, incompatible configuration, private source drift, private package-version manifest drift, and private backup drift. Negative changes touch only independent fixture copies/manifests. Each expected rejection must have its particular exception/message and exit status; OOM, arbitrary errors, or a killed child fail the qualification. The positive result must match the seed's independent field descriptors, thermostat digest, startup metadata and event sequence expectations, in addition to all frozen verifier checks.

Every request, source copy, original seed, backup, raw comparison, log, child launch/exit, expected rejection and final result is retained. The outer receipt always says `production_qualified: false`. Generic frozen maintenance-witness schemas inside these private runs are bound to `synthetic-restore-qualification-...` transactions and a plan explicitly labeled synthetic; they are never evidence of a real service drain or backup.

Each child has a 300-second watchdog. There is no forced stop, signal, retry or service cleanup. On timeout or interruption, the exact owned child may remain alive; its PID/argv and unresolved status are saved, no subsequent child starts, and the retained transaction must be reviewed. `launch-guard.json` prevents a blind second run of this version. Refused prelaunch attempts retain separate refusal receipts and start no child.

Successful private numerical qualification will still not enable production control. Real maintenance needs its independently reviewed controller/Host admission, retained-transaction recovery, current ownership/alignment, a successful final production backup restore, and startup acceptance.
