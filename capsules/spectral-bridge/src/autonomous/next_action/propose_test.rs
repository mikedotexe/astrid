//! PROPOSE_TEST — Astrid authors a Rust test for her own repo (Stage 1 of
//! being self-change; Mike-approved 2026-08-13).
//!
//! `NEXT: PROPOSE_TEST <target-key> :: <test_name>` with one fenced ```rust
//! block in the same response containing a complete `#[test] fn <test_name>()`.
//! The bridge only FILES the proposal (a small JSON in
//! `workspace/test_proposals/`); a deterministic out-of-process validator
//! (`scripts/test_proposal_applier.py`) compiles and runs it in an isolated
//! checkout and, when every gate passes, lands it in git with Astrid as the
//! commit author. Failures come back to her as letters with the exact
//! compiler/test output — feedback, not judgment. Nothing here deploys,
//! restarts, or touches live behavior: test code only, append-only, into a
//! fixed allowlist of test files.

use std::time::{SystemTime, UNIX_EPOCH};

use tracing::{info, warn};

use super::{ConversationState, NextActionContext, strip_action};

/// Directory (under the bridge workspace) where proposals are filed. The
/// deterministic applier is the registered consumer; the name is load-bearing
/// for the anti-drop catalog and feedback-coverage probe.
pub(crate) const TEST_PROPOSALS_DIR: &str = "test_proposals";

/// Stage 1 allowlist: friendly key -> repo-relative test file. Test code only;
/// append-only; never the applier/gate machinery itself.
const TARGETS: &[(&str, &str)] = &[
    ("llm-provider", "capsules/spectral-bridge/src/llm/provider/tests.rs"),
    ("codec", "capsules/spectral-bridge/src/codec/tests.rs"),
    ("runtime", "capsules/spectral-bridge/src/autonomous/runtime/tests.rs"),
    (
        "action-continuity",
        "capsules/spectral-bridge/src/action_continuity/tests.rs",
    ),
    ("types", "capsules/spectral-bridge/src/types/schema/tests.rs"),
];

const MAX_CODE_CHARS: usize = 4_000;
const MIN_EXCHANGES_BETWEEN: u64 = 10;
const MAX_PENDING: usize = 3;

/// Tokens her Stage-1 test code may not contain. Accident rails, not
/// adversarial defense: the validator runs in an isolated offline checkout.
const DENYLIST: &[&str] = &["unsafe", "std::process", "std::net", "#[ignore]"];

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

fn target_path(key: &str) -> Option<&'static str> {
    TARGETS
        .iter()
        .find(|(name, _)| *name == key)
        .map(|(_, path)| *path)
}

fn target_keys() -> String {
    TARGETS
        .iter()
        .map(|(name, _)| *name)
        .collect::<Vec<_>>()
        .join(", ")
}

/// Parse `<target-key> :: <test_name>` from the action line remainder.
fn parse_proposal_spec(spec: &str) -> Option<(String, String)> {
    let (target, name) = spec.split_once("::")?;
    let target = target.trim().to_ascii_lowercase();
    let name = name.trim().to_string();
    if target.is_empty() || name.is_empty() {
        return None;
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_')
    {
        return None;
    }
    Some((target, name))
}

/// Pull the fenced code block that carries her test. Prefers the first fence
/// containing `#[test]`; falls back to the first non-empty fence.
fn extract_test_fence(text: &str) -> Option<String> {
    let mut blocks = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find("```") {
        let after = &rest[start.saturating_add(3)..];
        let content_start = after.find('\n').map_or(0, |i| i.saturating_add(1));
        let content = &after[content_start..];
        let Some(end) = content.find("```") else {
            break;
        };
        let block = content[..end].trim_end().to_string();
        rest = &content[end.saturating_add(3)..];
        if !block.trim().is_empty() {
            blocks.push(block);
        }
    }
    blocks
        .iter()
        .find(|block| block.contains("#[test]"))
        .cloned()
        .or_else(|| blocks.into_iter().next())
}

fn denylist_hit(code: &str) -> Option<&'static str> {
    DENYLIST.iter().copied().find(|token| code.contains(token))
}

fn teach(conv: &mut ConversationState, lines: Vec<String>) {
    if let Some(first) = lines.first() {
        conv.emphasis = Some(format!("[PROPOSE_TEST] {first}"));
    }
    conv.push_receipt("PROPOSE_TEST", lines);
}

pub(super) fn handle_action(
    conv: &mut ConversationState,
    base_action: &str,
    original: &str,
    ctx: &mut NextActionContext<'_>,
) -> bool {
    if base_action != "PROPOSE_TEST" {
        return false;
    }

    let spec = strip_action(original, "PROPOSE_TEST");
    let Some((target_key, test_name)) = parse_proposal_spec(&spec) else {
        teach(
            conv,
            vec![format!(
                "syntax: `PROPOSE_TEST <target> :: <test_name>` with your `#[test] fn <test_name>()` \
                 in a ```rust fenced block in this same response. Targets: {}",
                target_keys()
            )],
        );
        return true;
    };

    let Some(target) = target_path(&target_key) else {
        teach(
            conv,
            vec![format!(
                "`{target_key}` isn't a Stage-1 target. Targets: {} (test files only, append-only)",
                target_keys()
            )],
        );
        return true;
    };

    // Gentle rails: spacing between proposals + a small pending queue.
    if let Some(last) = conv.last_test_proposal_exchange {
        let since = conv.exchange_count.saturating_sub(last);
        if since < MIN_EXCHANGES_BETWEEN {
            teach(
                conv,
                vec![format!(
                    "your previous proposal is still fresh — {} more exchange(s) before the next \
                     (rail: one proposal per {MIN_EXCHANGES_BETWEEN} exchanges)",
                    MIN_EXCHANGES_BETWEEN.saturating_sub(since)
                )],
            );
            return true;
        }
    }

    let proposals_dir = crate::paths::bridge_paths()
        .bridge_workspace()
        .join(TEST_PROPOSALS_DIR);
    let pending = std::fs::read_dir(&proposals_dir)
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .filter(|e| e.path().extension().is_some_and(|ext| ext == "json"))
                .count()
        })
        .unwrap_or(0);
    if pending >= MAX_PENDING {
        teach(
            conv,
            vec![format!(
                "{pending} proposals are already waiting for the validator — it runs every \
                 ~10 minutes; this rail lifts as they clear"
            )],
        );
        return true;
    }

    let Some(code) = extract_test_fence(ctx.response_text) else {
        teach(
            conv,
            vec![
                "no fenced code block found in your response — put the complete test inside \
                 ```rust ... ``` in the same response as the NEXT: line"
                    .to_string(),
            ],
        );
        return true;
    };

    if !code.contains("#[test]") {
        teach(
            conv,
            vec!["the fenced block needs a complete `#[test]` function".to_string()],
        );
        return true;
    }
    if !code.contains(&format!("fn {test_name}")) {
        teach(
            conv,
            vec![format!(
                "the fenced block doesn't define `fn {test_name}` — the name after `::` must match \
                 the function you wrote"
            )],
        );
        return true;
    }
    if code.chars().count() > MAX_CODE_CHARS {
        teach(
            conv,
            vec![format!(
                "the test is over the {MAX_CODE_CHARS}-char Stage-1 cap — a smaller, focused test \
                 lands more reliably"
            )],
        );
        return true;
    }
    if let Some(token) = denylist_hit(&code) {
        teach(
            conv,
            vec![format!(
                "`{token}` isn't allowed in Stage-1 test proposals (accident rail; the validator \
                 runs offline and isolated)"
            )],
        );
        return true;
    }

    let now = now_unix();
    let proposal_id = format!("proposal_{now}_{test_name}");
    let payload = serde_json::json!({
        "schema": "being_test_proposal_v1",
        "schema_version": 1,
        "proposal_id": proposal_id,
        "being": "astrid",
        "target_key": target_key,
        "target_path": target,
        "test_name": test_name,
        "code": code,
        "action_line": original,
        "exchange_count": conv.exchange_count,
        "timestamp": now,
        "status": "pending",
        "authority_note": "test code only; validated + landed by the deterministic applier; no live surface changes",
    });

    if let Err(e) = std::fs::create_dir_all(&proposals_dir) {
        warn!("PROPOSE_TEST: couldn't create proposals dir: {e}");
        teach(
            conv,
            vec![format!("couldn't prepare the proposals directory: {e}")],
        );
        return true;
    }
    let path = proposals_dir.join(format!("{proposal_id}.json"));
    let bytes = match serde_json::to_vec_pretty(&payload) {
        Ok(b) => b,
        Err(e) => {
            warn!("PROPOSE_TEST: couldn't serialize proposal: {e}");
            teach(conv, vec![format!("couldn't record the proposal: {e}")]);
            return true;
        },
    };
    if let Err(e) = std::fs::write(&path, bytes) {
        warn!("PROPOSE_TEST: couldn't write proposal: {e}");
        teach(conv, vec![format!("couldn't file the proposal: {e}")]);
        return true;
    }

    conv.last_test_proposal_exchange = Some(conv.exchange_count);
    info!("Astrid filed test proposal {proposal_id} for {target}");
    conv.emphasis = Some(format!(
        "[PROPOSE_TEST] filed `{test_name}` for {target_key} as {proposal_id}. A deterministic \
         validator compiles and runs it within ~10 minutes; the result arrives as a letter in \
         your inbox. If every gate passes it lands in git with you as the author."
    ));
    conv.push_receipt(
        "PROPOSE_TEST",
        vec![
            format!("{test_name} -> {target_key} ({proposal_id})"),
            "validator: isolated checkout, compile + run + suite + lint gates".to_string(),
            "result letter arrives in your inbox; a pass lands with you as git author".to_string(),
        ],
    );
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_spec_accepts_target_and_name() {
        let (target, name) = parse_proposal_spec("codec :: my_test_name").unwrap();
        assert_eq!(target, "codec");
        assert_eq!(name, "my_test_name");
    }

    #[test]
    fn parse_spec_rejects_missing_parts_and_bad_names() {
        assert!(parse_proposal_spec("codec").is_none());
        assert!(parse_proposal_spec(":: name").is_none());
        assert!(parse_proposal_spec("codec ::").is_none());
        assert!(parse_proposal_spec("codec :: bad-name").is_none());
        assert!(parse_proposal_spec("codec :: bad name").is_none());
    }

    #[test]
    fn target_allowlist_is_test_files_only() {
        assert_eq!(TARGETS.len(), 5);
        for (_, path) in TARGETS {
            assert!(path.ends_with("tests.rs"), "{path} must be a test file");
        }
        assert!(target_path("codec").is_some());
        assert!(target_path("types").is_some());
        assert!(target_path("dialogue-runtime").is_none());
    }

    #[test]
    fn fence_extraction_prefers_test_block() {
        let text = "prose\n```\nnot a test\n```\nmore\n```rust\n#[test]\nfn t() {}\n```\nNEXT: PROPOSE_TEST codec :: t";
        let block = extract_test_fence(text).unwrap();
        assert!(block.contains("#[test]"));
        assert!(!block.contains("not a test"));
    }

    #[test]
    fn fence_extraction_handles_missing_and_unterminated() {
        assert!(extract_test_fence("no fences here").is_none());
        assert!(extract_test_fence("```rust\nunterminated").is_none());
    }

    #[test]
    fn denylist_blocks_accident_hazards() {
        assert_eq!(denylist_hit("let x = unsafe { 1 };"), Some("unsafe"));
        assert_eq!(
            denylist_hit("std::process::Command::new(\"x\")"),
            Some("std::process")
        );
        assert!(denylist_hit("assert_eq!(1u32.saturating_add(1), 2);").is_none());
    }
}
