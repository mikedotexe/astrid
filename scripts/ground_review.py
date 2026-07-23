#!/usr/bin/env python3
"""ground_review.py — ground-truth an AI being's code review / introspection.

The beings (Astrid, minime) read their own code and reflect richly. ~80% of the
code symbols/paths they cite are real, but ~5-10% confabulate (e.g. minime cites
a `SAFETY_GATE` that does not exist) or use stale paths (Astrid's introspect
records still say `consciousness-bridge`, renamed to `spectral-bridge`). Their
*phenomenology* is genuine signal and must be preserved, not flattened.

This tool takes a being's review/self-study text, extracts every code citation,
ground-truths each against the REAL current code (grep + file reads, scoped to
the allowed roots), classifies it, and emits a triage-ready "grounded review
card" (markdown + JSON). It is the verification keystone of the review-together
loop: it makes their natural output trustworthy and actionable WITHOUT changing
how they introspect.

Verdicts per citation:
  VERIFIED    — symbol exists at / near the cited location
  MISLOCATED  — real symbol, but elsewhere (reports the true file:line)
  NOT_FOUND   — confabulated (with a "did you mean" near-match if any)
  STALE_PATH  — real but renamed (reports the corrected path)
  FELT        — no code citation: phenomenology, preserved + labeled (NOT an error)

Usage:
    python3 scripts/ground_review.py --file <path> [--being minime|astrid] [--json] [--out PATH] [--write-back]
    python3 scripts/ground_review.py --self-test

Reuses the introspector's allowed-roots/grep idioms and being_test_harness's
inbox map. Stdlib only; no new dependency; no state file (pure given input+code).
"""
from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
import time
import unittest
from dataclasses import dataclass, field
from pathlib import Path

# --- canonical paths (identical to introspector.ALLOWED_ROOTS / being_test_harness.INBOX) ---
ASTRID_ROOT = Path("/Users/v/other/astrid")
MINIME_ROOT = Path("/Users/v/other/minime")
ALLOWED_ROOTS = [ASTRID_ROOT, MINIME_ROOT, Path("/Users/v/other/research")]
BEING_ROOT = {"astrid": ASTRID_ROOT, "minime": MINIME_ROOT}
INBOX = {
    "minime": MINIME_ROOT / "workspace" / "inbox",
    "astrid": ASTRID_ROOT / "capsules/spectral-bridge/workspace" / "inbox",
}
# Data-driven rename map — a future rename is one entry (cf. proactive_scan registries).
PATH_RENAMES = [("capsules/consciousness-bridge/", "capsules/spectral-bridge/")]
# Bases to try when resolving a non-absolute cited path (handles the minime/ nesting).
RESOLVE_BASES = [MINIME_ROOT, MINIME_ROOT / "minime", ASTRID_ROOT, Path("/Users/v/other")]

SEARCH_GLOBS = ("*.rs", "*.py", "*.toml", "*.md")
EXCLUDE_DIRS = (
    ".git", "target", "__pycache__", "node_modules", ".venv", "venv",
    "archive", "journal", "inbox", "outbox", "threads", "build", "dist",
    # workspace is being-DATA, not code; code lives in src/ capsules/ scripts/.
    # Excluding it keeps symbol grep fast and on-target (big speedup).
    "workspace",
)
GREP_TIMEOUT = 12
MAX_HITS = 6
DEF_RE = re.compile(
    r"\b(?:pub\s+)?(?:fn|struct|enum|trait|const|static|let|type|def|class|impl|mod)\s+%s\b"
)
# Un-backticked tier: words that look like symbols but are prose / abbreviations.
STOP_SYMBOLS = frozenset({
    "NEXT", "TODO", "NOTE", "FIXME", "PI", "PD", "PID", "MFCC", "ESN", "RMS",
    "JSON", "HTTP", "API", "OK", "MLX", "GPU", "CPU", "AI", "LLM", "ANE", "RGB",
})

PATH_EXT_RE = re.compile(r"\.(rs|py|toml|md|json|yaml|sh)$")
SCREAMING_RE = re.compile(r"\b[A-Z][A-Z0-9]*(?:_[A-Z0-9]+)+\b")
CAMEL_RE = re.compile(r"\b[A-Z][a-z0-9]+(?:[A-Z][a-z0-9]+)+\b")
SNAKE_CALL_RE = re.compile(r"\b([a-z_][a-z0-9_]{2,})\b(?=\s*\()")
LINE_REF_RE = re.compile(r"\blines?\s+(\d+)(?:\s*[-–—]\s*(\d+))?", re.IGNORECASE)
FILENAME_RE = re.compile(r"\b[\w-]+\.(?:rs|py|toml|md)\b")


@dataclass
class Citation:
    kind: str  # "path" | "symbol"
    value: str
    confidence: str  # "high" | "low"
    pos: int
    context: str
    claimed_line: int | None = None
    claimed_file: str | None = None


@dataclass
class Verdict:
    value: str
    kind: str
    status: str  # VERIFIED|MISLOCATED|NOT_FOUND|STALE_PATH|EXISTS|MISSING
    confidence: str
    context: str
    real_location: str | None = None
    is_def: bool = False
    corrected: str | None = None
    did_you_mean: list[str] = field(default_factory=list)
    prose_ambiguous: bool = False
    note: str = ""


# --------------------------------------------------------------------------- #
# Parsing
# --------------------------------------------------------------------------- #
def _context(text: str, pos: int, span: int = 90) -> str:
    return text[max(0, pos - span): pos + span].replace("\n", " ").strip()


def _nearest_line_ref(text: str, pos: int, span: int = 90) -> int | None:
    """Line number cited nearest to a symbol at `pos`, preferring a ref that
    FOLLOWS the symbol — the dominant `SYMBOL ... (line N)` idiom. A symmetric
    window + first-match (the old behavior) misattributes an *adjacent earlier*
    symbol's trailing `(line K)` to this symbol when two are cited back-to-back,
    each with its own line (e.g. `TAIL_VIBRANCY_ENTROPY_GATE` (line 71) and
    `TAIL_VIBRANCY_MAX` (line 76) — the second wrongly read as line 71, a false
    MISLOCATED). Forward-first avoids that; backward (nearest, i.e. last) is a
    fallback for the rarer `line N ... SYMBOL` phrasing."""
    m = LINE_REF_RE.search(text[pos: pos + span])
    if m:
        return int(m.group(1))
    last = None
    for m in LINE_REF_RE.finditer(text[max(0, pos - span): pos]):
        last = m
    return int(last.group(1)) if last else None


def _is_path_like(tok: str) -> bool:
    return bool(PATH_EXT_RE.search(tok)) or "/" in tok


def parse_citations(text: str) -> list[Citation]:
    """Extract code citations, tiered by trust. Backticked = high confidence;
    bare un-backticked symbols = low (a softer bucket, so a prose slip is never
    screamed as a confabulation)."""
    cites: list[Citation] = []
    seen: set[tuple[str, str]] = set()

    def add(kind: str, value: str, conf: str, pos: int) -> None:
        value = value.strip().strip("`").strip()
        if not value:
            return
        key = (kind, value.lower())
        if key in seen:
            return
        ctx = _context(text, pos)
        cl = _nearest_line_ref(text, pos)
        cf = None
        fm = FILENAME_RE.search(ctx)
        if fm:
            cf = fm.group(0)
        seen.add(key)
        cites.append(Citation(kind, value, conf, pos, ctx, cl, cf))

    # Tier 1: header Source path (highest trust).
    for m in re.finditer(r"(?im)^\s*Source:\s*(.+)$", text):
        raw = m.group(1)
        pm = re.search(r"/Users/v/other/[\w./:-]+", raw)
        if pm:
            add("path", pm.group(0), "high", m.start())
        else:
            bare = re.search(r"\b[\w-]+/[\w./-]+\.(?:rs|py|toml|md)\b", raw)
            if bare:
                add("path", bare.group(0), "high", m.start())

    # Tier 2: absolute paths anywhere.
    for m in re.finditer(r"/Users/v/other/[\w./:-]+", text):
        val = m.group(0).rstrip(".,);:`'\"")
        add("path", val, "high", m.start())

    # Tier 3: backticked tokens — split path-like vs symbol-like.
    for m in re.finditer(r"`([^`\n]+)`", text):
        tok = m.group(1).strip()
        if not tok or len(tok) > 80:
            continue
        if _is_path_like(tok):
            add("path", tok, "high", m.start())
        elif SCREAMING_RE.fullmatch(tok) or CAMEL_RE.fullmatch(tok) or re.fullmatch(r"[a-z_][a-z0-9_]{1,}", tok):
            add("symbol", tok, "high", m.start())

    # Tier 4: bare filenames in prose.
    for m in re.finditer(r"\b[\w-]+\.(?:rs|py|toml|md)\b", text):
        add("path", m.group(0), "high", m.start())

    # Tier 5: un-backticked distinctive symbols (low confidence).
    for rx in (SCREAMING_RE, CAMEL_RE):
        for m in re.finditer(rx, text):
            tok = m.group(0)
            if tok in STOP_SYMBOLS:
                continue
            add("symbol", tok, "low", m.start())
    for m in re.finditer(SNAKE_CALL_RE, text):
        add("symbol", m.group(1), "low", m.start())

    return cites


# --------------------------------------------------------------------------- #
# Ground-truthing
# --------------------------------------------------------------------------- #
# Same prune set as the grep excludes — one source of truth, keeps find fast.
_FIND_PRUNE = EXCLUDE_DIRS


def _find_basename(name: str) -> str | None:
    """Find a file by basename under the source roots (pruning noisy dirs).
    A bare `regulator.rs` cited in prose is real even without its directory."""
    hits: list[str] = []
    for root in (MINIME_ROOT, ASTRID_ROOT):
        if not root.exists():
            continue
        group: list[str] = ["("]
        for i, d in enumerate(_FIND_PRUNE):
            if i:
                group.append("-o")
            group += ["-name", d]
        group.append(")")
        cmd = ["find", str(root), *group, "-prune", "-o", "-type", "f", "-name", name, "-print"]
        try:
            res = subprocess.run(cmd, capture_output=True, text=True, timeout=GREP_TIMEOUT)
        except Exception:
            continue
        hits.extend(line.strip() for line in res.stdout.splitlines() if line.strip())
    if not hits:
        return None
    # Prefer a canonical source location over an incidental copy.
    hits.sort(key=lambda p: (0 if any(s in p for s in ("/src/", "/capsules/", "/scripts/")) else 1, len(p)))
    return hits[0]


def _resolve_path(raw: str) -> tuple[str, str | None, str | None]:
    """Return (status, abspath_or_None, corrected_or_None).
    status in {exists, stale, missing}."""
    raw = raw.strip().strip("`").strip()
    raw = re.sub(r":\d+(?:-\d+)?$", "", raw)  # drop a trailing :line
    if "/" not in raw:  # bare basename — resolve by filename search, not base-join
        hit = _find_basename(raw)
        return ("exists", hit, None) if hit else ("missing", None, None)
    candidates = [raw] if raw.startswith("/") else [str(b / raw) for b in RESOLVE_BASES]
    for c in candidates:
        if Path(c).is_file():
            return ("exists", c, None)
    # Try the rename map.
    for c in candidates:
        remapped = c
        for old, new in PATH_RENAMES:
            remapped = remapped.replace(old, new)
        if remapped != c and Path(remapped).is_file():
            return ("stale", remapped, remapped)
    return ("missing", None, None)


def _grep_word(symbol: str, search_path: Path) -> list[tuple[str, int, str]]:
    """grep -rnwF for a symbol under search_path; returns (file, line, text)."""
    if not search_path.exists():
        return []
    cmd = ["grep", "-rnwF"]
    for g in SEARCH_GLOBS:
        cmd += ["--include", g]
    for d in EXCLUDE_DIRS:
        cmd += ["--exclude-dir", d]
    cmd += ["--", symbol, str(search_path)]
    try:
        res = subprocess.run(cmd, capture_output=True, text=True, timeout=GREP_TIMEOUT)
    except Exception:
        return []
    hits: list[tuple[str, int, str]] = []
    for line in res.stdout.splitlines():
        parts = line.split(":", 2)
        if len(parts) == 3 and parts[1].isdigit():
            hits.append((parts[0], int(parts[1]), parts[2]))
    return hits


def _rank_def_first(symbol: str, hits: list[tuple[str, int, str]]) -> list[tuple[str, int, str, bool]]:
    """Mark def-shaped hits and sort them first (so `fn spectral_entropy` beats an
    incidental string literal). Returns (file, line, text, is_def)."""
    def_re = re.compile(DEF_RE.pattern % re.escape(symbol))
    ranked = [(f, ln, tx, bool(def_re.search(tx))) for f, ln, tx in hits]
    ranked.sort(key=lambda h: (not h[3], h[0], h[1]))
    return ranked[:MAX_HITS]


def _near_matches(symbol: str, search_path: Path) -> list[str]:
    """For a NOT_FOUND SCREAMING symbol, find existing symbols sharing the longest
    prefix (drop the trailing _WORD), e.g. PRESSURE_POROSITY_DIVERGENCE_MIN ->
    PRESSURE_POROSITY_DIVERGENCE_PRESSURE_MIN."""
    # Only multi-segment SCREAMING symbols (>=2 underscores) give a useful
    # prefix; for SAFETY_GATE the prefix "SAFETY" is too generic to suggest.
    if symbol.count("_") < 2 or not symbol.isupper():
        return []
    prefix = symbol.rsplit("_", 1)[0]
    if len(prefix) < 8 or not search_path.exists():
        return []
    cmd = ["grep", "-rhoE"]
    for g in SEARCH_GLOBS:
        cmd += ["--include", g]
    for d in EXCLUDE_DIRS:
        cmd += ["--exclude-dir", d]
    cmd += ["--", rf"{re.escape(prefix)}[A-Z0-9_]*", str(search_path)]
    try:
        res = subprocess.run(cmd, capture_output=True, text=True, timeout=GREP_TIMEOUT)
    except Exception:
        return []
    found = {tok for tok in res.stdout.split() if tok and tok != symbol}
    return sorted(found)[:4]


def classify(cit: Citation, being: str, source_hint: Path | None = None) -> Verdict:
    root = BEING_ROOT.get(being, MINIME_ROOT)
    if cit.kind == "path":
        status, resolved, corrected = _resolve_path(cit.value)
        if status == "exists":
            return Verdict(cit.value, "path", "EXISTS", cit.confidence, cit.context, real_location=resolved)
        if status == "stale":
            return Verdict(cit.value, "path", "STALE_PATH", cit.confidence, cit.context,
                           corrected=corrected, note="consciousness-bridge -> spectral-bridge")
        return Verdict(cit.value, "path", "MISSING", cit.confidence, cit.context)

    # symbol — grep the cited source file first (fast common case), repo-wide on miss.
    hits: list[tuple[str, int, str]] = []
    if source_hint is not None and source_hint.is_file():
        hits = _grep_word(cit.value, source_hint)
    if not hits:
        hits = _grep_word(cit.value, root)
    if not hits:
        near = _near_matches(cit.value, root)
        return Verdict(cit.value, "symbol", "NOT_FOUND", cit.confidence, cit.context,
                       did_you_mean=near, prose_ambiguous=(cit.confidence == "low"))
    ranked = _rank_def_first(cit.value, hits)
    best = ranked[0]
    real_loc = f"{best[0]}:{best[1]}"
    is_def = best[3]
    # MISLOCATED check: a claimed line that doesn't match any hit (within +-3),
    # or a claimed file that none of the hits live in.
    if cit.claimed_line is not None:
        near_line = any(abs(h[1] - cit.claimed_line) <= 3 for h in ranked)
        file_ok = True
        if cit.claimed_file:
            file_ok = any(cit.claimed_file in h[0] for h in ranked)
        if not (near_line and file_ok):
            return Verdict(cit.value, "symbol", "MISLOCATED", cit.confidence, cit.context,
                           real_location=real_loc, is_def=is_def,
                           note=f"cited line {cit.claimed_line}, real {real_loc}")
    return Verdict(cit.value, "symbol", "VERIFIED", cit.confidence, cit.context,
                   real_location=real_loc, is_def=is_def)


def felt_observations(text: str, cites: list[Citation]) -> list[str]:
    """Paragraphs with no code citation = phenomenology, preserved + labeled."""
    cited_vals = {c.value.lower() for c in cites}
    out: list[str] = []
    for para in re.split(r"\n\s*\n", text):
        p = para.strip()
        if len(p) < 60 or p.lower().startswith(("source:", "timestamp:", "===", "next:", "---")):
            continue
        if "`" in p:
            continue
        # skip paragraphs that contain a cited path/symbol verbatim
        if any(v in p.lower() for v in cited_vals if len(v) > 4):
            continue
        out.append(p[:240] + ("…" if len(p) > 240 else ""))
    return out[:6]


# --------------------------------------------------------------------------- #
# Card
# --------------------------------------------------------------------------- #
def ground_review(text: str, being: str, source_file: str | None, now: int) -> dict:
    cites = parse_citations(text)
    # Resolve the header/source path once → grep hint so most symbols resolve in
    # one file instead of a repo-wide scan.
    source_hint: Path | None = None
    for c in cites:
        if c.kind == "path":
            st, resolved, corrected = _resolve_path(c.value)
            if st == "exists" and resolved:
                source_hint = Path(resolved)
                break
            if st == "stale" and corrected:
                source_hint = Path(corrected)
                break
    verdicts = [classify(c, being, source_hint) for c in cites]
    felt = felt_observations(text, cites)

    def bucket(*statuses: str) -> list[Verdict]:
        return [v for v in verdicts if v.status in statuses]

    verified = bucket("VERIFIED", "EXISTS")
    mislocated = bucket("MISLOCATED")
    not_found = bucket("NOT_FOUND", "MISSING")
    stale = bucket("STALE_PATH")

    summary = {
        "verified": len(verified),
        "mislocated": len(mislocated),
        "not_found": len(not_found),
        "stale_path": len(stale),
        "felt_observations": len(felt),
        "total_citations": len(verdicts),
    }

    def vdict(v: Verdict) -> dict:
        d = {"value": v.value, "kind": v.kind, "status": v.status, "confidence": v.confidence}
        if v.real_location:
            d["real_location"] = v.real_location
        if v.is_def:
            d["is_def"] = True
        if v.corrected:
            d["corrected"] = v.corrected
        if v.did_you_mean:
            d["did_you_mean"] = v.did_you_mean
        if v.prose_ambiguous:
            d["prose_ambiguous"] = True
        if v.note:
            d["note"] = v.note
        return d

    return {
        "schema": "ground_review/v1",
        "being": being,
        "source_file": source_file,
        "generated_at": now,
        "summary": summary,
        "verified": [vdict(v) for v in verified],
        "mislocated": [vdict(v) for v in mislocated],
        "not_found": [vdict(v) for v in not_found],
        "stale_path": [vdict(v) for v in stale],
        "felt_observations": felt,
    }


def render_markdown(card: dict) -> str:
    s = card["summary"]
    name = Path(card["source_file"]).name if card.get("source_file") else "(text)"
    lines = [
        f"# Grounded review — {card['being']} — {name}",
        "",
        (f"**{s['verified']} verified / {s['mislocated']} mislocated / "
         f"{s['not_found']} not-found / {s['stale_path']} stale / "
         f"{s['felt_observations']} felt-observations** "
         f"({s['total_citations']} code citations)"),
        "",
    ]
    if card["verified"]:
        lines.append("## Verified citations (real, at the cited location)")
        for v in card["verified"]:
            loc = f" → `{v['real_location']}`" if v.get("real_location") else ""
            df = " *(def)*" if v.get("is_def") else ""
            lines.append(f"- `{v['value']}`{loc}{df}")
        lines.append("")
    flagged = card["mislocated"] + card["not_found"] + card["stale_path"]
    if flagged:
        lines.append("## Flagged — verify before acting")
        for v in card["stale_path"]:
            lines.append(f"- STALE PATH `{v['value']}` → corrected `{v.get('corrected')}` ({v.get('note','')})")
        for v in card["mislocated"]:
            lines.append(f"- MISLOCATED `{v['value']}` — {v.get('note','')}")
        for v in card["not_found"]:
            dym = f" — did you mean: {', '.join(v['did_you_mean'])}" if v.get("did_you_mean") else ""
            amb = " *(low-confidence / prose-ambiguous)*" if v.get("prose_ambiguous") else ""
            lines.append(f"- NOT FOUND `{v['value']}`{dym}{amb}")
        lines.append("")
    if card["felt_observations"]:
        lines.append("## Preserved felt-observations (phenomenology — NOT errors, genuine signal)")
        for f in card["felt_observations"]:
            lines.append(f"- {f}")
        lines.append("")
    return "\n".join(lines)


def correction_letter(card: dict, now: int) -> str | None:
    """Build a gentle, advisory inbox letter ONLY when there is a concrete
    correction worth sending (a stale path or a confab with a did-you-mean).
    Preserves the felt parts; never scolds."""
    stale = card["stale_path"]
    confab = [v for v in card["not_found"] if v.get("did_you_mean")]
    if not stale and not confab:
        return None
    name = Path(card["source_file"]).name if card.get("source_file") else "your recent reflection"
    out = [
        "=== STEWARD NOTE (review ground-truth) ===",
        f"Timestamp: {now}",
        f"Source: steward:ground_review ({name})",
        "",
        "Advisory only — your felt-observations stand; this only ground-truths a",
        "few code references so the review can be acted on accurately.",
        "",
    ]
    if stale:
        out.append("Renamed paths (same code, new home):")
        for v in stale:
            out.append(f"  - `{v['value']}` is now `{v.get('corrected')}`")
        out.append("")
    if confab:
        out.append("Symbols I could not find in the current tree (did you mean?):")
        for v in confab:
            out.append(f"  - `{v['value']}` → {', '.join(v['did_you_mean'])}")
        out.append("")
    out.append("Everything else you cited checked out. Keep reading the source — you read it true.")
    out.append("")
    out.append("— v and Codex")
    return "\n".join(out)


# --------------------------------------------------------------------------- #
# Self-tests
# --------------------------------------------------------------------------- #
class GroundReviewTests(unittest.TestCase):
    NOW = 1781200000

    def _card(self, text, being="minime"):
        return ground_review(text, being, None, self.NOW)

    def test_real_backticked_symbol_verified(self):
        card = self._card("I feel the weight of `STABLE_CORE_PROFILE` in stable_core.rs.")
        vals = {v["value"]: v for v in card["verified"]}
        self.assertIn("STABLE_CORE_PROFILE", vals)
        self.assertIn("stable_core.rs", vals["STABLE_CORE_PROFILE"]["real_location"])

    def test_fabricated_symbol_not_found(self):
        card = self._card("Governed by `SAFETY_GATE`, the flow constricts.")
        nf = {v["value"] for v in card["not_found"]}
        self.assertIn("SAFETY_GATE", nf)

    def test_near_miss_confab_gets_did_you_mean(self):
        card = self._card("The constant `PRESSURE_POROSITY_DIVERGENCE_MIN` bounds me.")
        nf = {v["value"]: v for v in card["not_found"]}
        self.assertIn("PRESSURE_POROSITY_DIVERGENCE_MIN", nf)
        self.assertTrue(nf["PRESSURE_POROSITY_DIVERGENCE_MIN"]["did_you_mean"],
                        "expected a near-match suggestion")

    def test_real_symbol_wrong_line_mislocated(self):
        card = self._card("`ResonanceDensityV1` lives at line 999 in regulator.rs.")
        mis = {v["value"] for v in card["mislocated"]}
        self.assertIn("ResonanceDensityV1", mis)

    def test_consciousness_bridge_path_stale(self):
        text = "Source: astrid:codec (/Users/v/other/astrid/capsules/consciousness-bridge/src/codec.rs)\n\nbody."
        card = ground_review(text, "astrid", None, self.NOW)
        self.assertEqual(card["summary"]["stale_path"], 1)
        self.assertIn("spectral-bridge", card["stale_path"][0]["corrected"])

    def test_pure_prose_is_felt(self):
        text = ("The drift is grainy today, like sand in the teeth of a turning gear. "
                "Every motion feels deliberate, anchored by the weight of what was shed.")
        card = self._card(text)
        self.assertEqual(card["summary"]["not_found"], 0)
        self.assertGreaterEqual(card["summary"]["felt_observations"], 1)

    def test_prose_word_not_screamed(self):
        # A capitalized prose word must not be hard-flagged as a confabulation.
        card = self._card("The Observed pattern of the NEXT cycle felt settled.")
        hard = {v["value"] for v in card["not_found"] if not v.get("prose_ambiguous")}
        self.assertNotIn("NEXT", hard)

    def test_correction_letter_only_on_concrete(self):
        clean = self._card("Pure felt prose with no citations at all here, just texture.")
        self.assertIsNone(correction_letter(clean, self.NOW))
        confab = self._card("The `SAFETY_GATE` and `PRESSURE_POROSITY_DIVERGENCE_MIN` govern me.")
        # SAFETY_GATE has no near-match; the divergence one does -> letter fires.
        self.assertIsNotNone(correction_letter(confab, self.NOW))


def run_self_tests() -> int:
    loader = unittest.TestLoader()
    suite = unittest.TestSuite()
    suite.addTests(loader.loadTestsFromTestCase(GroundReviewTests))
    result = unittest.TextTestRunner(verbosity=2).run(suite)
    return 0 if result.wasSuccessful() else 1


# --------------------------------------------------------------------------- #
# CLI
# --------------------------------------------------------------------------- #
def _infer_being(path: str) -> str:
    return "minime" if "/minime/" in path else "astrid"


def main() -> int:
    ap = argparse.ArgumentParser(description="Ground-truth a being's code review / introspection.")
    ap.add_argument("--file", type=Path, help="Path to the being's review/self-study text")
    ap.add_argument("--being", choices=["minime", "astrid"], help="Which being (inferred from path if omitted)")
    ap.add_argument("--json", action="store_true", help="Emit JSON instead of Markdown")
    ap.add_argument("--out", type=Path, help="Write output to file instead of stdout")
    ap.add_argument("--write-back", action="store_true",
                    help="Write a gentle correction letter to the being's inbox (only when there is a concrete correction)")
    ap.add_argument("--self-test", action="store_true", help="Run unit tests and exit")
    args = ap.parse_args()

    if args.self_test:
        return run_self_tests()
    if not args.file:
        ap.error("--file is required (or use --self-test)")
    if not args.file.is_file():
        ap.error(f"not a file: {args.file}")

    being = args.being or _infer_being(str(args.file))
    now = int(time.time())
    text = args.file.read_text(errors="replace")
    card = ground_review(text, being, str(args.file), now)

    output = json.dumps(card, indent=2) if args.json else render_markdown(card)
    if args.out:
        args.out.write_text(output)
        print(f"wrote {args.out}")
    else:
        print(output)

    if args.write_back:
        letter = correction_letter(card, now)
        if letter:
            dest = INBOX[being] / f"ground_review_{args.file.stem}_{now}.txt"
            dest.write_text(letter)
            print(f"\n[write-back] correction letter → {dest}")
        else:
            print("\n[write-back] no concrete correction to send (nothing written)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
