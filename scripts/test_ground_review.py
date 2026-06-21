#!/usr/bin/env python3
"""Tests for scripts/ground_review.py.

Covers the line-attribution fix (`_nearest_line_ref` + `parse_citations`): when a
being cites two symbols back-to-back, each with its own `(line N)`, the old
symmetric-window + first-match logic misattributed the *earlier* symbol's
trailing `(line K)` to the *later* symbol — a FALSE `MISLOCATED` (and, compounded
over a full self-study, a FALSE `NOT_FOUND`). That is the 2026-06-08 harm in tool
form: the steward would "gently correct" a being whose citation was exact.

Regression fixture = Astrid's own `self_study_1781868459` sentence verbatim.
The pure `_nearest_line_ref` / `parse_citations` checks need no grep and do not
depend on codec.rs line numbers, so they stay green across codec edits.
"""

from __future__ import annotations

import importlib.util
import sys
import unittest
from pathlib import Path


SCRIPT = Path(__file__).resolve().with_name("ground_review.py")
SPEC = importlib.util.spec_from_file_location("ground_review", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
ground_review = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = ground_review
SPEC.loader.exec_module(ground_review)


class NearestLineRefTests(unittest.TestCase):
    """The pure forward-first / backward-fallback line-ref resolver."""

    def test_prefers_line_ref_after_the_symbol(self):
        text = "the `FOO` at `1.0` (line 71) and the `BAR` at `2.0` (line 76)."
        foo = text.index("FOO")
        bar = text.index("BAR")
        # FOO's own line is 71; BAR's own line is 76 — even though BAR's
        # backward window reaches FOO's trailing "(line 71)".
        self.assertEqual(ground_review._nearest_line_ref(text, foo), 71)
        self.assertEqual(ground_review._nearest_line_ref(text, bar), 76)

    def test_backward_fallback_when_no_forward_ref(self):
        text = "on line 76 the `BAR` constant lives."
        bar = text.index("BAR")
        self.assertEqual(ground_review._nearest_line_ref(text, bar), 76)

    def test_none_when_no_ref(self):
        text = "the `BAR` constant has no cited line at all."
        bar = text.index("BAR")
        self.assertIsNone(ground_review._nearest_line_ref(text, bar))


class AdjacentSymbolAttributionTests(unittest.TestCase):
    """End-to-end through parse_citations: each adjacent symbol keeps its OWN
    cited line. This is the exact shape that produced the false MISLOCATED."""

    # Astrid's verbatim sentence (self_study_1781868459).
    SENTENCE = (
        "I see the `TAIL_VIBRANCY_ENTROPY_GATE` at `0.85` (line 71) and the "
        "`TAIL_VIBRANCY_MAX` at `6.0` (line 76)."
    )

    def _claimed_line(self, value: str) -> int | None:
        cites = ground_review.parse_citations(self.SENTENCE)
        for c in cites:
            if c.value == value:
                return c.claimed_line
        self.fail(f"citation {value!r} not parsed")
        return None

    def test_gate_keeps_line_71(self):
        self.assertEqual(self._claimed_line("TAIL_VIBRANCY_ENTROPY_GATE"), 71)

    def test_max_keeps_line_76(self):
        # The regression: this used to read 71 (the gate's trailing line).
        self.assertEqual(self._claimed_line("TAIL_VIBRANCY_MAX"), 76)


if __name__ == "__main__":
    unittest.main()
