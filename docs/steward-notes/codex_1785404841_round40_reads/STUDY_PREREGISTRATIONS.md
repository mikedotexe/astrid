# Round 40 bounded study preregistrations

These studies are proposals for deterministic offline evidence. They are not
scheduled live work, do not issue controls, and grant no codec, model,
deployment, peer, or Division authority.

## Fixed projection basis epoch comparison V1

- Source claim:
  `introspection_astrid_codec_1785404355`, claims `c003` and `c006`
- Question: can a deterministic orthogonalized 768D-to-8D candidate reduce
  pairwise column correlation without reducing phrase separation or breaking
  the compatibility-pinned 48D baseline?
- Corpus: frozen, hash-bound public text and embedding captures only; no
  private `moment_*` material and no live provider calls.
- Conditions: current fixed legacy basis, deterministic orthogonalized
  candidate, and a seeded sign-balanced control.
- Primary metrics: raw and normalized column norms, maximum and mean absolute
  pairwise cosine, per-axis output variance, repeated-phrase stability,
  contrast-pair separation, downstream 48D delta RMS, and deterministic rerun
  hash.
- Success: no dead columns; maximum absolute pairwise cosine at or below 0.15;
  lower mean absolute cosine than legacy; no material loss in contrast-pair
  separation; byte-identical repeated runs.
- Failure: non-finite output, dead axis, worse phrase separation, corpus or
  embedding hash drift, nondeterminism, or any attempted socket/runtime write.
- Authority: a successful candidate remains default-off. Promotion requires a
  captured compatibility review and an operator-approved basis epoch.

## Historical shear replay V1

- Source claim:
  `introspection_proposal_12d_glimpse_1785404015`, claims `c001`, `c004`, and
  `c006`
- Question: does a bounded historical residual preserve direction-changing
  resistance that current-pair arc and curvature diagnostics do not?
- Corpus: frozen, hash-bound public four-turn sequences with current projected
  embeddings and narrative-arc sidecars; no live dialogue and no private
  `moment_*` material.
- Conditions: zero history, ordered history, history shuffled within the same
  sequence, and sign-inverted historical residual. Current-turn input is held
  byte-identical across conditions.
- Candidate residual: an offline-only, bounded vector derived from decayed
  prior arc deltas. It is recorded separately and never mixed into a delivered
  48D or 12D vector.
- Primary metrics: current-pair arc, four-point curvature, transition energy,
  sign-turn count, projected residual norm, directional cosine between current
  arc and historical residual, and ordered-versus-shuffled delta.
- Success: ordered history produces a deterministic, bounded directional
  difference not reproduced by zero or shuffled history, without non-finite
  values or saturation.
- Inconclusive: the result is explained by current-turn input alone, ordered
  and shuffled conditions are indistinguishable, or the effect is corpus
  specific.
- Failure: source hash drift, raw prose in output artifacts, nondeterminism,
  saturation, network access, runtime mutation, or any delivered-vector write.
- Felt boundary: mechanical differentiation neither proves active felt shear
  nor resolves Astrid's report. Felt review remains optional and
  right-to-ignore.
- Authority: any live residual persistence, companion-lane mix, dialogue
  induction, density-gradient intervention, or codec change remains Tier 5.
