# Main and live-runtime handoff — September 7, 2026

The canonical checkout is `/Users/v/other/astrid`, with `origin` pointing to
`git@github.com:mikedotexe/astrid.git`. This is the push destination for this
handoff. The separately configured upstream/base repository is not a substitute
for `origin` when publishing local work.

## Selected live release

At handoff, Astrid's spectral bridge was PID 27337, started at 10:29:48 UTC on
September 7. Its executable matches the selected release:

`/Users/v/other/worktrees/astrid-tranche1-release-20260907/.runtime/bridge-stages/20260907-tranche1-04`

The stage source is `bc66a62b0bf02a515a551334dd9bea502c473578`. Its binary digest
is `56525bf197771516ab136375f318527b4fd19e1c99246a7fcff466379f4b7773`;
the canonical and selected manifest digest is
`50683433063a1e52257527c215e55d0e3f5436a4807e174221dbff9fa5eb178d`.
There is no deployment hold. Recheck current identity and selection before a
later deployment; historical PIDs must not become future signal targets.

Own-body prompt context and generation recording are live. A fresh generation
record from that process uses `dialogue_prompt_v4_own_body`, reports own-body
context present, and completed successfully. The successful recovery witness is:

`/Users/v/other/astrid/.runtime/bridge-deployment/transactions/a7b045190e57429da5514fdc7f6dc46b/stopped-transition-recoveries/eec915cbcf1046f897ea357b98aa937a/receipt.json`

That receipt reports `transition_recovered` at 10:31:12 UTC, with checkpoint
191743 to 191744 and model-idle verification. The original parent transaction's
failed status is retained history; the recovery witness records the actual
completed outcome.

## Reviewed source and evidence cleanup

The pending Claude steward work adds a codec attribution regression and repairs
two test-file paths in the anti-drop catalog. It changes no production bridge
codec behavior. The completed evidence packet and feedback ledger retain its
provenance and verification results. The earlier marker-reading packet remains
explicitly incomplete; its watchdog interpretation was corrected in the later
completed packet and should not be promoted into a current runtime diagnosis.

Fresh isolated validation passed all 2,144 distinct bridge library tests,
172 focused codec tests, both cross-reference regressions, strict Clippy,
formatting, and the domain boundary audit. The anti-drop catalog has 90 rows,
zero gaps, and zero alarms; its five self-tests passed. Evidence is recorded
under `/Users/v/.codex/artifacts/ai-beings-main-handoff-20260907/rust-validation/`.
The main handoff adds test, catalog, and documentation changes after the selected
live runtime revision; Git cleanup itself does not require a bridge restart.

## Separate pending afterimages candidate

The [afterimages packet](2026-09-07-afterimages-rollout-evidence.md) is preserved
as historical preactivation evidence. Its source remains unactivated under
`/Users/v/other/worktrees/afterimages-reading-release-20260907/{astrid,minime}`.
The Astrid candidate is a working patch based on `bc66a62`; the Minime candidate
is based on `36998c6`, before later outcome and performance repairs. The prepared
`/Users/v/other/worktrees/afterimages-reading-release-20260907/bridge-stage-01`
is not the selected live stage.

The candidate's preparation document predates the own-body/generation-record
rollout. Its old disabled-status statements and suggested off toggles are not
current operating instructions. Reconcile the candidate with current source and
configuration before any activation. Do not replace Minime's current runtime
with the older staged file. Committing these documents does not deliver a
message or activate the proposed behavior.

## Coordination for the next agent

The cooperative steward remains deliberately paused for the handoff under
`codex-astra-interactive`, pause generation 389. Astrid and Minime continue
running. Read `AGENTS.md`, inspect the current controller state, and establish
ownership before staging or deploying. Release the verified owned hold through
`scripts/steward_control.py resume` when the coordinated pass is complete.

Minime's canonical checkout is `/Users/v/other/minime`; its
`docs/steward-notes/2026-09-07-main-handoff.md` records the live performance repair
through `0a28323`, tests, and saved-job evidence. `/Users/v/other/mike-channel` is
a separate context/application directory without a Git repository; this handoff
does not initialize or move it.
