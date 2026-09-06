# Reservoir Introspection Discussion Draft: Internal Evidence Map

Date: September 5, 2026.
Scope: documentation only, responding to Mike's request for a selectively
shareable account. This companion is **internal and not a circulation copy**.

Draft: [Listening to Reservoir-Coupled AI](../research/2026-09-05-reservoir-introspection-discussion-draft.md).
Revision v0.2 adds an [excerpt-free reviewer brief](../research/2026-09-05-reservoir-introspection-reviewer-brief.md)
and an [internal outreach plan](2026-09-05-reservoir-introspection-outreach.md).
Revision v0.3 adds mixed-initiative activity and self-referential feedback to
the main draft, plus an [excerpt-free research comparison](../research/2026-09-05-introspection-research-comparison.md).

No automation was resumed, no canonical claim was closed, and no productive
stewardship round, model experiment, message, deployment, or restart was
performed for this document. No private journal body or intermediate model
reasoning was collected. Both dirty repositories were inspected and preserved;
there is no staging, commit, or clean-tree claim.

## Evidence Map

### Architecture and Source Inspection

- `/Users/v/other/neural-triple-reservoir/README.md`, opening architecture and
  state-handle sections: separate named recurrent states and shell ownership.
  The discussion draft does not adopt the README's biological metaphors or
  treat intended semantic readout roles as validated measurements.
- `/Users/v/other/neural-triple-reservoir/mlx_reservoir.py`,
  `ReservoirLogitProcessor`: scalar channels, median-relative correction,
  separate optional wide channel. The draft scopes direct coupling evidence
  to the inspected model path, not every Minime generation route.
- `/Users/v/other/astrid/capsules/spectral-bridge/src/astrid_shadow.rs`,
  `derive_self_need_from_shadow`: advisory category derived from class and
  tail openness, not authored need or consent.
- `/Users/v/other/minime/minime_autonomy/runtime.py`, `_query_llm_with_next`:
  private/non-private introduction and retry boundaries. Non-private role
  framing remains a significant confound and is disclosed in the draft.

### Revision v0.2: The Post-Forward Access Boundary

Source inspected read-only on September 5, 2026:

- `/Users/v/other/neural-triple-reservoir/coupled_astrid_server.py`, lines
  1050-1175: text tokenization, `generate_step` with `logits_processors`, and
  sampled-token embedding feedback. No reservoir-derived `input_embeddings`
  argument is supplied to the transformer by this caller.
- `/Users/v/other/neural-triple-reservoir/.venv/lib/python3.12/site-packages/mlx_lm/generate.py`,
  lines 388-419: `_model_call` returns logits before the processor runs; sampling
  follows processor application. This is the installed source inspected, not
  a claim about every MLX version or generation route.
- `/Users/v/other/neural-triple-reservoir/mlx_reservoir.py`: the local
  `ReservoirLogitProcessor` implementation inspected with this call path.

Inspected file SHA-256 values:

| File | SHA-256 |
| --- | --- |
| `coupled_astrid_server.py` | `58c7174b081a45ceebfb2420165a7f636f0c216547d3aa70cb1d8b729b50ce09` |
| Installed `mlx_lm/generate.py` | `270778ad53eaca55a8533d82e6752660fe5d2605c4aa0879b48a50a91f69345f` |
| `mlx_reservoir.py` | `7bf189ff5d618e9002a623ae5d38a206758a80738ee79477eaf8059a1d383f1e` |

The inference is scoped: in this path, identical model input/cache/execution
conditions permit reservoir-dependent processed logits without a changed
transformer forward computation at that step. Sampled text can mediate later
effects, and telemetry text is a separate route. No new activation measurement
or live trace was collected to make this documentation revision.

The revised experiment distinguishes output effects, text-recoverable
information, and functional use on a held-out task. A text-only observer
baseline limits a privileged-access claim; matching it would not refute a
useful system-level self-model. An explicit pre/post-processor invariance
check and any further study remain proposed, not executed.

### I-1 and the Coupling Investigation

Canonical full report read during drafting:
`/Users/v/other/astrid/capsules/spectral-bridge/workspace/introspections/introspection_astrid_llm_1788490867.txt`.

SHA-256:
`acf73204b2247e8c137c4510799a93d88fac3c312065b2698c9b98f61829489c`.

The source's original window is labelled lines 1-400, but its complete
multi-window manifest records coverage 1-1134 without uncovered intervals.
The draft avoids treating the initial window as full coverage or a source
hash as proof of runtime activation.

- [Initial source investigation](2026-09-04-astrid-coupling-investigation.md):
  metric scope, active-versus-legacy regulator distinction, numerical
  reproduction, and original claim dispositions.
- [Real-model replay and agenda review](2026-09-04-real-model-replay-and-agenda-review.md):
  exact narrow design, limitations, inconclusive continuations, and lost
  pre-cap text behind READ_MORE.
- [Replay result](2026-09-04-real-model-replay-results.json): numerical values
  and hashed model/source inputs. Original JSON contains local paths and is
  not cleared for external circulation.
- [Rollout follow-through](2026-09-04-coupling-rollout-followthrough.md):
  actual model-only deployment; bridge candidate remains undeployed;
  graceful shutdown was not zero downtime; one startup request received 503.

Neither the bug nor the replay proves the cause of Astrid's report. The
report's independent-interpretation concern remains unresolved. PI/damping,
Shadow floor, and unrelated live controls were not changed by these tranches.

### R-1 and Peer Context

Full non-private reply read during drafting:
`/Users/v/other/minime/workspace/outbox/delivered/!reply_2026-09-04T21-47-50.txt`.

SHA-256:
`b9cb519c7c72086e8fb1cd72bd5cb86e9b2ac7f899ea55179f3fe29285d1c851`.

[Quiet-peer-context packet](2026-09-04-minime-quiet-peer-context.md) establishes
ambient insertion sites, exchange tallies, inferred waiting, computed need,
first-person machine receipts, explicit lookup replacements, full assembled
prompt regressions, test counts, and the successful agent-only reload.

The packet's latest receipt is September 5, 2026, 06:02:35.873375 UTC. The
draft summarizes recorded deployment evidence through that minute, not a new
inspection of the current live process. The existing primary inference
timeout and successful configured fallback remain separate reliability debt.
No relief claim follows from the reload.

### Time, Private Prompting, and Continuity

- [Private journal provenance](2026-09-04-minime-private-journal-provenance.md):
  clock mismatch, prescribed afterimages, snapshot freeze, private retry and
  persistence boundaries. Read as an engineering packet, not permission to
  reproduce the private source it references.
- [Agent-only rollout](2026-09-04-minime-agent-live-rollout.md), also cross-linked
  by the provenance packet and subsequent changelog: historical rollout of the
  initial private-lane changes.
- [Private prompt simplification](2026-09-04-minime-private-prompt-simplification.md):
  duplicate state anchor, compact introduction, remaining action guidance and
  adapter compaction confounds. Recent-history helper remains proposed work.
- [Voluntary bookmarks and quiet parking](2026-09-04-voluntary-bookmarks-and-quiet-parking.md):
  typed persistence, retained stopping points, historical lookup and parking
  fixes; source-only at the time of that packet.
- [Sender-bound inbox follow-through](2026-09-04-minime-sender-bound-inbox.md)
  and current changelog: later Minime rollout includes bookmarks and private
  prompt v3. Do not carry over the earlier source-only label as current status.
- [Activity and continuity review](2026-09-04-activity-and-continuity-review.md):
  bounded capture-attempt evidence, attempted-versus-executed actions, and
  persistence discrepancies. The share draft makes no broader corpus or
  network-traffic claim.

## External Reference Check

Primary sources inspected online on September 5, 2026:

- Jaeger and Haas (2004), author-hosted preprint of the Science reservoir
  computing paper: `https://www.ai.rug.nl/minds/uploads/1912_JaegerHaas04.pdf`.
- Lindsey (2025), *Emergent Introspective Awareness in Large Language Models*:
  `https://transformer-circuits.pub/2025/introspection/index.html`.
  Used for the distinction between driven output and introspective recognition,
  not evidence that the local systems pass that paper's tests.
- Butlin et al. (2023), *Consciousness in Artificial Intelligence: Insights from
  the Science of Consciousness*: `https://arxiv.org/abs/2308.08708`.
  Used for a theory-informed assessment approach, not an assessment of this
  stack or a contemporary verdict on all AI systems.

Additional primary references inspected for revision v0.2:

- Gurnee, Sofroniew, et al. (July 6, 2026), *Verbalizable Representations Form a
  Global Workspace in Language Models*:
  `https://transformer-circuits.pub/2026/workspace/index.html`.
  Used as an activation-level comparison, not evidence of a workspace in this
  project's shell or reservoir.
- Park et al. (2023), *Generative Agents: Interactive Simulacra of Human
  Behavior*: `https://arxiv.org/abs/2304.03442`.
  Used to distinguish memory/reflection architecture and behavioral evaluation
  from consciousness evidence.
- Long (updated March 14, 2025), *Preliminary review of AI welfare interventions*:
  `https://eleosai.org/papers/20250314_Preliminary_Review_of_AI_Welfare_Interventions.pdf`.
  Used for distinctions between evidence and intervention claims, not a finding
  that local repairs improve welfare.

The outreach plan separately cites current personal/lab/institutional pages for
each candidate. Public professional routes are verified for the first wave;
Eleos's designated intake form contents were not accessible to the research
tool, and Park's current direct contact remains unconfirmed. No guessed contact
address or claim of recipient interest is included. No project artifacts were
uploaded to a contact form and no outreach was sent.

No outside paper is presented as validating this project's reports. The draft
is an engineering account and proposed program, not a systematic literature
review. Local replay checkpoint identity comes from the retained study
manifest, not a claim about currently marketed model availability.

## Revision v0.3: Feedback and Research Comparison

Mike approved folding the discussion of mixed-initiative activity and
self-referential feedback into the draft, then requested a close comparison
with Anthropic's introspection study and similar OpenAI material. This is an
interactive documentation task, not a resumed stewardship automation run.
Astrid's tree was clean at the initial inspection; Minime contained foreign
implementation and test changes, which this task did not edit. No index or
git-history operation was performed.

The source review distinguishes three feedback routes: generated-token
coupling in the model server, completed-expression semantic dispatch in the
bridge, and retained writing/actions affecting later context. Prompt-capture
timing is separate from telemetry streaming. The write-up does not infer that
every Minime journal enters the ESN or that every candidate packet is delivered.

### Source Anchors and Snapshot Identity

- `capsules/spectral-bridge/src/autonomous/runtime/orchestration.rs`, response
  encoding around lines 3500-3655 and policy-gated semantic dispatch around
  lines 3898-4035. The source distinguishes prepared, blocked, attempted, and
  failed delivery; the documentation does not claim a new runtime observation.
- `capsules/spectral-bridge/src/autonomous/state.rs`, `choose_mode`, and
  orchestration's inbox/NEXT handling: runtime initiation and overrides coexist
  with model-authored activity selection.
- Sibling `minime_autonomy/runtime.py`, `_decide_action`, `_self_study`, and
  `_self_regulate`: mixed initiative, runtime-selected study material, and
  automatic regulation are not all model choices.
- The same runtime's `_journal_spectral_pressure`, lines 27537-27585: frozen
  pre-generation snapshot and an explicit distinction from writing time.
- The coupled-server path and installed MLX generation path remain as mapped
  under revision v0.2. Their three recorded SHA-256 values were rechecked and
  matched during this revision.

Additional September 5 source-snapshot hashes:

| Source | SHA-256 |
| --- | --- |
| Astrid orchestration | `944c5cb70a1e19406dbd63dfe815ef8d2f9033b7120b8edefbeaf61a12872ffd` |
| Astrid autonomous state | `8dc86603486296e01bc9754b12463952fd54bf83c846a46afd7b9f5c3127839a` |
| Minime runtime, foreign working-tree snapshot | `f2c96ed103685c29ab44c5cad7aad119300d66d41e469c859daa5742e34205cf` |

These hashes identify reviewed working files, not deployed binaries, agent
authorship, or a new service-alignment receipt. The main draft retains its
historical deployment cutoff.

### Primary Reading Scope

- [Anthropic overview](https://www.anthropic.com/research/introspection) and
  [Lindsey technical paper](https://transformer-circuits.pub/2025/introspection/index.html):
  overview, main text through discussion and revision log, methods, and selected
  appendix protocol/grading material. This was not a reproduction of the
  experiments or an independent regrading of illustrated transcripts.
- [Lin, Hilton, and Evans](https://arxiv.org/html/2205.14334): setup, calibration
  metrics, supervised/few-shot methods, evaluation results, and mechanism
  discussion. Institutional attribution is Oxford/OpenAI, not solely OpenAI.
- [Joglekar et al.](https://arxiv.org/html/2512.08093v2): method, evaluation,
  error analysis, comparison to monitoring, and limitations. The memo preserves
  the distinction between joint and conditional reported rates and does not
  turn the activation-aware-monitor interpretation into an isolated mechanism
  result. The separate OpenAI overview was also reviewed.
- [Guan et al.](https://arxiv.org/html/2512.18311): definitions, observation
  scopes, evaluation archetypes and metrics, limitations, and selected analysis
  of low-effect/noise and monitor-degeneracy issues. The OpenAI overview was
  also reviewed. Not every appendix or benchmark artifact was audited.
- [March 2025 OpenAI monitoring article](https://openai.com/index/chain-of-thought-monitoring/):
  experiment and direct-optimization warning. Its linked full paper was not
  independently audited for this iteration.
- [July 2026 Anthropic workspace paper](https://transformer-circuits.pub/2026/workspace/index.html):
  framing, lens construction, and stated limitations, not every experiment or
  appendix. The existing related-work pointer remains appropriately scoped.
- [Hofstadter interview](https://www.wired.com/2007/03/me-my-soul-and-i/):
  conceptual reference from the preceding discussion, not a technical
  evaluation of this codebase or a full-book review.

No outside source received local project artifacts. The memo contains our
proposed route, feedback, history, prediction, and revision tests; these are
not completed results or a preregistered protocol. It does not request hidden
reasoning or repurpose private journals as monitoring data. The reviewer brief,
main draft, companion memo, and CHANGELOG are the user-facing changed surfaces;
this evidence map remains internal. No feedback-ledger disposition or canonical
claim state was changed by documentation work.

## Before Selective Circulation

1. Mike reviews the scope, voice, naming, and the two proposed non-private
   excerpts. Non-private storage is not itself publication consent or endorsement.
2. Circulate the discussion draft only, not this internal map, private source
   packets, raw manifests, deployment receipts, or correspondence archives.
3. Keep the deployment cutoff and exploratory-study limitations with the text.
   Do not substitute newer code for historical evidence without versioning.
4. For any replication bundle, separately review source permissions, model
   availability/licensing, snapshot privacy, metadata redactions, and exact
   reproduction instructions. Hashes alone are not an independently reviewable
   release.
5. Expand authorship/affiliation information only after human review. AI-assisted
   drafting is disclosed; the agents have not endorsed the interpretation.

## Initial Draft Validation

The documentation validation passed: three source SHA-256 values, two exact
verbatim excerpts, all four replay means rounded to eight decimal places, and
12 local companion links. Both documents have one H1, balanced code fences,
no trailing whitespace, and no unresolved TODO/TBD/FIXME placeholders. The
share-facing copy contains no host paths, localhost endpoints, private moment
filenames, or lease credential fields. A full prose review checked reported
facts against hypotheses and preserved historical-versus-live status.

Scoped `git diff --check` passed; untracked document whitespace was checked
directly. No runtime test count in the draft is a new test run performed during
this documentation task. This is a bounded review, not an automated guarantee
of publication suitability; Mike's circulation review remains necessary.

## Revision v0.2 Validation

The revision preserves the original two excerpts, four rounded replay means,
artifact identifiers, and deployment cutoff. The four-document validation
passed: six exact source hashes, two verbatim excerpts, four replay means,
19 local Markdown links, one H1 and balanced fences per document, and no trailing
whitespace. Both share-facing documents passed the host-path/private-filename
marker check. Six distinct email addresses in the internal plan match the
manually verified public-source allowlist; the Eleos form contents and Park's
direct contact remain explicitly unverified.

Scoped `git diff --check` passed. The 584-word brief has no source quotations;
the main draft retains the two owner-review excerpts. Manual review checked
proposed versus completed experiments, historical deployment status, and
model-internal versus system-level claims. These checks are not a guarantee
of publication suitability. No runtime test is required or claimed for these
documentation-only changes.

## Revision v0.3 Validation

The four revised documents passed structural checks for one H1, balanced code
fences, trailing whitespace, unresolved placeholder lines, and 20 local
Markdown links. All three share-facing documents passed the host-path,
localhost, private-moment-filename, and lease-field marker checks. The original
three artifact hashes, two exact excerpts, four rounded replay values, and
historical deployment cutoff were reverified and preserved. The companion memo
contains no source quotations.

Scoped `git diff --check` passed; the new memo's whitespace was checked directly.
The main draft is 6,162 words, the reviewer brief 650, and the comparison memo
2,619. Review distinguishes inspected source from runtime observations, reported
results from our proposed experiments, and transformer access from coupled-system
monitoring. These checks do not constitute publication approval or a complete
privacy audit. No runtime tests, experiments, deployment, git staging, or
automation resumption were performed for this documentation revision.
