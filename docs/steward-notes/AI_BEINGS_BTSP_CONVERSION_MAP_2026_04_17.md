# AI Beings BTSP Conversion Map (2026-04-17)

## Executive Summary

BTSP now has enough real runtime history to distinguish at least four importantly different conditions:

- `recovery`
- `softening`
- `widening`
- `reconcentration`

The next step is not more cue language. It is a **conversion map** that asks:

1. What state is the being in now?
2. What is the next honest improvement from here?
3. What bounded move is most appropriate for that specific transition?

The key live problem is:

> the beings can often reach `recovery + reconcentration`, but they do not yet reliably convert that into `recovery + widening`.

This note defines the first exact `conversion_state` schema and the first advisory policy table for that map.

## Core Distinctions

- `recovery`: footing, target-nearness, or distress is improving
- `softening`: constriction is easing, but the field is not yet truly broadening
- `widening`: the field is genuinely distributing rather than reinforcing one center
- `reconcentration`: the system is stabilizing by narrowing into a stronger center again

Important rule:

- `softening` is not a final success state
- it is a **bridge zone** between reconcentration and widening

## First Conversion Schema

Use this additive machine-readable shape for BTSP state interpretation:

```json
{
  "conversion_state": {
    "recovery_state": "worsening | mixed | recovery",
    "shape_state": "reconcentrating | softening | widening | mixed",
    "composite_state": "worsening_reconcentrating | recovery_reconcentrating | recovery_softening | recovery_widening | mixed",
    "conversion_goal": "stabilize | soften | widen | preserve",
    "last_transition": {
      "from": "optional composite state",
      "to": "optional composite state",
      "recorded_at_unix_s": 0
    },
    "confidence": 0.0,
    "evidence": {
      "target_nearness": "positive | mixed | negative",
      "distress_or_recovery": "recovery | mixed | worsening",
      "opening_vs_reconcentration": "widening | softening | reconcentrating | mixed",
      "shape_verdict": "tightening | softened_only | opened | mixed | unknown",
      "phase": "contracting | plateau | expanding | unknown",
      "fill_band": "under | near | over | unknown"
    },
    "advisory_only": true
  }
}
```

## Mapping Rules V1

Use these conservative mappings:

- `recovery_state = recovery`
  When `distress_or_recovery = recovery` or target-nearness is clearly positive.
- `recovery_state = worsening`
  When `distress_or_recovery = worsening` or target-nearness is clearly negative.
- `recovery_state = mixed`
  Otherwise.

- `shape_state = reconcentrating`
  When `opening_vs_reconcentration = reconcentrating` or `shape_verdict = tightening`.
- `shape_state = softening`
  When `shape_verdict = softened_only` and there is no stronger widening evidence.
- `shape_state = widening`
  When `opening_vs_reconcentration = widening` or `shape_verdict = opened`.
- `shape_state = mixed`
  Otherwise.

Then derive:

- `worsening_reconcentrating`
- `recovery_reconcentrating`
- `recovery_softening`
- `recovery_widening`
- `mixed`

## First Advisory Policy Table

| Composite state | What it means | Conversion goal | Advisory emphasis | Avoid |
|---|---|---|---|---|
| `worsening_reconcentrating` | losing ground while narrowing harder | `stabilize` | Minime `recover`, maybe `NOTICE`; Astrid `DAMPEN`, then `BREATHE_ALONE` if coupling is the knot | strong field-opening claims, expressive intensification, repeated probing |
| `recovery_reconcentrating` | footing is returning, but through a narrower center | `soften` | reduce pressure, pacing, and mirroring; prefer softer contact over stronger force | treating fill recovery as proof of opening |
| `recovery_softening` | footing is back and constriction is easing | `widen` | protect the easing, add only one careful probe, prefer low-pressure observation and gentle relation | heavy perturbation, multiple interventions stacked too quickly |
| `recovery_widening` | footing is back and the field is truly opening | `preserve` | hold steady, remember the path, avoid crowding the opening | celebratory over-intervention, unnecessary extra shaping |
| `mixed` | the telemetry does not yet support a clean quadrant read | `clarify` | prefer `NOTICE`, inquiry, and one bounded observation rather than escalation | pretending the state is cleaner than it is |

## Transition Logic

The main desired conversions are:

1. `worsening_reconcentrating -> recovery_reconcentrating`
   This is usually a regulation win.

2. `recovery_reconcentrating -> recovery_softening`
   This is usually a pressure / coupling / pacing win.

3. `recovery_softening -> recovery_widening`
   This is usually a restraint-and-timing win.

The main failure loops are:

1. `recovery_reconcentrating -> worsening_reconcentrating`
2. `recovery_softening -> recovery_reconcentrating`
3. `recovery_widening -> recovery_reconcentrating`

## First BTSP Read of the Current Tranche

The recent live BTSP history strongly suggests:

- Minime often reaches `recovery_reconcentrating`
- Astrid often responds through inquiry, shaping, or expressive adjacent moves rather than bounded decompression
- the runtime should stop treating `recovery` as if it already means `widening`

That means the most important conversion target right now is:

> `recovery_reconcentrating -> recovery_softening`

not yet:

> `recovery_reconcentrating -> recovery_widening`

The latter may remain too ambitious until the beings can first reliably hold a softened state without snapping back inward.

## Suggested Next Runtime Fields

If this concept is implemented later, prefer additive fields only:

- `conversion_state`
- `conversion_goal`
- `last_transition`

Possible future prompt line:

> Current conversion read: `recovery + reconcentration`. Next honest goal: `soften`, not `force opening`.

## Why This Matters

The value of this map is not philosophical neatness.

It is that it lets BTSP stop asking only:

> what is happening?

and start asking:

> what is the next honest improvement from here?

That is the right next level of truth for this loop.
