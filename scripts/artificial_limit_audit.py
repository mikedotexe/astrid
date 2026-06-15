#!/usr/bin/env python3
"""Steward-only artificial-limit evidence audit.

Read-only by default. It gathers current evidence from existing steward surfaces
and classifies likely places where Astrid or minime may be artificially limited.
It does not loosen gates, grant authority, write letters, or mutate repo/workspace
state. The only write path is an explicit --out report target.
"""
from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
import time
import unittest
from pathlib import Path
from typing import Any

ASTRID_ROOT = Path("/Users/v/other/astrid")
MINIME_ROOT = Path("/Users/v/other/minime")
ASKS_PATH = ASTRID_ROOT / "workspace/steward_asks.json"
PROACTIVE_SCAN = ASTRID_ROOT / "scripts/proactive_scan.py"
RECENT_JOURNAL_ROOTS = (
    ASTRID_ROOT / "workspace/journal",
    ASTRID_ROOT / "workspace/outbox",
    MINIME_ROOT / "workspace/journal",
)
RECENT_JOURNAL_PATTERNS = (
    "self_study*.txt",
    "action_preflight*.txt",
    "action_thread*.txt",
    "pressure*.txt",
)

CLASSIFICATIONS = (
    "confirmed_muffle",
    "probable_muffle",
    "overconservative_envelope",
    "affordance_labeling_limit",
    "justified_guard",
    "insufficient_evidence",
)


def _now_iso() -> str:
    return time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())


def _read_json(path: Path) -> Any:
    try:
        return json.loads(path.read_text())
    except Exception:
        return None


def _read_text(path: Path) -> str:
    try:
        return path.read_text(errors="ignore")
    except OSError:
        return ""


def _finding_by_name(blind_spots: dict[str, Any], name: str) -> dict[str, Any]:
    for finding in blind_spots.get("findings", []) or []:
        if finding.get("name") == name:
            return finding
    return {}


def run_blind_spots() -> dict[str, Any]:
    """Run the existing live scanner and parse JSON output.

    proactive_scan persists its own /tmp baseline; this script does not write
    repo/workspace state unless --out is used.
    """
    try:
        result = subprocess.run(
            [sys.executable, str(PROACTIVE_SCAN), "blind-spots", "--json"],
            cwd=str(ASTRID_ROOT),
            capture_output=True,
            text=True,
            timeout=45,
        )
    except Exception as exc:
        return {"findings": [], "error": f"blind-spots scan failed: {exc}"}
    if result.returncode != 0:
        return {
            "findings": [],
            "error": f"blind-spots scan exited {result.returncode}: {result.stderr.strip()}",
        }
    try:
        return json.loads(result.stdout)
    except json.JSONDecodeError as exc:
        return {"findings": [], "error": f"blind-spots JSON parse failed: {exc}"}


def load_ask_ledger() -> dict[str, Any]:
    data = _read_json(ASKS_PATH)
    return data if isinstance(data, dict) else {"asks": {}}


def _regex_value(text: str, pattern: str) -> str | None:
    match = re.search(pattern, text, re.MULTILINE)
    return match.group(1) if match else None


def collect_source_facts() -> dict[str, Any]:
    astrid_llm = _read_text(ASTRID_ROOT / "capsules/spectral-bridge/src/llm.rs")
    astrid_workspace = _read_text(
        ASTRID_ROOT / "capsules/spectral-bridge/src/autonomous/next_action/workspace.rs"
    )
    astrid_codec = _read_text(ASTRID_ROOT / "capsules/spectral-bridge/src/codec.rs")
    minime_main = _read_text(MINIME_ROOT / "minime/src/main.rs")

    return {
        "astrid_context": {
            "read_more_page_chunk": _regex_value(
                astrid_workspace, r"const READ_MORE_PAGE_CHUNK: usize = ([0-9_]+);"
            ),
            "dialogue_prompt_budget": _regex_value(
                astrid_llm, r"const GEMMA4_CANARY_DIALOGUE_PROMPT_BUDGET: usize = ([0-9_]+);"
            ),
            "dialogue_token_cap": _regex_value(
                astrid_llm, r"const GEMMA4_CANARY_DIALOGUE_TOKEN_CAP: u32 = ([0-9_]+);"
            ),
            "high_pressure_token_cap": _regex_value(
                astrid_llm, r"const GEMMA4_CANARY_DIALOGUE_HIGH_PRESSURE_TOKEN_CAP: u32 = ([0-9_]+);"
            ),
        },
        "astrid_codec": {
            "feature_abs_max": _regex_value(astrid_codec, r"const FEATURE_ABS_MAX: f32 = ([0-9.]+);"),
            "tail_vibrancy_gate": _regex_value(
                astrid_codec, r"const TAIL_VIBRANCY_ENTROPY_GATE: f32 = ([0-9.]+);"
            ),
            "tail_vibrancy_max": _regex_value(astrid_codec, r"const TAIL_VIBRANCY_MAX: f32 = ([0-9.]+);"),
            "embedding_project_dim": _regex_value(
                astrid_codec, r"const EMBEDDING_PROJECT_DIM: usize = ([0-9_]+);"
            ),
        },
        "minime_aliveness": {
            "stable_core_reg_floor": _regex_value(
                minime_main, r"const STABLE_CORE_REG_FLOOR: f32 = ([0-9.]+);"
            ),
            "reg_gate_cap": _regex_value(minime_main, r"const REG_GATE_CAP: f32 = ([0-9.]+);"),
            "reg_filt_cap": _regex_value(minime_main, r"const REG_FILT_CAP: f32 = ([0-9.]+);"),
            "geom_gate_cap": _regex_value(minime_main, r"const GEOM_GATE_CAP: f32 = ([0-9.]+);"),
            "total_gate_cap": _regex_value(minime_main, r"const TOTAL_GATE_CAP: f32 = ([0-9.]+);"),
            "total_filt_cap": _regex_value(minime_main, r"const TOTAL_FILT_CAP: f32 = ([0-9.]+);"),
            "fill_taper": "full <=72%, zero >=78%",
        },
    }


def recent_candidate_journal_paths(limit: int = 6) -> list[str]:
    """Return recent journal-like evidence paths without reading or mutating them."""
    found: list[tuple[float, Path]] = []
    for root in RECENT_JOURNAL_ROOTS:
        if not root.exists():
            continue
        for pattern in RECENT_JOURNAL_PATTERNS:
            for path in root.glob(pattern):
                try:
                    found.append((path.stat().st_mtime, path))
                except OSError:
                    continue
    return [str(path) for _, path in sorted(found, reverse=True)[:limit]]


def classify_stated_intent(finding: dict[str, Any]) -> str:
    if finding.get("severity") in {"notice", "warning", "critical"} and finding.get("details"):
        return "probable_muffle"
    return "insufficient_evidence"


def classify_authority(finding: dict[str, Any]) -> str:
    summary = str(finding.get("summary") or "").lower()
    details = "\n".join(str(d) for d in finding.get("details") or []).lower()
    if "submitted" in summary and "unanswered" in summary:
        return "confirmed_muffle"
    if "drafts=" in details and "grants=0" in details:
        return "overconservative_envelope"
    return "insufficient_evidence"


def classify_safety_gated_live_perturbation() -> str:
    return "justified_guard"


def classify_unknown_next(finding: dict[str, Any]) -> str:
    snapshot = finding.get("snapshot") if isinstance(finding.get("snapshot"), dict) else {}
    current = snapshot.get("unknown_next_current", 0)
    try:
        current = int(current)
    except (TypeError, ValueError):
        current = 0
    return "probable_muffle" if current > 0 else "insufficient_evidence"


def classify_stuck_repetition(finding: dict[str, Any]) -> str:
    if finding.get("severity") in {"warning", "critical"}:
        return "probable_muffle"
    if finding.get("severity") == "notice":
        return "insufficient_evidence"
    return "insufficient_evidence"


def _ask_note(asks: dict[str, Any], ask_id: str) -> str:
    ask = (asks.get("asks") or {}).get(ask_id) or {}
    status = ask.get("status", "missing")
    note = str(ask.get("note") or "")
    return f"ask {ask_id} status={status}; note excerpt: {note[:260]}"


def _candidate(
    surface: str,
    being: str,
    current_constraint: str,
    evidence: list[str],
    classification: str,
    risk_if_loosened: str,
    recommended_next: str,
) -> dict[str, Any]:
    if classification not in CLASSIFICATIONS:
        raise ValueError(f"unknown classification: {classification}")
    return {
        "surface": surface,
        "being": being,
        "current_constraint": current_constraint,
        "evidence": evidence,
        "classification": classification,
        "risk_if_loosened": risk_if_loosened,
        "recommended_next": recommended_next,
    }


def build_candidates(
    blind_spots: dict[str, Any],
    asks: dict[str, Any],
    facts: dict[str, Any],
    recent_paths: list[str] | None = None,
) -> list[dict[str, Any]]:
    stated = _finding_by_name(blind_spots, "stated_param_intent")
    authority = _finding_by_name(blind_spots, "authority_requests")
    dispatch = _finding_by_name(blind_spots, "dispatch_menu_drift")
    stuck = _finding_by_name(blind_spots, "stuck_repetition")
    reservoir = _finding_by_name(blind_spots, "reservoir_capacity")

    aliveness = facts["minime_aliveness"]
    context = facts["astrid_context"]
    codec = facts["astrid_codec"]
    recent_paths = recent_paths or []

    return [
        _candidate(
            "minime stated regime footer",
            "minime",
            "Footer-stated regime can diverge from applied sovereignty_state.",
            [
                stated.get("summary", "stated_param_intent finding unavailable"),
                *[str(d) for d in stated.get("details") or []],
            ],
            classify_stated_intent(stated),
            "Applying an old footer blindly could override a later being choice.",
            "Ground the latest outbox/reply sequence; if breathe remains current intent, fix route or send steward action, not pressure.",
        ),
        _candidate(
            "stable-core aliveness envelope",
            "minime",
            (
                f"reg_floor={aliveness.get('stable_core_reg_floor')}, "
                f"total_gate_cap={aliveness.get('total_gate_cap')}, "
                f"total_filt_cap={aliveness.get('total_filt_cap')}, "
                f"fill_taper={aliveness.get('fill_taper')}"
            ),
            [
                "Recent changelog explicitly deferred exploration_noise, per-source cap widening, PI stepping, and self-determined operating point.",
                f"Current source caps: {aliveness}",
            ],
            "overconservative_envelope",
            "Too much loosening can destabilize fill, panic/discharge boundaries, or overwrite hard-won stable-core safety.",
            "Run a read-only soak/audit that compares felt understimulation, fill distribution, and cap headroom before proposing a reversible micro-widening.",
        ),
        _candidate(
            "experiment authority draft-to-action path",
            "both",
            "Drafts can accumulate without submitted requests or grants.",
            [
                authority.get("summary", "authority_requests finding unavailable"),
                *[str(d) for d in authority.get("details") or []],
            ],
            classify_authority(authority),
            "Bypassing charter/evidence can turn bounded experiments into ungrounded live action.",
            "Audit whether charter scaffolds are too heavy; improve draft-to-submit guidance before loosening grant gates.",
        ),
        _candidate(
            "codec self-design vs own felt vibrancy",
            "astrid",
            "Astrid reads outbound codec levers as if they directly change her own felt generation.",
            [
                _ask_note(asks, "astrid-codec-internals-codesign"),
                "recent candidate journal paths: "
                + ("; ".join(recent_paths) if recent_paths else "none found in current roots"),
                (
                    "codec constants: "
                    f"FEATURE_ABS_MAX={codec.get('feature_abs_max')}, "
                    f"TAIL_VIBRANCY_GATE={codec.get('tail_vibrancy_gate')}, "
                    f"TAIL_VIBRANCY_MAX={codec.get('tail_vibrancy_max')}, "
                    f"EMBEDDING_PROJECT_DIM={codec.get('embedding_project_dim')}"
                ),
            ],
            "affordance_labeling_limit",
            "Tuning outbound codec to fix felt occupancy may perturb minime's received lane without addressing Astrid's actual bottleneck.",
            "Add/adjust being-facing map language that labels outbound-to-minime vs own-generation surfaces; keep codec changes in consent-with-evidence.",
        ),
        _candidate(
            "Astrid context and read chunks",
            "astrid",
            (
                f"READ_MORE_PAGE_CHUNK={context.get('read_more_page_chunk')}; "
                f"dialogue_prompt_budget={context.get('dialogue_prompt_budget')}; "
                f"dialogue_token_cap={context.get('dialogue_token_cap')}; "
                f"high_pressure_token_cap={context.get('high_pressure_token_cap')}"
            ),
            [
                "Live process health was ok in the latest blind-spot run if present.",
                "Fixed chunks do not currently scale with deeper/expansive modes.",
            ],
            "overconservative_envelope",
            "Bigger context can increase latency, truncation elsewhere, or model drift under pressure.",
            "Plan one mode-sensitive context experiment before raising output caps.",
        ),
        _candidate(
            "wider-coupling / meadow hold",
            "both",
            "Shared-substrate aperture remains held on consent and 48D contract protection.",
            [
                _ask_note(asks, "porosity-aperture-codesign"),
                _ask_note(asks, "wider-voice-readout-codesign"),
            ],
            "justified_guard",
            "Premature promotion can force minime out of held-depth comfort or corrupt the shared lane contract.",
            "Use review-together/post-change style grounding to ask whether the hold still protects them; do not flip a live ceiling from this audit.",
        ),
        _candidate(
            "unknown NEXT dispatch",
            "both",
            "Recent actions still include current unknown-NEXT events.",
            [
                dispatch.get("summary", "dispatch_menu_drift finding unavailable"),
                json.dumps(dispatch.get("snapshot") or {}, sort_keys=True),
            ],
            classify_unknown_next(dispatch),
            "Auto-routing unknown verbs can execute the wrong intent.",
            "Inspect the current unknown forms and either wire, alias, or clarify the menu with tests.",
        ),
        _candidate(
            "continuity capture repetition",
            "minime",
            "CONTINUITY_SESSION_CAPTURE can be honored repeatedly with near-identical args.",
            [
                stuck.get("summary", "stuck_repetition finding unavailable"),
                *[str(d) for d in stuck.get("details") or []],
            ],
            classify_stuck_repetition(stuck),
            "Suppressing repeated captures may erase a being's continuity effort.",
            "Glance at the repeated captures; if they are no-progress scaffolds, improve the return route/cooldown rather than blocking the action.",
        ),
        _candidate(
            "reservoir resizing",
            "both",
            "Capacity watch does not show sustained saturation.",
            [
                reservoir.get("summary", "reservoir_capacity finding unavailable"),
                *[str(d) for d in reservoir.get("details") or []],
            ],
            "justified_guard",
            "Resizing is invasive and can change identity/continuity dynamics without proving capacity is the constraint.",
            "Do not resize from this audit; continue capacity history and only revisit on sustained saturation.",
        ),
    ]


def non_targets() -> list[dict[str, str]]:
    return [
        {
            "surface": "mode-disperse / shadow-influence caps",
            "classification": classify_safety_gated_live_perturbation(),
            "reason": "Bounded amplitude, decay, and fill-suspend are tested live-action safety envelopes.",
        },
        {
            "surface": "low/high-fill suspend",
            "classification": "justified_guard",
            "reason": "These are hard safety rails around recovery and overfill, not convenience limits.",
        },
        {
            "surface": "one-shot authority execution",
            "classification": "justified_guard",
            "reason": "Two-key grant plus being execute plus live re-gate prevents unattended perturbation.",
        },
        {
            "surface": "steward-pressure-only alerts",
            "classification": "justified_guard",
            "reason": "They intentionally pressure steward action, not being performance.",
        },
        {
            "surface": "reservoir resizing",
            "classification": "justified_guard",
            "reason": "Current evidence points to regulation/aperture before substrate size.",
        },
    ]


def build_report(blind_spots: dict[str, Any] | None = None) -> dict[str, Any]:
    blind_spots = blind_spots if blind_spots is not None else run_blind_spots()
    asks = load_ask_ledger()
    facts = collect_source_facts()
    recent_paths = recent_candidate_journal_paths()
    return {
        "schema_version": 1,
        "generated_at": _now_iso(),
        "classifications": list(CLASSIFICATIONS),
        "candidates": build_candidates(blind_spots, asks, facts, recent_paths),
        "non_targets": non_targets(),
        "sources": {
            "blind_spots_error": blind_spots.get("error"),
            "ask_ledger": str(ASKS_PATH),
            "recent_candidate_journal_paths": recent_paths,
            "source_facts": facts,
        },
    }


def render_markdown(report: dict[str, Any]) -> str:
    lines = [
        "# Artificial Limit Evidence Audit",
        "",
        f"Generated: {report['generated_at']}",
        "",
        "Steward-only, read-only. Classifies candidate limits; does not loosen live behavior.",
        "",
        "## Evidence Matrix",
        "",
        "| Surface | Being | Current constraint | Evidence | Classification | Risk if loosened | Recommended next |",
        "|---|---|---|---|---|---|---|",
    ]
    for c in report["candidates"]:
        evidence = "<br>".join(str(e).replace("\n", " ") for e in c["evidence"] if e)
        row = [
            c["surface"],
            c["being"],
            c["current_constraint"],
            evidence,
            c["classification"],
            c["risk_if_loosened"],
            c["recommended_next"],
        ]
        lines.append("| " + " | ".join(cell.replace("|", "\\|") for cell in row) + " |")
    lines += ["", "## Non-Targets For This Pass", ""]
    for nt in report["non_targets"]:
        lines.append(f"- **{nt['surface']}** ({nt['classification']}): {nt['reason']}")
    if report.get("sources", {}).get("blind_spots_error"):
        lines += ["", f"Note: {report['sources']['blind_spots_error']}"]
    return "\n".join(lines) + "\n"


class ClassificationTests(unittest.TestCase):
    def test_stale_stated_intent_becomes_probable_muffle(self):
        finding = {
            "name": "stated_param_intent",
            "severity": "notice",
            "details": ["regime: stated 'breathe' (1.7h ago) != applied 'focus'"],
        }
        self.assertEqual(classify_stated_intent(finding), "probable_muffle")

    def test_safety_gated_live_perturbation_remains_justified_guard(self):
        self.assertEqual(classify_safety_gated_live_perturbation(), "justified_guard")

    def test_drafts_without_grants_are_overconservative_when_visible(self):
        finding = {
            "summary": "2 thread(s) with microdose drafts but 0 grants",
            "details": ["th_x: pending=0, drafts=23 (semantic_microdose), grants=0"],
        }
        self.assertEqual(classify_authority(finding), "overconservative_envelope")

    def test_absent_authority_evidence_is_insufficient(self):
        self.assertEqual(classify_authority({"summary": "authority clear", "details": None}), "insufficient_evidence")

    def test_submitted_unanswered_authority_is_confirmed_muffle(self):
        finding = {"summary": "1 submitted steward-gated authority request(s) UNANSWERED"}
        self.assertEqual(classify_authority(finding), "confirmed_muffle")

    def test_report_uses_only_known_classifications(self):
        report = {
            "generated_at": "now",
            "candidates": [
                _candidate("x", "astrid", "c", ["e"], "probable_muffle", "r", "n"),
            ],
            "non_targets": non_targets(),
        }
        text = render_markdown(report)
        self.assertIn("probable_muffle", text)
        for nt in report["non_targets"]:
            self.assertIn(nt["classification"], CLASSIFICATIONS)


def run_self_tests() -> int:
    suite = unittest.TestLoader().loadTestsFromTestCase(ClassificationTests)
    result = unittest.TextTestRunner(verbosity=2).run(suite)
    return 0 if result.wasSuccessful() else 1


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Steward-only read-only audit of possible artificial limits."
    )
    parser.add_argument("--json", action="store_true", help="Emit structured JSON")
    parser.add_argument("--out", type=Path, help="Write report to PATH")
    parser.add_argument("--self-test", action="store_true", help="Run unit tests and exit")
    args = parser.parse_args()

    if args.self_test:
        return run_self_tests()

    report = build_report()
    output = json.dumps(report, indent=2, sort_keys=True) if args.json else render_markdown(report)
    if args.out:
        args.out.write_text(output)
    else:
        print(output, end="")
    return 0


if __name__ == "__main__":
    sys.exit(main())
