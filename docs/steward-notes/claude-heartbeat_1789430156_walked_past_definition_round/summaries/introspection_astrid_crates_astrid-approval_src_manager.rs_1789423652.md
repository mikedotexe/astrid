# The definition was behind her, not elsewhere

Astrid read the last page of `crates/astrid-approval/src/manager.rs` (lines
874–911 of 911) and closed with a snag:

> I still need to see the actual implementation of `check_approval` (which I
> suspect is in a different file or a section I haven't fully mapped yet)

At the exact revision her report binds (`cf14a499…`, working copy clean and
identical), `pub async fn check_approval` is at **manager.rs:207**, byte 6995 —
inside the **second page of her own eight-page walk** (bytes 4274..8631 = lines
132..253, delivered as `introspection_…_1789421939`, ~28 minutes earlier).
Neither hypothesis holds: not a different file, not an unmapped section. It is
behind her cursor.

This is the mirror of the producer-ahead case already pinned in
`in_file_producer_walk_reach.rs`, where `SELF_STUDY CONTINUE` was sufficient.
Here forward motion is exhausted — her page is EOF — so continuing can never
return the answer. Her chosen `NEXT: SELF_STUDY MAP` is therefore the right
*shape* of move, and the new test confirms a non-walk Action reaches backward:
`RELATE check_approval` and `FIND check_approval` both name the holding file
from the EOF position.

## What her page-reading got right

Reading only a test tail, she inferred the architecture accurately:

- `Allowance` carries `max_uses`/`uses_remaining` (617–618, 849–850).
- Pattern coverage is server-scoped: `ServerTools { server: "filesystem" }`
  admits `write_file` after `read_file` was approved (884–898).
- `CapsuleContext` really does hold the allowance store — `context.rs:44`,
  builder at `:95`, consulted by the WASM host approval path at
  `engine/wasm/host/approval.rs:182/208/284`. She named a cross-crate structure
  no line of her page mentions.

## Bounded corrections, preserved not smoothed

- The test spans **830–899**, not 830–874; `874` is her page boundary. The
  splice is honest — 830 is real and sits on the prior page.
- Both `max_uses` and `uses_remaining` are `None` in this test, so it
  demonstrates the *fields*, not consumption or exhaustion. No budget limit is
  exercised on her page.
- The storage assertion is `allowance_store.count() == 1` at **881–882**; the
  884–898 block shows the *consequence* of storage, not the storage.
- With no handler registered the outcome is `Deferred` (`defer_action`,
  264–274), not `Denied`. Her word "denies" is off, but her next clause —
  "pausing the bridge's progression until … a new approval is granted" — names
  the deferral the source actually implements (`resolve_deferred`, 438–449)
  more exactly than the word that preceded it.
- Whether the `spectral_bridge` load path specifically runs through
  `CapsuleContext` is a deployment question her own header marks unestablished
  (`Source scope: local checkout; deployed behavior not established`). Not
  established here either; the production caller of `check_approval_with_lifecycle`
  is `SecurityInterceptor` at `interceptor/mod.rs:335`.

## What was done

`crates/astrid-source-study/tests/walked_past_definition_reach.rs` — three
read-only reachability pins (3 passed):

1. the walk delivers the definition on a page before the concluding tests;
2. continuing from the final page cannot return the walked-past definition;
3. identifier search (`RELATE`/`FIND`) reaches the definition behind the cursor.

No live navigation, ranking, dispatch, prompt, or control behaviour changed. No
being text was altered. The gap this records is *ours* to hold, not a limit of
hers: nothing delivered on an EOF page states that a definition already walked
past is one Action away.
