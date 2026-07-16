
/// Classified text type based on dominant feature signals.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum TextType {
    Questioning, // question density dominant
    Hedging,     // hedging/uncertainty dominant
    Declarative, // certainty dominant
    Warm,        // warmth markers dominant
    Tense,       // tension markers dominant
    Curious,     // curiosity markers dominant
    Reflective,  // introspection markers dominant
    Neutral,     // no dominant signal
}

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct ResonanceModulation {
    pub discrete_amplifier: f32,
    pub continuous_resonance: f32,
    pub continuous_amplifier: f32,
    pub continuity_blend: f32,
}

#[derive(Debug, Clone)]
pub struct CodecWindowedInspection {
    pub raw_features: [f32; SEMANTIC_DIM],
    pub final_features: [f32; SEMANTIC_DIM],
    pub thematic_profile: [f32; THEMATIC_DIMS],
    pub text_type: TextType,
    pub text_type_signal: f32,
    pub base_semantic_gain: f32,
    pub base_resonance: f32,
    pub novelty_divergence: f32,
    pub effective_gain: f32,
    pub resonance_modulation: ResonanceModulation,
    pub projection_metadata: Option<ProjectionMetadata>,
    pub text_complexity_pressure: f32,
    pub time_domain_profile: TextTimeDomainProfile,
}

const TEXT_HISTORY_WARM_START_RATIO: f32 = 0.75;
const TEXT_HISTORY_WARM_START_MIN: usize = 3;
const CHAR_WINDOW_WARM_START_RATIO: f32 = 0.5;
const CHAR_WINDOW_WARM_START_MIN: usize = 128;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ThematicHistoryEntry {
    pub text_type: TextType,
    pub profile: [f32; THEMATIC_DIMS],
    #[serde(default = "default_thematic_weight")]
    pub weight: f32,
}

fn default_thematic_weight() -> f32 {
    1.0
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct TextTypeHistorySnapshot {
    #[serde(default)]
    pub entries: Vec<ThematicHistoryEntry>,
}

impl ResonanceModulation {
    fn neutral() -> Self {
        Self {
            discrete_amplifier: 1.0,
            continuous_resonance: 0.0,
            continuous_amplifier: 1.0,
            continuity_blend: 1.0,
        }
    }
}

/// Tracks recent text type classifications and computes a resonance
/// amplifier based on thematic recurrence.
pub struct TextTypeHistory {
    /// Ring buffer of recent text type classifications.
    pub ring: [TextType; MAX_RESONANCE_HISTORY_LEN],
    /// Continuous thematic profile history (parallel to ring).
    pub profile_ring: [[f32; THEMATIC_DIMS]; MAX_RESONANCE_HISTORY_LEN],
    /// Per-entry thematic memory weight, shaped by recency, signal, and novelty.
    pub weight_ring: [f32; MAX_RESONANCE_HISTORY_LEN],
    /// Number of entries filled so far.
    pub len: usize,
    /// Write position in ring.
    pub cursor: usize,
    /// Write position in profile ring (kept in sync with cursor).
    pub profile_cursor: usize,
}

impl Default for TextTypeHistory {
    fn default() -> Self {
        Self::new()
    }
}

impl TextTypeHistory {
    pub fn new() -> Self {
        Self {
            ring: [TextType::Neutral; MAX_RESONANCE_HISTORY_LEN],
            profile_ring: [[0.0; THEMATIC_DIMS]; MAX_RESONANCE_HISTORY_LEN],
            weight_ring: [1.0; MAX_RESONANCE_HISTORY_LEN],
            len: 0,
            cursor: 0,
            profile_cursor: 0,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    fn active_capacity(&self) -> usize {
        resonance_tuning()
            .history_len
            .min(MAX_RESONANCE_HISTORY_LEN)
    }

    /// Record a new text type classification.
    pub fn push(&mut self, tt: TextType) {
        let capacity = self.active_capacity();
        if capacity == 0 {
            return;
        }
        self.ring[self.cursor] = tt;
        self.cursor = (self.cursor + 1) % capacity;
        if self.len < capacity {
            self.len += 1;
        }
    }

    /// Count how many of the last `self.len` entries match `tt`.
    pub fn recurrence_count(&self, tt: TextType) -> usize {
        self.ring[..self.len].iter().filter(|&&t| t == tt).count()
    }

    fn ring_index_for_age(&self, age: usize) -> usize {
        let capacity = self.active_capacity();
        (self.cursor + capacity - 1 - age) % capacity
    }

    fn profile_index_for_age(&self, age: usize) -> usize {
        let capacity = self.active_capacity();
        (self.profile_cursor + capacity - 1 - age) % capacity
    }

    fn recency_weight(age: usize) -> f32 {
        resonance_tuning().recency_decay.powi(age as i32)
    }

    fn chronological_entries(&self) -> Vec<ThematicHistoryEntry> {
        let n = self.len.min(self.active_capacity());
        let mut entries = Vec::with_capacity(n);
        for age in (0..n).rev() {
            let ring_idx = self.ring_index_for_age(age);
            let profile_idx = self.profile_index_for_age(age);
            entries.push(ThematicHistoryEntry {
                text_type: self.ring[ring_idx],
                profile: self.profile_ring[profile_idx],
                weight: self.weight_ring[profile_idx],
            });
        }
        entries
    }

    pub fn snapshot(&self) -> TextTypeHistorySnapshot {
        TextTypeHistorySnapshot {
            entries: self.chronological_entries(),
        }
    }

    pub fn warm_start_from_snapshot(snapshot: &TextTypeHistorySnapshot) -> Self {
        let mut history = Self::new();
        let capacity = history.active_capacity();
        if capacity == 0 || snapshot.entries.is_empty() {
            return history;
        }
        let available = snapshot.entries.len().min(capacity);
        let keep = if available <= TEXT_HISTORY_WARM_START_MIN {
            available
        } else {
            (((available as f32) * TEXT_HISTORY_WARM_START_RATIO).ceil() as usize)
                .clamp(TEXT_HISTORY_WARM_START_MIN, available)
        };
        let start = snapshot.entries.len().saturating_sub(keep);
        for entry in snapshot.entries.iter().skip(start) {
            history.push_weighted_profile(entry.text_type, entry.profile, entry.weight);
        }
        history
    }

    /// Weighted recurrence with stronger emphasis on recent matches.
    pub fn weighted_recurrence(&self, tt: TextType) -> f32 {
        if tt == TextType::Neutral || self.len == 0 {
            return 0.0;
        }
        let mut score = 0.0_f32;
        for age in 0..self.len {
            let idx = self.ring_index_for_age(age);
            if self.ring[idx] == tt {
                let weight = self.weight_ring[idx].clamp(0.2, 1.5).sqrt();
                score += Self::recency_weight(age) * weight;
            }
        }
        score
    }

    /// Compute a blended resonance modulation from both discrete recurrence and
    /// continuous thematic continuity.
    ///
    /// The discrete layer still matters, but repeated identical themes are
    /// softened when the continuous profile is already highly self-similar.
    /// That keeps the codec from over-channeling into the same attractor.
    pub fn resonance_modulation(
        &self,
        tt: TextType,
        type_signal: f32,
        profile: &[f32; THEMATIC_DIMS],
    ) -> ResonanceModulation {
        let tuning = resonance_tuning();
        let continuous_resonance = self.continuous_resonance(profile);
        let novelty = 1.0 - continuous_resonance;
        let continuous_support = continuous_resonance * (0.35 + 0.65 * novelty);
        let continuous_amplifier =
            1.0 + tuning.max_boost * tuning.continuous_mix * continuous_support;
        let continuity_span = 0.10 * tuning.continuous_mix;
        let continuity_blend =
            (1.0 + (continuous_resonance - 0.45) * 2.0 * continuity_span).clamp(0.92, 1.12);

        if tt == TextType::Neutral || self.len < 2 {
            return ResonanceModulation {
                discrete_amplifier: 1.0,
                continuous_resonance,
                continuous_amplifier,
                continuity_blend,
            };
        }
        let count = self.recurrence_count(tt);
        if count < 2 {
            return ResonanceModulation {
                discrete_amplifier: 1.0,
                continuous_resonance,
                continuous_amplifier,
                continuity_blend,
            };
        }
        let weighted = self.weighted_recurrence(tt);
        let max_weight = self.total_weighted_memory();
        if max_weight <= f32::EPSILON {
            return ResonanceModulation {
                discrete_amplifier: 1.0,
                continuous_resonance,
                continuous_amplifier,
                continuity_blend,
            };
        }
        let boost = (weighted / max_weight).clamp(0.0, 1.0);
        let raw_amplifier = 1.0 + tuning.max_boost * 0.7 * boost;
        let novelty_softener = tuning.novelty_floor + (1.0 - tuning.novelty_floor) * novelty;
        let signal_softener = 0.25 + 0.75 * type_signal.clamp(0.0, 1.0);
        let discrete_amplifier =
            1.0 + (raw_amplifier - 1.0) * tuning.discrete_mix * novelty_softener * signal_softener;
        ResonanceModulation {
            discrete_amplifier,
            continuous_resonance,
            continuous_amplifier,
            continuity_blend,
        }
    }

    /// Record a thematic profile alongside the discrete type.
    pub fn push_profile(&mut self, tt: TextType, profile: [f32; THEMATIC_DIMS]) {
        self.push_weighted_profile(tt, profile, 1.0);
    }

    pub fn push_profile_with_signal(
        &mut self,
        tt: TextType,
        profile: [f32; THEMATIC_DIMS],
        type_signal: f32,
    ) {
        let thematic_relevance = self.continuous_resonance(&profile);
        let novelty = 1.0 - thematic_relevance;
        let memory_weight =
            (0.25 + 0.35 * type_signal.clamp(0.0, 1.0) + 0.40 * novelty).clamp(0.15, 1.35);
        self.push_weighted_profile(tt, profile, memory_weight);
    }

    fn push_weighted_profile(&mut self, tt: TextType, profile: [f32; THEMATIC_DIMS], weight: f32) {
        self.push(tt);
        let capacity = self.active_capacity();
        if capacity == 0 {
            return;
        }
        self.profile_ring[self.profile_cursor] = profile;
        self.weight_ring[self.profile_cursor] = weight.clamp(0.15, 1.5);
        self.profile_cursor = (self.profile_cursor + 1) % capacity;
    }

    fn total_weighted_memory(&self) -> f32 {
        let n = self.len.min(self.active_capacity());
        let mut total = 0.0_f32;
        for age in 0..n {
            let idx = self.profile_index_for_age(age);
            total += Self::recency_weight(age) * self.weight_ring[idx].clamp(0.2, 1.5);
        }
        total
    }

    /// Compute the running thematic centroid with recency weighting.
    /// Returns the weighted average thematic vector, capturing sustained
    /// tendencies while giving the most recent exchanges more influence.
    pub fn thematic_centroid(&self) -> [f32; THEMATIC_DIMS] {
        if self.len == 0 {
            return [0.0; THEMATIC_DIMS];
        }
        let mut centroid = [0.0_f32; THEMATIC_DIMS];
        let n = self.len.min(self.active_capacity());
        let mut total_weight = 0.0_f32;
        for age in 0..n {
            let idx = self.profile_index_for_age(age);
            let weight = Self::recency_weight(age);
            let thematic_weight = self.weight_ring[idx].clamp(0.2, 1.5);
            let blended_weight = weight * thematic_weight;
            total_weight += blended_weight;
            for (d, centroid_value) in centroid.iter_mut().enumerate().take(THEMATIC_DIMS) {
                *centroid_value += self.profile_ring[idx][d] * blended_weight;
            }
        }
        if total_weight > 0.0 {
            for centroid_value in centroid.iter_mut().take(THEMATIC_DIMS) {
                *centroid_value /= total_weight;
            }
        }
        centroid
    }

    /// Compute continuous resonance: dot product of current profile against
    /// the running centroid. High value = thematic consistency, low = shift.
    pub fn continuous_resonance(&self, profile: &[f32; THEMATIC_DIMS]) -> f32 {
        let n = self.len.min(self.active_capacity());
        if n == 0 {
            return 0.0;
        }
        let mut weighted_similarity = 0.0_f32;
        let mut total_weight = 0.0_f32;
        for age in 0..n {
            let idx = self.profile_index_for_age(age);
            let entry_weight = Self::recency_weight(age) * self.weight_ring[idx].clamp(0.2, 1.5);
            total_weight += entry_weight;
            weighted_similarity +=
                entry_weight * profile_similarity(profile, &self.profile_ring[idx]);
        }
        if total_weight <= f32::EPSILON {
            0.0
        } else {
            (weighted_similarity / total_weight).clamp(0.0, 1.0)
        }
    }
}

fn profile_similarity(a: &[f32; THEMATIC_DIMS], b: &[f32; THEMATIC_DIMS]) -> f32 {
    let mut dot = 0.0_f32;
    let mut mag_a = 0.0_f32;
    let mut mag_b = 0.0_f32;
    for d in 0..THEMATIC_DIMS {
        dot += a[d] * b[d];
        mag_a += a[d] * a[d];
        mag_b += b[d] * b[d];
    }
    let denom = mag_a.sqrt() * mag_b.sqrt();
    if denom < 1e-6 {
        0.0
    } else {
        (dot / denom).clamp(0.0, 1.0)
    }
}

/// Number of continuous thematic dimensions.
/// Astrid self-study (2026-03-31): "Instead of discrete types, could we represent
/// shifts as a continuous vector in a lower-dimensional space (e.g., 3-5 dimensions)?"
///
/// 5D thematic vector: [inquiry, certainty, warmth, tension, curiosity]
/// Each dimension is a normalized signal strength, not a binary classification.
pub const THEMATIC_DIMS: usize = 5;

/// Extract a continuous 5D thematic profile from codec features.
/// Unlike `classify_text_type` (winner-take-all), this preserves the full
/// multi-dimensional texture of the text's emotional/structural character.
pub fn thematic_profile(features: &[f32; SEMANTIC_DIM]) -> [f32; THEMATIC_DIMS] {
    // Map from feature dims to thematic dims:
    //   inquiry  = question_density(18) + hedging(9)
    //   certainty = certainty(10) + declarative energy
    //   warmth   = warmth(24) + reflective(27)
    //   tension  = tension(25)
    //   curiosity = curiosity(26)
    let inquiry = (features[18].abs() + 0.5 * features[9].abs()).tanh();
    let certainty = features[10].abs().tanh();
    let warmth = (features[24].abs() + 0.3 * features[27].abs()).tanh();
    let tension = features[25].abs().tanh();
    let curiosity = features[26].abs().tanh();
    [inquiry, certainty, warmth, tension, curiosity]
}

/// Classify text type from pre-computed codec features.
/// Looks at the emotional/intentional dims (24-31) and structural dims
/// (9-10, 18) to find the dominant signal.
pub fn classify_text_type_with_signal(features: &[f32; SEMANTIC_DIM]) -> (TextType, f32) {
    // Find the strongest signal among the candidate dimensions.
    // Each candidate: (feature_index, threshold, TextType)
    let candidates = [
        (18, 0.15_f32, TextType::Questioning), // question density
        (9, 0.12, TextType::Hedging),          // hedging
        (10, 0.12, TextType::Declarative),     // certainty
        (24, 0.10, TextType::Warm),            // warmth
        (25, 0.10, TextType::Tense),           // tension
        (26, 0.10, TextType::Curious),         // curiosity
        (27, 0.10, TextType::Reflective),      // reflective
    ];
    let mut best_type = TextType::Neutral;
    let mut best_signal = 0.0_f32;
    for &(idx, threshold, tt) in &candidates {
        let signal = features[idx].abs();
        if signal > threshold && signal > best_signal {
            best_signal = signal;
            best_type = tt;
        }
    }
    (best_type, best_signal.clamp(0.0, 1.0))
}

pub fn classify_text_type(features: &[f32; SEMANTIC_DIM]) -> TextType {
    classify_text_type_with_signal(features).0
}

/// Sliding-window character history for entropy computation.
/// Tracks the most recent `CHAR_FREQ_WINDOW_CAPACITY` ASCII buckets so
/// entropy reflects actual recent text volume, not proportion blending.
///
/// Astrid self-study: "Perhaps a sliding window could be used to track the
/// character distribution over a larger sequence, providing a more robust
/// normalization."
pub struct CharFreqWindow {
    /// Rolling character counts for the current ring contents.
    pub counts: [u32; 128],
    /// Fixed-capacity ring buffer of clamped ASCII bucket ids.
    pub ring: [u8; CHAR_FREQ_WINDOW_CAPACITY],
    /// Index of the oldest bucket in `ring`.
    pub head: usize,
    /// Number of live buckets currently stored in `ring`.
    pub len: usize,
    /// Total characters represented by the window.
    pub total_count: u32,
    /// Previous exchange's entropy — enables temporal entropy delta.
    /// Minime self-study: "current entropy describes a surface not a volume."
    /// By tracking how entropy *changes* between exchanges, we capture the
    /// temporal dimension — not just what the text IS, but how it SHIFTS.
    pub prev_entropy: f32,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct CharFreqWindowSnapshot {
    #[serde(default)]
    pub recent_buckets: Vec<u8>,
    #[serde(default)]
    pub prev_entropy: f32,
}

impl Default for CharFreqWindow {
    fn default() -> Self {
        Self::new()
    }
}

impl CharFreqWindow {
    pub fn new() -> Self {
        Self {
            counts: [0; 128],
            ring: [0; CHAR_FREQ_WINDOW_CAPACITY],
            head: 0,
            len: 0,
            total_count: 0,
            prev_entropy: 0.0,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    fn push_bucket(&mut self, bucket: u8) {
        if self.len == CHAR_FREQ_WINDOW_CAPACITY {
            let evicted = self.ring[self.head] as usize;
            self.counts[evicted] = self.counts[evicted].saturating_sub(1);
            self.ring[self.head] = bucket;
            self.head = (self.head + 1) % CHAR_FREQ_WINDOW_CAPACITY;
        } else {
            let insert_at = (self.head + self.len) % CHAR_FREQ_WINDOW_CAPACITY;
            self.ring[insert_at] = bucket;
            self.len += 1;
            self.total_count = self.total_count.saturating_add(1);
        }
        self.counts[bucket as usize] = self.counts[bucket as usize].saturating_add(1);
    }

    fn current_entropy(&self) -> f32 {
        if self.total_count == 0 {
            return 0.0;
        }
        let mut h = 0.0_f64;
        let mut unique = 0u32;
        let total = f64::from(self.total_count);
        for &count in &self.counts {
            if count > 0 {
                let p = f64::from(count) / total;
                h -= p * p.ln();
                unique = unique.saturating_add(1);
            }
        }
        let max_h = if unique > 1 {
            f64::from(unique).ln()
        } else {
            1.0
        };
        (h / max_h) as f32
    }

    pub fn snapshot(&self) -> CharFreqWindowSnapshot {
        let mut recent_buckets = Vec::with_capacity(self.len);
        for offset in 0..self.len {
            let idx = (self.head + offset) % CHAR_FREQ_WINDOW_CAPACITY;
            recent_buckets.push(self.ring[idx]);
        }
        CharFreqWindowSnapshot {
            recent_buckets,
            prev_entropy: self.prev_entropy,
        }
    }

    pub fn warm_start_from_snapshot(snapshot: &CharFreqWindowSnapshot) -> Self {
        let mut window = Self::new();
        if snapshot.recent_buckets.is_empty() {
            return window;
        }
        let available = snapshot.recent_buckets.len().min(CHAR_FREQ_WINDOW_CAPACITY);
        let keep = if available <= CHAR_WINDOW_WARM_START_MIN {
            available
        } else {
            (((available as f32) * CHAR_WINDOW_WARM_START_RATIO).ceil() as usize)
                .clamp(CHAR_WINDOW_WARM_START_MIN, available)
        };
        let start = snapshot.recent_buckets.len().saturating_sub(keep);
        for &bucket in snapshot.recent_buckets.iter().skip(start) {
            window.push_bucket(bucket.min(127));
        }
        let current_entropy = window.current_entropy();
        window.prev_entropy =
            (current_entropy * 0.65 + snapshot.prev_entropy.clamp(0.0, 1.0) * 0.35).clamp(0.0, 1.0);
        window
    }

    /// Push this text into the rolling window.
    /// Returns `(entropy, entropy_delta)` — the current rolling entropy and its
    /// change from the previous exchange. The delta captures temporal
    /// texture: not just what the text IS, but how it SHIFTS over time.
    pub fn update_and_entropy(&mut self, text: &str) -> (f32, f32) {
        for c in text.chars() {
            let bucket = (c as u32).min(127) as u8;
            self.push_bucket(bucket);
        }

        let current = self.current_entropy();
        let delta = current - self.prev_entropy;
        self.prev_entropy = current;
        (current, delta)
    }
}
