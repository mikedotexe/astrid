#![allow(clippy::arithmetic_side_effects, clippy::cast_precision_loss)]

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TextTimeDomainProfile {
    pub char_count: usize,
    pub word_count: usize,
    pub sentence_count: usize,
    pub avg_word_len: f32,
    pub punctuation_rate: f32,
    pub question_rate: f32,
    pub exclamation_rate: f32,
    pub uppercase_rate: f32,
    pub digit_rate: f32,
    pub line_break_rate: f32,
    pub rhythm_alternation_rate: f32,
    pub repetition_rate: f32,
    pub sentence_length_cv: f32,
    pub cadence_burstiness: f32,
    pub regularity_score: f32,
    pub temporal_complexity: f32,
    pub cadence_classification: String,
}

impl Default for TextTimeDomainProfile {
    fn default() -> Self {
        Self {
            char_count: 0,
            word_count: 0,
            sentence_count: 0,
            avg_word_len: 0.0,
            punctuation_rate: 0.0,
            question_rate: 0.0,
            exclamation_rate: 0.0,
            uppercase_rate: 0.0,
            digit_rate: 0.0,
            line_break_rate: 0.0,
            rhythm_alternation_rate: 0.0,
            repetition_rate: 0.0,
            sentence_length_cv: 0.0,
            cadence_burstiness: 0.0,
            regularity_score: 1.0,
            temporal_complexity: 0.0,
            cadence_classification: String::from("empty"),
        }
    }
}

#[must_use]
pub fn text_time_domain_profile(text: &str) -> TextTimeDomainProfile {
    let chars = text.chars().collect::<Vec<_>>();
    if chars.is_empty() {
        return TextTimeDomainProfile::default();
    }

    let char_count = chars.len();
    let safe_chars = char_count.max(1) as f32;
    let words = text.split_whitespace().collect::<Vec<_>>();
    let word_count = words.len();
    let word_chars = words
        .iter()
        .map(|word| word.chars().filter(|ch| ch.is_alphanumeric()).count())
        .sum::<usize>();
    let sentence_lengths = sentence_word_lengths(text);
    let sentence_count = sentence_lengths.len().max(1);
    let punctuation_count = chars.iter().filter(|ch| ch.is_ascii_punctuation()).count();
    let question_count = chars.iter().filter(|ch| **ch == '?').count();
    let exclamation_count = chars.iter().filter(|ch| **ch == '!').count();
    let digit_count = chars.iter().filter(|ch| ch.is_ascii_digit()).count();
    let line_break_count = chars.iter().filter(|ch| **ch == '\n').count();
    let letter_count = chars.iter().filter(|ch| ch.is_alphabetic()).count();
    let uppercase_count = chars
        .iter()
        .filter(|ch| ch.is_uppercase() && ch.is_alphabetic())
        .count();
    let repetition_count = chars
        .windows(2)
        .filter(|pair| {
            let left = pair[0];
            let right = pair[1];
            !left.is_whitespace() && left == right
        })
        .count();
    let adjacent_count = char_count.saturating_sub(1).max(1) as f32;
    let sentence_length_cv = coefficient_of_variation(&sentence_lengths);
    let punctuation_cluster_rate = punctuation_cluster_rate(&chars);
    let rhythm_alternation_rate = rhythm_alternation_rate(&chars);
    let repetition_rate = repetition_count as f32 / adjacent_count;
    let punctuation_rate = punctuation_count as f32 / safe_chars;
    let line_break_rate = line_break_count as f32 / safe_chars;
    let cadence_burstiness = (sentence_length_cv.clamp(0.0, 2.0) / 2.0 * 0.48
        + punctuation_cluster_rate * 0.24
        + line_break_rate.mul_add(8.0, 0.0).clamp(0.0, 1.0) * 0.16
        + repetition_rate.clamp(0.0, 1.0) * 0.12)
        .clamp(0.0, 1.0);
    let regularity_score = (1.0
        - (sentence_length_cv.clamp(0.0, 2.0) / 2.0 * 0.55
            + punctuation_cluster_rate * 0.25
            + repetition_rate.clamp(0.0, 1.0) * 0.20))
        .clamp(0.0, 1.0);
    let temporal_complexity = (rhythm_alternation_rate * 0.32
        + cadence_burstiness * 0.30
        + punctuation_rate.mul_add(4.0, 0.0).clamp(0.0, 1.0) * 0.18
        + line_break_rate.mul_add(8.0, 0.0).clamp(0.0, 1.0) * 0.10
        + (1.0 - regularity_score) * 0.10)
        .clamp(0.0, 1.0);
    let cadence_classification = classify_cadence(
        temporal_complexity,
        cadence_burstiness,
        regularity_score,
        rhythm_alternation_rate,
    );

    TextTimeDomainProfile {
        char_count,
        word_count,
        sentence_count,
        avg_word_len: if word_count == 0 {
            0.0
        } else {
            word_chars as f32 / word_count as f32
        },
        punctuation_rate,
        question_rate: question_count as f32 / safe_chars,
        exclamation_rate: exclamation_count as f32 / safe_chars,
        uppercase_rate: if letter_count == 0 {
            0.0
        } else {
            uppercase_count as f32 / letter_count as f32
        },
        digit_rate: digit_count as f32 / safe_chars,
        line_break_rate,
        rhythm_alternation_rate,
        repetition_rate,
        sentence_length_cv,
        cadence_burstiness,
        regularity_score,
        temporal_complexity,
        cadence_classification: cadence_classification.to_string(),
    }
}

fn sentence_word_lengths(text: &str) -> Vec<f32> {
    let mut lengths = Vec::new();
    let mut count = 0_usize;
    for token in text.split_whitespace() {
        if token.chars().any(char::is_alphanumeric) {
            count = count.saturating_add(1);
        }
        if token.ends_with('.') || token.ends_with('?') || token.ends_with('!') {
            if count > 0 {
                lengths.push(count as f32);
                count = 0;
            }
        }
    }
    if count > 0 {
        lengths.push(count as f32);
    }
    lengths
}

fn coefficient_of_variation(values: &[f32]) -> f32 {
    if values.len() <= 1 {
        return 0.0;
    }
    let mean = values.iter().sum::<f32>() / values.len() as f32;
    if mean <= f32::EPSILON {
        return 0.0;
    }
    let variance = values
        .iter()
        .map(|value| {
            let delta = *value - mean;
            delta * delta
        })
        .sum::<f32>()
        / values.len() as f32;
    (variance.sqrt() / mean).clamp(0.0, 4.0)
}

fn punctuation_cluster_rate(chars: &[char]) -> f32 {
    if chars.len() < 2 {
        return 0.0;
    }
    let clusters = chars
        .windows(2)
        .filter(|pair| pair[0].is_ascii_punctuation() && pair[1].is_ascii_punctuation())
        .count();
    clusters as f32 / chars.len().saturating_sub(1).max(1) as f32
}

fn rhythm_alternation_rate(chars: &[char]) -> f32 {
    let classes = chars
        .iter()
        .filter_map(|ch| {
            if !ch.is_alphabetic() {
                return None;
            }
            Some(is_vowel(*ch))
        })
        .collect::<Vec<_>>();
    if classes.len() < 2 {
        return 0.0;
    }
    let transitions = classes.windows(2).filter(|pair| pair[0] != pair[1]).count();
    transitions as f32 / classes.len().saturating_sub(1).max(1) as f32
}

fn is_vowel(ch: char) -> bool {
    matches!(ch.to_ascii_lowercase(), 'a' | 'e' | 'i' | 'o' | 'u' | 'y')
}

fn classify_cadence(
    temporal_complexity: f32,
    cadence_burstiness: f32,
    regularity_score: f32,
    rhythm_alternation_rate: f32,
) -> &'static str {
    if temporal_complexity >= 0.62 && cadence_burstiness >= 0.45 {
        "bursty_complex"
    } else if regularity_score >= 0.72 && rhythm_alternation_rate >= 0.48 {
        "regular_rhythmic"
    } else if cadence_burstiness >= 0.40 {
        "punctuated_bursts"
    } else if temporal_complexity <= 0.24 && regularity_score >= 0.65 {
        "steady_plain"
    } else {
        "mixed_cadence"
    }
}

#[cfg(test)]
mod tests {
    use super::text_time_domain_profile;

    #[test]
    fn time_domain_profile_distinguishes_bursty_from_plain_text() {
        let plain = text_time_domain_profile("this is a simple calm sentence with even pacing");
        let bursty = text_time_domain_profile("Now! Wait... again?!\nA sudden pivot; another one!");

        assert!(bursty.temporal_complexity > plain.temporal_complexity);
        assert!(bursty.cadence_burstiness > plain.cadence_burstiness);
        assert!(plain.regularity_score > bursty.regularity_score);
        assert_ne!(bursty.cadence_classification, "empty");
    }
}
