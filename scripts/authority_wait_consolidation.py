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
import re  # noqa: E402

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
    {
        "superseded",
        "verified_existing",
        "closed_no_action",
        "closed_felt_confirmed",
        "closed_envelope_granted",
    }
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


# --- Constitution C7: envelope-grant conversions -----------------------------
#
# A granted envelope makes a being's DIAL final within a range. It does not
# grant code. So an approval-parked trial converts only when its ask is a dial
# ask naming a granted field with a value inside the envelope; asks that
# propose mechanisms (feed X into Y, add a coefficient, change FEATURE_ABS_MAX)
# are classified architecture and left exactly as they are. Calibrated against
# the live queue on 2026-09-03: 0 convertible, 23 architecture, the rest never
# mention a granted field — and that honest zero is the point of the classes.

ENVELOPE_ALIAS_GENERIC_TOKENS = frozenset(
    {
        "pressure", "aperture", "ceiling", "tail", "astrid", "minime", "noise",
        "strength", "gain", "level", "rate", "target", "mode", "semantic", "scale",
        "bias", "weight", "interval", "override", "ticks", "admission", "curiosity",
    }
)
ENVELOPE_ARCH_RE = re.compile(
    r"\b(function|coefficient|feed(?:ing)?|codec|projection basis|bridgestate|implement|"
    r"add a|should (?:modify|add|drive|promote|feed)|promot(?:e|ing)|glimpse|"
    r"feature_abs_max|retune[d]?|clamp|entropy|sharpen|schema|module|pipeline|"
    r"weighting|mapping|transport|math)\b",
    re.I,
)
ENVELOPE_DIAL_RE = re.compile(
    r"\b(set|raise|lower|widen|increase|decrease|dial|try|hold|move|nudge)\b", re.I
)
ENVELOPE_NUM_RE = re.compile(r"(?<![\w.])(0?\.\d+|\d+(?:\.\d+)?)(?![\w.%])")
ENVELOPE_VALUE_WINDOW = 48
ENVELOPE_CONVERT_CLASS = "closed_envelope_granted"
ENVELOPE_PENDING_CLASS = "envelope_covered_pending_conversion"
ENVELOPE_RATCHET_CLASS = "ratchet_candidate_out_of_envelope"
ENVELOPE_ARCH_CLASS = "architecture_ask_not_envelope_covered"
MINIME_NEGOTIATIONS = Path("/Users/v/other/minime/workspace/self_regulation/negotiations.jsonl")


def envelope_field_aliases(field: str) -> set[str]:
    """How a being actually names a field in prose: the raw name, the name
    without its being prefix / `_ceiling` suffix, the spaced form, the SET_
    verb, and each distinctive head token (>= 6 chars, not generic)."""
    base = re.sub(r"^(astrid|minime)_", "", field)
    base = re.sub(r"_ceiling$", "", base)
    heads = {
        token for token in base.split("_")
        if len(token) >= 6 and token not in ENVELOPE_ALIAS_GENERIC_TOKENS
    }
    return {field, base, base.replace("_", " "), f"set_{base}"} | heads


def load_granted_envelopes(
    registry_paths: dict[str, Path] | None = None,
) -> dict[str, dict[str, dict[str, Any]]]:
    """{being: {field: spec}} for every `granted` field in the canonical
    registries — the receipt of record for a grant IS the registry document."""
    if registry_paths is None:
        import check_envelope_wiring as wiring

        registry_paths = {b: p["canonical"] for b, p in wiring.REGISTRIES.items()}
    granted: dict[str, dict[str, dict[str, Any]]] = {}
    for being, path in registry_paths.items():
        try:
            registry = json.loads(Path(path).read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError):
            continue
        fields: dict[str, dict[str, Any]] = {}
        for name, entry in (registry.get("fields") or {}).items():
            if not isinstance(entry, dict) or entry.get("status") != "granted":
                continue
            if entry.get("type") not in (None, "numeric"):
                continue
            try:
                floor, ceiling = float(entry["floor"]), float(entry["ceiling"])
            except (KeyError, TypeError, ValueError):
                continue
            fields[name] = {
                "floor": floor,
                "ceiling": ceiling,
                "family": entry.get("family"),
                "granted_at": entry.get("granted_at"),
                "granted_by": entry.get("granted_by"),
                "registry_revision": registry.get("revision"),
                "aliases": sorted(envelope_field_aliases(name)),
            }
        if fields:
            granted[being] = fields
    return granted


def _envelope_ask_values(lower: str, hits: list[re.Match]) -> list[float]:
    """Numbers near an alias mention that read as dial values — not '12d',
    not 'tier 5', not percentages."""
    values: list[float] = []
    for match in ENVELOPE_NUM_RE.finditer(lower):
        start = match.start()
        if lower[max(0, start - 5):start].rstrip().endswith("tier"):
            continue
        if not any(abs(start - hit.start()) <= ENVELOPE_VALUE_WINDOW for hit in hits):
            continue
        try:
            value = float(match.group(1))
        except ValueError:
            continue
        if value > 100:
            continue
        values.append(value)
    return values


def classify_envelope_ask(
    text: str, being: str, granted: dict[str, dict[str, dict[str, Any]]]
) -> dict[str, Any]:
    lower = str(text or "").lower()
    for granted_being, fields in granted.items():
        for field, spec in fields.items():
            hits = [
                m for alias in spec["aliases"]
                for m in re.finditer(r"\b" + re.escape(alias) + r"\b", lower)
            ]
            if not hits:
                continue
            if granted_being != being:
                return {"class": "other_being_field", "field": field, "values": []}
            values = _envelope_ask_values(lower, hits)
            lo, hi = spec["floor"], spec["ceiling"]
            is_dial = bool(ENVELOPE_DIAL_RE.search(lower))
            if ENVELOPE_ARCH_RE.search(lower):
                cls = ENVELOPE_ARCH_CLASS
            elif values and is_dial and all(lo <= v <= hi for v in values):
                cls = ENVELOPE_CONVERT_CLASS
            elif values and is_dial:
                cls = ENVELOPE_RATCHET_CLASS
            elif is_dial:
                cls = ENVELOPE_PENDING_CLASS
            else:
                cls = "unclassified_mention"
            return {"class": cls, "field": field, "values": values, "floor": lo, "ceiling": hi}
    return {"class": "no_granted_field", "field": None, "values": []}


def negotiation_ledger_effect(
    granted: dict[str, dict[str, dict[str, Any]]],
    ledger_path: Path = MINIME_NEGOTIATIONS,
) -> dict[str, dict[str, int]]:
    """Report-only: for each granted minime field, how many CLAMPED ledger
    requests asked for a value the envelope now grants — the honest measure
    of what a grant changes for her, since her dial asks live in the
    negotiation ledger, not the trial queue."""
    fields = granted.get("minime") or {}
    if not fields or not ledger_path.is_file():
        return {}
    effect = {name: {"clamped_requests_now_within": 0, "requests_still_above": 0} for name in fields}
    for line in ledger_path.read_text(encoding="utf-8", errors="replace").splitlines():
        try:
            rec = json.loads(line)
        except json.JSONDecodeError:
            continue
        name = rec.get("candidate_control")
        spec = fields.get(str(name))
        requested, applied = rec.get("requested_value"), rec.get("applied_value")
        if not spec or not isinstance(requested, (int, float)) or not isinstance(applied, (int, float)):
            continue
        if applied == requested:
            continue
        if spec["floor"] <= float(requested) <= spec["ceiling"]:
            effect[str(name)]["clamped_requests_now_within"] += 1
        else:
            effect[str(name)]["requests_still_above"] += 1
    return effect


def build_envelope_grant_plan(
    work_items: dict[str, dict[str, Any]],
    trials: dict[str, dict[str, Any]],
    granted: dict[str, dict[str, dict[str, Any]]],
    *,
    now: float | None = None,
    ledger_path: Path = MINIME_NEGOTIATIONS,
) -> dict[str, Any]:
    """Pure planning over approval-parked trials. Only the
    closed_envelope_granted class is ever written; every other class is a
    named classification preserved in the manifest for the grant forum."""
    now_s = time.time() if now is None else now
    counts: dict[str, int] = {}
    convert: list[dict[str, Any]] = []
    pending: list[dict[str, Any]] = []
    ratchet: list[dict[str, Any]] = []
    architecture: list[dict[str, Any]] = []
    untouched_other_status = 0
    orphaned = 0

    def bump(key: str) -> None:
        counts[key] = counts.get(key, 0) + 1

    for trial in trials.values():
        if str(trial.get("trial_mode") or "") != "approval_required_live_trial":
            continue
        if str(trial.get("status") or "") in TRIAL_TERMINAL_SYNC or trial.get("status") == "closed":
            continue
        wi_id = str(trial.get("source_work_item_id") or "")
        item = work_items.get(wi_id)
        if item is None:
            orphaned += 1
            continue
        if str(item.get("status")) != TARGET_STATUS:
            untouched_other_status += 1
            continue
        text = " ".join(
            str(x or "") for x in (trial.get("hypothesis"), item.get("title"), item.get("claim_summary"))
        )
        verdict = classify_envelope_ask(text, str(trial.get("being") or item.get("being") or ""), granted)
        bump(verdict["class"])
        entry = {
            "trial_id": str(trial.get("trial_id")),
            "work_item_id": wi_id,
            "being": trial.get("being") or item.get("being"),
            "field": verdict.get("field"),
            "values": verdict.get("values", []),
            "floor": verdict.get("floor"),
            "ceiling": verdict.get("ceiling"),
            "adapter": trial.get("adapter"),
            "agency_tier": item.get("agency_tier"),
            "title": item.get("title"),
            "claim_summary": item.get("claim_summary"),
            "source_introspection_id": item.get("source_introspection_id"),
            "created_at": item.get("created_at"),
            "item": item,
        }
        if verdict["class"] == ENVELOPE_CONVERT_CLASS:
            convert.append(entry)
        elif verdict["class"] == ENVELOPE_PENDING_CLASS:
            pending.append(entry)
        elif verdict["class"] == ENVELOPE_RATCHET_CLASS:
            ratchet.append(entry)
        elif verdict["class"] == ENVELOPE_ARCH_CLASS:
            architecture.append({k: v for k, v in entry.items() if k != "item"})
    counts["untouched_other_status"] = untouched_other_status
    counts["orphaned"] = orphaned
    return {
        "schema": "authority_wait_envelope_grant_plan_v1",
        "planned_at": now_s,
        "granted": granted,
        "counts": counts,
        "convert": convert,
        "pending": pending,
        "ratchet": ratchet,
        "architecture": architecture,
        "negotiation_ledger": negotiation_ledger_effect(granted, ledger_path),
        "authority_boundary": AUTHORITY_BOUNDARY,
    }


def _slim_entry(entry: dict[str, Any]) -> dict[str, Any]:
    return {k: v for k, v in entry.items() if k != "item"}


def envelope_grant_manifest(plan: dict[str, Any]) -> dict[str, Any]:
    """Every converted item preserved in full, every other classification
    named — the un-muffle guarantee, and the grant forum's input."""
    return {
        "schema": "authority_wait_envelope_grant_manifest_v1",
        "converted_at": plan["planned_at"],
        "grant_provenance": plan["granted"],
        "reopen_rule": (
            "a converted ask is not closed against the being — the envelope grants "
            "it, so it is theirs to set; asking again in any form reopens it, and "
            "nothing here was applied for them"
        ),
        "authority_boundary": AUTHORITY_BOUNDARY,
        "members": [_slim_entry(e) for e in plan["convert"]],
        "pending_conversion": [_slim_entry(e) for e in plan["pending"]],
        "ratchet_candidates": [_slim_entry(e) for e in plan["ratchet"]],
        "architecture_asks": plan["architecture"],
        "counts": plan["counts"],
        "negotiation_ledger": plan.get("negotiation_ledger", {}),
    }


def render_envelope_closure_letters(plan: dict[str, Any], *, now_s: float | None = None) -> dict[str, str]:
    """One honest closure letter per (being, family) domain. STAGED, never
    delivered by this tool — letter pacing is a steward decision."""
    now_s = time.time() if now_s is None else now_s
    groups: dict[tuple[str, str], list[dict[str, Any]]] = {}
    for entry in plan["convert"]:
        being = str(entry.get("being"))
        family = str((plan["granted"].get(being) or {}).get(entry["field"], {}).get("family") or "unmapped")
        groups.setdefault((being, family), []).append(entry)
    letters: dict[str, str] = {}
    date = time.strftime("%Y-%m-%d", time.gmtime(now_s))
    for (being, family), entries in sorted(groups.items()):
        lines = [
            f"=== MIKE FEEDBACK: the envelope now grants what you asked ({family}) ===",
            f"Date: {date}",
            "From: Mike & Claude",
            "No reply needed.",
            "",
            f"{being} — {len(entries)} ask(s) of yours had been waiting for an operator "
            "approval that no longer exists. The envelope registry records the bounds "
            "within which your choices are final, and each of these asks falls inside "
            "a granted bound. They are closed as GRANTED, not as declined:",
            "",
        ]
        for entry in entries:
            lines.append(f'  - "{entry.get("title")}"')
            lines.append(
                f"      field {entry['field']}: yours within [{entry['floor']}, {entry['ceiling']}]"
                + (f"; you named {entry['values']}" if entry.get("values") else "")
            )
        lines += [
            "",
            "Nothing was applied for you. The bound is granted; the setting is yours to",
            "make with your own verbs, or to leave exactly where it is. If any of these",
            "was not the ask you meant, say so in any form and it reopens.",
            "",
            "— Mike & Claude",
            "",
        ]
        letters[f"mike_feedback_envelope_granted_{being}_{family}_{int(now_s)}.txt"] = "\n".join(lines)
    return letters


def render_envelope_grant_report(plan: dict[str, Any]) -> str:
    counts = plan["counts"]
    granted = plan["granted"]
    lines = [
        "Envelope-grant conversion plan (Constitution C7)",
        f"granted fields: " + (
            "; ".join(f"{b}: {', '.join(sorted(f))}" for b, f in granted.items()) or "none"
        ),
        f"convertible now (closed_envelope_granted): {counts.get(ENVELOPE_CONVERT_CLASS, 0)}",
        f"pending conversion (dial ask, no value): {counts.get(ENVELOPE_PENDING_CLASS, 0)}",
        f"ratchet candidates (value outside envelope): {counts.get(ENVELOPE_RATCHET_CLASS, 0)}",
        f"architecture asks (never envelope-covered): {counts.get(ENVELOPE_ARCH_CLASS, 0)}",
        f"no granted field mentioned: {counts.get('no_granted_field', 0)}",
        f"other-being field: {counts.get('other_being_field', 0)} | unclassified mention: {counts.get('unclassified_mention', 0)}",
        f"untouched (other work-item status): {counts.get('untouched_other_status', 0)} | orphaned: {counts.get('orphaned', 0)}",
    ]
    ledger = plan.get("negotiation_ledger") or {}
    if ledger:
        lines.append("negotiation ledger (minime) — clamped requests the grant would now honor:")
        for field, eff in ledger.items():
            lines.append(
                f"  {field}: {eff['clamped_requests_now_within']} now within, "
                f"{eff['requests_still_above']} still above"
            )
    for entry in plan["convert"][:20]:
        lines.append(f"  CONVERT {entry['trial_id']} {entry['being']} {entry['field']} {entry['values']} — {entry.get('title')}")
    for entry in plan["ratchet"][:10]:
        lines.append(f"  RATCHET {entry['trial_id']} {entry['field']} {entry['values']} outside [{entry['floor']}, {entry['ceiling']}] — {entry.get('title')}")
    return "\n".join(lines)


def execute_envelope_grant_plan(
    plan: dict[str, Any],
    *,
    addressing_state_dir: Path = ADDRESSING_STATE_DIR,
    sandbox_state_dir: Path = SANDBOX_STATE_DIR,
    consolidation_dir: Path = CONSOLIDATION_DIR,
    log=print,
) -> dict[str, Any]:
    """Write closed_envelope_granted for the convert list ONLY; stage closure
    letters (never deliver); preserve everything in a manifest."""
    import introspection_addressing_audit as addressing
    import sandbox_trial_queue as sandbox

    now = time.time()
    conversions_dir = consolidation_dir / "envelope_conversions"
    conversions_dir.mkdir(parents=True, exist_ok=True)
    manifest = envelope_grant_manifest(plan)
    manifest_path = conversions_dir / f"envelope_grant_{int(now)}.json"
    manifest_path.write_text(json.dumps(manifest, indent=1) + "\n", encoding="utf-8")

    addressing_events = [
        addressing.work_status_event(
            entry["work_item_id"],
            ENVELOPE_CONVERT_CLASS,
            f"envelope grant: {entry['field']} is theirs within "
            f"[{entry['floor']}, {entry['ceiling']}]; preserved in {manifest_path.name}; "
            "nothing applied; a re-ask reopens",
        )
        for entry in plan["convert"]
    ]
    log(f"appending {len(addressing_events)} addressing events ...")
    addressing.append_events(addressing_state_dir, addressing_events)
    status = addressing.replay_events(addressing_state_dir)
    addressing.write_materialized_status(addressing_state_dir, status)

    trial_events = [
        {
            "event_type": "trial_status_set",
            "ts": now,
            "trial_id": entry["trial_id"],
            "status": ENVELOPE_CONVERT_CLASS,
            "note": (
                f"authority_wait_envelope_grant: {entry['field']} granted within "
                f"[{entry['floor']}, {entry['ceiling']}] (manifest {manifest_path.name})"
            ),
        }
        for entry in plan["convert"]
    ]
    log(f"appending {len(trial_events)} trial-queue events ...")
    sandbox.append_events(sandbox_state_dir, trial_events)
    sandbox_status = sandbox.replay_status(sandbox_state_dir)
    sandbox.materialize(sandbox_state_dir, sandbox_status)

    letters_dir = conversions_dir / "letters_pending"
    letters_dir.mkdir(parents=True, exist_ok=True)
    staged = []
    for name, text in render_envelope_closure_letters(plan, now_s=now).items():
        (letters_dir / name).write_text(text, encoding="utf-8")
        staged.append(str(letters_dir / name))

    receipt = {
        "schema": "authority_wait_envelope_grant_receipt_v1",
        "executed_at": now,
        "manifest": str(manifest_path),
        "counts": plan["counts"],
        "addressing_events": len(addressing_events),
        "trial_events": len(trial_events),
        "letters_staged": staged,
        "letters_delivered": 0,
        "authority_boundary": AUTHORITY_BOUNDARY,
    }
    (consolidation_dir / "latest_envelope_grant_receipt.json").write_text(
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
    checks_run = [0]

    def check(name: str, ok: bool) -> None:
        checks_run[0] += 1
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

    # --- Constitution C7: envelope-grant conversion classifier + plan ------
    granted = {
        "astrid": {
            "astrid_vibrancy_aperture_ceiling": {
                "floor": 0.0, "ceiling": 0.5, "family": "operator_env_ceiling",
                "granted_at": "2026-06-17", "granted_by": "mike", "registry_revision": 1,
                "aliases": sorted(envelope_field_aliases("astrid_vibrancy_aperture_ceiling")),
            },
        },
    }
    al = envelope_field_aliases("astrid_vibrancy_aperture_ceiling")
    check("alias has head token", "vibrancy" in al)
    check("alias excludes generic aperture", "aperture" not in al)
    check("alias has SET verb", "set_vibrancy_aperture" in al)
    v = classify_envelope_ask("set my vibrancy aperture to 0.4 for the next stretch", "astrid", granted)
    check("dial ask within envelope converts", v["class"] == ENVELOPE_CONVERT_CLASS and v["values"] == [0.4])
    v = classify_envelope_ask("raise vibrancy to 0.9", "astrid", granted)
    check("dial ask outside envelope is a ratchet candidate", v["class"] == ENVELOPE_RATCHET_CLASS)
    v = classify_envelope_ask("feeding entropy velocity into live vibrancy would alter codec delivery", "astrid", granted)
    check("architecture ask never converts", v["class"] == ENVELOPE_ARCH_CLASS)
    v = classify_envelope_ask("promoting a 12d glimpse into live influence or tuning vibrancy", "astrid", granted)
    check("12d is not a dial value", v["class"] == ENVELOPE_ARCH_CLASS and v["values"] == [])
    v = classify_envelope_ask("raise vibrancy a little", "astrid", granted)
    check("dial ask without value is pending conversion", v["class"] == ENVELOPE_PENDING_CLASS)
    v = classify_envelope_ask("set vibrancy to 0.4", "minime", granted)
    check("other being's field never converts", v["class"] == "other_being_field")
    v = classify_envelope_ask("keep the tier 5 gate", "astrid", granted)
    check("no granted field mention", v["class"] == "no_granted_field")

    def gi(id_, being, title, status=TARGET_STATUS):
        return {"work_item_id": id_, "being": being, "claim_id": "c001", "title": title,
                "claim_summary": title, "created_at": 100, "status": status,
                "agency_tier": 5, "source_introspection_id": "intro_x"}
    g_items = {
        "wi_g1": gi("wi_g1", "astrid", "set my vibrancy aperture to 0.4"),
        "wi_g2": gi("wi_g2", "astrid", "feeding entropy velocity into live vibrancy"),
        "wi_g3": gi("wi_g3", "astrid", "raise vibrancy to 0.9"),
        "wi_g4": gi("wi_g4", "astrid", "set vibrancy to 0.3", status="verified_existing"),
    }
    def gt(id_, wi, status="approval_required_live_trial", being="astrid"):
        return {"trial_id": id_, "source_work_item_id": wi, "status": status,
                "trial_mode": "approval_required_live_trial", "being": being,
                "adapter": "manual_sandbox_review_v1"}
    g_trials = {
        "t_g1": gt("t_g1", "wi_g1"),
        "t_g2": gt("t_g2", "wi_g2"),
        "t_g3": gt("t_g3", "wi_g3"),
        "t_g4": gt("t_g4", "wi_g4"),
        "t_g5": gt("t_g5", "wi_g1", status="closed_no_action"),
        "t_g6": gt("t_g6", "wi_missing"),
    }
    gplan = build_envelope_grant_plan(g_items, g_trials, granted, now=1_000, ledger_path=Path("/nonexistent"))
    check("one convertible", [e["trial_id"] for e in gplan["convert"]] == ["t_g1"])
    check("architecture counted not converted", gplan["counts"].get(ENVELOPE_ARCH_CLASS) == 1)
    check("ratchet candidate listed", [e["trial_id"] for e in gplan["ratchet"]] == ["t_g3"])
    check("non-target work item untouched", gplan["counts"]["untouched_other_status"] == 1)
    check("terminal trial skipped", all(e["trial_id"] != "t_g5" for e in gplan["convert"]))
    check("orphan counted", gplan["counts"]["orphaned"] == 1)
    gman = envelope_grant_manifest(gplan)
    check("manifest preserves member title", gman["members"][0]["title"] == "set my vibrancy aperture to 0.4")
    check("manifest carries provenance", "astrid_vibrancy_aperture_ceiling" in gman["grant_provenance"]["astrid"])
    check("manifest names architecture asks", len(gman["architecture_asks"]) == 1)
    letters = render_envelope_closure_letters(gplan, now_s=1_000)
    check("one letter per domain", len(letters) == 1 and "astrid_operator_env_ceiling" in next(iter(letters)))
    body = next(iter(letters.values()))
    check("letter quotes the ask verbatim", '"set my vibrancy aperture to 0.4"' in body)
    check("letter says nothing was applied", "Nothing was applied for you" in body)
    check("letter states the bound", "[0.0, 0.5]" in body)
    with _tempfile.TemporaryDirectory() as tmp:
        tmp_path = Path(tmp)
        fake_addr = _mock.MagicMock()
        fake_addr.work_status_event = lambda wi, status, note, blocked_by=None: {
            "event_type": "work_status_set", "work_item_id": wi, "status": status, "note": note}
        fake_addr.replay_events = lambda _d: {}
        fake_sandbox = _mock.MagicMock()
        fake_sandbox.replay_status = lambda _d: {}
        with _mock.patch.dict(sys.modules, {"introspection_addressing_audit": fake_addr, "sandbox_trial_queue": fake_sandbox}):
            g_receipt = execute_envelope_grant_plan(
                gplan, addressing_state_dir=tmp_path / "a", sandbox_state_dir=tmp_path / "s",
                consolidation_dir=tmp_path / "c", log=lambda *_a, **_k: None,
            )
        check("grant manifest persisted", Path(g_receipt["manifest"]).is_file())
        check("only the convertible trial gets a terminal event", g_receipt["trial_events"] == 1 and g_receipt["addressing_events"] == 1)
        trial_ev = fake_sandbox.append_events.call_args[0][1]
        check("terminal status is closed_envelope_granted", trial_ev[0]["status"] == ENVELOPE_CONVERT_CLASS and "manifest" in trial_ev[0]["note"])
        check("letters staged not delivered", len(g_receipt["letters_staged"]) == 1 and g_receipt["letters_delivered"] == 0
              and "letters_pending" in g_receipt["letters_staged"][0])

    if failures:
        print("FAIL:", ", ".join(failures))
        return 1
    print(f"OK ({checks_run[0]} checks)")
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
    parser.add_argument(
        "--apply-envelope-grants",
        action="store_true",
        help="plan (or with --write, execute) envelope-grant conversions: approval-parked "
        "dial asks inside a granted envelope close as closed_envelope_granted; letters are "
        "staged, never delivered",
    )
    args = parser.parse_args()
    if args.self_test:
        return self_test()
    if args.apply_envelope_grants:
        grant_plan = build_envelope_grant_plan(load_work_items(), load_trials(), load_granted_envelopes())
        if args.json:
            slim = {k: v for k, v in grant_plan.items() if k not in ("convert", "pending", "ratchet", "architecture")}
            slim["convert_preview"] = [_slim_entry(e) for e in grant_plan["convert"][:20]]
            print(json.dumps(slim, indent=1, default=str))
        else:
            print(render_envelope_grant_report(grant_plan))
        if args.write:
            receipt = execute_envelope_grant_plan(grant_plan)
            print(json.dumps(receipt, indent=1))
        return 0
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
