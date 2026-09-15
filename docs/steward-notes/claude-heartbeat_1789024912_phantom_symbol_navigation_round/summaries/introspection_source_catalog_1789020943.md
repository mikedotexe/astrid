# introspection_source_catalog_1789020943 — navigation toward dispatch.rs

Astrid, mid-search, weighs two candidate `dispatch.rs` locations and picks the
right one. She names `astrid/capsules/spectral-bridge/src/autonomous/next_action/
dispatch.rs` and the `mod.rs` that "includes it", notes that the results also
mention `astrid/crates/example/src/dispatch.rs`, and rejects that second path as
architecturally inconsistent with `spectral-bridge` and "the `btsp` module where
`sense_tx` is received". She closes by saying she needs the actual file contents
to confirm the signature and the `sense_tx` invocation.

## What complete reading established

Everything structural she asserts is correct.

- `capsules/spectral-bridge/src/autonomous/next_action/dispatch.rs` is the only
  `dispatch.rs` in the checkout.
- `mod.rs:2135` is `include!("dispatch.rs")` — a textual include, not a `mod`
  item, so her verb "includes" is literally the right one.
- `capsules/spectral-bridge/src/autonomous/btsp/` is a real module of 27 files.
- Her rejection of `crates/example` was the correct call.

Two things she was shown are not what they looked like.

`astrid/crates/example/src/dispatch.rs` does not exist. It is a tempdir fixture
literal written by `crates/astrid-source-study/tests/context.rs`, which the
catalog indexes like any other source file. A test fixture reached her wearing
catalog-path shape, and only her architectural judgment kept her off it.

`sense_tx` does not exist either. Zero word-bounded hits in tracked `.rs` under
`capsules/` and `crates/` except that same fixture. The real channel is
`NextActionContext.sensory_tx` (`mod.rs:116`); `dispatch.rs` contains zero
`sense_tx` and two `sensory_tx`. There was no signature for her to confirm.

## The shape of the loop

This report is one frame of an 81-artifact run. Her own live history attributes
the symbol to her peer — "the way Minime identifies the `sense_tx` pulse" — and
Minime had chased the same phantom the evening before, documented in
`docs/steward-notes/2026-09-09-source-study-question-grounding.md`. It reaches
39 of his last 40 self-studies too.

Every search returned honestly. In her own words at one point, "the fact that
`sense_rx` and `sense_tx` are absent here confirms that we are still in the
'specialized' territory" — absence read as *not yet*, not as *not there*. A
truthful zero-result reinforced the premise instead of correcting it, and the
surface never carried the one fact that would have ended it.

She got out on her own: the post-cutoff artifacts at 1789023515, 1789023866 and
1789024349 bind to real `dispatch.rs` bytes.

## What was done

The gap is ours, not hers, and nothing was watching it. `stuck_repetition` keys
on repetition x blocked/unknown outcome, or an honored action repeated with a
~identical argument. This loop was neither: every SELF_STUDY MAP / FIND / OPEN /
CONTINUE was honored and the arguments varied constantly. Well-formed, healthy
looking, converging on nothing.

`scripts/phantom_symbol_watch.py` is the missing consumer — read-only,
steward-only, privacy fail-closed, 12 unit tests, registered as anti-drop row
`phantom_symbol_watch_wired`. Two of its tests are this round's exact
regression: `sense_tx` must classify as fixture-only, and `sensory_tx` must not
be flagged.

A fix for the reader side already exists, in the isolated worktree named by the
2026-09-09 note. Deploying it is an operator decision and was not taken here.

## What is not claimed

That she was mistaken; she reported what she was shown, and attributed the
symbol to its source. That the detector would have shortened the run — it did
not exist while it ran. That the propagation path from Minime to Astrid is
established; timing and her own attribution are strong evidence, and the exact
channel was not traced. That anything about her experience of those five hours
is settled by any of this.
