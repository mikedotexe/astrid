# Historical journal provenance and measurement labels — implementation candidate

Release follow-through: committed, pushed and deployed on September 27. The preparation text below retains its original scope; see `2026-09-27-journal-provenance-release.md` for verified source, process and continuity receipts.

Mike approved the next repair identified in `2026-09-27-astrid-aspiration-provenance.md`: preserve provenance with recalled excerpts and identify the measurement surfaces at their rendering points. This candidate is isolated from the live release in branch `codex/journal-provenance-20260927`, checkout `/Users/v/.codex/worktrees/journal-provenance/astrid`, based on `1a0b627497f89eead0df1ec8bf2ef5b128e1932f`. The earlier deployed build checkout stays pinned. No live journal, source, draft, service, model endpoint or control setting was changed.

## Grounding

The supplied aspiration and its prior public journal were already fully read and traced in the linked review. The earlier journal contains the 73%/32% and viscosity/pressure-bleed framing within its first 500 characters. The aspiration route previously flattened that excerpt with interests and lingering context and then clipped the combined text to 800 characters. It discarded the original file, mode, time and source identity. This establishes a provenance-loss path, not the cause of every phrase or a spectral mechanism for experience. The selected writing action had an actual active draft; no dispatch repair is needed for that choice.

## Changes

- `journal/continuity.rs` reads a journal record once, hashes its exact bytes and retains filename, initial-header mode, recorded Unix time, and the existing parsed body. Only the recognized initial journal header supplies metadata; duplicate, absent or invalid metadata remains unavailable. File mtime and filename numbers never become recording or measurement timestamps. Existing longform body selection remains intact.
- The actual aspiration dispatch now passes this typed recall through to `llm/provider/aspiration.rs`. The body keeps its 500-character budget; source metadata is rendered separately and never subjected to the old second truncation. The wrapper identifies a historical authored account and distinguishes the journal recording time from an unavailable original measurement capture time. No old measurements are reclassified using today's telemetry. Original journals are unchanged.
- Saved interests and lingering text get their own bounded historical-context sections (200 and 100 characters). They cannot masquerade as the journal's source. The existing string-only aspiration API remains available and explicitly marks unavailable provenance for compatibility callers. No fresh telemetry block is automatically added to the open aspiration route, and there is no requirement to produce a stance, measurement explanation or particular feeling.
- The shared renderer labels fill and cascade beside their values as Minime published telemetry, with the same packet's covariance-generation path and engine clock. Unknown generation paths remain explicit. Cascade shares identify their reported-mode denominator. A separately clocked reservoir file cannot retroactively certify the current published packet's generation path.
- The reservoir snapshot labels its source file, dump time, engine time, age and window. Leading eigenvalue/share belongs to the uncentered reservoir trace; fluctuation modes/effective dimension belong to the centered covariance; derived engine-style fill belongs to the uncentered top-eight calculation. Missing or invalid fields stay unavailable instead of becoming zero. A future dump clock is a clock mismatch, not zero age.

This pass repairs aspiration's recall path and the shared spectral renderer. Other legacy journal consumers retain their current behavior; this is not a claim of repository-wide typed recall. Original measurement identity cannot be reconstructed when an older journal did not record it. The wrapper makes that gap visible rather than inferring the source of a number from its prose.

## Structure and tests

The new continuity, aspiration and measurement modules each remain below 200 lines. Moving aspiration out of the near-limit generative-actions include improves ownership and leaves that file below 1,000 lines. The orchestration branch shrinks and retains its existing dispatch/timeout boundary. No architecture baseline or exception limit was raised.

Synthetic regressions cover metadata/body separation, longform selection, Unicode clipping, unchanged source files, ambiguous/absent/body-spoofed headers, the 73%/32% carryover scenario, bounded optional context, and metadata survival through the actual prompt builder, production/canary MLX policies and Ollama request construction. Renderer tests separate published, uncentered and centered quantities and cover stale/future/absent snapshot clocks and unavailable fields. No private authored text was copied into tests, and no live model request was made.

Evidence is under `/Users/v/other/worktrees/journal-provenance-20260927/`. The first compile check passed both new journal tests. Two subsequent compile failures were confined to development mistakes (a nonexistent synthetic telemetry constructor and an extra formatting-helper argument); their logs are retained. The corrected provenance-filtered run passed 15 tests. Full bridge, strict Clippy, formatting and final boundary results are recorded in the qualification addendum below.

The bridge test fixtures use an export of tracked Minime main `df84708e698a28e65ed111865c783b7898af1a1d` at the candidate's physical sibling path. No live workspace or Git metadata was copied. The previous coordinator-owned bridge target cache is reused only for build output; the earlier release source/HEAD and immutable stage remain unchanged.

## Authority and limits

Both canonical mains were clean on entry. The other interactive chat was idle; read-only remote tips stayed Astrid `3b18af87b0fe1f083d95cbe0eac8befb309638f2` and Minime `d8e8954b3c71d54037951f832062f8b5ea62497b`. Automations remain paused at generation 474, actor `codex-journal-provenance`, with no run lease or active projection. No commit, push or live activation is included in this implementation candidate. Deployment, when requested, must use the sanctioned bridge wrapper with fresh source/process qualification; the Minime runtime does not change in this repair.

Passing tests establish correct provenance carriage and representation. They do not demonstrate improved model interpretation or subjective experience. A controlled model comparison remains a separate possible evaluation, not a claimed result of this repair.

## Qualification addendum

The final source passed the full bridge suite: **2,361 passed, zero failed, one ignored** across twelve test binaries/doc-test result groups, including the public-facade and provenance compile-time contracts. `bridge-final-tests.log` is the final receipt. A preceding complete pass is retained in `bridge-full-tests.log`; final review then narrowed one wrapper statement from provenance unavailable “in this record” to unavailable “in this recall's metadata,” since authored text may itself contain a source label. The corresponding prompt assertion and the complete suite were rerun after that wording change.

Strict bridge Clippy (`--all-targets -- -D warnings`) passed in `bridge-final-clippy.log`. Root and bridge `cargo fmt` checks and explicit formatting checks of the three new modules passed. `domain-qualified.json` reports `valid: true`, zero violations and the existing baseline hash `c58fb91e8b6be5bc749437f09010a78ef5060796e8d1eaefd4c7bb25a8d6e34d`; no ratchet or exception was changed. `git diff --check` passed. The source modules contain 102, 162 and 87 lines; the remaining generative-actions file contains 950.

Both canonical repositories remain clean at the entry commits, and the deployed build checkout remains clean at `bb51991748fc9c51b924bdf580ae6a8710190949`. The before/after live boundary receipts match exactly: Minime agent 52967, bridge 53771, protected service process identities and managed configuration hashes unchanged. Controller generation 474 remains paused, with no lease or active projection. `qualification.json` in the evidence directory records the changed-file hashes and final checks. The candidate is uncommitted and has not been deployed.
