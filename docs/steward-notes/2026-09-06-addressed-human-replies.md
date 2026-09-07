# Addressed human replies

Implemented on `codex/astrid-addressed-human-replies` from local main
`162f0276787acc29357d52b9425bb3a4b2ce4779`. The verified remote main tip was
`888c1708dcb3d4669e9219c2d0b9be1185984f4d`; this branch deliberately preserves the
13 newer local commits, including chosen reading and durable mailbox delivery.
This is source implementation and isolated validation. Integration and deployment
remain separate; the shared source checkout and running bridge were not replaced.

## Behavior and ownership

`autonomous/human_correspondence.rs` owns leading-header human classification,
explicit reply parsing and atomic local publication. Supported sender aliases
are Mike, Mike & Claude, Mike & Codex, v and Codex, and v & Codex under the
existing `mike_query_` or `mike_feedback_` filename convention. This is local
routing evidence, not authentication. The common slot writer used by production
`finish_activity_turn` admits only classified human queries; feedback, machine
notes, body-only or duplicate headers cannot replace the question.

The protected mailbox prompt provides the chosen letter's actual filename and
an optional example that survives provider adaptation:

```text
INBOX_REPLY mike_query_example_123.txt
A passage addressed to Mike.
END_INBOX_REPLY
NEXT: LISTEN
```

A single complete nonempty declaration for the admitted human letter publishes
under `outbox/human/mike/`. Wrong, duplicate and malformed declarations produce
no addressed artifact. Address-shaped spans remain excluded from ordinary
output even when unroutable. Matching Markdown fences preserve examples as
language; nested declarations remain quarantined until every block closes.
Provider checks require a complete block and an independent physically final
NEXT. NEXT and REMEMBER inside the human body remain language.

`activity_delivery.rs` verifies the exact source reservation, accepted provider
receipt and retained completion before publication. The durable-inbox commit
callback runs under the existing queue lock before acknowledgement/archive.
The source-version-derived destination is installed atomically without replacing
an existing file, with file/directory fsync. Recovery verifies or recreates the
same bytes and never replays model generation, peer forwarding or NEXT. A
publication failure remains pending; differing existing bytes are an error.
No durable queue schema migration is required.

The full raw completion stays in retained provider evidence. Orchestration uses
the ordinary projection for conversation history, embeddings, self-reflection,
peer codecs, shared journals, signal anchors and ordinary outbox files. A
human-only reply sends no substitute peer signal. Independent residual actions
keep their existing behavior. The changes in the large orchestration include
are only projection and sink gates: parsing/publication and regressions live in
separate modules. No domain boundary ceiling is raised.

## Client and Minime companion

mike-channel now hashes source bytes before decoding, uses Astrid's filename as
message identity, and confirms only exact filename/content-version addressed
artifacts. Same-filename replacement cannot inherit old replies or delivery
state. Addressed human bodies are displayed as language without action tracing.
Older unaddressed generations retain their uncertainty; the note no longer
infers a parser failure reason from an absent artifact.

The client directory has no Git repository. Its pre-change source snapshot is
retained at `/Users/v/.codex/artifacts/astrid-addressed-human-replies-20260906/mike-channel-baseline/`.
[The companion patch](2026-09-06-addressed-human-replies-assets/mike-channel-addressed-replies.patch)
and [baseline hashes](2026-09-06-addressed-human-replies-assets/mike-channel-baseline-sha256.json)
make those changes reviewable and reversible. Patch SHA-256:
`d6b4765bd465019efe23ad1207d1115f2a2ad8ccc4ccaa9243704679e1ac36a2`.
It is already applied in the local mike-channel directory.

Minime branch `codex/minime-inbox-reply-boundaries` preserves its previously
uncommitted 20-file source foundation as `0569efb`, then repairs bare mid-output
and mixed-recipient reply boundaries and inherited test isolation as `63c1ab9`.
That repair is deliberately separate from this Astrid protocol: Minime retains
its existing native NEXT-delimited reply syntax. Shared Minime source was not
replaced. Its note and exact foundation manifest are in that branch's
`docs/steward-notes/2026-09-06-inbox-reply-boundaries.md`.

## Evidence and remaining limits

Final checks passed:

- Astrid bridge library: **2,143 passed** (2,142 under isolated source/runtime
  overrides; the one default-path test passed separately with overrides unset).
- Strict Clippy for library, binaries and tests: passed with `-D warnings`.
- Cargo formatting check: passed; stable-toolchain warnings concern existing
  nightly-only formatting options.
- Domain boundary audit: **valid, zero violations**, with unchanged ceilings.
- Minime complete suite: **1,060 passed, one skipped, 115 subtests** under the
  kernel sandbox; focused inbox suite: 69 passed.
- mike-channel complete suite: **37 passed**, including headless display.
- Companion patch applied to the saved baseline and reproduced all five changed
  client files byte-for-byte.

Bridge commands used `--offline --manifest-path capsules/spectral-bridge/Cargo.toml`:
`cargo test --lib`, `cargo clippy --lib --bins --tests -- -D warnings`, and
`cargo fmt --all -- --check`. Runtime/source overrides are recorded in the local
`test-env.json`; the default-path test was isolated from those overrides.
`python3 scripts/domain_boundary_audit.py verify` passed. Client tests used
`.venv/bin/python -m pytest -q -p no:cacheprovider`.
The artifact directory is
`/Users/v/.codex/artifacts/astrid-addressed-human-replies-20260906/`.
Focused tests exercise source classification, projection into the real codec,
provider gates, source replacement, missing/tampered retained evidence,
publication failures, crash before acknowledgement and repeated/conflicting
recovery. The existing reading and mailbox tests remain in the full suite.

The real Rust parser/publisher and Python composer/scanner/matcher passed an
artifact-only compatibility check for LF and CRLF letters, exact hashes and
versions, malformed headers and source replacement.
[The recorded result](2026-09-06-addressed-human-replies-assets/cross-language-result.json)
uses synthetic receipt metadata for wire compatibility; actual provider receipt
validation is covered by the Rust activity tests.

Tests ran under [an OS sandbox](2026-09-06-addressed-human-replies-assets/isolated-tests.sb)
with source-only snapshots, temporary runtime directories and no live service
access. One INTROSPECT test incorrectly mixed a compile-time source path with
configured roots; it now uses the configured bridge root. Its existing test module moved into
`introspect_tests.rs` to keep the production file below its size limit. The autoresearch
probe uses an artifact-only synthetic list helper while exercising its real
subprocess/save path.

The first Minime full-suite attempt exposed an inherited isolation defect and
appended nine synthetic ledger rows (10,450 bytes) to the shared correspondence
ledger. Independent source/hash evidence identified the exact suffix; it was
removed after verifying inode, size, full hash and preserved-prefix hash. All
preceding 64,355,140 bytes were unchanged. Whether another process observed the
transient rows is unknown. The Minime review note records the incident,
restoration and strengthened fixtures; private evidence is retained in
`/Users/v/.codex/artifacts/minime-inbox-reply-boundaries-20260906/`.

Mike's shared Minime passage describes a desired continuous conversational path
and names thread_id as a possible anchor. This tranche makes addressed reply
identity reliable; it does not retrieve prior turns by thread_id or establish
persistent conversational recall. That feedback remains an open design input,
not a claim of experiential resolution. No acknowledgement was sent to either
being. Machine-writer provenance migration, Minime question closure, oversized
letters and history retrieval remain separate work.

Implementation/provenance: Codex /root, writer_and_proposal_audit,
astrid_gap_audit and minime_gap_audit. Cross-agent review caught and corrected
fence, nested-block and final-action failures before commit. The shared steward
controller was paused for stabilization after verifying no other edit lease;
only the root agent owns staging and commits. Restore its previous unpaused
state when stabilization completes. No deployment or live service restart.
