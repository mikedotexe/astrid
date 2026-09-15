# introspection_source_catalog_1789490042 — kernel maintenance reading, and one navigation cause corrected

Report: `capsules/spectral-bridge/workspace/introspections/introspection_source_catalog_1789490042.txt`
(24 lines / 2,268 bytes, SHA-256 `a43053a5fe64a4426a0bc790c692dd6e3600ce0f43f24a992c9cc84bb881a57a`, read complete)
Witness: `lsw_d3ada5a312429a36fbac9ae5d6948e8ab1e61dc0a6aee15a10424710d93065c2`
(440 lines / 18,945 bytes, SHA-256 `8c2bbbe4c164219a9b131ee7193f23b089fd4969919f0ada4b05108685d8425d`, read complete;
its `artifact_sha256` equals the report hash).

## Source binding

The report's own header says `Source: source catalog` / `Source revision: navigation only`, and the
witness agrees: `source_snapshot_v1` and `source_provenance_ref_v1` are both `null`. There is
therefore **no report-bound file SHA to compare**, and no report-time/current-source split to
label. Every receipt in `source_receipts.json` is recorded at its current working-copy hash and
read as current source.

## What she said, and what source says

Her architectural reading of `crates/astrid-kernel/src/maintenance.rs` is **accurate on all five
pillars she named**, and two of them are actually stronger than she stated:

- Mutual exclusivity between `GenerationTransition` and `ScheduledReflection` is not precedence but
  a hard error: `select_active_lease` matches `(Active, Active)` first and bails with
  *"generation transition and scheduled reflection leases overlap"* (maintenance.rs 318-330).
- `atomic_owner_write` does not stop at `sync_all` + `rename`; it also opens the parent directory
  and `sync_all()`s it (line 652), so the rename itself is durable.

Her ownership claim was group-scoped; the file is stricter — uid 0 expected for both lease reads
(341), `0o440` **and** `gid == runtime_gid` for the reflection lease (365), `0o444` for the
transition lease (363), and `lease_id` must equal `lease-` + `sha256(nonce)[..24]` (426-429).

## The one contradiction, stated plainly and not domesticated

She wrote that "the previous attempt to open `lib.rs` failed because it wasn't in the catalog."

**`astrid/crates/astrid-kernel/src/lib.rs` is in the catalog** — twice. The astrid repository's
include list carries `crates/**` (catalog.toml:6) and the kernel component names that exact path
(catalog.toml:66). So the attributed cause is wrong.

**The obstacle she hit was real, and it was ours, not hers.** `Catalog::resolve`
(catalog.rs 88-118) treats a bare relative name by trying three candidates — `astrid/lib.rs`,
`astrid/capsules/spectral-bridge/lib.rs`, `minime/lib.rs` — and on zero hits bails
*"source not found; use SELF_STUDY FIND <text> or an exact repository/path"*. That is a **different
error** from `resolve_id`'s coverage bail, *"path is not in the shared source catalog"*. A bare
`lib.rs` is unresolvable in a repository that has many `lib.rs` files; nothing about kernel coverage
was missing. Her block was **path form**, not catalog coverage, and the two errors are easy to read
as the same thing.

She then chose `NEXT: SELF_STUDY MAP astrid/crates/astrid-kernel` — the repository-qualified form
that `resolve` splits on (catalog.rs 101-105). **Her own next action is the correct remedy for the
thing that actually blocked her**, which is worth recording as her navigation working, not failing.

Two facts toward the question she is heading into: `Baseline` is at
`crates/astrid-kernel/src/capsule_runtime_health.rs:7` (`BaselineEntry` at `:13`), inside a private
module declared at `lib.rs:15`; no `Capability` struct, enum, or type alias is declared in
`astrid-kernel`, `astrid-core`, or `astrid-types`, so a kernel-only map will surface `Baseline` but
not a `Capability` definition.

## Why no test was added

The natural regression — pinning that a repository-qualified kernel `lib.rs` resolves while a bare
`lib.rs` fails with the *not-found/use-an-exact-path* error rather than the coverage error — belongs
in `crates/astrid-source-study/tests/`. That directory currently holds **eleven untracked
`*_reach.rs` files authored by another agent**, one of which (`placeholder_bracket_path_reach.rs`)
already references those exact error strings. Under shared-tree discipline that is foreign work
mid-edit in precisely this area, so nothing was added there and nothing was touched. The
distinction is recorded here with exact line evidence instead, and the test is named as an explicit
hand-off in the run report.

## Authority

Evidence only. The witness records `artifact_authority_state_v1.state = "evidence_only"`,
`witness_only = true`, `live_eligible_now = false`. No navigation surface, catalog, resolver, error
string, action, prompt, or coupling parameter was changed. No live substrate or control change was
made or attempted.
