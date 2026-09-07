#!/usr/bin/env python3
"""Grant shortlist: the reviewable view of the Tier-5 authority-wait pile.

Consumes a consolidation plan (authority_wait_consolidation.build_plan) and
the sandbox trial store, groups ask-families by live-risk surface (the same
seven domains the readiness map uses), and emits a compact operator menu:
which asks the beings keep making, how persistently, what evidence exists,
and the exact commands to gather evidence or record a grant. Textual
consolidation stays conservative (never over-merge a being's ask); THIS view
is where "fallback prompt weighting" and "provider route, sampler behavior"
correctly share one grant scope.

Evidence honesty: a family only counts as evidenced when a NON-STUB adapter
result exists (run-evidence / sandbox runs); families on surfaces with no
offline adapter are flagged needs_manual_review_or_new_adapter, never faked.

Read-only. Grants remain exclusively Mike/operator acts.
"""

from __future__ import annotations

import sys
from pathlib import Path
from typing import Any

SCRIPTS_DIR = Path(__file__).resolve().parent
if str(SCRIPTS_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPTS_DIR))

from authority_wait_readiness import DOMAIN_DEFINITIONS, keyword_matches  # noqa: E402

STUBS = frozenset({"approval_required_provider_change", "insufficient_output"})
OFFLINE_ADAPTERS = frozenset(
    {"fallback_distinguishability_v1", "shadow_loss_lattice_v1", "shadow_influence_replay_v1"}
)


def family_domain(family: dict[str, Any]) -> str:
    text = (
        f"{family['head'].get('title') or ''} {family['head'].get('claim_summary') or ''}"
    ).lower()
    for domain in DOMAIN_DEFINITIONS:
        if any(keyword_matches(k, text) for k in (domain.get("keywords") or [])):
            return str(domain.get("domain_id"))
    return "unclassified"


def trials_by_work_item(trials: dict[str, dict[str, Any]]) -> dict[str, dict[str, Any]]:
    index: dict[str, dict[str, Any]] = {}
    for trial in trials.values():
        wi = str(trial.get("source_work_item_id") or "")
        if wi:
            index[wi] = trial
    return index


def family_evidence(family: dict[str, Any], trial_index: dict[str, dict[str, Any]]) -> dict[str, Any]:
    trial = trial_index.get(str(family["head"].get("work_item_id") or ""))
    if trial is None:
        return {"state": "no_trial_record", "trial_id": None, "adapter": None}
    adapter = str(trial.get("adapter") or "")
    real_results = [
        r
        for r in (trial.get("results") or [])
        if isinstance(r, dict) and str(r.get("classification") or "") not in STUBS
    ]
    if real_results:
        return {
            "state": "evidenced",
            "trial_id": trial.get("trial_id"),
            "adapter": adapter,
            "latest_classification": real_results[-1].get("classification"),
        }
    if adapter in OFFLINE_ADAPTERS:
        return {"state": "evidence_runnable_now", "trial_id": trial.get("trial_id"), "adapter": adapter}
    return {"state": "needs_manual_review_or_new_adapter", "trial_id": trial.get("trial_id"), "adapter": adapter}


def build_shortlist(
    plan: dict[str, Any],
    trials: dict[str, dict[str, Any]],
    limit: int = 5,
    families_per_domain: int = 3,
) -> dict[str, Any]:
    trial_index = trials_by_work_item(trials)
    groups: dict[str, dict[str, Any]] = {}
    for family in plan["families"]:
        domain = family_domain(family)
        group = groups.setdefault(
            domain, {"domain_id": domain, "family_count": 0, "ask_weight": 0, "families": []}
        )
        group["family_count"] += 1
        group["ask_weight"] += family["member_count"]
        group["families"].append(family)
    ranked = sorted(groups.values(), key=lambda g: -g["ask_weight"])[:limit]
    out_groups = []
    for group in ranked:
        top = sorted(group["families"], key=lambda f: -f["member_count"])[:families_per_domain]
        out_groups.append(
            {
                "domain_id": group["domain_id"],
                "family_count": group["family_count"],
                "ask_weight": group["ask_weight"],
                "top_families": [
                    {
                        "family_id": f["family_id"],
                        "being": f["being"],
                        "member_count": f["member_count"],
                        "head_work_item_id": f["head"]["work_item_id"],
                        "title": str(f["head"].get("title") or "")[:160],
                        "evidence": family_evidence(f, trial_index),
                    }
                    for f in top
                ],
            }
        )
    return {
        "schema": "authority_wait_grant_shortlist_v1",
        "group_count": len(groups),
        "groups": out_groups,
        "authority_boundary": (
            "shortlist is a reviewable menu only; every grant remains an explicit "
            "Mike/operator act (approve-live-trial records it; execution stays on "
            "the manual deploy path)"
        ),
    }


def render_shortlist(
    plan: dict[str, Any], trials: dict[str, dict[str, Any]], limit: int = 5
) -> str:
    shortlist = build_shortlist(plan, trials, limit=limit)
    lines = [
        "# Tier-5 grant shortlist (surface-grouped)",
        "",
        f"{plan['target_count']} open operator waits -> {plan['family_count']} "
        f"ask-families -> top {len(shortlist['groups'])} surfaces by ask-weight.",
        "A grant is scoped to a surface or a single family head; nothing here",
        "auto-executes — record a grant with:",
        "  python3 scripts/sandbox_trial_queue.py approve-live-trial \\",
        "    --trial-id <trial> --steward mike --note '<scope>' --write",
        "Gather evidence first where runnable:",
        "  python3 scripts/sandbox_trial_queue.py run-evidence --trial-id <trial> --write",
        "",
    ]
    for group in shortlist["groups"]:
        lines.append(
            f"## {group['domain_id']}  (asked {group['ask_weight']}x across "
            f"{group['family_count']} families)"
        )
        for fam in group["top_families"]:
            ev = fam["evidence"]
            ev_note = {
                "evidenced": f"EVIDENCED ({ev.get('latest_classification')})",
                "evidence_runnable_now": f"evidence runnable now via {ev.get('adapter')}",
                "needs_manual_review_or_new_adapter": "needs manual review or a new adapter",
                "no_trial_record": "no trial record yet",
            }[ev["state"]]
            lines.append(
                f"- [{fam['member_count']:>2}x, {fam['being']}] {fam['title']}"
            )
            lines.append(
                f"    head {fam['head_work_item_id']} · trial {ev.get('trial_id')} · {ev_note}"
            )
        lines.append("")
    return "\n".join(lines)


def self_test() -> int:
    failures: list[str] = []

    def check(name: str, ok: bool) -> None:
        if not ok:
            failures.append(name)

    def fam(fid, wi, title, count, being="minime"):
        return {
            "family_id": fid,
            "being": being,
            "claim_id": "c001",
            "member_count": count,
            "members": [],
            "head": {"work_item_id": wi, "title": title, "claim_summary": title},
        }

    plan = {
        "target_count": 10,
        "family_count": 3,
        "families": [
            fam("fam_1", "wi_1", "soften the minime regulator toward a gentler pi control hold", 5),
            fam("fam_2", "wi_2", "raise the porosity receptivity buffer for semantic admission", 3),
            fam("fam_3", "wi_3", "an entirely unrelated bespoke request about nothing known", 1),
        ],
    }
    trials = {
        "trial_1": {"trial_id": "trial_1", "source_work_item_id": "wi_1",
                    "adapter": "manual_sandbox_review_v1", "results": []},
        "trial_2": {"trial_id": "trial_2", "source_work_item_id": "wi_2",
                    "adapter": "shadow_loss_lattice_v1",
                    "results": [{"classification": "lattice_transition_like"}]},
    }
    shortlist = build_shortlist(plan, trials)
    domains = {g["domain_id"]: g for g in shortlist["groups"]}
    check("regulator family grouped", any("regulator" in d for d in domains))
    check("unclassified captured", "unclassified" in domains)
    reg = next(g for d, g in domains.items() if "regulator" in d)
    check("manual adapter flagged honestly",
          reg["top_families"][0]["evidence"]["state"] == "needs_manual_review_or_new_adapter")
    porosity = next((g for d, g in domains.items() if "porosity" in d), None)
    check("porosity grouped", porosity is not None)
    if porosity:
        check("real result counts as evidenced",
              porosity["top_families"][0]["evidence"]["state"] == "evidenced")
    stub_trials = {
        "trial_2": {"trial_id": "trial_2", "source_work_item_id": "wi_2",
                    "adapter": "shadow_loss_lattice_v1",
                    "results": [{"classification": "insufficient_output"}]},
    }
    s2 = build_shortlist(plan, stub_trials)
    p2 = next((g for g in s2["groups"] if "porosity" in g["domain_id"]), None)
    check("stub never counts as evidence",
          p2 and p2["top_families"][0]["evidence"]["state"] == "evidence_runnable_now")
    rendered = render_shortlist(plan, trials)
    check("render names grant command", "approve-live-trial" in rendered)
    check("render shows ask weight", "asked 5x" in rendered)

    if failures:
        print("FAIL:", ", ".join(failures))
        return 1
    print("OK (8 checks)")
    return 0


if __name__ == "__main__":
    sys.exit(self_test() if "--self-test" in sys.argv else 0)
