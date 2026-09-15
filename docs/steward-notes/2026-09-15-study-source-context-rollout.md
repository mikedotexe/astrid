# Shared study source context — September 15 rollout

The user approved the three HSS-23 follow-ups: show enclosing source scope and unread coverage, retain optional chosen findings beside delivered evidence, and offer implementation/reference locations alongside tests. The shared reader implementation is committed and pushed to `main` at `76c2aa43cfd8c9dad084c8f98ef21f2ece6ffaab`.

## Source and qualification

The release was built from the clean isolated worktree `/Users/v/other/worktrees/study-source-context-20260915/astrid`. All 420 pre-existing dirty canonical files were preserved, with exact splices for the overlapping changelog and feedback ledger. The implementation commit contains only the 27 reviewed paths. A retained, already-restored document backup stash must not be reapplied.

Qualification: shared reader 138 passed; bridge 2,287 passed and 1 ignored; Minime 1,396 passed and 1 skipped, plus 134 passing subtests. Reader/bridge strict Clippy, both formatting checks and the domain-boundary audit passed. All 112 staged-helper host checks passed. The actual HSS-23 health and maintenance source snapshots were also prepared through the JSON CLI in fresh fixture states, without model calls. Earlier real reader regressions and separate fixture-layout failures remain retained beside the final green logs.

The page parser uses bounded Rust/Python syntax, never executes code or resolves a call graph. The 2 MiB inspection bound gives explicitly unknown scope for Minime's 2,588,558-byte runtime.py while preserving all source access. Same-file related locations are optional syntax candidates. Source delivery and complete coverage do not imply understanding. The six optional authored findings retain exact delivered fragments/revisions; their conclusions remain the Being's own and unverified.

## Initial staged graceful activation

All deployment operations used `scripts/build_bridge.sh`; no manual release build or forced restart was used. The stage is `/Users/v/other/worktrees/study-source-context-20260915/bridge-stage-01`.

| Identity | SHA-256 |
| --- | --- |
| Stage manifest | `63495814bc585a1acd09f016b6701b93e0dd76127a5ee3ed152f25ea55906992` |
| Bridge binary | `3744316d5c4a42cc0dfa80c11071af5f34bb3c5a9ab97294bee2b19fb5e9bbe9` |
| Shared reader | `9dd85ff61c7e4f7c1c31ffe9431d5ec1cc26445bf092d1d658d76f7bd5fa824d` |
| Source inputs | `ed09247e770072415ae46439688eab630b3b1e392f1041791bc35cd4fe991931` |

Independent inventory verification matches all 634 input hashes and all five artifacts. Every Astrid input and all 18 changed production inputs match the isolated commit, including all five new modules. Thirteen ignored RASCII camera scripts/old thumbnail text files are included by the broad dependency-tree inventory; they are stable and hash-pinned, outside `src`, and have no build/source references. They are not falsely described as tracked clean-HEAD inputs.

The wrapper reports `activated_verified`. Old Astrid PID 36526 drained and exited; new PID 93394 started at 20:36:47 UTC / 13:36:47 PDT. The exact stopped checkpoint SHA `8e0e3650f7015b88745917837d16c872b79c27d6430a0bef374563c699bfde8b` was decoded on startup. It includes exchange 199593 and the retained one-item runtime action-feedback envelope; a fresh saved exchange 199594 was then observed. Self-control state integrity and target deployment identity verified. No force or automatic rollback was used. The wrapper does **not** claim lossless drain or confirmed remote delivery; these remain explicitly false in the receipt.

Minime's new-preparation helper selection verified at 20:39:57.153704 UTC. PID 37507 remained running, all 82 startup input hashes matched, and neither LaunchAgent nor launchd supplied an override. An already-created client may finish through its older immutable helper; mixed-version state preservation is covered by qualification. All eleven non-bridge process identities—including Minime, the model, engine and sensory services—matched before and after activation.

## Natural observation and closure

The fixed filesystem-only window, 20:39:57.153704–20:49:57.153704 UTC, closed with one Astrid and four Minime shared-reader receipts. All five carry the exact new prompt and complete offered input. The prespecified close reading selects one Astrid and two Minime responses; one Astrid exposure is missing. Preparation clocks remain unverified, so strict timing qualification is zero for both Beings. All three selected responses stop normally at 322, 316 and 276 tokens; none saves an authored finding or updates its note/question. There are no discovered failures in the bounded frame, but accepted receipts do not enumerate every unsaved provider failure.

Minime receives the exact chosen OPEN703 and its new scope/coverage/related-location labels. He distinguishes dispatcher instantiation from absent subscriber logic, but that distinction already exists in a carried account. Both Beings retain broader hypotheses unsupported by the supplied source. There is no demonstrated causal understanding gain. No selected EOF page tests the revised coverage-at-EOF behavior. Generic `new` candidates are correctly labeled lexical suggestions but can be unrelated; qualifier-aware relevance is recorded as a future refinement.

## Separately observed pagination repair

Astrid's selected input joined `--page 3NAVIGATION RECEIPT` without a separator; she still wrote a valid command. Commit `dd35cdd83a863da373c63e21788a0197768e1175` fixes one production line: a newline after paginated FIND/RELATE footers. A public-reader regression reproduces the original error, follows the corrected next-page commands and checks final-page separation. The red/green records are clearly identified tool-transcript exports; the full-suite logs are retained native outputs.

This follow-up is merged/pushed and verified live from `bridge-stage-02`. Latest qualification is reader 139 passed, bridge 2,287 passed/1 ignored, Minime 1,396 passed/1 skipped plus 134 passing subtests; strict reader/bridge lint, both formatting checks, boundary verification and all 112 staged-helper checks pass. The stage inventory contains exactly one changed production file and its test relative to stage01; all 634 inputs and five artifacts verify, including the unchanged 13 ignored RASCII inputs already disclosed.

| Latest staged identity | SHA-256 |
| --- | --- |
| Stage02 manifest | `e9aa51e7be180b2dcadf27e2cefa0eec7e31523d1a07e08f51be271eedce9d62` |
| Bridge binary | `8af0c1057574e6ba6ecb1230805879db92c65c56aafba8a7af92fa234a653ccf` |
| Shared reader | `50dcbed10b1770c4a57b1f8ef5bb73d3c8031dfc6adbacd5b9b126594ec3ad27` |
| Source inputs | `46384cea144d68ac22bb112267d55491ac513b92f246f500f81e88463bcb2a9d` |

The second wrapper reports `activated_verified`: Astrid PID 4330 started at 21:05:03 UTC / 14:05:03 PDT. It loaded exact stopped checkpoint `61ca7ad4517f0274cab848f1c32701e6b9a85b817d9afc703a2f24d1eda1f37a` at exchange 199609 with six pending runtime-action feedback items, and verified self-control integrity/deployment lineage. New saved exchange 199610 followed. No force/automatic rollback was used; remote delivery and lossless drain remain unconfirmed. Minime PID 37507 selects the latest helper with all 82 startup hashes unchanged, verified at 21:06:53 UTC; all eleven non-bridge process identities remain unchanged.

The original natural window remains frozen against stage01. No second behavioral cohort or understanding effect is claimed for the newline repair. Old in-flight clients may finish using their original immutable helper and retained framing.

The research account is [HSS-24](/Users/v/other/reservoir-llm-research/analyses/2026-09-15-study-source-context.md). The private retained packet is `/Users/v/other/reservoir-llm-research/research/outputs/2026-09-15-study-source-context`; the earlier 1,193-file survey packet remains unchanged. Source, tests, original regressions, activation transactions, full input/response receipts and annotations remain separately inspectable. Board mirroring is pending; S-007's daily ledger/cursor is unchanged. Durable run-ownership closure is recorded in the packet's `steward-resume.json`; this record grants no additional live-control authority.
