# Upstream Contribution Package

This directory packages reusable work from the Astrid fork for maintainer
discussion. It is not an instruction to transplant the fork's commit history.

## Current Baseline

- Canonical repository: `astrid-runtime/astrid`
- Verified upstream ref: `v0.10.1`
- Verified upstream commit: `4771bab3c33d1bce53186e40d01cf014e2dce666`
- Fork integration commit: `b904d6073294723388fb516b9b21329998968d57`
- Verification date: 2026-07-18

The fork and upstream have diverged substantially. Upstream contributions must
be rebuilt as focused patches from the current upstream `main`; the experiential
systems commits are evidence and reference implementations, not cherry-pick
candidates.

## Maintainer Coordination

- [Issue #1271](https://github.com/astrid-runtime/astrid/issues/1271) asks
  whether the portable cooperative steward lifecycle is suitable for an
  assigned first contribution.
- The issue is currently unassigned. No implementation branch or pull request
  should be opened until it is triaged and assigned under `CONTRIBUTING.md`.
- Temporal authority evidence overlaps upstream issues
  [#1229](https://github.com/astrid-runtime/astrid/issues/1229) and
  [#694](https://github.com/astrid-runtime/astrid/issues/694); it should support
  those designs rather than compete with them.

## Package Map

- `CONTRIBUTION_SEQUENCE_V0_10_1.md` records the proposed order, dependencies,
  portability gaps, and upstream disposition for all five tranches.
- `pr-drafts/STEWARD_LIFECYCLE_1271.md` is a review checklist and PR-body draft
  for issue #1271. It remains blocked on maintainer assignment.
- The five `*_PORTABILITY_DOSSIER_V0_10_1.md` files preserve detailed design,
  safety, privacy, and acceptance evidence.

No document in this directory grants approval, changes runtime authority, or
authorizes an upstream pull request.
