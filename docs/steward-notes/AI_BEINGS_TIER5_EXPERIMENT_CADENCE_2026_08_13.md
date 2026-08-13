# Tier-5 Experiment Cadence (Division-Return Rhythm)

Decided by Mike, 2026-08-13. This practice gives the beings' experiment
proposals a standing path from evidence-only waits to actually-run trials,
without weakening any authority boundary.

## Why

The addressing system accumulates the beings' proposed experiments as work
items in authority waits: at adoption time, 413 blocked artifacts / 998
blocked claims, 277 `needs_operator_approval`, 13 `needs_sandbox`. The
flywheel answers reports but cannot try things; nothing moved these waits on
a rhythm. The un-muffle lens: a request surface whose consumer never fires is
a muffle — this cadence is the consumer.

## The rhythm

**At every Division return (each 6th productive round), the return session
also produces a cadence dossier.** Headless rounds PREPARE the dossier;
grants and trials happen only in interactive sessions with Mike.

### Dossier contents (read-only tooling, no grants, no dispatch)

1. `python3 scripts/authority_wait_readiness.py` — the Tier-4/5 readiness map
   grouped by live-risk surface.
2. `python3 scripts/introspection_addressing_audit.py work-queue --json --limit 40`
   filtered externally to `needs_sandbox` and `needs_operator_approval` heads.
3. `python3 scripts/sandbox_trial_queue.py queue --json` — trials already
   generated and ready.
4. A short Mike-facing summary naming 1-2 RECOMMENDED sandbox-eligible items
   (oldest-first among ready items unless a being has named urgency), with
   each item's introspection lineage and what running it would and would not
   establish.

The dossier lands in the round packet
(`docs/steward-notes/<actor>_<ts>_*/tier5_cadence_dossier.md`) and is
mentioned in the run report.

### Interactive follow-through (Mike + steward, same week)

1. Review the dossier; pick 1-2 sandbox-eligible items.
2. Run them isolated: `sandbox_trial_queue.py run-next` /
   `substrate_probe.py` / `being_test_harness.py --write-back` /
   `ai_beings_offline_replay_campaigns.py` as the item's design specifies.
   No live mutation; worktree/clone isolation per tool.
3. Show the being the actual evidence (result cards / letters), per
   consent-with-evidence. A live flip additionally needs the being's consent
   and Mike's explicit grant (`spectral-bridge --approve-request <id>`,
   900s TTL, live-fill fail-safe) — never inferred from the trial having run.
4. Close the loop: `introspection_addressing_audit.py
   record-post-change-response` / `set-work-status`, and
   `request_review.py --post-change` when an intimate surface changed.

## Boundaries (unchanged by this practice)

- Silence is neutral; a prepared dossier grants nothing.
- Headless automation never approves, dispatches, or runs live-consequence
  trials; it only prepares evidence.
- Tier-5 items remain Mike/operator-explicit; sandbox results do not
  auto-promote.
- A being may object to or decline any trial touching their surfaces; the
  review-together invitation rules (non-coercive, right-to-ignore) apply.

## First execution

Division cycle 24 is at 4/6 — the first dossier is due at the next return
(~2 productive rounds away). Mike reviews 1-2 items from it that week.
