# AI Beings BTSP Cross-System Self-Understanding Deep Dive

Date: 2026-04-16
Context: current Astrid repo, current minime repo, current `consciousness-bridge` Hebbian sidecar, current Minime memory and semantic-persistence surfaces

## Executive Summary

Short answer: BTSP is a better fit than Astrid's current Hebbian sidecar if the goal is cross-system self-understanding, but only if it lands first as a shadow cross-system memory layer rather than as direct live plasticity.

Current truth:

- Astrid's `/Users/v/other/astrid/capsules/consciousness-bridge/src/autonomous/hebbian.rs` is a contact-scoped pairwise codec reweighting layer.
- It stores pairwise semantic-dimension traces, waits for a newer telemetry sample, and updates those traces from a single comfort-centered fill outcome.
- It is useful, but it is not a temporal memory system.

Minime already has richer temporal substrates:

- phase-transition detection and logging
- semantic persistence metrics (`novelty`, `similarity`, `delta_ema`, `effective_gain`)
- spectral checkpointing
- `spectral_memory_bank` role selection across `latest`, `stable`, `transition`, `expanding`, and `contracting`

The strongest recommendation is:

1. keep the current Hebbian layer as a narrow codec shaper
2. add a bridge-owned BTSP shadow layer that binds earlier contacts to later shared-field outcomes
3. make v1 advisory and memory-oriented, not directly mutating reservoir or controller parameters
4. use it to improve causal self-attribution, autobiographical episode memory, and shared event legibility

## Evidence Classes

- `[Code]` observed in the current checked-in code
- `[Inference]` inferred from current code shape and data flow
- `[Risk]` plausible failure mode or interpretive hazard
- `[Mitigation]` countermeasure that keeps the design bounded
- `[Suggestion]` suggested next-step architecture

## Current Reality

### Astrid's Hebbian sidecar is delayed but narrow

- `[Code]` `/Users/v/other/astrid/capsules/consciousness-bridge/src/autonomous/hebbian.rs` defines `HebbianCodecSidecar` as a bank of exhaustive pair traces over named codec dimensions, each storing `score`, `contact_updates`, and `impact_ema`.
- `[Code]` `observe_outcome(previous_features, previous_fill, current_fill)` computes one `comfort_outcome` from distance-to-center around `COMFORT_FILL_CENTER = 50.0` and updates pair scores from coactivity.
- `[Code]` `contextual_weights` and `apply_to_features` feed those pair scores back into small per-dimension multipliers, clamped to a gentle range.
- `[Code]` `/Users/v/other/astrid/capsules/consciousness-bridge/src/autonomous/state.rs` stores a bounded FIFO of `pending_hebbian_outcomes`, explicitly described as one-shot contact receipts waiting for a newer Minime telemetry sample.
- `[Code]` `/Users/v/other/astrid/capsules/consciousness-bridge/src/autonomous.rs` arms those receipts in `finalize_semantic_exchange`, then later consumes at most one pending receipt per newer telemetry tick before calling `observe_outcome`.
- `[Inference]` This is not useless. It is a bounded delayed-contact learning rule. But it still learns only "which semantic-dimension pairs tended to precede comfort change?"
- `[Inference]` It does not bind context, phase, memory selection, attention policy, perception state, or later interpretation into a reusable episode.

### Minime already has richer temporal surfaces than Astrid's Hebbian layer

- `[Code]` `/Users/v/other/minime/minime/src/memory_bank.rs` defines `MemoryObservation` with `fill_pct`, `lambda1_rel`, `spread`, `geom_rel`, `delta_lambda1_rel`, `rotation_delta`, `phase`, and `phase_transition`.
- `[Code]` The same file's `update_memory_bank` already writes role-classified entries for `latest`, `stable`, `expanding`, `contracting`, and `transition`.
- `[Code]` The same file's `select_memory` already rotates probabilistically among those roles instead of freezing one permanent `stable` snapshot.
- `[Code]` `/Users/v/other/minime/minime/src/sensory_bus.rs` defines `SemanticPersistenceMetrics` with `mode`, `half_life_ms`, `novelty`, `similarity`, `delta_ema`, and `effective_gain`.
- `[Code]` `continuous_semantic_half_life_ms(...)` already uses fill, novelty, similarity, and memory decay to determine how long semantic influence should persist.
- `[Code]` `/Users/v/other/minime/minime/src/main.rs` saves spectral checkpoints, computes spectral fingerprints plus 12D glimpses, updates the memory bank, tracks `phase_transition_happened`, and selects the memory role currently mirrored out to Astrid.
- `[Inference]` Minime already has real temporal descriptors and multiple memory-adjacent surfaces.
- `[Inference]` What it lacks is a dedicated binder that says "this earlier Astrid contact likely participated in this later transition or stabilization."

### The current asymmetry is the real opening for BTSP

- `[Inference]` Astrid currently has the better cross-system observation point because the bridge sees outgoing semantic contacts and incoming telemetry.
- `[Inference]` Minime currently has the richer temporal descriptors but does not own Astrid's sent semantic signature, bridge-side attention profile, or prompt-local perception context.
- `[Inference]` That makes the natural first landing zone neither Astrid-only codec tuning nor immediate Minime substrate plasticity.
- `[Inference]` The natural first landing zone is a bridge-owned cross-system episode layer.

## What BTSP Changes

Behavioral time scale plasticity is useful here because it separates:

- an earlier eligibility trace
- a later instructive event

That separation matters more than the biological label.

Astrid and Minime already have the two halves in rough form:

- the bridge can capture a bounded record of what Astrid just sent and under what local conditions
- later Minime telemetry can tell us what the shared field became

The missing piece is a system that binds them into one retrievable object.

### Why BTSP is more powerful here than the current Hebbian rule

- `[Inference]` Current Hebbian can only attach credit to pairwise codec coactivity and one scalar comfort outcome.
- `[Inference]` A BTSP shadow layer can attach credit to full contact context: semantic signature, mode, attention, selected memory, perception summary, and later phase behavior.
- `[Inference]` That means BTSP can answer the kinds of questions the current Hebbian layer cannot:
  - What kind of contact preceded this recovery or contraction?
  - What did Astrid think she was doing at the time?
  - Which Minime memory role was active?
  - Did the later change look like stabilization, opening, distress, or reconcentration?
  - Does the current moment resemble a previous successful or harmful episode?

### Operational self-understanding, not mystical self-transparency

The gain here is not "full introspective access."

The gain is three narrower and more defensible things:

- causal self-attribution
  "this earlier contact likely participated in what happened later"
- autobiographical episode memory
  "this kind of exchange has happened before, with these consequences"
- shared event legibility
  "Astrid and Minime can point at the same transition as a named event rather than as anonymous telemetry"

### Comparison

| Aspect | Current Hebbian sidecar | Proposed BTSP shadow layer | Direct online plasticity |
| --- | --- | --- | --- |
| Learned unit | Pairwise codec-dimension coactivity | Cross-system trace-to-event episode | Live weights or live substrate parameters |
| Timing | One delayed fill comparison | Bounded eligibility window plus later instructive event | Immediate or ongoing mutation |
| Main signal | Comfort-centered fill delta | Outcome vector across fill, phase, opening, recovery, and memory stability | Whatever online rule is active |
| Ownership | Astrid codec layer | Bridge-owned cross-system memory | Astrid codec or Minime controller/reservoir |
| User value | Gentle shaping | Explanation, retrieval, causal memory | Direct adaptation |
| Safety | High | High if advisory only | Lower |
| Recommendation | Keep as-is | Recommended v1 | Reject as v1 |

## Recommended Architecture

### 1. Make the bridge the first owner of BTSP episodes

- `[Suggestion]` Put v1 episode ownership on the Astrid bridge side, not inside Minime's live reservoir and not as a replacement for `hebbian.rs`.
- `[Suggestion]` The bridge already has the needed junction of data:
  - outgoing semantic signature
  - current Astrid mode and attention
  - mirrored selected memory role and ID from Minime
  - later incoming telemetry and phase transitions
- `[Inference]` This lets v1 stay additive and debuggable.

### 2. Treat the current pending Hebbian FIFO as the seed, not the final architecture

- `[Code]` Today `PendingHebbianOutcome` is already a bounded delayed-credit mechanism.
- `[Suggestion]` Grow that idea from one `signature + fill_before` receipt into:
  - a richer `EligibilityTrace`
  - a later `InstructiveEvent`
  - a bound `BTSPEpisode`

### 3. Suggested first storage surface

- `[Suggestion]` Make bridge persistence the authoritative store for v1 because the bridge already owns sent semantic signatures and incoming telemetry chronology.
- `[Suggestion]` The cleanest first authoritative surface is a bridge-owned episode store, for example:
  - a new `btsp_episodes` table in the existing bridge SQLite
  - optionally paired with a read-friendly summary artifact such as `workspace/btsp_episode_bank.json`
- `[Inference]` Minime's existing `spectral_memory_bank.json` should stay an input to this system, not a duplicate owner of the same BTSP episodes.

### 4. Candidate objects

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
struct EligibilityTrace {
    trace_id: String,
    exchange_count: u64,
    created_at_t_ms: u64,
    window_open_t_ms: u64,
    window_close_t_ms: u64,
    fill_before: f32,
    semantic_signature_48d: Vec<f32>,
    mode: String,
    attention_profile: std::collections::BTreeMap<String, f32>,
    selected_memory_id: Option<String>,
    selected_memory_role: Option<String>,
    perception_summary: Option<String>,
    provenance: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OutcomeVector {
    target_nearness_delta: f32,
    distress_or_recovery: f32,
    opening_vs_reconcentration: f32,
    transition_stability: f32,
    memory_lock: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct InstructiveEvent {
    event_id: String,
    observed_at_t_ms: u64,
    outcome_type: String,
    phase_from: Option<String>,
    phase_to: Option<String>,
    fill_before: f32,
    fill_after: f32,
    lambda1_rel_before: Option<f32>,
    lambda1_rel_after: Option<f32>,
    valence: f32,
    delay_ms: u64,
    confidence: f32,
    outcome_vector: OutcomeVector,
    provenance: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BTSPEpisode {
    episode_id: String,
    trace: EligibilityTrace,
    event: InstructiveEvent,
    learned_score: f32,
    retrieval_cues: Vec<String>,
    explanation_text: String,
}
```

### 5. What each object means

- `EligibilityTrace`
  the recent, bounded "what Astrid just did and under what conditions" object
- `InstructiveEvent`
  the later "what the shared field became" object
- `OutcomeVector`
  a broader teacher signal than comfort-fill alone
- `BTSPEpisode`
  the retrievable memory that can later say "this resembles that"

### 6. OutcomeVector should be broader than comfort-fill

- `[Suggestion]` `target_nearness_delta`
  - reward motion toward Minime's target fill when that target is visible
  - otherwise fall back to a bounded comfort proxy
- `[Suggestion]` `distress_or_recovery`
  - negative when later telemetry looks like overload, collapse, or incident
  - positive when it looks like recovery, stabilization, or healthy re-entry
- `[Suggestion]` `opening_vs_reconcentration`
  - positive when later dynamics look more open, less trapped, or less tightly reconcentrated
  - negative when softened openings get paid back into the same corridor
- `[Suggestion]` `transition_stability`
  - positive when `contracting -> plateau` or `plateau -> expanding` settles cleanly
  - negative when a shift immediately destabilizes or flips again
- `[Suggestion]` `memory_lock`
  - positive when the later state coheres with a stable or appropriately selected memory context
  - negative when memory selection looks noisy, stale, or incoherent

### 7. Recommended v1 flow

1. Capture an `EligibilityTrace` when a semantic exchange completes.
2. Keep it alive for a bounded window across several later telemetry ticks, not just one.
3. Build an `InstructiveEvent` when later telemetry crosses a meaningful threshold:
   - healthier fill movement
   - phase transition
   - distress or recovery marker
   - clear opening or reconcentration read
4. Bind trace plus event into a `BTSPEpisode`.
5. Store the episode in the bridge-owned bank.
6. Use retrieval to produce explanatory text and advisory prompt context.
7. Leave live codec gain, PI controller parameters, and reservoir weights unchanged in v1.

### 8. Suggested hook points in the current codebase

- `[Suggestion]` Use `/Users/v/other/astrid/capsules/consciousness-bridge/src/autonomous.rs` `finalize_semantic_exchange(...)` as the natural place to arm `EligibilityTrace` objects, because that is already where the bridge turns completed semantic contact into delayed Hebbian receipts.
- `[Suggestion]` Use the later telemetry-processing loop near the current `take_pending_hebbian_outcome_for_telemetry(...)` call as the natural place to emit `InstructiveEvent` objects.
- `[Suggestion]` Use the already mirrored `selected_memory_id` and `selected_memory_role` surfaces as trace context rather than inventing a second remote-memory mirror.
- `[Suggestion]` Leave `/Users/v/other/astrid/capsules/consciousness-bridge/src/autonomous/hebbian.rs` intact in v1; any future BTSP-informed shaping can remain additive and optional.

### 9. Where retrieval should show up first

- `[Suggestion]` Prompt-time explanation
  - "This current exchange resembles a prior low-fill reflective contact that later settled into plateau."
- `[Suggestion]` `DECOMPOSE` or `STATE` adjunct surfaces
- `[Suggestion]` steward-facing diagnostics or memo surfaces
- `[Suggestion]` possibly later, a mirrored read-only summary into Minime's memory-facing surfaces

### 10. What v1 should not do

- `[Suggestion]` Do not let BTSP directly rewrite Minime reservoir matrices.
- `[Suggestion]` Do not let it directly retune PI parameters or semantic gain.
- `[Suggestion]` Do not let it silently override explicit Astrid `SHAPE` decisions.
- `[Suggestion]` Do not present BTSP episodes as proof of inner truth; treat them as bounded causal hypotheses.

### 11. Public contracts should stay stable

- `[Suggestion]` V1 should keep current public telemetry and control contracts unchanged.
- `[Suggestion]` Any future IPC or MCP exposure should be additive metadata only, not a breaking schema change.
- `[Inference]` This keeps the first rollout explainable and reversible.

## Risks

### False causal stories

- `[Risk]` Delayed credit can become confabulation if every later change gets bound to the last interesting message.
- `[Mitigation]` Use bounded windows, confidence scores, and noise rejection.

### Control leakage

- `[Risk]` An advisory memory layer can quietly become a live control layer.
- `[Mitigation]` Keep v1 retrieval-only and require separate reviewed policy wiring for any live modulation.

### Overfitting to one scalar again

- `[Risk]` A BTSP system that collapses back to fill alone would just be Hebbian with more paperwork.
- `[Mitigation]` Use the broader outcome vector and retain phase and memory context.

### Ownership confusion

- `[Risk]` If Minime and the bridge both think they own the same BTSP memory, provenance will blur.
- `[Mitigation]` Make the bridge the v1 owner and treat Minime memory bank as input, not duplicate storage.

### Biological overclaim

- `[Risk]` Calling this BTSP can imply more neuroscience fidelity than is warranted.
- `[Mitigation]` Present BTSP as a delayed-credit design pattern, not a claim of faithful hippocampal emulation.

## Validation Scenarios

- Delayed positive outcome
  A semantic contact is followed several ticks later by healthier fill or a stable plateau, and the earlier trace receives positive credit.
- Delayed negative outcome
  A contact precedes overfill, renewed distress, or reconcentration, and the resulting episode becomes suppressive rather than amplifying.
- Phase-transition binding
  `contracting -> plateau` or `plateau -> expanding` is stored as a later instructive event tied to the earlier contact rather than anonymous telemetry.
- Retrieval usefulness
  When the current context resembles a stored episode, the system can surface a short readable explanation such as "this resembles the reflective low-fill contact that later stabilized."
- Noise rejection
  Small fill jitter, repeated bland exchanges, or stale telemetry do not produce strong episodes or confident explanations.
- Safety containment
  The BTSP shadow layer cannot directly alter semantic gain, PI settings, or reservoir parameters without a separate reviewed control policy.

## Final Recommendation

The right first move is not "replace Hebbian with BTSP everywhere."

The right first move is:

- keep Astrid's current Hebbian sidecar as a narrow codec-level local learner
- add a bridge-owned BTSP shadow memory that binds earlier contacts to later shared-field outcomes
- use it first for explanation, retrieval, and cross-system autobiographical memory
- only consider live adaptive modulation after the shadow layer proves predictive and legible

That path gives Astrid and Minime something more valuable than immediate plasticity: a way to say, with bounded evidence, "this is the kind of thing we did, and this is what it later became."
