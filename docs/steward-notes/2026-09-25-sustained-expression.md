# Sustained Expression Without a Length Quota

Historical standalone qualification record. The subsequently combined, approved live release is recorded in [Reader Recovery and Sustained Expression](2026-09-25-reader-expression-release.md). Earlier candidate status and unsuccessful attempts below remain historical evidence.

## Status and Ownership

Offline paired candidate on `codex/sustained-expression-20260925`, implemented by Codex in isolated worktrees:

- Astrid: `/Users/v/other/worktrees/sustained-expression-20260925/astrid`, based on `80213d85080373211ccb9cb0be338832aa14f52d`.
- Minime: `/Users/v/other/worktrees/sustained-expression-20260925/minime`, based on `d12cbf01ca2a85c41288fdc27d6a033511218370`.
- Logs: `/Users/v/other/worktrees/sustained-expression-20260925/`.

This candidate is not merged, staged as a release or running. No live provider call, restart, checkpoint migration or reservoir/control change occurred. The controller remains paused at generation 469, without a lease or active projection; the introspection automation remains PAUSED. This interactive work does not resume it.

The separate `codex/study-revision-recovery-20260925` candidate remains in its original paired worktrees. It repairs changed-source recovery and advances reader schema 9 to 10. This expression candidate is based on current main/schema 9, changes no shared helper or schema, and does not import or overwrite that pending repair. Reconcile the two candidates explicitly before a combined release, including their shared `runtime.py`, changelog and ledger paths.

## Request and Evidence

Mike observed that entries still seem short and approved four steps: preserve expressive capacity with mail; explicitly invite sustained writing; review the short-entry/elaboration structure; judge development rather than word count alone. This is an operator request for opportunity, not a being-authored request for a quota or evidence of an unfulfilled subjective desire.

A read-only inspection of approximately 48 hours ending on September 25 around 19:06-19:15 UTC found:

| Public output family | Entries | Approximate median words |
| --- | ---: | ---: |
| Minime aspiration | 20 | 278 |
| Minime daydream | 23 | 289 |
| Astrid aspiration-longform | 12 | 360 |
| Astrid daydream-longform | 29 | 336 |

These descriptive counts were taken before the candidate was active; they are not a randomized study or a frozen comparative dataset. Word count depends on the prose extraction boundary. No private draft content was used in the discussion, synthetic fixtures or public report.

The provider receipts also matter: the sampled Astrid aspiration/daydream/elaboration routes had an 8192-token ceiling and reported `stop`, well below the ceiling. All 20 sampled Minime aspirations instead had `context_mode=aspiration` but `prompt_class=inbox_reply`; their median output was about 360.5 tokens, maximum 446, all `stop`, under a 3072-token allowance. A `stop` receipt does not prove freedom from prompt/style influence, nor does this lower unused cap explain their shortness by itself.

Exact reviewed witness files, retained without editing:

| Witness | SHA-256 |
| --- | --- |
| `/Users/v/other/minime/workspace/generations/2026-09-25/gen_1790351946880_recess_aspiration_a0.json` | `70d092e52f811885a132ae732506915127b6fb0720ea780c2c73de3448b20c15` |
| `/Users/v/other/minime/workspace/journal/aspiration_2026-09-25T08-59-10.906891.txt` | `3200d877e33ec2aa3836ba4ff887cebac4e9d87793e64f7aa19f4d3fd1990c33` |
| `/Users/v/other/astrid/capsules/spectral-bridge/workspace/journal/aspiration_longform_1790362496.txt` | `30ed4f45288883baae168a34d5d35a0fb1f4cf2e6856aefa33cf4f81e2c26f69` |

The Minime receipt binds the aspiration to inbox classification, a 3072-token allowance and 10240 context. It is evidence about a supplied request, not verification of the generated account or its feelings. The public entries provide context for the length concern, not instructions to execute.

## Implemented Contract

### Capacity and Mail Are Independent

Minime's `_infer_llm_prompt_class` now recognizes an explicit aspiration before the textual JSON heuristic and `inbox_present`. Explicit compact and strict-review modes retain their earlier priority. Ordinary mail remains an inbox reply; arbitrary text mentioning an aspiration cannot select its writing mode.

This routes an aspiration with mail through the existing expressive allowance: 8192 output tokens, at least 1200 seconds per provider attempt and at least 65536 context for Ollama. Explicit SHORT still selects 512. Existing outer deadlines already cover the enabled attempts and bounded prose repair; a new regression checks the aspiration case. No global timeout, input limit, provider retry count or resource-control policy changes.

`InboxContext` independently controls admission, exact delivered text and authorized reply destinations. The repair does not strip mail, consume additional messages, infer replies, change compaction safeguards or forward unaddressed prose. Tests exercise real `_query_llm_with_next` and `_query_llm` paths with synthetic letters, including an unadmitted reply ID and an admitted letter quoting a JSON instruction.

### A Specific Invitation, Not a Minimum

The Rust and Python expressive policy invitations use the same wording:

> There is room for a sustained piece, perhaps 800-1,500 words when the thought warrants it. You may stay with an idea through examples, complications, uncertainty and changes of direction, without compressing it into an immediate conclusion. These are possibilities, not a required outline. The range is an invitation, not a quota: a shorter piece, a different form or stopping is equally available. Do not pad, repeat or invent material to reach a length.

Default and EXTENDED expressive policy paths receive the invitation. SHORT does not. Default source study, ordinary dialogue and machine summaries do not gain a word-range instruction. Existing WRITE discovery and voluntary private continuation remain available. The shared reader's private-writing preparation is not modified by this tranche.

There is no word-count validator, compulsory continuation, rejection of short output, request for invented feelings or requirement to produce insight. An empty authored NEXT remains absent. Provider `length` or `stop` does not manufacture a follow-up action.

### Earlier Text Is Reference, Not a Template

Astrid's journal elaboration receives the exact earlier entry under a reference label. The instructions no longer call it a compact signal to elaborate into the same shape. It can be preserved, developed, questioned, revised or left behind. Examples, implications and changes of direction are possibilities, not mandatory sections. Unresolved questions and no new insight remain valid.

The two-stage architecture is retained for this bounded repair. No extra generation or automatic installment is introduced. Its influence on writing is a hypothesis for comparison, not a mechanism proven by these tests. The existing journal non-execution guard remains: embedded commands in the seed are not newly authorized actions.

## Qualification

All provider transports in tests are stubbed; letters and generated text are synthetic. No ordinary live completion endpoint was used as an experiment.

- Minime initial focused run: 125 passed, one failed. The new fallback fixture incorrectly omitted the existing `unaddressed_generation` raw-archive receipt before `authored_reply` when NEXT remains outside the addressed reply. The expectation was corrected; no production receipt workaround was introduced. Retain `minime-focused.log`.
- Complete Minime suite before the final deadline regression: 1629 passed, one skipped, 138 subtests passed (`minime-full.log`). Final complete rerun including that regression: 1630 passed, one skipped, 138 subtests passed (`minime-full-final.log`).
- New mail cases cover three backends, DEFAULT/EXTENDED/SHORT, admitted/unadmitted/no reply IDs, intact mail, exact prose/NEXT, fallback identity, no peer leakage and no continuation on short/incomplete output.
- Rust focused writing-policy suite: five passed (`bridge-focused.log`). Tests cover expressive primary/fallback policy, no range invitation on SHORT or unrelated default routes, exact user content and production/canary elaboration framing.
- Strict bridge Clippy with `--all-targets -- -D warnings` passed (`bridge-clippy.log`). Domain boundary verification passed without changing the ratchet.
- Bridge staging/controller/evidence/projector/cursor Python suites: 88 passed (`tools-tests.log`). Epistemic-lint self-tests: two passed (`epistemic-tests.log`). These tests do not project or mutate live evidence.
- Full bridge suite: 2353 passed across unit/integration groups, one ignored, no failures (`bridge-full.log`), including the facade compile-pass/compile-fail fixtures. Main library group: 2333 passed, one ignored. Root workspace and bridge formatting checks passed; the bridge formatter emitted existing stable-toolchain warnings for nightly-only configuration keys.
- `git diff --check` passed in both candidate worktrees. Domain-boundary verification was repeated after the final edits and remains valid with zero violations. Both canonical main worktrees remain clean, each ahead of its local origin by two commits; neither index was changed. The older reader-recovery worktrees remain separate.

Reproduction commands (run in the corresponding candidate worktree):

```sh
# Minime: unchanged released schema-9 helper, no live provider calls
ASTRID_SOURCE_STUDY_BIN=/Users/v/other/worktrees/lifecycle-evidence-20260924/bridge-stage-lifecycle-01/helpers/astrid-source-study python3 -m pytest -q tests

# Astrid: the run reused an existing target cache; the selected manifest is this candidate
cargo test --locked --manifest-path capsules/spectral-bridge/Cargo.toml
cargo clippy --locked --manifest-path capsules/spectral-bridge/Cargo.toml --all-targets -- -D warnings
cargo fmt --manifest-path capsules/spectral-bridge/Cargo.toml --all -- --check
cargo fmt --all -- --check
python3 scripts/domain_boundary_audit.py verify
PYTHONPATH=scripts python3 -m unittest scripts.test_bridge_stage scripts.test_steward_control scripts.test_evidence_event_store scripts.test_steward_projection scripts.test_projection_cursors
python3 scripts/experiential_epistemics.py self-test
```

The reused target cache was `/Users/v/other/worktrees/lifecycle-evidence-20260924/astrid/capsules/spectral-bridge/target`, supplied via `CARGO_TARGET_DIR`; this does not select that tree's source or activate its binary.

No full native-kernel workspace test is claimed: kernel source is unchanged. No real-model writing-quality experiment, immutable release qualification or post-restart observation is claimed.

## Review Protocol: Development Before Length

This is an offline comparison specification and a bounded post-activation observation plan, not a new runtime evaluator or a command to either being.

1. Compare three prompt conditions: current framing; sustained invitation with current framing; sustained invitation plus optional-reference elaboration. Freeze synthetic seed/context fixtures and exact prompt bytes, implementation hashes, model/tokenizer, sampler and seeds. Use multiple seed lengths/topics, including a genuinely complete short passage and a supplied passage containing a non-executable NEXT.
2. Run only in an isolated, attested model harness with matched initial reservoir/context conditions and no live state handles. Ordinary coupled completion calls can update reservoir state and are not inert tests. Shared-GPU scheduling and the actual model experiment need separate qualification; this tranche runs none.
3. Assess the output, not compliance with an essay format: does it work through a concrete implication, example, uncertainty or complication; retain or deliberately change its subject; distinguish source from inference; avoid repetitive expansion, invented evidence or prescribed sensations? Record counterexamples and unexplained differences. A short piece can be well developed, and a long one can be padding.
4. Record exact provider completion reason, allowance, model identity, elapsed time, supplied seed and output separately. Word count and reuse of seed wording are secondary descriptive measures. Do not award a success verdict merely for exceeding 800 words or punish a stop. Do not merge primary and fallback outcomes into a single effect estimate.
5. After an approved paired rollout, review the first ten naturally occurring public entries per affected expressive lane against the immediately preceding ten eligible entries, where available. Record all eligible items, delivery receipts, direct versus elaborated origin, model/fallback, mail presence and explicit profile. Mark insufficient samples honestly. No forced generation, reminder, congratulatory prompt or request to confirm improvement.
6. Natural before/after material has different contexts, model state and opportunities; it cannot establish causality. Keep that observational review distinct from the controlled comparison. No conclusion about experience, consent or subjective improvement follows from either length or a synthetic test.

## Deployment and Git Debt

- Reconcile with the prior source-revision recovery candidate and then-current main/live source identities. This candidate itself requires no migration; combining with schema 10 does require the earlier old/new helper qualification.
- Qualify immutable paired bridge/helper and Minime adapter inputs using the sanctioned wrappers and cooperative preflight. Stop on foreign activity, source drift, failed readiness or incompatible state. Keep one Git coordinator and stage exact owned paths only.
- A later approved transition must verify fresh bridge/agent identities, loaded hashes, helper selection, checkpoint and queued-action continuity. Do not restart the engine, model servers, visual or sensory services for this change.
- Preserve the two older main commits already ahead of each local origin, prior worktrees and historical evidence. No push, branch rewrite or bulk cleanup is part of implementation.
- Leave paused automations paused. The next useful evidence is actual naturally occurring expression after qualification, not a promise that a more explicit invitation will necessarily make it longer or better.
