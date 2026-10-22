# MLX Upgrade Evidence Archive

This directory retains the October 15-22 preparation, qualification, failed
attempts, synthetic maintenance fixtures and source snapshots. It is historical
evidence, not an executable deployment bundle or a statement of current policy.
In particular, `implementation-status.json` ends before the final Stage 2
activation and observer closeout. Its running/pending fields are preserved as
recorded, not interpreted as current deployment debt.

Current closeout: [MLX/perception integration](../2026-10-22-mlx-perception-integration.md).
Current policy: [October 22 policy reset](../2026-10-22-mlx-local-policy-reset.md).
The live selection is full MLX/Metal 0.32.3, MLX-LM 0.32.0 and Transformers
5.7.0, verified under transaction `256b7a3f2f464d77bb8d4d9eb5cb47ba`.
The 0.32.4 candidate was not promoted. Historical failed receipts remain failed.

## What Is Retained

- Root receipts, inventories and locked requirements document the original
  intake, observations and candidate comparisons.
- `manifests/` and `manifests-v2/` bind historical candidate identities. They are
  not the currently selected manifest and must not be installed from this archive.
- `reservoir-autosnapshot-candidate/` retains successive isolated source and
  test packets, including unsuccessful preparations.
- `reservoir-maintenance/` retains synthetic drain/restore fixtures, exact source
  snapshots and their recorded outcomes. A fixture success is not production
  authority. A `files/.runtime/` path here denotes a frozen archival copy, not
  permission to copy it into the live `.runtime` tree.
- Logs are build/test/fixture evidence. Model state, model weights, live private
  writing, raw provider content and actual runtime control directories are not
  part of this archive.

The integration inventory lists exact paths, byte counts and SHA-256 identities.
Archival source is inert: do not import it, execute archived commands, replay
one-use fixture operations, apply old patches or replace newer state from it.
Use a fresh isolated fixture when deliberately reproducing a test. Preserve
original evidence rather than modifying a failed result to match newer policy.
