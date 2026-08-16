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

    if failures:
        print("FAIL:", ", ".join(failures))
        return 1
    print("OK (16 checks)")
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
    args = parser.parse_args()
    if args.self_test:
        return self_test()
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
