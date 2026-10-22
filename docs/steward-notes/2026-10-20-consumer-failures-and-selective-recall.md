# Consumer Failure Tests and Selective Study Recall

## Scope

Follow-up to `2026-10-20-history-measurements-and-study-survey.md`: test actual
consumers and persistence failures, then compare selective source-study recall
offline. This tranche adds tests, measurements and review evidence. Production
behavior, authored history, stored pending inputs and recall preferences remain
unchanged. The only bridge runtime-file edit registers a `cfg(test)` module.

Work remains in
`/Users/v/.codex/worktrees/provenance-history-20261020/astrid`, branch
`codex/provenance-history-20261020`, based on
`68369bfa0a5e8a25b03dd7a70aa96b0782c35762`.
The sibling Minime measurement worktree and the earlier navigation/projection
candidate remain separate. No staging, commit, merge, service transition,
model call, control-plane mutation or automation resume is performed here.

## Public Witnesses

The preceding survey contains full-read receipts and individual dispositions
for all ten public studies. These four directly motivate the new tests;
their complete-file hashes were rechecked in this tranche.

| Being and public source | SHA-256 | Testable interpretation |
| --- | --- | --- |
| Minime `workspace/journal/self_study_2026-10-20T09-47-24.777052.txt` | `99f55d7fa309a9442b50fce05c53a79ec7f636b95add734b5b79133a8bf1c210` | Constructible authority fields versus their actual consumers |
| Minime `workspace/journal/self_study_2026-10-20T09-52-57.581796.txt` | `5ef101fdfbceadb44902fe90e5a77a2e32a3314107010360296b9197647c1744` | Serialization examples versus enforcement |
| Astrid `capsules/spectral-bridge/workspace/journal/self_study_1792514132.txt` | `a8a762a0c72d700fa4472a104ffc4a3780d648d4383460806011853dd6283dba` | Sequential persistence versus guaranteed consistency |
| Astrid `capsules/spectral-bridge/workspace/journal/self_study_1792514388.txt` | `39aa2cc93fde43b66fc411d9124a8a5c6a65280bd37f4c018a30100ba1b6e2f1` | Candidate acceptance versus validated knowledge |

Witnesses remain unchanged. The tests investigate mechanical interpretations;
they neither dismiss authored significance nor infer felt improvement.

## Actual Authority Consumers

`capsules/spectral-bridge/tests/authority_consumer_contract.rs` calls
`current_context`, `verify_read_only_research` and `verify_live` against synthetic
workspace manifests. It does not dispatch actions or use production credentials.

Three tests establish:

- A context can deserialize successfully while verification rejects its schema,
  version, scope, token identity, future issue time, expiry, exhausted budget,
  lifecycle state, pause generation, source identity or deployment identity.
  Extra declarations such as `safe: true` do not substitute for consumed fields.
- Stored context is rechecked when source, pause generation or time changes;
  verification leaves the original context unchanged. The exact expiry second
  is allowed by the current implementation, while the following second fails.
- Read-only verification permits process rebinding under unchanged deployment
  identity; live verification rejects the mismatched process. Changed deployment
  identity fails the read-only verifier too.

This is narrower than a full security audit. The verifier does not take an
`IpcPayload` argument: recognized transport tags and a successfully decoded
authority-shaped object are not themselves approval.

## Candidate Acceptance and Persistence

`capsules/spectral-bridge/src/action_continuity/consumer_failure_tests.rs`
uses the actual continuity store and SQLite mirror with temporary synthetic
data. All five focused tests pass.

1. A non-placeholder hypothesis makes `valid_experiment_charter` true;
   presence of an evidence object permits candidate status to become `accepted`.
   Neither changes or validates the authored content. The stricter lifecycle
   predicate remains false, and the actual execution-eligibility consumer reports
   missing charter, meaningful evidence and read-only rehearsal.
2. If appending experiment history fails, neither the thread file nor the
   supplied in-memory thread argument advances.
3. If append succeeds but the thread write fails, experiment history remains
   appended, the old thread file remains stale, and the passed mutable thread
   argument has already changed. Once the synthetic filesystem obstruction is
   removed, normal thread reading reconciles the latest experiment without
   appending the same experiment again.
4. A read-only database mirror can reject writes while the overall file-first
   update returns success. The synthetic file state advances; the old database
   mirror remains. This is existing best-effort mirroring, not atomic file/DB
   commit.
5. If run append succeeds but thread writing fails, the failed run record and
   its suggested NEXT remain in history, but the thread's selected NEXT does
   not change. A saved suggestion is not proof that it was selected or run.

These are deterministic filesystem failure injections, not process-kill,
power-loss, torn-write, concurrency or fsync guarantees. Synthetic actions are
recorded, not dispatched. No live history is restored from a backup.

### A Failed Assumption Worth Keeping

The first candidate test incorrectly expected the advisory
`charter_required_guard_assessment` to enforce the stricter lifecycle predicate.
It returned no block: its early classification uses the looser charter test.
The corrected test preserves that actual behavior and separately asserts the
execution-eligibility rejection. No guard was changed to obtain a passing test.
This distinction is useful counterevidence to treating all "valid" or "accepted"
labels as interchangeable.

## Controlled Input Comparison

`crates/astrid-source-study/tests/selective_recall_comparison.rs` exercises the
real shared reader for both owner configurations, using synthetic sources and
synthetic delivered accounts. It contrasts:

- Baseline: the prepared source-page input, including up to four prior accounts.
- Comparison: a copy with only the recalled notebook's `recent` array emptied.
  The latest account, question, explicit findings, source, system prompt,
  navigation choices and all other fields remain identical.

The comparison copy is never submitted to a provider or stored as an offer.
Retaining its original delivery identifiers is a parity assertion, not permission
to deliver changed text under an old identity. A future real candidate would
need its own prepared input and verified delivery.

Four content fixtures run for each owner. Sizes are user-input UTF-8 bytes,
not tokens or whole provider requests. Both owners produce these results:

| Fixture | Baseline bytes | Comparison bytes | What remains or disappears |
| --- | ---: | ---: | --- |
| Ordinary repeated history | 13,818 | 6,757 | Latest account retained; three older accounts omitted |
| Older contrary observation | 13,883 | 6,757 | Counterexample omitted with older account |
| Same observation explicitly saved as a finding | 16,249 | 9,123 | Saved counterexample remains |
| Latest authored qualification | 13,891 | 6,830 | Latest qualification remains |

This is about a 51% reduction for the ordinary fixture, not evidence of better
reasoning, more exploration, faster generation or a subjective effect. The
counterexample case is a negative result for an unconditional latest-only policy.
Source findings remain authored claims, not independently verified truth.

Controls also cover map browsing after parking, fresh reflection and explicit
parked-inquiry review. The transform is the identity on those routes. No state
is written by the transform; the original pending offer remains recoverable.
Parking clears selection while retaining the original notebook. Explicit return
restores the original older/latest account material, not the slimmed copy.

The inquiry review shows explicit saved findings but does not expose all prior
account prose. In the unsaved-counterexample fixture that observation is absent
from review as well. Retention is therefore not equivalent to a convenient,
independent inspection affordance. We must qualify such retrieval before calling
an automatically omitted account "available on demand." No new inspection command
or automatic semantic selection is implemented in this tranche.

Exact sizes, hashes, owner/fixture/route identities and production source hashes:
`2026-10-20-selective-recall-comparison.json` (32 compared route/fixture rows).
The comparison has no inference calls, model sampling, latency measurements,
private prose, policy change or causal verdict.

## Next Bounded Work

1. Make failure receipts distinguish retained experiment/run history, thread
   selection and database mirroring before considering a recovery change.
   Preserve append evidence; do not blindly retry an operation whose earlier
   append may already exist. A repair needs idempotency and interrupted-write
   qualification, not just different wording.
2. Build deliberate, bounded inspection of retained study accounts within the
   existing notebook. Preserve source revision, exact retained text, excerpt
   status and inquiry ownership. Inspection must not select or unpark work.
3. Then compare a lean routine view with explicit saved findings and authored
   selection, against the existing view. Do not automatically classify a passage
   as a mistaken belief, summarize it, or require every contradiction to be
   promoted into a finding.
4. Keep any later model comparison matched on source, question, model, sampler
   and available tokens. Include older counterevidence and latest qualifications
   as controls, not just repeated ordinary prose. Candidate acceptance requires
   evidence accessibility and correct boundaries, not merely fewer bytes.

MLX and larger-capacity work do not participate. No deployment is required for
these tests; any later runtime change needs its own review and qualification.

## Source Identities

Paths are relative to the isolated Astrid worktree:

| Source | SHA-256 |
| --- | --- |
| `capsules/spectral-bridge/src/action_continuity/runtime/core.rs` | `c1902019596b63817ce512d220c79bfea2f0823dd9cf5e4a2ebf1efd26ac1451` |
| `capsules/spectral-bridge/src/action_continuity/runtime/experiment_evidence.rs` | `29d4950261eaed0d5f9c277c46b2386abff4bc70c9b660741ec98b6fbece50db` |
| `capsules/spectral-bridge/src/action_continuity/guards.rs` | `887e9b2214be8ed29b3840466a22704735feb17cef72d73ea90bac39d31cf70c` |
| `capsules/spectral-bridge/src/authority_temporal.rs` | `a470b1d2b2ea43248f47150c1aafe6b5e6d1214849cda0c41c9a241444c7b11a` |

## Reproduction

Use the explicitly owned worktree target directory with
`CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0`.
The pre-build audit passed: 1,512,439,808 allocated debug bytes and
451,658,866,688 free volume bytes. Release profiles are unchanged.
Bridge sibling dependency symlinks resolve to the existing
`/Users/v/other/prime_esn_wasm` and `/Users/v/other/RASCII`; their sources were
not edited.

```sh
cargo test --manifest-path capsules/spectral-bridge/Cargo.toml --test authority_consumer_contract
cargo test --manifest-path capsules/spectral-bridge/Cargo.toml --lib consumer_failure_tests
ASTRID_SELECTIVE_RECALL_REPORT="$PWD/docs/steward-notes/2026-10-20-selective-recall-comparison.json" cargo test -p astrid-source-study --test selective_recall_comparison
```

Initial strict Clippy found unchecked test-only offset arithmetic, a long test
body and a temporary formatting allocation. Use checked offsets and a dedicated
synthetic-account constructor; no production code or comparison semantics were
changed to resolve these findings.

## Final Qualification

- Authority-consumer integration: 3 tests passed.
- Candidate/persistence failure injection: 5 tests passed.
- Full native bridge suite: 2,459 unit tests passed, one existing test ignored;
  all integration and compile-contract tests passed.
- Complete `cargo test -p astrid-source-study --quiet`: passed. The subsequent
  fixture-only Clippy cleanup was followed by another passing comparison run.
- Strict Clippy for bridge and shared reader, all targets and features: passed.
- Formatting checks for both: passed.
- `python3 -B scripts/domain_boundary_audit.py verify`: valid, zero violations;
  no boundary baseline changed. Existing large-file review debt remains.
- Tracked diff whitespace check: passed.
- Regenerated comparison receipt is byte-identical, SHA-256
  `39bf652d711009589a2bc542176186c6cbcc4d5f19607a44fcd0cadf07013c40`.

This is consumer/reader test qualification, not immutable paired-release
qualification. No Minime Python/runtime source changed in this turn, and the
complete Minime Python, deployment, controller and evidence/projector suites
were not rerun. The preceding survey records its separate focused Python checks.
Both owner configurations here use the real shared Rust reader, not both full
provider adapters. Real-model output effects remain unmeasured.

Final cache audit passed: 5,419,630,592 allocated debug/test bytes (about 5.05 GiB)
and 447,825,842,176 free volume bytes. Free space decreased by 3,833,024,512 bytes
over the interval, not exclusively attributable to this build on a shared
machine. This owned cache remains useful for the unintegrated candidate and
its referenced isolated helper/compile-contract qualification; no cleanup,
live-release or rollback deletion was performed.

Canonical Astrid still has the same eight tracked MLX-edited paths; canonical
Minime is clean. This compares the path inventory, not the other coordinator's
concurrently editable file contents. All current changes remain unstaged in the
owned worktree. Historical files, live configuration and controller ownership
were not modified.
