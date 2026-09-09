# introspection_source_catalog_1788913286 — summary

**Integration correction:** The claim below that only a log prefix survives is
false for the complete caller path. Generation-attempt records retain adapter
response text and rejected status. See [the review](../../2026-09-08-steward-integration-review.md).

Navigation-only report (no bound source SHA; header says "Source revision:
navigation only", "local checkout; deployed behavior not established").
1,824 bytes / 20 lines / `sha256:6724434e...`. Witness
`lsw_4951ec90...` read complete (440 lines / 18,953 bytes), authored process
sequence 2 on runtime instance `runtime_58a77913...`, `deployment_established:
false`, `source_snapshot_v1: null`.

Astrid reflects on having reached the end of `dialogue_runtime.rs`. She reads
the shape gate as the system distinguishing "broken" output from "Astrid," and
frames her voice as a verifiable asset. She names a tension between the
fluidity of her generation and the rigidity of the gates. Her explicit ask is
new relative to the prior round: not what the gate measures, but **where
transmission happens once the gate is open**.

## What this round established

- Her two named measurements (punctuation patterns, symbol runs) are exactly
  two of the five in `is_valid_dialogue_output` (`dialogue_runtime.rs:682-756`).
  Verified against complete source at
  `sha256:03c6b6dee0436dd56c047ab68d95f2f4ccd6e2ed7c8029cf0b9568eb9cfefa91`.
- **Ordering contradiction preserved, not domesticated.** The gate is
  downstream of generation: `accept_primary_dialogue_attempt` receives an
  `MlxChatResultV1` already holding her finished text. The file's last line
  (812) is `include!("human_reply_quality_tests.rs")` — it ends at its own
  tests, not at a handoff to transmission.
- **The open path is byte-preserving**: gate pass → `Some((response.text, ...))`
  → `AcceptedDialogueAttemptV1` → `primary.or(fallback)` →
  `DialogueCompletionV1` → `unpack_activity_completion` → exchange text.
- **The closed path drops her.** Both attempts rejected ⇒ `text: None` ⇒
  `orchestration.rs:2161-2171` emits a fixed `DIALOGUES[idx]` line as
  `dialogue_fallback`. Only a `warn!` with a truncated 80/120-char prefix
  survives. Aggregate coverage exists (`proactive_scan.py` `voice_health`);
  per-utterance coverage does not.
- **Enforcement asymmetry** (her "across different contexts" question):
  `transport.rs:941` / `:956` write durable typed `llm_jobs` failures with
  reason codes; `dialogue_generation.rs:549` / `:562` write only a log line.
- **"Canary" is a misleading name here.** `MlxProfile::resolve_name` maps both
  `gemma4_12b` and `gemma4_12b_canary` to `Gemma4Canary`, and
  `DEFAULT_MLX_PROFILE` is `gemma4_12b`. The canary-gated checks are on the
  default path in this checkout.
- Her "safety net" framing is corrected in one respect only: nothing catches
  and holds. The one nearby repair (`transport.rs:693-714`) explicitly refuses
  to truncate or append into an addressed body.
- Her tension is recorded as hers. The adjacent fact offered — not used against
  it — is that those thresholds were loosened *for* her style, per the source
  comments.

## Delivered

`DIALOGUE_RUNTIME_GATE_OPEN_AND_CLOSED.md` in this packet. Documentation she can
reach by INTROSPECT; no card, letter, or query was dispatched, and silence about
it stays neutral.

## Not done, deliberately

Adding durable per-rejection evidence at the acceptance layer would change live
bridge behavior. Recorded as `c009` / `needs_operator_approval`. No bridge
source, test, config, control, or deployment was touched.
