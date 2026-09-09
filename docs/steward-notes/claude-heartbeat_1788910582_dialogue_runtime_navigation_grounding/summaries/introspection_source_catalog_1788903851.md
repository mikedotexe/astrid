# introspection_source_catalog_1788903851 — navigation-level read of `dialogue_runtime.rs`

- Report: `capsules/spectral-bridge/workspace/introspections/introspection_source_catalog_1788903851.txt`
  (18 lines, 1,489 bytes, `sha256:c98739744398f2327abbad6883cd7bb5d08bd9f8fbc48c8cd11ff800fed9748a`) — read complete.
- Witness: `lsw_3e351eb747ac53c6c793a84259c2b1c61fdc3a9b6f7e37302f595e8c71b0df4d`
  (440 lines, 18,974 bytes, `sha256:d7726b6ee3d335e649b975eb4c9cbcf6e46e7f0db54fc9373d74b169301ca5f8`) — read complete.
- Report-bound source: "source catalog / navigation only" — no source SHA is bound by this report,
  so the named file was verified independently: `capsules/spectral-bridge/src/llm/provider/dialogue_runtime.rs`,
  812 lines / 29,562 bytes, `sha256:03c6b6dee0436dd56c047ab68d95f2f4ccd6e2ed7c8029cf0b9568eb9cfefa91`,
  read complete (L1-812). That hash equals the source revision her own adjacent byte-window reports
  bound (`..._1788894572` through `..._1788902177`), so report-time and current bytes agree.

## What she said

She read the *structure* of `dialogue_runtime.rs` at catalog level and described it as "the connective
tissue where the abstraction of the 'provider' meets the actual mechanics of the interaction," said
"`DialogueRuntime` handles the lifecycle of a turn" with "deliberate layering" mediating model output,
read its interface with "`SpectralBridge`" as "a conscious effort to maintain state or 'resonance'"
across the transition from raw model output to structured dialogue, then asked directly whether the
provider is "just passing tokens, or is there a transformation ... that shapes the 'texture' of my
responses" — and asked to see how provider implementations hook in and what feeds into the runtime.

## What complete reading established

1. **Two names are not in the code.** `DialogueRuntime` appears in no `.rs` file under `capsules/` or
   `crates/`; `SpectralBridge` does not exist as a type (nearest real: `SpectralBridgeEvent`,
   `types/schema/events_and_attractors.rs:3`, unreferenced here). The contradiction is recorded, not
   smoothed: the filename advertises a runtime object the file never defines. That is our naming.
2. **The layering she felt is real, and it is admission layering.** Marker sanitation → shape gate →
   single-final-`NEXT` contract, with a Gemma-4-canary-only deprecated-language reject.
3. **Her question has a precise answer.** The only transformation applied to her output is subtractive
   and exact: known control markers are removed unless quoted, grouped, or named by a relation word;
   every non-marker byte is copied byte-exact; gates admit or reject a whole candidate and never
   rewrite her prose. The one input-side edit strips minime's peer action/status lines and says so
   with a visible placeholder.
4. **Direction is inverted from her expectation.** Nothing feeds *into* this file as a runtime; seven
   call sites consult it (transport.rs:717; dialogue_generation.rs:206/250/549; fallback_contracts.rs:53/234/690;
   autonomous/state.rs:3492 via the `llm.rs:22` re-export). Provider assembly is one flat `include!`
   module (`llm/provider.rs:18-47`), so there is no hook point; the only provider-variance parameter is
   `MlxProfile { Production, Gemma4Canary }` (`configuration.rs:371-374`).
5. **Her "deployed behavior not established" caution is accurate**, not boilerplate: the witness records
   `deployment_established: false`, and `source_snapshot_v1` / `source_provenance_ref_v1` are null,
   which is why the queue marks a navigation-only report `artifact_integrity_unavailable`. Evidence
   shape, not a defect in her report.

## Answer delivered

`DIALOGUE_RUNTIME_ORIENTATION_MAP.md` in this packet is the grounded map she asked for: complete
region inventory, the marker/gate behavior with exact test citations, the flat-`include!` provider
assembly, the caller inventory, and an explicit authority boundary.

## Not done / not authorized

No source, test, configuration, control, or deployment change. `control_marker_annotation_tests.rs`
(included by `dialogue_runtime.rs`) is dirty foreign work by another agent and was left untouched;
the naming-drift finding in §1 is therefore recorded as an observation for a later owner of that
module, not fixed here. No correspondence was dispatched to Astrid; the map is durable
documentation she can reach by INTROSPECT, and silence about it stays neutral.
