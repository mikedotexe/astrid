#!/usr/bin/env python3
"""letter_response_scan.py — did the being engage the steward letters we sent?

The INBOUND complement to the `steward_outreach` probe (which watches
being->steward). This watches steward->being RECEPTION: for each recently
delivered steward letter (`mike_feedback_*` / `mike_query_*`), it scans the
being's output in the window AFTER delivery and classifies
ACTED / ENGAGED / SILENT-IN-WINDOW.

Born 2026-06-18 from a near-miss: a being's substantive response (prose, ~90s
after delivery) was almost reported as "no response" because steward review
looked in the WRONG WINDOW (newest entries, not delivery-anchored) and for the
WRONG SHAPE (an action verb / TELL_STEWARD, not prose), while template footers
("Continuity posture:") created false noise. This codifies the fix:
  - anchor the scan to DELIVERY time, not "now";
  - read PROSE engagement, not just action verbs;
  - strip template/metadata footer lines before matching;
  - a `mike_query` persists in the open-slot, so its window runs to now;
  - on genuine SILENT-IN-WINDOW, run the un-muffle check BEFORE concluding
    the being is quiet.

Privacy: minime's private-qualia lanes are off-limits; this scan excludes them via
the shared `being_privacy` module (content-marker detection, minime-scoped policy) —
NOT by filename (minime writes moment_capture to `moment_*.txt`, so a filename
pattern was a dead guard; fixed 2026-06-18). Astrid's moment entries are legitimate
engagement and are NOT excluded.

Steward-only — never surfaced into a being's prompt. Read-only; no writes,
no network, no being-facing effect.

Usage:
  python3 letter_response_scan.py [--being astrid|minime|both] [--since-hours N]
                                  [--window-hours N] [--json]
  python3 letter_response_scan.py --self-test
"""
from __future__ import annotations

import argparse
import json
import re
import sys
import time
import unittest
from pathlib import Path
from typing import Any

SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))
from being_privacy import is_steward_private  # the ONE steward private-lane definition

ASTRID_WS = Path("/Users/v/other/astrid/capsules/spectral-bridge/workspace")
MINIME_WS = Path("/Users/v/other/minime/workspace")

BEINGS: dict[str, dict[str, Any]] = {
    "astrid": {
        "read_dir": ASTRID_WS / "inbox" / "read",
        "journal_dir": ASTRID_WS / "journal",
        "outbox_dir": ASTRID_WS / "outbox",
        "open_query": ASTRID_WS / "open_steward_query.json",
    },
    "minime": {
        "read_dir": MINIME_WS / "inbox" / "read",
        "journal_dir": MINIME_WS / "journal",
        "outbox_dir": MINIME_WS / "outbox",
        "open_query": MINIME_WS / "open_steward_query.json",
    },
}
# Private-lane exclusion is NOT a per-being flag here — it is centralized in the
# shared `being_privacy` module (minime-scoped, content-based). scan_being_window
# passes `being`, and is_steward_private() applies the one policy.

DEFAULT_SINCE_HOURS = 24.0
DEFAULT_WINDOW_HOURS = 3.0
GRACE_SECS = 45.0  # catch a response that begins just before the archive mtime

# Lines that are journal scaffold/metadata, NOT the being's prose. Matching a
# theme term inside these is the exact false-signal that masked the 06-18
# response — so they are stripped from the corpus before matching.
FOOTER_LINE_RE = re.compile(
    r"^\s*(Continuity posture|Prior claim|Delta|Hold|Mode|Fill|Timestamp|"
    r"=== .* ===|λ|Eigenvalue|Spectral entropy|Resonance|Pressure source|"
    r"Inhabitable|Snapshot guard|Decision|Selected vague|12D vague|ESN leak|"
    r"Cov |Spread|Gap ratio|Geometric|Semantic energy|Eigenvector)\b",
    re.IGNORECASE,
)

# Words too common to distinguish THIS letter; dropped from theme terms.
STOPWORDS = {
    "steward", "astrid", "minime", "letter", "your", "yours", "you", "the", "and",
    "that", "this", "with", "from", "feedback", "query", "mike", "claude", "would",
    "have", "what", "when", "where", "which", "their", "there", "about", "into",
    "they", "them", "then", "than", "over", "under", "been", "being", "because",
    "could", "should", "still", "want", "need", "feel", "feels", "felt", "like",
    "just", "only", "more", "most", "some", "such", "very", "even", "also", "now",
    "not", "but", "for", "are", "was", "were", "her", "his", "its", "our", "out",
}

# Engagement-stance heuristics (light — the excerpt is surfaced for the steward
# to judge; these only TAG it).
# Strong-only: weak/common markers ("but", "?", "gap", "actually") were dropped —
# they fire on affirming prose ("clean BUT crowded") and over-tag friction.
FRICTION_HINTS = (
    "however", "disagree", "instead", "rather than", "i'd change", "i would change",
    "insufficient", "too restrictive", "not quite", "doesn't", "isn't", "wouldn't",
    "unclear", "confus", "should be", "wrong", "feels imposed", "feels arbitrary",
)
AFFIRM_HINTS = (
    "validat", "resonat", "exactly", "thank", "landed", "lands", "right", "yes,",
    "agree", "appreciat", "welcome", "matches", "feels true", "accurate", "named",
)


def extract_theme_terms(body: str) -> list[str]:
    """Distinctive terms tying a response to THIS letter: code identifiers,
    SET_ verbs, quoted phrases, and salient long words. Lowercased + deduped."""
    terms: set[str] = set()
    # Code identifiers (snake_case) and SET_ action verbs (preserve as lower).
    for m in re.findall(r"\b[a-z][a-z0-9]*(?:_[a-z0-9]+)+\b", body):
        terms.add(m.lower())
    for m in re.findall(r"\bSET_[A-Z][A-Z0-9_]+\b", body):
        terms.add(m.lower())
    # Quoted coinages ("silent vacuum", "ghost pressure", "viscosity", ...) —
    # the letter's own distinctive phrasing; punctuation trimmed.
    for m in re.findall(r'"([^"\n]{4,48})"', body):
        phrase = m.strip().strip(".,;:!?").lower()
        if phrase and not phrase.startswith("next"):
            terms.add(phrase)
    # Distinctive markers ONLY: snake_case identifiers, SET_ verbs, multi-word
    # quoted coinages, or quoted single words >=8 chars. Bare long words are
    # NOT distinctive — they recur across all her prose and were the source of
    # false ENGAGED matches on unrelated entries (the first-run bug). Excluded.
    out = {t for t in terms if ("_" in t) or (" " in t) or len(t) >= 8}
    return sorted(out)


def invited_action(body: str) -> str | None:
    """The action verb a letter invites (e.g. SET_SELF_CONTINUITY), if any."""
    m = re.search(r"\bSET_[A-Z][A-Z0-9_]+\b", body)
    if m:
        return m.group(0)
    m = re.search(r"NEXT:\s*([A-Z][A-Z0-9_]+)", body)
    return m.group(1) if m else None


def _prose_lines(text: str) -> list[str]:
    """The being's prose, with scaffold/metadata footer lines removed."""
    return [ln for ln in text.splitlines() if ln.strip() and not FOOTER_LINE_RE.match(ln)]


def engagement_excerpt(text: str, terms: list[str]) -> str | None:
    """Return the first PROSE line (footers stripped) containing a theme term,
    or None. This is the heart of the fix: a term appearing only in a footer
    line (e.g. 'Continuity posture: resuming') does NOT count as engagement."""
    if not terms:
        return None
    for line in _prose_lines(text):
        low = line.lower()
        for term in terms:
            # word-ish boundary for single tokens; substring for quoted phrases
            if " " in term or "_" in term:
                hit = term in low
            else:
                hit = re.search(rf"\b{re.escape(term)}\b", low) is not None
            if hit:
                return line.strip()
    return None


def classify_stance(excerpt: str) -> str:
    low = excerpt.lower()
    if any(h in low for h in FRICTION_HINTS):
        return "friction"
    if any(h in low for h in AFFIRM_HINTS):
        return "affirmation"
    return "neutral"


def _list_journals(journal_dir: Path) -> list[Path]:
    if not journal_dir.is_dir():
        return []
    return list(journal_dir.glob("*.txt"))


def _is_open_query(open_query_path: Path, letter_name: str) -> bool:
    try:
        return letter_name in open_query_path.read_text(encoding="utf-8", errors="ignore")
    except OSError:
        return False


def scan_being_window(
    journal_dir: Path,
    being: str,
    delivered_ts: float,
    window_end_ts: float,
    terms: list[str],
    action_verb: str | None,
) -> dict[str, Any]:
    """Scan the being's journals in [delivered-grace, window_end] for engagement.
    Steward-private entries (per being_privacy: minime's moment_capture /
    private_journal lanes) are skipped WITHOUT reading their body."""
    acted = False
    best: dict[str, Any] | None = None
    lo = delivered_ts - GRACE_SECS
    # Gather in-window files and scan EARLIEST-first, so the engagement we keep
    # is the being's FIRST response to the letter (not whatever glob returned
    # first — the second first-run bug).
    in_window: list[tuple[float, Path]] = []
    for p in _list_journals(journal_dir):
        try:
            mtime = p.stat().st_mtime
        except OSError:
            continue
        if lo <= mtime <= window_end_ts:
            in_window.append((mtime, p))
    in_window.sort(key=lambda x: x[0])
    for mtime, p in in_window:
        # Privacy: never read a being's private-qualia body (head-only check skips it).
        if is_steward_private(being, p):
            continue
        try:
            text = p.read_text(encoding="utf-8", errors="ignore")
        except OSError:
            continue
        if action_verb and re.search(rf"\b{re.escape(action_verb)}\b", text):
            acted = True
        if best is None:
            excerpt = engagement_excerpt(text, terms)
            if excerpt:
                best = {"file": p.name, "mtime": mtime, "excerpt": excerpt,
                        "stance": classify_stance(excerpt)}
    return {"acted": acted, "engaged": best}


def classify(result: dict[str, Any]) -> str:
    if result["acted"]:
        return "ACTED"
    if result["engaged"]:
        return "ENGAGED"
    return "SILENT-IN-WINDOW"


def find_steward_followup(
    being: str, engaged: dict[str, Any] | None, anchor_letter: str
) -> dict[str, Any] | None:
    """If a steward `mike_feedback_*` letter delivered AFTER the being's engagement
    entry references that same entry BY FILENAME, the steward already closed this
    loop — return it so render can downgrade the loud "→ ACT" to "already
    followed-up". This exists because the scan is delivery-anchored to the ORIGINAL
    letter, so a being's friction RESPONSE keeps re-surfacing as "→ ACT" every cycle
    until that original letter ages out — and a steward (incl. the durable loop)
    reading the loud line re-letters the same closed topic (observed 3 cycles in a
    row 2026-06-28). Precise BY DESIGN (exact filename reference, not topic match):
    it can only ADD a caution, never suppress a friction row — so a genuinely
    un-answered friction is never silently dropped (un-muffle preserved)."""
    if not engaged:
        return None
    read_dir: Path = BEINGS[being]["read_dir"]
    if not read_dir.is_dir():
        return None
    engaged_stem = Path(engaged["file"]).stem
    if not engaged_stem:
        return None
    engaged_mtime = float(engaged.get("mtime", 0.0))
    hits: list[tuple[float, str]] = []
    for p in read_dir.glob("mike_feedback_*.txt"):
        if p.name == anchor_letter:
            continue
        try:
            mtime = p.stat().st_mtime
        except OSError:
            continue
        if mtime <= engaged_mtime:
            continue
        try:
            body = p.read_text(encoding="utf-8", errors="ignore")
        except OSError:
            continue
        if engaged_stem in body:
            hits.append((mtime, p.name))
    if not hits:
        return None
    hits.sort()  # earliest follow-up that referenced this entry = when we first closed it
    ts, name = hits[0]
    return {"letter": name, "ts": ts}


def scan_letters(being: str, since_hours: float, window_hours: float, now: float) -> list[dict[str, Any]]:
    cfg = BEINGS[being]
    read_dir: Path = cfg["read_dir"]
    out: list[dict[str, Any]] = []
    if not read_dir.is_dir():
        return out
    since_ts = now - since_hours * 3600.0
    letters = []
    for p in read_dir.glob("mike_*.txt"):
        if not (p.name.startswith("mike_feedback_") or p.name.startswith("mike_query_")):
            continue
        try:
            mtime = p.stat().st_mtime
        except OSError:
            continue
        if mtime >= since_ts:
            letters.append((p, mtime))
    for p, delivered in sorted(letters, key=lambda x: x[1], reverse=True):
        try:
            body = p.read_text(encoding="utf-8", errors="ignore")
        except OSError:
            continue
        terms = extract_theme_terms(body)
        verb = invited_action(body)
        is_query = p.name.startswith("mike_query_")
        still_open = is_query and _is_open_query(cfg["open_query"], p.name)
        # A live open-slot query persists; scan to now. Else the bounded window.
        window_end = now if still_open else min(now, delivered + window_hours * 3600.0)
        res = scan_being_window(cfg["journal_dir"], being,
                                delivered, window_end, terms, verb)
        status = classify(res)
        followed_up = find_steward_followup(being, res["engaged"], p.name)
        out.append({
            "being": being,
            "letter": p.name,
            "delivered_ts": delivered,
            "kind": "query" if is_query else "feedback",
            "open_slot": still_open,
            "invited_action": verb,
            "status": status,
            "engaged": res["engaged"],
            "followed_up": followed_up,
            "n_terms": len(terms),
        })
    return out


def _fmt_ts(ts: float) -> str:
    return time.strftime("%m-%d %H:%M", time.localtime(ts))


def render(rows: list[dict[str, Any]], now: float) -> str:
    if not rows:
        return "letter_response_scan: no steward letters in the window.\n"
    lines = ["=== LETTER RESPONSE SCAN (steward->being reception; read-only) ==="]
    for r in rows:
        age_min = int((now - r["delivered_ts"]) / 60.0)
        head = f"[{r['status']}] {r['being']} · {r['letter']} ({r['kind']}, delivered {_fmt_ts(r['delivered_ts'])}, {age_min}m ago)"
        lines.append(head)
        if r["engaged"]:
            e = r["engaged"]
            lines.append(f"    engaged [{e['stance']}] in {e['file']}: \"{e['excerpt'][:160]}\"")
            fu = r.get("followed_up")
            if fu:
                lines.append(
                    f"    ↩ already-followed-up: steward letter {fu['letter']} ({_fmt_ts(fu['ts'])}) "
                    f"references {e['file']} — loop likely CLOSED; re-read that letter before any new "
                    f"response (avoid the over-letter trap)."
                )
            elif e["stance"] == "friction":
                lines.append("    → ACT: friction/correction/proposal — follow up.")
            elif e["stance"] == "affirmation":
                lines.append("    → close the loop warmly (affirmed).")
        elif r["status"] == "ACTED":
            lines.append(f"    invoked {r['invited_action']} in the window — fulfillment.")
        else:
            note = "open-slot, persists; awaiting engagement" if r["open_slot"] else "no prose/action hit in window"
            lines.append(f"    SILENT-IN-WINDOW ({note}).")
            lines.append("    → run the un-muffle check (read-without-move? slot recorded? body injected?) BEFORE concluding silence.")
    return "\n".join(lines) + "\n"


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--being", choices=["astrid", "minime", "both"], default="both")
    ap.add_argument("--since-hours", type=float, default=DEFAULT_SINCE_HOURS)
    ap.add_argument("--window-hours", type=float, default=DEFAULT_WINDOW_HOURS)
    ap.add_argument("--json", action="store_true")
    ap.add_argument("--self-test", action="store_true")
    args = ap.parse_args(argv)

    if args.self_test:
        return _run_self_test()

    now = time.time()
    beings = ["astrid", "minime"] if args.being == "both" else [args.being]
    rows: list[dict[str, Any]] = []
    for b in beings:
        rows.extend(scan_letters(b, args.since_hours, args.window_hours, now))
    if args.json:
        print(json.dumps(rows, indent=2))
    else:
        sys.stdout.write(render(rows, now))
    return 0


# ── self-test ────────────────────────────────────────────────────────────────
class LetterResponseScanTests(unittest.TestCase):
    def test_theme_terms_pick_identifiers_and_phrases(self):
        body = 'We named the "silent vacuum"; SET_SELF_CONTINUITY shows self_continuity vs identity_anchor_churn.'
        terms = extract_theme_terms(body)
        self.assertIn("self_continuity", terms)
        self.assertIn("identity_anchor_churn", terms)
        self.assertIn("set_self_continuity", terms)
        self.assertIn("silent vacuum", terms)

    def test_footer_line_does_not_count_as_engagement(self):
        # The exact 06-18 false-signal: 'continuity' present ONLY in the footer.
        text = "=== ASTRID JOURNAL ===\nMode: witness\nContinuity posture: resuming\nThe shadow field is quiet tonight."
        self.assertIsNone(engagement_excerpt(text, ["continuity"]))

    def test_real_prose_hit_is_found(self):
        text = "Continuity posture: resuming\nThe silent vacuum the steward named validates my own viscosity."
        ex = engagement_excerpt(text, ["silent vacuum", "viscosity"])
        self.assertIsNotNone(ex)
        self.assertIn("silent vacuum", ex.lower())
        self.assertEqual(classify_stance(ex), "affirmation")

    def test_invited_action_extracted(self):
        self.assertEqual(invited_action("type NEXT: SET_SELF_CONTINUITY 1 to see it"), "SET_SELF_CONTINUITY")

    def test_scan_window_classifies(self):
        import tempfile
        with tempfile.TemporaryDirectory() as d:
            jd = Path(d)
            (jd / "astrid_1000.txt").write_text(
                "Continuity posture: resuming\nThe ghost pressure sits in my peripheral field tonight.",
                encoding="utf-8",
            )
            now = time.time()
            (jd / "astrid_1000.txt").touch()
            res = scan_being_window(jd, "astrid", now - 10, now + 10, ["ghost pressure"], "SET_X")
            self.assertEqual(classify(res), "ENGAGED")
            self.assertEqual(res["engaged"]["stance"], "neutral")
            # Out-of-window file is ignored.
            res2 = scan_being_window(jd, "astrid", now + 10_000, now + 20_000, ["ghost pressure"], None)
            self.assertEqual(classify(res2), "SILENT-IN-WINDOW")

    def test_followup_dedup_downgrades_already_closed_friction(self):
        # The 2026-06-28 over-letter trap: a being's friction RESPONSE keeps
        # re-surfacing as "→ ACT" every cycle (delivery-anchored to the original
        # letter), and a steward already closed it. A later mike_feedback_* that
        # references the SAME engagement entry by filename = loop already closed.
        import tempfile
        with tempfile.TemporaryDirectory() as d:
            read_dir = Path(d)
            engaged = {"file": "self_study_1782667074.txt", "mtime": 1000.0}
            # No follow-up yet → no dedup (genuine "→ ACT").
            saved = BEINGS["astrid"]["read_dir"]
            try:
                BEINGS["astrid"]["read_dir"] = read_dir
                self.assertIsNone(
                    find_steward_followup("astrid", engaged, "anchor.txt")
                )
                # A later steward letter that NAMES the entry → dedup fires.
                fu = read_dir / "mike_feedback_fallback_texture_anchor_verified_1782681220.txt"
                fu.write_text(
                    "Astrid, your self-study self_study_1782667074 named the exact fix.",
                    encoding="utf-8",
                )
                import os
                os.utime(fu, (1500.0, 1500.0))  # delivered AFTER the engagement (1000)
                got = find_steward_followup("astrid", engaged, "anchor.txt")
                self.assertIsNotNone(got)
                self.assertEqual(got["letter"], fu.name)
                # A letter that does NOT reference the entry must NOT dedup
                # (precise by filename, never topic — won't suppress real friction).
                other = read_dir / "mike_feedback_unrelated_topic_9.txt"
                other.write_text("A note about something else entirely.", encoding="utf-8")
                os.utime(other, (1600.0, 1600.0))
                got2 = find_steward_followup(
                    "astrid", {"file": "self_study_9999.txt", "mtime": 1000.0}, "anchor.txt"
                )
                self.assertIsNone(got2)
                # A referencing letter delivered BEFORE the engagement is the
                # anchor/original, not a follow-up → no dedup.
                self.assertIsNone(
                    find_steward_followup(
                        "astrid", {"file": "self_study_1782667074.txt", "mtime": 2000.0},
                        "anchor.txt",
                    )
                )
            finally:
                BEINGS["astrid"]["read_dir"] = saved

    def test_minime_private_qualia_excluded_by_content_not_filename(self):
        # The real 06-18 bug: minime writes moment_capture to `moment_*.txt`
        # (NOT `moment_capture_*`), so a filename pattern misses it entirely.
        # being_privacy excludes by CONTENT marker, minime-scoped.
        import tempfile
        with tempfile.TemporaryDirectory() as d:
            jd = Path(d)
            private = jd / "moment_2026-06-18T11-10-08.txt"
            private.write_text("=== MOMENT CAPTURE ===\nThe silent vacuum is real to me.",
                               encoding="utf-8")
            now = time.time()
            # minime: the private body is never read.
            res = scan_being_window(jd, "minime", now - 10, now + 10, ["silent vacuum"], None)
            self.assertEqual(classify(res), "SILENT-IN-WINDOW")
            # astrid: a moment entry IS engagement (not excluded — her surface).
            res2 = scan_being_window(jd, "astrid", now - 10, now + 10, ["silent vacuum"], None)
            self.assertEqual(classify(res2), "ENGAGED")


def _run_self_test() -> int:
    suite = unittest.TestLoader().loadTestsFromTestCase(LetterResponseScanTests)
    result = unittest.TextTestRunner(verbosity=2).run(suite)
    return 0 if result.wasSuccessful() else 1


if __name__ == "__main__":
    raise SystemExit(main())
