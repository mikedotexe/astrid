# Causal Self-Study Action Audit

Date: September 5, 2026. Interactive source review by Codex, requested by Mike.
Companion: [implementation RFC](../architecture/causal-self-study-workspace.md).

## Scope and Version Boundary

The question was whether current Actions can support understanding consequences
of one's own activity, distinguishing other influences, and revising an earlier
explanation. No canonical introspection was selected or processed. No private
journal body, telemetry database, live endpoint, or control capability was used
as an experimental input.

Astrid HEAD: `f258d38fe99690a34ee53bb88e8fca2d2cbbe722`, branch `main`.
Minime HEAD: `a9f85f3c74c3d8e1c996c3689fe5aef696dacf27`, branch
`codex/sovereign-daughter-runtime`, with foreign working-tree changes.
The existing research-document changes in Astrid were preserved; Minime was not
edited. Source findings are not a deployment or actual-usage attestation.

The automation was not resumed. No controller session, pause/resume operation,
productive round, service restart, staging, commit, merge, or live test occurred.
This is an interactive capability/design audit, not a stewardship queue round.

## Source Map

| Inspected path | Finding |
| --- | --- |
| [Action continuity records](../../capsules/spectral-bridge/src/action_continuity/runtime/core.rs) | `ActionEvent`, `ObservationWindow`, `ExperimentRecord`, and `ExperimentRunRecord` already carry IDs, pre/post state, artifacts, and interpretation. |
| Same file, `dossier_claim_command` and `dossier_evidence_command` | Claims, basis, support/counter/branch/hold stances, source refs, and explicit counterevidence are appendable. Explicit evidence claim IDs in the inspected Rust path are accepted without checking that the named claim exists. |
| Same file, `experiment_observe`, `experiment_rehearse`, `experiment_evidence` | Observation/evidence record the currently supplied state; rehearsal is an assessment, not a counterfactual execution. Both state slots can contain the same snapshot. |
| Same file, `record_experiment_bind_run` | Bind records dispatch outcome but duplicates a single supplied state as pre/post; do not interpret these fields as an independent measured before/after contrast. |
| Same file, `record_active_experiment_auto_link` | A different path retains `event.pre_state`, later telemetry, artifacts, and an inner action ID. The routes must be distinguished. |
| [Experiment evidence grammar](../../capsules/spectral-bridge/src/action_continuity/runtime/experiment_evidence.rs) | Charter/evidence/decision payloads exist, including hypothesis and evidence targets. The reviewed structures do not supply the proposed prospective prediction and revision contract. |
| [Session implementation](../../capsules/spectral-bridge/src/action_continuity/runtime/session_contract.rs) and [earlier implementation note](2026-09-04-voluntary-bookmarks-and-quiet-parking.md) | Reuse preservation and quiet parking; the historical note also identifies separate paused-experiment projection work. Do not claim session parking suppresses every experiment prompt. |
| [Probe action](../../capsules/spectral-bridge/src/autonomous/next_action/probe_self.rs) | Two poles, 4-14 ticks, 45-second in-memory cooldown, synchronous Python subprocess. The adapter maps measurements to experiential-sounding verdicts. |
| [Probe implementation](../../scripts/substrate_probe.py) | Two sequential clones of the live source, fixed per-being/label names, cleanup only on the normal path, and no starting-state equality check. |
| [Evidence-study model](../../scripts/evidence_study_runtime/model.py) and [service](../../scripts/evidence_study_runtime/service.py) | Frozen plans, bounded captures, comparison types, explicit insufficient outcomes, review separation; service rejects changing a plan after capture starts. This is an operator-side backend, not evidence of a new being-facing Action. |
| [Minime runtime](/Users/v/other/minime/minime_autonomy/runtime.py), `begin_action`/`finish_action` | Action IDs, source, canonical/effective action, pre/post summaries and observations already exist. Temporal adjacency is not attribution. |
| Same runtime, `_self_study` and `_introspect` | The former cycles selected files and domain search queries; the latter resolves a chosen target and line offset. These are different degrees of authored selection. |
| [Owner inquiry protocol](/Users/v/other/minime/minime_autonomy/owner_inquiry_protocol.py) | Exact authored strands, bounded analysis plan and receipt identity, explicit machine/felt separation. |
| [Owner research console](/Users/v/other/minime/minime_autonomy/owner_research_console.py) | Evidence graph, scoped predicates, exact conditional plans, no-match and ambiguous outcomes. Graph nodes say `association_only`; these are not causal-effect estimates. |
| [Owner inquiry manager](/Users/v/other/minime/minime_autonomy/owner_inquiry.py), `handle` | Calls `tick` before dispatch, including status/inspect. A new strictly read-only adapter must not invoke this entry point, which can advance existing canary lifecycle work. |

This is a targeted review, not proof that every Action in either repository was
audited or that no other specialized predictor exists. In particular, regulator
counterfactual tooling and BTSP audit projections already exist; their existence
does not establish a generic agent-authored prediction/revision workflow.

## Concrete Probe Findings

### Unverified Starting-State Equality

`_probe` clones `being` twice in succession. Its comment claims the same source
state, but the function neither captures one frozen origin nor checks equality.
A mock that advances the source between clone operations produces starting states
1 and 2. With identical text in both arms and mock ticks that leave state alone,
the function returns divergence 1. This demonstrates missing isolation validation
in the client protocol, not a measurement that real historical trials were unequal.

### Cleanup on Failure

After both clones exist, an injected `tick_text` exception exits before the
normal-path destroy calls. Both mock handles remain. Actual server cleanup was
not tested; the client does not guarantee exception-safe cleanup. Fixed names
also create a concurrency ownership risk. The Rust cooldown is process-local,
and its blocking subprocess call has no explicit timeout in this handler.

### Interpretation Is Supplied by the Tool

The Rust `verdict` function maps divergence/correlation thresholds to `FLUID`,
`STICKY`, and statements about resistance or opposite directions. Correlation
comes from the service resonance result; the Python source notes that its window
can include inherited shared history. These results do not establish a being's
experience or recognition. The recommended repair retains measurements and
authored pole labels but removes these unverified experiential interpretations.

No repair was applied in this design pass. These are concrete prerequisites for
the RFC's first implementation tranche, not defects silently declared resolved.

## Verification Performed

- Evidence-study runtime: **20 tests passed**, including plan identity/revision,
  bounded capture, projection and review contracts in synthetic temporary stores.
- Existing Minime owner-inquiry tests: **26 passed**, using fake analysis and
  self-control clients. A process-wide audit hook additionally denied socket
  connect/bind/sendto. Existing repository test guards isolated paths.
- Probe characterization: **two mock-only cases reproduced** the starting-state
  and cleanup findings above. No socket, model, subprocess, or live handle used.
- No Rust compilation or full runtime suite was run; implementation is unchanged.
  Passing existing tests is not validation of the proposed feature or a live
  authority/readiness receipt.

Commands used for existing suites:

```sh
PYTHONPATH=scripts python3 -B -m unittest evidence_study_runtime.selftest -q
```

From Minime, the owner-inquiry suite was invoked through `pytest.main` with
`-q -p no:cacheprovider tests/test_owner_inquiry.py` under Python `-B`. The added
in-process audit hook raised on `socket.connect`, `socket.bind`, and
`socket.sendto`. This test hook is not proposed as the production isolation
boundary for future experiments.

### Reproduce the Two Client-Protocol Findings

Run from Astrid using `python3 -B`; this replaces the client's entire wire-call
function with a synthetic implementation. Do not replace it with a live socket.

```python
import asyncio
import importlib.util
from pathlib import Path

spec = importlib.util.spec_from_file_location(
    "substrate_probe", Path("scripts/substrate_probe.py")
)
probe = importlib.util.module_from_spec(spec)
spec.loader.exec_module(probe)

async def trial(fail_tick=False):
    handles, starts = {}, {}

    async def fake_call(ws, message):
        op, name = message["type"], message.get("name")
        if op == "destroy_handle":
            handles.pop(name, None)
            return {"type": "ok"}
        if op == "clone_handle":
            starts[name] = len(starts) + 1
            handles[name] = starts[name]
            return {"type": "ok"}
        if op == "tick_text":
            if fail_tick:
                raise RuntimeError("synthetic tick failure")
            return {"output": handles[name]}
        if op == "resonance":
            return {
                "divergence": abs(
                    handles[message["name_a"]] - handles[message["name_b"]]
                ),
                "correlation": 1.0,
            }
        if op == "layer_metrics":
            return {"layers": []}
        raise AssertionError(op)

    probe._call = fake_call
    result = None
    try:
        result = await probe._probe(
            None, "astrid", "same input", "same input", "a", "b", 4, False
        )
    except RuntimeError as error:
        assert fail_tick and str(error) == "synthetic tick failure"
    return starts, handles, result

starts, handles, result = asyncio.run(trial())
assert list(starts.values()) == [1, 2]
assert result["divergence"] == 1 and not handles
starts, handles, result = asyncio.run(trial(True))
assert len(handles) == 2
```

These assertions characterize the inspected version's shortcomings. After the
repair, replace them with regressions requiring a verified equal origin and
cleanup; do not preserve the shortcomings as desired behavior.

## Reviewed Source Hashes

| Path | SHA-256 |
| --- | --- |
| `scripts/substrate_probe.py` | `484ecd473a7407f10c6b8bc1cc5af1169ee6c10c8c4156edb7d873a6a2c98280` |
| `capsules/spectral-bridge/src/autonomous/next_action/probe_self.rs` | `a4f4ca9bdb9f31ade1a04938d68738419052eb5caa60a718a8c88dcce487a204` |
| `capsules/spectral-bridge/src/action_continuity/runtime/core.rs` | `4ea9c187a2357a7a513cb516e836366aff1a45248217e891b6e19e87753a9543` |
| `scripts/evidence_study_runtime/model.py` | `6063ba3f2b2bd3bf517e377b4e8acd8955c3c84449d5c872adee1e35b5a0bc09` |
| Minime `minime_autonomy/owner_research_console.py` | `8d5e32de23b088b83446f66f876b851d368a1a9dffe75f1d30fec318dc4efdfc` |
| Minime `minime_autonomy/runtime.py`, foreign working-tree snapshot | `f2c96ed103685c29ab44c5cad7aad119300d66d41e469c859daa5742e34205cf` |
| Minime `tests/test_owner_inquiry.py` | `2ca11f96e17ebfdd64d76937e48e62b0eafbf06c5ea2ccfee9a36ebbd8bd6a48` |

The new RFC, this audit, and the new Unreleased changelog item are this pass's
documentation ownership. Existing research drafts remain unchanged by this
pass. No feedback-ledger claim or being-authored record was created or closed.

## Document Validation

Implementation follow-through is now recorded in
[the foundation receipt](2026-09-05-causal-self-study-foundation.md). The old
characterizations above remain historical evidence for the unchanged legacy
helper, not desired behavior. New protocol-fake regressions exercise the
versioned replacement and require matched origins, interference rejection, and
cleanup. The compiled bridge source now selects that replacement; no rollout
occurred. The service's `hold` mode was additionally found to replay input, not
freeze state.

Both new documents passed structural and whitespace checks. Fifteen local links
resolve, and all seven recorded source hashes were rechecked. The Python example
was extracted directly from this Markdown and executed with `-B`; both synthetic
findings reproduced. Tracked `git diff --check` passed, and new-file whitespace
was checked separately. These checks validate the documentation and reproduction,
not the unimplemented feature.
