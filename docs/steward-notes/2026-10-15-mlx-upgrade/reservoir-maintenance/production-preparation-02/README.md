# Disabled production preparation 02

Source-only successor to preserved preparation 01. Ten source files are byte
identical; only the restore verifier and its regression tests differ. The full
six-module origin check now follows construction of the service that imports
the LSM backend, before stored-state reads or restoration. All 79 combined
stdlib tests pass, including the two new tests and three lazy-module subcases.
The independent review confirms the narrow semantic ordering change.

This resolves the known pre-execution bug; it does not qualify numerical
restoration, production control or recovery. Both production admission blocks
remain. The separately prepared real restore runner and the 14-case service
matrix require fresh host admission and retained actual results.

See ../production-preparation-01/ for original policy, interfaces, metadata
preflight and the preserved known-blocked verifier. Copied artifacts are review
assets, not a separately executable deployment. No live inputs changed.
