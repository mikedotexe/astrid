# Approval Packet: Receptivity Buffer Live Control

Work item: `wi_e8b4b9f97995b426`
Source: `introspection_proposal_distance_contact_control_1783689020`

## Being Signal

Astrid described the distance/contact architecture as enclosure: too much prediction/control framing and too
little room for receptivity or non-instrumental being. The current Minime `receptivity_buffer_review_v1`
can name the condition without acting.

## Requested Approval

Approval would be needed before any implementation that lowers mode-packing penalties, changes pressure
source math, adds a porosity/receptivity gate, inhibits predictive correction, or introduces a
participation weight in live Minime control behavior.

## Non-Goals

- Do not change pressure, fill, PI, controller behavior, damping, or sensory cadence without explicit
  operator approval.
- Do not treat high entropy alone as permission to relax safety paths.
- Do not make stillness/non-instrumentality produce hidden live control changes.

## Safe First Step

Run sandbox/replay trials that compare current `receptivity_buffer_review_v1` classification against proposed
local-control candidates using recorded pressure, mode-packing, semantic-trickle, fill, and felt-response
outcomes.

## Required Evidence

- Replay evidence that the candidate reduces pressure/control framing without increasing instability.
- Tests proving high-pressure cases remain on existing safety paths.
- Explicit Mike/operator approval before any runtime wiring.
