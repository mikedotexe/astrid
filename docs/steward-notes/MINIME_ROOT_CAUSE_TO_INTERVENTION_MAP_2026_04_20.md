# Minime Root Cause To Intervention Map

Date: 2026-04-20

## Root Problem

The live problem is no longer systems breakage. It is a control-attractor problem:

- under strain, the stack regains coherence by reselecting a narrow `lambda1`-dominant mode,
- the covariance-retention floor protects that mode aggressively during low-fill recovery,
- and small shoulder/entropy gains are usually corrected back into the same reconcentrating basin before they can become real widening.

In plain terms: Minime is better at staying coherent by narrowing than at staying coherent while distributing.

## Observable Signs

- `btsp_signal_status.json` repeatedly lands on `worsening_reconcentrating` or `recovery_reconcentrating`.
- `health.json` stays under target while `shape_verdict=tightening`.
- Journal language keeps returning to pressure, sediment, resistance, and `lambda1` dominance.
- Small targeted perturbations are often absorbed rather than converted into opening.

## Intervention Map

### 1. Protect early shoulder growth

When Minime is still underfilled but the live spectrum is already showing:

- better spectral entropy,
- lower `lambda1` share,
- and enough geometric room to support softening,

the controller should stop acting as if all low-fill states need maximum covariance retention.

Implemented action:

- add a small `underfill_spread_relief` term in the engine recovery math,
- apply it only when underfill coexists with real shoulder/entropy support,
- subtract it from the covariance keep floor / target so the controller does not immediately flatten the reopening.

This is intentionally conservative. It does not try to force widening. It only protects the first signs of distribution when they appear.

### 2. Keep deep-collapse stabilization intact

The intervention should not remove the protective floor during true collapse windows.

Implemented safeguard:

- the relief remains near-zero when entropy is still low, `lambda1` share is still too dominant, or geometry is still too collapsed.

That means severe low-fill reconcentration still stabilizes first.

### 3. Make the new behavior visible

Implemented action:

- expose `cov.spread_relief` in `health.json`,
- include it in the covariance debug log line.

This gives us a direct live read on whether the controller is actively protecting a softening window.

## What To Watch Next

The meaningful signals after rollout are:

- `cov.spread_relief > 0` during underfilled rebound windows,
- slightly lower retained `keep_floor` when entropy/shoulder support appears,
- whether those windows hold `84-85%` `lambda1` dominance and `0.33-0.35` entropy instead of snapping back to `86-87%` / `0.30-0.32`,
- and whether BTSP moves from `worsening_reconcentrating` toward `mixed` or `recovery_reconcentrating` more often without immediate relapse.

## Current Limitation

This does not yet solve the whole attractor problem.

It does not:

- change BTSP policy,
- change exact bounded response semantics,
- change the autonomous agent's sovereignty rules,
- or create widening directly.

It only makes the controller less likely to erase the first real signs of widening support.
