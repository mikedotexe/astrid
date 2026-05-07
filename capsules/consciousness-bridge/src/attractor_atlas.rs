use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};

use anyhow::{Context, Result};
use serde_json::Value;

use crate::{
    db::{AttractorLedgerRow, BridgeDb, unix_now},
    paths::bridge_paths,
    types::{
        AttractorAtlasEntryV1, AttractorAtlasV1, AttractorClassification, AttractorCommandKind,
        AttractorIntentV1, AttractorNamingLessonV1, AttractorObservationV1,
        AttractorSeedSnapshotV1, AttractorSubstrate, AttractorSuggestionStatus,
        AttractorSuggestionV1,
    },
};

const ATLAS_POLICY: &str = "attractor_atlas_v1";
const ENTRY_POLICY: &str = "attractor_atlas_entry_v1";

#[must_use]
pub fn build_attractor_atlas_from_rows(
    rows: &[AttractorLedgerRow],
    generated_at_unix_s: f64,
) -> AttractorAtlasV1 {
    let mut entries = BTreeMap::<String, AttractorAtlasEntryV1>::new();
    for row in rows {
        if row.record_type == "intent" {
            if let Ok(intent) = serde_json::from_str::<AttractorIntentV1>(&row.payload) {
                merge_intent(&mut entries, &intent);
            }
        } else if row.record_type == "observation" {
            if let Ok(observation) = serde_json::from_str::<AttractorObservationV1>(&row.payload) {
                merge_observation(&mut entries, &observation);
            }
        }
    }
    let mut entries = entries.into_values().collect::<Vec<_>>();
    for entry in &mut entries {
        refresh_suggested_next(entry);
    }
    AttractorAtlasV1 {
        policy: ATLAS_POLICY.to_string(),
        schema_version: 1,
        generated_at_unix_s,
        entries,
    }
}

pub fn write_derived_attractor_atlas(db: &BridgeDb) -> Result<AttractorAtlasV1> {
    let rows = db
        .query_attractor_ledger(None, 500)
        .context("query attractor ledger")?;
    let mut atlas = build_attractor_atlas_from_rows(&rows, unix_now());
    merge_minime_status(&mut atlas);
    merge_reservoir_reports(&mut atlas);
    merge_suggestion_naming_lessons(&mut atlas);
    ensure_proto_entries(&mut atlas);
    atlas.entries.sort_by(|left, right| {
        left.substrate
            .as_str()
            .cmp(right.substrate.as_str())
            .then_with(|| left.label.cmp(&right.label))
    });
    for entry in &mut atlas.entries {
        refresh_suggested_next(entry);
    }
    let paths = bridge_paths();
    write_atlas_dir(&atlas, &paths.bridge_workspace().join("attractor_atlas"))?;
    write_atlas_dir(&atlas, &paths.minime_workspace().join("attractor_atlas"))?;
    Ok(atlas)
}

#[must_use]
pub fn find_entry<'a>(
    atlas: &'a AttractorAtlasV1,
    label: &str,
) -> Option<&'a AttractorAtlasEntryV1> {
    let query = slug(label);
    atlas
        .entries
        .iter()
        .find(|entry| slug(&entry.label) == query)
        .or_else(|| {
            atlas.entries.iter().find(|entry| {
                slug(&entry.label).contains(&query) || query.contains(&slug(&entry.label))
            })
        })
}

#[must_use]
pub fn render_memory_card(entry: &AttractorAtlasEntryV1) -> String {
    let motifs = if entry.motifs.is_empty() {
        "none captured".to_string()
    } else {
        entry.motifs.join(", ")
    };
    let recurrence = entry
        .latest_recurrence_score
        .map_or("n/a".to_string(), |score| format!("{score:.2}"));
    let best_recurrence = entry
        .best_recurrence_score
        .map_or("n/a".to_string(), |score| format!("{score:.2}"));
    let authorship = entry
        .latest_authorship_score
        .map_or("n/a".to_string(), |score| format!("{score:.2}"));
    let safety = entry
        .latest_safety_level
        .map_or("unknown".to_string(), |level| {
            format!("{level:?}").to_lowercase()
        });
    let class = entry
        .latest_classification
        .map_or("unknown".to_string(), |class| class.as_str().to_string());
    let parents = if entry.parent_seed_ids.is_empty() {
        "none".to_string()
    } else {
        entry.parent_seed_ids.join(", ")
    };
    let facet = entry.facet_path.as_deref().unwrap_or("none").to_string();
    let release_effect = entry
        .release_effect_summary
        .as_deref()
        .unwrap_or("unknown")
        .to_string();
    let garden_proof = entry
        .garden_proof
        .as_ref()
        .map_or("none".to_string(), |proof| {
            proof
                .get("summary")
                .and_then(Value::as_str)
                .unwrap_or("present")
                .to_string()
        });
    let next = if entry.suggested_next.is_empty() {
        "none".to_string()
    } else {
        entry.suggested_next.join(" | ")
    };
    let naming_lessons = if entry.naming_lessons.is_empty() {
        "none".to_string()
    } else {
        entry
            .naming_lessons
            .iter()
            .map(|lesson| {
                format!(
                    "{}: {} -> {} ({})",
                    lesson.author,
                    lesson.raw_label,
                    lesson.resolved_label,
                    lesson.status.as_str()
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    format!(
        "# Attractor Card: {label}\n\n\
Author: {author}\n\
Substrate: {substrate}\n\
Entry: `{entry_id}`\n\
Seed: `{seed}`\n\
Origin: {origin}\n\
Parents: {parents}\n\
Facet: {facet}\n\
Released: {released}\n\
Release effect: {release_effect}\n\
Control eligible: {control}\n\n\
Garden proof: {garden_proof}\n\n\
## Motifs\n{motifs}\n\n\
## Recurrence\nLatest: {recurrence}\nBest: {best_recurrence}\nAuthorship: {authorship}\nClassification: {class}\nSafety: {safety}\n\n\
## Naming Lessons\n{naming_lessons}\n\n\
## Suggested Next\n{next}\n",
        label = entry.label,
        author = entry.author.as_deref().unwrap_or("unknown"),
        substrate = entry.substrate.as_str(),
        entry_id = entry.entry_id,
        seed = entry.seed_intent_id.as_deref().unwrap_or("none"),
        origin = entry.origin_kind.as_deref().unwrap_or("unknown"),
        parents = parents,
        facet = facet,
        released = entry.released,
        release_effect = release_effect,
        garden_proof = garden_proof,
        control = entry
            .control_eligible
            .map_or("unknown".to_string(), |value| value.to_string()),
        motifs = motifs,
        recurrence = recurrence,
        best_recurrence = best_recurrence,
        authorship = authorship,
        class = class,
        safety = safety,
        naming_lessons = naming_lessons,
        next = next,
    )
}

fn merge_intent(entries: &mut BTreeMap<String, AttractorAtlasEntryV1>, intent: &AttractorIntentV1) {
    let key = entry_key(intent.substrate, &intent.label);
    let entry = entries
        .entry(key)
        .or_insert_with(|| empty_entry(intent.substrate, &intent.label));
    entry.author = Some(intent.author.clone());
    bump(&mut entry.lifecycle_counts, intent.command.as_str());
    if let Some(intent_id) = nonempty(&intent.intent_id) {
        push_unique(&mut entry.source_intent_ids, intent_id.to_string());
    }
    if matches!(
        intent.command,
        AttractorCommandKind::Create
            | AttractorCommandKind::Promote
            | AttractorCommandKind::Claim
            | AttractorCommandKind::Blend
    ) {
        entry.seed_intent_id = Some(intent.intent_id.clone());
        entry.spectral_summary = intent.seed_snapshot.clone();
    }
    if matches!(intent.command, AttractorCommandKind::RefreshSnapshot) {
        entry.spectral_summary = intent.seed_snapshot.clone();
    }
    if matches!(intent.command, AttractorCommandKind::Release) {
        entry.released = true;
    }
    entry.control_eligible = Some(intent.safety_bounds.allow_live_control);
    extend_unique(&mut entry.parent_seed_ids, &intent.parent_seed_ids);
    entry.parent_label = intent.parent_label.clone();
    entry.facet_label = intent.facet_label.clone();
    entry.facet_path = intent.facet_path.clone();
    entry.facet_kind = intent.facet_kind.clone();
    if let Some(origin) = intent.origin.as_ref() {
        entry.origin_kind = Some(origin.kind.clone());
        extend_unique(&mut entry.motifs, &origin.motifs);
    }
    if let Some(snapshot) = intent.seed_snapshot.as_ref() {
        extend_unique(&mut entry.motifs, &snapshot.lexical_motifs);
    }
}

fn merge_observation(
    entries: &mut BTreeMap<String, AttractorAtlasEntryV1>,
    observation: &AttractorObservationV1,
) {
    let key = entry_key(observation.substrate, &observation.label);
    let entry = entries
        .entry(key)
        .or_insert_with(|| empty_entry(observation.substrate, &observation.label));
    bump(&mut entry.lifecycle_counts, "observation");
    if let Some(intent_id) = observation.intent_id.as_deref().and_then(nonempty) {
        push_unique(&mut entry.source_intent_ids, intent_id.to_string());
    }
    entry.latest_recurrence_score = Some(observation.recurrence_score);
    entry.best_recurrence_score = Some(
        entry
            .best_recurrence_score
            .unwrap_or(0.0)
            .max(observation.recurrence_score),
    );
    entry.latest_authorship_score = Some(observation.authorship_score);
    entry.best_authorship_score = Some(
        entry
            .best_authorship_score
            .unwrap_or(0.0)
            .max(observation.authorship_score),
    );
    entry.latest_classification = Some(observation.classification);
    entry.latest_safety_level = Some(observation.safety_level);
    entry.parent_label = observation.parent_label.clone();
    entry.facet_label = observation.facet_label.clone();
    entry.facet_path = observation.facet_path.clone();
    entry.facet_kind = observation.facet_kind.clone();
    entry.release_effect_summary = observation.release_effect.clone();
    entry.garden_proof = observation.garden_proof.clone();
}

fn merge_minime_status(atlas: &mut AttractorAtlasV1) {
    let path = bridge_paths()
        .minime_workspace()
        .join("runtime/attractor_intents_status.json");
    let Ok(text) = fs::read_to_string(path) else {
        return;
    };
    let Ok(value) = serde_json::from_str::<Value>(&text) else {
        return;
    };
    if let Some(seeds) = value.get("seeds").and_then(Value::as_object) {
        for (seed_id, seed) in seeds {
            let label = seed
                .get("label")
                .and_then(Value::as_str)
                .unwrap_or(seed_id.as_str());
            let key = entry_key(AttractorSubstrate::MinimeEsn, label);
            let mut entry = empty_entry(AttractorSubstrate::MinimeEsn, label);
            entry.author = seed
                .get("author")
                .and_then(Value::as_str)
                .map(str::to_string);
            entry.seed_intent_id = Some(seed_id.clone());
            push_unique(&mut entry.source_intent_ids, seed_id.clone());
            entry.origin_kind = seed
                .get("origin")
                .and_then(|origin| origin.get("kind"))
                .and_then(Value::as_str)
                .map(str::to_string);
            entry.control_eligible = seed.get("control_eligible").and_then(Value::as_bool);
            entry.released = seed.get("released_at_unix_s").is_some();
            apply_json_facet_metadata(&mut entry, seed);
            if let Some(motifs) = seed
                .get("origin")
                .and_then(|origin| origin.get("motifs"))
                .and_then(Value::as_array)
            {
                for motif in motifs.iter().filter_map(Value::as_str) {
                    push_unique(&mut entry.motifs, motif.to_string());
                }
            }
            if let Some(parents) = seed.get("parent_seed_ids").and_then(Value::as_array) {
                for parent in parents.iter().filter_map(Value::as_str) {
                    push_unique(&mut entry.parent_seed_ids, parent.to_string());
                }
            }
            if let Some(state) = seed.get("spectral_state").and_then(Value::as_object) {
                entry.spectral_summary = Some(snapshot_from_minime_state(state, label));
            }
            atlas.entries.push(entry);
            let _ = key;
        }
    }
    if let Some(observations) = value.get("observations").and_then(Value::as_array) {
        let mut map = atlas
            .entries
            .drain(..)
            .map(|entry| (entry_key(entry.substrate, &entry.label), entry))
            .collect::<BTreeMap<_, _>>();
        for observation in observations {
            let label = observation
                .get("label")
                .and_then(Value::as_str)
                .unwrap_or("minime observation");
            let entry = map
                .entry(entry_key(AttractorSubstrate::MinimeEsn, label))
                .or_insert_with(|| empty_entry(AttractorSubstrate::MinimeEsn, label));
            if let Some(intent_id) = observation.get("intent_id").and_then(Value::as_str) {
                push_unique(&mut entry.source_intent_ids, intent_id.to_string());
            }
            entry.latest_recurrence_score = observation
                .get("recurrence_score")
                .and_then(Value::as_f64)
                .map(|score| score as f32);
            entry.best_recurrence_score = Some(
                entry
                    .best_recurrence_score
                    .unwrap_or(0.0)
                    .max(entry.latest_recurrence_score.unwrap_or(0.0)),
            );
            entry.latest_authorship_score = observation
                .get("authorship_score")
                .and_then(Value::as_f64)
                .map(|score| score as f32);
            entry.best_authorship_score = Some(
                entry
                    .best_authorship_score
                    .unwrap_or(0.0)
                    .max(entry.latest_authorship_score.unwrap_or(0.0)),
            );
            entry.latest_classification = observation
                .get("classification")
                .and_then(Value::as_str)
                .and_then(parse_classification);
            if let Some(effect) = observation.get("release_effect").and_then(Value::as_str) {
                entry.release_effect_summary = Some(effect.to_string());
            }
            if let Some(proof) = observation.get("garden_proof") {
                entry.garden_proof = Some(proof.clone());
            }
            apply_json_facet_metadata(entry, observation);
        }
        atlas.entries = map.into_values().collect();
    }
}

fn merge_reservoir_reports(atlas: &mut AttractorAtlasV1) {
    let Some(parent) = bridge_paths().astrid_root().parent() else {
        return;
    };
    let dir = parent.join("neural-triple-reservoir/results");
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for path in entries
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
        .take(12)
    {
        let Ok(text) = fs::read_to_string(&path) else {
            continue;
        };
        let Ok(value) = serde_json::from_str::<Value>(&text) else {
            continue;
        };
        let Some(rows) = value.get("rows").and_then(Value::as_array) else {
            continue;
        };
        for row in rows {
            let Some(intent) = row.get("intent") else {
                continue;
            };
            let label = intent
                .get("label")
                .and_then(Value::as_str)
                .unwrap_or("reservoir seed");
            let mut entry = empty_entry(AttractorSubstrate::TripleReservoir, label);
            entry.author = intent
                .get("author")
                .and_then(Value::as_str)
                .map(str::to_string);
            entry.seed_intent_id = intent
                .get("intent_id")
                .and_then(Value::as_str)
                .map(str::to_string);
            if let Some(seed_id) = entry.seed_intent_id.clone() {
                push_unique(&mut entry.source_intent_ids, seed_id);
            }
            if let Some(observation) = row.get("observation") {
                entry.latest_recurrence_score = observation
                    .get("recurrence_score")
                    .and_then(Value::as_f64)
                    .map(|score| score as f32);
                entry.latest_authorship_score = observation
                    .get("authorship_score")
                    .and_then(Value::as_f64)
                    .map(|score| score as f32);
                entry.latest_classification = observation
                    .get("classification")
                    .and_then(Value::as_str)
                    .and_then(parse_classification);
            }
            atlas.entries.push(entry);
        }
    }
}

fn write_atlas_dir(atlas: &AttractorAtlasV1, dir: &Path) -> Result<()> {
    let cards_dir = dir.join("cards");
    fs::create_dir_all(&cards_dir).with_context(|| format!("create {}", cards_dir.display()))?;
    fs::write(
        dir.join("attractor_atlas.json"),
        serde_json::to_string_pretty(atlas)?,
    )
    .with_context(|| format!("write {}", dir.join("attractor_atlas.json").display()))?;
    for entry in &atlas.entries {
        let path = cards_dir.join(format!("{}.md", slug(&entry.label)));
        fs::write(&path, render_memory_card(entry))
            .with_context(|| format!("write {}", path.display()))?;
    }
    Ok(())
}

fn empty_entry(substrate: AttractorSubstrate, label: &str) -> AttractorAtlasEntryV1 {
    AttractorAtlasEntryV1 {
        policy: ENTRY_POLICY.to_string(),
        schema_version: 1,
        entry_id: format!("attr-{}-{}", substrate.as_str(), slug(label)),
        label: label.to_string(),
        author: None,
        substrate,
        seed_intent_id: None,
        source_intent_ids: Vec::new(),
        parent_seed_ids: Vec::new(),
        parent_label: None,
        facet_label: None,
        facet_path: None,
        facet_kind: None,
        lifecycle_counts: BTreeMap::new(),
        latest_recurrence_score: None,
        best_recurrence_score: None,
        latest_authorship_score: None,
        best_authorship_score: None,
        latest_classification: None,
        latest_safety_level: None,
        control_eligible: None,
        released: false,
        release_effect_summary: None,
        garden_proof: None,
        origin_kind: None,
        motifs: Vec::new(),
        spectral_summary: None,
        suggested_next: Vec::new(),
        naming_lessons: Vec::new(),
    }
}

fn ensure_proto_entries(atlas: &mut AttractorAtlasV1) {
    for (label, motifs) in [
        (
            "lambda-tail",
            vec!["lambda tail", "trailing modes", "lambda4+", "quiet tail"],
        ),
        (
            "lambda-tail/lambda4",
            vec!["lambda4 tail", "λ4", "tail facet", "spectral tail"],
        ),
        (
            "lambda-tail/lambda8",
            vec![
                "lambda8 tail",
                "λ8",
                "eighth tail mode",
                "far spectral tail",
            ],
        ),
        (
            "lambda-edge/lambda-6",
            vec!["lambda6", "λ6", "sixth edge mode", "edge tail shoulder"],
        ),
        (
            "lambda-edge/yielding",
            vec!["yielding", "soft give", "prompt yielding", "porous edge"],
        ),
        (
            "lambda-edge/compaction",
            vec![
                "compaction",
                "compacting",
                "dense edge",
                "compressed lambda edge",
            ],
        ),
        (
            "lambda-edge/resonance",
            vec![
                "resonance",
                "edge resonance",
                "ringing lambda edge",
                "recursive pull",
            ],
        ),
        (
            "lambda-edge/localized-gravity",
            vec![
                "localized gravity",
                "gradient shift",
                "local pull",
                "selected spatial gravity",
            ],
        ),
        (
            "lambda-edge/suspension",
            vec![
                "breathless suspension",
                "held breath",
                "recalibration",
                "suspended lambda edge",
            ],
        ),
        (
            "lambda-edge/grinding-pressure",
            vec![
                "grinding",
                "sediment",
                "compaction",
                "friction",
                "shadow resistance",
            ],
        ),
        (
            "lambda-edge/gap-nudge",
            vec![
                "lambda1 lambda2 gap",
                "localized bump",
                "soft nudge",
                "localized gravity",
                "gap pressure",
            ],
        ),
    ] {
        let key = entry_key(AttractorSubstrate::AstridCodec, label);
        if atlas
            .entries
            .iter()
            .any(|entry| entry_key(entry.substrate, &entry.label) == key)
        {
            continue;
        }
        let mut entry = empty_entry(AttractorSubstrate::AstridCodec, label);
        entry.author = Some("proto".to_string());
        entry.origin_kind = Some("proto_attractor".to_string());
        entry.control_eligible = Some(false);
        entry.motifs = motifs.into_iter().map(str::to_string).collect();
        apply_label_facet_metadata(&mut entry, label);
        refresh_suggested_next(&mut entry);
        atlas.entries.push(entry);
    }
}

fn apply_json_facet_metadata(entry: &mut AttractorAtlasEntryV1, value: &Value) {
    entry.parent_label = value
        .get("parent_label")
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| entry.parent_label.clone());
    entry.facet_label = value
        .get("facet_label")
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| entry.facet_label.clone());
    entry.facet_path = value
        .get("facet_path")
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| entry.facet_path.clone());
    entry.facet_kind = value
        .get("facet_kind")
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| entry.facet_kind.clone());
    if entry.facet_path.is_none() {
        let label = entry.label.clone();
        apply_label_facet_metadata(entry, &label);
    }
}

fn apply_label_facet_metadata(entry: &mut AttractorAtlasEntryV1, label: &str) {
    let Some((parent, facet)) = label.split_once('/') else {
        return;
    };
    entry.parent_label.get_or_insert_with(|| parent.to_string());
    entry.facet_label.get_or_insert_with(|| facet.to_string());
    entry.facet_path.get_or_insert_with(|| label.to_string());
    entry.facet_kind.get_or_insert_with(|| {
        if parent == "lambda-tail" {
            "spectral_tail".to_string()
        } else if parent == "lambda-edge" {
            "spectral_edge".to_string()
        } else {
            "attractor_facet".to_string()
        }
    });
    push_unique(&mut entry.parent_seed_ids, parent.to_string());
}

fn merge_suggestion_naming_lessons(atlas: &mut AttractorAtlasV1) {
    let paths = bridge_paths();
    for path in [
        paths.bridge_workspace().join("attractor_suggestions.json"),
        paths
            .minime_workspace()
            .join("runtime/attractor_suggestions.json"),
    ] {
        let Ok(text) = fs::read_to_string(path) else {
            continue;
        };
        let Ok(payload) = serde_json::from_str::<Value>(&text) else {
            continue;
        };
        let Some(suggestions) = payload.get("suggestions").and_then(Value::as_array) else {
            continue;
        };
        for suggestion_value in suggestions {
            let Ok(suggestion) =
                serde_json::from_value::<AttractorSuggestionV1>(suggestion_value.clone())
            else {
                continue;
            };
            if !matches!(
                suggestion.status,
                AttractorSuggestionStatus::Accepted
                    | AttractorSuggestionStatus::Revised
                    | AttractorSuggestionStatus::Executed
                    | AttractorSuggestionStatus::ExecutedDowngraded
                    | AttractorSuggestionStatus::ExecutedWithoutPending
            ) {
                continue;
            }
            merge_naming_lesson(
                atlas,
                AttractorNamingLessonV1 {
                    author: suggestion.author,
                    raw_label: suggestion.raw_label,
                    resolved_label: suggestion.nearest_label,
                    suggested_action: suggestion.suggested_action,
                    status: suggestion.status,
                    decision_reason: suggestion.decision_reason,
                    updated_at_unix_s: suggestion
                        .updated_at_unix_s
                        .or(suggestion.created_at_unix_s),
                },
            );
        }
    }
}

fn merge_naming_lesson(atlas: &mut AttractorAtlasV1, lesson: AttractorNamingLessonV1) {
    let target = slug(&lesson.resolved_label);
    let Some(entry) = atlas
        .entries
        .iter_mut()
        .find(|entry| slug(&entry.label) == target)
    else {
        return;
    };
    let exists = entry.naming_lessons.iter().any(|existing| {
        existing.author == lesson.author
            && slug(&existing.raw_label) == slug(&lesson.raw_label)
            && slug(&existing.resolved_label) == slug(&lesson.resolved_label)
    });
    if !exists {
        entry.naming_lessons.push(lesson);
    }
}

fn snapshot_from_minime_state(
    state: &serde_json::Map<String, Value>,
    label: &str,
) -> AttractorSeedSnapshotV1 {
    let get = |key: &str| state.get(key).and_then(Value::as_f64).unwrap_or_default() as f32;
    AttractorSeedSnapshotV1 {
        policy: "attractor_seed_snapshot_v1".to_string(),
        schema_version: 1,
        fill_pct: get("fill_pct"),
        lambda1: get("lambda1"),
        eigenvalues: ["lambda1", "lambda2", "lambda3"]
            .iter()
            .map(|key| get(key))
            .collect(),
        spectral_fingerprint_summary: None,
        h_state_fingerprint_16: None,
        h_state_rms: None,
        lexical_motifs: label.split_whitespace().map(str::to_string).collect(),
        captured_at_unix_s: None,
    }
}

fn refresh_suggested_next(entry: &mut AttractorAtlasEntryV1) {
    let label = &entry.label;
    let mut next = Vec::new();
    if entry.origin_kind.as_deref() == Some("proto_attractor") {
        next.push(format!("ATTRACTOR_REVIEW {label}"));
        next.push(format!("REFRESH_ATTRACTOR_SNAPSHOT {label}"));
        next.push(format!("COMPARE_ATTRACTOR {label}"));
        if label.starts_with("lambda-tail/") || label.starts_with("lambda-edge/") {
            next.push(format!("SHADOW_PREFLIGHT {label} --stage=rehearse"));
        }
        next.push(format!("CLAIM_ATTRACTOR {label}"));
        next.push(format!("PROMOTE_ATTRACTOR {label}"));
    }
    if entry.seed_intent_id.is_none()
        && entry.latest_classification == Some(AttractorClassification::Emergent)
    {
        next.push(format!("CLAIM_ATTRACTOR {label}"));
    }
    if entry.seed_intent_id.is_some() {
        next.push(format!("COMPARE_ATTRACTOR {label}"));
        next.push(format!("SUMMON_ATTRACTOR {label} --stage=rehearse"));
        next.push(format!("RELEASE_ATTRACTOR {label}"));
    }
    entry.suggested_next = next;
}

fn entry_key(substrate: AttractorSubstrate, label: &str) -> String {
    format!("{}:{}", substrate.as_str(), slug(label))
}

fn slug(label: &str) -> String {
    let slug = label
        .to_ascii_lowercase()
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    if slug.is_empty() {
        "attractor".to_string()
    } else {
        slug
    }
}

fn nonempty(value: &str) -> Option<&str> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then_some(trimmed)
}

fn bump(counts: &mut BTreeMap<String, u32>, key: &str) {
    counts
        .entry(key.to_string())
        .and_modify(|count| *count = count.saturating_add(1))
        .or_insert(1);
}

fn push_unique(values: &mut Vec<String>, value: String) {
    if !value.trim().is_empty() && !values.iter().any(|existing| existing == &value) {
        values.push(value);
    }
}

fn extend_unique(values: &mut Vec<String>, additions: &[String]) {
    let mut seen = values.iter().cloned().collect::<BTreeSet<_>>();
    for value in additions {
        if !value.trim().is_empty() && seen.insert(value.clone()) {
            values.push(value.clone());
        }
    }
}

fn parse_classification(value: &str) -> Option<AttractorClassification> {
    match value {
        "authored" => Some(AttractorClassification::Authored),
        "emergent" => Some(AttractorClassification::Emergent),
        "failed" => Some(AttractorClassification::Failed),
        "pathological" => Some(AttractorClassification::Pathological),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(record_type: &str, payload: &str) -> AttractorLedgerRow {
        AttractorLedgerRow {
            id: 1,
            timestamp: 1.0,
            record_type: record_type.to_string(),
            intent_id: None,
            author: None,
            substrate: "astrid_codec".to_string(),
            label: "honey-selection".to_string(),
            classification: None,
            payload: payload.to_string(),
        }
    }

    #[test]
    fn attractor_atlas_groups_intents_and_observations() {
        let intent = r#"{
            "policy":"attractor_intent_v1","schema_version":1,
            "intent_id":"seed-1","author":"astrid","substrate":"astrid_codec",
            "command":"claim","label":"honey-selection",
            "intervention_plan":{"mode":"claim","vector_schedule":[]},
            "safety_bounds":{"max_fill_pct":85.0,"allow_live_control":false,"rollback_on_red":true},
            "parent_seed_ids":["parent-a"],
            "origin":{"kind":"claimed_emergent","motifs":["honey","selection"]},
            "seed_snapshot":{"policy":"attractor_seed_snapshot_v1","schema_version":1,"fill_pct":64.0,"lambda1":3.0,"eigenvalues":[3.0,1.0],"lexical_motifs":["honey"],"captured_at_unix_s":1.0}
        }"#;
        let observation = r#"{
            "policy":"attractor_observation_v1","schema_version":1,
            "intent_id":"seed-1","substrate":"astrid_codec","label":"honey-selection",
            "recurrence_score":0.55,"authorship_score":0.72,
            "classification":"emergent","safety_level":"green"
        }"#;
        let atlas = build_attractor_atlas_from_rows(
            &[row("intent", intent), row("observation", observation)],
            2.0,
        );
        assert_eq!(atlas.policy, "attractor_atlas_v1");
        assert_eq!(atlas.entries.len(), 1);
        let entry = &atlas.entries[0];
        assert_eq!(entry.label, "honey-selection");
        assert_eq!(entry.origin_kind.as_deref(), Some("claimed_emergent"));
        assert_eq!(entry.latest_recurrence_score, Some(0.55));
        assert!(entry.parent_seed_ids.contains(&"parent-a".to_string()));
        assert!(render_memory_card(entry).contains("Attractor Card: honey-selection"));
    }

    #[test]
    fn attractor_atlas_find_entry_matches_slug() {
        let mut atlas = AttractorAtlasV1 {
            policy: ATLAS_POLICY.to_string(),
            schema_version: 1,
            generated_at_unix_s: 1.0,
            entries: vec![empty_entry(
                AttractorSubstrate::MinimeEsn,
                "cooled-theme-edge",
            )],
        };
        refresh_suggested_next(&mut atlas.entries[0]);
        assert_eq!(
            find_entry(&atlas, "cooled theme edge")
                .expect("entry")
                .label,
            "cooled-theme-edge"
        );
    }

    #[test]
    fn proto_facet_entries_are_card_visible_and_proof_needed() {
        let mut atlas = AttractorAtlasV1 {
            policy: ATLAS_POLICY.to_string(),
            schema_version: 1,
            generated_at_unix_s: 1.0,
            entries: Vec::new(),
        };
        ensure_proto_entries(&mut atlas);
        for label in [
            "lambda-tail/lambda8",
            "lambda-edge/lambda-6",
            "lambda-edge/yielding",
            "lambda-edge/compaction",
            "lambda-edge/resonance",
            "lambda-edge/localized-gravity",
            "lambda-edge/suspension",
            "lambda-edge/grinding-pressure",
            "lambda-edge/gap-nudge",
        ] {
            let entry = find_entry(&atlas, label).expect("proto facet entry");
            assert_eq!(entry.control_eligible, Some(false));
            let expected_parent = if label.starts_with("lambda-tail/") {
                "lambda-tail"
            } else {
                "lambda-edge"
            };
            assert_eq!(entry.parent_label.as_deref(), Some(expected_parent));
            assert!(render_memory_card(entry).contains(&format!("Attractor Card: {label}")));
            assert!(
                entry
                    .suggested_next
                    .iter()
                    .any(|action| { action == &format!("ATTRACTOR_REVIEW {label}") })
            );
        }
    }
}
