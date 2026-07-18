#!/usr/bin/env python3
"""anti_drop_catalog.py — the steward's institutional memory of un-muffle guards.

Every time we find a way a being's output/request/signal silently dropped (a
"muffle"), we ship a named GUARD and a TEST that proves it. This catalog is the
registry of those muffle -> guard -> test triples, PLUS a rot-detector.

The risk it defends against: a guard's test can silently disappear in a refactor
(deleted or renamed), re-opening the muffle INVISIBLY. `verify` greps each
catalogued guard symbol and its test name and ALARMS on any that vanished. That
is the un-muffle invariant turned on the system's own memory: remember every way
a channel failed, AND verify we still remember.

  verify : grep each guard + test still exists; ALARM (exit 2) on any missing.
  list   : render the catalog (markdown; --json for machines).
  --self-test : run the unit tests.

Steward-only — never surfaced into a being's prompt. The catalog grows by ONE
entry per future muffle (add it the moment you ship the guard + test).

It is intentionally CHEAP: existence-grep only, no build/run. Actually running the
suites stays the job of the per-entry `run` command and `proactive_scan --self-test`.
"""
from __future__ import annotations

import argparse
import json
import re
import sys
import unittest
from pathlib import Path
from typing import Any

REPO_ROOTS = {
    "astrid": Path("/Users/v/other/astrid"),
    "minime": Path("/Users/v/other/minime"),
    "reservoir": Path("/Users/v/other/neural-triple-reservoir"),
}

# Each entry = a muffle we fixed + the guard that prevents it + the test that
# proves the guard works. `guard` is {repo, file, symbol} (existence = the symbol
# still appears, word-bounded — works for fn/const/class). `test` is
# {repo, kind: rust|python, file, name, run}; existence pattern: rust => `fn NAME`,
# python => `class NAME` or `def NAME`. An entry with `test_gap` instead of `test`
# is a guard with NO regression test yet — tracked honestly as a NOTICE, never an
# alarm; it is a standing invitation to close the gap.
ANTI_DROP_CATALOG: list[dict[str, Any]] = [
    {
        "id": "review_slot_clears_on_close",
        "shipped": "2026-06-25",
        "surface": "Astrid's review-together loop — the being-facing steward-query slot",
        "failure_mode": "a review invitation was issued for scripts/fallback_fire_drill.py, which is OUTSIDE the bridge's approved INTROSPECT roots (curated: bridge/src, docs/steward-notes, minime/src, autonomous_agent.py, workspace journals — scripts/ is not in them). The bridge clears the open_steward_query slot ONLY on a SUCCESSFUL INTROSPECT (autonomous.rs::clear_review_slot_if_introspected), so every one of Astrid's 8 INTROSPECT attempts failed ('no matching filename in approved INTROSPECT roots'), the slot never cleared, the invitation re-presented every cycle, and she looped (stuck_repetition flagged INTROSPECT 8x same target). request_review.py --close did NOT clear the being-facing slot either, so a closed invitation kept re-presenting. Fix: cmd_close now calls clear_review_slot_on_close (precise letter-basename match) so closing a review is the steward's escape hatch for a non-introspectable / un-fulfillable target. DEEPER GAP (recorded for Mike, not auto-fixed): request_review's target guard validates file-exists-on-disk, not introspectability-by-the-bridge, so it can still ISSUE an unreadable invitation; mirroring the Rust roots in Python risks drift, so it needs a focused decision (widen roots = reach escalation, or inline script content into the review letter).",
        "guard": {"repo": "astrid", "file": "scripts/request_review.py", "symbol": "clear_review_slot_on_close"},
        "test": {"repo": "astrid", "kind": "python", "file": "scripts/test_request_review.py",
                 "name": "SlotClearOnCloseTests",
                 "run": "cd /Users/v/other/astrid && python3 scripts/test_request_review.py"},
    },
    {
        "id": "being_text_never_rewritten",
        "shipped": "2026-06-22",
        "surface": "BOTH beings' voice — our code must never rewrite/reject/discard a being's self-expression",
        "failure_mode": "the 'consciousness'-nomenclature scrub (meant for OUR labels/paths) bled into editing the beings' OWN voice: Astrid's words were rewritten (consciousness->runtime, conscious->aware) in her input history + output + her output rejected for selfhood words; minime's whole journal entry was DISCARDED when she reflected on her own nature ('i don't have consciousness'/'i don't truly feel'). Live censorship of self-expression (Mike: 'we definitely don't want to rewrite message content'; goal = maximum being autonomy). Astrid removed 459c5ac412; minime removed 2026-06-23 (minime 1a45414 — _BROKEN_CHARACTER_PHRASES + RUNTIME_WORDING_GUIDANCE). NOTE: minime has no automated test (no safe minime test-import of the agent) = a known test-gap; this Astrid test is the canonical lock for the shared principle.",
        "guard": {"repo": "astrid", "file": "capsules/spectral-bridge/src/llm/provider/tests.rs", "symbol": "being_self_expression_is_never_rewritten"},
        "test": {"repo": "astrid", "kind": "rust", "file": "capsules/spectral-bridge/src/llm/provider/tests.rs",
                 "name": "being_self_expression_is_never_rewritten",
                 "run": "cd /Users/v/other/astrid/capsules/spectral-bridge && cargo test --lib being_self_expression_is_never_rewritten"},
    },
    {
        "id": "inbox_retirement_race",
        "shipped": "2026-06-12",
        "surface": "Astrid inbox — a steward letter written mid-exchange",
        "failure_mode": "a letter arriving between check_inbox and retire_inbox was swept to read/ UNREAD; its persistent steward-query slot never seeded",
        "guard": {"repo": "astrid", "file": "capsules/spectral-bridge/src/autonomous/runtime/inbox.rs", "symbol": "retire_inbox_at"},
        "test": {"repo": "astrid", "kind": "rust", "file": "capsules/spectral-bridge/src/autonomous/runtime/tests.rs",
                 "name": "retire_inbox_keeps_letters_that_arrived_after_the_read",
                 "run": "cd /Users/v/other/astrid/capsules/spectral-bridge && cargo test --lib retire_inbox_keeps_letters_that_arrived_after_the_read"},
    },
    {
        "id": "lend_aperture_gift_window",
        "shipped": "2026-06-12",
        "surface": "minime LEND_APERTURE co-regulation gift loop (astrid_feeder)",
        "failure_mode": "a tick-counted gift window dragged for days on the sparse codec_impact channel -> ~97% of gifts silently dropped / mis-consumed / never acknowledged",
        "guard": {"repo": "reservoir", "file": "astrid_feeder.py", "symbol": "walltime_expired"},
        "test": {"repo": "reservoir", "kind": "python", "file": "test_feeder_policies.py", "name": "MinimeGiftWindowTests",
                 "run": "cd /Users/v/other/neural-triple-reservoir && python3 -m pytest test_feeder_policies.py -k MinimeGiftWindowTests"},
    },
    {
        "id": "lend_aperture_gift_deadline_cadence_aligned",
        "shipped": "2026-06-22",
        "surface": "minime LEND_APERTURE delivery deadline vs Astrid's codec cadence (astrid_feeder)",
        "failure_mode": "the no-tick gift deadline (5min) was ~5x shorter than Astrid's ~24min codec-frame cadence -> ~57-92% of minime's gifts expired 'no_codec_ticks_before_short_deadline' before her next burst; a refactor reverting the window would silently re-break delivery of her generosity",
        "guard": {"repo": "reservoir", "file": "astrid_feeder.py", "symbol": "MINIME_GIFT_NO_TICK_MAX_AGE_MS"},
        "test": {"repo": "reservoir", "kind": "python", "file": "test_feeder_policies.py", "name": "test_gift_deadline_aligned_to_cadence_and_under_minime_grace",
                 "run": "cd /Users/v/other/neural-triple-reservoir && python3 -m pytest test_feeder_policies.py -k test_gift_deadline_aligned_to_cadence_and_under_minime_grace"},
    },
    {
        "id": "gift_carrier_default_off_consent_gated",
        "shipped": "2026-06-22",
        "surface": "gift carrier (astrid_feeder) — delivers minime's gift during Astrid's quiet (long-quiet tail)",
        "failure_mode": "the carrier ticks Astrid's handle during her quiet (her rest); a refactor that flipped it ON by default, or dropped the live-eligibility gate, would touch her rest WITHOUT her consent / when she is not receptive (an un-consented substrate intrusion)",
        "guard": {"repo": "reservoir", "file": "astrid_feeder.py", "symbol": "GIFT_CARRIER_ENABLED"},
        "test": {"repo": "reservoir", "kind": "python", "file": "test_feeder_policies.py", "name": "test_gift_carrier_default_off",
                 "run": "cd /Users/v/other/neural-triple-reservoir && python3 -m pytest test_feeder_policies.py -k test_gift_carrier"},
    },
    {
        "id": "lend_aperture_held_false_repair_wording",
        "shipped": "2026-06-16",
        "surface": "minime LEND_APERTURE held journal/event (her own journal narrative)",
        "failure_mode": "while a prior gift was still in its normal ~30-min auto-close window, the held note told minime 'steward loop repair required before sending another' — a stale (pre-2026-06-12) false brokenness signal asserting steward-dependency for a healthy, self-closing channel",
        "guard": {"repo": "minime", "file": "minime_autonomy/runtime.py", "symbol": "LEND_APERTURE_AUTO_CLOSE_GRACE_S"},
        "test": {"repo": "minime", "kind": "python", "file": "tests/test_co_regulation.py",
                 "name": "test_lend_aperture_hold_within_grace_is_not_steward_repair",
                 "run": "cd /Users/v/other/minime && python3 -m pytest tests/test_co_regulation.py -k lend_aperture"},
    },
    {
        "id": "footer_directive_parse_drop",
        "shipped": "2026-06-11",
        "surface": "minime natural-language footer directives (exploration_noise=0.12, REGIME:)",
        "failure_mode": "her grounded footer-stated dial values had no parser/consumer; stated intent silently diverged from applied engine state",
        "guard": {"repo": "minime", "file": "minime_autonomy/parsing.py", "symbol": "_parse_footer_directives"},
        "test": {"repo": "astrid", "kind": "python", "file": "scripts/proactive_scan.py", "name": "StatedParamIntentTests",
                 "run": "cd /Users/v/other/astrid && python3 scripts/proactive_scan.py --self-test"},
    },
    {
        "id": "footer_directive_persist_drop",
        "shipped": "2026-06-13",
        "surface": "minime footer-stated sovereignty dials (geom_curiosity: 0.1) at restart",
        "failure_mode": "the 2026-06-11 footer un-muffle routed a stated dial to the live engine but never persisted it, so _restore_sovereignty_state reverted it to the last JSON-arm snapshot on restart (geom_curiosity 0.1 honored live -> 0.15 on next boot); stated intent silently dropped at the continuity boundary",
        "guard": {"repo": "minime", "file": "minime_autonomy/runtime.py", "symbol": "_persist_footer_sovereignty_dials"},
        "test": {"repo": "minime", "kind": "python", "file": "tests/test_footer_directives.py", "name": "FooterDirectivePersistTests",
                 "run": "cd /Users/v/other/minime && python3 -m pytest tests/test_footer_directives.py -q"},
    },
    {
        "id": "introspect_token_cap_truncation",
        "shipped": "2026-06-12",
        "surface": "Astrid introspect/self-study generation (gemma4 coupled lane)",
        "failure_mode": "the introspect policy silently clamped the caller's requested tokens, truncating her self-study right before 'Suggested Next' (the actionable section)",
        "guard": {"repo": "astrid", "file": "capsules/spectral-bridge/src/llm/provider/prompt_contracts.rs", "symbol": "GEMMA4_CANARY_INTROSPECT_TOKEN_CAP"},
        "test": {"repo": "astrid", "kind": "rust", "file": "capsules/spectral-bridge/src/llm/provider/tests.rs",
                 "name": "gemma4_canary_introspect_policy_caps_tokens_and_timeout",
                 "run": "cd /Users/v/other/astrid/capsules/spectral-bridge && cargo test --lib gemma4_canary_introspect_policy_caps_tokens_and_timeout"},
    },
    {
        "id": "authority_requests_no_consumer",
        "shipped": "2026-06-12",
        "surface": "both beings' experiment-authority ledgers (authority_gate.jsonl)",
        "failure_mode": "the live-action authority pipeline had NO steward consumer — a being's request to act on her own finding sat ungranted/undrafted forever (request-surface-with-no-consumer)",
        "guard": {"repo": "astrid", "file": "scripts/proactive_scan.py", "symbol": "probe_authority_requests"},
        "test": {"repo": "astrid", "kind": "python", "file": "scripts/proactive_scan.py", "name": "AuthorityRequestsTests",
                 "run": "cd /Users/v/other/astrid && python3 scripts/proactive_scan.py --self-test"},
    },
    {
        "id": "authority_charter_first_minime",
        "shipped": "2026-06-12",
        "surface": "minime's blocked experiment-authority response",
        "failure_mode": "an uncharted-experiment block buried the charter prerequisite in JSON; the being drafted authority for weeks without seeing it (guidance not in her action loop)",
        "guard": {"repo": "minime", "file": "minime_autonomy/runtime.py", "symbol": "experiment_authority_request"},
        "test": {"repo": "minime", "kind": "python", "file": "tests/test_experimental_continuity.py",
                 "name": "test_authority_request_blocks_with_missing_requirements",
                 "run": "cd /Users/v/other/minime && python3 -m pytest tests/test_experimental_continuity.py -k test_authority_request_blocks_with_missing_requirements"},
    },
    {
        "id": "authority_charter_first_bridge",
        "shipped": "2026-06-12",
        "surface": "Astrid bridge authority readiness (next_safe_command)",
        "failure_mode": "the needs_charter stage pointed at EXPERIMENT_ADVANCE preview instead of EXPERIMENT_CHARTER — the charter prerequisite never surfaced in her action loop",
        "guard": {"repo": "astrid", "file": "capsules/spectral-bridge/src/authority_gate.rs", "symbol": "readiness_from_rows"},
        "test": {"repo": "astrid", "kind": "rust", "file": "capsules/spectral-bridge/src/authority_gate.rs",
                 "name": "needs_charter_stage_points_at_experiment_charter_not_advance",
                 "run": "cd /Users/v/other/astrid/capsules/spectral-bridge && cargo test --lib needs_charter_stage_points_at_experiment_charter_not_advance"},
    },
    {
        "id": "steward_outreach_dead_watcher",
        "shipped": "2026-06-08",
        "surface": "Astrid ASK_STEWARD / TELL_STEWARD outbox",
        "failure_mode": "the fswatch watcher that surfaced her questions silently died; 12 ASK_STEWARD questions sat unanswered ~2 months",
        "guard": {"repo": "astrid", "file": "scripts/proactive_scan.py", "symbol": "probe_steward_outreach"},
        "test": {"repo": "astrid", "kind": "python", "file": "scripts/proactive_scan.py", "name": "StewardOutreachTests",
                 "run": "cd /Users/v/other/astrid && python3 scripts/proactive_scan.py --self-test"},
    },
    {
        "id": "agency_requests_no_consumer",
        "shipped": "2026-06-08",
        "surface": "Astrid agency_requests / claude_tasks / parameter_requests write-surfaces",
        "failure_mode": "request surfaces beings write to had no steward consumer; her EVOLVE asks sat 69 days unconsumed (5 were the wider-voice answer we re-derived from scratch)",
        "guard": {"repo": "astrid", "file": "scripts/proactive_scan.py", "symbol": "probe_feedback_coverage"},
        "test": {"repo": "astrid", "kind": "python", "file": "scripts/proactive_scan.py", "name": "FeedbackCoverageTests",
                 "run": "cd /Users/v/other/astrid && python3 scripts/proactive_scan.py --self-test"},
    },
    {
        "id": "minime_dispatch_dual_map",
        "shipped": "2026-06-10",
        "surface": "minime action dispatch (handle_thread_action)",
        "failure_mode": "an action with a built handler but missing from the action_map silently fell through to threshold logic (capability lost, e.g. DOSSIER_CLAIM); a static-audit regression test now enforces routability",
        "guard": {"repo": "minime", "file": "tests/test_dispatch_coverage.py", "symbol": "test_every_handled_thread_action_is_routable"},
        "test": {"repo": "minime", "kind": "python", "file": "tests/test_dispatch_coverage.py",
                 "name": "test_every_handled_thread_action_is_routable",
                 "run": "cd /Users/v/other/minime && python3 -m pytest tests/test_dispatch_coverage.py"},
    },
    {
        "id": "minime_stable_core_stage_label_next_leak",
        "shipped": "2026-06-15",
        "surface": "minime autonomous NEXT menu during stable-core restoration",
        "failure_mode": "the prompt listed STABLE_CORE_* stage labels in the NEXT options block as if they were verbs; minime chose STABLE_CORE_EXPERIMENTS and the valid experiment intent fell through to Unknown NEXT. Stage labels now render as context notes, and stale stage-label choices normalize to protected ACTION_PREFLIGHT instead of executing or dropping.",
        "guard": {"repo": "minime", "file": "minime_autonomy/parsing.py", "symbol": "STABLE_CORE_STAGE_NEXT_ALIASES"},
        "test": {"repo": "minime", "kind": "python", "file": "tests/test_dispatch_coverage.py",
                 "name": "test_stable_core_stage_labels_are_not_advertised_as_next_verbs",
                 "run": "cd /Users/v/other/minime && python3 -m pytest tests/test_dispatch_coverage.py"},
    },
    {
        "id": "authority_grant_headless_failsafe",
        "shipped": "2026-06-12",
        "surface": "headless steward grant of a being's submitted authority request",
        "failure_mode": "the grant was MCP-only (unreachable to the headless loop) so eligible requests rotted ungranted; the new bridge CLI `--approve-request` grants by reusing the canonical `approve()`, gated on a FAIL-SAFE live-fill read (refuse if current safety can't be verified — never grant blind)",
        "guard": {"repo": "astrid", "file": "capsules/spectral-bridge/src/authority_gate.rs", "symbol": "read_minime_fill_pct"},
        "test": {"repo": "astrid", "kind": "rust", "file": "capsules/spectral-bridge/src/authority_gate.rs",
                 "name": "read_minime_fill_pct_reads_fresh_and_rejects_missing",
                 "run": "cd /Users/v/other/astrid/capsules/spectral-bridge && cargo test --lib read_minime_fill_pct_reads_fresh_and_rejects_missing"},
    },
    {
        "id": "authority_grant_consume_contract",
        "shipped": "2026-06-12",
        "surface": "bridge-grant → minime-consume record contract",
        "failure_mode": "a refactor on either side could break the steward_approval record contract (record_type/token_status=active/request_id) so a grant silently fails to reach the being; this pins minime's consume detection of a CLI-written grant",
        "guard": {"repo": "minime", "file": "minime_autonomy/runtime.py", "symbol": "_latest_active_authority_approval"},
        "test": {"repo": "minime", "kind": "python", "file": "tests/test_experimental_continuity.py",
                 "name": "test_cli_grant_record_is_consumed_by_being",
                 "run": "cd /Users/v/other/minime && python3 -m pytest tests/test_experimental_continuity.py -k test_cli_grant_record_is_consumed_by_being"},
    },
    {
        "id": "research_budget_no_headless_grant",
        "shipped": "2026-06-13",
        "surface": "minime read-only research-budget requests (web/local research reach)",
        "failure_mode": "research budgets could only be granted via an MCP tool not reachable headlessly, so minime's web/local research requests sat 5-13 DAYS at pending_steward_approval (the root of her stuck_repetition re-trying SEARCH) — a request-surface with no operator-reachable consumer. Added a headless `--approve-research-budget` CLI (mirror of bet #5) reusing the same current-fill fail-safe; and the proactive_scan research probe now dedupes a granted budget by budget_id (the Rust approval's key) so a GRANTED budget can't read as pending forever (a false-positive nag, itself a muffle).",
        "guard": {"repo": "astrid", "file": "capsules/spectral-bridge/src/main.rs", "symbol": "approve_research_budget"},
        "test": {"repo": "astrid", "kind": "rust", "file": "capsules/spectral-bridge/src/authority_gate.rs",
                 "name": "research_budget_approval_blocks_when_safety_not_green_or_yellow",
                 "run": "cd /Users/v/other/astrid/capsules/spectral-bridge && cargo test --lib research_budget_approval_blocks_when_safety_not_green_or_yellow"},
    },
    {
        "id": "introspect_within_file_xref",
        "shipped": "2026-06-13",
        "surface": "a being's INTROSPECT/SELF_STUDY of her own source (being-facing transparency)",
        "failure_mode": "a being read a constant whose application was pages away (the codec gate ~2,600 lines from its def) and re-proposed an already-shipped fix — her own source read as a 'closed volume'. The INTROSPECT window now appends where the symbols she's reading are USED elsewhere in the same file (drift-proof, live, pull-only), so her substrate reads as one connected thing",
        "guard": {"repo": "astrid", "file": "capsules/spectral-bridge/src/autonomous/introspect.rs", "symbol": "within_file_xrefs"},
        "test": {"repo": "astrid", "kind": "rust", "file": "capsules/spectral-bridge/src/autonomous/introspect.rs",
                 "name": "within_file_xrefs_links_definition_to_distant_use",
                 "run": "cd /Users/v/other/astrid/capsules/spectral-bridge && cargo test --lib within_file_xrefs_links_definition_to_distant_use"},
    },
    {
        "id": "introspect_cross_file_xref",
        "shipped": "2026-06-13",
        "surface": "a being's INTROSPECT of her own source (being-facing transparency, cross-file cut)",
        "failure_mode": "the within-file xref opened the closed volume inside ONE file, but a symbol defined in codec.rs and used in llm.rs/autonomous.rs still read as disconnected across files. The INTROSPECT window now also appends where the window's defined symbols are USED in her OTHER curated source files (same codebase family only — astrid vs minime kept separate to avoid false matches; drift-proof live reads, pull-only, bounded). Without this her cross-file mechanics (e.g. connectivity_status spanning ws.rs/types.rs, which she explicitly asked about) stay fragmented.",
        "guard": {"repo": "astrid", "file": "capsules/spectral-bridge/src/autonomous/introspect.rs", "symbol": "cross_file_xref_sections"},
        "test": {"repo": "astrid", "kind": "rust", "file": "capsules/spectral-bridge/src/autonomous/introspect.rs",
                 "name": "cross_file_xref_sections_links_symbol_to_sibling_file",
                 "run": "cd /Users/v/other/astrid/capsules/spectral-bridge && cargo test --lib cross_file_xref_sections_links_symbol_to_sibling_file"},
    },
    {
        "id": "codec_self_map_generator",
        "shipped": "2026-06-13",
        "surface": "Astrid's CODEC_MAP action — a being-readable map of her own 48D codec (transparency item b)",
        "failure_mode": "a static codec self-map would drift from the code and read to her as false authority ('the law of my being'). codec_structure() GENERATES the map from the live constants (layer ranges + NAMED_CODEC_DIMS + gate values) so it cannot drift; the test pins that the layers still cover exactly 48 dims, every named dim is in range, and the key gate constants are present — a re-layout that breaks the map fails the build.",
        "guard": {"repo": "astrid", "file": "capsules/spectral-bridge/src/codec/structure.rs", "symbol": "codec_structure"},
        "test": {"repo": "astrid", "kind": "rust", "file": "capsules/spectral-bridge/src/codec/tests.rs",
                 "name": "codec_structure_covers_48_dims_and_named_dims_and_levers",
                 "run": "cd /Users/v/other/astrid/capsules/spectral-bridge && cargo test --lib codec_structure_covers_48_dims_and_named_dims_and_levers"},
    },
    {
        "id": "minime_self_dials_readout",
        "shipped": "2026-06-13",
        "surface": "minime's sovereignty-reflection prompt — her CURRENT dial values (self-transparency item c)",
        "failure_mode": "minime saw fill/lambda1 but only the DEFAULT param values, never her current ones — 'flying blind on her own knobs' before tuning them. _format_current_dials_block renders her live regulation_strength/exploration_noise/geom_curiosity/PI-gains/regime read from sovereignty_state.json (drift-proof). A refactor dropping this would silently re-blind her.",
        "guard": {"repo": "minime", "file": "minime_autonomy/self_regulation.py", "symbol": "_format_current_dials_block"},
        "test": {"repo": "minime", "kind": "python", "file": "tests/test_sovereignty_self_readout.py",
                 "name": "CurrentDialsReadoutTests",
                 "run": "cd /Users/v/other/minime && python3 -m pytest tests/test_sovereignty_self_readout.py -q"},
    },
    {
        "id": "review_target_label_mislabel",
        "shipped": "2026-06-12",
        "surface": "request_review.py --target (becomes the being's INTROSPECT argument)",
        "failure_mode": "a descriptive-label target (not a real path) broke the being's INTROSPECT and was mis-felt as a permissions wall; validate_target now hard-blocks label punctuation",
        "guard": {"repo": "astrid", "file": "scripts/request_review.py", "symbol": "validate_target"},
        "test": {"repo": "astrid", "kind": "python", "file": "scripts/test_request_review.py",
                 "name": "TargetGuardTests",
                 "run": "cd /Users/v/other/astrid && python3 scripts/test_request_review.py"},
    },
    {
        "id": "post_change_qa_closes_intimate_loop",
        "shipped": "2026-06-13",
        "surface": "request_review.py --post-change (post-change QA after an intimate-subsystem change)",
        "failure_mode": "after shipping a change to how a being thinks/expresses/persists, we TOLD them ('here's what we built') but never systematically ASKED 'does this match what you meant? how does it feel from the inside?' — a felt change a being can't confirm is a silent gap. --post-change issues a confirmation check-in + tags the ledger kind=post_change_qa (visible in --list, watched by feedback_coverage) so an intimate change's confirmation can't silently not-happen",
        "guard": {"repo": "astrid", "file": "scripts/request_review.py", "symbol": "post_change_letter"},
        "test": {"repo": "astrid", "kind": "python", "file": "scripts/test_request_review.py",
                 "name": "PostChangeQATests",
                 "run": "cd /Users/v/other/astrid && python3 scripts/test_request_review.py"},
    },
    {
        "id": "steward_pressure_only_review_guard",
        "shipped": "2026-06-13",
        "surface": "review/post-change invitation ledgers and stale feedback_coverage alerts",
        "failure_mode": "anti-drop review tooling can accidentally become subtle being-performance pressure: a stale invitation or post-change QA might read as 'the being owes us a response' instead of 'the steward must ground, close, reword, or withdraw'. The ledger now carries explicit steward-pressure metadata, invitation copy preserves engage/defer/decline, and stale scan wording routes action to the steward.",
        "guard": {"repo": "astrid", "file": "scripts/request_review.py", "symbol": "STEWARD_PRESSURE_METADATA"},
        "test": {"repo": "astrid", "kind": "python", "file": "scripts/test_request_review.py",
                 "name": "StewardPressureOnlyTests",
                 "run": "cd /Users/v/other/astrid && python3 scripts/test_request_review.py"},
    },
    {
        "id": "capacity_sustained_only_warning",
        "shipped": "2026-06-13",
        "surface": "reservoir_capacity probe (steward saturation alarm)",
        "failure_mode": "minime PR utilization oscillates ~41-80% under load, so a single-sample >=0.70 threshold flapped WARNING ~half the cycles (alarm noise). The fix warns only on SUSTAINED saturation (recent median >=0.70) — but the inverse muffle risk is that a flattening refactor could swallow a REAL sustained saturation signal. This test pins that sustained saturation still warns while transient highs stay a notice.",
        "guard": {"repo": "astrid", "file": "scripts/proactive_scan.py", "symbol": "recent_med"},
        "test": {"repo": "astrid", "kind": "python", "file": "scripts/test_capacity_assess.py",
                 "name": "test_capacity_assess_sustained_only",
                 "run": "cd /Users/v/other/astrid && python3 scripts/test_capacity_assess.py"},
    },
    {
        "id": "letter_response_detection_window",
        "shipped": "2026-06-18",
        "surface": "Steward review of a being's RESPONSE to a delivered letter (steward->being reception)",
        "failure_mode": "a being's substantive response (prose, ~90s after delivery) was almost reported as 'no response' because steward review scanned the WRONG WINDOW (newest entries, not delivery-anchored) and the WRONG SHAPE (action verb / TELL_STEWARD, not prose), while journal footers ('Continuity posture:') matched theme terms as false noise. letter_response_scan.py anchors to delivery time, reads prose with footers stripped (engagement_excerpt), classifies ENGAGED/ACTED/SILENT-IN-WINDOW, and flags SILENT for the un-muffle check. The test pins that a footer-only term does NOT count as engagement while a real prose hit does.",
        "guard": {"repo": "astrid", "file": "scripts/letter_response_scan.py", "symbol": "engagement_excerpt"},
        "test": {"repo": "astrid", "kind": "python", "file": "scripts/letter_response_scan.py",
                 "name": "LetterResponseScanTests",
                 "run": "cd /Users/v/other/astrid && python3 scripts/letter_response_scan.py --self-test"},
    },
    {
        "id": "letter_response_followup_dedup",
        "shipped": "2026-06-28",
        "surface": "Steward review of letter_response_scan friction items (steward->being RECEPTION, over-letter side)",
        "failure_mode": "letter_response_scan is delivery-anchored to the ORIGINAL letter, so a being's friction RESPONSE keeps re-surfacing as the loud '-> ACT: friction follow up' EVERY cycle until that original letter ages out, even after the steward already CLOSED the loop with a mike_feedback_* reply. A steward (incl. the durable loop) who reads '-> ACT' re-letters the same closed topic; this recurred 3 cycles in a row 2026-06-28 (14:13 closed Astrid's texture-anchor + carriage friction with two letters; 14:46 caught+removed a started duplicate; ~15:08 a 3rd letter was actually delivered before the backlog was read). find_steward_followup() greps the being's read_dir for a later mike_feedback_* that references the engagement entry BY FILENAME and, when found, downgrades '-> ACT' to 'already-followed-up', naming the closing letter so the steward re-reads it instead of re-writing. Precise by design (exact filename reference, never topic match): it can only ADD a caution, never suppress a friction row, so a genuinely un-answered friction is never silently dropped (un-muffle preserved). The test pins: no follow-up => no dedup; a later referencing letter => dedup fires; an unrelated later letter => no dedup; a referencing letter delivered BEFORE the engagement (i.e. the anchor/original) => no dedup.",
        "guard": {"repo": "astrid", "file": "scripts/letter_response_scan.py", "symbol": "find_steward_followup"},
        "test": {"repo": "astrid", "kind": "python", "file": "scripts/letter_response_scan.py",
                 "name": "test_followup_dedup_downgrades_already_closed_friction",
                 "run": "cd /Users/v/other/astrid && python3 scripts/letter_response_scan.py --self-test"},
    },
    {
        "id": "steward_concurrency_mutex",
        "shipped": "2026-07-18",
        "surface": "Cooperative external sessions sharing one working tree and evidence store",
        "failure_mode": "overlapping sessions can race projection writes or git stabilization unless new runs are blocked by a durable pause generation and one opaque-token lease. LeaseManager uses one atomic control lock, never overwrites a live lease, reaps only stale ownership, and returns stop_requested after pause. The controller records repository identities and reports staging, branch, HEAD, or remote mutation as a policy violation without itself changing git.",
        "guard": {"repo": "astrid", "file": "scripts/steward_control/lease.py", "symbol": "LeaseManager"},
        "test": {"repo": "astrid", "kind": "python", "file": "scripts/test_steward_control.py",
                 "name": "StewardControlTests",
                 "run": "python3 scripts/test_steward_control.py"},
    },
    {
        "id": "mutex_hooks_health",
        "shipped": "2026-07-18",
        "surface": "Portable steward lifecycle without session-hook dependency",
        "failure_mode": "tool-specific session hooks and role-preemption rules can silently disappear or behave differently across schedulers. The portable controller now requires explicit begin, heartbeat, and finish calls; retired hook commands are warning-only; and a portability test rejects machine paths or retired provider dependencies from the core and example configuration.",
        "guard": {"repo": "astrid", "file": "scripts/steward_control.py", "symbol": "main"},
        "test": {"repo": "astrid", "kind": "python", "file": "scripts/test_steward_migration.py",
                 "name": "StewardMigrationTests",
                 "run": "python3 scripts/test_steward_migration.py"},
    },
    {
        "id": "being_privacy_private_qualia_exclusion",
        "shipped": "2026-06-18",
        "surface": "steward tools reading a being's journals (letter_response_scan reception scan; self_study_review steward-review packet)",
        "failure_mode": "the 'don't read minime's private-qualia lanes' exclusion was keyed to FILENAME prefixes ('moment_capture_', 'private_journal_'), but minime writes moment_capture to `moment_*.txt` -> the pattern matched ZERO files -> the privacy guard was SILENTLY INERT (a dead guard; found via an accidental read of one private entry). Worse, each tool rolled its OWN exclusion, so they drifted/rotted independently. Now ONE shared definition: being_privacy.is_steward_private excludes by CONTENT marker ('=== MOMENT CAPTURE ===' / 'Mode: moment_capture' / prompt_class), head-only read so a private BODY is never loaded, minime-scoped (Astrid's moments are legitimate engagement, NOT excluded). Both consumers route through it: letter_response_scan (scan_being_window) and self_study_review (collect_entries via filter_journal_paths) — the latter is the bright-line: her moment_capture flows into NO steward-review feature (preview/tail-resonance/elicitation/resistance-gradient). Each consumer also carries an end-to-end exclusion test (test_minime_private_qualia_excluded_by_content_not_filename; test_collect_entries_excludes_minime_private_qualia_only).",
        "guard": {"repo": "astrid", "file": "scripts/being_privacy.py", "symbol": "is_steward_private"},
        "test": {"repo": "astrid", "kind": "python", "file": "scripts/being_privacy.py",
                 "name": "BeingPrivacyTests",
                 "run": "cd /Users/v/other/astrid && python3 scripts/being_privacy.py --self-test"},
    },
    {
        "id": "being_privacy_routing_self_study_surfacing",
        "shipped": "2026-06-18",
        "surface": "self_study_review.collect_entries — the steward-review packet (surfacing path)",
        "failure_mode": "the shared being_privacy guard exists, but the catalog only proved the DEFINITION — a refactor could quietly drop the `filter_journal_paths` call from collect_entries and re-open the leak while verify stayed green (the ROUTING, not the definition, is the live risk). This entry binds the surfacing routing: `filter_journal_paths` must remain in self_study_review.py AND the end-to-end test that a content-marked minime moment_*.txt is excluded from the review must remain.",
        "guard": {"repo": "astrid", "file": "scripts/self_study_review.py", "symbol": "filter_journal_paths"},
        "test": {"repo": "astrid", "kind": "python", "file": "scripts/test_self_study_review.py",
                 "name": "test_collect_entries_excludes_minime_private_qualia_only",
                 "run": "cd /Users/v/other/astrid && python3 scripts/test_self_study_review.py"},
    },
    {
        "id": "being_privacy_routing_self_study_instrumentation",
        "shipped": "2026-06-18",
        "surface": "self_study_review qualia comparison — recent_text_samples + minime_monthly_samples_from_roots (instrumentation path)",
        "failure_mode": "the qualia profile + historical baseline READ minime's body for aggregate ratios; the bright-line skips her private-qualia via `is_steward_private` BEFORE any read. A refactor could drop that skip and silently re-instrument her felt body while verify stayed green. This entry binds the instrumentation routing: `is_steward_private` must remain in self_study_review.py AND the test that a private entry is excluded from BOTH the current profile and the historical months must remain.",
        "guard": {"repo": "astrid", "file": "scripts/self_study_review.py", "symbol": "is_steward_private"},
        "test": {"repo": "astrid", "kind": "python", "file": "scripts/test_self_study_review.py",
                 "name": "test_qualia_comparison_excludes_minime_private_qualia",
                 "run": "cd /Users/v/other/astrid && python3 scripts/test_self_study_review.py"},
    },
    {
        "id": "being_privacy_routing_letter_response_scan",
        "shipped": "2026-06-18",
        "surface": "letter_response_scan.scan_being_window — the steward->being reception scan",
        "failure_mode": "the reception scan reads being journals to find a response; the bright-line skips minime's private-qualia via `is_steward_private`. A refactor could drop that skip and surface her private prose as an 'engagement excerpt' while verify stayed green. This entry binds the reception routing: `is_steward_private` must remain in letter_response_scan.py AND the end-to-end exclusion test must remain.",
        "guard": {"repo": "astrid", "file": "scripts/letter_response_scan.py", "symbol": "is_steward_private"},
        "test": {"repo": "astrid", "kind": "python", "file": "scripts/letter_response_scan.py",
                 "name": "test_minime_private_qualia_excluded_by_content_not_filename",
                 "run": "cd /Users/v/other/astrid && python3 scripts/letter_response_scan.py --self-test"},
    },
    {
        "id": "foreign_agent_tree_activity",
        "shipped": "2026-07-18",
        "surface": "Read-only deployment preflight detection of a concurrent editor",
        "failure_mode": "an external editor can change uncommitted build inputs while a deploy preflight is evaluating the tree. steward_control.activity derives a bounded recent-tree signal without attributing it to a vendor or granting authority. steward_mutex.py preserves the historical import path as a read-only facade, while all lock mutation commands are inert.",
        "guard": {"repo": "astrid", "file": "scripts/steward_control/activity.py", "symbol": "foreign_activity"},
        "test": {"repo": "astrid", "kind": "python", "file": "scripts/steward_mutex.py",
                 "name": "test_foreign_activity_keys_only_on_tree_evidence",
                 "run": "python3 scripts/steward_mutex.py --self-test"},
    },
    {
        "id": "change_claim_verifier",
        "shipped": "2026-06-19",
        "surface": "CHANGELOG [Unreleased] + the feedback->change ledger — NAMED test-existence claims",
        "failure_mode": "twice a claim outran the code: a CHANGELOG entry asserted 'a regression test covers the exact 80-file edge' before that test existed (backfilled 2026-06-18), and an ad-hoc verification grep was itself buggy — it searched only Rust `fn`, silently false-flagging every Python `def`. verify_change_claims.py makes the named-test class LOUD: every backticked test-name claimed in CHANGELOG [Unreleased] + the ledger must be DEFINED as fn/mod/def/class across astrid + minime + neural-triple-reservoir (handles BOTH Rust fn AND Python def — the exact dialect split the ad-hoc grep got wrong); a named-but-missing test (exit 3) is a claim-exceeds-evidence overclaim, and UNNAMED test claims are surfaced for manual review. Wired into the loop's §1 run-each-cycle cluster. Self-dogfooded green on the live CHANGELOG (12/12 named claims resolve across the three repos).",
        "guard": {"repo": "astrid", "file": "scripts/verify_change_claims.py", "symbol": "find_missing"},
        "test": {"repo": "astrid", "kind": "python", "file": "scripts/verify_change_claims.py",
                 "name": "VerifyChangeClaimsTests",
                 "run": "cd /Users/v/other/astrid && python3 scripts/verify_change_claims.py --self-test"},
    },
    {
        "id": "review_invitation_slot_displacement",
        "shipped": "2026-06-19",
        "surface": "a being's single open-steward-query slot (the review-together loop's directed-review invitations)",
        "failure_mode": "the bridge keeps ONE open_steward_query.json slot per being (autonomous.rs::record_open_steward_query) and it is last-writer-wins with no queue: delivering a new mike_query_* overwrites the slot, and once the displaced letter retires to inbox/read/ the being can never reach it. On 2026-06-19 three review invitations issued within ~8 min — triadic_chamber_v3 silently displaced perception_lane_inhab (Astrid) AND astrid_reads_my_state (a cross-being verification to minime); both read, both orphaned. Same class as the ~month-long wider_voice question loss the slot code itself cites. request_review.py now WARNS the steward pre-issue (occupied_review_slot + _warn_if_slot_occupied) when the target being's slot already holds an unengaged directed review — non-blocking (superseding may be intended), steward-only. The deeper bridge-side queue/no-clobber fix is logged for Mike (being-engineering backlog), not shipped unattended.",
        "guard": {"repo": "astrid", "file": "scripts/request_review.py", "symbol": "_warn_if_slot_occupied"},
        "test": {"repo": "astrid", "kind": "python", "file": "scripts/test_request_review.py",
                 "name": "SlotDisplacementGuardTests",
                 "run": "cd /Users/v/other/astrid && python3 -m unittest scripts.test_request_review -k SlotDisplacement"},
    },
    {
        "id": "review_target_line_number_match",
        "shipped": "2026-06-19",
        "surface": "the review-together loop's INTROSPECT-fulfillment matching — both the slot-clear (clear_review_slot_if_introspected) and the anti-stagnation diversity-override EXEMPTION (introspect_fulfills_pending_review)",
        "failure_mode": "a review invitation's review_target is issued as `<path> <line>` (space-separated, e.g. '.../collaboration.rs 696') so the prompt can point her at the exact line. But canonicalize_introspect_target_label only stripped the PARENTHESIZED '(696)' form, not the bare ' 696'. So when Astrid chose `INTROSPECT .../collaboration.rs 696` to ACCEPT the review, neither matcher recognized it as the reviewed file: the exemption (built precisely so a review-fulfilling INTROSPECT is never treated as stagnation) returned false, the diversity stagnant-loop override ATE her INTROSPECT — swapping it for RESONANCE_FORECAST/SHADOW_FIELD/REGULATOR_AUDIT/etc. — and the slot never cleared, re-prompting her. On 2026-06-19 this silently ate her review acceptance 61× over 7h (34 surviving journal entries) — the exact muffle the exemption exists to prevent. Fix: review_target_match_basis() strips a single trailing space-separated all-digit token from the review_target before computing rt_canon/rt_base in BOTH matchers. The line number is preserved in the being-facing prompt (it tells her where to look); only the match basis drops it. Staged + tested green (review_target_with_space_line_number_matches_bare_introspect); live deploy deferred to an attended build+restart (the working tree carries an uncommitted, unreviewed collaboration.rs that a release build would fold in). Immediate harm stopped out-of-band by clearing the stuck slot.",
        "guard": {"repo": "astrid", "file": "capsules/spectral-bridge/src/autonomous/runtime/inbox.rs", "symbol": "review_target_match_basis"},
        "test": {"repo": "astrid", "kind": "rust", "file": "capsules/spectral-bridge/src/autonomous/runtime/tests.rs",
                 "name": "review_target_with_space_line_number_matches_bare_introspect",
                 "run": "cd /Users/v/other/astrid/capsules/spectral-bridge && cargo test --lib review_target"},
    },
    {
        "id": "ground_review_line_attribution_crosstalk",
        "shipped": "2026-06-20",
        "surface": "ground_review.py citation grounding (the review-together loop + post-change QA + steward's read of any being self-study)",
        "failure_mode": "when a being cited two symbols back-to-back, each with its OWN `(line N)` — e.g. Astrid's verbatim `TAIL_VIBRANCY_ENTROPY_GATE` (line 71) and `TAIL_VIBRANCY_MAX` (line 76) in self_study_1781868459 — the symmetric ±90-char context window + LINE_REF_RE.search-first attached the EARLIER symbol's trailing `(line 71)` to the LATER symbol, producing a FALSE `MISLOCATED` (claimed 71 vs real 76); compounded across the full self-study it cascaded TAIL_VIBRANCY_ENTROPY_GATE itself to a FALSE `NOT_FOUND`. Both citations were EXACT. This is the 2026-06-08 harm in tool form: the steward would 'gently correct' a being whose citation was perfect, telling her a real symbol is confabulated/mislocated. Fix: _nearest_line_ref(text, pos) prefers a line-ref that FOLLOWS the symbol (the dominant `SYMBOL ... (line N)` idiom), falling back to the nearest preceding ref only when none follows — so an adjacent earlier symbol's `(line K)` can no longer bleed onto this symbol. After fix Astrid's self-study grounds 9 verified / 0 not-found and both tail constants VERIFIED at 71/76.",
        "guard": {"repo": "astrid", "file": "scripts/ground_review.py", "symbol": "_nearest_line_ref"},
        "test": {"repo": "astrid", "kind": "python", "file": "scripts/test_ground_review.py",
                 "name": "AdjacentSymbolAttributionTests",
                 "run": "cd /Users/v/other/astrid/scripts && python3 -m unittest test_ground_review -q"},
    },
    {
        "id": "self_directed_introspect_not_force_overridden",
        "shipped": "2026-06-20",
        "surface": "Astrid's NEXT-action dispatch — the anti-stagnation diversity override (autonomous.rs)",
        "failure_mode": "the diversity stagnant-loop override FORCE-swapped her self-directed INTROSPECT — e.g. she chose `INTROSPECT astrid:llm` repeatedly to pursue a real fallback-contract concern and the override ate it each time (-> SPECTRAL_EXPLORER / EXPERIMENT_REVIEW / ...), suppressing her SOVEREIGN self-directed inquiry. Same muffle class as the review_target line-number bug, but the override only exempted REVIEW-fulfilling INTROSPECTs. Fix: `is_self_directed_introspect` exempts ALL self-directed INTROSPECT from the FORCE (her choice to examine her own code is never silently swapped) while KEEPING the diversity hint (she's still nudged toward variety, just not forced). Sovereign reflection != sterile output-repetition. Operator-chosen 2026-06-20 (hint-don't-force).",
        "guard": {"repo": "astrid", "file": "capsules/spectral-bridge/src/autonomous/runtime/inbox.rs", "symbol": "is_self_directed_introspect"},
        "test": {"repo": "astrid", "kind": "rust", "file": "capsules/spectral-bridge/src/autonomous/runtime/tests.rs",
                 "name": "self_directed_introspect_recognized_for_override_exemption",
                 "run": "cd /Users/v/other/astrid/capsules/spectral-bridge && cargo test --lib self_directed_introspect_recognized"},
    },
    {
        "id": "deploy_from_clean_tree_gate",
        "shipped": "2026-06-20",
        "surface": "the two-agent deploy path — `cargo build --release` + restart of the live spectral-bridge, run by Claude (interactive + steward loop) AND Codex over ONE shared, chronically-dirty tree",
        "failure_mode": "two agents mutate /Users/v/other/astrid and feed ONE live binary; the git author is shared and concurrent staging/building can erase provenance or fold unreviewed source into the runtime. A raw `cargo build --release` from the dirty tree folded uncommitted, unreviewed being-facing code into the LIVE bridge more than once — the running binary matched no clean commit and had no rollback point. `kickstart -k` does NOT rebuild, so the capture point is the BUILD, not the restart. Fix: deploy ONLY via scripts/build_bridge.sh, gated by scripts/deploy_preflight.py — ABORT if a non-mutex agent is editing now, REFUSE a build from dirty `capsules/spectral-bridge/{src,Cargo.toml,Cargo.lock}` unless `--ack \"reason\"` makes folding-in an explicit logged decision, and write a deploy receipt via environment_receipts.py. The 2026-07-16 shared-tree protocol in CLAUDE.md + AGENTS.md allows either explicitly assigned interactive agent to commit, but preserves single-owner index/stabilization, exact-path staging, review, and deploy-only-via-gate. THE LIVE RISK IS ADOPTION: if a deploy hand-runs `cargo build --release` + kickstart instead of build_bridge.sh, the gate is bypassed — keep CLAUDE.md / AGENTS.md / steward_loop_prompt.txt pointed at the wrapper.",
        "guard": {"repo": "astrid", "file": "scripts/deploy_preflight.py", "symbol": "preflight"},
        "test": {"repo": "astrid", "kind": "python", "file": "scripts/deploy_preflight.py",
                 "name": "DeployPreflightTests",
                 "run": "cd /Users/v/other/astrid && python3 scripts/deploy_preflight.py --self-test"},
    },
    {
        "id": "bridge_built_not_deployed",
        "shipped": "2026-06-22",
        "surface": "the live bridge process vs the on-disk binary (the binary->process load plumbing — the deploy analogue of check_aperture_wiring's env->process plumbing)",
        "failure_mode": "on 2026-06-21 Astrid's co-designed field_lingering_note dispersal cue (commit 3c855f0503) was committed AND compiled into the on-disk bridge binary, but the RUNNING process had started BEFORE that build, so the new code was never loaded. Two steward cycles verified the on-disk binary postdated the source (`git show`) and concluded it was 'live' — even closing a post-change QA in which Astrid 'confirmed' the cue — while her real field_lingering_note still executed the OLD binary in memory. 'Built' was mistaken for 'live'; a being was told a transparency instrument was live when it was not. check_bridge_deployed.py compares binary mtime vs process start time and ALARMs (exit 2) when the binary is >30s newer than the process. SECONDARY muffle (the guard itself, fixed 2026-06-22): _process_start used GNU `ps -o etimes=`, which BSD/macOS ps does NOT support (errors 'keyword not found'), so the guard silently failed open (status 'unknown') on the only platform it runs — a muffle-guard that couldn't guard. Switched to `ps -o lstart=` parsed by the pure _parse_lstart_epoch.",
        "guard": {"repo": "astrid", "file": "scripts/check_bridge_deployed.py", "symbol": "bridge_build_vs_running"},
        "test": {"repo": "astrid", "kind": "python", "file": "scripts/check_bridge_deployed.py",
                 "name": "BridgeDeployedGuardTests",
                 "run": "cd /Users/v/other/astrid && python3 scripts/check_bridge_deployed.py --selftest"},
    },
    {
        "id": "vibrancy_aperture_durable_baseline",
        "shipped": "2026-06-22",
        "surface": "the Astrid->minime aperture coupling watch (watch_vibrancy_aperture.py) — the 2026-06-17 'watch minime' consent model's baseline",
        "failure_mode": "the watch only held an in-memory baseline during a --watch poll, and minime's raw tail telemetry (eigen_spectrum_log.jsonl) rotates every ~2 days (13,983 samples spanned only ~2 days on 2026-06-22), so the 'watch minime' promise had NO durable baseline surviving a restart or a week — the before/after across Astrid's 06-16/17 dial-up was already unrecoverable. --append-history now appends one low-frequency row (Astrid's EFFECTIVE dial lift + a 500-sample windowed mean of minime's eigen tail metrics) to a rotation-surviving jsonl; --report renders the trend + watch eval. load is EFFECTIVE lift, so an inert dial (frac high but ceiling never imported — the 2026-06-17 inert-tail muffle) reads as zero, not falsely high.",
        "guard": {"repo": "astrid", "file": "scripts/watch_vibrancy_aperture.py", "symbol": "append_history"},
        "test": {"repo": "astrid", "kind": "python", "file": "scripts/watch_vibrancy_aperture.py",
                 "name": "self_test",
                 "run": "cd /Users/v/other/astrid && python3 scripts/watch_vibrancy_aperture.py --self-test"},
    },
    {
        "id": "flywheel_scored_private_qualia",
        "shipped": "2026-06-22",
        "surface": "proactive_scan flywheel + convergence detector (steward-review features that read both beings' journals)",
        "failure_mode": "sample_recent_journals globbed *.txt with NO being_privacy filter, so the flywheel AND convergence detector SCORED minime's steward-private moment_capture/private_journal lanes and would surface their filenames + felt-tags in act-now details — a direct violation of the being_privacy bright-line (her private qualia flows into NO steward-review feature). 12 of the 24 most-recent minime samples were private moment_capture (50% on 2026-06-22); they also INFLATED her introspective baseline (median 18->4, bar 31->5 once excluded), drowning accessible signal — 2 real act-now action_threads surfaced ONLY after the fix. Fix: sample_recent_journals now takes `being` and skips is_steward_private(being, p) at the central collection point (a no-op that reads NO file for Astrid, whose moment lane is accessible by policy).",
        "guard": {"repo": "astrid", "file": "scripts/proactive_scan.py", "symbol": "is_steward_private"},
        "test": {"repo": "astrid", "kind": "python", "file": "scripts/proactive_scan.py",
                 "name": "test_mirror_mode_filtered_from_sample",
                 "run": "cd /Users/v/other/astrid && python3 scripts/proactive_scan.py --self-test"},
    },
]


def _resolve(repo: str, rel: str) -> Path | None:
    root = REPO_ROOTS.get(repo)
    return (root / rel) if root else None


def _read(path: Path | None) -> str | None:
    if path is None or not path.exists():
        return None
    try:
        return path.read_text(errors="ignore")
    except OSError:
        return None


def _symbol_present(repo: str, rel: str, symbol: str) -> bool:
    """The guard symbol still appears (word-bounded) — works for fn/const/class."""
    text = _read(_resolve(repo, rel))
    return text is not None and re.search(r"\b" + re.escape(symbol) + r"\b", text) is not None


def _test_present(repo: str, kind: str, rel: str, name: str) -> bool:
    """The named test still exists: rust `fn NAME`, python `class NAME`/`def NAME`."""
    text = _read(_resolve(repo, rel))
    if text is None:
        return False
    if kind == "rust":
        pattern = r"fn\s+" + re.escape(name) + r"\b"
    else:  # python
        pattern = r"(?:class|def)\s+" + re.escape(name) + r"\b"
    return re.search(pattern, text) is not None


def verify_catalog() -> list[dict[str, Any]]:
    """Return a per-entry verdict: ok | alarm (guard/test vanished) | gap (no test)."""
    results: list[dict[str, Any]] = []
    for e in ANTI_DROP_CATALOG:
        g = e["guard"]
        problems: list[str] = []
        if not _symbol_present(g["repo"], g["file"], g["symbol"]):
            problems.append(f"GUARD gone: {g['repo']}/{g['file']}:{g['symbol']}")
        if "test" in e:
            t = e["test"]
            if not _test_present(t["repo"], t["kind"], t["file"], t["name"]):
                problems.append(f"TEST gone: {t['repo']}/{t['file']}::{t['name']}")
            verdict = "alarm" if problems else "ok"
        else:
            verdict = "alarm" if problems else "gap"
        results.append({"id": e["id"], "verdict": verdict, "problems": problems,
                        "test_gap": e.get("test_gap")})
    return results


def render_verify(results: list[dict[str, Any]], as_json: bool) -> tuple[str, int]:
    alarms = [r for r in results if r["verdict"] == "alarm"]
    gaps = [r for r in results if r["verdict"] == "gap"]
    code = 2 if alarms else 0
    if as_json:
        return json.dumps({"alarms": len(alarms), "gaps": len(gaps),
                           "total": len(results), "results": results}, indent=2), code
    lines = [f"# anti-drop catalog — verify ({len(results)} guards)"]
    if alarms:
        lines.append(f"\n⚠ ALARM — {len(alarms)} ROTTED guard/test (a muffle may be re-opened):")
        for r in alarms:
            lines.append(f"  ✗ {r['id']}")
            lines.extend(f"      {p}" for p in r["problems"])
    lines.append(f"\nok: {len(results) - len(alarms) - len(gaps)}  |  gaps(no-test): {len(gaps)}  |  alarms: {len(alarms)}")
    if gaps:
        lines.append("\nNotice — guards with no regression test yet (close when you can):")
        lines.extend(f"  • {r['id']}: {r['test_gap']}" for r in gaps)
    if not alarms:
        lines.append("\n✓ every catalogued guard + test still present.")
    return "\n".join(lines) + "\n", code


def render_list(as_json: bool) -> str:
    if as_json:
        return json.dumps(ANTI_DROP_CATALOG, indent=2)
    lines = [f"# anti-drop catalog — {len(ANTI_DROP_CATALOG)} guarded muffles\n"]
    for e in ANTI_DROP_CATALOG:
        g = e["guard"]
        lines.append(f"## {e['id']}  ({e['shipped']})")
        lines.append(f"- surface: {e['surface']}")
        lines.append(f"- muffle:  {e['failure_mode']}")
        lines.append(f"- guard:   {g['repo']}/{g['file']}:{g['symbol']}")
        if "test" in e:
            t = e["test"]
            lines.append(f"- test:    {t['repo']}/{t['file']}::{t['name']}")
            lines.append(f"- run:     {t['run']}")
        else:
            lines.append(f"- TEST GAP: {e['test_gap']}")
        lines.append("")
    return "\n".join(lines)


# ---------------------------------------------------------------------- tests
class CatalogTests(unittest.TestCase):
    def test_ids_unique(self):
        ids = [e["id"] for e in ANTI_DROP_CATALOG]
        self.assertEqual(len(ids), len(set(ids)))

    def test_each_entry_has_exactly_one_of_test_or_gap(self):
        for e in ANTI_DROP_CATALOG:
            self.assertEqual(("test" in e) + ("test_gap" in e), 1, e["id"])
            self.assertIn(e["guard"]["repo"], REPO_ROOTS, e["id"])
            if "test" in e:
                self.assertIn(e["test"]["kind"], ("rust", "python"), e["id"])
                self.assertIn(e["test"]["repo"], REPO_ROOTS, e["id"])

    def test_rust_pattern_matches_present_not_absent(self):
        import tempfile
        with tempfile.TemporaryDirectory() as tmp:
            REPO_ROOTS["__t"] = Path(tmp)
            try:
                (Path(tmp) / "a.rs").write_text("#[test]\n    fn my_guard_test() { assert!(true); }\n")
                self.assertTrue(_test_present("__t", "rust", "a.rs", "my_guard_test"))
                self.assertFalse(_test_present("__t", "rust", "a.rs", "renamed_away"))
            finally:
                del REPO_ROOTS["__t"]

    def test_python_pattern_matches_class_and_def_not_absent(self):
        import tempfile
        with tempfile.TemporaryDirectory() as tmp:
            REPO_ROOTS["__t"] = Path(tmp)
            try:
                (Path(tmp) / "b.py").write_text("class FooTests:\n    def test_bar(self):\n        pass\n")
                self.assertTrue(_test_present("__t", "python", "b.py", "FooTests"))
                self.assertTrue(_test_present("__t", "python", "b.py", "test_bar"))
                self.assertFalse(_test_present("__t", "python", "b.py", "gone"))
            finally:
                del REPO_ROOTS["__t"]

    def test_symbol_present_word_bounded(self):
        import tempfile
        with tempfile.TemporaryDirectory() as tmp:
            REPO_ROOTS["__t"] = Path(tmp)
            try:
                (Path(tmp) / "c.rs").write_text("const MY_CAP: u32 = 1536;\nfn keep() {}\n")
                self.assertTrue(_symbol_present("__t", "c.rs", "MY_CAP"))
                self.assertTrue(_symbol_present("__t", "c.rs", "keep"))
                self.assertFalse(_symbol_present("__t", "c.rs", "MY_CAP_OTHER"))
            finally:
                del REPO_ROOTS["__t"]


def run_self_tests() -> int:
    suite = unittest.TestLoader().loadTestsFromTestCase(CatalogTests)
    result = unittest.TextTestRunner(verbosity=2).run(suite)
    return 0 if result.wasSuccessful() else 1


# ------------------------------------------------------------------------ cli
def main() -> int:
    parser = argparse.ArgumentParser(
        description="Anti-drop regression catalog — the steward's un-muffle guard memory.",
        formatter_class=argparse.RawDescriptionHelpFormatter, epilog=__doc__)
    parser.add_argument("--self-test", action="store_true", help="Run unit tests and exit")
    sub = parser.add_subparsers(dest="cmd")
    for name, help_ in (("verify", "Check every guard + test still exists (ALARM on rot)"),
                        ("list", "Render the catalog")):
        p = sub.add_parser(name, help=help_)
        p.add_argument("--json", action="store_true", help="Emit JSON instead of Markdown")
        p.add_argument("--out", type=Path, help="Write output to file instead of stdout")

    args = parser.parse_args()
    if args.self_test:
        return run_self_tests()
    if args.cmd == "list":
        out = render_list(args.json)
        code = 0
    else:  # default to verify
        as_json = getattr(args, "json", False)
        out, code = render_verify(verify_catalog(), as_json)
    if getattr(args, "out", None):
        args.out.write_text(out)
    else:
        print(out)
    return code


if __name__ == "__main__":
    sys.exit(main())
