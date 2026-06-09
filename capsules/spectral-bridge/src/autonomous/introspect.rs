use std::fs;
use std::path::{Path, PathBuf};

use crate::paths::{BridgePaths, bridge_paths};

const INTROSPECT_WINDOW_LINES: usize = 400;
const INTROSPECT_MAX_FILE_BYTES: u64 = 2_000_000;
const REQUIRED_SECTIONS: &[&str] = &[
    "observed",
    "likely snags",
    "one test each",
    "suggested next",
];
const ALLOWED_EXTENSIONS: &[&str] = &[
    "py", "rs", "md", "txt", "json", "jsonl", "toml", "yaml", "yml", "csv", "sh", "plist",
];
const BLOCKED_DIRS: &[&str] = &[
    ".git",
    ".cache",
    ".venv",
    "__pycache__",
    "backups",
    "build",
    "dist",
    "node_modules",
    "target",
    "venv",
];

/// Source files for introspection — alternates between Astrid's own code
/// and minime's code so both architectures get examined.
#[derive(Debug, Clone)]
pub(super) struct IntrospectSource {
    pub(super) label: &'static str,
    pub(super) path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ResolvedIntrospectTarget {
    pub label: String,
    pub path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct IntrospectWindow {
    pub text: String,
    pub next_offset: Option<usize>,
}

#[must_use]
pub(super) fn introspect_sources() -> Vec<IntrospectSource> {
    let paths = bridge_paths();
    let bridge_root = paths.bridge_root();
    let minime_root = paths.minime_root();
    let astrid_root = paths.astrid_root();

    vec![
        IntrospectSource {
            label: "astrid:codec",
            path: bridge_root.join("src/codec.rs"),
        },
        IntrospectSource {
            label: "astrid:autonomous",
            path: bridge_root.join("src/autonomous.rs"),
        },
        IntrospectSource {
            label: "astrid:ws",
            path: bridge_root.join("src/ws.rs"),
        },
        IntrospectSource {
            label: "astrid:types",
            path: bridge_root.join("src/types.rs"),
        },
        IntrospectSource {
            label: "astrid:llm",
            path: bridge_root.join("src/llm.rs"),
        },
        IntrospectSource {
            label: "minime:regulator",
            path: minime_root.join("minime/src/regulator.rs"),
        },
        IntrospectSource {
            label: "minime:sensory_bus",
            path: minime_root.join("minime/src/sensory_bus.rs"),
        },
        IntrospectSource {
            label: "minime:esn",
            path: minime_root.join("minime/src/esn.rs"),
        },
        IntrospectSource {
            label: "minime:main(excerpt)",
            path: minime_root.join("minime/src/main.rs"),
        },
        IntrospectSource {
            label: "minime:autonomous_agent",
            path: minime_root.join("autonomous_agent.py"),
        },
        IntrospectSource {
            label: "proposal:phase_transitions",
            path: astrid_root
                .join("docs/steward-notes/AI_BEINGS_PHASE_TRANSITION_ARCHITECTURE.md"),
        },
        IntrospectSource {
            label: "proposal:bidirectional_contact",
            path: astrid_root.join(
                "docs/steward-notes/AI_BEINGS_BIDIRECTIONAL_CONTACT_AND_CORRESPONDENCE_ARCHITECTURE.md",
            ),
        },
        IntrospectSource {
            label: "proposal:distance_contact_control",
            path: astrid_root.join(
                "docs/steward-notes/AI_BEINGS_DISTANCE_CONTACT_CONTAINMENT_CONTROL_AND_PARTICIPATION_AUDIT.md",
            ),
        },
        IntrospectSource {
            label: "proposal:12d_glimpse",
            path: astrid_root.join(
                "docs/steward-notes/AI_BEINGS_MULTI_SCALE_REPRESENTATION_AND_12D_GLIMPSE_AUDIT.md",
            ),
        },
    ]
}

#[must_use]
pub(super) fn normalize_introspect_lookup(text: &str) -> String {
    let text = canonicalize_introspect_target_label(text);
    let mut normalized = String::with_capacity(text.len());
    let mut last_space = true;
    for ch in text.to_lowercase().chars() {
        if ch.is_ascii_alphanumeric() {
            normalized.push(ch);
            last_space = false;
        } else if !last_space {
            normalized.push(' ');
            last_space = true;
        }
    }
    normalized.trim().to_string()
}

#[must_use]
pub(super) fn canonicalize_introspect_target_label(text: &str) -> String {
    let mut cleaned = text.trim();

    if let Some((head, tail)) = cleaned.rsplit_once(" (")
        && tail.strip_suffix(')').is_some_and(|digits| {
            !digits.is_empty() && digits.chars().all(|ch| ch.is_ascii_digit())
        })
    {
        cleaned = head.trim_end();
    }

    loop {
        let trimmed = cleaned.trim();
        let unwrapped = [('[', ']'), ('(', ')'), ('`', '`'), ('"', '"'), ('\'', '\'')]
            .iter()
            .find_map(|(open, close)| trimmed.strip_prefix(*open)?.strip_suffix(*close))
            .filter(|inner| !inner.is_empty())
            .unwrap_or(trimmed);
        if unwrapped == cleaned {
            break;
        }
        cleaned = unwrapped;
    }

    for prefix in ["source=", "src=", "code=", "path=", "target="] {
        if let Some(rest) = cleaned.strip_prefix(prefix) {
            cleaned = rest.trim();
            break;
        }
    }

    cleaned.trim().to_string()
}

#[must_use]
fn is_placeholder_introspect_target(target_label: &str) -> bool {
    let normalized = normalize_introspect_lookup(target_label);
    matches!(
        normalized.as_str(),
        "source" | "line" | "source line" | "target" | "label" | "path" | "file"
    )
}

fn placeholder_introspect_error(target_label: &str) -> String {
    let display = target_label.trim();
    let display = if display.is_empty() {
        "the placeholder"
    } else {
        display
    };
    format!(
        "`{display}` is a syntax placeholder, not an INTROSPECT target; use a concrete label or path such as `astrid:llm`, `minime:regulator`, `minime:autonomous_agent`, or `capsules/spectral-bridge/src/autonomous/introspect.rs`"
    )
}

#[must_use]
pub(super) fn looks_like_introspect_path(target_label: &str) -> bool {
    let cleaned = canonicalize_introspect_target_label(target_label);
    cleaned.contains('/')
        || cleaned.contains('\\')
        || ALLOWED_EXTENSIONS.iter().any(|suffix| {
            cleaned
                .to_ascii_lowercase()
                .ends_with(&format!(".{suffix}"))
        })
}

#[must_use]
pub(super) fn introspect_path_candidates(target_label: &str) -> Vec<String> {
    let cleaned = canonicalize_introspect_target_label(target_label);
    if cleaned.is_empty() || !looks_like_introspect_path(&cleaned) {
        return Vec::new();
    }

    let mut candidates = vec![cleaned.clone()];
    for separator in [" — ", " -- ", " - "] {
        if let Some((prefix, _)) = cleaned.split_once(separator) {
            let candidate = prefix.trim();
            if candidate.is_empty()
                || !looks_like_introspect_path(candidate)
                || candidates.iter().any(|existing| existing == candidate)
            {
                continue;
            }
            candidates.push(candidate.to_string());
        }
    }
    candidates
}

fn introspect_source_aliases(source: &IntrospectSource) -> Vec<String> {
    let mut aliases = vec![normalize_introspect_lookup(source.label)];
    if let Some(name) = source.path.file_name().and_then(std::ffi::OsStr::to_str) {
        aliases.push(normalize_introspect_lookup(name));
    }
    if let Some(stem) = source.path.file_stem().and_then(std::ffi::OsStr::to_str) {
        aliases.push(normalize_introspect_lookup(stem));
    }
    if let Some((_, suffix)) = source.label.split_once(':') {
        aliases.push(normalize_introspect_lookup(suffix));
    }
    for alias in introspect_source_extra_aliases(source.label) {
        aliases.push(normalize_introspect_lookup(alias));
    }
    aliases.retain(|alias| !alias.is_empty());
    aliases.sort();
    aliases.dedup();
    aliases
}

fn introspect_source_extra_aliases(label: &str) -> &'static [&'static str] {
    match label {
        "minime:esn" => &[
            "async_rank1_submitted",
            "async rank1 submitted",
            "pending_rank1_depth",
            "pending rank1 depth",
            "rank1_us",
            "host_norm_us",
            "async_submit_us",
            "async_drain_us",
            "intro_fused_wait_us",
            "intro_tail_wait_us",
            "intro_first_read_us",
            "intro_tail_read_us",
            "rank1 ewma",
            "rank1 update",
            "host norm",
            "async rank1",
        ],
        "minime:autonomous_agent" => &[
            "pulse",
            "pulse model",
            "pulse ripple",
            "normalize action arg",
            "normalize action",
            "normalize perturb mode",
            "perturb parser",
            "action arg",
        ],
        _ => &[],
    }
}

fn semantic_introspect_target(target_label: &str) -> Option<(String, PathBuf)> {
    let normalized_target = normalize_introspect_lookup(target_label);
    if normalized_target.is_empty() {
        return None;
    }

    let paths = bridge_paths();
    let rules = [
        (
            &[
                "waveform",
                "wave",
                "morph wave",
                "pulse ripple",
                "chimera",
                "render audio",
                "spectral mix",
                "blend",
                "wav",
            ][..],
            paths.bridge_root().join("src/chimera.rs"),
        ),
        (
            &[
                "normalize audio",
                "write wav",
                "midi to frequency",
                "support",
                "audio utility",
            ][..],
            paths.bridge_root().join("src/chimera_support.rs"),
        ),
        (
            &[
                "pulse",
                "pulse model",
                "normalize action arg",
                "normalize action",
                "normalize perturb mode",
                "perturb parser",
                "action arg",
            ][..],
            paths.minime_root().join("autonomous_agent.py"),
        ),
    ];

    let mut best_match: Option<(usize, String, PathBuf)> = None;
    for (aliases, path) in rules {
        for alias in aliases {
            let normalized_alias = normalize_introspect_lookup(alias);
            if normalized_alias.is_empty() {
                continue;
            }
            let exact = normalized_alias == normalized_target;
            let token_match =
                format!(" {normalized_target} ").contains(&format!(" {normalized_alias} "));
            if exact || token_match {
                let score = normalized_alias.len();
                if score > best_match.as_ref().map_or(0, |(best, _, _)| *best) {
                    let label = path
                        .file_name()
                        .and_then(std::ffi::OsStr::to_str)
                        .unwrap_or(alias)
                        .to_string();
                    best_match = Some((score, label, path.clone()));
                }
            }
        }
    }
    best_match.map(|(_, label, path)| (label, path))
}

fn should_skip_introspect_dir(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    name.starts_with('.') || BLOCKED_DIRS.contains(&lower.as_str())
}

fn source_roots(paths: &BridgePaths) -> Vec<PathBuf> {
    vec![
        paths.bridge_root().join("src"),
        paths.astrid_root().join("docs/steward-notes"),
        paths.minime_root().join("minime/src"),
        paths.minime_root().join("autonomous_agent.py"),
    ]
}

fn workspace_memory_roots(paths: &BridgePaths) -> Vec<PathBuf> {
    let bridge_workspace = paths.bridge_workspace();
    let minime_workspace = paths.minime_workspace();
    [
        bridge_workspace.join("inbox/read"),
        bridge_workspace.join("outbox/delivered"),
        bridge_workspace.join("journal"),
        bridge_workspace.join("research"),
        bridge_workspace.join("action_threads"),
        minime_workspace.join("inbox/read"),
        minime_workspace.join("outbox/delivered"),
        minime_workspace.join("journal"),
        minime_workspace.join("research"),
        minime_workspace.join("action_threads"),
    ]
    .into_iter()
    .collect()
}

fn allowed_roots(paths: &BridgePaths) -> Vec<PathBuf> {
    source_roots(paths)
        .into_iter()
        .chain(workspace_memory_roots(paths))
        .collect()
}

fn is_relative_to(candidate: &Path, root: &Path) -> bool {
    if root.is_file() {
        candidate == root
    } else {
        candidate.starts_with(root)
    }
}

fn path_has_allowed_extension(path: &Path) -> bool {
    path.extension()
        .and_then(std::ffi::OsStr::to_str)
        .map(|ext| ALLOWED_EXTENSIONS.contains(&ext.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}

fn path_contains_blocked_dir(path: &Path) -> Option<String> {
    path.components().find_map(|component| {
        let text = component.as_os_str().to_string_lossy();
        let lower = text.to_ascii_lowercase();
        BLOCKED_DIRS
            .contains(&lower.as_str())
            .then(|| text.to_string())
    })
}

fn text_looks_noisy_or_binary(text: &str) -> bool {
    if text.as_bytes().contains(&0) {
        return true;
    }
    let mut total = 0usize;
    let mut suspicious = 0usize;
    for ch in text.chars().take(8000) {
        total = total.saturating_add(1);
        if ch.is_control() && !matches!(ch, '\n' | '\r' | '\t') {
            suspicious = suspicious.saturating_add(1);
        }
    }
    total > 0 && suspicious.saturating_mul(20) > total
}

fn canonicalize_root(root: &Path) -> Option<PathBuf> {
    if root.exists() {
        fs::canonicalize(root).ok()
    } else {
        None
    }
}

pub(super) fn validate_introspect_path(path: &Path) -> Result<PathBuf, String> {
    validate_introspect_path_with_roots(path, &allowed_roots(bridge_paths()))
}

fn validate_introspect_path_with_roots(path: &Path, roots: &[PathBuf]) -> Result<PathBuf, String> {
    let metadata = fs::metadata(path).map_err(|err| format!("target is not readable: {err}"))?;
    if !metadata.is_file() {
        return Err("target is not a regular file".to_string());
    }
    if metadata.len() > INTROSPECT_MAX_FILE_BYTES {
        return Err(format!(
            "target is too large for INTROSPECT ({} bytes > {} bytes)",
            metadata.len(),
            INTROSPECT_MAX_FILE_BYTES
        ));
    }
    if !path_has_allowed_extension(path) {
        return Err("target extension is not text/source-like".to_string());
    }

    let canonical = fs::canonicalize(path)
        .map_err(|err| format!("target could not be canonicalized: {err}"))?;
    if let Some(blocked) = path_contains_blocked_dir(&canonical) {
        return Err(format!("target is under blocked directory `{blocked}`"));
    }

    let allowed = roots
        .iter()
        .filter_map(|root| canonicalize_root(root))
        .any(|root| is_relative_to(&canonical, &root));
    if !allowed {
        return Err("target is outside approved INTROSPECT roots".to_string());
    }

    Ok(canonical)
}

fn candidate_relative_variants(candidate: &str, paths: &BridgePaths) -> Vec<PathBuf> {
    let normalized = candidate.replace('\\', "/");
    let stripped_workspace = normalized.strip_prefix("workspace/").unwrap_or(&normalized);
    let mut variants = vec![
        paths.bridge_root().join(&normalized),
        paths.astrid_root().join(&normalized),
        paths.minime_root().join(&normalized),
        paths.bridge_workspace().join(stripped_workspace),
        paths.minime_workspace().join(stripped_workspace),
    ];

    if let Some(rest) = normalized.strip_prefix("src/") {
        variants.push(paths.bridge_root().join("src").join(rest));
    }
    if let Some(rest) = normalized.strip_prefix("minime/src/") {
        variants.push(paths.minime_root().join("minime/src").join(rest));
    }
    variants
}

fn resolve_relative_introspect_path(target_label: &str) -> Result<PathBuf, String> {
    let cleaned = canonicalize_introspect_target_label(target_label);
    if cleaned.is_empty() {
        return Err("empty INTROSPECT target".to_string());
    }

    let candidate = PathBuf::from(&cleaned);
    if candidate.is_absolute() {
        return validate_introspect_path(&candidate);
    }

    let paths = bridge_paths();
    let mut last_error = None;
    for resolved in candidate_relative_variants(&cleaned, paths) {
        if resolved.is_file() {
            match validate_introspect_path(&resolved) {
                Ok(path) => return Ok(path),
                Err(reason) => last_error = Some(reason),
            }
        }
    }

    Err(last_error.unwrap_or_else(|| {
        "no matching text/source file was found in approved INTROSPECT roots".to_string()
    }))
}

fn search_introspect_roots_for_filename(file_name: &str) -> Result<PathBuf, String> {
    let file_name = canonicalize_introspect_target_label(file_name);
    let needle = Path::new(&file_name)
        .file_name()
        .and_then(std::ffi::OsStr::to_str)
        .ok_or_else(|| "INTROSPECT filename was empty".to_string())?
        .to_lowercase();
    let paths = bridge_paths();
    let mut stack: Vec<PathBuf> = source_roots(paths)
        .into_iter()
        .chain(workspace_memory_roots(paths))
        .collect();

    while let Some(path) = stack.pop() {
        if !path.exists() {
            continue;
        }
        if path.is_file() {
            if path
                .file_name()
                .and_then(std::ffi::OsStr::to_str)
                .is_some_and(|name| name.eq_ignore_ascii_case(&needle))
            {
                return validate_introspect_path(&path);
            }
            continue;
        }

        let entries = match fs::read_dir(&path) {
            Ok(entries) => entries,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let child = entry.path();
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if child.is_dir() {
                if should_skip_introspect_dir(&name) {
                    continue;
                }
                stack.push(child);
            } else if name.eq_ignore_ascii_case(&needle) {
                return validate_introspect_path(&child);
            }
        }
    }

    Err("no matching filename was found in approved INTROSPECT roots".to_string())
}

pub(super) fn resolve_introspect_target_result(
    target_label: &str,
    sources: &[IntrospectSource],
) -> Result<ResolvedIntrospectTarget, String> {
    let cleaned_target = canonicalize_introspect_target_label(target_label);
    let normalized_target = normalize_introspect_lookup(&cleaned_target);
    if normalized_target.is_empty() {
        return Err("empty INTROSPECT target".to_string());
    }
    if is_placeholder_introspect_target(target_label) {
        return Err(placeholder_introspect_error(target_label));
    }

    for source in sources {
        if introspect_source_aliases(source)
            .iter()
            .any(|alias| alias == &normalized_target)
        {
            let path = validate_introspect_path(&source.path)?;
            return Ok(ResolvedIntrospectTarget {
                label: source.label.to_string(),
                path,
            });
        }
    }

    if let Some((label, path)) = semantic_introspect_target(&cleaned_target) {
        let path = validate_introspect_path(&path)?;
        return Ok(ResolvedIntrospectTarget { label, path });
    }

    let path_candidates = introspect_path_candidates(&cleaned_target);
    if !path_candidates.is_empty() {
        let mut last_error = None;
        for candidate in &path_candidates {
            match resolve_relative_introspect_path(candidate)
                .or_else(|_| search_introspect_roots_for_filename(candidate))
            {
                Ok(path) => {
                    let label = path
                        .file_name()
                        .and_then(std::ffi::OsStr::to_str)
                        .unwrap_or(candidate.as_str())
                        .to_string();
                    return Ok(ResolvedIntrospectTarget { label, path });
                },
                Err(reason) => last_error = Some(reason),
            }
        }
        return Err(last_error.unwrap_or_else(|| "INTROSPECT target was not found".to_string()));
    }

    let target_tokens: Vec<&str> = normalized_target
        .split_whitespace()
        .filter(|token| token.len() >= 2)
        .collect();
    let mut best_match: Option<(usize, &IntrospectSource)> = None;

    for source in sources {
        let mut score = 0usize;
        for alias in introspect_source_aliases(source) {
            if alias.contains(&normalized_target) || normalized_target.contains(&alias) {
                score = score.max(80);
            }
            let alias_tokens: Vec<&str> = alias.split_whitespace().collect();
            let overlap = target_tokens
                .iter()
                .filter(|token| alias_tokens.contains(token))
                .count();
            score = score.max(overlap.saturating_mul(10));
        }
        if score > best_match.as_ref().map_or(0, |(best, _)| *best) {
            best_match = Some((score, source));
        }
    }

    if let Some((score, source)) = best_match
        && score >= 10
    {
        let path = validate_introspect_path(&source.path)?;
        return Ok(ResolvedIntrospectTarget {
            label: source.label.to_string(),
            path,
        });
    }

    Err(format!("no INTROSPECT target matched `{cleaned_target}`"))
}

/// Read a source file for introspection with pagination.
///
/// `line_offset`: start reading from this line (0 = beginning).
/// Shows up to 400 lines from the offset. Includes a pagination hint
/// so Astrid can request the next page: `INTROSPECT label next_offset`.
pub(super) fn read_introspect_window(
    label: &str,
    path: &Path,
    line_offset: usize,
) -> Result<IntrospectWindow, String> {
    let canonical = validate_introspect_path(path)?;
    let content =
        fs::read_to_string(&canonical).map_err(|err| format!("target read failed: {err}"))?;
    if text_looks_noisy_or_binary(content.get(..8000).unwrap_or(&content)) {
        return Err("target text looks binary or decoder-noisy".to_string());
    }

    let all_lines: Vec<&str> = content.lines().collect();
    let total = all_lines.len();
    let start = line_offset.min(total);
    let end = start.saturating_add(INTROSPECT_WINDOW_LINES).min(total);
    let page: String = all_lines[start..end]
        .iter()
        .enumerate()
        .map(|(i, line)| format!("{:>4}  {line}", start + i + 1))
        .collect::<Vec<_>>()
        .join("\n");

    let header = format!(
        "// Source: {label} ({})\n// Showing lines {}-{} of {total}\n",
        canonical.display(),
        if total == 0 { 0 } else { start + 1 },
        end
    );

    let (footer, next_offset) = if end < total {
        (
            format!(
                "\n// ... {} more lines. To continue reading: INTROSPECT {} {}",
                total - end,
                label,
                end
            ),
            Some(end),
        )
    } else {
        ("\n// (end of file)".to_string(), None)
    };

    Ok(IntrospectWindow {
        text: format!("{header}{page}{footer}"),
        next_offset,
    })
}

#[must_use]
pub(super) fn introspection_has_required_sections(response: Option<&str>) -> bool {
    let Some(response) = response else {
        return false;
    };
    let body = introspection_body_without_next(response);
    let body = body.trim();
    if body.len() < 160 {
        return false;
    }

    if !REQUIRED_SECTIONS
        .iter()
        .all(|section| body.lines().any(|line| line_matches_section(line, section)))
    {
        return false;
    }

    introspection_is_source_grounded(body) && !introspection_has_peer_boundary_violation(body)
}

#[must_use]
pub(super) fn introspection_has_required_sections_for_target(
    response: Option<&str>,
    label: &str,
    source_path: &Path,
) -> bool {
    if !introspection_has_required_sections(response) {
        return false;
    }
    let Some(response) = response else {
        return false;
    };
    let body = introspection_body_without_next(response);
    introspection_mentions_target(&body, label, source_path)
}

fn introspection_body_without_next(response: &str) -> String {
    response
        .lines()
        .filter(|line| !line.trim_start().to_ascii_uppercase().starts_with("NEXT:"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn introspection_is_source_grounded(body: &str) -> bool {
    let lower = body.to_ascii_lowercase();
    ALLOWED_EXTENSIONS
        .iter()
        .any(|ext| lower.contains(&format!(".{ext}")))
        || lower.contains("source:")
        || lower.contains("path:")
        || lower.contains("function ")
        || lower.contains("variable ")
        || has_numbered_line_reference(&lower)
        || has_source_construct_reference(&lower)
}

fn has_numbered_line_reference(lower: &str) -> bool {
    let mut previous: Option<&str> = None;
    for token in lower.split(|ch: char| !ch.is_ascii_alphanumeric()) {
        if token.is_empty() {
            continue;
        }
        if matches!(previous, Some("line" | "lines"))
            && token.bytes().all(|byte| byte.is_ascii_digit())
        {
            return true;
        }
        if token.len() > 1
            && token.starts_with('l')
            && token[1..].bytes().all(|byte| byte.is_ascii_digit())
        {
            return true;
        }
        previous = Some(token);
    }
    false
}

fn has_source_construct_reference(lower: &str) -> bool {
    lower
        .split(|ch: char| !matches!(ch, '_' | '-') && !ch.is_ascii_alphanumeric())
        .any(|token| matches!(token, "fn" | "def" | "struct" | "enum" | "class"))
}

fn introspection_has_peer_boundary_violation(body: &str) -> bool {
    let upper = body.to_ascii_uppercase();
    if !upper.contains("EXPERIMENT_BIND") {
        return false;
    }
    body.to_ascii_lowercase().contains("exp_minime_")
}

fn introspection_mentions_target(body: &str, label: &str, source_path: &Path) -> bool {
    let lower = body.to_ascii_lowercase();
    let mut targets = Vec::new();
    if let Some(name) = source_path.file_name().and_then(std::ffi::OsStr::to_str) {
        targets.push(name.to_ascii_lowercase());
    }
    if let Some(stem) = source_path.file_stem().and_then(std::ffi::OsStr::to_str)
        && stem.len() >= 4
    {
        targets.push(stem.to_ascii_lowercase());
    }
    let label_lower = label.to_ascii_lowercase();
    targets.push(label_lower.clone());
    targets.extend(
        label_lower
            .split(|ch: char| !matches!(ch, '_' | '-') && !ch.is_ascii_alphanumeric())
            .filter(|token| token.len() >= 4)
            .map(ToString::to_string),
    );
    targets
        .into_iter()
        .filter(|target| !target.trim().is_empty())
        .any(|target| lower.contains(&target))
}

fn line_matches_section(line: &str, section: &str) -> bool {
    let cleaned = line.trim().trim_start_matches('#').trim();
    let lower = cleaned.to_ascii_lowercase();
    lower == section
        || lower.starts_with(&format!("{section}:"))
        || lower.starts_with(&format!("{section} -"))
        || lower.starts_with(&format!("{section}-"))
}

#[must_use]
pub(super) fn continuation_note(label: &str, next_offset: Option<usize>) -> String {
    next_offset.map_or_else(
        || "This window reaches the end of the file.".to_string(),
        |offset| format!("Continuation available: write NEXT: INTROSPECT {label} {offset} to read the next window."),
    )
}

#[must_use]
pub(super) fn safe_artifact_label(label: &str) -> String {
    let safe = label
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.') {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>();
    let trimmed = safe.trim_matches('_');
    if trimmed.is_empty() {
        "target".to_string()
    } else {
        trimmed.to_string()
    }
}

#[must_use]
pub(super) fn thin_introspection_output_notice(
    label: &str,
    source_path: &Path,
    line_offset: usize,
    next_offset: Option<usize>,
    first_response: Option<&str>,
    repair_response: Option<&str>,
) -> String {
    let continuation = next_offset.map_or_else(
        || "(end of file)".to_string(),
        |offset| format!("NEXT: INTROSPECT {label} {offset}"),
    );
    format!(
        "Observed:\nINTROSPECT successfully read `{label}` from `{}` at offset {line_offset}, but the reflective output did not provide the required target-grounded snag/test shape after one repair prompt.\n\nLikely Snags:\n- The model may have treated the continuation hint as the whole task instead of reading the source window.\n- The answer may have omitted a concrete file, line, function, variable, or artifact anchor.\n- The answer may have drifted to a nearby experiment instead of naming `{label}` or the requested source file.\n- The answer may have treated a peer experiment ID as something Astrid can bind or mutate locally.\n- The artifact is protected so this thin answer is visible as a diagnostic, not mistaken for completed self-study.\n\nOne Test Each:\n- Mock a continuation-only response and assert one stricter repair prompt is attempted.\n- Mock two thin responses and assert the final artifact kind is `thin_introspection_output`.\n- Mock a sectioned but source-ungrounded response and assert it is repaired or protected.\n- Mock a sectioned response grounded in the wrong file and assert target-specific review rejects it.\n- Mock `EXPERIMENT_BIND exp_minime_* :: ...` in Suggested Next and assert strict review rejects it.\n\nSuggested Next:\nRetry with a narrower source target, or use `EXPERIMENT_STATUS <peer-id>` / `EXPERIMENT_PEER_REVIEW <peer-id>` for peer experiment references: {continuation}\n\nFirst output:\n```\n{}\n```\n\nRepair output:\n```\n{}\n```",
        source_path.display(),
        first_response.unwrap_or("(empty)"),
        repair_response.unwrap_or("(empty)")
    )
}

#[must_use]
pub(super) fn blocked_introspection_notice(target: Option<&str>, reason: &str) -> String {
    if target.is_some_and(is_placeholder_introspect_target) {
        return format!(
            "Observed:\nINTROSPECT received `{}` as a literal placeholder: {reason}. No source was read, and no runtime control authority was granted.\n\nLikely Snags:\n- The prompt or help text copied bracketed syntax literally instead of choosing a concrete source.\n- `source`, `line`, `path`, `target`, and `label` are syntax placeholders, not readable targets.\n- The request needs a curated label or approved text path before source can enter LLM context.\n\nOne Test Each:\n- Attempt `NEXT: INTROSPECT [source]` and assert the protected notice names the placeholder repair.\n- Attempt `NEXT: INTROSPECT astrid:llm` and assert the source window remains available.\n- Attempt `NEXT: INTROSPECT minime:regulator 400` and assert pagination reads a concrete target.\n\nSuggested Next:\nUse a concrete target, for example:\n- NEXT: INTROSPECT astrid:llm\n- NEXT: INTROSPECT minime:regulator\n- NEXT: INTROSPECT capsules/spectral-bridge/src/autonomous/introspect.rs 400",
            target.unwrap_or("[source]")
        );
    }

    format!(
        "Observed:\nINTROSPECT could not read `{}`: {reason}. The request stayed read-only and no runtime control authority was granted.\n\nLikely Snags:\n- The target may be outside the approved source or memory roots.\n- The file may be too large, binary/noisy, unreadable, or under a blocked build/cache directory.\n- The target may need a curated label such as `astrid:autonomous`, `astrid:codec`, `minime:esn`, or `minime:autonomous_agent`.\n\nOne Test Each:\n- Attempt an out-of-scope absolute path and assert it produces a protected notice.\n- Attempt a binary or disallowed extension and assert it is blocked before LLM context.\n- Attempt a curated source label and assert the source window remains available.\n\nSuggested Next:\nUse a curated label or an approved text artifact under inbox/read, outbox/delivered, journal, research, or action_threads.",
        target.unwrap_or("rotation")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn required_sections_rejects_next_only() {
        assert!(!introspection_has_required_sections(Some(
            "NEXT: INTROSPECT autonomous 400"
        )));
    }

    #[test]
    fn required_sections_accepts_sectioned_output() {
        let output = "Observed:\nA concrete observation with line 12 and a real source path.\n\nLikely Snags:\n- A snag that names a function and behavior.\n\nOne Test Each:\n- A test that asserts the route.\n\nSuggested Next:\nKeep the investigation read-only and continue carefully.";
        assert!(introspection_has_required_sections(Some(output)));
    }

    #[test]
    fn required_sections_rejects_ungrounded_output() {
        let output = "Observed:\nThe reflection notices something broad about attention and continuity.\n\nLikely Snags:\n- The system may drift into general claims without a concrete implementation anchor.\n\nOne Test Each:\n- Add a test that proves strict review needs a real source anchor before it is accepted.\n\nSuggested Next:\nKeep the investigation read-only and continue carefully.";
        assert!(!introspection_has_required_sections(Some(output)));
    }

    #[test]
    fn required_sections_rejects_peer_experiment_bind() {
        let output = "Observed:\nLine 42 in action_continuity.rs shows a peer experiment selector entering the review path.\n\nLikely Snags:\n- A strict review could accidentally suggest binding a Minime experiment from Astrid.\n\nOne Test Each:\n- Assert that peer experiment IDs are advisory refs, not local bind targets.\n\nSuggested Next:\nEXPERIMENT_BIND exp_minime_20990101_peer-thread :: THREAD_STATUS current";
        assert!(!introspection_has_required_sections(Some(output)));
    }

    #[test]
    fn required_sections_allows_peer_experiment_status_reference() {
        let output = "Observed:\nLine 42 in experiment_continuity.rs keeps peer experiment IDs advisory.\n\nLikely Snags:\n- Review language may still confuse status lookup with local mutation.\n\nOne Test Each:\n- Assert peer status review renders a protected advisory notice.\n\nSuggested Next:\nEXPERIMENT_STATUS exp_minime_20990101_peer-thread";
        assert!(introspection_has_required_sections(Some(output)));
    }

    #[test]
    fn required_sections_for_target_rejects_wrong_source_anchor() {
        let output = "Observed:\nLine 42 in experiment_continuity.rs keeps peer experiment IDs advisory.\n\nLikely Snags:\n- The review can drift to a nearby experiment instead of the requested target.\n\nOne Test Each:\n- Assert target-grounded review names the requested file.\n\nSuggested Next:\nEXPERIMENT_STATUS exp_minime_20990101_peer-thread";
        assert!(!introspection_has_required_sections_for_target(
            Some(output),
            "introspect.rs",
            Path::new("/tmp/src/autonomous/introspect.rs"),
        ));
    }

    #[test]
    fn required_sections_for_target_accepts_requested_file_anchor() {
        let output = "Observed:\nLine 42 in introspect.rs keeps peer experiment IDs advisory.\n\nLikely Snags:\n- The validator may accept source-grounded but target-drifting prose.\n\nOne Test Each:\n- Assert target-grounded review names introspect.rs before acceptance.\n\nSuggested Next:\nEXPERIMENT_STATUS exp_minime_20990101_peer-thread";
        assert!(introspection_has_required_sections_for_target(
            Some(output),
            "introspect.rs",
            Path::new("/tmp/src/autonomous/introspect.rs"),
        ));
    }

    #[test]
    fn safe_label_replaces_path_punctuation() {
        assert_eq!(
            safe_artifact_label("astrid:autonomous/mod.rs"),
            "astrid_autonomous_mod.rs"
        );
    }

    #[test]
    fn introspect_placeholder_target_is_rejected_with_guidance() {
        let sources = introspect_sources();
        let err = resolve_introspect_target_result("[source]", &sources)
            .expect_err("literal placeholder should not resolve");

        assert!(err.contains("syntax placeholder"));
        assert!(err.contains("astrid:llm"));
        assert!(err.contains("minime:regulator"));
    }

    #[test]
    fn introspect_placeholder_notice_names_concrete_examples() {
        let notice = blocked_introspection_notice(
            Some("[source]"),
            "`[source]` is a syntax placeholder, not an INTROSPECT target",
        );

        assert!(notice.contains("literal placeholder"));
        assert!(notice.contains("NEXT: INTROSPECT astrid:llm"));
        assert!(notice.contains("NEXT: INTROSPECT minime:regulator"));
        assert!(!notice.contains("target may be outside the approved source"));
    }
}
