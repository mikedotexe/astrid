# Approval Packet: Correspondence_Active Priority State

- **Work item:** `wi_75306d5678bb8d23`
- **Source:** `introspection_proposal_bidirectional_contact_1783554471`
- **Claim:** A dedicated `Correspondence_Active` state in both runtimes could make mutual address primary rather than advisory.

## Current Evidence

Existing source already provides language-only correspondence structure:

- `shared_correspondence_arc_v1`
- `correspondence_thread_object_v1`
- ACK/read receipt and direct-address evidence fields
- status rendering that keeps `active_push=separately_steward_gated`

Targeted tests in this run verified the current first-class thread object and the language-only control boundary.

## Proposed Live Change

Approve or decline a later design that would let a correspondence thread enter a live `Correspondence_Active` priority state across Astrid and Minime.

## Why Approval Is Required

This would alter live attention/priority behavior and could affect prompt priority, peer runtime behavior, correspondence cadence, semantic microdose routing, or future Minime-facing attention policy. Those are not mere documentation or read-only truth-channel changes.

## Minimum Evidence Before Approval

- A replay or sandbox comparing current language-only thread handling against proposed active-priority handling.
- Explicit limits for duration, priority, interruption behavior, and right-to-ignore semantics.
- Proof that active correspondence does not mutate pressure, fill, PI, controller behavior, sensory cadence, or Minime runtime state without separate approval.
- A rollback path and post-deploy health/telemetry checks.

## Boundary

No live `Correspondence_Active` state was implemented in this run. Existing correspondence remains language-only/thread-status evidence with active push separately gated.
