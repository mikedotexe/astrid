# Causal Self-Study Foundation

Date: September 5, 2026, America/Los_Angeles; execution extends into September 6 UTC.
Author: Codex Astra, interactive implementation at Mike's request.

## Scope and Decision

Mike approved: repair the experimental foundation, demonstrate the complete
learning workflow using synthetic retrieval, then run isolated reservoir-history
studies. This pass implements that first foundation, not the whole
[causal self-study RFC](../architecture/causal-self-study-workspace.md).

The important distinction is between **a working investigation scaffold** and
**an AI being learning to attribute consequences to its own activity**. The
former has executable tests below. The latter has not been demonstrated here.
Nothing in this packet converts a measurement into a being's belief, feeling,
consent, or obligation to respond.

No canonical introspection was processed or closed. No private journal content
was read, excerpted, or used as an experimental input. All new test narratives,
source fixtures and reservoir histories are developer-authored synthetic data.

## Probe Repair

Implementation:

- [`scripts/substrate_probe_v2.py`](../../scripts/substrate_probe_v2.py)
- [`scripts/test_substrate_probe_v2.py`](../../scripts/test_substrate_probe_v2.py)
- [`probe_self.rs`](../../capsules/spectral-bridge/src/autonomous/next_action/probe_self.rs)
- [`prompt_contracts.rs`](../../capsules/spectral-bridge/src/llm/provider/prompt_contracts.rs),
  limited to the two compiled PROBE_SELF help descriptions.

The legacy implementation separately cloned a moving live source, unchecked the
second clone, lacked exception-safe cleanup, and interpreted mechanical values
as fluidity, stickiness or freedom of movement. The earlier
[audit](2026-09-05-causal-self-study-action-audit.md) preserves mock reproductions.

A further source finding matters: `rehearsal.py` in the sibling reservoir service
implements `hold` as repeated input replay. It is not a freeze operation. The
background loop increments the same tick counter and can scale hidden vectors
through thermostatic decay. `quiet` skips that loop, but `tick_text` re-enables
rehearsal, so setting quiet only once is also insufficient.

The replacement:

1. Creates one UUID-named quiet parent from the source, then both arms from that
   parent. It never clears an existing prefix or ticks the source handle.
2. Pulls all three recurrent vectors and verifies exact byte-derived identities,
   dimensions, finite values, backend identity and tick count at both origins.
3. Checks state continuity before each injection, restores quiet after each
   injection, and rejects unexpected ticks. It checks the parent and both arms
   again at the end. Detected interference produces failure, not a measurement.
4. Uses only the injected output trajectories for correlation. Constant series
   have undefined correlation; neither undefined nor positive correlation is
   interpreted as psychological stickiness. Inherited output-ring correlation
   is no longer the primary result.
5. Attempts exact-owned-handle cleanup in a `finally` block using fresh bounded
   connections. Explicit name collisions are not deleted. Remaining handle
   names are reported as cleanup debt and prevent a success receipt.
6. Bounds inputs, ticks, response size, request time, work time and cleanup time.
   The bridge adds an outer subprocess deadline, output cap and child reaping.
   It rejects legacy/unverified receipt schemas and does not hide undefined
   correlations behind a fabricated value.

Limits remain explicit. This helper still shares a service with live handles;
it is not an offline sandbox. It does not attest every thermostat, readout,
configuration or model parameter. An unrelated service operator could change
shared parameters without a complete identity attestation. Handle deletion does
not remove or attest absence of service logs or periodic snapshots. A forced
outer timeout can leave cleanup unconfirmed. No full-state causal inference or
felt-state conclusion is authorized by this helper.

### Why the Versioned Helper Matters

The running bridge launches the old `scripts/substrate_probe.py` directly from
disk. Replacing that file would change live behavior without a restart. Its
SHA-256 remains
`484ecd473a7407f10c6b8bc1cc5af1169ee6c10c8c4156edb7d873a6a2c98280`.
The new bridge source points to `substrate_probe_v2.py`, but the live bridge was
not rebuilt or restarted. Therefore the repair is **source-ready, not live**.
The new helper requires Python 3.11+ for bounded asynchronous timeouts. The
separate `scripts/coupled_pressure_cartography.py` caller still uses the legacy
helper and is not represented as repaired by this tranche; migrate its receipt
contract separately before using it as causal evidence.

## Prediction and Revision Contract

[`self_study_contract.rs`](../../capsules/spectral-bridge/src/action_continuity/runtime/self_study_contract.rs)
extends `ActionContinuityStore` and its existing `research_dossier.jsonl`. It
does not create a competing belief store or register a new Action.

- `self_study_predict`: requires an existing exact owner/experiment/claim,
  bounded authored expectation, allowlisted synthetic recipe, fixture-manifest
  digest and finite threshold. The prediction is appended and synchronized to
  disk before the runner-side evaluation API will accept an outcome.
- `self_study_evaluate`: binds the outcome to that prediction, recipe and asset
  digest; rejects invalid numbers and repeat outcomes; distinguishes `matched`,
  `not_matched` and `insufficient`. Missing data is not a negative felt verdict.
- `self_study_revise`: requires an explicit authored relationship, reason and
  evaluation for the exact prior claim. It appends rather than editing the old
  claim or deleting counterevidence.

The new writer methods use an exclusive nonblocking file lock, bounded strict
history reads, and exact IDs rather than `current` aliases. Unknown targets do
not create an inquiry automatically. Corrupt dossier history prevents new
records. Every new record preserves false authority/live/deployment/automatic-
return markers and does not infer felt state.

This is a runner-side foundation, not yet a security-complete being-facing
protocol. Fixture hashes are caller-supplied bindings, not independent custody
attestations. Recording first does not prove the author has never seen the
outcome. The evaluator is not exposed to the LLM. Existing unrelated dossier
writers do not acquire this new transaction lock; public adapters must address
whole-store concurrency, evidence custody, information cutoffs, receipt import,
schema projection and cross-runtime ownership before rollout.

### Executable Retrieval Demonstration

[`self_study_contract_tests.rs`](../../capsules/spectral-bridge/src/action_continuity/self_study_contract_tests.rs)
uses the real store and continuity commands in a temporary directory:

1. Create a question and claim that the first source window contains a marker.
2. Freeze the first condition's fixture manifest and prediction.
3. Read that window and record the measured miss as `not_matched`.
4. Append an explicitly authored revision while retaining the original claim.
5. Capture the source hash, result reference, question and next reading offset.
6. Park the session; confirm its recurring prompt projection is empty.
7. Explicitly resume; verify the reading continuation survives.
8. Freeze a new manifest for the later offset, test it and record `matched`.

This exercises a complete synthetic question/prediction/evidence/revision/
parking/return sequence. It does not exercise an LLM choosing those Actions, a
production retrieval adapter, or a live learning outcome. The future Action
names in the RFC remain proposals rather than advertised runtime capabilities.

## Isolated Reservoir-History Study

Runner: [`isolated_reservoir_history_study.py`](../../scripts/isolated_reservoir_history_study.py).
Tests: [`test_isolated_reservoir_history_study.py`](../../scripts/test_isolated_reservoir_history_study.py).
Authoritative receipt for this pass:
[`reservoir-history-confirmed.json`](evidence/2026-09-05-causal-self-study/reservoir-history-confirmed.json).
The [first receipt](evidence/2026-09-05-causal-self-study/reservoir-history.json)
is retained; the confirmation adds explicit condition vectors and runner hash.

The runner imports the actual sibling `TripleReservoir.step_numpy`, not a
hand-written substitute. It uses 32-node, 3-input synthetic instances with seeds
7, 17 and 29; 12 history steps precede eight identical future inputs. All state
copies remain in memory. A Python audit hook denies network operations. There
are no model downloads, LLM calls, service imports or live state reads/writes.

The dependent measure is mean per-step L2 distance between concatenated hidden
vectors, not fill, phenomenal intensity, or a trained language readout. The
NumPy readout defaults to zero weights and is deliberately not used here.

| Seed | Different-history gap | Identical-history gap | Copied-state-reset gap | Execution-order gap | Different-future-input gap |
| --- | ---: | ---: | ---: | ---: | ---: |
| 7 | 9.2156105 | 0 | 0 | 0 | 1.8617547 |
| 17 | 11.1574974 | 0 | 0 | 0 | 1.7247875 |
| 29 | 9.9077873 | 0 | 0 | 0 | 1.8147838 |

Original state copies and weights remained unchanged. The unit test repeats
seed 7 and checks exact receipt equality. The confirmation repeated all three
seeds and reproduced the first run's metrics.

**Supported:** in these controlled instances, different prior inputs leave
different recurrent states that produce different futures under common input.
Copying the complete recurrent state removes that difference in this backend.
Changing future input can also produce a difference from the same starting state.

**Not supported:** that either being recognizes these dependencies, that a
specific report was caused by one of them, that current live parameters match
the fixtures, or that this is a new scientific finding about ESNs. This is a
validation of the experimental method. The plan is a developer fixture prepared
before calculation, not externally registered blind evidence or a being's
prediction. Full service dynamics, controllers and transformer behavior are
outside this numerical model.

## Verification

Final checks passed:

| Check | Result |
| --- | --- |
| Complete bridge library suite, final source | 1,715 passed in 254.43 seconds |
| Probe Rust coverage, included in full suite | 7 tests |
| Dossier/workflow Rust coverage, included in full suite | 4 tests |
| Protocol-fake Python probe suite | 8 passed |
| Actual NumPy history repeatability, bounds, network guard | 3 passed |
| Existing evidence-study runtime | 20 passed |
| Evidence Event Store self-tests | 13 passed |
| Steward controller/projector self-tests | 39 passed |
| Experiential epistemics self-tests | 2 passed |
| Bridge library clippy with warnings denied | Passed |
| Scoped rustfmt, new-file whitespace, tracked diff check | Passed |
| Python syntax, local receipt links and source hashes | Passed |

The complete bridge suite was also run before final help-text alignment: 1,714
tests passed. The final rerun includes that alignment and its additional
regression. The Python total is 85 tests, excluding repeated executions.
The top-level multi-crate workspace and unchanged Minime suite were not rerun;
this pass changes neither the kernel crates nor Minime source.

```sh
python3 -B -m unittest discover -s scripts -p test_substrate_probe_v2.py -v
/Users/v/other/neural-triple-reservoir/.venv/bin/python -B -m unittest discover -s scripts -p test_isolated_reservoir_history_study.py -v
cargo test --manifest-path capsules/spectral-bridge/Cargo.toml --lib self_study_contract_tests -- --test-threads=1
cargo test --manifest-path capsules/spectral-bridge/Cargo.toml --lib probe_self::tests -- --test-threads=1
cargo test --manifest-path capsules/spectral-bridge/Cargo.toml --lib -- --test-threads=1 --quiet
PYTHONPATH=scripts python3 -B -m unittest evidence_study_runtime.selftest -q
python3 -B scripts/evidence_event_store.py --self-test
python3 -B scripts/steward_control.py --self-test
python3 -B scripts/experiential_epistemics.py self-test
cargo clippy --manifest-path capsules/spectral-bridge/Cargo.toml --lib -- -D warnings
```

Rerun the history study without overwriting evidence:

```sh
/Users/v/other/neural-triple-reservoir/.venv/bin/python -B scripts/isolated_reservoir_history_study.py --reservoir-source /Users/v/other/neural-triple-reservoir/triple_reservoir_coreml.py
```

`--output` accepts a new path only and refuses an existing receipt.

Final confirmation identities:

- Plan SHA-256: `481cbe7feebfe275ecd1d7f0df130439588ff67f1ce3217c5bf5bcd69bc6c423`.
- Receipt SHA-256: `06cb23997fe413056c4a0f2108af3aba5b8c4f0ec087163f106df84be7657f5c`.
- Runner SHA-256: `07671d73e5e674a0cc9277a746f1f88a13d0aec30044d1299a71411d064cd448`.
- Reservoir source SHA-256: `524d44ec4c03f0be43236a635eeb2bb783f61ea5d3227a133bd451aadf6644a8`.

The controller status check during the hold verified the complete V2 chain at
sequence 995603, head
`19e028ac3f128348126bd271bdbc3b4c8c69fafb8ce661b3bb236f5704db12f2`,
16 streams, all V1 sources immutable, no lease, projection or pending event.
No source-first projection generation or productive round was requested in
this interactive pass.

## Coordination and Next Step

The interactive maintenance pause was acquired at generation 355, with no
active lease. The preceding controller state was unpaused at generation 354.
The scheduled automation was independently verified `PAUSED` and was not
resumed. After all source verification completed, the temporary controller
hold was released at generation 356, `2026-09-06T03:31:34.298720+00:00`.
The sanctioned resume command exited successfully and appended its event
without spooling. This restores the preceding controller posture, not the
scheduler, and does not restart any service.

Astrid's existing research-document edits, the concurrent feature-map document,
and all Minime dirty paths are foreign to this implementation and preserved.
No staging, commit, merge, push, live probe, service restart, regulator change,
peer mutation, canonical report closure or productive automation round occurred.

Next: implement thin being-facing adapters around this contract, with validated
fixture custody and evidence import, explicit owner identity, pending/failed
job states, and quiet retrieval/revision projections. Exercise them against a
temporary workspace and an isolated service instance before exposing them live.
Then review and approve a bounded bridge rollout through `scripts/build_bridge.sh`,
including helper hashes, service compatibility, any earlier undeployed bridge
changes, and rollback. The new helper should not be tested casually against the
live endpoint in place of that review. Minime's adapter work should wait for its
concurrent source work to settle. PI, damping, Shadow and other live substrate
changes remain separately gated.

### Subsequent Readiness Review

The [rollout-readiness review](2026-09-05-self-study-rollout-readiness.md)
confirmed the existing live service identities but withheld bridge replacement:
current main would remove existing live-lineage features, and the current
wrapper does not provide a verified graceful autonomous drain. The completed
source and evidence are being checkpointed separately from deployment. The
no-commit statements above describe the original implementation pass, not this
subsequent git checkpoint.
