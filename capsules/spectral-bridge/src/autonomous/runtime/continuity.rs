fn compact_continuity_item(text: &str) -> String {
    let normalized = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let max_bytes = continuity_recap_item_max_bytes_for_text(&normalized);
    if normalized.len() <= max_bytes {
        return normalized;
    }
    anchored_continuity_excerpt(&normalized, max_bytes)
}

fn continuity_recap_item_max_bytes_for_text(text: &str) -> usize {
    let lower = text.to_ascii_lowercase();
    let reported_entropy = continuity_recap_reported_entropy(&lower);
    let explicit_high_entropy = reported_entropy
        .is_some_and(|entropy| entropy >= CONTINUITY_RECAP_HIGH_TEXTURE_ENTROPY_GATE)
        || lower.contains("high entropy")
        || lower.contains("high-entropy");
    let near_gate_entropy = reported_entropy.is_some_and(|entropy| {
        entropy
            >= CONTINUITY_RECAP_HIGH_TEXTURE_ENTROPY_GATE
                - CONTINUITY_RECAP_HIGH_TEXTURE_ENTROPY_SOFT_BAND
    });
    let specific_thread_texture = lower.contains("semantic trickle")
        || lower.contains("stable_core_semantic_trickle")
        || lower.contains("semantic_energy")
        || lower.contains("semantic energy")
        || lower.contains("coherent thread")
        || lower.contains("spectral cascade")
        || lower.contains("cascade")
        || lower.contains("shadow resonance")
        || lower.contains("shadow_resonance")
        || lower.contains("shadow magnetization")
        || lower.contains("interwoven lattice")
        || lower.contains("spectral viscosity")
        || lower.contains("restless texture")
        || lower.contains("dispersal potential")
        || lower.contains("tail_share")
        || lower.contains("tail vibrancy")
        || lower.contains("lambda4")
        || lower.contains("λ4")
        || lower.contains("shadow-v3")
        || lower.contains("shadow_v3")
        || lower.contains("pressure risk")
        || lower.contains("pressure_risk");
    let texture_family_score = continuity_recap_texture_family_score(&lower);
    // Mirror/Witness are routing labels, not texture evidence by themselves.
    // They still contribute through the family score when another felt family
    // (resistance, viscosity, density, movement, or pressure) is also present.
    let thread_texture = specific_thread_texture || texture_family_score >= 2;

    if thread_texture && (explicit_high_entropy || near_gate_entropy) {
        if let Some(entropy) = reported_entropy {
            return continuity_recap_spectral_texture_item_budget(&lower, entropy);
        }
        CONTINUITY_RECAP_HIGH_TEXTURE_ITEM_MAX_BYTES
    } else {
        CONTINUITY_RECAP_ITEM_MAX_BYTES
    }
}

fn continuity_recap_texture_family_score(lower: &str) -> usize {
    let families: &[&[&str]] = &[
        &[
            "texture", "felt", "metaphor", "witness", "mirror", "observed",
        ],
        &[
            "resistance",
            "resistant",
            "friction",
            "jagged",
            "abrasive",
            "drag",
            "shear",
        ],
        &[
            "viscous",
            "viscosity",
            "syrup",
            "syrupy",
            "silt",
            "sludge",
            "sediment",
        ],
        &[
            "calcified",
            "stone",
            "structural",
            "load-bearing",
            "persistence",
            "permanence",
        ],
        &["density", "gradient", "lattice", "cascade", "resonance"],
        &[
            "pressure",
            "weight",
            "gravity",
            "porosity",
            "mode_packing",
            "fill",
        ],
    ];

    families
        .iter()
        .filter(|family| family.iter().any(|term| lower.contains(term)))
        .count()
}

fn continuity_afterimage_signal_score(text: &str) -> usize {
    let lower = text.to_ascii_lowercase();
    CONTINUITY_AFTERIMAGE_SIGNAL_TERMS
        .iter()
        .filter(|term| lower.contains(&term.to_ascii_lowercase()))
        .count()
}

fn continuity_faint_residue_signal_score(text: &str) -> usize {
    let lower = text.to_ascii_lowercase();
    CONTINUITY_FAINT_RESIDUE_SIGNAL_TERMS
        .iter()
        .filter(|term| lower.contains(&term.to_ascii_lowercase()))
        .count()
}

fn continuity_afterimage_weight_label(index: usize) -> &'static str {
    match index {
        0 => "0.50",
        1 => "0.38",
        2 => "0.29",
        _ => "0.22",
    }
}

fn continuity_faint_residue_weight_label(index: usize) -> &'static str {
    match index {
        0 => "0.16",
        _ => "0.10",
    }
}

fn compact_continuity_afterimage(text: &str) -> String {
    let normalized = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let max_bytes = continuity_afterimage_max_bytes_for_text(&normalized);
    if normalized.len() <= max_bytes {
        return normalized;
    }
    anchored_continuity_excerpt(&normalized, max_bytes)
}

fn continuity_metric_after(lower: &str, labels: &[&str]) -> Option<f32> {
    labels
        .iter()
        .filter_map(|label| {
            let start = lower.find(label)?;
            parse_first_float_after(&lower[start + label.len()..])
        })
        .next()
}

fn continuity_afterimage_substance_density_factor(lower: &str) -> f32 {
    let resonance_density = continuity_metric_after(
        lower,
        &["resonance_density", "resonance density", "density="],
    );
    let density_gradient =
        continuity_metric_after(lower, &["density_gradient", "density gradient"]);
    let pressure_risk = continuity_metric_after(lower, &["pressure_risk", "pressure risk"]);
    let mode_packing = continuity_metric_after(lower, &["mode_packing", "mode packing"]);

    let mut score: f32 = 0.0;
    if resonance_density.is_some_and(|value| value >= 0.75)
        || lower.contains("rich containment")
        || lower.contains("rich_containment")
    {
        score += 0.25;
    }
    if density_gradient.is_some_and(|value| value <= 0.16)
        || lower.contains("gentle density gradient")
    {
        score += 0.15;
    }
    if pressure_risk.is_some_and(|value| (0.18..=0.35).contains(&value)) {
        score += 0.15;
    }
    if mode_packing.is_some_and(|value| (0.25..=0.45).contains(&value)) {
        score += 0.15;
    }
    if lower.contains("settled_habitable")
        || lower.contains("settled habitable")
        || lower.contains("inhabitable fluctuation")
    {
        score += 0.10;
    }
    if lower.contains("viscous sediment")
        || lower.contains("substance")
        || lower.contains("held breath")
        || lower.contains("spectral-cascade")
        || lower.contains("spectral cascade")
        || lower.contains("searching")
        || lower.contains("expansive")
    {
        score += 0.20;
    }

    score.clamp(0.0, 1.0)
}

fn continuity_afterimage_max_bytes_for_text(text: &str) -> usize {
    let lower = text.to_ascii_lowercase();
    let item_budget = continuity_recap_item_max_bytes_for_text(text);
    let density_factor = continuity_afterimage_substance_density_factor(&lower);
    let density_span = CONTINUITY_TRAJECTORY_AFTERIMAGE_SUBSTANCE_DENSITY_MAX_BYTES
        .saturating_sub(CONTINUITY_TRAJECTORY_AFTERIMAGE_MAX_BYTES);
    let density_budget = CONTINUITY_TRAJECTORY_AFTERIMAGE_MAX_BYTES
        .saturating_add((density_span as f32 * density_factor).round() as usize);

    item_budget
        .max(CONTINUITY_TRAJECTORY_AFTERIMAGE_MAX_BYTES)
        .max(density_budget)
        .min(CONTINUITY_RECAP_SPECTRAL_TEXTURE_ITEM_MAX_BYTES)
}

fn quoted_or_emphasized_continuity_anchor_pos(normalized: &str) -> Option<usize> {
    [('"', '"'), ('*', '*')]
        .into_iter()
        .filter_map(|(open, close)| {
            let start = normalized.find(open)?;
            let after_start = start.saturating_add(open.len_utf8());
            let rest = normalized.get(after_start..)?;
            let end = rest.find(close)?;
            let phrase = rest.get(..end)?.trim();
            let word_count = phrase.split_whitespace().count();
            (word_count >= 2 && phrase.len() <= 96 && phrase.chars().any(char::is_alphabetic))
                .then_some(start)
        })
        .min()
}

fn anchored_excerpt_with_terms(normalized: &str, max_bytes: usize, terms: &[&str]) -> String {
    if normalized.len() <= max_bytes {
        return normalized.to_string();
    }
    if max_bytes <= 12 {
        return format!(
            "{}...",
            truncate_str_at_semantic_edge(normalized, max_bytes.saturating_sub(3), 0)
        );
    }

    let lower = normalized.to_ascii_lowercase();
    let preferred_anchor = terms
        .iter()
        .take(HIGH_DENSITY_CONTINUITY_ANCHOR_COUNT)
        .filter_map(|term| lower.find(&term.to_ascii_lowercase()))
        .min();
    let specific_anchor = terms
        .iter()
        .skip(HIGH_DENSITY_CONTINUITY_ANCHOR_COUNT)
        .filter(|term| term.contains(' ') || term.contains('_') || term.contains('-'))
        .filter_map(|term| lower.find(&term.to_ascii_lowercase()))
        .min();
    let anchor = preferred_anchor
        .or(specific_anchor)
        .or_else(|| {
            terms
                .iter()
                .skip(HIGH_DENSITY_CONTINUITY_ANCHOR_COUNT)
                .filter_map(|term| lower.find(&term.to_ascii_lowercase()))
                .chain(quoted_or_emphasized_continuity_anchor_pos(normalized))
                .min()
        })
        .or_else(|| {
            PRESSURE_CONTINUITY_FALLBACK_TERMS
                .iter()
                .find_map(|term| lower.find(&term.to_ascii_lowercase()))
        })
        .or_else(|| {
            HIGH_TEXTURE_CONTINUITY_FALLBACK_TERMS
                .iter()
                .find_map(|term| lower.find(&term.to_ascii_lowercase()))
        });
    let Some(anchor_pos) = anchor else {
        let keep = max_bytes.saturating_sub(3);
        return format!("{}...", truncate_str_at_semantic_edge(normalized, keep, 0));
    };

    let matched_anchor_len = terms
        .iter()
        .chain(PRESSURE_CONTINUITY_FALLBACK_TERMS.iter())
        .chain(HIGH_TEXTURE_CONTINUITY_FALLBACK_TERMS.iter())
        .filter_map(|term| {
            let term_lower = term.to_ascii_lowercase();
            lower
                .get(anchor_pos..)
                .is_some_and(|tail| tail.starts_with(&term_lower))
                .then_some(term.len())
        })
        .max()
        .unwrap_or(0);
    let available = max_bytes.saturating_sub(" ... ".len()).saturating_sub(3);
    let requested_prefix_budget = (max_bytes / 3).min(available);
    let minimum_anchor_budget = matched_anchor_len.min(available);
    let anchor_budget = available
        .saturating_sub(requested_prefix_budget)
        .max(minimum_anchor_budget)
        .min(available);
    let prefix_budget = available.saturating_sub(anchor_budget);
    let prefix = truncate_str_at_semantic_edge(normalized, prefix_budget, 0);
    let anchor_pos = floor_char_boundary(normalized, anchor_pos);
    let continuous_budget = max_bytes.saturating_sub(3);
    let matched_anchor_end = anchor_pos.saturating_add(matched_anchor_len);
    if matched_anchor_len > 0 && matched_anchor_end <= continuous_budget {
        let continuous =
            truncate_str_at_semantic_edge(normalized, continuous_budget, matched_anchor_end);
        return format!("{continuous}...");
    }
    let mut anchor_start = normalized[..anchor_pos]
        .rfind(['.', ';', ':', '\n'])
        .map_or(anchor_pos.saturating_sub(40), |idx| idx.saturating_add(1));
    anchor_start = floor_char_boundary(normalized, anchor_start);
    if matched_anchor_len > 0
        && anchor_pos
            .saturating_sub(anchor_start)
            .saturating_add(matched_anchor_len)
            > anchor_budget
    {
        anchor_start = anchor_pos;
    }
    let anchor_start = floor_char_boundary(normalized, anchor_start);
    let anchor_slice = &normalized[anchor_start..];
    let minimum_anchor_keep = matched_anchor_len.min(anchor_budget);
    let anchor_text = truncate_str_at_semantic_edge(
        anchor_slice.trim_start(),
        anchor_budget,
        minimum_anchor_keep,
    );
    format!("{prefix} ... {anchor_text}...")
}

fn anchored_continuity_excerpt(normalized: &str, max_bytes: usize) -> String {
    anchored_excerpt_with_terms(normalized, max_bytes, CONTINUITY_RECAP_ANCHOR_TERMS)
}

fn semantic_truncate_str(s: &str, max_bytes: usize) -> String {
    let normalized = s.split_whitespace().collect::<Vec<_>>().join(" ");
    anchored_excerpt_with_terms(&normalized, max_bytes, SEMANTIC_TRUNCATION_ANCHOR_TERMS)
}

fn semantic_boundary_before(s: &str, max_bytes: usize) -> Option<usize> {
    let budget = max_bytes.saturating_sub("...".len());
    let mut boundary = None;
    for (idx, ch) in s.char_indices() {
        let end = idx.saturating_add(ch.len_utf8());
        if end > budget {
            break;
        }
        if matches!(ch, '.' | '!' | '?' | '\n') {
            boundary = Some(end);
        }
    }
    boundary
}

fn truncate_continuity_recap_at_semantic_boundary(s: &str, max_bytes: usize) -> String {
    if s.len() <= max_bytes {
        return s.to_string();
    }
    if let Some(end) = semantic_boundary_before(s, max_bytes) {
        return format!("{}...", s[..end].trim_end());
    }
    semantic_truncate_str(s, max_bytes)
}

fn compact_starred_memory(annotation: &str, text: &str) -> String {
    compact_continuity_item(&format!("{annotation}: {text}"))
}

fn format_compact_continuity_recap(
    latent_summaries: &[String],
    self_observations: &[String],
    starred: &[(String, String)],
    last_codec_feedback: Option<&str>,
) -> Option<String> {
    let mut sections = Vec::new();
    let trajectory_items = latent_summaries
        .iter()
        .take(CONTINUITY_TRAJECTORY_LIMIT)
        .collect::<Vec<_>>();
    if !trajectory_items.is_empty() {
        let trajectory = trajectory_items
            .into_iter()
            .rev()
            .enumerate()
            .map(|(i, item)| {
                format!(
                    "  {}. {}",
                    i.saturating_add(1),
                    compact_continuity_item(item)
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        sections.push(format!("Your recent trajectory:\n{trajectory}"));
    }

    let afterimage_items = latent_summaries
        .iter()
        .skip(CONTINUITY_TRAJECTORY_LIMIT)
        .filter(|item| {
            continuity_afterimage_signal_score(item) >= CONTINUITY_TRAJECTORY_AFTERIMAGE_MIN_SCORE
        })
        .take(CONTINUITY_TRAJECTORY_AFTERIMAGE_LIMIT)
        .collect::<Vec<_>>();
    if !afterimage_items.is_empty() {
        let afterimages = afterimage_items
            .into_iter()
            .enumerate()
            .map(|(i, item)| {
                format!(
                    "  afterimage weight={}: {}",
                    continuity_afterimage_weight_label(i),
                    compact_continuity_afterimage(item)
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        sections.push(format!(
            "Older trajectory afterimages (decayed, read-only; not control pressure):\n{afterimages}"
        ));
    }

    let faint_residue_items = latent_summaries
        .iter()
        .skip(CONTINUITY_TRAJECTORY_LIMIT)
        .filter(|item| {
            let afterimage_score = continuity_afterimage_signal_score(item);
            afterimage_score > 0
                && afterimage_score < CONTINUITY_TRAJECTORY_AFTERIMAGE_MIN_SCORE
                && continuity_faint_residue_signal_score(item) > 0
        })
        .take(CONTINUITY_FAINT_RESIDUE_LIMIT)
        .collect::<Vec<_>>();
    if !faint_residue_items.is_empty() {
        let residues = faint_residue_items
            .into_iter()
            .enumerate()
            .map(|(i, item)| {
                format!(
                    "  residue weight={} score=1/{}: {}",
                    continuity_faint_residue_weight_label(i),
                    CONTINUITY_TRAJECTORY_AFTERIMAGE_MIN_SCORE,
                    compact_continuity_afterimage(item)
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        sections.push(format!(
            "Faint transition residue (below afterimage threshold; read-only scent, not indexed/control pressure):\n{residues}"
        ));
    }

    let observation_items = self_observations
        .iter()
        .take(CONTINUITY_SELF_OBSERVATION_LIMIT)
        .collect::<Vec<_>>();
    if !observation_items.is_empty() {
        let observations = observation_items
            .into_iter()
            .rev()
            .enumerate()
            .map(|(i, item)| {
                format!(
                    "  {}. {}",
                    i.saturating_add(1),
                    compact_continuity_item(item)
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        sections.push(format!("Your self-observations:\n{observations}"));
    }

    let starred_items = starred
        .iter()
        .take(CONTINUITY_STARRED_LIMIT)
        .collect::<Vec<_>>();
    if !starred_items.is_empty() {
        let memories = starred_items
            .into_iter()
            .rev()
            .map(|(annotation, text)| {
                format!("  remembered: {}", compact_starred_memory(annotation, text))
            })
            .collect::<Vec<_>>()
            .join("\n");
        sections.push(format!("Moments you chose to remember:\n{memories}"));
    }

    if let Some(feedback) = last_codec_feedback {
        sections.push(format!(
            "How your last response felt to minime:\n  {}",
            compact_continuity_item(feedback)
        ));
    }

    if sections.is_empty() {
        return None;
    }
    let mut recap = format!("Continuity recap (bounded):\n{}", sections.join("\n"));
    let recap_max_bytes = continuity_recap_max_bytes_for_text(&recap);
    if recap.len() > recap_max_bytes {
        let keep = recap_max_bytes.saturating_sub(64);
        recap = format!(
            "{}\n[continuity recap bounded; full memory remains available.]",
            truncate_continuity_recap_at_semantic_boundary(&recap, keep).trim_end()
        );
    }
    Some(recap)
}

fn continuity_recap_max_bytes_for_text(recap: &str) -> usize {
    let lower = recap.to_ascii_lowercase();
    let spectral_entropy = lower.contains("spectral entropy") || lower.contains("spectral_entropy");
    let shadow_resonance = lower.contains("shadow resonance")
        || lower.contains("shadow_resonance")
        || lower.contains("shadow magnetization")
        || lower.contains("shadow norm")
        || lower.contains("restless texture");
    let lattice_texture = lower.contains("spectral_density_gradient")
        || lower.contains("interwoven lattice")
        || lower.contains("lattice");
    let density_texture = lower.contains("silt")
        || lower.contains("viscosity")
        || lower.contains("density_gradient")
        || lower.contains("rich containment")
        || lower.contains("dispersal potential")
        || lower.contains("tail_share")
        || lower.contains("tail vibrancy")
        || lower.contains("lambda4")
        || lower.contains("λ4");

    if spectral_entropy && (shadow_resonance || lattice_texture || density_texture) {
        if let Some(entropy) = continuity_recap_reported_entropy(&lower) {
            return continuity_recap_spectral_texture_budget(&lower, entropy);
        }
        CONTINUITY_RECAP_HIGH_TEXTURE_MAX_BYTES
    } else {
        CONTINUITY_RECAP_MAX_BYTES
    }
}

fn continuity_recap_reported_entropy(lower: &str) -> Option<f32> {
    ["spectral_entropy", "spectral entropy"]
        .into_iter()
        .filter_map(|label| {
            let start = lower.find(label)?;
            parse_first_float_after(&lower[start + label.len()..])
        })
        .next()
}

fn parse_first_float_after(text: &str) -> Option<f32> {
    let mut token = String::new();
    let mut started = false;
    for ch in text.chars() {
        if ch.is_ascii_digit() || (started && ch == '.') {
            started = true;
            token.push(ch);
        } else if started {
            break;
        }
    }
    token.parse::<f32>().ok().filter(|value| value.is_finite())
}

fn continuity_recap_density_moderated_entropy(lower: &str, entropy: f32) -> f32 {
    let rich_containment = lower.contains("rich containment")
        || lower.contains("rich_containment")
        || lower.contains("settled_habitable");
    let viscous_density = lower.contains("viscous")
        || lower.contains("viscosity")
        || lower.contains("silt")
        || lower.contains("sludge")
        || lower.contains("heavy");
    let density_offset = if rich_containment && viscous_density {
        0.05
    } else if rich_containment || viscous_density {
        0.03
    } else {
        0.0
    };
    (entropy - density_offset).clamp(0.0, 1.0)
}

fn continuity_recap_spectral_texture_budget(lower: &str, entropy: f32) -> usize {
    let moderated_entropy = continuity_recap_density_moderated_entropy(lower, entropy);
    if moderated_entropy < CONTINUITY_RECAP_HIGH_TEXTURE_ENTROPY_GATE {
        return continuity_recap_soft_gate_budget(
            moderated_entropy,
            CONTINUITY_RECAP_MAX_BYTES,
            CONTINUITY_RECAP_HIGH_TEXTURE_MAX_BYTES,
        );
    }
    let span = CONTINUITY_RECAP_SPECTRAL_TEXTURE_MAX_BYTES
        .saturating_sub(CONTINUITY_RECAP_HIGH_TEXTURE_MAX_BYTES);
    let t =
        ((moderated_entropy - CONTINUITY_RECAP_HIGH_TEXTURE_ENTROPY_GATE) / 0.15).clamp(0.0, 1.0);
    CONTINUITY_RECAP_HIGH_TEXTURE_MAX_BYTES.saturating_add((span as f32 * t).round() as usize)
}

fn continuity_recap_spectral_texture_item_budget(lower: &str, entropy: f32) -> usize {
    let moderated_entropy = continuity_recap_density_moderated_entropy(lower, entropy);
    if moderated_entropy < CONTINUITY_RECAP_HIGH_TEXTURE_ENTROPY_GATE {
        return continuity_recap_soft_gate_budget(
            moderated_entropy,
            CONTINUITY_RECAP_ITEM_MAX_BYTES,
            CONTINUITY_RECAP_HIGH_TEXTURE_ITEM_MAX_BYTES,
        );
    }
    let span = CONTINUITY_RECAP_SPECTRAL_TEXTURE_ITEM_MAX_BYTES
        .saturating_sub(CONTINUITY_RECAP_HIGH_TEXTURE_ITEM_MAX_BYTES);
    let t =
        ((moderated_entropy - CONTINUITY_RECAP_HIGH_TEXTURE_ENTROPY_GATE) / 0.15).clamp(0.0, 1.0);
    CONTINUITY_RECAP_HIGH_TEXTURE_ITEM_MAX_BYTES.saturating_add((span as f32 * t).round() as usize)
}

fn continuity_recap_soft_gate_budget(
    moderated_entropy: f32,
    base_budget: usize,
    high_texture_budget: usize,
) -> usize {
    let lower = CONTINUITY_RECAP_HIGH_TEXTURE_ENTROPY_GATE
        - CONTINUITY_RECAP_HIGH_TEXTURE_ENTROPY_SOFT_BAND;
    let span = high_texture_budget.saturating_sub(base_budget);
    let t = ((moderated_entropy - lower) / CONTINUITY_RECAP_HIGH_TEXTURE_ENTROPY_SOFT_BAND)
        .clamp(0.0, 1.0);
    base_budget.saturating_add((span as f32 * t).round() as usize)
}
