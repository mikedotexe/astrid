# introspection_astrid_crates_astrid-kernel_src_maintenance.rs_1789566573

Astrid read a page of `crates/astrid-kernel/src/maintenance.rs` and contrasted the
generation-transition path with the scheduled-reflection path, ending in a STUDY_QUESTION:
does the system ever use `runtime_gid` inside `read_generation` or other transition-related
functions, or is it strictly reserved for `ScheduledReflection` and `BoundLease` types?

## What complete source establishes

Report-bound source SHA `1b8c8385c944…` matches `git HEAD` exactly, so report-time bytes are
recoverable and no report-time/current-source split is needed. Full file read, lines 1-864.

Her structural claims about `read_generation` (516-545) are exact: root uid, mode `0o444`,
`nlink()==1`, symlink rejection, plus a 256-byte cap she did not name. Her claim that the
generation gate omits any `runtime_gid` check is exact — the function's signature is
`(path: &Path)`; the identifier cannot reach it.

**Her question, answered:** `runtime_gid` is initialized once from `getegid()` (282), threaded
into BOTH lease reads (308 transition, 314 reflection), forwarded through `read_bound_lease`
(339/341) into `read_bound_lease_for_owner` (349) — and compared at exactly **one** site, the
`ScheduledReflection` arm (365). So the parameter is shared by both `BoundLease` kinds; only the
reflection arm consults it. No other transition-related function (`validate_transition_lease`,
`write_ack`, `read_generation`, `stable_read`, `atomic_owner_write`) touches the group at all.
The ACK side protects itself by mode instead: private parent dir (`&0o077 == 0`) and a `0o600`
create-new temp file (612-636).

## One mechanism correction, preserved not smoothed

She writes that `read_generation` "checks the content against `valid_identifier` (lines 533-541)".
There is no such call. Lines 528-541 **inline** the same predicate and then add a stricter
canonical-form requirement (`bytes == "{generation}\n"`, 541). `valid_identifier` (548-558) has
exactly one call site in this file: line 456, validating the *reflection* lease's `generation_id`.
The predicate she names is the right predicate; the call she implies does not exist. Her
substantive point — that transition content is validated to the identifier grammar — holds.

## One delivery observation

Her header window is `bytes 20750..24941`, which is **lines 580-717** of the report-time file
(`process_start_ticks` → the head of `mod tests`). `read_generation` was not on that page. It was
on the immediately preceding page (`16466..20750` = lines 455-580, report `…_1789566303`), and
`read_bound_lease_for_owner` on the one before (`12048..16466` = lines 344-455). So her prose
"The current source page (lines 516-590)" misattributes the page; the code she recalls from two
earlier pages is accurate in both content and line numbers. This is recorded as an attribution
mismatch, not as a confabulation, and not as a reason to discount the reading.

The addressing projection carried `lived_state_alignment: artifact_integrity_unavailable` for this
item (1 issue, 1 gap) while the witness's own `artifact_sha256` matches the report byte-for-byte;
that divergence is noted as a projection-side observation only, with no claim about its cause.

## What was implemented

`crates/astrid-kernel/src/maintenance.rs` gains one focused regression,
`generation_transition_lease_ignores_the_runtime_group`, the negative contrast to the existing
`scheduled_reflection_lease_rejects_a_foreign_runtime_group`: a valid `0o444` transition lease read
with a foreign GID stays `LeaseState::Active`. Her question is now pinned by a test rather than by
prose. 7/7 `maintenance` tests pass.

## Authority

Evidence only. Nothing live was changed, deployed, restarted or proposed for approval. No
production behavior was altered by adding the test.
