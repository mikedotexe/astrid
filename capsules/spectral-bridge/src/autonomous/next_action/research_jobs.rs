//! Durable research identity across the deferred web-fetch/generation boundary.
use super::*;
use crate::action_continuity::{ActionContinuityStore, ResearchReservation, ResearchWebRequest};
use anyhow::{Context, Result};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct BoundResearchJob {
    reservation: ResearchReservation,
    request: ResearchWebRequest,
}

impl BoundResearchJob {
    pub(crate) fn prepare(
        store: &ActionContinuityStore,
        reservation: ResearchReservation,
    ) -> Result<Self> {
        let request = action_continuity::research_web_request(reservation.action()?)?
            .context("deferred research needs an exact web request")?;
        store.transition_research(&reservation, "queued")?;
        Ok(Self {
            reservation,
            request,
        })
    }

    pub(in crate::autonomous) fn install(&self, conv: &mut ConversationState) {
        match &self.request {
            ResearchWebRequest::Search(query) => {
                conv.browse_url = None;
                conv.wants_search = true;
                conv.search_topic = Some(query.clone());
            },
            ResearchWebRequest::Browse(url) => {
                conv.wants_search = false;
                conv.search_topic = None;
                conv.browse_url = Some(url.clone());
            },
        }
    }

    pub(crate) fn start(&self, store: &ActionContinuityStore) -> Result<()> {
        anyhow::ensure!(
            action_continuity::research_web_request(self.reservation.action()?)?
                == Some(self.request.clone()),
            "queued research target differs from reserved action"
        );
        store.claim_research_start(&self.reservation)
    }

    pub(crate) fn finish(&self, store: &ActionContinuityStore, succeeded: bool) -> Result<()> {
        store.transition_research(
            &self.reservation,
            if succeeded { "completed" } else { "failed" },
        )?;
        Ok(())
    }
}

fn clear_request(conv: &mut ConversationState, request: &ResearchWebRequest) {
    match request {
        ResearchWebRequest::Search(query) if conv.search_topic.as_ref() == Some(query) => {
            conv.wants_search = false;
            conv.search_topic = None;
        },
        ResearchWebRequest::Browse(url) if conv.browse_url.as_ref() == Some(url) => {
            conv.browse_url = None;
        },
        _ => {},
    }
}

pub(in crate::autonomous) fn clear_prepared(conv: &mut ConversationState, action: &str) {
    if let Ok(Some(request)) = action_continuity::research_web_request(action) {
        clear_request(conv, &request);
    }
}

pub(in crate::autonomous) fn enqueue(
    store: &ActionContinuityStore,
    conv: &mut ConversationState,
    reservation: ResearchReservation,
) -> Result<()> {
    let action = reservation.action()?.to_string();
    match BoundResearchJob::prepare(store, reservation) {
        Ok(job) => {
            job.install(conv);
            conv.pending_bound_research = Some(job);
            Ok(())
        },
        Err(error) => {
            clear_prepared(conv, &action);
            Err(error)
        },
    }
}

/// Only the actual web worker may claim execution. Old saved pointers cannot
/// replay a start: the canonical owner stream, not conversation state, decides.
pub(in crate::autonomous) fn begin(conv: &mut ConversationState) -> Option<BoundResearchJob> {
    let job = conv.pending_bound_research.take()?;
    let store = ActionContinuityStore::for_astrid_workspace();
    let matches = match &job.request {
        ResearchWebRequest::Search(query) => {
            conv.wants_search
                && conv.search_topic.as_ref() == Some(query)
                && conv.browse_url.is_none()
        },
        ResearchWebRequest::Browse(url) => {
            conv.browse_url.as_ref() == Some(url) && !conv.wants_search
        },
    };
    let started = if matches {
        job.start(&store)
    } else {
        Err(anyhow::anyhow!("queued research selection changed"))
    };
    match started {
        Ok(()) => Some(job),
        Err(error) => {
            // No network work happened in this worker. A canonical started row
            // from an earlier process refuses release and remains uncertain.
            let _ = store.transition_research(&job.reservation, "released_unstarted");
            clear_request(conv, &job.request);
            conv.pending_file_listing = Some(format!(
                "Bound research did not start: {error:#}. Inspect the recorded operation; it was not replayed."
            ));
            None
        },
    }
}

pub(in crate::autonomous) fn finish(
    conv: &mut ConversationState,
    job: Option<BoundResearchJob>,
    succeeded: bool,
) {
    if let Some(job) = job
        && let Err(error) = job.finish(&ActionContinuityStore::for_astrid_workspace(), succeeded)
    {
        conv.pending_file_listing = Some(format!(
            "Research worker returned but terminal persistence failed: {error:#}. Execution is uncertain; do not replay."
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn dispatch_queue_preserves_binding_parks_reader_and_start_is_single_use() {
        let _web = crate::public_web::fixture::Fixture::new();
        let root = tempfile::tempdir().unwrap();
        let _scope = action_continuity::scoped_test_action_continuity_root(
            root.path().join("action_threads"),
        );
        let store = ActionContinuityStore::for_astrid_workspace();
        let experiment = store
            .start_experiment(None, "Synthetic web inquiry", "Read an exact public source")
            .unwrap();
        let thread = store.current_thread().unwrap().unwrap();
        let path = store
            .root()
            .join("threads")
            .join(&thread.thread_id)
            .join("authority_gate.jsonl");
        action_continuity::append_native_authority_record(&path, &json!({
            "record_schema":"research_budget_v1", "record_type":"research_budget_approval", "budget_id":"fixture",
            "being":"astrid", "thread_id":thread.thread_id, "experiment_id":experiment.experiment_id,
            "scope":"read_only_research", "status":"active", "max_actions":4,
            "allowed_sources":["web"], "expires_at_unix_s":4_102_444_800u64
        })).unwrap();
        let mut conv = ConversationState::new(Vec::new(), None);
        let source = root.path().join("prior.txt");
        std::fs::write(&source, "Synthetic earlier reading, not the web source.").unwrap();
        super::super::super::activity_reading::choose_saved_text_in(
            &store,
            &mut conv,
            &source,
            "Prior reading",
        )
        .unwrap();
        // Saved reading creates its own native thread. Explicitly select the
        // experiment's thread without changing the foreground reading cursor.
        assert!(
            store
                .resolve_experiment_bind_target(Some(&experiment.experiment_id))
                .is_err()
        );
        store.resume_thread(&thread.thread_id).unwrap();
        assert!(conv.activity.foreground_reader.is_some());
        let db = BridgeDb::open(":memory:").unwrap();
        let (sensory_tx, _rx) = mpsc::channel(1);
        let telemetry: SpectralTelemetry =
            serde_json::from_value(json!({"t_ms":0,"eigenvalues":[1.0],"fill_ratio":0.68}))
                .unwrap();
        let mut burst = 0;
        let outcome = super::super::handle_next_action(
            &mut conv,
            &format!(
                "EXPERIMENT_BIND {} :: SEARCH \"CONTROL Vec<T>\"  - exact;",
                experiment.experiment_id
            ),
            super::super::NextActionContext {
                operation_id: Some("actual-dispatch"),
                burst_count: &mut burst,
                db: &db,
                sensory_tx: &sensory_tx,
                telemetry: &telemetry,
                fill_pct: 68.0,
                response_text: "",
                workspace: None,
            },
        );
        assert_eq!(outcome.status, "queued", "{}", outcome.outcome_summary);
        assert_eq!(
            conv.search_topic.as_deref(),
            Some("\"CONTROL Vec<T>\"  - exact;")
        );
        assert!(conv.activity.foreground_reader.is_none());
        assert!(conv.activity.return_reader.is_some());
        let rows = action_continuity::read_native_authority_records(&path).unwrap();
        assert_eq!(rows.last().unwrap()["lifecycle"], "queued");
        assert!(
            !rows
                .iter()
                .any(|row| row["record_type"] == "research_budget_debit")
        );
        let old = conv.pending_bound_research.clone().unwrap();
        // Defensive recovery cannot erase a newer unrelated selection.
        conv.search_topic = Some("newer selection".to_string());
        assert!(begin(&mut conv).is_none());
        assert_eq!(conv.search_topic.as_deref(), Some("newer selection"));
        assert!(conv.wants_search);
        assert!(old.start(&store).is_err());
        assert_eq!(
            action_continuity::read_native_authority_records(&path)
                .unwrap()
                .last()
                .unwrap()["lifecycle"],
            "released_unstarted"
        );
        let ticket = store
            .reserve_research(
                &experiment.experiment_id,
                "SEARCH empty fixture",
                "successful-empty",
                &json!({}),
            )
            .unwrap();
        enqueue(&store, &mut conv, ticket).unwrap();
        let empty_job = begin(&mut conv).unwrap();
        conv.wants_search = false;
        conv.search_topic = None;
        let reader_before = serde_json::to_value(&conv.activity).unwrap();
        let anchor_before = conv.last_research_anchor.clone();
        let history_before = conv.history.len();
        finish(&mut conv, Some(empty_job), true);
        assert_eq!(
            action_continuity::read_native_authority_records(&path)
                .unwrap()
                .last()
                .unwrap()["lifecycle"],
            "completed"
        );
        assert_eq!(serde_json::to_value(&conv.activity).unwrap(), reader_before);
        assert_eq!(conv.last_research_anchor, anchor_before);
        assert_eq!(conv.history.len(), history_before);
        let ticket = store
            .reserve_research(
                &experiment.experiment_id,
                "SEARCH failed handoff",
                "failed-handoff",
                &json!({}),
            )
            .unwrap();
        conv.search_topic = Some("failed handoff".to_string());
        let mut corrupt = std::fs::read(&path).unwrap();
        corrupt.extend_from_slice(b"{\"incomplete\":");
        std::fs::write(&path, &corrupt).unwrap();
        assert!(enqueue(&store, &mut conv, ticket).is_err());
        assert!(conv.pending_bound_research.is_none());
        assert!(!conv.wants_search);
        assert!(conv.search_topic.is_none());
        assert_eq!(std::fs::read(&path).unwrap(), corrupt);
    }
}
