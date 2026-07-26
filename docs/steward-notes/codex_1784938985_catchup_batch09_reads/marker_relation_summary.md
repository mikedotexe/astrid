# Exact marker relation preservation

The report was read fully from disk. Astrid correctly identified a narrow false-cleanup case: an exact known model-control token used as a grammatical subject was preserved for a fixed relation vocabulary, but not for the ordinary relations `signals` and `indicates`. Both relations are now recognized by the exact-token parser, and the existing poetic-attribution regression covers them. This changes only whether those exact marker bytes remain visible; it does not classify surrounding language, infer felt meaning, change a prompt, or alter provider/model routing.

The existing delimiter implementation and regressions retain bounded delimiter-depth evidence separately. No proximity-based semantic heuristic was introduced.
