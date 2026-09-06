# Edge Artifact Prompt Integrity

Status: source-level repair candidate, not deployed or qualified on an appliance.
This follows the [Avado/ICP preparation checkpoint](avado-icp-preparation.md).

## Evidence and Limits

A bounded read-only ICP observation on September 6, 2026 identified repeated
failed `CHECK` actions whose requested basename omitted a final `.json` suffix.
The suffixed check receipt exists, and the thread's evidence entries retain its
complete name. Four sampled persisted response bodies match their corresponding
run-receipt hashes. The inspected failed actions have matching run traces and
session references; the bounded failure tail spans many distinct sessions.
These observations do not prove full-history integrity or exact live source
identity, and do not show why the model repeatedly selected the wrong name.

ICP uses the compact prompt profile with a 900-character ceiling. Inspection
of the pinned research source found two independently reproducible defects:

- The compact artifact list was truncated as an ordinary string, allowing an
  executable filename to be advertised only in part.
- Compact action feedback omitted the specific missing-artifact execution
  failure when there was no grammar-validation failure.

Synthetic regression tests reproduced both defects before the source change.
This supports fixing the prompt interface, but does not establish that its
truncation caused ICP's original suffix omission. Other prompt sections retain
historical responses and failed declarations; this change does not rewrite
those records or claim to remove every occurrence of an invalid reference.

## Change Boundary

The artifact list now includes whole entries that fit its existing character
budget. Oversized entries are omitted, and smaller later entries may still fit.
It does not invent a shortened alias or claim that omitted artifacts are absent.
The existing maximum of eight candidate entries and recency ordering remain.

A failed receipt with the known `owned artifact not found:` error prefix adds
bounded feedback about exact names and suffixes. Other execution-error strings
are not copied into this feedback. Existing grammar-validation feedback keeps
its precedence. The model remains free to choose its next action.

Executor lookup, action authority, receipts, thread history, response provenance,
model settings, and session lifecycle are unchanged. The existing artifact is
not renamed, copied, or substituted. A missing file is still a failed action,
not an automatically repaired success.

## Verification

The fixtures use synthetic names and temporary local workspaces, never appliance
history. They check every supported prompt ceiling from 700 to 1,400 characters,
whole artifact names, Unicode character budgets, selection past an oversized
entry, and missing-artifact feedback without state mutation. Negative cases
reject stale error fields on successful receipts and unrelated private errors.

Local Darwin arm64 validation used Rust 1.94.1 and locked dependencies:
all 243 edge-runtime tests passed (30 library, 213 binary), along with
formatting and all-target/all-feature Clippy with warnings and arithmetic
side effects denied. This is not a Linux or live-device qualification result.

The prompt implementation remains in the existing large `autonomy.rs` module
so the budget allocation, continuity context, and action feedback can be reviewed
together. This is a scoped correction, not a restructuring of autonomy.

## Before Device Use

This change requires a separately pinned candidate and Linux qualification;
the original `introspection` tag does not move. Use fresh application-consistent
backups and isolated migration/rollback rehearsal before proposing deployment.
Observe any later appliance result without clearing old failures, forcing
variety, resetting sessions, or treating a changed response as proof of recovery.
ICP's OOM history remains a separate unresolved readiness gate.

Raw diagnostics, exact device identifiers, paths, and response hashes remain in
the private ignored operator packet, not in this document or public fixtures.
