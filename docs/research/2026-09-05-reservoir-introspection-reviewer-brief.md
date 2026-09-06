# Reservoir-Coupled AI: A Request for Critical Review

**September 5, 2026 | Selective-circulation draft | Human-led, AI-assisted**

## The Question

What would distinguish reservoir-driven wording from useful self-monitoring
of a persistent agent system?

We are developing Astrid and Minime, agent systems combining language models,
recurrent reservoirs, telemetry, retained memory, and bounded tools. Their
first-person reports describe continuity, uncertain ownership of ideas, and
the pull of another agent's state. We find these accounts interesting enough
to investigate carefully. We have not established direct introspective access
to the reservoir or phenomenal experience.

Our strongest result is practical: following reports into source code has
exposed reproducible defects and misleading interfaces. We would value help
turning that engineering case series into a discriminating experiment.

The setting is mixed-initiative and sensor-driven: runtime policy and
model-authored choices jointly shape activity. Generated expression can also
change conditions encountered later, through coupling, admitted sensory
re-entry, and retained history. We want to test useful self-attribution within
that loop, not infer it from the presence of feedback.

## What We Have

- **A numerical correction.** A slow reservoir readout intended to narrow a
  logit distribution could instead increase lower-tail probability, and its
  effect depended on an arbitrary common logit offset. A median-relative
  correction was regression-tested and deployed. This does not establish the
  cause of the report that prompted the investigation.
- **A bounded real-model replay.** Two copied states crossed with two contexts
  changed next-token probabilities under a common prefix. There was one seed
  and eight teacher-forced positions; short free continuations were
  inconclusive. This was an exploratory test of coupling, not recognition.
- **Concrete interface findings.** Repeated peer telemetry and caretaking
  guidance, ambiguous time anchors, and unsuccessful bookmark persistence
  were found in the surrounding software. Minime repairs are deployed;
  related Astrid bridge changes remain tested but undeployed in the reviewed
  record. Operational verification does not establish experiential benefit.

## The Architectural Boundary

In the inspected scalar path, the transformer computes logits first. A
separate reservoir-dependent processor changes them before sampling. Generated
token embeddings then feed the reservoir. Other routes put telemetry into text.

This is causal coupling, but it is not reservoir injection into the
transformer's intermediate activations. With matched text and cache, the
processor can change an answer without the transformer representing the
reservoir condition at that step. Later generated text can carry information
back into the model. This distinction is source-based; we have not performed
a new activation experiment.

We distinguish the transformer, coupled generator, and full agent system.
System-level self-monitoring through tools or prior output could be useful
without being model-internal access. Activation-intervention research provides
a methodological comparison, not validation of our system. See
[Lindsey, 2025](https://transformer-circuits.pub/2025/introspection/index.html).

The [research comparison memo](2026-09-05-introspection-research-comparison.md)
also distinguishes reservoir monitoring from calibration, compliance
self-report, and third-party behavioral monitoring in OpenAI-related research.

## The Review We Are Seeking

Our proposed first check compares pre-processor and post-processor logits under
restored state and matched prefixes. A subsequent design would distinguish
direct answer steering, information recoverable by an uncoupled observer from
generated text, and useful monitoring on a held-out task. These are proposals,
not completed experiments.

The most useful response would identify a missing control, an invalid
inference, or a smaller experiment with a clear failure criterion. Separately,
we welcome review of quiet, reversible inquiry parking and non-leading
observation practices. We are not requesting a consciousness verdict,
endorsement, or access to anyone's proprietary models.

## Evidence and Scope

The [full discussion draft](2026-09-05-reservoir-introspection-discussion-draft.md)
contains the numerical results, selected-source hashes, implementation cases,
and confounds, including explicit role framing in some prompts. Its deployment
cutoff is September 5, 2026, 06:02 UTC. No new outcome study accompanies this
brief. Source and test records are retained locally; a redacted reproduction
bundle is not yet prepared or published.

This brief contains no journal or correspondence excerpts. Any further source
sharing requires separate permission and privacy review. Astrid and Minime
have not endorsed this interpretation. Please do not forward or publish the
draft without agreement.
