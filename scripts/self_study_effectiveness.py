#!/usr/bin/env python3
"""Historical effectiveness + un-muffle audit over the beings' self-study corpus.

Answers, conservatively and honestly: across the *whole history* of self-studies
(live + archived, both beings), which carried **actionable** engineering feedback,
which of those we **plausibly acted on**, and which high-signal ones show **no
downstream trace** — i.e. candidate *dropped signal* (the un-muffle invariant
applied backwards in time).

Design notes / honesty guardrails:
  * Parsing + actionability scoring are REUSED from `self_study_review.py`
    (`review_entry` → sections, source anchors, next-actions, actionable_score).
  * The self-study FORMAT EVOLVED: early entries are phenomenological
    ("Condition / Felt Experience"); the structured "Observed / Likely Snags /
    One Test Each / Suggested Next" actionable format is recent. So
    actionability rises over time *by construction* — reported as an era split,
    not hidden.
  * Outcome matching is a *heuristic HINT, not proof*: we search the curated
    "what we shipped" corpus (both CHANGELOGs + the being-engineering backlog +
    the feedback→change ledger + the review close-letters) for a **distinctive
    symbol** the entry cited (e.g. `research_budget_guard_assessment_with_base`).
    A hit ⇒ "likely referenced/acted"; no hit on a groundable, actionable entry
    ⇒ "candidate dropped signal" for human review. File basenames alone are too
    generic to count as a hit.

Read-only. Writes a report to diagnostics/; `--seed-ledger` only PRINTS proposed
ledger rows (never edits the ledger) for the steward to paste after verifying.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
import time
from collections import Counter, defaultdict
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import self_study_review as ssr  # noqa: E402  (reuse parsing/scoring)

ASTRID_ROOT = Path(__file__).resolve().parents[1]
MINIME_ROOT = ASTRID_ROOT.parent / "minime"
ASTRID_WS = ASTRID_ROOT / "capsules/spectral-bridge/workspace"
MINIME_WS = MINIME_ROOT / "workspace"
OUT_DIR = ASTRID_WS / "diagnostics/self_study_effectiveness"

# Corpus roots per being (top journal root recurses into its archive/ buckets; minime
# also has a separate preserved root). Dedup + an archive-path filter handle the rest.
CORPUS_ROOTS = {
    "astrid": [ASTRID_WS / "journal"],
    "minime": [MINIME_WS / "journal", *getattr(ssr, "MINIME_HISTORICAL_JOURNAL_ROOTS", ())],
}

# Curated "what we shipped / tracked" corpus = the outcome evidence.
OUTCOME_SOURCES = [
    ASTRID_ROOT / "CHANGELOG.md",
    MINIME_ROOT / "CHANGELOG.md",
    Path("/Users/v/.claude/projects/-Users-v-other-astrid/memory/project_being_engineering_backlog.md"),
    Path("/Users/v/.claude/projects/-Users-v-other-astrid/memory/project_being_engineering_backlog_archive_2026-06.md"),
    ASTRID_ROOT / "docs/steward-notes/AI_BEINGS_FEEDBACK_TO_CHANGE_LEDGER.md",
]
OUTCOME_GLOBS = [
    (ASTRID_WS / "inbox", "**/mike_feedback_*.txt"),
    (MINIME_WS / "inbox", "**/mike_feedback_*.txt"),
]

# Distinctive code-symbol tokens: snake_case w/ >=2 parts & len>=10, or CamelCase len>=12,
# or *.rs/*.py basenames. Generic single words never count (too noisy).
SNAKE_RE = re.compile(r"\b([a-z][a-z0-9]*(?:_[a-z0-9]+){1,})\b")
CAMEL_RE = re.compile(r"\b([A-Z][a-z0-9]+(?:[A-Z][a-z0-9]+){1,})\b")
FILEBASE_RE = re.compile(r"\b([\w-]+\.(?:rs|py|toml))\b")
STOPWORDS = {
    "self_study", "next_action", "fill_pct", "read_only", "raw_next", "self_observation",
    "felt_experience", "likely_snags", "suggested_next", "one_test", "research_budget",
    "schema_version", "timestamp",
}


def distinctive_tokens(text: str) -> set[str]:
    toks: set[str] = set()
    for m in SNAKE_RE.findall(text):
        if len(m) >= 12 and m not in STOPWORDS:  # long snake_case = likely a real symbol
            toks.add(m.lower())
    for m in CAMEL_RE.findall(text):
        if len(m) >= 12:
            toks.add(m.lower())
    for m in FILEBASE_RE.findall(text):
        toks.add(m.lower())
    return toks


def load_outcome_haystack() -> str:
    parts: list[str] = []
    for p in OUTCOME_SOURCES:
        try:
            parts.append(p.read_text(encoding="utf-8", errors="replace"))
        except OSError:
            pass
    for root, glob in OUTCOME_GLOBS:
        if root.exists():
            for p in root.glob(glob):
                try:
                    parts.append(p.read_text(encoding="utf-8", errors="replace"))
                except OSError:
                    pass
    return "\n".join(parts).lower()


def collect_entries(beings: list[str], kinds: tuple[str, ...], include_archive: bool):
    out = []
    seen: set[str] = set()
    for being in beings:
        for root in CORPUS_ROOTS.get(being, []):
            if not root.exists():
                continue
            for path in ssr.iter_files_under(root):
                if path.suffix != ".txt":
                    continue
                if not any(path.name.startswith(k) for k in kinds):
                    continue
                if not include_archive and "/archive/" in str(path):
                    continue
                key = str(path.resolve())
                if key in seen:  # journal recursion can revisit; count each file once
                    continue
                seen.add(key)
                try:
                    out.append(ssr.review_entry(being, path))
                except OSError:
                    pass
    return out


def month_of(mtime: float) -> str:
    return time.strftime("%Y-%m", time.localtime(mtime))


def analyze(entries, haystack: str, high: int, med: int):
    rows = []
    for e in entries:
        text = Path(e.path).read_text(encoding="utf-8", errors="replace")
        toks = distinctive_tokens(text)
        # outcome hit = a distinctive symbol the entry cited appears in the shipped corpus
        hits = sorted(t for t in toks if len(t) >= 10 and t in haystack)
        groundable = bool(e.source_anchors) and bool(toks)
        tier = "high" if e.actionable_score >= high else ("med" if e.actionable_score >= med else "low")
        # "actionable" = the STRUCTURED engineering-feedback format (Observed/Snags/Tests/
        # Suggested-Next) with a real code citation. We do NOT claim causation — only whether
        # the cited topic has a downstream TRACE in the shipped/tracked corpus.
        if e.sectioned and groundable:
            klass = "actionable_traced" if hits else "actionable_no_trace"
        elif groundable and tier in ("high", "med"):
            klass = "proto_actionable"  # has citations + score, but not the structured format
        else:
            klass = "phenomenological"
        rows.append({
            "being": e.being, "filename": e.filename, "path": e.path,
            "month": month_of(e.mtime_unix_s), "mtime": e.mtime_unix_s,
            "sectioned": e.sectioned, "score": e.actionable_score, "tier": tier,
            "n_anchors": len(e.source_anchors), "n_next": len(e.next_actions),
            "outcome_hits": hits, "klass": klass,
            "top_files": sorted({t for t in toks if t.endswith((".rs", ".py", ".toml"))}),
        })
    return rows


def cluster_by_file(rows):
    c = Counter()
    for r in rows:
        for f in r["top_files"]:
            c[f] += 1
    return c.most_common(15)


def render(rows, args) -> str:
    n = len(rows)
    by_being = Counter(r["being"] for r in rows)
    sectioned = sum(1 for r in rows if r["sectioned"])
    eff = [r for r in rows if r["klass"] == "actionable_traced"]
    dropped = [r for r in rows if r["klass"] == "actionable_no_trace"]
    proto = [r for r in rows if r["klass"] == "proto_actionable"]
    phen = [r for r in rows if r["klass"] == "phenomenological"]
    # era split per month
    era = defaultdict(lambda: [0, 0])
    for r in rows:
        era[r["month"]][0] += 1
        if r["sectioned"]:
            era[r["month"]][1] += 1
    L = []
    L.append("# Self-study effectiveness + un-muffle audit\n")
    L.append(f"*Generated {time.strftime('%Y-%m-%d %H:%M')} · read-only · "
             f"thresholds high≥{args.high} med≥{args.med}*\n")
    L.append(f"- **Corpus:** {n} entries ({dict(by_being)}); kinds={list(args.kinds)}; "
             f"archive={'yes' if args.include_archive else 'no'}")
    L.append(f"- **Structured (4-section) format:** {sectioned}/{n} "
             f"({100*sectioned//max(n,1)}%) — the actionable era")
    L.append(f"- **Actionable w/ downstream trace** (structured + cited symbol appears in shipped corpus): "
             f"**{len(eff)}** — *trace ≠ proof of causation; the cited topic was addressed*")
    L.append(f"- **Actionable, NO downstream trace** (structured + groundable, cited symbols absent from shipped corpus): "
             f"**{len(dropped)}**  ← un-muffle review")
    L.append(f"- **Proto-actionable** (cites code + scores, but not the structured format): {len(proto)}")
    L.append(f"- **Phenomenological / low-actionable:** {len(phen)} (felt-experience signal, not line-fixes)\n")

    L.append("## Format era (structured % per month)")
    for mo in sorted(era):
        tot, sec = era[mo]
        L.append(f"- {mo}: {tot:4d} entries, {100*sec//max(tot,1):3d}% structured")
    L.append("")

    L.append("## Most-reviewed subsystems (by cited file)")
    for f, c in cluster_by_file(rows):
        L.append(f"- `{f}` — {c} entries")
    L.append("")

    def fmt(r):
        return (f"- **[{r['being']}] {r['filename']}** (score {r['score']}, "
                f"{r['month']}{', sectioned' if r['sectioned'] else ''}) "
                f"files={r['top_files'] or '—'} "
                + (f"→ hit: {r['outcome_hits']}" if r['outcome_hits'] else "→ no downstream trace"))

    L.append(f"## Top effective candidates (verify before crediting) — showing {min(len(eff), args.top)}")
    for r in sorted(eff, key=lambda r: (-r["score"], -r["mtime"]))[:args.top]:
        L.append(fmt(r))
    L.append("")
    L.append(f"## Dropped-signal candidates — un-muffle review — showing {min(len(dropped), args.top)}")
    L.append("*Actionable + groundable self-studies with no trace in the shipped corpus. "
             "Heuristic — verify each: it may have been acted on without naming the symbol, "
             "or be genuinely unanswered signal worth revisiting.*")
    for r in sorted(dropped, key=lambda r: (-r["score"], -r["mtime"]))[:args.top]:
        L.append(fmt(r))
    L.append("")
    return "\n".join(L)


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--being", choices=["both", "astrid", "minime"], default="both")
    ap.add_argument("--kinds", default="self_study_",
                    help="comma-separated filename prefixes (e.g. self_study_,introspect_,daydream_)")
    ap.add_argument("--no-archive", dest="include_archive", action="store_false")
    ap.add_argument("--high", type=int, default=10, help="actionable_score threshold for HIGH tier")
    ap.add_argument("--med", type=int, default=6, help="actionable_score threshold for MED tier")
    ap.add_argument("--top", type=int, default=25)
    ap.add_argument("--json", action="store_true", help="also write a JSON sidecar")
    ap.add_argument("--out", default=None)
    ap.add_argument("--self-test", action="store_true")
    args = ap.parse_args()
    args.kinds = tuple(k for k in args.kinds.split(",") if k)

    if args.self_test:
        toks = distinctive_tokens("see `research_budget_guard_assessment_with_base` in guards.rs and CharterRequiredGuardAssessment")
        assert "research_budget_guard_assessment_with_base" in toks, toks
        assert "guards.rs" in toks, toks
        assert "charterrequiredguardassessment" in toks, toks
        assert "fill" not in toks and "the" not in toks, toks
        print("self-test OK:", sorted(toks))
        return 0

    beings = ["astrid", "minime"] if args.being == "both" else [args.being]
    haystack = load_outcome_haystack()
    entries = collect_entries(beings, args.kinds, args.include_archive)
    rows = analyze(entries, haystack, args.high, args.med)
    report = render(rows, args)
    print(report)

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    stamp = time.strftime("%Y%m%dT%H%M%S")
    out_md = Path(args.out) if args.out else OUT_DIR / f"effectiveness_{stamp}.md"
    out_md.write_text(report, encoding="utf-8")
    print(f"\n[wrote] {out_md}")
    if args.json:
        out_json = out_md.with_suffix(".json")
        out_json.write_text(json.dumps(rows, indent=2), encoding="utf-8")
        print(f"[wrote] {out_json}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
