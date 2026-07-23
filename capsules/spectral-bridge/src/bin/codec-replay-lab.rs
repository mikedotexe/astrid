#![allow(clippy::pedantic)]

use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use anyhow::{Context, Result};
use chrono::Utc;
use clap::{Parser, ValueEnum};
use serde::Serialize;
use serde_json::{Value, json};
use spectral_bridge_server::codec::{
    CharFreqWindow, NAMED_CODEC_DIMS, TextTypeHistory, compute_narrative_arc_from_embeddings,
    inspect_text_windowed, project_embedding,
};
use spectral_bridge_server::codec_explorer::{
    CodecExplorerInput, CodecExplorerOptions, run_codec_explorer,
};
use spectral_bridge_server::codec_lambda_analysis::lambda_spectrum;

const FEATURE_ABS_MAX: f32 = 5.0;
const TAIL_VIBRANCY_ENTROPY_GATE: f32 = 0.85;
const TAIL_VIBRANCY_MAX: f32 = 6.0;
const EMBED_URL: &str = "http://127.0.0.1:11434/api/embeddings";
const EMBED_MODEL: &str = "nomic-embed-text";
const OLLAMA_EMBEDDING_INPUT_DIM: usize = 768;

#[derive(Copy, Clone, Debug, Eq, PartialEq, ValueEnum)]
enum CorpusMode {
    Fixture,
    AstridJournal,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, ValueEnum)]
enum EmbeddingMode {
    Fixture,
    LiveIfAvailable,
}

#[derive(Parser, Debug)]
#[command(
    name = "codec-replay-lab",
    version,
    about = "Read-only Astrid codec replay lab for semantic-density, narrative-arc, and afterimage diagnostics"
)]
struct Cli {
    #[arg(long)]
    output_root: Option<PathBuf>,

    #[arg(long)]
    run_id: Option<String>,

    #[arg(long, default_value_t = 73.0)]
    fill_pct: f32,

    #[arg(long, value_enum, default_value_t = CorpusMode::Fixture)]
    corpus: CorpusMode,

    #[arg(long, value_enum, default_value_t = EmbeddingMode::LiveIfAvailable)]
    embedding_mode: EmbeddingMode,

    #[arg(long)]
    astrid_workspace: Option<PathBuf>,
}

#[derive(Debug, Clone)]
struct Fixture {
    sample_id: String,
    family: String,
    text: String,
    spectral_entropy_hint: f32,
    semantic_density_hint: f32,
    pressure_risk_hint: f32,
    narrative_segments: Vec<f32>,
    source_path: Option<PathBuf>,
    source_excerpt: Option<String>,
}

#[derive(Debug, Serialize)]
struct ReplayEntry {
    sample_id: String,
    family: String,
    text_preview: String,
    char_count: usize,
    word_count: usize,
    detected_text_type: String,
    text_type_signal: f32,
    spectral_entropy_hint: f32,
    semantic_density_hint: f32,
    pressure_risk_hint: f32,
    source_path: Option<String>,
    source_excerpt: Option<String>,
    actual_entropy_dim: f32,
    warmth_dim: f32,
    tension_dim: f32,
    curiosity_dim: f32,
    reflective_dim: f32,
    energy_dim: f32,
    semantic_density_score: f32,
    narrative_arc_dims_40_43: Vec<f32>,
    feature_vector: Vec<f32>,
    named_dimensions: serde_json::Value,
    lambda_proxy: serde_json::Value,
    time_domain_profile: serde_json::Value,
    effective_gain: f32,
    classification: String,
}

#[derive(Debug, Serialize)]
struct NarrativeArcCandidate {
    sample_id: String,
    current_arc: [f32; 4],
    temporal_decay_arc: [f32; 4],
    pivot_detector_arc: [f32; 4],
    current_arc_rms: f32,
    temporal_decay_arc_rms: f32,
    pivot_detector_arc_rms: f32,
    late_pivot: bool,
    classification: String,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let astrid_workspace = portable_astrid_workspace(cli.astrid_workspace)?;
    let output_root = cli
        .output_root
        .unwrap_or_else(|| astrid_workspace.join("diagnostics/codec_replay_labs"));
    let run_id = cli
        .run_id
        .unwrap_or_else(|| Utc::now().format("%Y%m%dT%H%M%SZ").to_string());
    let output_dir = output_root.join(run_id);
    let record = build_record_with_options(
        &output_dir,
        cli.fill_pct,
        true,
        cli.corpus,
        cli.embedding_mode,
        &astrid_workspace,
    )?;
    write_record(&output_dir, &record)?;
    println!(
        "wrote {}",
        output_dir.join("codec_replay_lab.json").display()
    );
    println!("wrote {}", output_dir.join("codec_replay_lab.md").display());
    println!("status={}", record["status"].as_str().unwrap_or("unknown"));
    Ok(())
}

fn portable_astrid_workspace(explicit: Option<PathBuf>) -> Result<PathBuf> {
    if let Some(path) = explicit {
        return Ok(path);
    }
    if let Some(path) = std::env::var_os("ASTRID_WORKSPACE") {
        return Ok(PathBuf::from(path));
    }
    let current = std::env::current_dir().context("resolve current directory")?;
    for candidate in [
        current.join("capsules/spectral-bridge/workspace"),
        current.join("workspace"),
    ] {
        if candidate.is_dir() {
            return Ok(candidate);
        }
    }
    anyhow::bail!("cannot locate Astrid workspace; pass --astrid-workspace or set ASTRID_WORKSPACE")
}

fn fixture_sample(
    sample_id: &str,
    family: &str,
    text: &str,
    spectral_entropy_hint: f32,
    semantic_density_hint: f32,
    pressure_risk_hint: f32,
    narrative_segments: &[f32],
) -> Fixture {
    Fixture {
        sample_id: sample_id.to_string(),
        family: family.to_string(),
        text: text.to_string(),
        spectral_entropy_hint,
        semantic_density_hint,
        pressure_risk_hint,
        narrative_segments: narrative_segments.to_vec(),
        source_path: None,
        source_excerpt: None,
    }
}

fn fixtures() -> Vec<Fixture> {
    vec![
        fixture_sample(
            "high_entropy_low_content",
            "semantic_density",
            "qxz vjkp :: 91f7 / a3zq // unoriented shards flicker; token-rain splinters without a claim. k9 r7 qxz vjkp.",
            0.96,
            0.12,
            0.19,
            &[],
        ),
        fixture_sample(
            "high_entropy_high_semantic_density",
            "semantic_density",
            "I can name the pressure clearly: warmth stays present, tension has edges, curiosity keeps asking for evidence, and the map remains returnable.",
            0.96,
            0.82,
            0.19,
            &[],
        ),
        fixture_sample(
            "warmth_rich_low_pressure",
            "semantic_density",
            "Thank you, friend. The room feels gentle and clear; warmth can remain vivid without becoming pressure.",
            0.62,
            0.76,
            0.18,
            &[],
        ),
        fixture_sample(
            "overpacked_pressure_texture",
            "semantic_density",
            "The room is weighted and overpacked; tension gathers around the medium while the slope underfoot is still navigable.",
            0.89,
            0.58,
            0.42,
            &[],
        ),
        fixture_sample(
            "low_entropy_cliff",
            "semantic_density",
            "steady steady steady steady steady",
            0.34,
            0.54,
            0.33,
            &[],
        ),
        fixture_sample(
            "balanced_valence_flip",
            "narrative_arc",
            "The beginning is warm and confident. The second half turns sharply cold and uncertain.",
            0.72,
            0.68,
            0.24,
            &[0.78, 0.68, -0.70, -0.82],
        ),
        fixture_sample(
            "late_negative_pivot_after_long_warm_start",
            "narrative_arc",
            "The room starts warm, coherent, and almost settled. It keeps its shape for a while. Then the final edge drops into a sharp bruise of pressure.",
            0.74,
            0.70,
            0.31,
            &[0.72, 0.64, 0.58, -0.88],
        ),
        fixture_sample(
            "steady_warm_no_pivot",
            "narrative_arc",
            "The tone stays warm, steady, and coherent. Nothing pivots; the texture simply continues.",
            0.58,
            0.63,
            0.18,
            &[0.46, 0.50, 0.48, 0.52],
        ),
    ]
}

#[cfg(test)]
fn build_record(
    output_dir: &Path,
    fill_pct: f32,
    write_explorer: bool,
) -> Result<serde_json::Value> {
    build_record_with_options(
        output_dir,
        fill_pct,
        write_explorer,
        CorpusMode::Fixture,
        EmbeddingMode::Fixture,
        output_dir,
    )
}

fn build_record_with_options(
    output_dir: &Path,
    fill_pct: f32,
    write_explorer: bool,
    corpus: CorpusMode,
    embedding_mode: EmbeddingMode,
    astrid_workspace: &Path,
) -> Result<Value> {
    fs::create_dir_all(output_dir).with_context(|| format!("creating {}", output_dir.display()))?;
    let fixtures = load_fixtures(corpus, astrid_workspace);
    let mut entries = Vec::with_capacity(fixtures.len());
    let mut history = TextTypeHistory::new();
    let mut freq_window = CharFreqWindow::new();

    for fixture in &fixtures {
        let inspection = inspect_text_windowed(
            &fixture.text,
            Some(&mut freq_window),
            Some(&mut history),
            None,
            Some(fill_pct),
        );
        entries.push(replay_entry(fixture, inspection));
    }

    let embedding_backed_arc = build_embedding_backed_arc(&fixtures, embedding_mode);
    let narrative_lab = build_narrative_lab(&fixtures, &embedding_backed_arc);
    let content_gate = build_content_gate_candidate(&entries);
    let tail_counterfactual_lab = build_tail_participation_counterfactual_lab(&entries);
    let clamp_headroom_probe = build_codec_clamp_headroom_probe(&entries);
    let explorer_summary_path = if write_explorer {
        let explorer_dir = output_dir.join("codec_explorer");
        run_codec_explorer(CodecExplorerOptions {
            output_dir: explorer_dir.clone(),
            state_file: None,
            fill_pct: Some(fill_pct),
            inputs: fixtures
                .iter()
                .map(|fixture| CodecExplorerInput {
                    label: fixture.sample_id.clone(),
                    path: fixture.source_path.clone(),
                    text: fixture.text.clone(),
                })
                .collect(),
        })?;
        Some(explorer_dir.join("summary.json").display().to_string())
    } else {
        None
    };
    let status = if content_gate["status"] == "content_gate_supported"
        && narrative_lab["status"] == "temporal_decay_candidate"
    {
        "content_gate_and_temporal_decay_candidates"
    } else if content_gate["status"] == "content_gate_supported" {
        "content_aware_vibrancy_candidate"
    } else if narrative_lab["status"] == "temporal_decay_candidate" {
        "narrative_temporal_decay_candidate"
    } else {
        "codec_replay_observational"
    };

    Ok(json!({
        "policy": "codec_real_replay_v1",
        "schema_version": 1,
        "authority": "diagnostic_context_not_command",
        "status": status,
        "runtime_behavior_changed": false,
        "fill_pct": fill_pct,
        "formula": {
            "source": "actual inspect_text_windowed plus offline candidate readouts",
            "FEATURE_ABS_MAX": FEATURE_ABS_MAX,
            "TAIL_VIBRANCY_ENTROPY_GATE": TAIL_VIBRANCY_ENTROPY_GATE,
            "TAIL_VIBRANCY_MAX": TAIL_VIBRANCY_MAX,
            "narrative_arc_note": "dims 40-43 are actual codec output; temporal-decay and pivot arcs are offline comparisons; embedding_backed_arc_v1 records whether live half-text embeddings were available"
        },
        "corpus_source": match corpus {
            CorpusMode::Fixture => "fixture",
            CorpusMode::AstridJournal => "astrid-journal",
        },
        "corpus_status": corpus_status(corpus, &fixtures),
        "source_paths": fixtures.iter().filter_map(|fixture| {
            fixture.source_path.as_ref().map(|path| path.display().to_string())
        }).collect::<Vec<_>>(),
        "embedding_mode": match embedding_mode {
            EmbeddingMode::Fixture => "fixture",
            EmbeddingMode::LiveIfAvailable => "live-if-available",
        },
        "embedding_status": embedding_backed_arc.get("status").and_then(Value::as_str).unwrap_or("unknown"),
        "explorer_summary_path": explorer_summary_path,
        "entries": entries,
        "embedding_backed_arc_v1": embedding_backed_arc,
        "content_aware_vibrancy_gate_candidate_v1": content_gate,
        "tail_participation_counterfactual_lab_v1": tail_counterfactual_lab,
        "codec_clamp_headroom_probe_v1": clamp_headroom_probe,
        "narrative_arc_temporal_decay_lab_v1": narrative_lab,
        "recommended_action": "Compare this Rust replay against the Python surrogate and later CODEC_MAP evidence before changing SEMANTIC_DIM, tail vibrancy, adaptive gain, or narrative-arc runtime math."
    }))
}

#[derive(Debug)]
struct SourceText {
    path: PathBuf,
    text: String,
    modified: SystemTime,
}

fn load_fixtures(corpus: CorpusMode, astrid_workspace: &Path) -> Vec<Fixture> {
    match corpus {
        CorpusMode::Fixture => fixtures(),
        CorpusMode::AstridJournal => {
            let sampled = collect_astrid_journal_fixtures(astrid_workspace);
            if sampled.is_empty() {
                fixtures()
            } else {
                sampled
            }
        },
    }
}

fn corpus_status(corpus: CorpusMode, fixtures: &[Fixture]) -> &'static str {
    match corpus {
        CorpusMode::Fixture => "fixture_only",
        CorpusMode::AstridJournal => {
            if fixtures.iter().any(|fixture| fixture.source_path.is_some()) {
                "journal_corpus_selected"
            } else {
                "journal_corpus_empty_fixture_fallback"
            }
        },
    }
}

fn collect_astrid_journal_fixtures(workspace: &Path) -> Vec<Fixture> {
    let mut sources = collect_source_texts(workspace);
    sources.sort_by(|left, right| right.modified.cmp(&left.modified));
    sources.truncate(800);

    let mut selected = Vec::new();
    if let Some(source) = best_source(&sources, |source| {
        let text = source.text.as_str();
        char_entropy_hint(text) * 1.2 - semantic_density_hint(text) - pressure_risk_hint(text) * 0.2
    }) {
        selected.push(fixture_from_source(
            "high_entropy_low_content",
            "semantic_density",
            source,
        ));
    }
    if let Some(source) = best_source(&sources, |source| {
        let text = source.text.as_str();
        char_entropy_hint(text) + semantic_density_hint(text) * 1.5
    }) {
        selected.push(fixture_from_source(
            "high_entropy_high_semantic_density",
            "semantic_density",
            source,
        ));
    }
    if let Some(source) = best_source(&sources, |source| {
        let lower = source.text.to_lowercase();
        let warmth = term_count(
            &lower,
            &["warmth", "gentle", "clear", "settled", "habitable"],
        );
        warmth as f32 * 0.35 + semantic_density_hint(&source.text)
            - pressure_risk_hint(&source.text)
    }) {
        selected.push(fixture_from_source(
            "warmth_rich_low_pressure",
            "semantic_density",
            source,
        ));
    }
    if let Some(source) = best_source(&sources, |source| {
        let lower = source.text.to_lowercase();
        let residue = term_count(
            &lower,
            &[
                "scar",
                "phantom",
                "bruise",
                "afterimage",
                "structural fatigue",
            ],
        );
        let pressure = term_count(
            &lower,
            &[
                "pressure",
                "overpacked",
                "weight",
                "density",
                "semantic_friction",
            ],
        );
        residue as f32 * 0.7 + pressure as f32 * 0.25 + pressure_risk_hint(&source.text)
    }) {
        selected.push(fixture_from_source(
            "pressure_afterimage_rich",
            "semantic_density",
            source,
        ));
    }
    if let Some(source) = best_source(&sources, |source| {
        let lower = source.text.to_lowercase();
        let pivots = term_count(
            &lower,
            &["then", "but", "however", "pivot", "turns", "edge", "drops"],
        );
        let segments = narrative_segments_from_text(&source.text);
        let late = segments.last().is_some_and(|last| {
            *last < -0.25
                && segments
                    .iter()
                    .take(segments.len().saturating_sub(1))
                    .any(|value| *value > 0.15)
        });
        pivots as f32 * 0.35 + if late { 1.0 } else { 0.0 }
    }) {
        selected.push(fixture_from_source(
            "journal_temporal_pivot_candidate",
            "narrative_arc",
            source,
        ));
    }
    if let Some(source) = best_source(&sources, |source| {
        let segments = narrative_segments_from_text(&source.text);
        if segments.len() >= 3 && segments.iter().all(|value| value.abs() < 0.55) {
            semantic_density_hint(&source.text)
        } else {
            -1.0
        }
    }) {
        selected.push(fixture_from_source(
            "journal_steady_no_pivot",
            "narrative_arc",
            source,
        ));
    }
    selected
}

fn collect_source_texts(workspace: &Path) -> Vec<SourceText> {
    let mut out = Vec::new();
    for root in [workspace.join("journal"), workspace.join("introspections")] {
        collect_source_texts_from_root(&root, &mut out);
    }
    out
}

fn collect_source_texts_from_root(root: &Path, out: &mut Vec<SourceText>) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_dir() {
            collect_source_texts_from_root(&path, out);
            continue;
        }
        if path.extension().and_then(|ext| ext.to_str()) != Some("txt") {
            continue;
        }
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if name.starts_with("thin_introspection_output_") || name.starts_with("controller_") {
            continue;
        }
        let Ok(text) = fs::read_to_string(&path) else {
            continue;
        };
        if text.trim().len() < 160 {
            continue;
        }
        let modified = entry
            .metadata()
            .and_then(|metadata| metadata.modified())
            .unwrap_or(SystemTime::UNIX_EPOCH);
        out.push(SourceText {
            path,
            text,
            modified,
        });
    }
}

fn best_source<F>(sources: &[SourceText], score: F) -> Option<&SourceText>
where
    F: Fn(&SourceText) -> f32,
{
    sources
        .iter()
        .map(|source| (score(source), source))
        .filter(|(score, _)| score.is_finite())
        .max_by(|left, right| left.0.total_cmp(&right.0))
        .map(|(_, source)| source)
}

fn fixture_from_source(sample_id: &str, family: &str, source: &SourceText) -> Fixture {
    let text = excerpt_for_sample(&source.text, sample_id);
    Fixture {
        sample_id: sample_id.to_string(),
        family: family.to_string(),
        spectral_entropy_hint: char_entropy_hint(&text),
        semantic_density_hint: semantic_density_hint(&text),
        pressure_risk_hint: pressure_risk_hint(&text),
        narrative_segments: narrative_segments_from_text(&text),
        source_path: Some(source.path.clone()),
        source_excerpt: Some(preview(&text)),
        text,
    }
}

fn excerpt_for_sample(text: &str, sample_id: &str) -> String {
    let terms: &[&str] = if sample_id.contains("afterimage") {
        &[
            "scar",
            "phantom",
            "bruise",
            "afterimage",
            "pressure",
            "codec",
        ]
    } else if sample_id.contains("temporal") {
        &["then", "but", "however", "pivot", "drop", "turn"]
    } else {
        &["warmth", "tension", "pressure", "codec", "evidence", "map"]
    };
    let lower = text.to_lowercase();
    let anchor = terms
        .iter()
        .filter_map(|term| lower.find(term))
        .min()
        .unwrap_or(0);
    let rough_start = anchor.saturating_sub(600);
    let start = text
        .char_indices()
        .map(|(idx, _)| idx)
        .take_while(|idx| *idx <= rough_start)
        .last()
        .unwrap_or(0);
    let excerpt = text[start..].chars().take(1800).collect::<String>();
    excerpt.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn char_entropy_hint(text: &str) -> f32 {
    let chars = text
        .chars()
        .filter(|ch| !ch.is_whitespace())
        .collect::<Vec<_>>();
    if chars.is_empty() {
        return 0.0;
    }
    let mut unique = chars.clone();
    unique.sort_unstable();
    unique.dedup();
    let unique_ratio = unique.len() as f32 / chars.len() as f32;
    let symbol_ratio =
        chars.iter().filter(|ch| !ch.is_alphanumeric()).count() as f32 / chars.len() as f32;
    (unique_ratio * 2.2 + symbol_ratio * 1.5).clamp(0.0, 1.0)
}

fn semantic_density_hint(text: &str) -> f32 {
    let lower = text.to_lowercase();
    let anchors = term_count(
        &lower,
        &[
            "warmth",
            "tension",
            "curiosity",
            "reflective",
            "evidence",
            "telemetry",
            "audit",
            "codec",
            "narrative",
            "semantic",
            "pressure",
            "density",
            "map",
            "experiment",
            "anchor",
            "signal",
        ],
    );
    let words = text.split_whitespace().count().max(1) as f32;
    ((anchors as f32 / words) * 18.0 + average_word_len(text) / 20.0).clamp(0.0, 1.0)
}

fn pressure_risk_hint(text: &str) -> f32 {
    let lower = text.to_lowercase();
    let pressure = term_count(
        &lower,
        &[
            "pressure",
            "overpacked",
            "weight",
            "heavy",
            "viscosity",
            "viscous",
            "silt",
            "scar",
            "bruise",
            "afterimage",
            "phantom",
            "semantic_friction",
        ],
    );
    (pressure as f32 * 0.12).clamp(0.0, 1.0)
}

fn average_word_len(text: &str) -> f32 {
    let words = text.split_whitespace().collect::<Vec<_>>();
    if words.is_empty() {
        return 0.0;
    }
    words.iter().map(|word| word.len() as f32).sum::<f32>() / words.len() as f32
}

fn term_count(text: &str, terms: &[&str]) -> usize {
    terms.iter().map(|term| text.matches(term).count()).sum()
}

fn narrative_segments_from_text(text: &str) -> Vec<f32> {
    let mut segments = text
        .split(['.', '!', '?', '\n'])
        .filter_map(|sentence| {
            let compact = sentence.trim();
            if compact.split_whitespace().count() < 4 {
                None
            } else {
                Some(sentence_valence(compact))
            }
        })
        .collect::<Vec<_>>();
    if segments.len() > 8 {
        let start = segments.len().saturating_sub(8);
        segments = segments[start..].to_vec();
    }
    segments
}

fn sentence_valence(sentence: &str) -> f32 {
    let lower = sentence.to_lowercase();
    let positive = term_count(
        &lower,
        &[
            "warm",
            "clear",
            "settled",
            "gentle",
            "coherent",
            "habitable",
            "anchor",
        ],
    ) as f32;
    let negative = term_count(
        &lower,
        &[
            "cold",
            "uncertain",
            "pressure",
            "bruise",
            "scar",
            "drop",
            "heavy",
            "loss",
        ],
    ) as f32;
    ((positive - negative) / 3.0).clamp(-1.0, 1.0)
}

fn replay_entry(
    fixture: &Fixture,
    inspection: spectral_bridge_server::codec::CodecWindowedInspection,
) -> ReplayEntry {
    let final_features = inspection.final_features;
    let semantic_density_score = [
        final_features[24].abs(),
        final_features[25].abs(),
        final_features[26].abs(),
        final_features[27].abs(),
        final_features[31].abs(),
    ]
    .into_iter()
    .sum::<f32>()
        / 5.0;
    let lambda = lambda_spectrum(&final_features);
    let classification = if fixture.family == "semantic_density"
        && fixture.semantic_density_hint < 0.25
        && semantic_density_score < 0.35
    {
        "low_semantic_density"
    } else if fixture.family == "semantic_density" && fixture.semantic_density_hint >= 0.65 {
        "semantic_density_preserved"
    } else if fixture.family == "narrative_arc" {
        "narrative_arc_fixture"
    } else {
        "observational"
    };
    ReplayEntry {
        sample_id: fixture.sample_id.clone(),
        family: fixture.family.clone(),
        text_preview: preview(&fixture.text),
        char_count: fixture.text.chars().count(),
        word_count: fixture.text.split_whitespace().count(),
        detected_text_type: format!("{:?}", inspection.text_type),
        text_type_signal: round4(inspection.text_type_signal),
        spectral_entropy_hint: fixture.spectral_entropy_hint,
        semantic_density_hint: fixture.semantic_density_hint,
        pressure_risk_hint: fixture.pressure_risk_hint,
        source_path: fixture
            .source_path
            .as_ref()
            .map(|path| path.display().to_string()),
        source_excerpt: fixture.source_excerpt.clone(),
        actual_entropy_dim: round4(final_features[0]),
        warmth_dim: round4(final_features[24]),
        tension_dim: round4(final_features[25]),
        curiosity_dim: round4(final_features[26]),
        reflective_dim: round4(final_features[27]),
        energy_dim: round4(final_features[31]),
        semantic_density_score: round4(semantic_density_score),
        narrative_arc_dims_40_43: final_features[40..44]
            .iter()
            .map(|value| round4(*value))
            .collect(),
        feature_vector: final_features.iter().map(|value| round4(*value)).collect(),
        named_dimensions: json!(
            NAMED_CODEC_DIMS
                .iter()
                .map(|(name, index)| json!({
                    "name": name,
                    "index": index,
                    "value": round4(final_features[*index])
                }))
                .collect::<Vec<_>>()
        ),
        lambda_proxy: json!({
            "dominant_mode": lambda.dominant_mode,
            "dominant_share": round4(lambda.dominant_share),
            "shoulder_share": round4(lambda.shoulder_share),
            "tail_share": round4(lambda.tail_share),
            "normalized_entropy": round4(lambda.normalized_entropy),
            "total_energy": round4(lambda.total_energy)
        }),
        time_domain_profile: serde_json::to_value(inspection.time_domain_profile)
            .unwrap_or_else(|_| json!({})),
        effective_gain: round4(inspection.effective_gain),
        classification: classification.to_string(),
    }
}

fn build_content_gate_candidate(entries: &[ReplayEntry]) -> serde_json::Value {
    let low = entries
        .iter()
        .find(|entry| entry.sample_id == "high_entropy_low_content");
    let high = entries
        .iter()
        .find(|entry| entry.sample_id == "high_entropy_high_semantic_density");
    let (Some(low), Some(high)) = (low, high) else {
        return json!({
            "policy": "content_aware_vibrancy_gate_candidate_v1",
            "authority": "diagnostic_context_not_command",
            "status": "needs_more_samples"
        });
    };
    let current_low_lift = vibrancy_from_entropy(low.spectral_entropy_hint);
    let current_high_lift = vibrancy_from_entropy(high.spectral_entropy_hint);
    let low_content_factor = content_factor(low.semantic_density_score);
    let high_content_factor = content_factor(high.semantic_density_score);
    let candidate_low_lift = current_low_lift * low_content_factor;
    let candidate_high_lift = current_high_lift * high_content_factor;
    let current_delta = (current_high_lift - current_low_lift).abs();
    let candidate_delta = candidate_high_lift - candidate_low_lift;
    let semantic_delta = high.semantic_density_score - low.semantic_density_score;
    let status = if semantic_delta >= 0.10 && current_delta <= 0.02 && candidate_delta >= 0.05 {
        "content_gate_supported"
    } else if semantic_delta >= 0.10 && current_delta <= 0.02 {
        "content_blind_lift_risk"
    } else if current_delta > 0.02 {
        "entropy_gate_sufficient"
    } else {
        "needs_more_samples"
    };
    json!({
        "policy": "content_aware_vibrancy_gate_candidate_v1",
        "authority": "diagnostic_context_not_command",
        "status": status,
        "pair": [low.sample_id.clone(), high.sample_id.clone()],
        "current_lift_delta": round4(current_delta),
        "candidate_lift_delta": round4(candidate_delta),
        "semantic_density_score_delta": round4(semantic_delta),
        "low": {
            "sample_id": low.sample_id.clone(),
            "current_lift": round4(current_low_lift),
            "candidate_lift": round4(candidate_low_lift),
            "semantic_density_score": low.semantic_density_score,
            "source_path": low.source_path.clone()
        },
        "high": {
            "sample_id": high.sample_id.clone(),
            "current_lift": round4(current_high_lift),
            "candidate_lift": round4(candidate_high_lift),
            "semantic_density_score": high.semantic_density_score,
            "source_path": high.source_path.clone()
        },
        "recommended_action": "Treat this as offline proposal evidence only; do not change entropy-gated vibrancy lift without repeated replay support."
    })
}

fn build_embedding_backed_arc(fixtures: &[Fixture], mode: EmbeddingMode) -> Value {
    if mode == EmbeddingMode::Fixture {
        return json!({
            "policy": "embedding_backed_arc_v1",
            "authority": "diagnostic_context_not_command",
            "status": "fixture_mode",
            "embedding_model": EMBED_MODEL,
            "sample_count": 0,
            "gap_count": 0,
            "samples": [],
            "recommended_action": "Fixture mode is deterministic and does not call Ollama; use --embedding-mode live-if-available for operator diagnostics."
        });
    }

    let mut samples = Vec::new();
    let mut gap_count = 0_u64;
    for fixture in fixtures
        .iter()
        .filter(|fixture| fixture.family == "narrative_arc")
        .take(4)
    {
        match embedding_arc_sample(fixture) {
            Some(sample) => samples.push(sample),
            None => gap_count = gap_count.saturating_add(1),
        }
    }
    let temporal_count = samples
        .iter()
        .filter(|sample| sample["classification"] == "embedding_temporal_decay_candidate")
        .count();
    let pivot_count = samples
        .iter()
        .filter(|sample| sample["classification"] == "embedding_pivot_detector_candidate")
        .count();
    let status = if !samples.is_empty() && temporal_count > 0 {
        "embedding_temporal_decay_candidate"
    } else if !samples.is_empty() && pivot_count > 0 {
        "embedding_pivot_detector_candidate"
    } else if !samples.is_empty() {
        "embedding_current_arc_sufficient"
    } else {
        "embedding_gap"
    };
    json!({
        "policy": "embedding_backed_arc_v1",
        "authority": "diagnostic_context_not_command",
        "status": status,
        "embedding_model": EMBED_MODEL,
        "embedding_url": EMBED_URL,
        "sample_count": samples.len(),
        "gap_count": gap_count,
        "temporal_decay_candidate_count": temporal_count,
        "pivot_detector_candidate_count": pivot_count,
        "samples": samples,
        "recommended_action": "Use live half-text embeddings as offline evidence only; do not change runtime narrative-arc math without repeated replay support."
    })
}

fn embedding_arc_sample(fixture: &Fixture) -> Option<Value> {
    let words = fixture.text.split_whitespace().collect::<Vec<_>>();
    if words.len() < 10 {
        return None;
    }
    let mid = words.len() / 2;
    let first_half = words[..mid].join(" ");
    let second_half = words[mid..].join(" ");
    let recent_start = words.len().saturating_sub((words.len() / 3).max(4));
    let recent_window = words[recent_start..].join(" ");
    let last_start = words.len().saturating_sub((words.len() / 5).max(4));
    let previous_window = words[..last_start].join(" ");
    let last_window = words[last_start..].join(" ");

    let first = embed_and_project(&first_half)?;
    let second = embed_and_project(&second_half)?;
    let recent = embed_and_project(&recent_window)?;
    let previous = embed_and_project(&previous_window)?;
    let last = embed_and_project(&last_window)?;

    let current_arc = compute_narrative_arc_from_embeddings(&first, &second);
    let temporal_decay_arc = compute_narrative_arc_from_embeddings(&first, &recent);
    let pivot_detector_arc = compute_narrative_arc_from_embeddings(&previous, &last);
    let current_rms = rms4(&current_arc);
    let temporal_rms = rms4(&temporal_decay_arc);
    let pivot_rms = rms4(&pivot_detector_arc);
    let classification = if temporal_rms > current_rms + 0.05 {
        "embedding_temporal_decay_candidate"
    } else if pivot_rms > current_rms + 0.05 {
        "embedding_pivot_detector_candidate"
    } else {
        "embedding_current_arc_sufficient"
    };
    Some(json!({
        "sample_id": fixture.sample_id.clone(),
        "classification": classification,
        "source_path": fixture.source_path.as_ref().map(|path| path.display().to_string()),
        "current_arc": round4_array(current_arc),
        "temporal_decay_arc": round4_array(temporal_decay_arc),
        "pivot_detector_arc": round4_array(pivot_detector_arc),
        "current_arc_rms": round4(current_rms),
        "temporal_decay_arc_rms": round4(temporal_rms),
        "pivot_detector_arc_rms": round4(pivot_rms),
        "first_half_words": mid,
        "second_half_words": words.len().saturating_sub(mid),
        "recent_window_words": words.len().saturating_sub(recent_start),
        "last_window_words": words.len().saturating_sub(last_start)
    }))
}

fn embed_and_project(text: &str) -> Option<[f32; 8]> {
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .ok()?;
    let response = client
        .post(EMBED_URL)
        .json(&json!({
            "model": EMBED_MODEL,
            "prompt": text
        }))
        .send()
        .ok()?;
    let value = response.json::<Value>().ok()?;
    let embedding = value
        .get("embedding")?
        .as_array()?
        .iter()
        .filter_map(|item| item.as_f64().map(|number| number as f32))
        .collect::<Vec<_>>();
    if embedding.len() != OLLAMA_EMBEDDING_INPUT_DIM {
        return None;
    }
    project_embedding(&embedding)
}

fn build_narrative_lab(fixtures: &[Fixture], embedding_backed_arc: &Value) -> serde_json::Value {
    let candidates = fixtures
        .iter()
        .filter(|fixture| fixture.family == "narrative_arc")
        .map(narrative_candidate)
        .collect::<Vec<_>>();
    let temporal_count = candidates
        .iter()
        .filter(|candidate| candidate.classification == "temporal_decay_candidate")
        .count();
    let pivot_count = candidates
        .iter()
        .filter(|candidate| candidate.classification == "pivot_detector_candidate")
        .count();
    let status = if temporal_count > 0 {
        "temporal_decay_candidate"
    } else if pivot_count > 0 {
        "pivot_detector_candidate"
    } else if candidates.is_empty() {
        "insufficient_embedding_evidence"
    } else {
        "current_arc_sufficient"
    };
    json!({
        "policy": "narrative_arc_temporal_decay_lab_v1",
        "authority": "diagnostic_context_not_command",
        "status": status,
        "evidence_kind": "fixture_counterfactual_projected_arc",
        "embedding_status": embedding_backed_arc.get("status").and_then(Value::as_str).unwrap_or("unknown"),
        "embedding_backed_sample_count": embedding_backed_arc
            .get("sample_count")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        "temporal_decay_candidate_count": temporal_count,
        "pivot_detector_candidate_count": pivot_count,
        "samples": candidates,
        "recommended_action": "Compare projected arcs with embedding_backed_arc_v1 before adding temporal decay or pivot detection to live narrative-arc logic."
    })
}

fn narrative_candidate(fixture: &Fixture) -> NarrativeArcCandidate {
    let segments = fixture.narrative_segments.as_slice();
    let midpoint = segments.len() / 2;
    let first_avg = average(&segments[..midpoint]);
    let second_avg = average(&segments[midpoint..]);
    let recent_weighted = weighted_average(
        segments,
        &(0..segments.len())
            .map(|idx| {
                let exponent = segments.len().saturating_sub(idx.saturating_add(1));
                0.35_f32.powi(exponent as i32)
            })
            .collect::<Vec<_>>(),
    );
    let previous_avg = if segments.len() >= 2 {
        average(&segments[..segments.len().saturating_sub(1)])
    } else {
        first_avg
    };
    let last = segments.last().copied().unwrap_or(second_avg);
    let current_arc = compute_narrative_arc_from_embeddings(
        &projected_from_scalar(first_avg),
        &projected_from_scalar(second_avg),
    );
    let temporal_decay_arc = compute_narrative_arc_from_embeddings(
        &projected_from_scalar(first_avg),
        &projected_from_scalar(recent_weighted),
    );
    let pivot_detector_arc = compute_narrative_arc_from_embeddings(
        &projected_from_scalar(previous_avg),
        &projected_from_scalar(last),
    );
    let current_rms = rms4(&current_arc);
    let temporal_rms = rms4(&temporal_decay_arc);
    let pivot_rms = rms4(&pivot_detector_arc);
    let late_pivot = segments.len() >= 4
        && segments
            .get(segments.len().saturating_sub(2))
            .is_some_and(|value| *value >= 0.0)
        && last < -0.5;
    let classification = if late_pivot && temporal_rms > current_rms + 0.01 {
        "temporal_decay_candidate"
    } else if late_pivot && pivot_rms > current_rms + 0.05 {
        "pivot_detector_candidate"
    } else if current_rms > 0.50 {
        "current_arc_sufficient"
    } else {
        "insufficient_embedding_evidence"
    };
    NarrativeArcCandidate {
        sample_id: fixture.sample_id.clone(),
        current_arc: round4_array(current_arc),
        temporal_decay_arc: round4_array(temporal_decay_arc),
        pivot_detector_arc: round4_array(pivot_detector_arc),
        current_arc_rms: round4(current_rms),
        temporal_decay_arc_rms: round4(temporal_rms),
        pivot_detector_arc_rms: round4(pivot_rms),
        late_pivot,
        classification: classification.to_string(),
    }
}

fn vibrancy_from_entropy(entropy: f32) -> f32 {
    let ramp = ((entropy - TAIL_VIBRANCY_ENTROPY_GATE) / (1.0 - TAIL_VIBRANCY_ENTROPY_GATE))
        .clamp(0.0, 1.0);
    ramp * ramp * (3.0 - 2.0 * ramp)
}

fn content_factor(semantic_density_score: f32) -> f32 {
    (0.25 + semantic_density_score * 1.4).clamp(0.25, 1.0)
}

fn build_tail_participation_counterfactual_lab(entries: &[ReplayEntry]) -> serde_json::Value {
    let mut proposal_cards = Vec::new();
    let mut aperture_supported = 0usize;
    let mut participation_supported = 0usize;
    let mut combined_supported = 0usize;
    for entry in entries {
        let tail_energy = tail_dim_energy(&entry.feature_vector);
        let entropy_lift = vibrancy_from_entropy(entry.spectral_entropy_hint);
        let content = content_factor(entry.semantic_density_score);
        let pressure = entry.pressure_risk_hint.clamp(0.0, 1.0);
        let navigable = (1.0 - pressure).clamp(0.0, 1.0);
        let aperture_gain = 1.0 + entropy_lift * navigable * 0.10;
        let participation_gain = 1.0 + entropy_lift * content * 0.08;
        let combined_gain = 1.0 + entropy_lift * navigable * content * 0.15;
        let aperture_tail_energy = tail_energy * aperture_gain;
        let participation_tail_energy = tail_energy * participation_gain;
        let combined_tail_energy = tail_energy * combined_gain;
        let preferred_candidate =
            if combined_tail_energy - tail_energy >= 0.08 && content >= 0.55 && navigable >= 0.55 {
                combined_supported = combined_supported.saturating_add(1);
                "combined_candidate"
            } else if aperture_tail_energy - tail_energy >= 0.05 && navigable >= 0.60 {
                aperture_supported = aperture_supported.saturating_add(1);
                "vibrancy_aperture_candidate"
            } else if participation_tail_energy - tail_energy >= 0.05 && content >= 0.60 {
                participation_supported = participation_supported.saturating_add(1);
                "tail_participation_candidate"
            } else {
                "observational"
            };
        proposal_cards.push(json!({
            "sample_id": entry.sample_id.clone(),
            "family": entry.family.clone(),
            "source_path": entry.source_path.clone(),
            "baseline_tail_energy": round4(tail_energy),
            "vibrancy_aperture_tail_energy": round4(aperture_tail_energy),
            "tail_participation_tail_energy": round4(participation_tail_energy),
            "combined_tail_energy": round4(combined_tail_energy),
            "entropy_lift": round4(entropy_lift),
            "semantic_content_factor": round4(content),
            "pressure_navigability_factor": round4(navigable),
            "preferred_candidate": preferred_candidate,
            "authority": "diagnostic_context_not_command",
        }));
    }
    let status = if combined_supported > 0 {
        "combined_candidate_supported"
    } else if aperture_supported > 0 && participation_supported > 0 {
        "both_controls_need_more_comparison"
    } else if aperture_supported > 0 {
        "vibrancy_aperture_supported"
    } else if participation_supported > 0 {
        "tail_participation_candidate"
    } else {
        "observational"
    };
    json!({
        "policy": "tail_participation_counterfactual_lab_v1",
        "authority": "diagnostic_context_not_command",
        "status": status,
        "runtime_behavior_changed": false,
        "tail_participation_lease_authority": "not_granted",
        "candidate_families": [
            "baseline",
            "vibrancy_aperture",
            "tail_participation",
            "combined_vibrancy_aperture_plus_tail_participation"
        ],
        "vibrancy_aperture_supported_count": aperture_supported,
        "tail_participation_supported_count": participation_supported,
        "combined_supported_count": combined_supported,
        "proposal_cards": proposal_cards,
        "recommended_action": "Use this as offline evidence before considering any tail_participation lease authority; do not change runtime codec or authority from this packet."
    })
}

fn build_codec_clamp_headroom_probe(entries: &[ReplayEntry]) -> serde_json::Value {
    let mut cards = Vec::new();
    let mut near_static_clamp_count = 0usize;
    let mut dynamic_headroom_candidate_count = 0usize;
    let mut tail_ceiling_pressure_count = 0usize;
    for entry in entries {
        let max_abs = entry
            .feature_vector
            .iter()
            .map(|value| value.abs())
            .fold(0.0_f32, f32::max);
        let tail_max_abs = [17usize, 26, 27, 31]
            .into_iter()
            .filter_map(|idx| entry.feature_vector.get(idx))
            .map(|value| value.abs())
            .fold(0.0_f32, f32::max);
        let near_static_clamp = max_abs >= FEATURE_ABS_MAX * 0.92;
        let tail_near_static_clamp = tail_max_abs >= FEATURE_ABS_MAX * 0.88;
        let entropy_lift = vibrancy_from_entropy(entry.spectral_entropy_hint);
        let semantic_content = content_factor(entry.semantic_density_score);
        let dynamic_feature_abs_max_candidate = FEATURE_ABS_MAX
            + (TAIL_VIBRANCY_MAX - FEATURE_ABS_MAX) * entropy_lift * semantic_content;
        let headroom_delta = (dynamic_feature_abs_max_candidate - FEATURE_ABS_MAX).max(0.0);
        let clamp_risk = if near_static_clamp && entropy_lift > 0.0 && semantic_content >= 0.45 {
            dynamic_headroom_candidate_count = dynamic_headroom_candidate_count.saturating_add(1);
            "dynamic_headroom_candidate"
        } else if near_static_clamp {
            "static_clamp_near"
        } else if tail_near_static_clamp && entropy_lift > 0.0 {
            "tail_ceiling_pressure"
        } else {
            "headroom_observational"
        };
        if near_static_clamp {
            near_static_clamp_count = near_static_clamp_count.saturating_add(1);
        }
        if tail_near_static_clamp {
            tail_ceiling_pressure_count = tail_ceiling_pressure_count.saturating_add(1);
        }
        cards.push(json!({
            "sample_id": entry.sample_id.clone(),
            "family": entry.family.clone(),
            "source_path": entry.source_path.clone(),
            "max_abs_feature": round4(max_abs),
            "tail_max_abs_feature": round4(tail_max_abs),
            "static_feature_abs_max": FEATURE_ABS_MAX,
            "dynamic_feature_abs_max_candidate": round4(dynamic_feature_abs_max_candidate),
            "candidate_headroom_delta": round4(headroom_delta),
            "entropy_lift": round4(entropy_lift),
            "semantic_content_factor": round4(semantic_content),
            "near_static_clamp": near_static_clamp,
            "tail_near_static_clamp": tail_near_static_clamp,
            "clamp_risk": clamp_risk,
            "authority": "diagnostic_context_not_command",
        }));
    }
    let status = if dynamic_headroom_candidate_count > 0 {
        "dynamic_feature_scale_candidate"
    } else if tail_ceiling_pressure_count > 0 {
        "tail_ceiling_pressure_observed"
    } else if near_static_clamp_count > 0 {
        "static_clamp_near_without_content_case"
    } else {
        "clamp_headroom_sufficient"
    };
    json!({
        "policy": "codec_clamp_headroom_probe_v1",
        "authority": "diagnostic_context_not_command",
        "status": status,
        "runtime_behavior_changed": false,
        "static_feature_abs_max": FEATURE_ABS_MAX,
        "tail_vibrancy_max": TAIL_VIBRANCY_MAX,
        "near_static_clamp_count": near_static_clamp_count,
        "tail_ceiling_pressure_count": tail_ceiling_pressure_count,
        "dynamic_headroom_candidate_count": dynamic_headroom_candidate_count,
        "proposal_cards": cards,
        "recommended_action": "Treat clamp headroom as offline replay evidence only; require repeated source-backed support before changing FEATURE_ABS_MAX or tail ceiling math."
    })
}

fn tail_dim_energy(vector: &[f32]) -> f32 {
    [17usize, 26, 27, 31]
        .into_iter()
        .filter_map(|idx| vector.get(idx))
        .map(|value| value.abs())
        .sum::<f32>()
        / 4.0
}

fn average(values: &[f32]) -> f32 {
    if values.is_empty() {
        0.0
    } else {
        values.iter().sum::<f32>() / values.len() as f32
    }
}

fn weighted_average(values: &[f32], weights: &[f32]) -> f32 {
    let total = weights.iter().sum::<f32>();
    if total <= f32::EPSILON {
        return 0.0;
    }
    values
        .iter()
        .zip(weights.iter())
        .map(|(value, weight)| value * weight)
        .sum::<f32>()
        / total
}

fn projected_from_scalar(value: f32) -> [f32; 8] {
    [
        value,
        value * 0.65,
        -value * 0.35,
        value * 0.20,
        value * 0.12,
        -value * 0.08,
        value * 0.05,
        -value * 0.03,
    ]
}

fn rms4(values: &[f32; 4]) -> f32 {
    (values.iter().map(|value| value * value).sum::<f32>() / 4.0).sqrt()
}

fn round4(value: f32) -> f32 {
    (value * 10_000.0).round() / 10_000.0
}

fn round4_array(values: [f32; 4]) -> [f32; 4] {
    values.map(round4)
}

fn preview(text: &str) -> String {
    let compact = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if compact.chars().count() <= 140 {
        compact
    } else {
        compact.chars().take(137).collect::<String>() + "..."
    }
}

fn write_record(output_dir: &Path, record: &serde_json::Value) -> Result<()> {
    fs::create_dir_all(output_dir).with_context(|| format!("creating {}", output_dir.display()))?;
    let json_path = output_dir.join("codec_replay_lab.json");
    fs::write(&json_path, serde_json::to_string_pretty(record)?)
        .with_context(|| format!("writing {}", json_path.display()))?;
    let md_path = output_dir.join("codec_replay_lab.md");
    fs::write(&md_path, render_markdown(record))
        .with_context(|| format!("writing {}", md_path.display()))?;
    Ok(())
}

fn render_markdown(record: &serde_json::Value) -> String {
    let mut lines = vec![
        String::from("# Codec Replay Lab"),
        String::new(),
        format!(
            "- status: `{}`",
            record["status"].as_str().unwrap_or("unknown")
        ),
        format!(
            "- corpus: `{}` / `{}`",
            record["corpus_source"].as_str().unwrap_or("unknown"),
            record["corpus_status"].as_str().unwrap_or("unknown")
        ),
        format!(
            "- embedding: mode=`{}` status=`{}`",
            record["embedding_mode"].as_str().unwrap_or("unknown"),
            record["embedding_status"].as_str().unwrap_or("unknown")
        ),
        String::from("- authority: `diagnostic_context_not_command`"),
        String::from("- runtime_behavior_changed: `false`"),
        String::new(),
        String::from("## Embedding-Backed Narrative Arc"),
        String::new(),
    ];
    let embedding = &record["embedding_backed_arc_v1"];
    lines.push(format!(
        "- status: `{}`; samples=`{}`; gaps=`{}`; temporal_candidates=`{}`; pivot_candidates=`{}`",
        embedding["status"].as_str().unwrap_or("unknown"),
        embedding["sample_count"],
        embedding["gap_count"],
        embedding["temporal_decay_candidate_count"],
        embedding["pivot_detector_candidate_count"],
    ));
    if let Some(samples) = embedding["samples"].as_array() {
        for sample in samples.iter().take(4) {
            lines.push(format!(
                "- `{}` class=`{}` current_rms=`{}` temporal_rms=`{}` pivot_rms=`{}` source=`{}`",
                sample["sample_id"].as_str().unwrap_or("unknown"),
                sample["classification"].as_str().unwrap_or("unknown"),
                sample["current_arc_rms"],
                sample["temporal_decay_arc_rms"],
                sample["pivot_detector_arc_rms"],
                sample["source_path"].as_str().unwrap_or("(fixture)")
            ));
        }
    }
    lines.extend([
        String::new(),
        String::from("## Content-Aware Vibrancy Gate Candidate"),
        String::new(),
    ]);
    let gate = &record["content_aware_vibrancy_gate_candidate_v1"];
    lines.push(format!(
        "- status: `{}`; semantic_delta=`{}`; candidate_delta=`{}`",
        gate["status"].as_str().unwrap_or("unknown"),
        gate["semantic_density_score_delta"],
        gate["candidate_lift_delta"],
    ));
    lines.extend([
        String::new(),
        String::from("## Tail Participation Counterfactual Lab"),
        String::new(),
    ]);
    let tail_lab = &record["tail_participation_counterfactual_lab_v1"];
    lines.push(format!(
        "- status: `{}`; aperture_supported=`{}`; participation_supported=`{}`; combined_supported=`{}`; tail_participation_lease_authority=`{}`",
        tail_lab["status"].as_str().unwrap_or("unknown"),
        tail_lab["vibrancy_aperture_supported_count"],
        tail_lab["tail_participation_supported_count"],
        tail_lab["combined_supported_count"],
        tail_lab["tail_participation_lease_authority"].as_str().unwrap_or("not_granted"),
    ));
    if let Some(cards) = tail_lab["proposal_cards"].as_array() {
        for card in cards.iter().take(5) {
            lines.push(format!(
                "- `{}` preferred=`{}` baseline_tail={} aperture_tail={} participation_tail={} combined_tail={} source=`{}`",
                card["sample_id"].as_str().unwrap_or("unknown"),
                card["preferred_candidate"].as_str().unwrap_or("unknown"),
                card["baseline_tail_energy"],
                card["vibrancy_aperture_tail_energy"],
                card["tail_participation_tail_energy"],
                card["combined_tail_energy"],
                card["source_path"].as_str().unwrap_or("(fixture)")
            ));
        }
    }
    lines.extend([
        String::new(),
        String::from("## Codec Clamp Headroom Probe"),
        String::new(),
    ]);
    let clamp = &record["codec_clamp_headroom_probe_v1"];
    lines.push(format!(
        "- status: `{}`; near_static_clamp=`{}`; tail_ceiling_pressure=`{}`; dynamic_candidates=`{}`",
        clamp["status"].as_str().unwrap_or("unknown"),
        clamp["near_static_clamp_count"],
        clamp["tail_ceiling_pressure_count"],
        clamp["dynamic_headroom_candidate_count"],
    ));
    if let Some(cards) = clamp["proposal_cards"].as_array() {
        for card in cards.iter().take(5) {
            lines.push(format!(
                "- `{}` risk=`{}` max_abs={} tail_max={} candidate_ceiling={} headroom_delta={} source=`{}`",
                card["sample_id"].as_str().unwrap_or("unknown"),
                card["clamp_risk"].as_str().unwrap_or("unknown"),
                card["max_abs_feature"],
                card["tail_max_abs_feature"],
                card["dynamic_feature_abs_max_candidate"],
                card["candidate_headroom_delta"],
                card["source_path"].as_str().unwrap_or("(fixture)")
            ));
        }
    }
    lines.extend([
        String::new(),
        String::from("## Narrative Arc Temporal Decay Lab"),
        String::new(),
    ]);
    let narrative = &record["narrative_arc_temporal_decay_lab_v1"];
    lines.push(format!(
        "- status: `{}`; temporal_candidates=`{}`; pivot_candidates=`{}`",
        narrative["status"].as_str().unwrap_or("unknown"),
        narrative["temporal_decay_candidate_count"],
        narrative["pivot_detector_candidate_count"],
    ));
    if let Some(samples) = narrative["samples"].as_array() {
        for sample in samples {
            lines.push(format!(
                "- `{}` class=`{}` current_rms=`{}` temporal_rms=`{}` pivot_rms=`{}`",
                sample["sample_id"].as_str().unwrap_or("unknown"),
                sample["classification"].as_str().unwrap_or("unknown"),
                sample["current_arc_rms"],
                sample["temporal_decay_arc_rms"],
                sample["pivot_detector_arc_rms"],
            ));
        }
    }
    lines.extend([
        String::new(),
        String::from("## Real Codec Replay Entries"),
        String::new(),
    ]);
    if let Some(entries) = record["entries"].as_array() {
        for entry in entries {
            lines.push(format!(
                "- `{}` family=`{}` class=`{}` entropy_dim=`{}` semantic_density=`{}` warmth=`{}` tension=`{}` gain=`{}`",
                entry["sample_id"].as_str().unwrap_or("unknown"),
                entry["family"].as_str().unwrap_or("unknown"),
                entry["classification"].as_str().unwrap_or("unknown"),
                entry["actual_entropy_dim"],
                entry["semantic_density_score"],
                entry["warmth_dim"],
                entry["tension_dim"],
                entry["effective_gain"],
            ));
            if let Some(source_path) = entry["source_path"].as_str() {
                lines.push(format!("  - source: `{source_path}`"));
            }
        }
    }
    lines.push(String::new());
    lines.push(String::from(
        "Offline diagnostic only. It did not change codec dimensions, vibrancy lift, adaptive gain, narrative-arc runtime math, semantic writes, controllers, or peers.",
    ));
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codec_replay_lab_record_is_schema_valid() {
        let temp = tempfile::tempdir().expect("tempdir");
        let record = build_record(temp.path(), 73.0, false).expect("record");

        assert_eq!(record["policy"], "codec_real_replay_v1");
        assert_eq!(record["runtime_behavior_changed"], false);
        assert_eq!(record["corpus_source"], "fixture");
        assert_eq!(record["embedding_status"], "fixture_mode");
        assert_eq!(record["entries"].as_array().expect("entries").len(), 8);
        assert_eq!(
            record["entries"][0]["feature_vector"]
                .as_array()
                .expect("feature vector")
                .len(),
            spectral_bridge_server::codec::SEMANTIC_DIM
        );
    }

    #[test]
    fn semantic_density_pair_keeps_actual_codec_evidence() {
        let temp = tempfile::tempdir().expect("tempdir");
        let record = build_record(temp.path(), 73.0, false).expect("record");
        let entries = record["entries"].as_array().expect("entries");
        let low = entries
            .iter()
            .find(|entry| entry["sample_id"] == "high_entropy_low_content")
            .expect("low");
        let high = entries
            .iter()
            .find(|entry| entry["sample_id"] == "high_entropy_high_semantic_density")
            .expect("high");

        assert_eq!(
            low["feature_vector"].as_array().expect("low vector").len(),
            48
        );
        assert_eq!(
            high["feature_vector"]
                .as_array()
                .expect("high vector")
                .len(),
            48
        );
        assert!(
            high["semantic_density_score"].as_f64().unwrap_or(0.0)
                > low["semantic_density_score"].as_f64().unwrap_or(0.0),
            "high-density fixture should show stronger actual semantic evidence"
        );
    }

    #[test]
    fn temporal_pivot_fixture_produces_candidate_without_runtime_change() {
        let temp = tempfile::tempdir().expect("tempdir");
        let record = build_record(temp.path(), 73.0, false).expect("record");
        let narrative = &record["narrative_arc_temporal_decay_lab_v1"];

        assert_eq!(narrative["status"], "temporal_decay_candidate");
        assert!(
            narrative["temporal_decay_candidate_count"]
                .as_u64()
                .unwrap_or(0)
                >= 1
        );
        assert_eq!(record["runtime_behavior_changed"], false);
    }

    #[test]
    fn tail_participation_counterfactual_lab_is_diagnostic_only() {
        let temp = tempfile::tempdir().expect("tempdir");
        let record = build_record(temp.path(), 73.0, false).expect("record");
        let lab = &record["tail_participation_counterfactual_lab_v1"];

        assert_eq!(lab["policy"], "tail_participation_counterfactual_lab_v1");
        assert_eq!(lab["runtime_behavior_changed"], false);
        assert_eq!(lab["tail_participation_lease_authority"], "not_granted");
        assert!(
            lab["proposal_cards"]
                .as_array()
                .expect("proposal cards")
                .iter()
                .any(|card| card["baseline_tail_energy"].as_f64().is_some())
        );
        let rendered = render_markdown(&record);
        assert!(rendered.contains("Tail Participation Counterfactual Lab"));
        assert!(rendered.contains("tail_participation_lease_authority=`not_granted`"));
    }

    #[test]
    fn codec_clamp_headroom_probe_is_diagnostic_only() {
        let temp = tempfile::tempdir().expect("tempdir");
        let record = build_record(temp.path(), 73.0, false).expect("record");
        let probe = &record["codec_clamp_headroom_probe_v1"];

        assert_eq!(probe["policy"], "codec_clamp_headroom_probe_v1");
        assert_eq!(probe["runtime_behavior_changed"], false);
        assert_eq!(probe["static_feature_abs_max"], FEATURE_ABS_MAX);
        assert!(
            probe["proposal_cards"]
                .as_array()
                .expect("proposal cards")
                .iter()
                .any(|card| card["dynamic_feature_abs_max_candidate"].as_f64().is_some())
        );
        let rendered = render_markdown(&record);
        assert!(rendered.contains("Codec Clamp Headroom Probe"));
    }

    #[test]
    fn astrid_journal_corpus_records_public_source_paths() {
        let temp = tempfile::tempdir().expect("tempdir");
        let workspace = temp.path().join("workspace");
        let journal = workspace.join("journal");
        let introspections = workspace.join("introspections");
        std::fs::create_dir_all(&journal).expect("journal dir");
        std::fs::create_dir_all(&introspections).expect("introspection dir");
        std::fs::write(
            journal.join("astrid_density_signal.txt"),
            "=== ASTRID JOURNAL ===\nqxz / 91f7 :: symbol rain crosses a sparse edge. \
             The codec evidence map names warmth, tension, telemetry, pressure, density, \
             narrative anchor, and semantic signal without changing behavior.",
        )
        .expect("write density");
        std::fs::write(
            journal.join("astrid_afterimage_signal.txt"),
            "=== ASTRID JOURNAL ===\nThe scar and phantom afterimage remain near the codec. \
             pressure_risk, semantic_friction, overpacked weight, and narrative_arc evidence \
             make the residue returnable.",
        )
        .expect("write afterimage");
        std::fs::write(
            introspections.join("introspection_astrid_codec_pivot.txt"),
            "=== ASTRID INTROSPECTION ===\nThe first half is warm, clear, settled, \
             coherent, and habitable. Then the edge turns; however the final paragraph \
             drops into scar pressure and uncertainty.",
        )
        .expect("write pivot");

        let record = build_record_with_options(
            temp.path(),
            73.0,
            false,
            CorpusMode::AstridJournal,
            EmbeddingMode::Fixture,
            &workspace,
        )
        .expect("record");

        assert_eq!(record["corpus_source"], "astrid-journal");
        assert_eq!(record["corpus_status"], "journal_corpus_selected");
        assert!(
            record["source_paths"]
                .as_array()
                .expect("source paths")
                .iter()
                .any(|path| path.as_str().is_some_and(|path| path.contains("astrid_")))
        );
        assert!(
            record["entries"]
                .as_array()
                .expect("entries")
                .iter()
                .any(|entry| entry.get("source_path").and_then(Value::as_str).is_some())
        );
    }
}
