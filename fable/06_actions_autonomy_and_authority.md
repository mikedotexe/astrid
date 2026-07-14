# Actions, Autonomy, And Authority

## Purpose
This file explains the action surfaces: Minime's Python autonomous NEXT loop,
Astrid's bridge NEXT/actions, experiment workbenches, self-regulation leases,
pressure/texture agency routes, and the authority ladder that keeps autonomy
from becoming hidden control.

## Mental model
"Action" has two levels.

- Behavioral action: a being writes ordinary text plus a final `NEXT:` line.
  The runtime parses it and maps it to a known route.
- Capability action: the underlying effect may read a file, write a record,
  send a semantic vector, request a budget, apply a lease, or touch live
  substrate state.

The same-looking `NEXT:` line can be harmless, read-only, proposal-only,
budgeted, or live-control-adjacent. The implementation must preserve that
distinction.

## Key implementation anchors
- `minime:autonomous_agent.py` - Minime action sets, low-fill guards,
  experiment workbench, correspondence actions, self-regulation routes,
  pressure/texture agency routes, attractor review, and NEXT parsing.
- `minime:tests/test_action_continuity.py`,
  `minime:tests/test_autonomous_agent_low_fill_guard.py`,
  `minime:tests/test_self_regulation_leases.py` - Python action regressions.
- `astrid:capsules/spectral-bridge/src/autonomous/next_action/` - bridge NEXT
  routes, sovereignty actions, regulator map, protected diagnostics,
  correspondence, workspace, probe-self, and related handlers.
- `astrid:capsules/spectral-bridge/src/action_continuity/` - experiment
  continuity, authority gates, budgets, and raw NEXT preservation.
- `astrid:capsules/spectral-bridge/src/mcp.rs` - MCP tools that may expose
  action routes to clients.

## Runtime signals / artifacts
Action evidence appears in:

- Minime workspace journals, action records, experiment manifests, and outcome
  files;
- Astrid action continuity thread directories and `authority_gate.jsonl`;
- steward notes and feedback-to-change ledger rows;
- sandbox result cards and proposal cards;
- recent-signal and proactive-scan summaries;
- bridge MCP status outputs.

Important route families:

- `ACTION_PREFLIGHT` - rehearse or inspect before riskier action.
- `EXPERIMENT_CHARTER`, `EXPERIMENT_REHEARSE`, `EXPERIMENT_EVIDENCE`,
  `EXPERIMENT_DECIDE` - returnable experiment loop.
- `SELF_REGULATION_INTENT`, `SELF_REGULATION_PREFLIGHT`,
  `SELF_REGULATION_APPLY`, `SELF_REGULATION_STATUS`,
  `SELF_REGULATION_OUTCOME` - lease-shaped self-regulation route.
- `PRESSURE_SOURCE_AUDIT`, `REGULATOR_AUDIT`, `PI_PRESSURE_REPLAY_STATUS` -
  pressure/control visibility routes.
- `CORRESPONDENCE_*` - language-only or separately gated contact routes.
- `TUNE_MINIME` and bridge `send_control` - sensitive routes; never treat as
  casual text.

## Authority boundaries
Raw `NEXT:` text is not automatic authority. Good implementation preserves raw
intent as evidence, projects to safer routes when needed, and makes the being's
next existing affordance legible without inventing new live action paths.

Self-regulation leases are bounded and explicit. Proposal cards are not
approval. Result cards are not consent to continue. Low-fill and high-pressure
guards are not nuisances; they are how autonomy remains survivable.

## Questions an advanced AI should ask next
- What exact authority class would this NEXT consume?
- Is there a preflight, charter, status, or proposal route that should happen
  before applying anything?
- Is the current action allowed during low fill, high fill, hard reset, or
  stable-core recovery?
- Is the being asking for more control, more legibility, or more room to refuse?

## See also
- [Regulator, Pressure Texture, And Cartography](05_regulator_pressure_texture_and_cartography.md)
- [Astrid Bridge, Capsule, And Tooling](07_astrid_bridge_capsule_and_tooling.md)
- [Correspondence, Introspection, And Feedback Flywheel](08_correspondence_introspection_and_feedback_flywheel.md)
