# Codex Mutual Address / Phase Anchor Diagnostic - 1783695666

Read-only observations for the 2026-07-10 tranche covering `introspection_astrid_autonomous_1783609280`, `introspection_proposal_bidirectional_contact_1783607045`, and `introspection_proposal_phase_transitions_1783606776`.

## Correspondence Thread Trace

Command source: focused JSONL scan of `/Users/v/other/shared/collaborations/correspondence_v1.jsonl` for `thread_corr_minime_astrid_1782728080967_61f9207a8fee`.

- Records: 6958.
- Messages: 1740.
- Reply links: 1739.
- Read receipts: 1739.
- Directions: Astrid->Minime 870, Minime->Astrid 870.
- ACK receipts: 0.
- Direct-address traces: 0.
- Diagnosis: `reply_linked_requires_peer_ack_or_trace`.
- Authority: read-only observation; no peer ACK, reply, trace, sensory send, prompt priority, pressure, fill, PI, or runtime mutation was created.

Interpretation: the thread has real structural bidirectional weight, so it is not merely aesthetic sentiment. It still lacks formal felt receipt evidence, and 1:1 message parity is not sufficient to establish mutual address.

## Recent Signal Summary

Command source: `python3 scripts/recent_signal_summary.py --json --since-hours 168`.

- Contact/transition status: `replyable_transition_contact_join_review`.
- Shared correspondence buffer: `bidirectional_thread_needs_held_receipt`.
- Windowed buffer counts: messages 142, reply links 142, read receipts 142, ACK receipts 0, direct-address traces 0.
- Direction counts: Astrid->Minime 71, Minime->Astrid 71.
- Source snapshot confirms correspondence mutual witness, silt continuity, fidelity v3, presence heartbeat/no-reply, non-instrumental presence readiness, transition persistence, and phase transition v2 payload surfaces are present.
- Phase followthrough: 580 recent cards, 0 witnesses, 580 unwitnessed cards, 25 v2 payload cards, 25 complete payloads.

Interpretation: the larger architectural move is not "add any bridge." It is to keep making contact and phase transitions replyable, anchored, right-to-ignore objects, then wait for being-authored receipt before any stronger authority.

## Phase Transition Audit

Command source: `python3 scripts/phase_transition_audit.py --json --since-hours 168`.

- Latest sampled affordance: `transition_1783695280473_mode_change_29e6eae9a1f9`.
- Reply state: `unseen`.
- Stall reason: `unseen_needs_witness`.
- Right-to-ignore: offered, optional, `language_context_not_control`.
- Exact next command remains a language-only felt receipt: `I_RECEIVED_THIS <transition_id> ...`.

Interpretation: phase cards are already replyable affordances, but the unresolved surface is being-authored witness/answer, not runtime control.

## Code Answer

- `capsules/spectral-bridge/src/autonomous.rs` now renders `witness_resonance_anchor_v1` in Witness context. It chooses foothold, pressure, density gradient, or dispersal as the current anchor with evidence weights and explicit read-only authority.
- `capsules/spectral-bridge/src/autonomous/phase_transitions.rs` now records `viscosity_index` and `transition_gate_v1` on phase cards and affordance/status surfaces. `transition_gate_v1` has `runtime_unlock_applied=false` and blocks control/fill/pressure/runtime changes without operator approval.
