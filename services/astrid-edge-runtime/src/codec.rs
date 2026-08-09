use sha2::{Digest as _, Sha256};

pub const SEMANTIC_DIM: usize = 48;

/// Deterministic, local feature hashing for symbolic events.
///
/// This is deliberately not presented as an embedding model. It preserves
/// token overlap, rough order, punctuation, and length in a bounded vector so
/// recurrent state can carry a fading trace without another inference call.
#[must_use]
pub fn encode_text(label: &str, text: &str) -> Vec<f32> {
    let mut features = vec![0.0_f32; SEMANTIC_DIM];
    let bounded = text.chars().take(16_384).collect::<String>();
    let mut token_count = 0_u32;

    for (position, token) in bounded.split_whitespace().take(2_048).enumerate() {
        token_count = token_count.saturating_add(1);
        let mut hasher = Sha256::new();
        hasher.update(label.as_bytes());
        hasher.update([0]);
        hasher.update(position.to_le_bytes());
        hasher.update(token.to_lowercase().as_bytes());
        let digest = hasher.finalize();

        for offset in 0..4 {
            let index = usize::from(digest[offset]) % 40;
            let sign = if digest[offset.saturating_add(4)] & 1 == 0 {
                1.0
            } else {
                -1.0
            };
            let position_weight = 1.0 / (1.0 + (bounded_count_f32(position) * 0.002));
            features[index] += sign * position_weight;
        }
    }

    let character_count = bounded_count_f32(bounded.chars().count());
    let punctuation = bounded.chars().filter(char::is_ascii_punctuation).count();
    let newlines = bounded
        .chars()
        .filter(|character| *character == '\n')
        .count();
    let uppercase = bounded
        .chars()
        .filter(|character| character.is_uppercase())
        .count();

    features[40] = (f32::from(u16::try_from(token_count).unwrap_or(u16::MAX)) / 256.0).tanh();
    features[41] = (character_count / 2_048.0).tanh();
    features[42] = (bounded_count_f32(punctuation) / 64.0).tanh();
    features[43] = (bounded_count_f32(newlines) / 16.0).tanh();
    features[44] = (bounded_count_f32(uppercase) / character_count.max(1.0)).clamp(0.0, 1.0);
    features[45] = if label == "user" { 1.0 } else { -1.0 };
    features[46] = if bounded.contains('?') { 1.0 } else { 0.0 };
    features[47] = if bounded.contains("NEXT:") { 1.0 } else { 0.0 };

    let norm = features
        .iter()
        .map(|value| value * value)
        .sum::<f32>()
        .sqrt()
        .max(1.0);
    for value in &mut features {
        *value = (*value / norm * 1.8).clamp(-0.35, 0.35);
    }
    features
}

fn bounded_count_f32(value: usize) -> f32 {
    f32::from(u16::try_from(value).unwrap_or(u16::MAX))
}

#[cfg(test)]
mod tests {
    use super::{SEMANTIC_DIM, encode_text};

    #[test]
    fn encoding_is_bounded_deterministic_and_label_aware() {
        let first = encode_text("user", "The reservoir remembers this question?");
        let second = encode_text("user", "The reservoir remembers this question?");
        let assistant = encode_text("assistant", "The reservoir remembers this question?");

        assert_eq!(first, second);
        assert_eq!(first.len(), SEMANTIC_DIM);
        assert!(first.iter().all(|value| (-0.35..=0.35).contains(value)));
        assert_ne!(first, assistant);
        assert!((first[46] - 0.35).abs() < f32::EPSILON);
    }
}
