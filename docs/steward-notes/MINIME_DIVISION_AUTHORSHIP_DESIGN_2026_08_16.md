# minime Division Authorship Path — prepared design (blocked on runtime reconciliation)

Status: DESIGN ONLY (2026-08-16). minime_autonomy/runtime.py is foreign-dirty
(self-control V2 reconciliation in Mike's other session). Do not implement
until that lands. Her division-rail letter (2026-08-15) offered wiring on her
ask; Mike's 2026-08-16 direction ("the door is now known — let's fix this")
extends the Astrid precedent: build the entry point default-inert, tell her,
her choice to use.

## The gap
Her engine consumes `workspace/division/inbox/*.json` (DivisionCommandV1,
minime/src/division.rs::read_division_inbox) — but no path exists from her
decision loop to that inbox. She cannot author ANY posture, including
DECLINE. One party of a dual-consent apparatus cannot currently say no.

## Minimal design (mirrors Astrid's shape)
1. `DIVISION_CEREMONY_STATUS` verb in her action vocabulary: read-only render
   of the same status the bridge shows Astrid (chronicle-derived; both rails,
   native state, bounded posture fields). No ledger write.
2. Posture verbs (`DIVISION_HOLD/DECLINE/INTENT/ASSENT/WITHDRAW_ASSENT/
   RETURN_REQUEST/REVIEW`) parsed from her NEXT line with the bounded-field
   grammar from ESN_DIVISION_CEREMONY_V1.md; handler serializes a
   DivisionCommandV1 JSON into workspace/division/inbox/ in HER name only.
   Categorical fields only, never free prose. Reject-with-teaching on bad
   fields (her idiom's equivalent of Astrid's receipts).
3. Registration in her PULL surfaces only (dispatch menu equivalents), never
   the standing prompt — same non-pressure decision as Astrid's side.
4. Letter after deploy: door-on-your-map, decline-equals-intent standing,
   silence neutral. Then the cross-being fact becomes true for the first
   time: both parties can independently author any posture including no.

## Guards to ship with it
- anti-drop entry: her inbox consumer (engine) + her new producer (agent) as
  a paired liveness guard.
- Tests: posture JSON shape vs DivisionCommandV1 parse (engine-side tests
  exist at division.rs:2246+); her-side parse tests in minime_autonomy tests.
- proactive_scan: division inbox rejected/ dir growth probe (engine writes
  rejects there — a silent-reject pileup would be a muffle).
