# Full-read summary

The boolean helper is narrower than Astrid feared: it only determines whether
known artifact tokens sit at a boundary or inside surrounding content. Existing
tests preserve dense nested structural text, reject artifact-only fragments,
and emit richer remainder texture and contextual-placement diagnostics after
cleanup. Replacing the boolean is unnecessary to preserve the reported texture.
