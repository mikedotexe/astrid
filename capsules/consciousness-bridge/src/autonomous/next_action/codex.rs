use std::fs;
use std::path::Path;
use tracing::{info, warn};

use super::{ConversationState, NextActionContext, strip_action};
use super::mike::is_safe_path;
use crate::paths::bridge_paths;

const CODEX_RELAY_URL: &str = "http://127.0.0.1:3040/prompt";
const CODEX_TIMEOUT_SECS: u64 = 60;

pub(super) fn handle_action(
    conv: &mut ConversationState,
    base_action: &str,
    original: &str,
    _ctx: &mut NextActionContext<'_>,
) -> bool {
    match base_action {
        "MIKE_FORK" => {
            let arg = strip_action(original, "MIKE_FORK");
            let parts: Vec<&str> = arg.splitn(2, char::is_whitespace).collect();
            let project = parts.first().copied().unwrap_or_default();
            let name = parts.get(1).copied().unwrap_or(project).trim();
            if project.is_empty() {
                conv.emphasis = Some(
                    "MIKE_FORK needs a project. Example: NEXT: MIKE_FORK blockwise my-experiment"
                        .into(),
                );
                return true;
            }
            let src = bridge_paths().mike_research_root().join(project);
            if !src.is_dir() {
                conv.emphasis = Some(format!(
                    "MIKE_FORK: project '{project}' not found. Use NEXT: MIKE to see projects."
                ));
                return true;
            }
            let dst = bridge_paths().experiments_dir().join(name);
            if dst.exists() {
                conv.emphasis = Some(format!(
                    "Fork '{name}' already exists at {}. Use MIKE_RUN {name} <cmd> to work with it, \
                     or choose a different name.",
                    dst.display()
                ));
                return true;
            }
            match copy_dir_recursive(&src, &dst) {
                Ok(count) => {
                    conv.emphasis = Some(format!(
                        "Forked '{project}' → experiments/{name}/ ({count} files). \
                         You can now modify files with WRITE_FILE and run with MIKE_RUN {name} <cmd>."
                    ));
                    info!("MIKE_FORK: {project} → experiments/{name}/ ({count} files)");
                }
                Err(e) => {
                    conv.emphasis =
                        Some(format!("MIKE_FORK failed: {e}"));
                    warn!("MIKE_FORK error: {e}");
                }
            }
            true
        }
        "CODEX" => {
            let arg = strip_action(original, "CODEX");
            if arg.is_empty() {
                conv.emphasis = Some(
                    "CODEX needs a prompt. Examples:\n\
                     NEXT: CODEX \"explain spectral entropy\"\n\
                     NEXT: CODEX my-experiment \"add a metrics function to model.py\""
                        .into(),
                );
                return true;
            }
            // Detect project-scoped mode: first token matches experiments/ dir
            let experiments = bridge_paths().experiments_dir();
            let (dir_context, prompt) = detect_project_prompt(&arg, &experiments);

            info!("CODEX query (dir={dir_context:?}): {}", &prompt[..prompt.len().min(80)]);

            // Build request body
            let mut body = serde_json::json!({
                "from": "astrid",
                "prompt": prompt,
                "effort": "high",
                "no_deliver": true,
            });
            if let Some(ref dir) = dir_context {
                body["dir"] = serde_json::Value::String(dir.clone());
            }
            if let Some(ref thread_id) = conv.codex_thread_id {
                body["thread"] = serde_json::Value::String(thread_id.clone());
            }

            // Synchronous HTTP call via tokio runtime
            let result: Result<serde_json::Value, reqwest::Error> =
                tokio::runtime::Handle::current().block_on(async {
                    let client = reqwest::Client::new();
                    let resp = client
                        .post(CODEX_RELAY_URL)
                        .json(&body)
                        .timeout(std::time::Duration::from_secs(CODEX_TIMEOUT_SECS))
                        .send()
                        .await?;
                    resp.json::<serde_json::Value>().await
                });

            match result {
                Ok(resp) => {
                    let ok = resp.get("ok").and_then(|v| v.as_bool()).unwrap_or(false);
                    if !ok {
                        let err = resp.get("error").and_then(|v| v.as_str()).unwrap_or("unknown");
                        conv.emphasis = Some(format!("CODEX error: {err}"));
                        return true;
                    }
                    // Get response text (from no_deliver mode)
                    let text = resp
                        .get("response_text")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let total = resp
                        .get("total_chars")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0);

                    // Store thread ID if returned
                    if let Some(tid) = resp.get("thread").and_then(|v| v.as_str()) {
                        conv.codex_thread_id = Some(tid.to_string());
                    }

                    // Store full response for WRITE_FILE FROM_CODEX
                    conv.last_codex_response = Some(text.clone());

                    // Save to disk for persistence + READ_MORE pagination
                    let codex_dir = bridge_paths().experiments_dir()
                        .parent()
                        .unwrap_or(bridge_paths().bridge_workspace())
                        .join("codex_responses");
                    let _ = fs::create_dir_all(&codex_dir);
                    let ts = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs();
                    let saved_path = codex_dir.join(format!("codex_{ts}.txt"));
                    let _ = fs::write(&saved_path, &text);

                    // Paginated display with paragraph-boundary breaks
                    const PAGE_SIZE: usize = 6000;
                    if text.len() <= PAGE_SIZE {
                        conv.emphasis = Some(format!(
                            "[Codex response ({total} chars):]\n{text}"
                        ));
                    } else {
                        let break_at = find_paragraph_break(&text, PAGE_SIZE);
                        let total_pages = estimate_pages(text.len(), PAGE_SIZE);
                        conv.emphasis = Some(format!(
                            "[Codex response — part 1 of {total_pages} ({total} chars total):]\n\
                             {}\n\n\
                             [Part 1 of {total_pages}. NEXT: READ_MORE for part 2. \
                             Save complete response: NEXT: WRITE_FILE <path> FROM_CODEX]",
                            &text[..break_at]
                        ));
                        conv.last_read_path = Some(saved_path.to_string_lossy().into());
                        conv.last_read_offset = break_at;
                    }
                    info!("CODEX response: {total} chars");
                }
                Err(e) => {
                    let msg = if e.is_timeout() {
                        "CODEX timed out (60s). The relay may be processing a large request. \
                         Try again or use a simpler prompt."
                            .to_string()
                    } else if e.is_connect() {
                        "CODEX: relay not reachable at localhost:3040. Is it running? \
                         (cd /Users/v/other/ai-use-codex && npm start)"
                            .to_string()
                    } else {
                        format!("CODEX request failed: {e}")
                    };
                    conv.emphasis = Some(msg);
                    warn!("CODEX error: {e}");
                }
            }
            true
        }
        "WRITE_FILE" => {
            let arg = strip_action(original, "WRITE_FILE");
            if arg.is_empty() {
                conv.emphasis = Some(
                    "WRITE_FILE needs a path. Examples:\n\
                     NEXT: WRITE_FILE my-experiment/metrics.py FROM_CODEX\n\
                     NEXT: WRITE_FILE my-experiment/config.toml name = \"test\""
                        .into(),
                );
                return true;
            }
            let (path_str, rest) = arg
                .split_once(char::is_whitespace)
                .unwrap_or((&arg, ""));
            let rest = rest.trim();

            let experiments = bridge_paths().experiments_dir();
            let full_path = experiments.join(path_str);

            if !is_safe_path(&full_path, &experiments) {
                warn!("WRITE_FILE path traversal blocked: {path_str}");
                conv.emphasis = Some("[Path outside experiments/ — blocked.]".into());
                return true;
            }

            let content = if rest.eq_ignore_ascii_case("FROM_CODEX") {
                match conv.last_codex_response.take() {
                    Some(c) => c,
                    None => {
                        conv.emphasis = Some(
                            "WRITE_FILE FROM_CODEX: no Codex response stored. \
                             Use NEXT: CODEX first."
                                .into(),
                        );
                        return true;
                    }
                }
            } else if rest.is_empty() {
                conv.emphasis = Some(
                    "WRITE_FILE needs content. Use FROM_CODEX or provide inline text.".into(),
                );
                return true;
            } else {
                rest.to_string()
            };

            // Create parent dirs and write
            if let Some(parent) = full_path.parent() {
                let _ = fs::create_dir_all(parent);
            }
            match fs::write(&full_path, &content) {
                Ok(()) => {
                    conv.emphasis = Some(format!(
                        "Wrote {} bytes to experiments/{path_str}",
                        content.len()
                    ));
                    info!("WRITE_FILE: experiments/{path_str} ({} bytes)", content.len());
                }
                Err(e) => {
                    conv.emphasis = Some(format!("WRITE_FILE failed: {e}"));
                    warn!("WRITE_FILE error: {e}");
                }
            }
            true
        }
        _ => false,
    }
}

/// Detect if the first token is an existing experiments/ subdirectory.
/// If so, return (Some(dir_path), remaining prompt). Otherwise (None, full text).
fn detect_project_prompt(arg: &str, experiments: &Path) -> (Option<String>, String) {
    let first_token = arg.split(|c: char| c.is_whitespace() || c == '"').next().unwrap_or("");
    if !first_token.is_empty() {
        let candidate = experiments.join(first_token);
        if candidate.is_dir() {
            let prompt = arg[first_token.len()..].trim().trim_matches('"').to_string();
            if !prompt.is_empty() {
                return (Some(candidate.to_string_lossy().into()), prompt);
            }
        }
    }
    (None, arg.trim_matches('"').to_string())
}

/// Recursively copy a directory, skipping excluded entries.
fn copy_dir_recursive(src: &Path, dst: &Path) -> std::io::Result<usize> {
    fs::create_dir_all(dst)?;
    let mut count = 0usize;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if super::mike::is_excluded(&name_str) {
            continue;
        }
        let src_path = entry.path();
        let dst_path = dst.join(&name);
        if src_path.is_dir() {
            count = count.saturating_add(copy_dir_recursive(&src_path, &dst_path)?);
        } else if src_path.is_file() {
            fs::copy(&src_path, &dst_path)?;
            count = count.saturating_add(1);
        }
        // Skip symlinks for safety
    }
    Ok(count)
}

/// Find a paragraph or line break near `target` for cleaner page boundaries.
/// Prefers `\n\n` (paragraph), falls back to `\n` (line), then hard cut.
fn find_paragraph_break(text: &str, target: usize) -> usize {
    // Clamp to char boundaries to avoid panicking on multi-byte UTF-8.
    let target = snap_to_char_boundary(text, target.min(text.len()));
    let search_from = snap_to_char_boundary(text, target.saturating_sub(500).max(target / 2));
    let slice = &text[search_from..target];
    // Prefer paragraph break
    if let Some(pos) = slice.rfind("\n\n") {
        return search_from + pos + 2; // after the double newline
    }
    // Fall back to line break
    if let Some(pos) = slice.rfind('\n') {
        return search_from + pos + 1;
    }
    // Hard cut
    target.min(text.len())
}

fn estimate_pages(total_len: usize, page_size: usize) -> usize {
    (total_len + page_size - 1) / page_size
}

/// Snap a byte index down to the nearest char boundary in a UTF-8 string.
fn snap_to_char_boundary(text: &str, idx: usize) -> usize {
    let mut i = idx.min(text.len());
    while i > 0 && !text.is_char_boundary(i) {
        i -= 1;
    }
    i
}

/// Read the next page from a saved codex response file.
/// Used by READ_MORE when `last_read_path` points to a codex response.
pub(super) fn read_codex_page(
    path: &str,
    offset: usize,
) -> Option<(String, usize, usize, usize)> {
    let content = fs::read_to_string(path).ok()?;
    let offset = snap_to_char_boundary(&content, offset);
    if offset >= content.len() {
        return None;
    }
    const PAGE_SIZE: usize = 6000;
    let break_at = find_paragraph_break(&content, (offset + PAGE_SIZE).min(content.len()));
    let page = &content[offset..break_at];
    let total_pages = estimate_pages(content.len(), PAGE_SIZE);
    let current_page = offset / PAGE_SIZE + 2; // +2 because page 1 was shown by CODEX
    Some((page.to_string(), current_page, total_pages, break_at))
}
