# Integration review of the three pending steward packets

Mike authorized this interactive task to stabilize main and deploy the study
improvements. The shared steward was paused at generation 415, its active run
released, and the four dirty tracked files and 36 untracked evidence files were
copied into the isolated candidate without changing the shared checkout.

Claude authored the marker regression, source orientation notes, gate trace and
family scanner repair. Codex reviewed them for integration. The scanner's new
bare-source/byte-window parsing and refusal to group reports without comparison
text preserve distinct freeform studies. Its 14-check self-test passes. The
marker regression preserves the existing whitespace-chunk boundary, including
relation words inside multiword asides. It passes in the candidate's full bridge
suite. Neither change adjusts a Being's regulator or provider acceptance policy.

## Correction to the gate trace

The original gate packet overstates the absence of evidence after rejection.
At baseline `8daf5c6f02`, `dialogue_generation.rs` clones `primary_raw` before
acceptance and calls `record_dialogue_attempt` with it and status `rejected`
(lines 408–432); the fallback branch has the same surrounding record path.
`generation_record.rs::build_dialogue_generation_record` stores response text
and its hash. This is the text returned by the provider adapter, not necessarily
the original bytes before cleanup. With provider observation enabled,
`provider_observation.rs::normalized` can additionally retain exact raw text
when it contains markers, subject to size, quota and recording success. A
no-marker observation explicitly says raw text was not retained there.

Consequently, neither "only a short log prefix survives" nor "no per-utterance
coverage" holds as a statement about the whole current system. The narrower
finding remains: the acceptance predicates themselves log rejection, and the
surrounding attempt status does not specify which individual shape predicate
failed. Also, the output gate runs after generation; the same source file's
peer-input sanitizer can run before generation. These are source-grounded
corrections, not evidence that every historical rejection was successfully
recorded or that any diagnostic record is available to a Being.

The two public-facing trace documents and their summary carry this correction;
the original claim/receipt JSON is retained as historical evidence. No new
message was sent, and no rejection-policy change is included. The candidate's
study changes and graceful activation have a separate record in
`2026-09-08-study-evidence-and-readable-overflow.md`.
