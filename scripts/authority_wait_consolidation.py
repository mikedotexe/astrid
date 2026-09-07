#!/usr/bin/env python3
"""Consolidate the Tier-5 authority-wait backlog into ask-families.

The approval-required pile (1,571 live candidates on 2026-08-16) is raw
duplication, not demand: work items are minted per (introspection, claim), so
every fresh-pass re-read of the same ask creates a new Tier-5 item. This tool
clusters `needs_operator_approval` work items into FAMILIES — same being, same
claim shape, similar ask text — keeps the newest best-grounded member as the
family HEAD, and (with --write) supersedes the members with a note pointing at
the head, writes a family manifest preserving every member, links the manifest
onto the head, and mirrors upstream terminal statuses onto the sandbox trial
queue so the readiness map counts distinct asks.

Un-muffle guarantee: nothing is deleted. Every member stays readable in the
addressing store and in the family manifest; the head carries the family's
full provenance; family size becomes a RANKING signal (how persistently the
being asked), not lost information. Consolidation is bookkeeping, never a
verdict on any ask.

Read-only by default. `--write` acts. `--report` / `--shortlist` emit the
dossier views. `--self-test` runs the built-in tests.
"""

from __future__ import annotations

import argparse
import json
import sys
import time
from pathlib import Path
from typing import Any

SCRIPTS_DIR = Path(__file__).resolve().parent
if str(SCRIPTS_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPTS_DIR))

from introspection_family_scan import STOPWORDS, TOKEN_RE, jaccard  # noqa: E402

ADDRESSING_STATE_DIR = Path(
    "/Users/v/other/astrid/capsules/spectral-bridge/workspace/diagnostics/introspection_addressing_v1"
)
SANDBOX_STATE_DIR = Path(
    "/Users/v/other/astrid/capsules/spectral-bridge/workspace/diagnostics/sandbox_trial_queue_v1"
)
CONSOLIDATION_DIR = Path(
    "/Users/v/other/astrid/capsules/spectral-bridge/workspace/diagnostics/authority_wait_consolidation_v1"
)
# 0.35 chosen empirically (2026-08-16 sweep: 0.5->129, 0.4->207, 0.35->261
# supersedes on the live corpus). Conservative by design: textual families
# only supersede true near-duplicates; the GRANT-level grouping happens in
# the shortlist by readiness surface, where "fallback prompt weighting" and
# "provider route, sampler behavior" correctly land in one grant scope
# without superseding either ask.
DEFAULT_THRESHOLD = 0.35
TARGET_STATUS = "needs_operator_approval"
# Trial statuses the readiness map treats as terminal; upstream statuses in
# this set mirror directly onto the trial record.
TRIAL_TERMINAL_SYNC = frozenset(
    {"superseded", "verified_existing", "closed_no_action", "closed_felt_confirmed"}
)

AUTHORITY_BOUNDARY = (
    "consolidation is bookkeeping only: it supersedes duplicate records of one "
    "ask into a single head, preserves every member in a manifest, and grants, "
    "dispatches, runs, and approves nothing"
)


# The Tier-5 corpus is dominated by authority-boundary framing ("Changing X,
# Y, or Z requires separate operator approval / alters live behavior /
# remains Tier 5 pending Mike"). Those framing tokens swamp the subject
# tokens, splitting one ask into many families. Similarity must run on WHAT
# is asked about, not how the boundary sentence is phrased.
BOILERPLATE = frozenset(
    """change changes changing changed require requires required separate
    approval approvals approve approved remains remain remained tier pending
    mike operator operators live behavior behaviour alters alter altered
    would risk risks keep keeps behind explicit scoped grant grants granted
    request requests needs need needed must adjustment adjustments adjusting
    adjust""".split()
)


def tokenize(text: str) -> set[str]:
    return {
        tok
        for tok in TOKEN_RE.findall(str(text or "").lower())
        if len(tok) > 2
        and tok not in STOPWORDS
        and tok not in BOILERPLATE
        and not tok.isdigit()
    }


def item_text(item: dict[str, Any]) -> str:
    return f"{item.get('title') or ''} {item.get('claim_summary') or ''}"


def cluster_items(
    items: list[dict[str, Any]], threshold: float = DEFAULT_THRESHOLD
) -> list[dict[str, Any]]:
    """Deterministic family clustering. Bucket by (being, claim_id); within a
    bucket, newest-first greedy: an item joins the first family whose head it
    matches at or above the Jaccard threshold, else founds a family. Newest
    ordering makes the most recent phrasing the head candidate pool."""
    # Bucket by being only. claim_id is an ORDINAL (c001, c002...) assigned by
    # position within each introspection, not a semantic class — bucketing on
    # it walls off cross-ordinal duplicates of the same ask (found 2026-08-16:
    # claim_id bucketing left 1,470 families where being-only + similarity
    # finds the true duplicate structure).
    buckets: dict[tuple[str], list[dict[str, Any]]] = {}
    for item in items:
        key = (str(item.get("being") or ""),)
        buckets.setdefault(key, []).append(item)
    families: list[dict[str, Any]] = []
    for key in sorted(buckets):
        bucket = sorted(
            buckets[key],
            key=lambda i: (-(float(i.get("created_at") or 0)), str(i.get("work_item_id"))),
        )
        bucket_families: list[dict[str, Any]] = []
        for item in bucket:
            tokens = tokenize(item_text(item))
            placed = False
            for family in bucket_families:
                if jaccard(family["_head_tokens"], tokens) >= threshold:
                    family["members"].append(item)
                    placed = True
                    break
            if not placed:
                bucket_families.append(
                    {
                        "being": key[0],
                        "claim_id": str(item.get("claim_id") or ""),
                        "_head_tokens": tokens,
                        "members": [item],
                    }
                )
        families.extend(bucket_families)
    for family in families:
        family.pop("_head_tokens")
        # Head = newest member with the most evidence links (newest wins ties
        # because members are already newest-first).
        family["head"] = max(
            family["members"],
            key=lambda i: (
                len(i.get("evidence_links") or []),
                float(i.get("created_at") or 0),
            ),
        )
        family["family_id"] = "fam_" + str(family["head"].get("work_item_id", ""))[3:]
        family["member_count"] = len(family["members"])
    families.sort(key=lambda f: -f["member_count"])
    return families


def load_work_items(state_dir: Path = ADDRESSING_STATE_DIR) -> dict[str, dict[str, Any]]:
    status = json.loads((state_dir / "status.json").read_text(encoding="utf-8"))
    items = status.get("work_items") or {}
    return items if isinstance(items, dict) else {i["work_item_id"]: i for i in items}


def load_trials(state_dir: Path = SANDBOX_STATE_DIR) -> dict[str, dict[str, Any]]:
    status = json.loads((state_dir / "status.json").read_text(encoding="utf-8"))
    trials = status.get("trials") or {}
    return trials if isinstance(trials, dict) else {t["trial_id"]: t for t in trials}


def build_plan(
    work_items: dict[str, dict[str, Any]],
    trials: dict[str, dict[str, Any]],
    threshold: float = DEFAULT_THRESHOLD,
) -> dict[str, Any]:
    """Pure planning: families + supersede list + trial sync list."""
    targets = [i for i in work_items.values() if i.get("status") == TARGET_STATUS]
    families = cluster_items(targets, threshold)
    supersede: list[dict[str, Any]] = []
    for family in families:
        head_id = family["head"]["work_item_id"]
        for member in family["members"]:
            if member["work_item_id"] != head_id:
                supersede.append(
                    {
                        "work_item_id": member["work_item_id"],
                        "head": head_id,
                        "family_id": family["family_id"],
                    }
                )
    superseded_ids = {s["work_item_id"] for s in supersede}
    trial_sync: list[dict[str, Any]] = []
    for trial in trials.values():
        wi = str(trial.get("source_work_item_id") or "")
        if not wi:
            continue
        trial_status = str(trial.get("status") or "")
        if trial_status in TRIAL_TERMINAL_SYNC or trial_status == "closed":
            continue
        upstream = work_items.get(wi)
        if wi in superseded_ids:
            trial_sync.append(
                {"trial_id": trial["trial_id"], "status": "superseded", "reason": "family_member"}
            )
        elif upstream is not None and str(upstream.get("status")) in TRIAL_TERMINAL_SYNC:
            trial_sync.append(
                {
                    "trial_id": trial["trial_id"],
                    "status": str(upstream.get("status")),
                    "reason": "upstream_terminal_sync",
                }
            )
    return {
        "schema": "authority_wait_consolidation_plan_v1",
        "threshold": threshold,
        "target_status": TARGET_STATUS,
        "target_count": len(targets),
        "family_count": len(families),
        "head_count": len(families),
        "supersede_count": len(supersede),
        "trial_sync_count": len(trial_sync),
        "families": families,
        "supersede": supersede,
        "trial_sync": trial_sync,
        "authority_boundary": AUTHORITY_BOUNDARY,
    }


AGE_SWEEP_ADAPTER = "manual_sandbox_review_v1"
AGE_SWEEP_DEFAULT_DAYS = 28.0


def build_age_sweep_plan(
    work_items: dict[str, dict[str, Any]],
    trials: dict[str, dict[str, Any]],
    *,
    now: float | None = None,
    threshold_days: float = AGE_SWEEP_DEFAULT_DAYS,
    threshold: float = DEFAULT_THRESHOLD,
) -> dict[str, Any]:
    """Pure planning for the 4-week aging sweep over manual-review placeholders.

    Targets ONLY trials with the dead-end manual adapter (evidence-run trials
    are excluded BY CONSTRUCTION via the adapter filter). Dispositions:
      - untouched_recent: younger than the threshold.
      - untouched_reasked: aged, but its ask-family has a member created inside
        the threshold window — the being is still asking; normal flow keeps it.
      - superseded (newer_coverage_age_sweep): aged, family head is a
        different newer non-terminal item.
      - closed (aged_out_manual_placeholder): aged, no re-ask; preserved in an
        age-sweep manifest; a re-ask reopens (a fresh introspection mints a
        fresh work item — recency of a matching item IS the reopen).
      - closed (orphaned_placeholder): trial has no source work item.
    """
    now_s = time.time() if now is None else now
    threshold_s = threshold_days * 86400.0
    cutoff = now_s - threshold_s

    # Family recency over ALL of each being's work items (any status): a fresh
    # re-read of the same ask mints a new item, so family recency IS coverage.
    families = cluster_items(list(work_items.values()), threshold)
    family_by_member: dict[str, dict[str, Any]] = {}
    for family in families:
        for member in family["members"]:
            family_by_member[str(member.get("work_item_id"))] = family

    supersede: list[dict[str, Any]] = []
    close: list[dict[str, Any]] = []
    orphaned: list[dict[str, Any]] = []
    untouched_recent = 0
    untouched_reasked = 0
    untouched_other_status = 0
    seen_work_items: set[str] = set()

    for trial in trials.values():
        if str(trial.get("adapter") or "") != AGE_SWEEP_ADAPTER:
            continue
        if str(trial.get("trial_mode") or "") != "approval_required_live_trial":
            continue
        if str(trial.get("status") or "") in TRIAL_TERMINAL_SYNC or trial.get("status") == "closed":
            continue
        trial_id = str(trial.get("trial_id"))
        wi_id = str(trial.get("source_work_item_id") or "")
        item = work_items.get(wi_id)
        if item is None:
            orphaned.append({"trial_id": trial_id, "reason": "orphaned_placeholder"})
            continue
        if str(item.get("status")) != TARGET_STATUS:
            # Already granted/terminal/other lane — regular consolidation sync
            # owns it; the age sweep never double-writes a status.
            untouched_other_status += 1
            continue
        created = float(item.get("created_at") or 0)
        if created > cutoff:
            untouched_recent += 1
            continue
        if wi_id in seen_work_items:
            continue
        seen_work_items.add(wi_id)
        family = family_by_member.get(wi_id)
        newest_in_family = (
            max(float(m.get("created_at") or 0) for m in family["members"]) if family else created
        )
        if newest_in_family > cutoff:
            untouched_reasked += 1
            continue
        head = family["head"] if family else item
        head_id = str(head.get("work_item_id"))
        head_terminal = str(head.get("status")) in TRIAL_TERMINAL_SYNC
        if head_id != wi_id and not head_terminal:
            supersede.append(
                {
                    "work_item_id": wi_id,
                    "trial_id": trial_id,
                    "head": head_id,
                    "family_id": family["family_id"] if family else "",
                    "reason": "newer_coverage_age_sweep",
                }
            )
        else:
            close.append(
                {
                    "work_item_id": wi_id,
                    "trial_id": trial_id,
                    "item": item,
                    "reason": "aged_out_manual_placeholder",
                }
            )

    return {
        "schema": "authority_wait_age_sweep_plan_v1",
        "threshold_days": threshold_days,
        "cutoff": cutoff,
        "planned_at": now_s,
        "supersede": supersede,
        "close": close,
        "orphaned": orphaned,
        "untouched_recent": untouched_recent,
        "untouched_reasked": untouched_reasked,
        "untouched_other_status": untouched_other_status,
        "counts": {
            "superseded_newer_coverage": len(supersede),
            "closed_aged_out": len(close),
            "closed_orphaned": len(orphaned),
            "untouched_recent": untouched_recent,
            "untouched_reasked": untouched_reasked,
            "untouched_other_status": untouched_other_status,
        },
        "authority_boundary": AUTHORITY_BOUNDARY,
    }


def age_sweep_manifest(plan: dict[str, Any]) -> dict[str, Any]:
    """Every closed item preserved in full — the un-muffle guarantee."""
    return {
        "schema": "authority_wait_age_sweep_manifest_v1",
        "swept_at": plan["planned_at"],
        "threshold_days": plan["threshold_days"],
        "reopen_rule": (
            "asking again — in any introspection or letter — mints a fresh work "
            "item and reopens the ask; nothing here was deleted"
        ),
        "authority_boundary": AUTHORITY_BOUNDARY,
        "members": [
            {
                "work_item_id": entry["work_item_id"],
                "trial_id": entry["trial_id"],
                "source_introspection_id": entry["item"].get("source_introspection_id"),
                "title": entry["item"].get("title"),
                "claim_summary": entry["item"].get("claim_summary"),
                "created_at": entry["item"].get("created_at"),
                "agency_tier": entry["item"].get("agency_tier"),
            }
            for entry in plan["close"]
        ],
    }


def execute_age_sweep_plan(
    plan: dict[str, Any],
    *,
    addressing_state_dir: Path = ADDRESSING_STATE_DIR,
    sandbox_state_dir: Path = SANDBOX_STATE_DIR,
    consolidation_dir: Path = CONSOLIDATION_DIR,
    log=print,
) -> dict[str, Any]:
    import introspection_addressing_audit as addressing
    import sandbox_trial_queue as sandbox

    now = time.time()
    sweeps_dir = consolidation_dir / "age_sweeps"
    sweeps_dir.mkdir(parents=True, exist_ok=True)
    manifest = age_sweep_manifest(plan)
    manifest_path = sweeps_dir / f"age_sweep_{int(now)}.json"
    manifest_path.write_text(json.dumps(manifest, indent=1) + "\n", encoding="utf-8")

    addressing_events: list[dict[str, Any]] = []
    for entry in plan["supersede"]:
        addressing_events.append(
            addressing.work_status_event(
                entry["work_item_id"],
                "superseded",
                f"age sweep: newer coverage on {entry['head']} (family {entry['family_id']}); "
                "the ask survives on the head",
            )
        )
    for entry in plan["close"]:
        addressing_events.append(
            addressing.work_status_event(
                entry["work_item_id"],
                "closed_no_action",
                f"aged out: manual-review placeholder, no re-ask in "
                f"{plan['threshold_days']:.0f} days; preserved in {manifest_path}; "
                "a re-ask reopens",
            )
        )
    log(f"appending {len(addressing_events)} addressing events ...")
    addressing.append_events(addressing_state_dir, addressing_events)
    status = addressing.replay_events(addressing_state_dir)
    addressing.write_materialized_status(addressing_state_dir, status)

    trial_events: list[dict[str, Any]] = []
    for entry in plan["supersede"]:
        trial_events.append(
            {
                "event_type": "trial_status_set",
                "ts": now,
                "trial_id": entry["trial_id"],
                "status": "superseded",
                "note": f"authority_wait_age_sweep: {entry['reason']}",
            }
        )
    for entry in plan["close"]:
        trial_events.append(
            {
                "event_type": "trial_status_set",
                "ts": now,
                "trial_id": entry["trial_id"],
                "status": "closed_no_action",
                "note": f"authority_wait_age_sweep: {entry['reason']} (manifest {manifest_path.name})",
            }
        )
    for entry in plan["orphaned"]:
        trial_events.append(
            {
                "event_type": "trial_status_set",
                "ts": now,
                "trial_id": entry["trial_id"],
                "status": "closed_no_action",
                "note": f"authority_wait_age_sweep: {entry['reason']}",
            }
        )
    log(f"appending {len(trial_events)} trial-queue events ...")
    sandbox.append_events(sandbox_state_dir, trial_events)
    sandbox_status = sandbox.replay_status(sandbox_state_dir)
    sandbox.materialize(sandbox_state_dir, sandbox_status)

    receipt = {
        "schema": "authority_wait_age_sweep_receipt_v1",
        "executed_at": now,
        "manifest": str(manifest_path),
        "counts": plan["counts"],
        "addressing_events": len(addressing_events),
        "trial_events": len(trial_events),
        "authority_boundary": AUTHORITY_BOUNDARY,
    }
    (consolidation_dir / "latest_age_sweep_receipt.json").write_text(
        json.dumps(receipt, indent=1) + "\n", encoding="utf-8"
    )
    return receipt


def render_age_sweep_report(plan: dict[str, Any]) -> str:
    counts = plan["counts"]
    lines = [
        f"age sweep plan (threshold {plan['threshold_days']:.0f}d): "
        f"{counts['closed_aged_out']} close (aged out), "
        f"{counts['superseded_newer_coverage']} supersede (newer coverage), "
        f"{counts['closed_orphaned']} close (orphaned trial), "
        f"{counts['untouched_reasked']} kept (actively re-asked), "
        f"{counts['untouched_recent']} kept (younger than threshold), "
        f"{counts['untouched_other_status']} kept (other status/lane)",
    ]
    for entry in plan["close"][:10]:
        lines.append(
            f"  close: {entry['work_item_id']}  {str(entry['item'].get('title') or '')[:90]}"
        )
    if len(plan["close"]) > 10:
        lines.append(f"  ... and {len(plan['close']) - 10} more closes")
    return "\n".join(lines)


def family_manifest(family: dict[str, Any], now_s: float) -> dict[str, Any]:
    return {
        "schema": "authority_wait_family_manifest_v1",
        "family_id": family["family_id"],
        "being": family["being"],
        "claim_id": family["claim_id"],
        "head_work_item_id": family["head"]["work_item_id"],
        "member_count": family["member_count"],
        "consolidated_at": now_s,
        "authority_boundary": AUTHORITY_BOUNDARY,
        "members": [
            {
                "work_item_id": m.get("work_item_id"),
                "source_introspection_id": m.get("source_introspection_id"),
                "title": m.get("title"),
                "claim_summary": m.get("claim_summary"),
                "created_at": m.get("created_at"),
                "agency_tier": m.get("agency_tier"),
            }
            for m in family["members"]
        ],
    }


def execute_plan(
    plan: dict[str, Any],
    *,
    addressing_state_dir: Path = ADDRESSING_STATE_DIR,
    sandbox_state_dir: Path = SANDBOX_STATE_DIR,
    consolidation_dir: Path = CONSOLIDATION_DIR,
    log=print,
) -> dict[str, Any]:
    import introspection_addressing_audit as addressing
    import sandbox_trial_queue as sandbox

    now = time.time()
    families_dir = consolidation_dir / "families"
    families_dir.mkdir(parents=True, exist_ok=True)

    addressing_events: list[dict[str, Any]] = []
    manifests_written = 0
    for family in plan["families"]:
        manifest = family_manifest(family, now)
        manifest_path = families_dir / f"{family['family_id']}.json"
        manifest_path.write_text(json.dumps(manifest, indent=1) + "\n", encoding="utf-8")
        manifests_written += 1
        if family["member_count"] > 1:
            addressing_events.append(
                addressing.work_evidence_event(
                    family["head"]["work_item_id"],
                    "steward_note",
                    str(manifest_path),
                    f"ask made {family['member_count']} times (family {family['family_id']}); "
                    "every member preserved in the manifest; family size is a persistence "
                    "signal, not noise",
                )
            )
    for entry in plan["supersede"]:
        addressing_events.append(
            addressing.work_status_event(
                entry["work_item_id"],
                "superseded",
                f"consolidated into {entry['head']} (family {entry['family_id']}); "
                "the ask survives on the head with full provenance",
            )
        )
    log(f"appending {len(addressing_events)} addressing events ...")
    addressing.append_events(addressing_state_dir, addressing_events)
    status = addressing.replay_events(addressing_state_dir)
    addressing.write_materialized_status(addressing_state_dir, status)

    trial_events: list[dict[str, Any]] = []
    for entry in plan["trial_sync"]:
        trial_events.append(
            {
                "event_type": "trial_status_set",
                "ts": now,
                "trial_id": entry["trial_id"],
                "status": entry["status"],
                "note": f"authority_wait_consolidation: {entry['reason']}",
            }
        )
    log(f"appending {len(trial_events)} trial-queue events ...")
    sandbox.append_events(sandbox_state_dir, trial_events)
    sandbox_status = sandbox.replay_status(sandbox_state_dir)
    sandbox.materialize(sandbox_state_dir, sandbox_status)

    receipt = {
        "schema": "authority_wait_consolidation_receipt_v1",
        "executed_at": now,
        "manifests_written": manifests_written,
        "addressing_events": len(addressing_events),
        "trial_events": len(trial_events),
        "family_count": plan["family_count"],
        "supersede_count": plan["supersede_count"],
        "trial_sync_count": plan["trial_sync_count"],
        "authority_boundary": AUTHORITY_BOUNDARY,
    }
    (consolidation_dir / "latest_receipt.json").write_text(
        json.dumps(receipt, indent=1) + "\n", encoding="utf-8"
    )
    return receipt


def render_report(plan: dict[str, Any], limit: int = 15) -> str:
    lines = [
        f"authority-wait consolidation: {plan['target_count']} {TARGET_STATUS} items "
        f"-> {plan['family_count']} distinct ask-families "
        f"({plan['supersede_count']} members to supersede, "
        f"{plan['trial_sync_count']} trial records to sync)",
        "",
    ]
    for family in plan["families"][:limit]:
        head = family["head"]
        lines.append(
            f"[{family['member_count']:>3}x] {family['being']}/{family['claim_id']} "
            f"{family['family_id']}  {str(head.get('title') or '')[:96]}"
        )
    if plan["family_count"] > limit:
        lines.append(f"... and {plan['family_count'] - limit} more families")
    return "\n".join(lines)


def self_test() -> int:
    failures: list[str] = []

    def check(name: str, ok: bool) -> None:
        if not ok:
            failures.append(name)

    def wi(id_, being, claim, title, created, status=TARGET_STATUS, evidence=0):
        return {
            "work_item_id": id_,
            "being": being,
            "claim_id": claim,
            "title": title,
            "claim_summary": title,
            "created_at": created,
            "status": status,
            "evidence_links": [{}] * evidence,
        }

    items = {
        "wi_a1": wi("wi_a1", "minime", "c004", "inject a high pressure spike to test local control response", 100),
        "wi_a2": wi("wi_a2", "minime", "c004", "a high-pressure spike should be injected to test the local control response", 200),
        "wi_a3": wi("wi_a3", "minime", "c004", "inject high pressure spike testing local control response", 300, evidence=2),
        "wi_b1": wi("wi_b1", "minime", "c004", "raise the porosity receptivity buffer threshold for semantic admission", 150),
        "wi_c1": wi("wi_c1", "astrid", "c002", "widen the codec gain on reserved dims", 120),
        "wi_d1": wi("wi_d1", "astrid", "c002", "widen codec gain on the reserved dims", 90, status="verified_existing"),
    }
    trials = {
        "trial_1": {"trial_id": "trial_1", "source_work_item_id": "wi_a1", "status": "ready_for_sandbox"},
        "trial_2": {"trial_id": "trial_2", "source_work_item_id": "wi_a3", "status": "ready_for_sandbox"},
        "trial_3": {"trial_id": "trial_3", "source_work_item_id": "wi_d1", "status": "ready_for_sandbox"},
        "trial_4": {"trial_id": "trial_4", "source_work_item_id": "wi_d1", "status": "superseded"},
    }
    plan = build_plan(items, trials)
    check("five targets", plan["target_count"] == 5)
    check("three families", plan["family_count"] == 3)
    fam = next(f for f in plan["families"] if f["member_count"] == 3)
    check("head has most evidence", fam["head"]["work_item_id"] == "wi_a3")
    check("two superseded in big family", sum(1 for s in plan["supersede"] if s["family_id"] == fam["family_id"]) == 2)
    check("different claim splits", any(f["head"]["work_item_id"] == "wi_b1" for f in plan["families"]))
    check("terminal item excluded from targets", all(
        m["work_item_id"] != "wi_d1" for f in plan["families"] for m in f["members"]))
    sync = {s["trial_id"]: s for s in plan["trial_sync"]}
    check("member trial superseded", sync.get("trial_1", {}).get("status") == "superseded")
    check("head trial untouched", "trial_2" not in sync)
    check("stale upstream synced", sync.get("trial_3", {}).get("status") == "verified_existing")
    check("already-terminal trial skipped", "trial_4" not in sync)

    import tempfile
    from unittest import mock

    with tempfile.TemporaryDirectory() as tmp:
        cdir = Path(tmp)
        fake_addr = mock.MagicMock()
        fake_addr.work_evidence_event = lambda w, k, t, n: {"event_type": "work_evidence_linked", "work_item_id": w, "kind": k}
        fake_addr.work_status_event = lambda w, s, n, blocked_by=None: {"event_type": "work_status_set", "work_item_id": w, "status": s}
        fake_sandbox = mock.MagicMock()
        with mock.patch.dict(sys.modules, {"introspection_addressing_audit": fake_addr, "sandbox_trial_queue": fake_sandbox}):
            receipt = execute_plan(
                plan,
                addressing_state_dir=cdir,
                sandbox_state_dir=cdir,
                consolidation_dir=cdir,
                log=lambda *_: None,
            )
        check("manifests written", receipt["manifests_written"] == plan["family_count"])
        check("receipt persisted", (cdir / "latest_receipt.json").is_file())
        check("addressing events appended once", fake_addr.append_events.call_count == 1)
        appended = fake_addr.append_events.call_args[0][1]
        head_notes = [e for e in appended if e["event_type"] == "work_evidence_linked"]
        check("only multi-member families get head note", len(head_notes) == 1)
        supersedes = [e for e in appended if e["event_type"] == "work_status_set"]
        check("supersede count matches", len(supersedes) == plan["supersede_count"])
        check("sandbox materialized", fake_sandbox.materialize.call_count == 1)
        manifest = json.loads(next((cdir / "families").glob("*.json")).read_text())
        check("manifest preserves members", manifest["member_count"] >= 1 and manifest["members"])

    # ---- age sweep dispositions ----
    NOW = 10_000_000.0
    DAY = 86400.0
    old = NOW - 40 * DAY
    fresh = NOW - 3 * DAY

    def sweep_wi(id_, title, created, status=TARGET_STATUS):
        return wi(id_, "minime", "c001", title, created, status=status)

    def sweep_trial(id_, wi_id, adapter=AGE_SWEEP_ADAPTER, status="approval_required_live_trial"):
        return {
            "trial_id": id_,
            "source_work_item_id": wi_id,
            "adapter": adapter,
            "trial_mode": "approval_required_live_trial",
            "status": status,
        }

    sweep_items = {
        # aged, no re-ask -> close
        "wi_s1": sweep_wi("wi_s1", "modulate the shadow trajectory persistence window internals", old),
        # aged, but the same ask was re-made recently -> untouched (re-asked)
        "wi_s2": sweep_wi("wi_s2", "raise the porosity admission buffer for semantic intake lanes", old),
        "wi_s2b": sweep_wi("wi_s2b", "raise porosity admission buffer for the semantic intake lanes", fresh),
        # aged, newer non-terminal head exists (also aged but newer) -> supersede
        "wi_s3": sweep_wi("wi_s3", "recalibrate the fallback texture weighting curve exponents", old - 5 * DAY),
        "wi_s3b": sweep_wi("wi_s3b", "recalibrate fallback texture weighting curve exponent values", old, ),
        # young -> untouched
        "wi_s4": sweep_wi("wi_s4", "introduce an entirely new resonance chamber calibration ritual", fresh),
    }
    sweep_trials = {
        "t_s1": sweep_trial("t_s1", "wi_s1"),
        "t_s2": sweep_trial("t_s2", "wi_s2"),
        "t_s3": sweep_trial("t_s3", "wi_s3"),
        "t_s4": sweep_trial("t_s4", "wi_s4"),
        # orphaned trial -> close
        "t_s5": sweep_trial("t_s5", "wi_gone"),
        # evidence-run adapter -> NEVER swept, by construction
        "t_s6": sweep_trial("t_s6", "wi_s1", adapter="shadow_loss_lattice_v1"),
        # already terminal -> skipped
        "t_s7": {**sweep_trial("t_s7", "wi_s1"), "status": "superseded"},
    }
    sweep_plan = build_age_sweep_plan(
        sweep_items, sweep_trials, now=NOW, threshold_days=28.0
    )
    closes = {e["work_item_id"] for e in sweep_plan["close"]}
    sups = {e["work_item_id"]: e for e in sweep_plan["supersede"]}
    check("aged no-reask closes", "wi_s1" in closes)
    check("reasked untouched", "wi_s2" not in closes and "wi_s2" not in sups
          and sweep_plan["untouched_reasked"] >= 1)
    check("newer coverage supersedes to head", sups.get("wi_s3", {}).get("head") == "wi_s3b")
    check("young untouched", "wi_s4" not in closes and sweep_plan["untouched_recent"] >= 1)
    check("orphaned trial closes", any(e["trial_id"] == "t_s5" for e in sweep_plan["orphaned"]))
    check("real-adapter trial excluded by construction",
          all(e.get("trial_id") != "t_s6" for e in sweep_plan["close"] + sweep_plan["supersede"]))
    manifest = age_sweep_manifest(sweep_plan)
    check("sweep manifest preserves member text", any(
        m["work_item_id"] == "wi_s1" and m["title"] for m in manifest["members"]))
    check("sweep manifest states reopen rule", "reopens" in manifest["reopen_rule"])

    import tempfile as _tempfile
    from unittest import mock as _mock

    with _tempfile.TemporaryDirectory() as tmp2:
        cdir2 = Path(tmp2)
        fake_addr2 = _mock.MagicMock()
        fake_addr2.work_status_event = lambda w, s, n, blocked_by=None: {
            "event_type": "work_status_set", "work_item_id": w, "status": s, "note": n}
        fake_sandbox2 = _mock.MagicMock()
        with _mock.patch.dict(sys.modules, {
            "introspection_addressing_audit": fake_addr2,
            "sandbox_trial_queue": fake_sandbox2,
        }):
            sweep_receipt = execute_age_sweep_plan(
                sweep_plan,
                addressing_state_dir=cdir2,
                sandbox_state_dir=cdir2,
                consolidation_dir=cdir2,
                log=lambda *_: None,
            )
        check("sweep receipt counts", sweep_receipt["counts"]["closed_aged_out"] == 1
              and sweep_receipt["counts"]["superseded_newer_coverage"] == 1)
        check("sweep manifest persisted", Path(sweep_receipt["manifest"]).is_file())
        appended2 = fake_addr2.append_events.call_args[0][1]
        closed_events = [e for e in appended2 if e["status"] == "closed_no_action"]
        check("close note names manifest + reopen", closed_events
              and "reopens" in closed_events[0]["note"]
              and "age_sweep_" in closed_events[0]["note"])

    if failures:
        print("FAIL:", ", ".join(failures))
        return 1
    print("OK (27 checks)")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--threshold", type=float, default=DEFAULT_THRESHOLD)
    parser.add_argument("--write", action="store_true")
    parser.add_argument("--report", action="store_true")
    parser.add_argument("--shortlist", action="store_true", help="emit the grant shortlist (see --shortlist-limit)")
    parser.add_argument("--shortlist-limit", type=int, default=5)
    parser.add_argument("--json", action="store_true")
    parser.add_argument("--self-test", action="store_true")
    parser.add_argument(
        "--age-sweep",
        action="store_true",
        help="plan (or with --write, execute) the aging sweep over manual-review placeholders",
    )
    parser.add_argument("--age-threshold-days", type=float, default=AGE_SWEEP_DEFAULT_DAYS)
    args = parser.parse_args()
    if args.self_test:
        return self_test()
    if args.age_sweep:
        sweep_plan = build_age_sweep_plan(
            load_work_items(),
            load_trials(),
            threshold_days=args.age_threshold_days,
            threshold=args.threshold,
        )
        if args.json:
            slim = {k: v for k, v in sweep_plan.items() if k not in ("supersede", "close")}
            slim["close_preview"] = [
                {"work_item_id": e["work_item_id"], "title": e["item"].get("title")}
                for e in sweep_plan["close"][:20]
            ]
            print(json.dumps(slim, indent=1))
        else:
            print(render_age_sweep_report(sweep_plan))
        if args.write:
            receipt = execute_age_sweep_plan(sweep_plan)
            print(json.dumps(receipt, indent=1))
        return 0
    plan = build_plan(load_work_items(), load_trials(), args.threshold)
    if args.shortlist:
        from authority_wait_shortlist import render_shortlist  # local sibling

        print(render_shortlist(plan, load_trials(), limit=args.shortlist_limit))
        return 0
    if args.json:
        slim = {k: v for k, v in plan.items() if k not in ("families", "supersede", "trial_sync")}
        slim["top_families"] = [
            {
                "family_id": f["family_id"],
                "being": f["being"],
                "claim_id": f["claim_id"],
                "member_count": f["member_count"],
                "head": f["head"]["work_item_id"],
                "title": f["head"].get("title"),
            }
            for f in plan["families"][:20]
        ]
        print(json.dumps(slim, indent=1))
    else:
        print(render_report(plan))
    if args.write:
        receipt = execute_plan(plan)
        print(json.dumps(receipt, indent=1))
    return 0


if __name__ == "__main__":
    sys.exit(main())
