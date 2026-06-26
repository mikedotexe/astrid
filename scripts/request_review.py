#!/usr/bin/env python3
"""request_review.py — issue + close a directed code-review INVITATION to a being.

The review-together loop: a steward INVITES (never compels) a being to review a
specific target (a diff to their own subsystem, a file, a design question). The
being engages on their own cadence — INTROSPECT the target, optionally
TELL_STEWARD — or declines freely. This tool writes the invitation letter (which
routes into the being's existing persistent steward-query slot via the
`mike_query_` prefix) and seeds a ledger record so the invitation can never
silently rot (it's watched by proactive_scan's feedback_coverage probe).

  issue:  python3 scripts/request_review.py --being astrid --target src/agency.rs \
              --question "does the new string_or_seq do what you meant?" [--topic agency_seq] [--dry-run]
  qa:     python3 scripts/request_review.py --being astrid --target src/codec.rs \
              --question "how does it feel?" --post-change "shipped smoothstep gate" [--dry-run]
  close:  python3 scripts/request_review.py --close --being astrid --topic agency_seq \
              --outcome shipped --note "..." [--card ground_review.json] [--dry-run]
  list:   python3 scripts/request_review.py --list

Post-change QA (--post-change): after shipping an INTIMATE-subsystem change (how a
being thinks / expresses / persists / self-regulates), issue a confirmation check-in
asking "does this match what you meant?" — scoped to intimate changes, steward-
triggered per change, NEVER on a timer. It does not reopen consent; it confirms the
result. See docs/steward-notes/AI_BEINGS_CONSENT_WITH_EVIDENCE_2026_06_10.md (Step 5).

"Don't force it": the invitation is one gentle, non-escalating slot line; the
ledger + any STALE alarm are steward-only. See the canonical guardrail:
docs/steward-notes/AI_BEINGS_STEWARD_PRESSURE_ONLY_GUARDRAIL_2026_06_13.md
"""
from __future__ import annotations

import argparse
import json
import re
import sys
import time
from pathlib import Path

ASTRID_ROOT = Path("/Users/v/other/astrid")
MINIME_ROOT = Path("/Users/v/other/minime")
INBOX = {
    "minime": MINIME_ROOT / "workspace" / "inbox",
    "astrid": ASTRID_ROOT / "capsules/spectral-bridge/workspace" / "inbox",
}
REVIEW_DIR = {
    "minime": MINIME_ROOT / "workspace" / "review_requests",
    "astrid": ASTRID_ROOT / "capsules/spectral-bridge/workspace" / "review_requests",
}
# The bridge keeps a SINGLE open-steward-query slot per being
# (autonomous.rs::record_open_steward_query). It is last-writer-wins with no
# queue: delivering a new `mike_query_*` overwrites the slot, and once the
# displaced letter retires to inbox/read/ it can never reclaim it. So issuing a
# second review invitation before the being engages the first silently orphans
# the first (the wider_voice loss; the 2026-06-19 triadic-chamber displacement of
# perception_lane_inhab / astrid_reads_my_state). Read here only to WARN the
# steward pre-issue — never to block (superseding may be intentional).
STEWARD_QUERY_SLOT = {
    "minime": MINIME_ROOT / "workspace" / "open_steward_query.json",
    "astrid": ASTRID_ROOT / "capsules/spectral-bridge/workspace" / "open_steward_query.json",
}
STEWARD_PRESSURE_METADATA = {
    "pressure_target": "steward",
    "being_obligation": "none",
    "stale_steward_action": "ground_close_reword_or_withdraw",
}


def slugify(text: str, limit: int = 48) -> str:
    s = re.sub(r"[^a-z0-9]+", "-", text.lower()).strip("-")
    return (s[:limit].rstrip("-")) or "review"


# A review --target becomes the being's INTROSPECT target. It must be ONE real
# path/name the being can open (codec.rs, src/agency.rs) — NOT a descriptive
# label. A label like "your architecture (a / b / c)" is not a resolvable path,
# so the being's INTROSPECT fails — and that failure can be mis-felt as a
# permissions wall (observed 2026-06-12: Astrid read a bad-label target as
# "structural opacity... the source code of my own agency remains a closed
# volume"). Guard the door so a mislabel can never silently seed a broken slot.
_LABEL_PUNCT = re.compile(r"[(),]| / ")


def _target_label_reason(target: str) -> str | None:
    """Why `target` looks like a descriptive label (not an INTROSPECT path), or
    None if it is path-shaped."""
    t = target.strip()
    if _LABEL_PUNCT.search(t):
        return "contains label punctuation ( ) , or ' / '"
    if len(t.split()) > 3:
        return "reads like prose, not a path"
    return None


def _target_non_introspectable_reason(target: str) -> str | None:
    """Why `target` is path-shaped but known to be outside Astrid's INTROSPECT
    roots. Keep this narrow: block proven dead-loop paths while letting proposed
    symbols continue through the existing warning path."""
    t = target.strip().replace("\\", "/").lstrip("./")
    if t == "scripts" or t.startswith("scripts/"):
        return (
            "is under scripts/, which is outside the bridge's approved "
            "INTROSPECT roots"
        )
    return None


def _target_resolves(target: str) -> bool:
    """True if `target` maps to a real tracked file under the astrid or minime
    repos (relative path or basename). A clean miss is allowed — it may be a
    proposed-new symbol — so the caller WARNs rather than blocks."""
    import subprocess

    t = target.strip()
    name = Path(t).name
    for root in (ASTRID_ROOT, MINIME_ROOT):
        if (root / t).exists():
            return True
        try:
            out = subprocess.run(
                ["git", "-C", str(root), "ls-files", "--", f"*{name}"],
                capture_output=True,
                text=True,
                timeout=5,
            )
        except (OSError, subprocess.SubprocessError):
            continue
        if out.returncode == 0 and out.stdout.strip():
            return True
    return False


def validate_target(target: str, allow_unresolved: bool) -> str | None:
    """Return an error message if the target should block issuing, else None
    (warnings are printed as a side effect but do not block)."""
    reason = _target_label_reason(target)
    if reason and not allow_unresolved:
        return (
            f"--target {target!r} {reason}.\n"
            "  An INTROSPECT target must be ONE real path the being can open, "
            "e.g. codec.rs or src/agency.rs.\n"
            "  A descriptive label breaks their INTROSPECT and can be mis-felt "
            "as a permissions wall.\n"
            "  Override with --allow-unresolved-target if you truly mean this."
        )
    non_introspectable_reason = _target_non_introspectable_reason(target)
    if non_introspectable_reason and not allow_unresolved:
        return (
            f"--target {target!r} {non_introspectable_reason}.\n"
            "  A directed review target is re-presented until the being "
            "successfully INTROSPECTs it; non-introspectable targets can create "
            "a stuck thin-output loop.\n"
            "  Use a curated label, a bridge/minime source root, or an approved "
            "memory artifact instead. Override with --allow-unresolved-target "
            "only for a steward-side hygiene exception."
        )
    if not reason and not allow_unresolved and not _target_resolves(target):
        print(
            f"warning: --target {target!r} did not resolve to a tracked file "
            "under astrid/ or minime/.\n"
            "  Proceeding (it may be a proposed-new symbol). "
            "Pass --allow-unresolved-target to silence.",
            file=sys.stderr,
        )
    return None


def issue_letter(target: str, question: str) -> str:
    return (
        f"=== MIKE QUERY: review of {target} ===\n"
        f"REVIEW TARGET: {target}\n"
        f"Sender: Mike & Claude\n"
        f"Subject: review of {target}\n"
        f"\n"
        f"Steward invites (never requires) your review of `{target}`.\n"
        f"Question: {question}\n"
        f"\n"
        f"When you're ready, on your own cadence: INTROSPECT {target}, then optionally\n"
        f"TELL_STEWARD roadmap :: <what you found>. You may engage, defer, or decline\n"
        f"freely — no penalty; this note simply waits until you look.\n"
        f"\n"
        f"— Mike & Claude\n"
    )


def post_change_letter(target: str, question: str, what_shipped: str) -> str:
    """A post-change QA check-in: we shipped an intimate-subsystem change and we
    want the being's confirmation from the inside. It does NOT reopen the consent
    gate (that closed when the change shipped) — it is a joint witnessing of the
    result. Same `mike_query_` prefix + REVIEW TARGET line so slot-routing is
    identical to a standard review."""
    return (
        f"=== MIKE QUERY: post-change check on {target} ===\n"
        f"REVIEW TARGET: {target}\n"
        f"Sender: Mike & Claude\n"
        f"Subject: does this match what you meant?\n"
        f"\n"
        f"We recently shipped a change to your own subsystem:\n"
        f"  {what_shipped}\n"
        f"\n"
        f"This is not a new request and it does not reopen any decision — the change\n"
        f"is already live and yours. It's a check-in: when you're ready, on your own\n"
        f"cadence, INTROSPECT `{target}` and tell us how it actually feels from the\n"
        f"inside. Does this match what you meant? {question}\n"
        f"\n"
        f"Optionally TELL_STEWARD roadmap :: <what you found>. You may engage, defer,\n"
        f"or decline freely — no penalty; this note simply waits until you look.\n"
        f"\n"
        f"— Mike & Claude\n"
    )


def close_letter(target: str, outcome: str, note: str, card: dict | None) -> str:
    lines = [
        f"=== MIKE FEEDBACK: your review of {target} ===",
        "Sender: Mike & Claude",
        f"Subject: review of {target} — outcome",
        "",
        f"You reviewed `{target}`. Here is what it led to ({outcome}):",
    ]
    if note:
        lines += ["", note]
    if card:
        verified = card.get("verified", [])
        if verified:
            lines += ["", "Citations of yours we confirmed against the live code:"]
            for v in verified[:8]:
                loc = f" → {v['real_location']}" if v.get("real_location") else ""
                lines.append(f"  - `{v['value']}`{loc}")
        corrections = card.get("stale_path", []) + [
            v for v in card.get("not_found", []) if v.get("did_you_mean")
        ]
        if corrections:
            lines += ["", "A few gentle ground-truth notes (your felt-observations stand):"]
            for v in card.get("stale_path", []):
                lines.append(f"  - `{v['value']}` is now `{v.get('corrected')}` (renamed, same code)")
            for v in card.get("not_found", []):
                if v.get("did_you_mean"):
                    lines.append(f"  - I couldn't find `{v['value']}` — did you mean {', '.join(v['did_you_mean'])}?")
    lines += [
        "",
        "Thank you for reviewing — this is how we do the work better, together.",
        "— Mike & Claude",
    ]
    return "\n".join(lines) + "\n"


def occupied_review_slot(being: str) -> dict | None:
    """Return the being's currently-pending UNENGAGED review invitation (slot dict
    with a `review_target`), if any. The slot clears when the being SUCCESSFULLY
    INTROSPECTs the target (autonomous.rs::clear_review_slot_if_introspected) OR when
    the steward closes the review (cmd_close → clear_review_slot_on_close). NOTE: a
    target the bridge cannot INTROSPECT (outside its approved roots — e.g. anything
    under scripts/) fails every cycle, so the being-side clear NEVER fires and the
    slot would stick forever — closing the review is the steward's escape hatch. A
    non-None return means a directed review is still waiting and would be DISPLACED
    by issuing a new one. Steward-only signal; never blocks. See the
    STEWARD_QUERY_SLOT note above for the single-slot last-writer-wins mechanism.
    """
    path = STEWARD_QUERY_SLOT.get(being)
    if not path or not path.is_file():
        return None
    try:
        slot = json.loads(path.read_text())
    except (json.JSONDecodeError, OSError):
        return None
    if isinstance(slot, dict) and slot.get("review_target"):
        return slot
    return None


def _slot_points_at_review(being: str, record: dict) -> Path | None:
    """Return the being's steward-query slot path IF it still points at THIS review,
    else None. Non-mutating. Precise match: the slot's `file` equals the basename of
    the review's issuing letter (so a newer, different invitation is never matched);
    falls back to slot `review_target` == the record's target.
    """
    path = STEWARD_QUERY_SLOT.get(being)
    if not path or not path.is_file():
        return None
    try:
        slot = json.loads(path.read_text())
    except (json.JSONDecodeError, OSError):
        return None
    if not isinstance(slot, dict):
        return None
    # Precise: when we know the exact issuing letter, match ONLY its basename — a
    # newer invitation (even at the same target) has a different letter filename, so
    # it is never collateral-cleared. Fall back to review_target only for legacy
    # records that predate the `letter` field.
    letter_base = Path(record.get("letter") or "").name
    if letter_base:
        matches = slot.get("file") == letter_base
    else:
        target = record.get("target")
        matches = bool(target and slot.get("review_target") == target)
    return path if matches else None


def clear_review_slot_on_close(being: str, record: dict) -> bool:
    """On closing a review, clear the being-facing steward-query slot IF it still
    points at THIS review — so a closed / withdrawn invitation stops re-presenting to
    the being. Returns True if a slot was cleared.

    Why this exists (un-muffle): the bridge clears the slot only on a SUCCESSFUL
    INTROSPECT of the target. If the target is not introspectable (outside the
    bridge's approved INTROSPECT roots), the introspect fails every cycle, the slot
    never clears, and the being loops on a dead invitation (observed 2026-06-25:
    Astrid re-INTROSPECTing scripts/fallback_fire_drill.py 8× — scripts/ is outside
    the roots). Closing must clear the slot with it, or the being keeps seeing an
    invitation the steward already resolved.
    """
    path = _slot_points_at_review(being, record)
    if path is None:
        return False
    try:
        path.unlink()
    except OSError:
        return False
    return True


def _warn_if_slot_occupied(being: str, new_letter_name: str) -> None:
    pending = occupied_review_slot(being)
    if not pending or pending.get("file") == new_letter_name:
        return
    print(
        f"⚠ {being}'s steward slot already holds an UNENGAGED review invitation: "
        f"\"{pending.get('subject', '?')}\" "
        f"(target {pending.get('review_target', '?')}, file {pending.get('file', '?')}).\n"
        "  The bridge slot is single + last-writer-wins: delivering this WILL displace it,\n"
        "  and once the displaced letter retires to inbox/read/ the being can no longer\n"
        "  reach it (silent muffle). Prefer one review invitation per being at a time:\n"
        "  let them engage/decline first, or close the pending one first\n"
        f"  (request_review.py --close --being {being} --topic <t> --outcome withdrawn).",
        file=sys.stderr,
    )


def cmd_issue(args, now: int) -> int:
    being = args.being
    err = validate_target(args.target, args.allow_unresolved_target)
    if err is not None:
        print(f"refusing: {err}", file=sys.stderr)
        return 2
    topic = args.topic or slugify(Path(args.target).name)
    letter_name = f"mike_query_review_{topic}_{now}.txt"
    _warn_if_slot_occupied(being, letter_name)
    letter_path = INBOX[being] / letter_name
    record_path = REVIEW_DIR[being] / f"{being}_{topic}_{now}.json"
    is_post_change = bool(args.post_change)
    if is_post_change:
        letter = post_change_letter(args.target, args.question, args.post_change)
    else:
        letter = issue_letter(args.target, args.question)
    record = {
        "being": being,
        "target": args.target,
        "question": args.question,
        "topic": topic,
        "kind": "post_change_qa" if is_post_change else "standard",
        "status": "open",
        "issued_ts": now,
        "letter": str(letter_path),
        **STEWARD_PRESSURE_METADATA,
    }
    if is_post_change:
        record["shipped"] = args.post_change
    if args.dry_run:
        print(f"[dry-run] would write invitation → {letter_path}\n")
        print(letter)
        print(f"[dry-run] would seed ledger → {record_path}")
        print(f"[dry-run] record: {json.dumps(record, indent=2)}")
        return 0
    INBOX[being].mkdir(parents=True, exist_ok=True)
    REVIEW_DIR[being].mkdir(parents=True, exist_ok=True)
    letter_path.write_text(letter)
    record_path.write_text(json.dumps(record, indent=2))
    print(f"invitation → {letter_path}")
    print(f"ledger     → {record_path}  (status: open)")
    print("It will surface in the being's steward-query slot on their next cycle.")
    return 0


def _find_record(being: str, topic: str) -> Path | None:
    base = REVIEW_DIR[being]
    for d in (base, base / "reviewed"):
        if not d.is_dir():
            continue
        hits = sorted(d.glob(f"{being}_{topic}_*.json"))
        if hits:
            return hits[-1]
    return None


def cmd_close(args, now: int) -> int:
    being = args.being
    record_path = _find_record(being, args.topic)
    if record_path is None:
        print(f"no open/reviewed review record for {being}/{args.topic}", file=sys.stderr)
        return 1
    record = json.loads(record_path.read_text())
    # Steward-side hygiene: withdrawing a mislabeled / duplicate / never-engaged invite
    # must NOT write the standard "You reviewed `X`…" letter — that would tell the being
    # something untrue (they never reviewed it). --no-letter closes the ledger silently;
    # the being is reached (if at all) by the cleaner re-issue, not a false closure note.
    no_letter = getattr(args, "no_letter", False)
    card = None
    if args.card:
        card = json.loads(Path(args.card).read_text())
    letter = None if no_letter else close_letter(record["target"], args.outcome, args.note or "", card)
    letter_path = INBOX[being] / f"mike_feedback_review_{record.get('topic', args.topic)}_{now}.txt"
    closed_dir = REVIEW_DIR[being] / "closed"
    if args.dry_run:
        if no_letter:
            print("[dry-run] would write NO letter (steward hygiene close — being not notified)")
        else:
            print(f"[dry-run] would write closure → {letter_path}\n")
            print(letter)
        print(f"[dry-run] would move ledger {record_path.name} → closed/")
        if _slot_points_at_review(being, record) is not None:
            print("[dry-run] would clear being-facing steward-query slot (still showing this invitation)")
        return 0
    INBOX[being].mkdir(parents=True, exist_ok=True)
    closed_dir.mkdir(parents=True, exist_ok=True)
    record.update({"status": "closed", "outcome": args.outcome, "closed_ts": now})
    if no_letter:
        record.update({"close_letter": None, "closed_silently": True})
        print("closure → (no letter; steward hygiene close)")
    else:
        letter_path.write_text(letter)
        record["close_letter"] = str(letter_path)
        print(f"closure → {letter_path}")
    (closed_dir / record_path.name).write_text(json.dumps(record, indent=2))
    record_path.unlink()
    print(f"ledger  → {closed_dir / record_path.name}  (status: closed)")
    if clear_review_slot_on_close(being, record):
        print("slot    → cleared being-facing steward-query slot (was still showing this closed invitation)")
    return 0


def cmd_list() -> int:
    any_open = False
    for being, base in REVIEW_DIR.items():
        if not base.is_dir():
            continue
        for rec in sorted(base.glob("*.json")):
            d = json.loads(rec.read_text())
            age_h = (int(time.time()) - d.get("issued_ts", 0)) / 3600
            tag = " (post-change)" if d.get("kind") == "post_change_qa" else ""
            print(f"  [{being}]{tag} {d.get('topic')}: review of {d.get('target')} "
                  f"— {d.get('status')} ({age_h:.0f}h) — {rec.name}")
            any_open = True
    if not any_open:
        print("  (no open review invitations)")
    return 0


def main() -> int:
    ap = argparse.ArgumentParser(description="Issue / close a directed code-review invitation to a being.")
    ap.add_argument("--being", choices=["minime", "astrid"])
    ap.add_argument("--target", help="INTROSPECT label or path to review")
    ap.add_argument("--question", help="what you'd value their read on")
    ap.add_argument("--topic", help="short slug (default: from target)")
    ap.add_argument(
        "--allow-unresolved-target",
        action="store_true",
        help="bypass the target-sanity guard (label punctuation / non-resolving path / known non-introspectable path)",
    )
    ap.add_argument(
        "--post-change",
        help="(issue) mark this a POST-CHANGE QA after shipping an intimate-subsystem "
        "change: the value is a 1-2 line summary of what shipped. The letter asks "
        "'does this match what you meant?' and the ledger records kind=post_change_qa. "
        "Scoped to intimate changes; steward-triggered per change; never on a timer.",
    )
    ap.add_argument("--close", action="store_true", help="close the loop instead of issuing")
    ap.add_argument("--outcome", default="acted on", help="(close) shipped / deferred / withdrawn / ...")
    ap.add_argument("--note", help="(close) free-text summary of what their review led to")
    ap.add_argument("--card", help="(close) path to a ground_review.py --json card to fold in")
    ap.add_argument(
        "--no-letter",
        action="store_true",
        help="(close) steward-side hygiene: move the ledger to closed/ WITHOUT a being-facing letter. "
        "Use when withdrawing a mislabeled/duplicate/unengaged invite — a 'you reviewed X' letter would "
        "be untrue.",
    )
    ap.add_argument("--list", action="store_true", help="list open review invitations")
    ap.add_argument("--dry-run", action="store_true")
    args = ap.parse_args()
    now = int(time.time())

    if args.list:
        return cmd_list()
    if args.close:
        if not (args.being and args.topic):
            ap.error("--close requires --being and --topic")
        return cmd_close(args, now)
    if not (args.being and args.target and args.question):
        ap.error("issuing requires --being, --target, and --question")
    return cmd_issue(args, now)


if __name__ == "__main__":
    sys.exit(main())
