# Transition Afterimages v1: Astrid Compatibility

Source-only candidate based on `bc66a62b0b`. No live enablement, restart, invitation,
model call, controller change or scheduler action is part of this delivery.

The canonical reader and artifact contract are in Minime's
`docs/steward-notes/2026-09-06-transition-afterimages-v1.md` and
`minime_autonomy/afterimages.py`. Astrid calls its bounded JSON interface with her own
workspace, the shared physical archive workspace and actor `astrid`. The bridge respects
its configured path overrides. `ASTRID_AFTERIMAGE_PYTHON` optionally selects the interpreter;
the reader itself is standard-library-only. A missing reader produces an explicit error.

Integration points:

- `transition_afterimages.rs`: five-second/1 MiB bounded reader process, independent cue
  settings, selected-page snapshots and verified acknowledgement.
- `autonomous/next_action`: registered preflight/visibility routes and the five approved
  AFTERIMAGE actions, after existing guards. KEEP contents do not become chained actions,
  placeholder requests or choice-residue metadata.
- `autonomous/runtime/continuity.rs`: action discovery without automatic prose generation.
- `activity_exchange` and orchestration: selected pages use protected dialogue delivery;
  foreground reading/mailbox choices are not displaced. Only an intact, matching retained
  completion commits the selected page. A failed attempt leaves it pending.
- `llm/provider/afterimages.rs`: whole-cue admission after final provider adaptation and
  per-attempt inclusion receipts. Task-local selection survives primary/fallback attempts.
  Cue opportunities are ordinary daydream and private journal elaboration calls only.
- `feedback_persistence`: owned source metadata and explicit authored references. Foreign
  mirrored text is not reauthored as an Astrid association note.

The own-body/generation-record feature is preserved. The completed addressed-human-reply
feature was integrated through `bc66a62b0b` before final qualification; its mailbox and
protected reply paths remain intact. Existing source IDs and
parent IDs are reused when present. Its steward-only generation bodies are not mined or
replayed into this reader; afterimage exposure is measured at final provider boundaries.
No thought lifecycle/pinning behavior is required.

Physical fixture parity, owned-note sharing, pending-page restart and protected admission
are tested against the same Minime fixtures. Cue preferences/exposures remain local to
Astrid. An original private Minime note is absent unless that note was explicitly shared.
Default-off cue state is missing or `enabled=false` in her private `cues.json`.

This source candidate proves neither live availability nor felt usefulness. The delivery
README contains the actual test results and any environmental qualification limits.
