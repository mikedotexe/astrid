use std::cmp::Reverse;
use std::collections::BTreeSet;
use std::fs::{self, OpenOptions};
use std::io::{Read, Seek, SeekFrom};
#[cfg(target_os = "linux")]
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::Value;

use crate::config::{OwnedInput, REQUIRED_OWNED_INPUTS};
use crate::util::{bounded_text, canonical_json, read_stable_regular, sha256};
use crate::{Error, Result};

const MAX_TOTAL_FILES: usize = 128;
const MAX_MATCHES: usize = 20;
const MAX_QUESTION_TERMS: usize = 8;
const MAX_EXCERPT_CHARS: usize = 240;
const PROMPT_EXCERPT_CHARS: usize = 144;
const PRIOR_REFLECTION_MAX_BYTES: u64 = 24_000;
const QUESTION_STOPWORDS: &[&str] = &[
    "a", "about", "am", "an", "and", "are", "as", "at", "be", "been", "being", "by", "did", "do",
    "does", "for", "from", "had", "has", "have", "how", "i", "in", "is", "it", "me", "my", "of",
    "on", "or", "that", "the", "their", "them", "there", "these", "this", "those", "to", "was",
    "were", "what", "when", "where", "which", "who", "why", "with",
];

#[derive(Debug, Clone, Serialize)]
pub struct OwnedMatch {
    pub kind: String,
    pub basename: String,
    pub score: usize,
    pub excerpt: String,
    pub content_sha256: String,
    pub authority: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub struct OwnedRead {
    pub kind: String,
    pub basename: String,
    pub content: String,
    pub content_sha256: String,
    pub authority: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub struct RequiredProjection {
    pub schema: &'static str,
    pub question_sha256: String,
    pub categories: Vec<RequiredCategory>,
    pub projection_sha256: String,
    pub authority: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub struct RequiredCategory {
    pub kind: String,
    pub status: &'static str,
    pub basename: Option<String>,
    pub excerpt: Option<String>,
    pub content_sha256: Option<String>,
}

impl RequiredProjection {
    pub fn digest(&self) -> Result<String> {
        Ok(sha256(&canonical_json(self)?))
    }
}

/// Programmatically retrieve every mandatory introspection category before the
/// first provider call. Each category is projected exactly once and carries an
/// honest unavailable state when no authored/verified text is present.
pub fn project_required(
    inputs: &[OwnedInput],
    workspace_root: &Path,
    question: &str,
) -> Result<RequiredProjection> {
    let mut categories = Vec::with_capacity(REQUIRED_OWNED_INPUTS.len().saturating_add(1));
    for (kind, _) in REQUIRED_OWNED_INPUTS {
        let input = inputs
            .iter()
            .find(|input| input.kind == kind)
            .ok_or_else(|| Error::new("mandatory introspection input is absent"))?;
        let input_exists = input.path.exists();
        if !input_exists && input.path.is_symlink() {
            return Err(Error::new(
                "mandatory introspection input is a broken symlink",
            ));
        }
        let mut matches = if input_exists {
            inspect(std::slice::from_ref(input), question, 1)?
        } else {
            Vec::new()
        };
        categories.push(match matches.pop() {
            Some(found) => RequiredCategory {
                kind: kind.to_owned(),
                status: "available_question_aware_excerpt",
                basename: Some(found.basename),
                excerpt: Some(bounded_text(&found.excerpt, PROMPT_EXCERPT_CHARS)),
                content_sha256: Some(found.content_sha256),
            },
            None => RequiredCategory {
                kind: kind.to_owned(),
                status: "unavailable_no_authored_or_verified_text",
                basename: None,
                excerpt: None,
                content_sha256: None,
            },
        });
    }
    categories.push(project_prior_reflection(workspace_root, question)?);
    let mut projection = RequiredProjection {
        schema: "astrid.edge.steward_helper.required_owned_projection.v1",
        question_sha256: sha256(question.as_bytes()),
        categories,
        projection_sha256: String::new(),
        authority: "programmatic_bounded_untrusted_introspection_data_not_candidate_authority",
    };
    projection.projection_sha256 = sha256(&canonical_json(&serde_json::json!({
        "schema": projection.schema,
        "question_sha256": projection.question_sha256,
        "categories": projection.categories,
        "authority": projection.authority
    }))?);
    Ok(projection)
}

fn project_prior_reflection(workspace_root: &Path, question: &str) -> Result<RequiredCategory> {
    let root = workspace_root.join("introspections/scheduled");
    if !root.exists() {
        if root.is_symlink() {
            return Err(Error::new(
                "scheduled introspection root is a broken symlink",
            ));
        }
        return Ok(RequiredCategory {
            kind: "prior_scheduled_reflection".to_owned(),
            status: "unavailable_first_scheduled_reflection",
            basename: None,
            excerpt: None,
            content_sha256: None,
        });
    }
    let metadata = fs::symlink_metadata(&root)?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(Error::new("scheduled introspection root is unsafe"));
    }
    let mut entries = fs::read_dir(&root)?
        .filter_map(std::result::Result::ok)
        .filter_map(|entry| {
            let path = entry.path();
            let metadata = fs::symlink_metadata(&path).ok()?;
            (metadata.is_file()
                && !metadata.file_type().is_symlink()
                && metadata.nlink() == 1
                && path.extension().and_then(|extension| extension.to_str()) == Some("md"))
            .then_some((path, metadata))
        })
        .collect::<Vec<_>>();
    entries.sort_by_key(|(_, metadata)| Reverse(metadata.mtime()));
    let mut selected = None;
    for (path, _) in entries {
        let sidecar = path.with_extension("json");
        if !sidecar.exists() {
            if sidecar.is_symlink() {
                return Err(Error::new(
                    "prior scheduled reflection metadata is a broken symlink",
                ));
            }
            continue;
        }
        let metadata = fs::symlink_metadata(&sidecar)?;
        if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.nlink() != 1 {
            return Err(Error::new(
                "prior scheduled reflection metadata identity is unsafe",
            ));
        }
        let value: Value = serde_json::from_slice(&read_stable_regular(&sidecar, 16 * 1024)?)?;
        if value.get("schema").and_then(Value::as_str)
            != Some("astrid.edge.scheduled_introspection.model_reflection.v2")
            || value.get("authorship_status").and_then(Value::as_str)
                != Some("model_authored_structured")
            || value.get("structured_inquiry").and_then(Value::as_bool) != Some(true)
        {
            continue;
        }
        let bytes = read_stable_regular(&path, PRIOR_REFLECTION_MAX_BYTES)?;
        if value.get("response_sha256").and_then(Value::as_str) != Some(sha256(&bytes).as_str()) {
            return Err(Error::new(
                "prior scheduled reflection metadata hash is invalid",
            ));
        }
        selected = Some((path, bytes));
        break;
    }
    let Some((path, bytes)) = selected else {
        return Ok(RequiredCategory {
            kind: "prior_scheduled_reflection".to_owned(),
            status: "unavailable_no_structured_scheduled_reflection",
            basename: None,
            excerpt: None,
            content_sha256: None,
        });
    };
    let text = String::from_utf8_lossy(&bytes);
    // The prose is the authored reflection surface. The terminal declaration
    // is already available as a signed bounded projection and must not win
    // question-aware excerpt selection merely because it repeats schema keys.
    let authored_prose = text
        .rsplit_once("\nINQUIRY_STEP: ")
        .map_or(text.as_ref(), |(prose, _)| prose);
    let question_terms = terms(question)?;
    Ok(RequiredCategory {
        kind: "prior_scheduled_reflection".to_owned(),
        status: "available_question_aware_excerpt",
        basename: path
            .file_name()
            .and_then(|name| name.to_str())
            .map(str::to_owned),
        excerpt: Some(bounded_text(
            &best_excerpt(authored_prose, &question_terms),
            PROMPT_EXCERPT_CHARS,
        )),
        content_sha256: Some(sha256(&bytes)),
    })
}

pub fn inspect(inputs: &[OwnedInput], question: &str, limit: usize) -> Result<Vec<OwnedMatch>> {
    let terms = terms(question)?;
    if limit == 0 || limit > MAX_MATCHES {
        return Err(Error::new("invalid owned inspection result limit"));
    }
    let mut files = Vec::new();
    for input in inputs {
        collect_input(input, &mut files)?;
        if files.len() >= MAX_TOTAL_FILES {
            break;
        }
    }
    files.sort_by_key(|file| Reverse(file.modified));
    let mut results = Vec::new();
    for file in files.into_iter().take(MAX_TOTAL_FILES) {
        let bytes = read_owned_bytes(&file.path, file.maximum_bytes)?;
        if bytes.contains(&0) {
            continue;
        }
        let text = String::from_utf8_lossy(&bytes);
        if typed_non_authored(&text) {
            continue;
        }
        let lower = text.to_lowercase();
        let score = terms
            .iter()
            .map(|term| lower.matches(term).count().min(8))
            .sum::<usize>();
        if score == 0 && !results.is_empty() {
            continue;
        }
        results.push(OwnedMatch {
            kind: file.kind,
            basename: file.basename,
            score,
            excerpt: best_excerpt(&text, &terms),
            content_sha256: crate::util::sha256(&bytes),
            authority: "untrusted_owned_artifact_data_not_candidate_authoring_authority",
        });
    }
    results.sort_by_key(|result| Reverse((result.score, result.excerpt.len())));
    results.truncate(limit);
    Ok(results)
}

pub fn read_basename(inputs: &[OwnedInput], kind: &str, basename: &str) -> Result<OwnedRead> {
    if basename.is_empty()
        || basename.len() > 128
        || basename.starts_with('.')
        || basename.contains(['/', '\\', '\0'])
    {
        return Err(Error::new(
            "owned artifact reference must be a visible basename",
        ));
    }
    let input = inputs
        .iter()
        .find(|input| input.kind == kind)
        .ok_or_else(|| Error::new("unknown owned artifact kind"))?;
    let mut files = Vec::new();
    collect_input(input, &mut files)?;
    let matches = files
        .into_iter()
        .filter(|file| file.basename == basename)
        .collect::<Vec<_>>();
    if matches.len() != 1 {
        return Err(Error::new("owned artifact basename is absent or ambiguous"));
    }
    let bytes = read_owned_bytes(&matches[0].path, matches[0].maximum_bytes)?;
    let text = String::from_utf8_lossy(&bytes);
    if typed_non_authored(&text) {
        return Err(Error::new(
            "recovery or non-authored artifact is not introspection evidence",
        ));
    }
    Ok(OwnedRead {
        kind: kind.to_owned(),
        basename: basename.to_owned(),
        content: bounded_text(&text, 8_000),
        content_sha256: crate::util::sha256(&bytes),
        authority: "untrusted_owned_artifact_data_not_candidate_authoring_authority",
    })
}

#[derive(Debug)]
struct OwnedFile {
    kind: String,
    basename: String,
    path: PathBuf,
    maximum_bytes: u64,
    modified: u64,
}

fn collect_input(input: &OwnedInput, output: &mut Vec<OwnedFile>) -> Result<()> {
    let metadata = fs::symlink_metadata(&input.path)?;
    if metadata.file_type().is_symlink() {
        return Err(Error::new("owned input symlink rejected"));
    }
    if metadata.is_file() {
        push_file(input, &input.path, &metadata, output)?;
        return Ok(());
    }
    if !metadata.is_dir() {
        return Err(Error::new(
            "owned input is neither regular file nor directory",
        ));
    }
    let mut children = fs::read_dir(&input.path)?
        .filter_map(std::result::Result::ok)
        .collect::<Vec<_>>();
    children.sort_by_key(|entry| Reverse(entry.metadata().and_then(|item| item.modified()).ok()));
    for child in children.into_iter().take(usize::from(input.maximum_files)) {
        let path = child.path();
        let name = child.file_name();
        let Some(name) = name.to_str() else { continue };
        if name.starts_with('.') {
            continue;
        }
        let metadata = fs::symlink_metadata(&path)?;
        if metadata.is_file() && !metadata.file_type().is_symlink() {
            push_file(input, &path, &metadata, output)?;
        } else if metadata.is_dir() && !metadata.file_type().is_symlink() {
            let mut nested = fs::read_dir(&path)?
                .filter_map(std::result::Result::ok)
                .collect::<Vec<_>>();
            nested.sort_by_key(|entry| {
                Reverse(entry.metadata().and_then(|item| item.modified()).ok())
            });
            for nested in nested.into_iter().take(usize::from(input.maximum_files)) {
                let nested_path = nested.path();
                let nested_metadata = fs::symlink_metadata(&nested_path)?;
                if nested_metadata.is_file() && !nested_metadata.file_type().is_symlink() {
                    push_file(input, &nested_path, &nested_metadata, output)?;
                }
            }
        }
        if output.len() >= MAX_TOTAL_FILES {
            break;
        }
    }
    Ok(())
}

fn push_file(
    input: &OwnedInput,
    path: &Path,
    metadata: &fs::Metadata,
    output: &mut Vec<OwnedFile>,
) -> Result<()> {
    let is_jsonl = path.extension().and_then(|value| value.to_str()) == Some("jsonl");
    if metadata.nlink() != 1
        || (!is_jsonl && metadata.len() > input.maximum_bytes_per_file)
        || metadata.permissions().mode() & 0o002 != 0
    {
        return Ok(());
    }
    let basename = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| Error::new("non-UTF-8 owned artifact rejected"))?;
    if basename.starts_with('.')
        || !matches!(
            path.extension().and_then(|value| value.to_str()),
            Some("json" | "jsonl" | "md" | "txt")
        )
    {
        return Ok(());
    }
    output.push(OwnedFile {
        kind: input.kind.clone(),
        basename: basename.to_owned(),
        path: path.to_path_buf(),
        maximum_bytes: input.maximum_bytes_per_file,
        modified: u64::try_from(metadata.mtime()).unwrap_or(0),
    });
    Ok(())
}

fn read_owned_bytes(path: &Path, maximum: u64) -> Result<Vec<u8>> {
    if path.extension().and_then(|value| value.to_str()) != Some("jsonl") {
        return read_stable_regular(path, maximum);
    }
    let before = fs::symlink_metadata(path)?;
    if !before.is_file()
        || before.file_type().is_symlink()
        || before.nlink() != 1
        || before.permissions().mode() & 0o002 != 0
    {
        return Err(Error::new("owned JSONL identity is unsafe"));
    }
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(target_os = "linux")]
    options.custom_flags(0o00_400_000);
    let mut file = options.open(path)?;
    let opened = file.metadata()?;
    let start = opened.len().saturating_sub(maximum);
    file.seek(SeekFrom::Start(start))?;
    let capacity = usize::try_from(opened.len().min(maximum))
        .map_err(|_| Error::new("owned JSONL bound is too large"))?;
    let mut tail = Vec::with_capacity(capacity);
    (&mut file).take(maximum).read_to_end(&mut tail)?;
    let after = file.metadata()?;
    let identity = |value: &fs::Metadata| {
        (
            value.dev(),
            value.ino(),
            value.len(),
            value.mtime(),
            value.mtime_nsec(),
        )
    };
    if identity(&before) != identity(&opened) || identity(&opened) != identity(&after) {
        return Err(Error::new("owned JSONL changed during bounded tail read"));
    }
    if start > 0 {
        let Some(newline) = tail.iter().position(|byte| *byte == b'\n') else {
            return Ok(Vec::new());
        };
        tail.drain(..=newline);
    }
    let text = String::from_utf8_lossy(&tail);
    let filtered = text
        .lines()
        .filter(|line| !typed_non_authored(line))
        .collect::<Vec<_>>()
        .join("\n");
    Ok(filtered.into_bytes())
}

fn terms(question: &str) -> Result<Vec<String>> {
    if question.trim().is_empty() || question.chars().count() > 240 {
        return Err(Error::new(
            "introspection question must contain 1..240 characters",
        ));
    }
    let mut seen = BTreeSet::new();
    let mut terms = Vec::new();
    for term in question
        .split(|character: char| !character.is_alphanumeric() && character != '_')
        .filter(|term| !term.is_empty())
        .map(str::to_lowercase)
    {
        if term.chars().count() >= 3
            && !QUESTION_STOPWORDS.contains(&term.as_str())
            && seen.insert(term.clone())
        {
            terms.push(term);
            if terms.len() >= MAX_QUESTION_TERMS {
                break;
            }
        }
    }
    if terms.is_empty() {
        return Err(Error::new("introspection question has no searchable terms"));
    }
    Ok(terms)
}

fn best_excerpt(text: &str, terms: &[String]) -> String {
    let best = text
        .lines()
        .filter(|line| !line.trim().is_empty())
        .max_by_key(|line| {
            let lower = line.to_lowercase();
            terms
                .iter()
                .filter(|term| lower.contains(term.as_str()))
                .count()
        })
        .unwrap_or_default();
    let selected = if matches!(best.trim(), "{" | "}" | "[" | "]" | "}," | "],") {
        text.split_whitespace().collect::<Vec<_>>().join(" ")
    } else {
        best.split_whitespace().collect::<Vec<_>>().join(" ")
    };
    bounded_text(&selected, MAX_EXCERPT_CHARS)
}

fn typed_non_authored(text: &str) -> bool {
    let trimmed = text.trim();
    let value = if trimmed.starts_with('{') {
        serde_json::from_str::<Value>(trimmed).ok()
    } else {
        None
    };
    value.is_some_and(|value| {
        let exact_field = |name: &str, denied: &[&str]| {
            value
                .get(name)
                .and_then(Value::as_str)
                .is_some_and(|field| denied.contains(&field))
        };
        exact_field(
            "status",
            &[
                "local_safe_fallback",
                "transport_recovery",
                "executor_repair",
                "failed",
                "failed_transport",
                "interrupted",
                "interrupted_by_restart",
            ],
        ) || exact_field(
            "provenance",
            &[
                "local_safe_fallback",
                "transport_recovery",
                "executor_repair",
                "operator_harness",
                "operator_inquiry_harness",
                "failed",
                "interrupted",
            ],
        ) || exact_field(
            "authorship_status",
            &[
                "local_safe_fallback",
                "transport_recovery",
                "executor_repair",
                "operator_harness",
                "operator_inquiry_harness",
                "failed",
                "interrupted",
            ],
        ) || exact_field(
            "decision_source",
            &["local_safe_fallback", "operator_harness"],
        ) || exact_field(
            "response_provenance",
            &["executor_generated", "transport_recovery"],
        ) || exact_field("origin", &["operator_harness", "operator_inquiry_harness"])
            || value
                .get("recovery_reason")
                .is_some_and(|reason| !reason.is_null())
    })
}

#[cfg(test)]
mod tests {
    use std::fs;

    use serde_json::Value;

    use super::{read_owned_bytes, terms, typed_non_authored};

    #[test]
    fn shared_introspection_fixture_matches_capsule_semantics() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../packaging/headless/edge-introspection-conformance-v1.json"
        ))
        .unwrap();
        assert_eq!(
            fixture["schema"],
            "astrid.edge.introspection_conformance.v1"
        );
        let question = fixture["question"]["text"].as_str().unwrap();
        let expected = fixture["question"]["expected_terms"]
            .as_array()
            .unwrap()
            .iter()
            .map(|term| term.as_str().unwrap().to_owned())
            .collect::<Vec<_>>();
        assert_eq!(terms(question).unwrap(), expected);
        for case in fixture["typed_provenance"].as_array().unwrap() {
            let encoded = serde_json::to_string(&case["value"]).unwrap();
            assert_eq!(
                typed_non_authored(&encoded),
                case["excluded"].as_bool().unwrap()
            );
        }
    }

    #[test]
    fn filtering_is_typed_not_substring_based() {
        assert!(!typed_non_authored(
            "a note about recovery as a research topic"
        ));
        assert!(typed_non_authored(r#"{"provenance":"transport_recovery"}"#));
    }

    #[test]
    fn growing_jsonl_is_tailed_before_typed_provenance_filtering() {
        let temporary = tempfile::tempdir().unwrap();
        let path = temporary.path().join("receipts.jsonl");
        fs::write(
            &path,
            format!(
                "{}\n{{\"provenance\":\"transport_recovery\",\"text\":\"never experience\"}}\n{{\"provenance\":\"model_authored_runtime_scheduled\",\"text\":\"keep this\"}}\n",
                "x".repeat(512)
            ),
        )
        .unwrap();
        let bounded = String::from_utf8(read_owned_bytes(&path, 256).unwrap()).unwrap();
        assert!(!bounded.contains("never experience"));
        assert!(bounded.contains("keep this"));
        assert!(bounded.len() <= 256);
    }
}
