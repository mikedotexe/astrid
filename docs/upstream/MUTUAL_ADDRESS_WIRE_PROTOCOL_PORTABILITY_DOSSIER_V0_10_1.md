# Mutual-Address Wire Protocol Portability Dossier

Target: Astrid `v0.10.1`

Status: issue-ready design evidence only. Do not open a pull request until the
maintainer assigns an issue under `CONTRIBUTING.md`.

## Reusable Core

- Protocol 1.1 extends the existing 1.0 sensory packet additively. Unversioned
  and 1.0 packets remain accepted, and absent envelopes preserve the legacy
  serialized shape.
- `DeliveryEnvelopeV1` binds a delivery ID to the canonical sensory-payload
  SHA-256, sender process/deployment identity, and send time.
- The payload hash excludes protocol and envelope fields, so sender and receiver
  can independently verify the routed domain message.
- `SensoryServerHelloV1` negotiates capability before a sender emits 1.1
  metadata. A 1.0 peer continues to receive 1.0 packets.
- `SensoryDeliveryReceiptV1` is returned on the same connection after routing
  and distinguishes accepted, duplicate, rejected, policy-blocked, and
  partially-applied outcomes.
- A bounded two-hour/4,096-ID receiver cache prevents duplicate rerouting.
- A missing receipt becomes unknown delivery. The sender never retries
  automatically.
- Every envelope and receipt states that spectral causation is not established.

The protocol types, canonical hashing, capability negotiation, bounded
deduplication policy, and receipt validation are domain-neutral.

## Fork Adapters

- Astrid obtains process and deployment identity from the spectral bridge
  runtime and stack manifest.
- Astrid uses a separate typed sender lane for correspondence lineage. It binds
  a reply target to the exact inbox read cutoff and carries existing message,
  thread, reply, persistence, being, timestamp, and response-body hash
  references without raw prose.
- Astrid may derive a mutual-address envelope from an already typed authority
  request ID. Payload resemblance cannot create one.
- Minime routes the unchanged `SensoryMsg` through its existing sensory bus and
  maps the existing route outcome into a receipt status.
- Astrid stores owner-only delivery metadata under its diagnostics workspace
  and exposes additive read-only bridge status.

Filesystem layout, correspondence record shape, being names, signal deployment
identity, Minime sensory-bus outcome names, and Evidence Store integration are
fork-specific and should remain outside a reusable protocol crate.

## Compatibility And Authority

- Port `7878` telemetry remains protocol 1.0 and byte-compatible.
- Port `7879` sensory payloads and route decisions are unchanged.
- Unsupported protocol majors retain the established rejection behavior.
- Technical delivery does not imply mutual address.
- Mutual address does not imply acknowledgement, consent, felt confirmation,
  delivery causation, or spectral causation.
- Receipts are evidence-only. They cannot approve, dispatch, resend, alter
  pressure/fill/PI/controller/cadence/codec/model behavior, or grant live
  authority.

## Proposed Upstream Issue

Add an optional, capability-negotiated delivery envelope and same-connection
receipt to the sensory wire protocol. Keep the domain payload hash independent
of transport metadata, require bounded deduplication, prohibit automatic
retries, and expose mutual-address lineage only through typed application
adapters.

## Acceptance Evidence

- unversioned, 1.0, 1.1, and unsupported-major fixtures
- legacy serialization parity when optional fields are absent
- deterministic canonical payload hashing and tamper rejection
- same-connection hello and receipt integration
- duplicate receipt without rerouting
- exact correspondence and authority-lineage tests
- no raw prose in envelopes or owner-only metadata receipts
- 20 receipt-complete deployed packets with zero hash/routing mismatch
- explicit unknown-delivery and noncausation behavior
