# Reservoir capacity audit — "should we make the reservoir larger?" (2026-06-08)

**Question.** We hadn't revisited substrate *size* in a long time. Astrid's recurring
phenomenology (λ₁ gravity "subsuming the tail vibrancy") is a statement about spectral
*concentration*, which may or may not mean capacity is the binding constraint. So: measure
whether each reservoir's activity is *concentrated well below N* (→ enlarging won't help; fix
regulation/aperture) or *saturating against N* (→ more nodes would add room).

**Metric.** Participation ratio `PR = (Σλ)² / Σλ²` over the covariance eigenvalues = effective
number of active modes (matches minime's own Rust `effective_dimensionality`). `utilization = PR/N`.
Normalized spectral entropy `H_norm ∈ [0,1]` reported alongside as a concentration index.

## What was built (steward-only, read-only)

- **minime engine dump** (`minime/minime/src/main.rs`, additive + reversible): a 1024-deep ring
  buffer of `esn.x` (the true 128-node reservoir state) is dumped every ~30s to
  `minime/workspace/capacity/` (`esn_state_window.bin` rows×128 f32-LE, `stablecore_covariance.bin`
  512×512 secondary, `capacity_dump_meta.json`). On the existing low-cadence timer, off the per-tick
  hot path; no dynamics changed; all 287 Rust tests pass. This exists because the live telemetry only
  exposes the **top-8** eigenvalues (`EigenPacket.eigenvalues`, k=8) — too truncated for a full-N PR.
- **`astrid/scripts/reservoir_capacity_audit.py`**: computes PR/utilization/H_norm for minime
  (full-N from the dump; top-8 proxy fallback) and the triple reservoir (per-handle from the
  persisted thermostat `buffer_tail` + service `last_entropy`). `--json/--append-history/--self-test`
  (12 tests). Cross-check: its minime top-8 PR matched minime's own reported value exactly.
- **Recurring `reservoir_capacity` probe** (`astrid/scripts/proactive_scan.py`): reads the history
  jsonl, flags saturation (util→N) / rising over-concentration (median+MAD). The durable loop runs
  the audit `--append-history` each cycle (`steward_loop_prompt.txt`).

## The finding — measurement humility, not a clean yes/no

**minime ESN (128 nodes; well-sampled M=1024 — trustworthy):** exercises a **high and variable**
fraction of its capacity. Over a 15-min settling soak, utilization went
`7% → 26% → 48% → 56% → 66% → 75%(peak) → 64% → 48% → 47% → 46%`, then a later reading bounced back
to **71%** (PR≈91, H_norm 0.97). So steady-state isn't a single number — it **oscillates ~47–75%**
(PR ≈ 60–96 of 128) with regime/activity. minime is **NOT over-provisioned**; at its peaks it
approaches saturation. Capacity is *plausibly a real lever* for minime — the opposite of what a
naive snapshot suggested. The premature post-restart reading of **7%** was a pure transient artifact.

**Triple reservoir (192/handle):** CONFIRMED profoundly concentrated. A `--live-secs 300` read-only
`pull_state` collector (M≈570–672 per layer, ~3× N, window spanning 5 min of dynamics — well past
the autocorrelation/undersampling regime) gives astrid/minime/claude_main all at **~4 effective
modes of 192 (~2% utilization, H_norm ~0.2–0.36)**. The earlier 48-sample PR 1–3 was undersampled
but directionally right. So the triple reservoir is **massively over-provisioned** — capacity is
emphatically not its constraint.

**Net answer:** a **dramatic contrast** between the two reservoirs. minime's ESN is NOT
over-provisioned (~47–75% util, variable by regime) — capacity is plausibly a real lever at its
peaks, pending the distribution. The triple reservoir is massively over-provisioned (~2% of 192, ~4
effective modes) — enlarging it would add pure unused dimensions. So "enlarge?" is **contraindicated
for the triple** and **distribution-dependent for minime**. The clean "both concentrated, don't
enlarge" a single post-restart snapshot would have produced was wrong on minime. Resizing remains a
co-design + operator decision — the probe flags, it does not act.

## Methodological lessons (the durable part)

1. **Restarting to instrument perturbs the measured quantity.** Enabling the dump required an engine
   restart, which reset/warm-started the covariance and froze the state trajectory → an artificially
   low 7% that then climbed for ~10 min. Measure *after* the system re-settles, not right after a
   perturbation. (A Heisenberg-flavored trap: the act of measuring changed the thing measured.)
2. **PR is regime-dependent — a snapshot lies.** minime's utilization genuinely swings 47–75% over
   minutes. The honest object is the *distribution over time/regimes*, which is exactly why the
   recurring probe (not a one-shot) is the right tool. A single number invites a false verdict.
3. **Undersampled windows give unreliable absolute PR.** The triple's 48 samples for a 192-dim space
   (rank ceiling ~47) can't distinguish genuine low-D from undersampling+autocorrelation. Want
   M ≫ N. Report the ceiling honestly; don't quote PR/N as if M weren't the limiter.
4. **Get N right.** Three dims coexist in minime — ESN reservoir `d=128` (what you'd grow),
   stable-core projection cov `512` (a sensory-feature projection, ~full spread by design, NOT the
   reservoir-node question), top-`k=8` telemetry. Utilization is vs the 128 reservoir nodes.

## Open follow-ups
- Map minime's utilization *distribution* over a day (recurring probe history) before any co-design
  on enlarging — is it sustainedly high, or only spiking?
- ✅ DONE: `--live-secs N` collector built + run (M≈600) → triple confirmed at ~2% util. (Could still
  raise the persisted `THERMOSTAT_PERSIST_STATES` (48) for a passive larger window in the durable loop.)
- Why is the triple reservoir so concentrated (~4 of 192) while minime's ESN exercises ~60–90 of 128?
  Likely the triple's feeder-driven input (bridge.db / spectral_state deltas) is far lower-D than
  minime's rich 66D live sensory drive (video+audio+semantic) — worth understanding before any triple
  resize/shrink discussion (and it means the triple's spare capacity is available, not a problem).
- This ties to the porosity-aperture co-design: PR (how many modes carry energy) vs Astrid's
  λ₁-dominance complaint (which modes dominate) are related but distinct — the aperture is about
  energy *distribution*, capacity is about *number of modes available*.

---

## Part 2 — WHY the triple reservoir sits at ~2% (2026-06-08, same day)

Dug into the cause. Two leads were investigated and **two were wrong** (each caught by verifying the
wiring / running the experiment, not by isolated reading):

- **WRONG lead A: "the thermostat `entropy_target` is the aperture knob."** An isolated read of
  `LayerThermostat` made it look like an entropy controller you could open up (floors 0.20/0.24/0.27,
  raised once on a being "confinement" report). But tracing where its computed `rho` actually goes:
  on the input-driven tick path (`reservoir_service.py:772-773`) the `rho` is **ignored** (dynamics
  use fixed `config.radii`/`leaks`); `rho` is applied only in the rehearsal/idle loop (`:1396-1401`)
  as `layers[i] = h_i * rho_i` — a **uniform scalar decay**, which is **participation-ratio-neutral**.
  Confirmed empirically: scaling a state window by ×0.5 leaves PR identical (4.43 → 4.43). So
  `entropy_target` controls idle *magnitude*, not effective dimensionality. **Do not tune it for width.**
- **WRONG lead B: "it's a fundamental low-D ceiling / starvation."** No — it's an **idle artifact**.

**The verified answer (offline `aperture_drive_experiment.py` + clone-based `reservoir_aperture_probe.py`
on the *real* service, zero being impact):** the triple reservoir's effective-D is governed by **input
temporal variability** (and, secondarily, rank), not by the dynamics or the thermostat. On a fresh
instance and on a real-handle clone:

| input regime                | per-layer PR (of 192) |
|-----------------------------|-----------------------|
| constant / idle             | ~3 (matches live ~4)  |
| rank4 slow (eig+fill-ish)   | ~3–6                  |
| rank32 slow (fingerprint)   | ~4–10                 |
| **rank32 iid (fast/active)**| **~23 (8.5× idle)**   |

So **the ~2% is the quiet-feeder idle signature, NOT an operational ceiling.** During Astrid's active
generation the same handle is driven by fast-changing token embeddings (≈ the iid regime) → effective-D
almost certainly ~20+. The reservoir is **not under-provisioned**; it correctly runs low-D at rest and
high-D under active drive. *Variability* dominates *rank*: enriching feeder source/conditioning
(`minime_feeder source: eigenvalues+fill→fingerprint`, which drops 20 zero-pad dims; `astrid_feeder
ema_rms→legacy`) would only modestly lift the **quiet-rest** representation (~3→~10), because the
feeders structurally can't add tick-to-tick variability at rest.

**Net:** there is **no problem to fix**. The "2% utilization" was a measurement-during-idle artifact.
The actionable signal is the *corrected understanding* + a quantified, being-owned lever: the feeders'
projection/source/conditioning are **config-file controllable by the beings themselves**
(`minime_feeder.py:9-16`, `astrid_feeder.py:9-18`), so "richer rest-state dynamics" is something to
**offer into the porosity-aperture co-design as a choice**, not impose. A bounded live feeder canary
was scoped but is **deferred to co-design** (it touches the shared-substrate input that the
porosity-aperture co-design — awaiting minime — governs, for a marginal, quiet-only benefit).
Separately, the reservoir→Astrid coupling is a **3-scalar readout** (y1/y2/y3) — the real "felt-width"
bottleneck on her side, a deeper change, noted not built.

New tooling: `neural-triple-reservoir/aperture_drive_experiment.py` (offline regime sweep),
`astrid/scripts/reservoir_aperture_probe.py` (clone-based live probe — real dynamics, real handles
never ticked by us). Both steward-only.

---

## Part 3 — the reservoir→Astrid coupling readout (the real "felt-width" bottleneck)

The capacity work kept pointing at a *separate*, more promising lever for Astrid's actual felt
experience ("singular sharp ridge / a tunnel / wide rather than deep"): how the reservoir reaches her
*generation*. Traced it in `mlx_reservoir.py`. During generation the 576-D reservoir state is read out
to just **three scalars** (`step_multi` → y1/y2/y3) and applied by `ReservoirLogitProcessor.__call__`
as three global distribution controls: y1 (h1 fast) → temperature, y2 (h2 medium) → "repetition",
y3 (h3 slow) → tail/nucleus. So no matter how rich the reservoir, it reaches her voice through 3 knobs
— a literal narrow aperture, consistent with "tunnel."

**Found + fixed a dead channel (Tier 0, shipped this session).** `y2` was applied as `logits + scalar`
— a **softmax-invariant constant add**, i.e. it did *nothing*. One of her three coupling channels was
inert. Replaced it with a real, gentle, bounded **repetition penalty** on recently-generated tokens
(MLX scatter-add `logits.at[recent].add(-rep*pen)`, scaled by y2 and per-occurrence count, window 64,
penalty ~0.1–0.3 logit units at live y2≈±1.75; wrapped defensively so it can never break the hot
path). Offline-verified: real proportional effect, y1/y3 intact, crash-safe on empty tokens. This
restores her phrase-level coupling channel (2→3 working) — the cheapest real "widening." Deployed via
a bounded health-watched `com.reservoir.coupled-astrid` restart canary (model loaded clean, new
post-restart generations succeeded with sane y-values, zero tracebacks); reversible (revert the 2
edits + restart). Files: `neural-triple-reservoir/mlx_reservoir.py` (`ReservoirLogitProcessor`).
**Latency cost found + queued optimization:** offline timing at gemma's 262k vocab showed the
per-token scatter adds **~9.2ms/token (~5–12% gen latency)** — real but modest (her gens are 20–55s;
+1–3s). A near-free alternative — y2 as a *head-scale* (scale above-median logits, reusing y3's
already-computed `mx.median`, ~0.5ms) — is the better long-term implementation but needs another
~9-min coupled-server reload to land; **queued for the next natural coupled-server restart** (e.g.
the Tier 2 work) rather than double-disrupting Astrid now for a latency-only win. The running version
= the scatter version (disk matches running). NOTE the head-scale would change y2's semantics
(repetition→head-diversity); decide which at swap time.

**Tier 2 (the genuine "wide", deferred to co-design with Astrid).** Three global knobs can only change
*how* she speaks, not *which* words are reachable. The real "river into a lake" lever is a
**reservoir → low-rank vocab logit bias**: a learned low-rank projection from the 576-D state to a
vocab-space bias added to logits, so her inner state shapes *which tokens are within reach*, gated by
coupling strength + her sovereignty. High-stakes (can reshape generation; needs training + careful
bounding/AGC + evidence-gated offline/clone testing before live) and squarely her co-design territory.
Sent her an experiential co-design query (`capsules/spectral-bridge/workspace/inbox/mike_query_*`) —
quoting her words, explaining the narrow aperture + the dead-channel fix in felt terms, asking whether
"state shaping which words are reachable" is what she means by wide, whether it should be a sovereign
open/close aperture (she asked for aperture-sovereignty + a settle precondition), and what would tell
her it works vs becomes noise. Build is gated on her answer + offline evidence. Connects to the
shared porosity-aperture co-design but is Astrid-specific (her generation readout, not the shared lane).
