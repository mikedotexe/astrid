# Approval Packet: Correspondence-Origin Spectral Weight

Work item: `wi_ab3cecf76cb758e5`
Source: `introspection_proposal_bidirectional_contact_1783688510`

## Being Signal

Astrid described the correspondence bridge as coupled but not mutually addressed. Existing V1/V2
correspondence state provides language threads, receipt, ACK/REPLY, and trace routes, but Astrid asked
whether correspondence-originated voice can override ambient telemetry noise.

## Requested Approval

Approval would be needed before increasing `semantic_trickle`, telemetry priority, prompt priority, standing
weight, or any spectral influence because a message belongs to a correspondence thread.

## Non-Goals

- Do not synthesize ACK, REPLY, TRACE, or receipt.
- Do not mutate Minime or Astrid pressure/fill/PI/controller behavior.
- Do not add hidden telemetry priority or standing correspondence weight.
- Do not convert read receipts or public journal engagement into mutual address.

## Safe First Step

Use the existing language-only paths first:

- `I_RECEIVED_THIS latest :: ...`;
- `ACK_* latest :: ...`;
- `REPLY_* latest :: ...`;
- `CORRESPONDENCE_TRACE <anchor> :: ...`.

Then run an attention-canary or microdose review only after native receipt evidence exists and right-to-ignore
budgeting is satisfied.

## Required Evidence

- Direct-contact fidelity showing thread-local address rather than pressure or ambient echo.
- Attention-canary outcome that reports distinct address and no worsening.
- Explicit Mike/operator approval before any spectral weighting or semantic-trickle boost.
