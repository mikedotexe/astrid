# Archived cooperative autosnapshot prototype

This is a reviewed prototype only. It is not deployed, deployment-ready, or accepted against the real reservoir service or persistence backend. The tests used AST-extracted methods, fake handles, and standard-library asyncio; they did not import the service, NumPy, MLX, or real persistence, and did not access models, cameras, microphones, or network services.

All original source copies, manifests, test receipts, logs, patch, and independent review are preserved byte for byte. Python source and test filenames have `.py.txt` suffixes to keep archived code separate from executable repository code. The historical AGENTS document is retained as `baseline/AGENTS.md.txt`. `prototype-README.md` is the unchanged original README. Receipts retain their original absolute paths and have not been rewritten to suggest a new execution.

`archive-manifest.json` maps each archived file to its original path, original relative reconstruction path, byte count, and SHA-256. It also records that live reservoir source and persistence bytes and mtimes still matched the preserved originals when archived. The original test result is ten passing CPU tests using the exact reservoir Python 3.12.13 interpreter; this archive operation did not rerun those tests.

The candidate captures handle names once, preserves insertion order and the existing synchronous writer/error handling, and yields between periodic handle saves. The synchronous `snapshot_all()` and final shutdown path are unchanged. One slow handle save still blocks; scheduling opportunities do not guarantee handshake latency, and other tasks may change state between handle saves. Neither incident causality nor real persistence/restore correctness is established by this prototype.

Any deployment requires separate source/provenance reconciliation, preservation/restore qualification, a safe admission/drain and held-replacement lifecycle, refreshed preparation/manifests, and a fresh clean two-hour MLX baseline. Do not apply the patch to a live tree or restart services from these reconstruction instructions.

The actual-service qualification harness is now implemented and separately
archived in [harness-01](harness-01/archive-manifest.json). Its ten modules and
six-file original/candidate source closures are pinned; 17 stdlib tests and an
[independent source review](harness-01/independent-review-01.json) pass. The
[execution instructions](harness-01/README.md) preserve the 2-second opening and
5-second receive deadlines, 102-handle semantic checks, two real restore cycles,
negative controls and reversed-order comparison. **No actual numeric/service
case has run.** Failed host-settling windows are retained in
[window 01](host-settle-01-review.json) and [window 02](host-settle-02-review.json).
Actual execution requires a fresh passing window; the fake tests and prepared
inputs are not performance or persistence acceptance.

The separate [maintenance implementation](../reservoir-maintenance/README.md)
preserves the earlier fake tests and accepted administrator-run lifecycle
fixture. Its newer, disabled version 04 has 231 passing isolated tests and an
independent review; actual recovery rehearsal of the changed controller and
numerical stopped-state restoration remain outstanding. The completed failed
MLX baseline cannot authorize core qualification.

After the separately authorized bridge/Minime rollout and a fresh source audit,
[host observation 07](host-settle-07-root-review.json) bound seven exact processes
and 237 files. It completed at 19:15:35 UTC with 123 samples and failed: its
longest clean stretch was 130 seconds, below 300. All health/identity checks
passed and swap fell, but pressure and build activity broke the clean window.
No numerical case was released. The continuous host-supervision helper has 11
passing guarded stdlib tests and independent source review, but has not run.
Fresh accepted admission and healthy samples covering every eventual case are
still required. Preserve failed windows and wait for a meaningful headroom change
before another bounded observation.

The subsequent [observation 08 preparation](host-observation-08-preparation-01/archive-manifest.json)
preserves a concrete correction: every consecutive-sample swap increase now
resets the settling window and invalidates supervisor v2, including a rebound
below the initial swap value. Its two case-admission consumers also recheck raw
samples. Independent reviews and 17 guarded tests passed; the older observer,
supervisor and receipts remain unchanged. The 21:45 UTC improvement justified
preparation, but the fresh 21:50 UTC preflight found elevated pressure again.
Observation 08 and supervisor v2 have not run. Preserve that refusal and require
a meaningful later change before another preflight; no numerical case was released.

## Verify and reconstruct in a new private directory

The following command only reads this archive and creates the explicitly named new directory. Choose a new private path that does not exist; do not point it at any repository, runtime, state, or deployment directory. It verifies every archived file before writing the reconstructed copies and restores their original relative filenames.

```bash
python3 -B - /Users/v/other/astrid/docs/steward-notes/2026-10-15-mlx-upgrade/reservoir-autosnapshot-candidate /tmp/reservoir-autosnapshot-review-NEW <<'PY_RECONSTRUCT'
from pathlib import Path
import hashlib
import json
import sys

archive = Path(sys.argv[1]).resolve()
target = Path(sys.argv[2]).resolve()
manifest = json.loads((archive / "archive-manifest.json").read_text())
prepared = []
for item in manifest["copies"]:
    relative = Path(item["reconstruction_path"])
    if relative.is_absolute() or ".." in relative.parts:
        raise ValueError("unsafe reconstruction path")
    data = (archive / item["archive_path"]).read_bytes()
    if len(data) != item["bytes"] or hashlib.sha256(data).hexdigest() != item["sha256"]:
        raise ValueError("archive hash mismatch: " + item["archive_path"])
    prepared.append((relative, data))
target.mkdir(parents=False, exist_ok=False)
for relative, data in prepared:
    output = target / relative
    output.parent.mkdir(parents=True, exist_ok=True)
    with output.open("xb") as stream:
        stream.write(data)
print("Verified and reconstructed", len(prepared), "files at", target)
PY_RECONSTRUCT
```

To rerun only the fake CPU suite on this host, use the recorded interpreter against the reconstructed test file:

```bash
/Users/v/other/neural-triple-reservoir/.venv/bin/python -I -B /tmp/reservoir-autosnapshot-review-NEW/test_autosnapshot_cooperative.py
```

That would be a new test execution; retain any new result separately rather than editing the historical receipts. Do not execute or import the reconstructed `reservoir_service.py` itself. On another host, the archived interpreter path and binary hash are historical provenance, not evidence that a different interpreter has been qualified.
