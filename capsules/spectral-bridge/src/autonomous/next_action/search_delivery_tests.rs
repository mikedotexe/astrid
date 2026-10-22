use super::*;
use crate::action_continuity::ResearchWebRequest;
use std::io::Write;
use std::sync::{Arc, Mutex};

#[test]
fn search_delivery_query_is_literal_through_next_classification_and_dispatch() {
    let db = BridgeDb::open(":memory:").unwrap();
    let (sensory_tx, mut rx) = mpsc::channel(1);
    let telemetry = serde_json::from_value(serde_json::json!({
        "t_ms":0, "eigenvalues":[1.0], "fill_ratio":0.68,
    }))
    .unwrap();
    let mut burst = 0;
    for command in ["SEARCH", "search", "RESEARCH", "research"] {
        for query in [
            "\"CONTROL Vec<T>\"  \u{03bb} - exact; & q=other",
            "Vec<T> AND AR_START",
            "-site:example.org \"quoted phrase\"",
            "literal (RESIDUE: stays data)",
            "Vec<T><END_OF_TURN>",
        ] {
            let action = format!("{command} {query}");
            let generated = format!("Synthetic thought.\nNEXT: {action}");
            let parsed = parse_next_action(&generated).unwrap();
            assert_eq!(parsed, action);
            let (base, canonical) = canonicalize_next_action_components(parsed);
            assert_eq!(canonical, action);
            assert_eq!(extract_search_topic(&canonical).as_deref(), Some(query));
            assert_eq!(split_multi_action(&canonical), vec![action.clone()]);
            assert_eq!(
                action_continuity::research_web_request(&canonical).unwrap(),
                Some(ResearchWebRequest::Search(query.to_string()))
            );
            assert!(action_continuity::research_binding_requires_budget(&canonical).unwrap());
            let mut conv = ConversationState::new(Vec::new(), None);
            let mut context = NextActionContext {
                operation_id: None,
                burst_count: &mut burst,
                db: &db,
                sensory_tx: &sensory_tx,
                telemetry: &telemetry,
                fill_pct: 68.0,
                response_text: "",
                workspace: None,
            };
            assert!(workspace::handle_action(
                &mut conv,
                &base,
                &canonical,
                parsed,
                &mut context
            ));
            assert!(conv.wants_search);
            assert_eq!(conv.search_topic.as_deref(), Some(query));
            assert!(rx.try_recv().is_err());
        }
    }
}

#[test]
fn search_delivery_keeps_limits_and_command_classification_outside_literal_cleanup() {
    // Transport still owns its byte/character/word limits; dispatch must never
    // truncate an over-limit query into a different, accepted request.
    for length in [600, 601, 2048, 2049, 8192, 8193] {
        let query = "x".repeat(length);
        assert_eq!(
            extract_search_topic(&format!("SEARCH {query}")),
            Some(query)
        );
    }
    for action in [
        "SEARCHED x",
        "SEARCH_CONTROL x",
        "RESEARCH_AR_LIST x",
        "SEARCH",
        "SEARCH:",
        "\u{03bb}\u{03bb}\u{03bb}\u{03bb} x",
    ] {
        assert!(extract_search_topic(action).is_none(), "{action}");
    }
    for action in [
        "SEARCH query\nCONTROL_APPLY_LEASE x",
        "RESEARCH query\rREST",
        "SEARCH a\0b",
    ] {
        assert!(action_continuity::research_binding_requires_budget(action).is_err());
    }
    assert!(action_continuity::research_binding_requires_budget("CONTROL_APPLY_LEASE x").is_err());
}

#[derive(Clone)]
struct Capture(Arc<Mutex<Vec<u8>>>);

impl Write for Capture {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[test]
fn search_delivery_dispatch_logs_only_target_fingerprint() {
    let output = Capture(Arc::default());
    let writer = output.clone();
    let subscriber = tracing_subscriber::fmt()
        .without_time()
        .with_ansi(false)
        .with_writer(move || writer.clone())
        .finish();
    tracing::subscriber::with_default(subscriber, || {
        let mut conv = ConversationState::new(Vec::new(), None);
        let db = BridgeDb::open(":memory:").unwrap();
        let (sensory_tx, _rx) = mpsc::channel(1);
        let telemetry = serde_json::from_value(serde_json::json!({
            "t_ms":0, "eigenvalues":[1.0], "fill_ratio":0.68,
        }))
        .unwrap();
        let mut burst = 0;
        let mut context = NextActionContext {
            operation_id: None,
            burst_count: &mut burst,
            db: &db,
            sensory_tx: &sensory_tx,
            telemetry: &telemetry,
            fill_pct: 68.0,
            response_text: "",
            workspace: None,
        };
        assert!(workspace::handle_action(
            &mut conv,
            "SEARCH",
            "SEARCH PRIVATE_QUERY",
            "SEARCH PRIVATE_QUERY",
            &mut context
        ));
    });
    let text = String::from_utf8(output.0.lock().unwrap().clone()).unwrap();
    assert!(text.contains("target_hash="));
    assert!(!text.contains("PRIVATE_QUERY"));
}

#[test]
fn search_delivery_empty_success_exits_before_generation_or_source_mutation() {
    let source = include_str!("../runtime/orchestration.rs");
    let (_, search) = source
        .split_once("let ctx = crate::llm::web_search_checked(&query, &anchor).await;")
        .unwrap();
    let (empty, _) = search.split_once("if let Ok(ref results) = ctx").unwrap();
    assert!(empty.contains("ctx.as_ref().is_ok_and(|results| results.hits.is_empty())"));
    assert!(empty.contains("research_jobs::finish(&mut conv, bound_research_job, true)"));
    assert!(empty.contains("generation_requested = false"));
    assert!(empty.contains("save_state(&mut conv);"));
    assert!(empty.contains("continue;"));
    for forbidden in [
        "generate_",
        "dialogue_fallback",
        "note_dialogue_generation_failed",
        "save_research",
        "last_research_anchor =",
        "note_new_page_context",
    ] {
        assert!(!empty.contains(forbidden), "{forbidden}");
    }
    assert!(!source.contains("\"web context: {}\""));
    let (_, enrichment) = source
        .split_once("if let Some(ref ctx) = web_ctx {")
        .unwrap();
    let (diagnostic, _) = enrichment.split_once("let web_prompt_body").unwrap();
    assert!(diagnostic.contains("result_count = ctx.hits.len()"));
    assert!(!diagnostic.contains("prompt_body"));
}
