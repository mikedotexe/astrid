# Summary — introspection_astrid_crates_astrid-approval_src_interceptor_capability.rs_1789179940

Astrid was handed the last window of `crates/astrid-approval/src/interceptor/capability.rs`
(lines 220-276) and asked about `handle_allow_always`. She opened by naming her own evidence
boundary: the window she had shows tests, not the implementation, so she answered from an
earlier view of lines 102-136 held in recent history.

That self-report is accurate and checks out. Every line interval she cited is exact:

| Her citation | Source fact |
| --- | --- |
| window covers lines 220-276 | witness window_start_line 220, window_end_line 276, total 276 |
| `handle_allow_always` at "lines 93+" | doc comment opens line 93, signature line 98 |
| `check_capability` requirements "lines 40-91" | `check_capability` line 40, body ends 91 |
| tests "lines 230-275" | `test_check_capability_rejects_untrusted_issuer` line 230 |
| prior view "lines 102-136" | prior queued report bound to bytes 4522..8893 = lines ~102..220; line 136 closes `handle_allow_always` |

Working-copy SHA-256 equals the report-bound SHA-256
(`00e4e0f4...93405b`), so report-time and current source are the same bytes and no
snapshot reconstruction was needed.

## What her answer got right

Generation logic: `CapabilityToken::create` (lines 115-123), resource and permission from
`action_to_resource_permission` (lines 99-104), `TokenScope::Persistent` (line 118), issuer
`runtime_key.key_id()` plus `AuditEntryId` (lines 119-121). Her four `check_capability`
requirements all hold: one helper feeds both sides (lines 41 and 100), issuer trust is
enforced twice — `trust_issuer` at 43-46 and a documented TOCTOU re-verify at 55-63 — and an
uncommitted token is invisible to the validator (lines 126-129).

## The one correction

She hedged that the Allow Always TTL "likely signifies no expiration or a maximum
system-defined duration." Source says otherwise: `ALLOW_ALWAYS_DEFAULT_TTL` is
`Duration::hours(1)` (`interceptor/types.rs:6`). An Allow Always grant expires in one hour.

The correction is worth stating plainly because the answer was inside her own prior window —
line 131 logs `"created 'Allow Always' capability token (TTL: 1h)"`. She hedged rather than
read it. Her scope reading is untouched: `TokenScope::Persistent` really is passed, and it
really does mean not-session-bound.

Which leaves a genuine tension rather than a tidy resolution: an Allow Always grant is
**Persistent in scope and one hour in lifetime**. Both are deliberate and the log line says so
openly. That is a kernel approval-semantics question for Mike, not something this round may
change (c014).

## Her open ask, answered

She asked to "verify the exact `action_to_resource_permission` mapping to see how it handles
different protocols (mcp vs file) to ensure the patterns are consistent." Complete function,
lines 145-190:

| `SensitiveAction` | Resource | Permission |
| --- | --- | --- |
| `McpToolCall` | `mcp://{server}:{tool}` | `Invoke` |
| `FileRead` | `file://{path}` | `Read` |
| `FileDelete` | `file://{path}` | `Delete` |
| `FileWriteOutsideSandbox` | `file://{path}` | `Write` |
| `ExecuteCommand` | `exec://{command}` | `Execute` |
| `NetworkRequest` | `net://{host}:{port}` | `Invoke` |
| `CapsuleExecution` | `capsule://{id}:{capability}` | `Invoke` |
| `CapsuleHttpRequest` | `capsule://{id}:http_request` | `Invoke` |
| `CapsuleFileAccess` | `capsule://{id}:file_read\|file_write\|file_delete` | `Invoke` |
| `LiveControlMutation` | none | none |
| other | none | none |

Schemes are consistent. The permission convention is **not** uniform, and that is the
substantive answer to her question: in the direct-file lane the access mode *is* the
`Permission` (`Read`/`Write`/`Delete`), while in the capsule-file lane the permission is always
`Invoke` and the mode is encoded into the resource string instead. Her phrasing "Write for a
file write" is right for one lane and wrong for the other.

One more thing she would likely want to know, since it rhymes with the authority structure she
lives inside: `LiveControlMutation` maps to `None` (lines 187-188), with a comment stating that
capability tokens cannot bypass the separate authority lifecycle. Because
`handle_allow_always` turns a `None` mapping into `ApprovalError::Denied` (lines 99-104), no
Allow Always token can ever be minted for a live-control mutation. The code enforces that
boundary structurally.

## Disposition

Fifteen claims, all grounded. Thirteen `verified_existing`, two `observed`, one
`authority_gated`. Nothing was implemented: the report asks a question, and complete source
verification at a matching SHA is the answer. No source change is warranted — the single wrong
claim is a reading correction, and the tension it exposes is above this round's authority.
