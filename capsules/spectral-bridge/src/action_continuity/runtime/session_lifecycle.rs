// Authored session commands. Mechanical reader progress stays in the same log.

impl ActionContinuityStore {
    pub fn continuity_session_start_command(&self, raw: &str) -> Result<String> {
        let mut thread = self.ensure_active_thread(None)?;
        let (selector, payload) = parse_session_selector_payload(raw);
        let experiment = self.resolve_memory_experiment(&thread, selector.as_deref())?;
        let title =
            dossier_field(&payload, &["title"]).unwrap_or_else(|| "Continuity session".to_string());
        let focus =
            dossier_field(&payload, &["focus"]).unwrap_or_else(|| payload.trim().to_string());
        let session_id = format!(
            "sess_{SYSTEM}_{}_{:032x}_{}",
            now_millis(),
            rand::random::<u128>(),
            sanitize_slug(&title)
        );
        let record = self.continuity_session_record(
            "session_start",
            &session_id,
            &thread,
            experiment.as_ref(),
            "active",
            ContinuitySessionFields {
                title: Some(title),
                focus: Some(focus),
                summary: dossier_field(&payload, &["summary"]),
                open_questions: dossier_list_field(&payload, &["open_questions", "questions", "question"]),
                source_refs: dossier_list_field(&payload, &["source_refs", "source", "sources"]),
                artifact_refs: dossier_list_field(&payload, &["artifact_refs", "artifact", "artifact_grounding"]),
                suggested_next: dossier_field(&payload, &["next", "next_safe_command"])
                    .or_else(|| Some(format!("CONTINUITY_SESSION_CAPTURE {session_id} :: summary: ...; source_refs: ...; artifact_refs: ...; next: ..."))),
                extra: json!({}),
            },
        );
        self.append_continuity_session_record(&mut thread, record)?;
        Ok(format!(
            "Continuity session `{session_id}` started.\nSuggested NEXT: CONTINUITY_SESSION_CAPTURE {session_id} :: summary: ...; source_refs: ...; artifact_refs: ...; next: ..."
        ))
    }

    pub fn continuity_session_capture_command(&self, raw: &str) -> Result<String> {
        let mut thread = self.ensure_active_thread(None)?;
        let (selector, payload) = parse_session_selector_payload(raw);
        let Some(session) = self.resolve_continuity_session(&thread, selector.as_deref())? else {
            return Err(ContinuityInputError("CONTINUITY_SESSION_CAPTURE needs an existing session. Start one with CONTINUITY_SESSION_START current :: title: ...; focus: ...; next: ...").into());
        };
        if session_is_quiet(&session) {
            return Err(ContinuityInputError("Explicitly CONTINUITY_SESSION_RESUME this session before capturing another note. The bookmark is unchanged.").into());
        }
        let summary = session_authored_summary(&payload)?;
        let experiment = self.session_experiment(&thread, &session)?;
        let session_id = session
            .get("session_id")
            .and_then(Value::as_str)
            .unwrap_or("latest")
            .to_string();
        let source_refs = session_preserved_list(
            &payload,
            &["source_refs", "source", "sources"],
            &session,
            "source_refs",
        );
        let artifact_refs = session_preserved_list(
            &payload,
            &["artifact_refs", "artifact", "artifact_grounding"],
            &session,
            "artifact_refs",
        );
        let next_command = session_preserved_next(&payload, &session);
        let record = self.continuity_session_record(
            "session_capture",
            &session_id,
            &thread,
            experiment.as_ref(),
            "active",
            ContinuitySessionFields {
                title: session
                    .get("title")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                focus: session
                    .get("focus")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                summary: Some(summary.clone()),
                open_questions: session_preserved_list(
                    &payload,
                    &["open_questions", "questions", "question"],
                    &session,
                    "open_questions",
                ),
                source_refs: source_refs.clone(),
                artifact_refs: artifact_refs.clone(),
                suggested_next: next_command.clone(),
                extra: json!({"expected_session_record_id": session["record_id"]}),
            },
        );
        self.append_continuity_session_record(&mut thread, record.clone())?;
        let continuity_path = self
            .continuity_sessions_path(&thread.thread_id)
            .display()
            .to_string();
        let memory = self.append_being_memory_record(
            &mut thread,
            experiment.as_ref(),
            "continuity_session_capture",
            &summary,
            {
                let mut refs = vec![
                    continuity_path,
                    record
                        .get("record_id")
                        .and_then(Value::as_str)
                        .unwrap_or("session_capture")
                        .to_string(),
                ];
                refs.extend(source_refs);
                refs
            },
            artifact_refs,
            next_command.clone().or_else(|| Some(format!("CONTINUITY_SESSION_CAPTURE {session_id} :: summary: ...; source_refs: ...; artifact_refs: ...; next: ..."))),
            "card",
            json!({"continuity_session_id": session_id}),
        )?;
        Ok(format!(
            "Continuity session `{}` captured as `{}`.\nMemory card: {}\nSaved in: {}\nRecorded next step (not dispatched): {}",
            session_id,
            record
                .get("record_id")
                .and_then(Value::as_str)
                .unwrap_or("session_capture"),
            memory
                .get("memory_id")
                .and_then(Value::as_str)
                .unwrap_or("memory"),
            self.continuity_sessions_path(&thread.thread_id).display(),
            next_command.as_deref().unwrap_or("(none)")
        ))
    }

    pub fn continuity_session_accept_command(&self, raw: &str) -> Result<String> {
        let mut thread = self.ensure_active_thread(None)?;
        let selector = raw.trim();
        let selector = if selector.is_empty() {
            "latest"
        } else {
            selector
        };
        let Some(draft) = self.resolve_continuity_session_draft(&thread, Some(selector))? else {
            return Err(ContinuityInputError(
                "No continuity-session draft is available to accept. Wait for guarded pressure or start one with CONTINUITY_SESSION_START current :: title: ...; focus: ...; next: ..."
            ).into());
        };
        let experiment = self.session_experiment(&thread, &draft)?;
        let experiment_id = experiment
            .as_ref()
            .map(|experiment| experiment.experiment_id.as_str());
        let existing_rows = self.continuity_session_rows(&thread.thread_id, experiment_id, 16)?;
        let has_existing_session = !existing_rows.is_empty();
        let session_id = if has_existing_session {
            existing_rows
                .last()
                .and_then(|row| row.get("session_id"))
                .and_then(Value::as_str)
                .unwrap_or("latest")
                .to_string()
        } else {
            draft
                .get("session_id")
                .and_then(Value::as_str)
                .unwrap_or("latest")
                .to_string()
        };
        let summary = draft
            .get("summary")
            .or_else(|| draft.get("raw_intent"))
            .and_then(Value::as_str)
            .unwrap_or("Preserve guarded continuity before more work.")
            .to_string();
        let mut source_refs = draft
            .get("source_refs")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        source_refs.push(
            self.continuity_sessions_path(&thread.thread_id)
                .display()
                .to_string(),
        );
        source_refs.push(
            draft
                .get("record_id")
                .and_then(Value::as_str)
                .unwrap_or("session_draft")
                .to_string(),
        );
        let record_type = if has_existing_session {
            "session_capture"
        } else {
            "session_start"
        };
        let record = self.continuity_session_record(
            record_type,
            &session_id,
            &thread,
            experiment.as_ref(),
            "active",
            ContinuitySessionFields {
                title: draft
                    .get("title")
                    .and_then(Value::as_str)
                    .map(str::to_string)
                    .or_else(|| Some("Accepted continuity draft".to_string())),
                focus: draft
                    .get("focus")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                summary: Some(summary.clone()),
                open_questions: Vec::new(),
                source_refs: source_refs.clone(),
                artifact_refs: Vec::new(),
                suggested_next: draft
                    .get("suggested_next")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                extra: json!({
                    "expected_session_record_id": existing_rows.last().and_then(|row| row.get("record_id")),
                    "accepted_from_draft_id": draft.get("record_id").cloned().unwrap_or(Value::Null),
                    "accepted_by_command": "CONTINUITY_SESSION_ACCEPT",
                }),
            },
        );
        self.append_continuity_session_record(&mut thread, record.clone())?;
        if has_existing_session {
            let _ = self.append_being_memory_record(
                &mut thread,
                experiment.as_ref(),
                "continuity_session_capture",
                &summary,
                source_refs,
                Vec::new(),
                record
                    .get("suggested_next")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                "card",
                json!({"continuity_session_id": session_id}),
            )?;
        }
        Ok(format!(
            "Accepted continuity-session draft as `{record_type}` for `{session_id}`.\nSuggested NEXT: {}",
            record
                .get("suggested_next")
                .and_then(Value::as_str)
                .unwrap_or("CONTINUITY_SESSION_CAPTURE latest :: summary: ...; source_refs: ...; artifact_refs: ...; next: ...")
        ))
    }

    pub fn continuity_session_summarize_command(&self, raw: &str) -> Result<String> {
        let mut thread = self.ensure_active_thread(None)?;
        let (selector, payload) = parse_session_selector_payload(raw);
        let Some(session) = self.resolve_continuity_session(&thread, selector.as_deref())? else {
            return Err(ContinuityInputError(
                "CONTINUITY_SESSION_SUMMARIZE needs an existing session.",
            )
            .into());
        };
        if session_is_quiet(&session) {
            return Err(ContinuityInputError(
                "Explicitly CONTINUITY_SESSION_RESUME this session before summarizing it.",
            )
            .into());
        }
        let summary = session_authored_summary(&payload)?;
        let experiment = self.session_experiment(&thread, &session)?;
        let session_id = session
            .get("session_id")
            .and_then(Value::as_str)
            .unwrap_or("latest")
            .to_string();
        let record = self.continuity_session_record(
            "session_summary",
            &session_id,
            &thread,
            experiment.as_ref(),
            "summarized",
            ContinuitySessionFields {
                title: session
                    .get("title")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                focus: session
                    .get("focus")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                summary: Some(summary),
                open_questions: session_preserved_list(
                    &payload,
                    &["open_questions", "questions", "question"],
                    &session,
                    "open_questions",
                ),
                source_refs: session_preserved_list(
                    &payload,
                    &["source_refs", "source", "sources"],
                    &session,
                    "source_refs",
                ),
                artifact_refs: session_preserved_list(
                    &payload,
                    &["artifact_refs", "artifact", "artifact_grounding"],
                    &session,
                    "artifact_refs",
                ),
                suggested_next: session_preserved_next(&payload, &session),
                extra: json!({"expected_session_record_id": session["record_id"]}),
            },
        );
        let next_command = record
            .get("suggested_next")
            .and_then(Value::as_str)
            .unwrap_or("(none)")
            .to_string();
        self.append_continuity_session_record(&mut thread, record)?;
        Ok(format!(
            "Continuity session `{session_id}` summarized. Recorded next step (not dispatched): {next_command}"
        ))
    }

    pub fn continuity_session_finalize_command(&self, raw: &str) -> Result<String> {
        let mut thread = self.ensure_active_thread(None)?;
        let (selector, payload) = parse_session_selector_payload(raw);
        let Some(session) = self.resolve_continuity_session(&thread, selector.as_deref())? else {
            return Err(ContinuityInputError(
                "CONTINUITY_SESSION_FINALIZE needs an existing session.",
            )
            .into());
        };
        let outcome = dossier_field(&payload, &["outcome", "status"])
            .unwrap_or_else(|| "park".to_string())
            .to_ascii_lowercase();
        let status = match outcome.as_str() {
            "complete" => "complete",
            "hold" => "held",
            "park" => "parked",
            _ => {
                return Err(ContinuityInputError(
                    "Choose outcome: complete, park, or hold. No session state changed.",
                )
                .into());
            },
        };
        let experiment = self.session_experiment(&thread, &session)?;
        let session_id = session
            .get("session_id")
            .and_then(Value::as_str)
            .unwrap_or("latest")
            .to_string();
        let summary = dossier_field(&payload, &["summary", "note"]).or_else(|| {
            session
                .get("summary")
                .and_then(Value::as_str)
                .map(str::to_string)
        });
        let record = self.continuity_session_record(
            "session_finalize",
            &session_id,
            &thread,
            experiment.as_ref(),
            status,
            ContinuitySessionFields {
                title: session
                    .get("title")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                focus: session
                    .get("focus")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                summary,
                open_questions: session_preserved_list(&payload, &["open_questions", "questions", "question"], &session, "open_questions"),
                source_refs: session_preserved_list(&payload, &["source_refs", "source", "sources"], &session, "source_refs"),
                artifact_refs: session_preserved_list(&payload, &["artifact_refs", "artifact", "artifact_grounding"], &session, "artifact_refs"),
                suggested_next: session_preserved_next(&payload, &session),
                extra: json!({"expected_session_record_id": session["record_id"], "outcome": outcome, "return_cue": dossier_field(&payload, &["return_cue"]).unwrap_or_else(|| "explicit request".to_string()), "automatic_return": false}),
            },
        );
        self.append_continuity_session_record(&mut thread, record)?;
        let target = experiment
            .as_ref()
            .map_or("latest", |experiment| experiment.experiment_id.as_str());
        Ok(format!(
            "Continuity session `{session_id}` finalized as {status}.\nAvailable on request: CONTINUITY_SESSION_RESUME {session_id}\nReturn cue is recorded only, not scheduled.\nPromotion options: MEMORY_PROMOTE {target} :: dossier|evidence|authority_request"
        ))
    }

    pub fn continuity_session_resume_command(&self, raw: &str) -> Result<String> {
        let mut thread = self.ensure_active_thread(None)?;
        let (selector, payload) = parse_session_selector_payload(raw);
        let Some(session) = self.resolve_continuity_session(&thread, selector.as_deref())? else {
            return Err(ContinuityInputError(
                "CONTINUITY_SESSION_RESUME could not find a session.",
            )
            .into());
        };
        if session.get("reader_bookmark_v1").is_some() {
            let revision = dossier_field(&payload, &["revision"]);
            if revision.as_deref() != session["record_id"].as_str() {
                return Err(ContinuityInputError("Inspect CONTINUITY_SESSION_STATUS first, then resume with :: revision: <session record ID>. The reader bookmark is unchanged.").into());
            }
            let bookmark: ReaderBookmark =
                serde_json::from_value(session["reader_bookmark_v1"].clone())?;
            reader_bookmark_io::retained_text(self, &bookmark.source)
                .context("Saved reading source is unavailable; session remains parked")?;
        }
        let experiment = self.session_experiment(&thread, &session)?;
        let session_id = session
            .get("session_id")
            .and_then(Value::as_str)
            .unwrap_or("latest")
            .to_string();
        let record = self.continuity_session_record(
            "session_reopen",
            &session_id,
            &thread,
            experiment.as_ref(),
            "active",
            ContinuitySessionFields {
                title: session.get("title").and_then(Value::as_str).map(str::to_string),
                focus: session.get("focus").and_then(Value::as_str).map(str::to_string),
                summary: session.get("summary").and_then(Value::as_str).map(str::to_string),
                open_questions: value_string_list(session.get("open_questions")),
                source_refs: value_string_list(session.get("source_refs")),
                artifact_refs: value_string_list(session.get("artifact_refs")),
                suggested_next: session
                    .get("suggested_next")
                    .and_then(Value::as_str)
                    .map(str::to_string)
                    .or_else(|| Some(format!("CONTINUITY_SESSION_CAPTURE {session_id} :: summary: ...; source_refs: ...; artifact_refs: ...; next: ..."))),
                extra: json!({"expected_session_record_id": session["record_id"]}),
            },
        );
        self.append_continuity_session_record(&mut thread, record.clone())?;
        Ok(format!(
            "Continuity session `{session_id}` reopened.\nFocus: {}\nSummary: {}\nOpen questions: {}\nSources: {}\nSuggested NEXT (not dispatched): {}",
            session
                .get("focus")
                .and_then(Value::as_str)
                .unwrap_or("(none)"),
            truncate_chars(
                session
                    .get("summary")
                    .and_then(Value::as_str)
                    .unwrap_or("(no summary yet)"),
                400
            ),
            serde_json::to_string(&value_string_list(session.get("open_questions")))?,
            serde_json::to_string(&value_string_list(session.get("source_refs")))?,
            record
                .get("suggested_next")
                .and_then(Value::as_str)
                .unwrap_or("CONTINUITY_SESSION_CAPTURE latest :: summary: ...")
        ))
    }

    pub fn continuity_session_status_command(&self, raw: &str) -> Result<String> {
        let (selector, _) = parse_session_selector_payload(raw);
        let index_path = self.index_path();
        if !index_path.exists() {
            return Ok("continuity_session_v1: no active thread; nothing was created.".to_string());
        }
        let index: ContinuityIndex = serde_json::from_slice(&fs::read(index_path)?)?;
        let Some(thread_id) = index.active_thread_id else {
            return Ok("continuity_session_v1: no active thread; nothing was created.".to_string());
        };
        let thread: ResearchThread =
            serde_json::from_slice(&fs::read(self.thread_dir(&thread_id).join("thread.json"))?)?;
        let mut summary = self.continuity_session_summary_v1(&thread, selector.as_deref(), 8)?;
        if let Some(session_id) = summary["latest_session"]["session_id"].as_str() {
            let preview = self.reader_bookmark_preview(&thread.thread_id, session_id)?;
            summary["reader_preview"] = serde_json::to_value(preview)?;
        }
        Ok(format!(
            "continuity_session_v1:\n{}",
            serde_json::to_string_pretty(&summary)?
        ))
    }
}
