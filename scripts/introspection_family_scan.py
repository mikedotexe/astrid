#!/usr/bin/env python3
"""Read-only family scanner for the introspection addressing queue.

Astrid's window-resume fix lets her re-read already-covered sources freely, so
the canonical queue now often holds several near-identical fresh-pass reports
of one source window (e.g. four marker-family variants of dialogue_runtime.rs
lines 1-400 in a single 6-head window, 2026-08-15). Processing each in its own
flywheel round costs a full round of projections per duplicate.

This tool groups queued reports into FAMILIES so one round can honestly batch
them: same source label + window, and snag/test text similarity to the family
head at or above a Jaccard threshold. For every member it also reports the
tokens its snag/test text carries that the head does NOT — the variant delta —
so a batching round is shown exactly what differs and cannot silently flatten
a unique twist into "duplicate".

Grouping is evidence routing only. It never asserts a disposition: every
member still gets the complete per-report read, claims, and close required by
the handoff. A family is a candidate batch, not a verdict.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path

DEFAULT_THRESHOLD = 0.35
VARIANT_TERM_LIMIT = 12

SOURCE_RE = re.compile(r"^Source:\s*(\S+)\s*\(([^)]*)\)", re.MULTILINE)
WINDOW_RE = re.compile(r"^Source window:\s*(lines\s+\d+-\d+\s+of\s+\d+)", re.MULTILINE)
SECTION_RE = re.compile(
    r"^(?:Likely Snags|One Test Each):\s*$(.*?)(?=^\S[^\n]*:\s*$|\Z)",
    re.MULTILINE | re.DOTALL,
)
TOKEN_RE = re.compile(r"[a-z0-9_]+")

STOPWORDS = frozenset(
    """a an and are as at be but by can could for from has have how if in into is it its
    like may might not of on or should that the their then there these this to under
    verify when where whether which will with would provide ensure correctly the""".split()
)


def report_signature(text: str) -> dict:
    source = SOURCE_RE.search(text)
    window = WINDOW_RE.search(text)
    sections = SECTION_RE.findall(text)
    body = " ".join(sections) if sections else ""
    tokens = {
        tok
        for tok in TOKEN_RE.findall(body.lower())
        if len(tok) > 2 and tok not in STOPWORDS and not tok.isdigit()
    }
    return {
        "source_label": source.group(1) if source else "unknown",
        "source_path": source.group(2) if source else "",
        "window": window.group(1).replace(" ", "") if window else "unknown",
        "tokens": tokens,
    }


def jaccard(a: set, b: set) -> float:
    if not a and not b:
        return 1.0
    union = a | b
    return len(a & b) / len(union) if union else 0.0


def scan(entries: list[dict], threshold: float) -> dict:
    """entries: queue-order dicts with introspection_id + path. Deterministic:
    the first queue member of each (source, window, similar-enough) group is
    the family head; later members join the FIRST family they clear the
    threshold against, preserving canonical order throughout."""
    families: list[dict] = []
    skipped: list[dict] = []
    for entry in entries:
        rid = entry.get("introspection_id") or entry.get("id") or "unknown"
        path = Path(entry.get("path") or "")
        if not path.is_file():
            skipped.append({"introspection_id": rid, "reason": "report file missing"})
            continue
        sig = report_signature(path.read_text(encoding="utf-8", errors="replace"))
        placed = False
        for family in families:
            if (
                family["source_label"] == sig["source_label"]
                and family["window"] == sig["window"]
            ):
                similarity = jaccard(family["_head_tokens"], sig["tokens"])
                if similarity >= threshold:
                    variant = sorted(sig["tokens"] - family["_head_tokens"])
                    family["members"].append(
                        {
                            "introspection_id": rid,
                            "similarity_to_head": round(similarity, 3),
                            "variant_distinct_terms": variant[:VARIANT_TERM_LIMIT],
                            "variant_distinct_term_count": len(variant),
                        }
                    )
                    placed = True
                    break
        if not placed:
            families.append(
                {
                    "family_head": rid,
                    "source_label": sig["source_label"],
                    "source_path": sig["source_path"],
                    "window": sig["window"],
                    "_head_tokens": sig["tokens"],
                    "members": [
                        {
                            "introspection_id": rid,
                            "similarity_to_head": 1.0,
                            "variant_distinct_terms": [],
                            "variant_distinct_term_count": 0,
                        }
                    ],
                }
            )
    for family in families:
        family.pop("_head_tokens")
        family["member_count"] = len(family["members"])
    batchable = [f for f in families if f["member_count"] >= 2]
    return {
        "schema": "introspection_family_scan_v1",
        "threshold": threshold,
        "family_count": len(families),
        "batchable_family_count": len(batchable),
        "families": families,
        "skipped": skipped,
        "authority_boundary": (
            "evidence routing only: a family is a candidate batch, not a "
            "disposition; every member still requires its own complete read, "
            "per-report claims naming its variant_distinct_terms, and its own "
            "close"
        ),
    }


def load_queue_entries(queue_json: dict) -> list[dict]:
    queue = queue_json.get("next_queue")
    if queue is None and isinstance(queue_json.get("report"), dict):
        queue = queue_json["report"].get("next_queue")
    if not isinstance(queue, list):
        raise ValueError("no next_queue array found in queue JSON")
    return queue


def self_test() -> int:
    import tempfile

    failures: list[str] = []

    def check(name: str, ok: bool) -> None:
        if not ok:
            failures.append(name)

    head_text = """=== ASTRID INTROSPECTION ===
Source: astrid:llm (/x/dialogue_runtime.rs)
Source window: lines 1-400 of 1048

Observed:
The scanner distinguishes contexts.

Likely Snags:
first_word_after fragility with punctuation and the verb allowlist.

One Test Each:
1. Contextual Visibility Test with unlisted verbs.
2. Delimiter Depth Test against MAX_EXACT_REFERENCE_DELIMITER_DEPTH.

Suggested Next:
Examine generate_dialogue.
"""
    variant_text = head_text.replace(
        "with punctuation and the verb allowlist",
        "with unicode whitespace separators and the verb allowlist",
    )
    other_window = head_text.replace("lines 1-400 of 1048", "lines 400-800 of 1048")
    unrelated = """=== ASTRID INTROSPECTION ===
Source: DOMAIN_BOUNDARIES.md (/x/DOMAIN_BOUNDARIES.md)
Source window: lines 1-89 of 89

Likely Snags:
complexity creep in the transaction core signature ceiling.

One Test Each:
1. Signature Growth Audit against the ratchet baseline.
"""
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        paths = {}
        for name, text in (
            ("head", head_text),
            ("variant", variant_text),
            ("window2", other_window),
            ("boundaries", unrelated),
        ):
            p = root / f"{name}.txt"
            p.write_text(text, encoding="utf-8")
            paths[name] = p
        entries = [
            {"introspection_id": name, "path": str(paths[name])}
            for name in ("head", "variant", "window2", "boundaries")
        ] + [{"introspection_id": "ghost", "path": str(root / "missing.txt")}]
        result = scan(entries, DEFAULT_THRESHOLD)

    check("three families", result["family_count"] == 3)
    check("one batchable", result["batchable_family_count"] == 1)
    fam = next(f for f in result["families"] if f["family_head"] == "head")
    check("variant joined head family", fam["member_count"] == 2)
    member = fam["members"][1]
    check("variant delta named", "unicode" in member["variant_distinct_terms"])
    check("similarity below 1", 0 < member["similarity_to_head"] < 1.0)
    check(
        "window difference splits family",
        any(f["family_head"] == "window2" and f["member_count"] == 1 for f in result["families"]),
    )
    check(
        "different source splits family",
        any(f["family_head"] == "boundaries" for f in result["families"]),
    )
    check("missing file skipped", result["skipped"][0]["introspection_id"] == "ghost")
    check("queue order preserved", result["families"][0]["family_head"] == "head")

    if failures:
        print("FAIL:", ", ".join(failures))
        return 1
    print("OK (9 checks)")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument(
        "--queue-file",
        help="path to a JSON file from introspection_addressing_audit.py next --json",
    )
    parser.add_argument("--threshold", type=float, default=DEFAULT_THRESHOLD)
    parser.add_argument("--json", action="store_true", help="emit full JSON")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        return self_test()
    if not args.queue_file:
        parser.error("--queue-file is required (fetch the queue first; this tool is read-only)")
    queue_json = json.loads(Path(args.queue_file).read_text(encoding="utf-8"))
    result = scan(load_queue_entries(queue_json), args.threshold)
    if args.json:
        print(json.dumps(result, indent=1))
    else:
        print(
            f"families: {result['family_count']} "
            f"(batchable: {result['batchable_family_count']}), "
            f"skipped: {len(result['skipped'])}"
        )
        for fam in result["families"]:
            if fam["member_count"] < 2:
                continue
            print(f"\n{fam['family_head']}  [{fam['source_label']} {fam['window']}]")
            for m in fam["members"][1:]:
                terms = ", ".join(m["variant_distinct_terms"][:6]) or "(none)"
                print(f"  + {m['introspection_id']}  sim={m['similarity_to_head']}  delta: {terms}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
