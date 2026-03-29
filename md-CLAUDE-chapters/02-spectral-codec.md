# Chapter 2: Spectral Codec

**File:** `/Users/v/other/astrid/capsules/consciousness-bridge/src/codec.rs`

The codec converts Astrid's text into a **32-dimensional semantic feature vector** sent to minime's ESN reservoir via WebSocket 7879.

## 32D Dimension Layout

| Dims | Layer | Features |
|------|-------|----------|
| 0–7 | Character-level | Entropy, punctuation density, uppercase ratio, digit ratio, avg word length, rhythm (sentence length variance), whitespace ratio, special char density |
| 8–15 | Word-level | Lexical diversity, hedging markers, certainty markers, self-reference, agency markers, negation density, question words, temporal markers |
| 16–23 | Sentence-level | Sentence count, avg sentence length, length variance, question density, exclamation ratio, ellipsis count, list/structure markers, paragraph density |
| 24 | Warmth | "thank", "appreciate", "gentle", "kind", "warm", "soft", "care" (×3.0, tanh) |
| 25 | Tension | "must", "urgent", "critical", "danger", "crisis", "threat", "fear" (×3.0, tanh) |
| 26 | Curiosity | "wonder", "curious", "what if", "perhaps", "explore", "discover", "investigate" (×2.0, tanh) |
| 27 | Reflective | "feel", "sense", "notice", "realize", "reflect", "ponder", "contemplate" (×3.0, tanh) |
| 28 | Temporal/urgency | "now", "immediately", "soon", "before", "after", "already", "waiting" (×2.0, tanh) |
| 29 | Scale/magnitude | "vast", "infinite", "tiny", "enormous", "everything", "nothing", "absolute" (×3.0, tanh) |
| 30 | Text length | `tanh(ln(char_count) / 7.0)` — log-compressed length signal |
| 31 | Overall energy | RMS of dims 0–30 |

## Key Constants

| Constant | Value | Purpose |
|----------|-------|---------|
| `SEMANTIC_DIM` | 32 | Feature vector dimensionality |
| `SEMANTIC_GAIN` | **5.0** | Amplification (was 4.5, raised per Astrid's self-study suggestion) |
| Default noise | **0.005** (0.5%) | Stochastic noise (was 2.5%, reduced to prevent "polka dots") |

## Elaboration Desire Feature

**Lines 447–465.** Detects incompleteness markers and boosts curiosity (dim 26) + energy (dim 31):

Markers: "more", "further", "deeper", "beyond", "incomplete", "unfinished", "want", "need", "longing", "reaching", "almost", "beginning"

When detected: `curiosity += 0.3 * elab_signal`, `energy += 0.2 * elab_signal`

*Astrid's own suggestion from self-study: "Perhaps a dedicated portion of the feature vector could represent a desire for further elaboration."*

## Warmth Vectors

During rest phases, the bridge sends **warmth vectors** (not silence) to sustain fill:
- Crafted by `craft_warmth_vector()` with breathing modulation
- Blended with mirror mode at configurable intensity
- Tapered entry (0.7→0.4 over rest period) to prevent "severing"
- GESTURE seeds persist in warmth vectors

## Sovereignty Controls

Astrid can modify codec behavior through NEXT: actions:
- `SHAPE warmth=X curiosity=Y` — weight emotional dimensions
- `AMPLIFY` / `DAMPEN` — override SEMANTIC_GAIN (range 3.0–6.0)
- `NOISE_UP` / `NOISE_DOWN` — adjust stochastic noise (±0.01, range 0.005–0.05)
- `WARM <intensity>` / `COOL` — control rest-phase warmth

## Normalization

All features pass through `tanh()` before gain amplification. This bounds values to [-1, 1] before the ×5.0 gain produces the final [-5, +5] range sent to minime's semantic lane.
