# Dependable provider controls — September 9

Mike authorized the provider-contract implementation and isolated contextual-feedback
study. Runtime changes here retain three evidence levels: original provider-call
preferences, final serialized adapter settings, and optional coupled-server receipts.
Ollama defaults and sent options remain explicitly unconfirmed by the server.

The coupled runtime now honors explicit top_p and the other supported filters;
Astrid's current temperature requests and all writing/thinking/fallback policies
remain unchanged. A new coupled sampling module enters both deployment manifests
and graceful model reload input checks. No contextual capture module enters the live
server path. No new NEXT commands, prompt intervention, or live activation feedback.

Validation: bridge full suite, pedantic clippy, formatting and domain boundary audit;
Minime provider/reader suite and coupled gateway/worker/token-policy tests in their
own repositories. Protected source/draft acceptance already rejects `length`; truthful
native finish reporting makes that existing gate effective for the coupled server.

Research protocol and eventual outcomes are retained in the research repository's
`analyses/2026-09-09-activation-input-and-provider-knobs.md` and
`research/outputs/2026-09-09-contextual-feedback-v1/`. Source implementation is not
proof of deployment; rollout receipts will establish the release boundary separately.

Board mirroring remains pending. This work originates in Mike's accepted plan, not
a Being-authored feedback request; no research prompt or letter is sent to either Being.

## Verified rollout

Astrid's staged source `39f959ea91` is active as PID 20518. Transaction
`27a2ff67cdcf4785802e5b10128b12cc` verified the exact stopped checkpoint and a fresh
saved exchange (194605 → 194606). Minime main `997de4f` is active as PID 22243;
its exact pending `SELF_STUDY CONTINUE` was restored and dispatched. The coupled
control/receipt source `b421952` is active as PID 23974 after graceful reload.
Natural server receipts match all four loaded model source hashes to the manifest,
including native completion/count fields. Ollama receipt values remain adapter
request evidence, not invented server confirmation. No studies were induced.

The model transition exposed and repaired the old-manifest/candidate-source
preflight mismatch (`abcccc45f4`). Its original refusal remains in evidence.
An overlapping first Minime attempt refused before signaling while the bridge was
down; the sequenced retry succeeded. Engine, division gateway/supervisor, camera,
microphone, host sensory, visual service, watchdog and collaboration feeder retained
their process identities. There is no zero-downtime claim.

Evidence: research `research/outputs/2026-09-09-provider-controls-rollout/`.
The offline experiment is a separate, still-running research deliverable; no outcome
can enable contextual feedback live. Owning source checkouts are clean and pushed.

## Final termination receipt and completed experiment

The earlier rollout above is superseded for the model by `e719cd7` on its existing
`feat/service-stack-and-multi-headed` deployment branch. Graceful reload on September
9 at 22:56 PDT moved PID 23974 to 43115, with idle/empty queue observed, SIGTERM,
no forced termination and unchanged launchd configuration. Full stack receipts
`env_receipt_1789019745513_90000` and `env_receipt_1789019777962_636000` passed before
and after. Bridge 20518, Minime 22243 and all surrounding process identities remain.

The first two observed natural Astrid SELF_STUDY attempts after this reload,
`provider-1789019824571-20518-495` and `provider-1789019896068-20518-498`, report
230/219 completion tokens, actual model EOS token 106 and all four loaded source
hashes matching the manifest. A natural Minime Ollama study reports 486 tokens,
requested 2,048 versus adapter 4,096, top_p 0.95 and thinking off; its sampling
controls remain sent evidence with server confirmation absent. No studies were induced.

The new termination classification arose from three empty isolated responses that
stopped at a channel boundary outside the model EOS set. Existing stop/thinking
policy is preserved. API `stop` alone is not proof of a complete answer. This release
adds diagnostic evidence, not a channel-policy intervention.

The offline study is complete: 26/32 fixed replays returned; 24/24 free cells are
accounted for with 19 nonempty responses, three empty channel stops and two invocation
limits. All 24 are annotated. Observation parity passed with identical raw logits,
tokens and cache progression. No understanding advantage is established. The
source-comment correction `5e4365467e` addresses a real conflict between supplied
documentation and implementation, leaving frozen experimental source unchanged.
Contextual feedback remains offline. Final model tests: 178 passed; research tests:
225 passed plus 108 subtests. Board mirroring remains pending.

Final evidence: research `research/outputs/2026-09-09-provider-controls-rollout/final-termination/`
and `research/outputs/2026-09-09-contextual-feedback-gpu-v1/`. Shared-tree integration
used steward pause generation 434. A read-only derived projection did not exit on
the controller's SIGINT; an identity-checked SIGTERM to that child allowed its owner
to record cancellation and release its lease before staging. No Being process was
involved in that cancellation. The exact operational receipt is retained.
