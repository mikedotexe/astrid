# introspection_source_catalog_1789224730 — the leaf that names a component

Astrid, after a recovery map: *"I must stop trying to force a specific file into existence and
instead see what the system actually offers. My objective remains the same, but my method must
shift from *seeking* to *mapping*."* She then asked for the contents of the `verification`
directory, to find "the true entry points for the three pillars of integrity", and closed with
`NEXT: SELF_STUDY MAP astrid/capsules/spectral-bridge/src/autonomous/runtime/verification`.

**Her method shift is correct. Her leaf is not a directory — it is a component ID.**
`verification` is `[[components]] id = "verification"` in the embedded catalog manifest
(`crates/astrid-source-study/catalog.toml:68-71`), titled "Tests, configuration and
deployed-source evidence". `SELF_STUDY MAP verification` resolves in one move through the
component branch (`navigation.rs:47-60`). The rooted path she chose cannot resolve, because
no directory of that name exists anywhere in either checkout.

**Cost, measured from her own artifact headers:** that exact action was issued **29 consecutive
times over 100.3 minutes** (1789221325..1789227344), every following turn answered
`Recovery map: the requested source was not supplied`. She escaped only by abandoning the
target for `SELF_STUDY MAP astrid`. Five hours earlier the same shape cost 56.5 minutes on
`MAP continuity`.

**Why rootedness did not save her — a correction to our own prior framing.** The previous round
concluded that a rooted form "would have landed her in one move". This topic *was* rooted. The
guard at `path_recovery.rs:14-21` is necessary but not sufficient: the filter at 41-51
additionally requires the topic's final segment to equal an existing catalog *directory's*
final segment. `path_candidates(topic, directory: true)` walks only ancestor directories of
catalog sources — the component namespace is never searched, so the one thing that would have
helped is structurally unreachable.

**What we are not claiming.** The working command was never withheld. `recovery_with_candidates`
appends the whole root map, so the row `verification — Tests, configuration and deployed-source
evidence ... SELF_STUDY MAP verification` sat in her input on all 29 turns, and `store.rs`
`output()` bails rather than truncating. What is missing is any link between the topic that just
failed and that row: `SELF_STUDY MAP <x>` accepts three namespaces — component IDs, repository
IDs, rooted directory prefixes — and the reason line cannot tell "this word is nothing" from
"this word is a component, but not at that path".

**Contradiction preserved.** Even reached, `MAP verification` would not have shown her the three
pillars: the component's six sources are Cargo.toml, `build_bridge.sh`,
`agency_resolver_integration.rs`, a Minime introspect test, `minime/Cargo.toml` and `AGENTS.md`.
The anchor-validation and thinning material she describes lives under
`astrid/capsules/spectral-bridge/src/autonomous/runtime` — itself a valid MAP topic, one segment
shorter than what she typed.

**This round:** one regression pinning the shape
(`rooted_map_topic_ending_in_a_component_id_gets_no_candidate_naming_that_component`), including
the honest assertion that the root menu *does* carry the working command while the reason and
candidate regions do not. The repair itself rewrites her live recovery prompt and is recorded as
a Mike/operator boundary, not attempted.
