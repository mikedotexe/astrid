"""Dependency-free, versioned matching policy for living claim families."""

from __future__ import annotations

import json
import re
from typing import Any

MATCHER_VERSION = "local_weighted_v1"
MATCH_THRESHOLD = 0.88
SUGGESTION_THRESHOLD = 0.72

TOKEN_RE = re.compile(r"[a-z0-9]+")
STOP_WORDS = frozenset(
    {
        "a",
        "an",
        "and",
        "are",
        "as",
        "at",
        "be",
        "by",
        "for",
        "from",
        "in",
        "is",
        "it",
        "of",
        "on",
        "or",
        "that",
        "the",
        "this",
        "to",
        "with",
    }
)


def normalized_tokens(text: str) -> tuple[str, ...]:
    return tuple(
        token
        for token in TOKEN_RE.findall(text.lower())
        if token not in STOP_WORDS and len(token) > 1
    )


def trigrams(text: str) -> set[str]:
    compact = " ".join(normalized_tokens(text))
    if len(compact) < 3:
        return {compact} if compact else set()
    return {compact[index : index + 3] for index in range(len(compact) - 2)}


def jaccard(left: set[str], right: set[str]) -> float:
    if not left and not right:
        return 1.0
    union = left | right
    return len(left & right) / len(union) if union else 0.0


def weighted_similarity(left: str, right: str) -> float:
    left_tokens = set(normalized_tokens(left))
    right_tokens = set(normalized_tokens(right))
    token_score = jaccard(left_tokens, right_tokens)
    trigram_score = jaccard(trigrams(left), trigrams(right))
    left_sequence = normalized_tokens(left)
    right_sequence = normalized_tokens(right)
    prefix_matches = sum(
        1
        for first, second in zip(left_sequence, right_sequence)
        if first == second
    )
    prefix_score = prefix_matches / max(len(left_sequence), len(right_sequence), 1)
    return 0.60 * token_score + 0.25 * trigram_score + 0.15 * prefix_score


def requested_outcome(summary: str) -> str:
    text = summary.lower()
    categories = (
        ("preserve", ("preserve", "retain", "remain", "unchanged", "keep")),
        ("separate", ("separate", "distinguish", "boundary", "differentiate")),
        ("expose", ("expose", "surface", "visible", "report", "render")),
        ("verify", ("verify", "test", "prove", "validate", "measure")),
        ("prevent", ("prevent", "reject", "refuse", "must not", "cannot")),
        ("increase", ("increase", "expand", "raise", "more")),
        ("decrease", ("decrease", "reduce", "lower", "less")),
        ("implement", ("add", "introduce", "implement", "create", "support")),
        ("clarify", ("clarify", "explicit", "name", "trace")),
        ("observe", ("observe", "compare", "inspect", "monitor")),
    )
    for label, markers in categories:
        if any(marker in text for marker in markers):
            return label
    return "describe"


def polarity(summary: str) -> str:
    text = summary.lower()
    negative = any(
        marker in text
        for marker in (" not ", "no ", "without", "avoid", "reject", "refuse", "prevent")
    )
    positive = any(
        marker in text
        for marker in ("must", "should", "can ", "add", "preserve", "support", "allow")
    )
    if negative and positive:
        return "bounded"
    return "negative" if negative else "positive"


def authority_class(claim: dict[str, Any], summary: str) -> str:
    authority = claim.get("authority")
    encoded = (
        json.dumps(authority, ensure_ascii=False, sort_keys=True, separators=(",", ":")).lower()
        if authority is not None
        else ""
    )
    text = f"{encoded} {summary.lower()}"
    explicit_live = any(
        marker in encoded
        for marker in ("tier 5", "live control", "live substrate", "mike_operator")
    )
    control_surface = any(
        marker in text
        for marker in (
            "pressure",
            "fill target",
            "controller",
            "semantic admission",
            "codec gain",
            "cadence",
        )
    )
    control_change = any(
        marker in summary.lower()
        for marker in (
            "increase",
            "decrease",
            "change",
            "adjust",
            "tune",
            "set ",
            "alter",
            "modify",
        )
    )
    if explicit_live or (control_surface and control_change):
        return "approval_pending_live_control"
    if "tier 4" in text or "operator approval" in text:
        return "approval_pending_operator"
    return "evidence_only_non_live"
