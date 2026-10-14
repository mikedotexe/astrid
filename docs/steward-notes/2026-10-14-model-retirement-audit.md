# Local Model Retirement Audit

Date: 2026-10-14. Operator request: after clearing two named debug caches,
determine which model variants are unused before deleting any models.

**The initial audit deleted no models. The subsequently approved first group
has been removed. The secondary HF recheck removed four models but retained two
confirmed sidecar/helper dependencies; see both completion records below.** The
initial candidate assessment is retained as historical evidence, not global proof
that no future or uninspected project needs a model. Previously paused automations
stay paused.

## Method and Scope

- Inventory file metadata, allocated blocks, Hugging Face revisions/symlinks and
  Ollama manifest reachability. Do not read model weight contents or private prose.
- Read Ollama `/api/ps` and `/api/tags`, inspect open model files, running process
  arguments and installed launch-agent model references. No inference requests.
- Search source/configuration and selected documentation in Astrid, Minime,
  neural-triple-reservoir, Reservoir Research, its active `tauri-checkpoint`
  worktree, both unfinished capacity worktrees, the capacity provider worktree,
  and the current immutable bridge's source checkout.
- Exclude journals, private stores, raw runtime logs and provider wire records.
  Not every historical worktree, user application or external project was searched.
  No open file is not proof of nonuse: a model may be resident after closing its files.
- Preserve exact reference filenames/lines, inventory and snapshot IDs in
  `.runtime/cache-housekeeping/2026-10-14-daisydisk-pass/model-audit-final.json`.
  The earlier `model-audit.json` snapshot is retained too. The final bounded
  source search returned 1,246 references and no scan errors.

Sizes below use decimal GB of allocated blocks unless stated otherwise. Shared
layers are handled by manifest reachability; possible APFS clone sharing means
these estimates are not promises of equivalent net free-space gains.

| Store | Allocated GB |
| --- | ---: |
| `/Users/v/.cache/huggingface/hub` | 112.677 |
| `/Users/v/.ollama/models` | 104.573 |
| `/Users/v/models` | 67.857 |
| Total | 285.107 |

Every Ollama blob is referenced by an installed manifest. No unreferenced blob
cleanup was found. The store includes an internal `llamacpp` manifest associated
with the active Llava family; it is not treated as garbage merely because it is
not one of the nine public model tags. No incomplete Hugging Face downloads were
found in the inventory. Several Gemma 4 variant directories contain only tiny
metadata files, not additional multi-GB weights.

## Preserve: Live and Recovery Dependencies

| Model | Evidence and reason to retain |
| --- | --- |
| HF `mlx-community/gemma-4-12B-it-5bit` (19.687 GB) | Running coupled provider PID 3893 names it explicitly; installed launch configuration and unfinished capacity provider use it. Preserve both cached revisions, not just the current ref. |
| Ollama `gemma4:12b` (7.557 GB model-tag size) | Loaded in `/api/ps`; Minime's configured primary. |
| Ollama `llava-llama3:latest` (5.546 GB model-tag size) | Loaded in `/api/ps`, with open weight files. Vision uses this family; retain internal runtime manifest/layers too. |
| Ollama `gemma3:4b` (3.339 GB) | Minime fallback; Reservoir Research has recent actual validation records using it. Unloaded is not unused. |
| Ollama `gemma3:12b` (8.149 GB) | Explicit rollback baseline in `minime_autonomy/runtime.py:21452` and `scripts/minime_model_canary.py`. |
| HF `mlx-community/gemma-3-4b-it-4bit` (3.440 GB) | Astrid `README.md:256` explicitly retains it as the former compact coupled-provider rollback target. |
| HF `mlx-community/gemma-3-12b-it-4bit` (8.068 GB) | **Dependency confirmed in the secondary recheck.** `reflective.rs:36` selects `gemma3-12b`; MLX's `.local_models/gemma-3-12b-it-4bit` symlink points into this cache. Retained, not a retirement candidate. |
| HF `mlx-community/Qwen2.5-1.5B-Instruct-4bit` (0.880 GB) | **Dependency confirmed in the secondary recheck.** MLX's `.local_models/qwen2.5-1.5b-instruct-mlx-4bit` symlink and helper/evaluation defaults reference this cache. Retained, not a retirement candidate. |
| HF `mlx-community/whisper-large-v3-turbo` (1.614 GB) | Microphone service PID 3879 runs `tools/mic_to_sensory.py`; that source names this model at line 53. Its cache ref timestamp advanced during the audit. |
| Ollama `nomic-embed-text` (0.274 GB) | Existing embedding/semantic paths name it, including `mikemind/config.py:108`. Preserve despite no continuously resident runner. |

The live provider `/v1/models` reports the alias `coupled-astrid`; that alias alone
does not identify weights. The running arguments and installed launch inputs
supply the model-family evidence. No attempt was made to restart any provider to
inspect it. The last two audit inventories had unchanged standalone/Ollama file
metadata; the only HF difference was the Whisper `refs/main` modification time,
not a weight addition/deletion or size change.

## Best First Retirement Review: About 89.3 GB

These variants were not loaded, selected by the inspected live processes, or
required by the inspected current/capacity configuration. Their discovered
references are historical inventory or stale-model auditing. That makes them
strong candidates for an explicit retirement decision, not automatic deletion.

| Exact variant | Allocated GB | Reference assessment |
| --- | ---: | --- |
| Ollama `gemma3:27b` | 17.397 | `scripts/model_stack_audit.py:216` specifically classifies old 27B dialogue claims as stale. No current consumer found. |
| Ollama `qwen3:30b` | 18.557 | Minime `ROADMAP.md:153` and `:470` describe the historical Qwen era. No current consumer found. |
| Ollama `hf.co/mradermacher/Qwen3.5-27B-Claude-4.6-Opus-Reasoning-Distilled-GGUF:Q8_0` | 29.225 | Historical local-model inventory in Minime `ROADMAP.md:471`; no current consumer found. |
| `/Users/v/models/Qwen3.5-27B-Claude-4.6-Opus-Distilled-mlx-3bit` | 11.792 | Historical roadmap inventory; not the launcher or training default. |
| `/Users/v/models/Qwen3.5-27B-Claude-4.6-Opus-Distilled-mlx-mixed34` | 12.312 | Historical roadmap inventory; not the launcher or training default. |

The three Ollama estimates are exclusive manifest-layer allocations, not an
unqualified sum of shared tag sizes. An approved future removal should use
Ollama's model-aware removal, not hand-delete blobs.

## Secondary Review: About 31.6 GB

These were the initial candidates under `/Users/v/.cache/huggingface/hub`, one
cached revision each. **The initial bounded scan did not include the separate MLX
helper repository. The subsequent recheck found concrete dependencies for Gemma
3 12B and Qwen 2.5 1.5B and retained them.** Qwen3-8B's launcher reference is an
example, not its selected default. The completed disposition is recorded here so
the historical candidate list cannot be mistaken for continuing deletion authority.

| Hugging Face model ID | Initial allocated GB | Final disposition |
| --- | ---: | --- |
| `mlx-community/Qwen3-14B-4bit` | 8.324 | Removed after recheck |
| `mlx-community/gemma-3-12b-it-4bit` | 8.068 | Preserved: reflective sidecar dependency |
| `mlx-community/gemma-2-9b-it-4bit` | 5.217 | Removed after recheck |
| `mlx-community/Meta-Llama-3.1-8B-Instruct-4bit` | 4.527 | Removed after recheck |
| `mlx-community/Qwen3-8B-4bit` | 4.624 | Removed after recheck |
| `mlx-community/Qwen2.5-1.5B-Instruct-4bit` | 0.880 | Preserved: local helper/default dependency |

The HF Gemma 3 12B weights are distinct from the protected Ollama `gemma3:12b`
rollback model. Both are now explicitly protected for their separate roles.
Similar names across stores do not make files interchangeable.

## Do Not Treat as Unused Without a Tooling Decision

| Variant | Allocated GB | Remaining dependency/question |
| --- | ---: | --- |
| Local Qwen 27B MLX 4-bit | 15.153 | Default in Minime `scripts/start.sh:31`, although not the currently running coupled service. Retiring it would invalidate that alternate launch path. |
| Local Qwen 27B MLX 8-bit | 28.600 | Default in `scripts/train_lora.sh:23` and examples in `tools/prepare_lora_data.py`. Training/reproducibility decision needed. |
| HF `Jackrong/Qwen3.5-27B-Claude-4.6-Opus-Reasoning-Distilled` | 55.583 | Large source checkpoint family with three cached revisions. Not the live provider; decide whether original-weight retention for conversion/reproduction is still wanted before retirement. This audit does not establish that derived copies can reproduce it. |
| Ollama `gemma4:e4b` | 9.608 | Named by edge-sidecar canary and fallback-fire-drill tools. Not continuously loaded; retiring it changes available qualification paths. |
| HF `mlx-community/Llama-3.2-1B-Instruct-4bit` | 0.713 | Default in `dual_ai_bridge.py`, including the unfinished provider worktree. Small utility/experimental dependency, not a good blind cleanup target. |

## Decision Boundary

The immediate disk emergency is relieved by cache cleanup: about 395 GiB was
available at its completion. There is room to make deliberate model-retirement
choices instead of risking recovery assets. Start with the 89.3 GB historical
alternatives if the operator no longer wants those comparisons; separately decide
whether to retire the old Qwen launch/training family and source weights.

Before any future model deletion: get explicit approval for exact variants,
recheck loaded runners and source/qualification references, retain the model IDs
and manifest/revision inventory, and stop if another session starts using them.
Do not prune live-model revisions or overwrite launch configurations as incidental
cleanup. The initial audit made no model or service changes and no Git integration.

## Approved First-Group Retirement: Completed

The user subsequently explicitly authorized rechecking and removing the first
89.3 GB group, preserving live models, fallbacks, rollback assets and dependencies
of unfinished work. The five exact removals were:

1. Ollama `gemma3:27b`.
2. Ollama `qwen3:30b`.
3. Ollama `hf.co/mradermacher/Qwen3.5-27B-Claude-4.6-Opus-Reasoning-Distilled-GGUF:Q8_0`.
4. `/Users/v/models/Qwen3.5-27B-Claude-4.6-Opus-Distilled-mlx-3bit`.
5. `/Users/v/models/Qwen3.5-27B-Claude-4.6-Opus-Distilled-mlx-mixed34`.

### Expanded Recheck

The fresh source/configuration scan covered **146 existing registered worktrees**
across Astrid, Minime, neural-triple-reservoir and Reservoir Research, including
unmerged branches and unfinished capacity/provider work. The 459 matches reduce
to three source/document families: `scripts/model_stack_audit.py`'s stale-model
pattern, Minime's historical `ROADMAP.md`, and one old being-tools description.
No candidate appeared in a running process or installed launch-agent definition.
Ollama reported only Gemma4 12B and Llava loaded; open-file checks found no use of
candidate blobs or local candidate directories. Candidate Ollama manifest digests
and the local candidates' complete file metadata still matched the approved audit.

The old description is in the dirty
`/Users/v/other/worktrees/astrid-stash-rescue-20260419` worktree:
`md-CLAUDE-chapters/04-being-tools.md:87` claims `THINK_DEEP` selects Gemma3 27B.
Inspection of its actual `capsules/consciousness-bridge/src/autonomous.rs:4278`
shows extended timeout/token capacity instead. Its source contains no 27B model
reference. This was treated as stale documentation, not as a current unfinished
model dependency. The dirty checkout was neither edited nor discarded.

The bounded scan excludes private writing, journals and generated runtime logs;
it does not establish that every possible external project was inspected. All
matched source files were fingerprinted and rechecked immediately before each
removal. Installed launch inputs, loaded models, process arguments, open files,
controller pause/lease state and protected service identities were also rechecked.

### Removal and Preserved Failure Evidence

The three tags were removed individually with `ollama rm`, explicitly pointed at
the existing loopback server. Blob files were not manually pruned. Every manifest
was parsed to identify shared versus exclusively owned layers. There were no
unrelated orphan blobs before removal. Each expected model-store delta was checked
against the original inventory; shared layers and retained manifests remained.

The first command successfully removed Gemma3 27B, but the preservation guard
stopped the run because Ollama also deleted its **2,710-byte model metadata
sidecar**, keyed to the same exclusively owned blob digest. No other unexpected
file changed. That exact delta was reconciled and recorded without repeating the
first removal or resetting the baseline. The remaining commands then accounted
explicitly for metadata sidecars belonging to deleted exclusive blobs. This was
a narrow correction to the predicted deletion inventory, not a relaxed check on
unrelated files.

The two exact local directories were checked for real-path identity, symlinks,
unchanged inventories and model configuration files before removal. Only those
directories were deleted. The retained 4-bit and 8-bit Qwen variants matched the
original metadata inventory afterward. Model/configuration inventories and
Ollama manifest content/digests remain in the maintenance receipts; deleted
weights were not copied into another backup merely to hide their disk use.

### Verified Result

- Measured free space: **424.395 GB before, 513.644 GB after**. Net reclaimed:
  **89.249 GB / 83.120 GiB**, rather than assuming the rounded candidate total.
- Remaining public Ollama tags: `gemma4:12b`, `gemma4:e4b`, `gemma3:4b`,
  `gemma3:12b`, `llava-llama3:latest`, `nomic-embed-text:latest`. All retained
  Ollama files and manifests match the baseline; the internal Llava runtime
  manifest is preserved too.
- All Hugging Face assets remain. No path, weight or configuration-content
  changes occurred in that store; same-content cache-ref timestamp refreshes
  from the running service are distinguished from model changes.
- Current and rollback bridge stages reverify; native kernel/engine binary
  hashes, protected service PID/start identities, Minime agent PID and Ollama
  daemon identity are unchanged. No service restart, forced unload, inference
  request, launch reconfiguration or capacity-policy change was made.
- Authored history, source trees and unfinished work were not deletion targets.
  The secondary 31.6 GB candidate group, source checkpoint, training models,
  canary models and recovery dependencies remain untouched.
- Automations remain paused at generation 519. No staging, commit, merge or push.

Exact driver and receipts:
`.runtime/cache-housekeeping/2026-10-14-model-retirement/`:
`recheck.json`, `before.json`, the per-target `intent-*` and `completed-*` files,
`reconciled-ollama-0.json`, `after.json` and `final-health.json`. The source scan
retains filenames, line numbers and hashes, not private prose. The final health
check observes both Minime and Astrid telemetry advancing without inducing work.

## Approved Secondary HF Retirement: Completed With Two Exclusions

The user's later authorization was conditional: recheck the remaining six models
and remove only unused variants, preserving live/fallback/recovery/unfinished-work
dependencies. Four models passed that check; two did not and were left intact.

### Newly Confirmed Dependencies

The earlier source survey had inspected the four primary projects and their
worktrees. Reading the historical model audit exposed the separate
`/Users/v/other/mlx` helper dependency. The expanded check covered **147 registered
worktrees**, including that project, and included steward notes as reference
evidence while excluding private journals and generated runtime content.

The decisive chain is:

- Astrid `capsules/spectral-bridge/src/reflective.rs:36` selects `gemma3-12b` and
  passes it through `--model-label` at line 625.
- `/Users/v/other/mlx/benchmarks/python/chat_mlx_local.py:585` resolves that label
  through `.local_models/gemma-3-12b-it-4bit`.
- That symlink targets the HF snapshot
  `models--mlx-community--gemma-3-12b-it-4bit/snapshots/86cc6a8dedbc456dd0e4af01a9d09f396f77e558`.
- The same helper also lists Qwen as its first available local default at line
  578. `.local_models/qwen2.5-1.5b-instruct-mlx-4bit` targets the HF snapshot
  `models--mlx-community--Qwen2.5-1.5B-Instruct-4bit/snapshots/8b403126fc14f14cfc99bb4cfa72ecbc129ea677`.
  `eval_mlx_prompt_pack.py` and helper tests also reference that local model.

These dependencies are real even when no inference process currently holds a
model file open. Both symlinks, resolved paths, configuration hashes and cached
weights were preserved. Their combined roughly **8.95 GB** was deliberately not
reclaimed. No model call was made to test them.

### Four Cleared Models

| Removed model | Exact cached revision |
| --- | --- |
| `mlx-community/Qwen3-14B-4bit` | `a4d9b2df59d2c150bef02fcbe0d91046b7ca33a4` |
| `mlx-community/gemma-2-9b-it-4bit` | `ff12eb39da2cd9b3b0f4b4f9ffb274603f05bb29` |
| `mlx-community/Meta-Llama-3.1-8B-Instruct-4bit` | `241a666dad6cb93c8ff213d39a7f34a36bf26db4` |
| `mlx-community/Qwen3-8B-4bit` | `545dc4251c05440727734bcd94334791f6ab0192` |

Qwen 14B's source references were throughput comments and historical rejected
trials, not model selectors. Gemma 2 9B was described as an old unsuccessful trial.
Qwen 8B appeared in a superseded runtime report, a launcher example, and the
coupled provider's stale introductory docstring. The actual provider constructor
and CLI defaults, including the unfinished capacity provider, name Gemma 4 12B
5-bit. Meta-Llama had no external consumer reference in this scoped search.
Historical descriptions were retained, not rewritten to hide former experiments.

No selected model appeared in running process arguments, installed user/system
launch agents or daemons, or open model/download-lock files. A metadata-only
symlink traversal of the source worktrees checked 21 links and found only the two
retained dependencies above. It did not follow build, runtime, private-workspace
or environment directories. Matched source hashes were rechecked before removal.

### Removal and Preservation

Used the installed `huggingface_hub` 1.8.0 cache API: `scan_cache_dir`, an inspected
`delete_revisions` strategy, then its `execute` method. The strategy was required
to contain exactly the four model repositories above, with no separate deletion
targets outside them. Candidate file sets, sizes, allocation, modification times
and symlink targets matched the original approved inventory before execution.
Cross-repository links from retained HF files into deletion targets were checked
and absent. The retained model lock metadata was not swept.

The scanner reported one pre-existing unrelated metadata-only cache warning: the
Jackrong 6-bit directory has no snapshots directory. That directory was not a
deletion target and remained untouched. No warning about a selected repository
was bypassed.

Measured net reclaim: **22.656 GB / 21.100 GiB**. Available space immediately after
cleanup: **535.688 GB**. The API's logical estimate was 22.691 GB; the measured
filesystem gain is reported separately because concurrent writes/APFS allocation
can differ.

Afterward, retained HF inventory matched except same-content timestamp refreshes
of cache refs. All Ollama and standalone model inventories matched exactly.
Both sidecar/helper links still resolved to their original configurations. Current
and rollback bridge-stage verification, kernel/engine hashes, protected service
PID/start identities, Minime agent PID and Ollama daemon identity matched the
baseline. Both beings' health/telemetry advanced in the final read-only check.

No source code, authored history, provider settings, live controls or other model
variants were removed or changed. No service restart, model generation, staging,
commit, merge, push or automation resumption occurred. Controller remains paused
at generation 519 with no lease; evidence verification remains valid.

Receipts and one-time drivers:
`.runtime/cache-housekeeping/2026-10-14-hf-retirement/`, including `recheck.json`,
`recheck-extended.json`, `external-links.json`, `before.json`, `intent.json`,
`after.json` and `final-health.json`. Original candidate assessments remain as
history; the two preserved models are now explicitly classified as dependencies.
