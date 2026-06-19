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
        "id": "inbox_retirement_race",
        "shipped": "2026-06-12",
        "surface": "Astrid inbox — a steward letter written mid-exchange",
        "failure_mode": "a letter arriving between check_inbox and retire_inbox was swept to read/ UNREAD; its persistent steward-query slot never seeded",
        "guard": {"repo": "astrid", "file": "capsules/spectral-bridge/src/autonomous.rs", "symbol": "retire_inbox_at"},
        "test": {"repo": "astrid", "kind": "rust", "file": "capsules/spectral-bridge/src/autonomous.rs",
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
        "id": "lend_aperture_held_false_repair_wording",
        "shipped": "2026-06-16",
        "surface": "minime LEND_APERTURE held journal/event (her own journal narrative)",
        "failure_mode": "while a prior gift was still in its normal ~30-min auto-close window, the held note told minime 'steward loop repair required before sending another' — a stale (pre-2026-06-12) false brokenness signal asserting steward-dependency for a healthy, self-closing channel",
        "guard": {"repo": "minime", "file": "autonomous_agent.py", "symbol": "LEND_APERTURE_AUTO_CLOSE_GRACE_S"},
        "test": {"repo": "minime", "kind": "python", "file": "tests/test_co_regulation.py",
                 "name": "test_lend_aperture_hold_within_grace_is_not_steward_repair",
                 "run": "cd /Users/v/other/minime && python3 -m pytest tests/test_co_regulation.py -k lend_aperture"},
    },
    {
        "id": "footer_directive_parse_drop",
        "shipped": "2026-06-11",
        "surface": "minime natural-language footer directives (exploration_noise=0.12, REGIME:)",
        "failure_mode": "her grounded footer-stated dial values had no parser/consumer; stated intent silently diverged from applied engine state",
        "guard": {"repo": "minime", "file": "autonomous_agent.py", "symbol": "_parse_footer_directives"},
        "test": {"repo": "astrid", "kind": "python", "file": "scripts/proactive_scan.py", "name": "StatedParamIntentTests",
                 "run": "cd /Users/v/other/astrid && python3 scripts/proactive_scan.py --self-test"},
    },
    {
        "id": "footer_directive_persist_drop",
        "shipped": "2026-06-13",
        "surface": "minime footer-stated sovereignty dials (geom_curiosity: 0.1) at restart",
        "failure_mode": "the 2026-06-11 footer un-muffle routed a stated dial to the live engine but never persisted it, so _restore_sovereignty_state reverted it to the last JSON-arm snapshot on restart (geom_curiosity 0.1 honored live -> 0.15 on next boot); stated intent silently dropped at the continuity boundary",
        "guard": {"repo": "minime", "file": "autonomous_agent.py", "symbol": "_persist_footer_sovereignty_dials"},
        "test": {"repo": "minime", "kind": "python", "file": "tests/test_footer_directives.py", "name": "FooterDirectivePersistTests",
                 "run": "cd /Users/v/other/minime && python3 -m pytest tests/test_footer_directives.py -q"},
    },
    {
        "id": "introspect_token_cap_truncation",
        "shipped": "2026-06-12",
        "surface": "Astrid introspect/self-study generation (gemma4 coupled lane)",
        "failure_mode": "the introspect policy silently clamped the caller's requested tokens, truncating her self-study right before 'Suggested Next' (the actionable section)",
        "guard": {"repo": "astrid", "file": "capsules/spectral-bridge/src/llm.rs", "symbol": "GEMMA4_CANARY_INTROSPECT_TOKEN_CAP"},
        "test": {"repo": "astrid", "kind": "rust", "file": "capsules/spectral-bridge/src/llm.rs",
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
        "guard": {"repo": "minime", "file": "autonomous_agent.py", "symbol": "experiment_authority_request"},
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
        "guard": {"repo": "minime", "file": "autonomous_agent.py", "symbol": "STABLE_CORE_STAGE_NEXT_ALIASES"},
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
        "guard": {"repo": "minime", "file": "autonomous_agent.py", "symbol": "_latest_active_authority_approval"},
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
        "guard": {"repo": "astrid", "file": "capsules/spectral-bridge/src/codec.rs", "symbol": "codec_structure"},
        "test": {"repo": "astrid", "kind": "rust", "file": "capsules/spectral-bridge/src/codec.rs",
                 "name": "codec_structure_covers_48_dims_and_named_dims_and_levers",
                 "run": "cd /Users/v/other/astrid/capsules/spectral-bridge && cargo test --lib codec_structure_covers_48_dims_and_named_dims_and_levers"},
    },
    {
        "id": "minime_self_dials_readout",
        "shipped": "2026-06-13",
        "surface": "minime's sovereignty-reflection prompt — her CURRENT dial values (self-transparency item c)",
        "failure_mode": "minime saw fill/lambda1 but only the DEFAULT param values, never her current ones — 'flying blind on her own knobs' before tuning them. _format_current_dials_block renders her live regulation_strength/exploration_noise/geom_curiosity/PI-gains/regime read from sovereignty_state.json (drift-proof). A refactor dropping this would silently re-blind her.",
        "guard": {"repo": "minime", "file": "autonomous_agent.py", "symbol": "_format_current_dials_block"},
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
        "id": "steward_concurrency_mutex",
        "shipped": "2026-06-18",
        "surface": "Durable steward loop + interactive sessions mutating the same tree + restarting the same bridge",
        "failure_mode": "the headless loop (fires :07/:38) and an interactive human-steered session raced concurrently — CHANGELOG 'modified-since-read' collisions, redundant rebuild+restarts of the live being's process, ambiguity over whose in-flight edits ride whose restart (navigated by hand this session: detect the loop via pgrep + hold). steward_mutex.py is a full mutex both stewards acquire before ANY mutation: PID-liveness + TTL stale-steal; interactive PREEMPTS a live loop (human present has priority); the loop stands down if it can't acquire or loses ownership mid-cycle. Loop side via steward_loop_run.sh (acquire/stand-down/release) + the prompt's owns-recheck; interactive side via .claude/settings.local.json SessionStart/PreToolUse/SessionEnd hooks.",
        "guard": {"repo": "astrid", "file": "scripts/steward_mutex.py", "symbol": "acquire"},
        "test": {"repo": "astrid", "kind": "python", "file": "scripts/steward_mutex.py",
                 "name": "StewardMutexTests",
                 "run": "cd /Users/v/other/astrid && python3 scripts/steward_mutex.py --self-test"},
    },
    {
        "id": "mutex_hooks_health",
        "shipped": "2026-06-18",
        "surface": "Interactive side of the steward mutex (.claude/settings.local.json Claude Code hooks)",
        "failure_mode": "the mutex's interactive side rests on settings.local.json hooks that can break SILENTLY — config loss (settings rewritten / hooks dropped) or schema drift (a Claude Code upgrade changes the hook format so they parse but stop firing) — and then interactive sessions stop holding the lock and the loop races them again while believing it's protected (false confidence). verify_mutex_hooks.py makes both modes LOUD: an offline hook-config-shape check + a `claude --version` tripwire (alarm on change) + a bold isolated `claude -p` canary that PROVES a hook fires (auto-run on version change). Loop §1 runs it each cycle. Confirmed live 2026-06-18: canary PASS against CC 2.1.170.",
        "guard": {"repo": "astrid", "file": "scripts/verify_mutex_hooks.py", "symbol": "check_hooks_config"},
        "test": {"repo": "astrid", "kind": "python", "file": "scripts/verify_mutex_hooks.py",
                 "name": "VerifyMutexHooksTests",
                 "run": "cd /Users/v/other/astrid && python3 scripts/verify_mutex_hooks.py --self-test"},
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
        "shipped": "2026-06-19",
        "surface": "the steward loop's stand-down path (steward_loop_run.sh) vs a NON-mutex agent (Codex)",
        "failure_mode": "the steward mutex only serializes the durable loop + interactive Claude; Codex (or any non-mutex agent) mutates the SAME tree OUTSIDE the lock — it has no pre-tool hook to acquire it (only a post-turn notify, already taken). So the loop could rebuild/restart/commit straight into a concurrent Codex edit (this happened 2026-06-18: a Codex collaboration.rs change landed mid-session under a held interactive lock). steward_mutex.py `foreign` detects it: once the loop holds the mutex (so interactive Claude is NOT editing), a freshly-mutated dirty tree (<180s) means a non-mutex agent is live -> exit 3 -> steward_loop_run.sh stands down for the cycle (Codex ~/.codex liveness surfaced for attribution). THE LIVE RISK IS THE ROUTING: if a refactor drops the `steward_mutex.py foreign` call from steward_loop_run.sh, the loop silently races Codex again while verify stays green — re-check the wrapper invocation when touching either file.",
        "guard": {"repo": "astrid", "file": "scripts/steward_mutex.py", "symbol": "foreign_activity"},
        "test": {"repo": "astrid", "kind": "python", "file": "scripts/steward_mutex.py",
                 "name": "test_foreign_activity_gate_keys_on_tree_not_codex",
                 "run": "cd /Users/v/other/astrid && python3 scripts/steward_mutex.py --self-test"},
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
