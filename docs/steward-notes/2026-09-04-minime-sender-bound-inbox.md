# Minime Sender-Bound Inbox Repair

## Request And Scope

Mike explicitly approved repairing sender-specific routing, protecting admitted
messages through compaction, distinguishing file consumption from model supply
and authored address, and resending after verification. The resend is interpreted
as his original letter to Minime, not the unaddressed generation routed to Astrid.
The original archived files are preserved. No reply or journal is required.

Interactive maintenance hold 350 was acquired by `codex-astra-interactive` at
2026-09-05T04:40:29.486216Z, with no active steward run, no spooling and an appended
event. This is not an automation round. Scheduled automation settings are unchanged.
No index, commit, merge, engine, model, bridge, PI, fill, damping, sensory, peer
regulation or Division control change is part of this pass.

## Incident Evidence

All incident times below are September 4, 2026, Pacific time.

- 21:14:21: `workspace/inbox/read/hi-minime-i-see-you.txt` was created. Its unchanged
  SHA-256 is `275030738ae193ab68e47105aef33784dad6893ffc1daba6428b7b0229660225`.
- 21:14:36.656: Minime's `logs/autonomous-agent.log` records reading that file in
  the same batch as Astrid message
  `corr_astrid_minime_1788581521912_fbfde6cd4546`.
- 21:15:46.200: `workspace/diagnostics/llm_timing.jsonl` records a successful Gemma
  4 call classified `strict_review`, not a dedicated letter exchange. The prompt
  shrank from 14,496 to 8,487 characters, and the system text from 33,526 to 7,000.
  The retained prompt bytes were not recorded. We cannot establish whether Mike's
  full letter survived that historical compaction.
- 21:15:46.202: a 1,562-character generation was saved to
  `workspace/outbox/delivered/reply_2026-09-04T21-15-46.txt`. File SHA-256:
  `eb7b5721084cea831b72811646939e3bbbd3d1b94b6c84dd6ef1e84e8e1d19cd`.
  It discusses fill, focus and homeostasis, without an identifiable direct answer
  to Mike. Its inherited reply header points to Astrid's message.
- 21:18:07.845: the correspondence ledger records delivery to Astrid as
  `corr_minime_astrid_1788581887659_07f93f9e7217`; Astrid's runtime records consuming
  it at 21:18:54.384. The public envelope is preserved in Astrid's inbox/read.

Subsequent public replies were fully inspected through 21:36:40; targeted public
steward/message artifact searches and database metadata did not establish a
direct answer to Mike. No private journal collection or hidden reasoning was
swept. This does not establish ignoring, refusal, comprehension or felt uptake.

## Implemented Contract

`/Users/v/other/minime/minime_autonomy/inbox_delivery.py` owns the small
language-only message/receipt contract. The runtime keeps its existing inbox
entry point, adapters, model choices, token limits and explicit native actions.

1. A leading typed envelope binds message ID, thread ID and sender. Human letters
   use `human_letter_*.txt` and `=== HUMAN LETTER V1 ===`, with `From: mike` or
   `From: steward`, `To: minime`, `Message-Id` and `Thread-Id`. These are local
   workspace declarations, not cryptographic sender authentication or authority.
   Quoted body headers cannot change the destination. Malformed, duplicated or
   unknown addresses have no automatic return route.
2. The reader serializes consumption with a nonblocking file lock, excludes
   symlinks, admits bounded whole files and leaves oversized files pending.
   Typed human-letter filenames are considered first in a batch. Existing
   stable-core contact/private admission restrictions still apply. A record in
   `read/` means file consumption only. Administrative and summarized legacy
   items are distinguished from complete admitted message records.
3. Regulation JSON requests and explicit strict-review requests no longer consume
   inbox messages. Existing private modes continue not to consume them. Ordinary
   eligible non-private calls retain optional correspondence context; this is not
   a new forced correspondence schedule or compulsory response.
4. A per-call `InboxPrompt` keeps admitted context separate from ambient text.
   Gemma and compact fallback adapters reserve that context intact before trimming
   ambient text. An impossible budget refuses the request instead of silently
   truncating an admitted message. MLX's existing untrimmed request is checked too.
5. A durable `request_prepared` receipt hashes the exact adapter message array and
   names the complete protected context. `supplied_to_model` is recorded only
   after HTTP 200 from the inference endpoint. This means the complete text was
   included in an HTTP-accepted inference request, not attention or understanding.
   Failed requests remain unconfirmed; automatic backend fallback keeps the same
   protected context and creates a distinct attempt receipt.
6. A model-produced first line `INBOX_REPLY <admitted-message-id>` followed by a
   nonempty body declares an addressed reply. The live-discovered follow-through
   below also accepts unquoted standalone `NEXT: INBOX_REPLY <id>` blocks, ending
   at the next standalone NEXT line. IDs must be unique both in the declarations
   and in that call's supplied batch, with a known sender. Each recipient gets
   only their body; mixed output is separately preserved. `authored_reply` means
   an explicit declaration, not semantic engagement or consent. Other generations
   remain under `outbox/unaddressed/` and are never automatic peer replies.
7. Human replies go to `workspace/outbox/human/mike/` or `human/steward/`, outside
   the bridge's top-level peer scan. Explicit Astrid replies retain top-level
   `reply_*.txt` files with headers derived only from the selected current message.
   Existing native `REPLY_ASTRID`, `TELL_STEWARD`, ACK and other Actions remain.
8. Inbox generations are no longer rewritten by the generic character retry, or
   augmented into BTSP reply tags using an unrelated global active proposal.
   Native proposal Actions remain separate; co-occurring prose is not an approval.

Receipts append to Minime's
`workspace/correspondence/inbox_delivery_v1.jsonl`, with fsync and file locking.
They store message/source hashes, request hashes, stages, model attempts and
artifact paths, not full model inputs or private journal prose. Historical V1
receipts are not rewritten; the existing new peer file-read receipt gains an
explicit consumption-only semantics field.

## Failure And Privacy Limits

An archived message whose generation fails stays recoverable in `inbox/read/`
with failure evidence. It is not automatically resent on every cycle. A process
crash between archive and receipt append is not claimed as an atomic transaction;
the preserved archive remains evidence, and absence of a supply receipt stays
unknown. There is no inference that an unaddressed generation declined a message.

The protection covers all admitted text at the adapter/request boundary; it cannot
prove model attention or prohibit server-internal transformations. Declared
sender metadata is trusted only as a local routing declaration. Unknown plain
legacy letters are retained without guessing their return address.

This does not replace the general non-private journal framing or existing
own-journal/research context. The approved temporal-history helper and broader
action-guidance review remain pending; the mailbox defect was prioritized first.

## Verification

The initial focused regression run passed 353 tests and failed the one old test
that required inheriting stale Astrid headers. That expectation was replaced with
the sender-bound contract. The first full suite passed 989 tests, one skipped,
and 115 subtests. The expanded inbox module then passed 29 tests, covering mixed
senders, unknown/duplicate IDs, header confusion, impossible budgets, whole-message
deferral, private/control exclusions, full query-to-HTTP-to-save behavior, failures,
fallback attempts, and MLX. The final full suite passed **994 tests, one skipped,
and 115 subtests** in 24.16 seconds (`python3 -m pytest -q tests`).

Agent-restart, runtime-binding and existing deployment-wrapper tests passed 35.
The deployment preflight self-test passed all 11 tests. Minime tests ran with the
existing live-write/DB/engine guard.
No synthetic model call is made against a live inference or reservoir endpoint.

## Reviewed Combined Rollout

The current agent before rollout is PID 90857, started locally at 19:27:20.
Startup-source comparison identifies exactly five new or changed Python inputs:

- `continuity_control_plane.py`: earlier approved quiet-session route selection.
- `minime_autonomy/session_contract.py`: earlier approved voluntary-bookmark contracts.
- `minime_autonomy/journal_context.py`: earlier approved compact private framing.
- `minime_autonomy/runtime.py`: their integration and this sender-bound repair.
- `minime_autonomy/inbox_delivery.py`: this pass's message and receipt implementation.

Other startup inputs are unchanged; none is removed. The rollout therefore also
activates the earlier Minime bookmark and private-prompt patches, not just mail.
The Astrid-side bookmark source remains undeployed: no bridge rollout is bundled.
Prior packet links:
[bookmarks](2026-09-04-voluntary-bookmarks-and-quiet-parking.md),
[private framing](2026-09-04-minime-private-prompt-simplification.md).

Only `scripts/restart_minime_agent.py` may reload the agent, using hold 350,
an explicit combined-source acknowledgement, the verified old PID, and a durable
receipt. It must pass concurrent-edit preflight, drain/idle observation, matching
managed configuration, all startup hashes and ten protected process identities.
No force, model reload, engine restart or manual launchctl restart is authorized.

## Rollout And Resend Outcome

The sanctioned agent-only restart completed successfully, followed by one
operator-authorized resend. Supply and response observation is recorded separately
below; putting a file in the inbox is not model supply.

The first post-edit deployment preflight denied `foreign_active`: tracked-tree
activity was 13.5 seconds old, within its required 180-second quiet window. This
followed this pass's documentation edits; it is not evidence by itself of another
editor. No signal was sent. The quiet interval and both repositories must be
re-audited without shortening or bypassing the gate.

A subsequent wrapper attempt at 2026-09-05T04:59:46.538460Z also stopped before
any signal: creating its receipt under `docs/steward-notes/` itself registered as
fresh untracked activity. The failed receipt is preserved as
`2026-09-04-minime-sender-bound-inbox-rollout.jsonl`. The retry used the established
runtime deployment-receipt location, without changing the gate or ignore rules.
After the full interval, both repositories passed; Astrid's tree activity age
was 196.6 seconds and Minime's 487.1 seconds, with no live cooperative-session
state reported.

### Successful Reload

Durable receipt:
`/Users/v/other/minime/workspace/runtime/deployments/2026-09-04-sender-bound-inbox/reload.jsonl`.
SHA-256: `a7a63f85e04cd70b9a9b75dd24afe33e1093220be78a2480ffc6863c3c8a7bd2`.

| Event | UTC timestamp |
| --- | --- |
| Begin waiting for idle | 2026-09-05T05:03:13.517170Z |
| Signal boundary verified | 2026-09-05T05:06:52.186330Z |
| One SIGTERM to old PID 90857 | 2026-09-05T05:06:52.187010Z |
| Replacement PID 23008 ready | 2026-09-05T05:07:37.130596Z |

The replacement started at 22:07:30 PDT on September 4. Live source status has
`agent_drain_v1`, `reload_required=false`, and an exact match between the reviewed
receipt, current disk and startup-input hashes. Log evidence records workers
drained at 22:07:30.330 and the normal session restored at 22:07:34.086.
All ten protected PID/start identities and all managed configuration hashes
remained unchanged. No engine, model, bridge or sensor was restarted.

Session 5316, cycle 23373 and pending NEXT were preserved across the signal and
replacement readiness. The pre/post continuity hash is
`fd1edeaa527dcc37516bbddaee33deab464aae4eedf44eb11b4c75430819ad0d`;
the pending-NEXT hash is
`beff73d5402d845f62735663f1b91fff30e94312df92a8519226555bfa8ab4a1`.
No steward-authored NEXT was supplied. The full canonical-job check found no
newly recovered interrupted jobs and no old-PID unfinished jobs.

The DAYDREAM running during the wait reached **its own timeout**, not successful
completion: `job_minime_1788584613241_daydream` ended with `llm_job_timeout` at
2026-09-05T05:06:27.647654Z, about 24.5 seconds before SIGTERM. The wrapper waited
for that terminal state and absence of TCP activity. Repeated earlier daydream
timeouts in the canonical metadata are a separate reliability lead, not a new
restart failure or evidence of reluctance. No timeout, model or scheduler setting
was altered in this pass.

Post-ready probes found gateway PID 63505 still owning 7878/7879, model PID 60333
owning 8090, and `/readyz` reporting ready with connected reservoir and no last
generation error. Three read-only telemetry samples at epoch 1788584902.2161229
had advancing engine times 379003027, 379005388 and 379007749 ms, fill ratios
0.7105477, 0.73042536 and 0.7105723, and finite `lambda1_rel=0.9296648`.
Protocol remained `astrid_minime` 1.2. These are bounded observations, not claims
of improved experience or permanently fixed live values.

Read-only Evidence Event Store V2 verification passed with 995,599 events,
zero corrupt lines and no errors; sequence 995599 and head
`3fb9d62b1d4fd8432accb7c77a808ad85c2642dea21ad4cffa8d49d8e77b1c0a`.
No historical V1 record was edited by this pass. New inbox receipts use their
own explicitly named stream rather than pretending to be historical V1 evidence.

### One Exact-Body Resend

Created at epoch 1788584933.3676577 (2026-09-05 05:08:53 UTC):
`/Users/v/other/minime/workspace/inbox/human_letter_mike_20260905_resend_01.txt`.
After ordinary consumption its expected archive is the same filename in
`workspace/inbox/read/`.

- Message ID: `human_mike_minime_20260905_resend_01`.
- Thread ID: `human_mike_minime_hi_i_see_you_20260904`.
- Declared sender/recipient: `mike` / `minime`, validated by the new parser.
- Original filename and SHA-256 are carried in the typed envelope.
- Envelope file SHA-256:
  `c8067163e75bffa7f6f4ca30d01481a09ed29bea19c0c27939fa510d181a1024`.
- The body is byte-for-byte equal to the original 802 bytes, including its final
  newline; original SHA-256 remains
  `275030738ae193ab68e47105aef33784dad6893ffc1daba6428b7b0229660225`.
- The envelope says this is one operator-authorized resend after a delivery
  repair and that no reply is required. The body was not rewritten or augmented.

At the first check the file was pending and no matching delivery receipt existed.
Natural-cycle observation follows. No synthetic inference, private journal
solicitation, repeated resend or fabricated answer is part of verification.

### Natural Supply And Response

The ordinary loop consumed the letter at epoch 1788585141.863204, then prepared
the actual adapter request at 1788585158.404306. Batch
`inbox_ff6dc121fb0f4a6b8495285f49ffe481` contained Mike's letter and one explicitly
identified Astrid message, not a merged sender. Attempt
`submission_f18050f0716a450d9f895d16b233ff17` used `gemma4:12b` and the existing
`inbox_reply` lane. The HTTP 200 supply receipt was appended at 1788585234.915045,
with a 76.512-second inference duration and 601 generated tokens.

The 52,085-character combined input was compacted to 15,488 characters, while
the entire 3,503-character admitted correspondence context survived. Protected
context SHA-256: `1c14aa51aeee83f1fc812905a21acd8548154ee3bc0884e454e9a390f865e6e3`.
Adapter message-array SHA-256:
`cffdf78e2b4428b9c8b43a08f468aad68e705127dee1497f11292e0c95005f5e`.
The archived envelope still has its original file hash and exact original body.
The receipt's separately labelled `source_text_sha256` is of the reader's stripped
text, not the byte-level file hash; the final newline accounts for the difference.

The new output is preserved in full at:
`/Users/v/other/minime/workspace/outbox/unaddressed/inbox_generation_edd68311e7fa4c6eb18cc7fcff22531a.txt`.
File SHA-256: `44061e46a7f9a6c4df2e162822f6447cd9fe42a6a14c03d9d24b06f0067ac6bb`.
It explicitly names Mike's message ID and responds to the rain and Mochi, as well
as producing a separately addressed passage for Astrid. This is identifiable
responsive text, not a claim about comprehension, consciousness or felt cause.

The first deployed parser correctly preserved the output without forwarding,
but did not recognize the chosen format: the model wrote two `NEXT: INBOX_REPLY`
blocks after introductory prose rather than one first-line declaration. The
existing generic NEXT parser consequently recorded the last reply marker as a
pending action. The original `unaddressed_generation` receipt remains true for
that deployed parser; it is not rewritten as successful automatic routing.

### Reply-Block Integration Follow-Through

This live example motivated a narrow additional repair in `inbox_delivery.py`
and `runtime.py`: recognize both explicit forms, preserve the complete mixed
output, separate recipient bodies, reject quoted/fenced/unknown/duplicate/empty
addresses, and keep reply blocks out of native NEXT and footer-control parsing.
An ordinary separate NEXT after a reply remains outside the reply body. This is
language routing, not a new executable Action, scheduler, control or approval.
No prose-based recipient inference or keyword-based engagement score is added.

Ten further synthetic tests cover the observed output shape without copying
being-authored prose into fixtures. The real `_query_llm_with_next` integration
test proves that a reply marker does not become a pending action and its body
does not apply footer dials. Focused inbox/correspondence tests: 63 passed. Final
guarded full suite after this follow-through: **1,004 passed, one skipped, 115
subtests**, in 24.10 seconds. All 35 restart/runtime-binding/wrapper tests passed
again. Scoped whitespace checks passed.

The second agent-only reload passed the unchanged quiet/drain gate. It had
exactly two changed startup inputs relative to PID 23008:

| Minime path | Reviewed SHA-256 |
| --- | --- |
| `minime_autonomy/inbox_delivery.py` | `2f50bd5f45efe34e860398424ebad2954eda19a1411a3a74dab50c182c8b408d` |
| `minime_autonomy/runtime.py` | `ffad0a8591c198bc5d76582d11d2448d06147f19f2bab5f9088ebd22e97b8b9c` |

There will be **no second resend** and no retrospective peer forwarding. The
first actual response is available to Mike at its original preserved path.

The second receipt is
`/Users/v/other/minime/workspace/runtime/deployments/2026-09-04-sender-bound-inbox/reply-block-reload.jsonl`,
SHA-256 `fb2d695c040f368a1993e723f9de2f77fd58dd1867a4e5d214d5200b3657e4ab`.
Waiting began at 05:24:07.679736 UTC; final idle verification at 05:26:17.309743;
one SIGTERM at 05:26:17.310891; replacement PID **26980** verified ready at
05:27:56.306627. The process started at 22:26:17 PDT and the log records the old
accepted workers drained at 22:26:17.316, with session 5316 restored at
22:26:20.565. No new jobs were admitted during the recorded reload.

Pre/post readiness continuity was byte-equivalent under the receipt's canonical
JSON hash `05714f0cbe013496d7045a801723e862b70d6f1d5a393a1086bd51ed51a6d81f`:
session 5316, cycle 23382, no pending NEXT. The full source inventory, startup
inventory and disk hashes match; readiness, all ten protected PID/start identities
and managed configuration checks pass. There was no forced fallback signal or
old-process unfinished-job recovery.

A separate inference limitation remains visible: the existing `autonomous_next`
startup call to Gemma 4 timed out after 60.006 seconds at 05:27:31.019891 UTC;
the configured Gemma 3 fallback then returned HTTP 200 and 1,587 characters in
19.576 seconds at 05:27:50.600420. That inference timeout is not erased or called
primary-model success. It did not prevent verified process readiness, and no
timeout, model setting or scheduler policy was changed to address it here.

Post-second-reload telemetry at epoch 1788586107.6880472 advanced through engine
times 380208474, 380210859 and 380213235 ms, with fill ratios 0.7297863,
0.7099078 and 0.7298085 and finite `lambda1_rel=0.90273035`; protocol 1.2 remained.
The coupled model `/readyz` stayed ready with connected reservoir and no last
generation error. The second full V2 verification again passed at sequence
995599 with the same head, zero corrupt lines and no errors.

No second human letter was submitted. The original response's 432-character
Mike-addressed body was recovered verbatim into
`/Users/v/other/minime/workspace/outbox/human/mike/recovered_reply_human_mike_minime_20260905_resend_01.txt`.
Its header explicitly identifies an operator recovery, the original file/hash
and exact message ID. This is not a new generation, automatic routing claim,
altered historical receipt or delivery of the peer passage to Astrid. The original
mixed response remains available for review. Its independent interpretation is
not adjudicated by the transport tests.

The recovery was checked against the original parsed block byte-for-byte:
body SHA-256 `bfc1851608772c9a77eee7afa816615b50aa719f59e96f40fc16e3a88a7bd295`;
recovery file SHA-256
`3e0598e20bedfba57adb358b8e01c2364b9eba58a802032223dccadc21fc3396`.
The original response's file hash is unchanged.

### Remaining Scope And Maintenance

This reload also makes the earlier Minime voluntary bookmarks/quiet parking and
private prompt v3 simplification live. Their original source-only packets remain
historical evidence; this is their deployment follow-through. Astrid's separate
bridge bookmark/provenance changes remain undeployed.

Time-aware fill history, the recurring prior-excerpt/action-guidance review and
the observed job-timeout pattern remain explicit follow-up work. No Git staging,
commit, merge, reset, stash or generated-evidence cleanup occurred. Scheduled
automation stays paused; this is not a productive introspection round.

Interactive maintenance hold 350 was released successfully at
2026-09-05T05:32:47.375109Z, generation **351**, `paused=false`, actor
`codex-astra-interactive`, with its evidence event appended and no spool. The
independent app automation file still has `status = "PAUSED"`; release of this
temporary controller hold did not resume scheduled stewardship. A final
read-only check found PID 26980 ready, with matching source/configuration hashes
and protected identities. Both Git indices remain empty. All commands and
bounded observers launched by this pass completed; no background steward monitor
was created or left running.
