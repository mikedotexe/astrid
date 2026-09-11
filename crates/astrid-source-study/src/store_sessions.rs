//! Atomic multi-page study acceptance and read-only delivery traces.
use super::*;
impl Reader {
    pub(super) fn prepare_session(
        &self,
        state: &mut State,
        targets: &[(String, usize)],
    ) -> Result<StudyOutput> {
        if !(2..=3).contains(&targets.len()) {
            bail!("a source session needs two or three chosen pages");
        }
        let budget = 7000usize
            .checked_div(targets.len())
            .context("session size")?;
        let mut pages = Vec::new();
        for (name, line) in targets {
            let source = match self.requested_source(name) {
                Ok(source) => source,
                Err(error) => return self.source_recovery(state, name, &error),
            };
            match Page::read_with_budget(&source, None, *line, None, budget) {
                Ok(page) => pages.push(page),
                Err(error) => return self.recovery_map(state, &error),
            }
        }
        state.sequence = state
            .sequence
            .checked_add(1)
            .context("study sequence exhausted")?;
        let mut text="You chose these source pages together. Inspect their relationship before writing if useful. One freeform response covers this session; no report or minimum length is required. CONTINUE after success follows the last selected page; OPEN or RELATE can choose another direction.\n\n".to_string();
        for (index, page) in pages.iter_mut().enumerate() {
            let id = digest(format!("session:{}:{index}:{}", state.sequence, page.id));
            page.text = page.text.replace(&page.id, &id);
            page.id = id;
            page.question_id.clone_from(&state.questions.active);
            text.push_str(&page.text);
            text.push('\n');
        }
        let mut output = self.output(state, text, None, InputKind::SourceSession)?;
        output.session_pages = pages;
        state.pending_navigation = None;
        state.pending = None;
        state.pending_page_output = None;
        state.pending_session = Some(output.clone());
        self.save(state)?;
        Ok(output)
    }

    pub(super) fn record_output(
        &self,
        state: &mut State,
        output: &StudyOutput,
        receipt: &DeliveryReceipt,
        request: &str,
        response: &str,
    ) -> Result<()> {
        output.verify_delivery(request, response)?;
        for page in &output.session_pages {
            verify_wire(page, request, response)?;
            let artifact = serde_json::to_vec_pretty(
                &serde_json::json!({"schema":"source_study_delivery_v1","page":page,"request_json":request,"response_json":response,"session_id":output.navigation_id}),
            )?;
            let hash = digest(&artifact);
            let path = self
                .directory
                .join("deliveries")
                .join(&page.id)
                .join(format!("{hash}.json"));
            fs::create_dir_all(path.parent().context("session page directory")?)?;
            atomic_write(&path, &artifact)?;
            let page_receipt = DeliveryReceipt {
                page_id: page.id.clone(),
                request_sha256: digest(request),
                response_sha256: digest(response),
                artifact_path: path,
                artifact_sha256: hash,
                choice_feedback: None,
            };
            progress::record(state.progress.get_or_insert_with(Progress::new), page);
            state.bookmarks.insert(page.source.clone(), page.clone());
            state.receipts.insert(page.id.clone(), page_receipt);
            state.current = Some(page.source.clone());
        }
        state.questions.record(
            output.question_id.as_deref(),
            &mut state.notebook,
            response,
            &completion_text(response)?,
            &output.session_pages,
        );
        if output.session_pages.is_empty() {
            state.pending_navigation = None;
        } else {
            state.pending_session = None;
        }
        if !matches!(
            output.input_kind,
            InputKind::RuntimeTrace | InputKind::Questions
        ) {
            state.last_input = Some(receipt.clone());
        }
        state.last_navigation = Some(receipt.clone());
        record_choice(state, receipt, request, response)?;
        Ok(())
    }

    pub(super) fn trace_input(&self, state: &State, target: &str) -> Result<String> {
        if target != "LAST" {
            return self
                .runtime
                .as_ref()
                .context("host execution records not configured")?
                .view(target);
        }
        let receipt = state
            .last_input
            .as_ref()
            .or_else(|| {
                state
                    .current
                    .as_ref()
                    .and_then(|source| state.bookmarks.get(source))
                    .and_then(|page| state.receipts.get(&page.id))
            })
            .or(state.last_navigation.as_ref())
            .context("no retained study delivery yet")?;
        let path = fs::canonicalize(&receipt.artifact_path)?;
        if !path.starts_with(fs::canonicalize(&self.directory)?)
            || fs::metadata(&path)?.len() > 1024 * 1024
        {
            bail!("study trace artifact outside reader or size limit");
        }
        let raw = fs::read(path)?;
        if digest(&raw) != receipt.artifact_sha256 {
            bail!("study trace receipt hash mismatch");
        }
        let artifact: Value = serde_json::from_slice(&raw)?;
        let request = artifact["request_json"]
            .as_str()
            .context("request missing")?;
        let response = artifact["response_json"]
            .as_str()
            .context("response missing")?;
        if digest(request) != receipt.request_sha256 || digest(response) != receipt.response_sha256
        {
            bail!("study trace wire hash mismatch");
        }
        let pages = if let Some(output) = artifact.get("output").filter(|o| !o.is_null()) {
            let output: StudyOutput = serde_json::from_value(output.clone())?;
            output.verify_delivery(request, response)?;
            if let Some(page) = output.page {
                vec![page]
            } else {
                output.session_pages
            }
        } else {
            let page: Page = serde_json::from_value(artifact["page"].clone())?;
            page.verify_delivery(request, response)?;
            vec![page]
        };
        let req: Value = serde_json::from_str(request)?;
        let resp: Value = serde_json::from_str(response)?;
        let supplied=pages.iter().map(|p|serde_json::json!({"source":p.source,"revision":p.revision.sha256,"bytes":[p.start.byte,p.end.byte],"question_id":p.question_id,"saved_bookmark_is_this_page":state.bookmarks.get(&p.source).is_some_and(|b|b.id==p.id)})).collect::<Vec<_>>();
        let text = completion_text(response)?;
        let tail = text
            .chars()
            .rev()
            .take(400)
            .collect::<String>()
            .chars()
            .rev()
            .collect::<String>();
        Ok(serde_json::to_string_pretty(
            &serde_json::json!({"view":"last retained study input; questions and traces do not replace it","receipt_id":receipt.page_id,"artifact_sha256":receipt.artifact_sha256,"verified_request_sha256":receipt.request_sha256,"verified_response_sha256":receipt.response_sha256,"source_pages":supplied,"model":req["model"],"output_ceiling":req.get("max_tokens").or_else(||req.pointer("/options/num_predict")),"context_tokens":req.pointer("/options/num_ctx"),"finish":resp.pointer("/choices/0/finish_reason").or_else(||resp.get("done_reason")),"response_tail_reference_only":tail,"action_execution":"This receipt establishes source input and completion. It does not establish that the response's NEXT ran. TRACE <your exact job_id or action_id> inspects a separately retained execution record; absence remains unknown."}),
        )?)
    }
}
