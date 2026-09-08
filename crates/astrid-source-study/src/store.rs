use crate::notebook::Notebook;
use crate::progress::{self, Progress};
use crate::{Catalog, Command, Page, SCHEMA_VERSION, digest};
use anyhow::{Context as _, Result, bail};
use fs2::FileExt as _;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::io::Write as _;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct StudyOutput {
    pub system_prompt: String,
    pub text: String,
    pub page: Option<Page>,
    #[serde(default)]
    pub navigation_id: Option<String>,
}
impl StudyOutput {
    /// Verify the entire offered input, including its study notebook.
    /// # Errors
    /// Rejects shortened input or an incomplete provider response.
    pub fn verify_delivery(&self, request: &str, response: &str) -> Result<()> {
        verify_text(&self.text, request, response)
    }
}
impl Page {
    /// Check that this whole page appears in a completed provider request.
    /// # Errors
    /// Returns an error for missing bytes or an unsuccessful provider response.
    pub fn verify_delivery(&self, request: &str, response: &str) -> Result<()> {
        verify_wire(self, request, response)
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct DeliveryReceipt {
    pub page_id: String,
    pub request_sha256: String,
    pub response_sha256: String,
    pub artifact_path: PathBuf,
    pub artifact_sha256: String,
}
#[derive(Default, Serialize, Deserialize)]
struct State {
    version: u32,
    #[serde(default)]
    sequence: u64,
    pending: Option<Page>,
    current: Option<String>,
    bookmarks: BTreeMap<String, Page>,
    receipts: BTreeMap<String, DeliveryReceipt>,
    #[serde(default)]
    progress: Option<Progress>,
    #[serde(default)]
    notebook: Notebook,
    #[serde(default)]
    pending_navigation: Option<StudyOutput>,
    #[serde(default)]
    last_navigation: Option<DeliveryReceipt>,
}
pub struct Reader {
    catalog: Catalog,
    directory: PathBuf,
}

impl Reader {
    #[must_use]
    pub fn new(catalog: Catalog, directory: PathBuf) -> Self {
        Self { catalog, directory }
    }

    fn output(
        &self,
        state: &mut State,
        mut text: String,
        page: Option<Page>,
    ) -> Result<StudyOutput> {
        text.push_str(&state.notebook.render());
        let mut output = StudyOutput {
            system_prompt: crate::STUDY_PROMPT.into(),
            text,
            page,
            navigation_id: None,
        };
        if output.page.is_none() {
            state.sequence = state
                .sequence
                .checked_add(1)
                .context("source-study sequence exhausted")?;
            output.navigation_id = Some(digest(format!(
                "navigation:{}:{}",
                state.sequence, output.text
            )));
            state.pending_navigation = Some(output.clone());
            self.save(state)?;
        }
        Ok(output)
    }

    fn map(&self, state: &State, topic: &str, page: usize) -> Result<String> {
        let progress = state
            .progress
            .as_ref()
            .context("study progress not loaded")?;
        self.catalog
            .map(topic, page, progress, state.current.as_deref())
    }

    // Migrate only retained, verified shared-reader deliveries. Preparation-only
    // legacy coverage is intentionally not imported. Keep the latest bookmark's
    // revision, even if older receipt filenames sort after newer ones.
    fn hydrate(&self, state: &mut State) -> Result<()> {
        if state.progress.is_some() {
            return Ok(());
        }
        let mut progress = Progress::new();
        for receipt in state.receipts.values() {
            let raw = fs::read(&receipt.artifact_path)?;
            if digest(&raw) != receipt.artifact_sha256 {
                bail!("retained study receipt hash mismatch");
            }
            let value: Value = serde_json::from_slice(&raw)?;
            let page: Page = serde_json::from_value(value["page"].clone())?;
            let request = value["request_json"]
                .as_str()
                .context("retained request missing")?;
            let response = value["response_json"]
                .as_str()
                .context("retained response missing")?;
            if page.id != receipt.page_id
                || digest(request) != receipt.request_sha256
                || digest(response) != receipt.response_sha256
            {
                bail!("retained study wire identity mismatch");
            }
            verify_wire(&page, request, response)?;
            if state
                .bookmarks
                .get(&page.source)
                .is_some_and(|p| p.revision == page.revision)
            {
                progress::record(&mut progress, &page);
            }
            if state.current.as_ref() == Some(&page.source)
                && state
                    .bookmarks
                    .get(&page.source)
                    .is_some_and(|p| p.id == page.id)
            {
                state
                    .notebook
                    .record(response, &completion_text(response)?, Some(&page));
            }
        }
        state.progress = Some(progress);
        self.save(state)
    }

    /// Save a navigation response in the same Being-owned study notebook.
    /// Navigation never advances a source bookmark or source coverage.
    /// # Errors
    /// Rejects stale offers, shortened input, incomplete responses and I/O failures.
    pub fn navigation_delivered(
        &self,
        navigation_id: &str,
        request_json: &str,
        response_json: &str,
    ) -> Result<DeliveryReceipt> {
        let _lock = self.lock()?;
        let mut state = self.load()?;
        self.hydrate(&mut state)?;
        if let Some(receipt) = &state.last_navigation
            && receipt.page_id == navigation_id
        {
            if receipt.request_sha256 != digest(request_json)
                || receipt.response_sha256 != digest(response_json)
            {
                bail!("navigation was already delivered with different wire evidence");
            }
            return Ok(receipt.clone());
        }
        let offered = state
            .pending_navigation
            .as_ref()
            .filter(|p| p.navigation_id.as_deref() == Some(navigation_id))
            .context("navigation delivery does not match the pending offer")?;
        verify_text(&offered.text, request_json, response_json)?;
        let artifact = serde_json::to_vec_pretty(&serde_json::json!({
            "schema":"source_study_navigation_delivery_v1", "output":offered,
            "request_json":request_json, "response_json":response_json,
        }))?;
        let artifact_sha256 = digest(&artifact);
        let artifact_path = self
            .directory
            .join("navigation")
            .join(navigation_id)
            .join(format!("{artifact_sha256}.json"));
        fs::create_dir_all(
            artifact_path
                .parent()
                .context("navigation artifact directory")?,
        )?;
        atomic_write(&artifact_path, &artifact)?;
        let receipt = DeliveryReceipt {
            page_id: navigation_id.into(),
            request_sha256: digest(request_json),
            response_sha256: digest(response_json),
            artifact_path,
            artifact_sha256,
        };
        state
            .notebook
            .record(response_json, &completion_text(response_json)?, None);
        state.pending_navigation = None;
        state.last_navigation = Some(receipt.clone());
        self.save(&state)?;
        Ok(receipt)
    }

    /// Import a verified bridge navigation artifact without reconstructing its wire bodies.
    /// # Errors
    /// Returns an error for missing evidence or an invalid navigation response.
    pub fn navigation_delivered_artifact(
        &self,
        navigation_id: &str,
        path: &Path,
    ) -> Result<DeliveryReceipt> {
        let artifact: Value = serde_json::from_slice(&fs::read(path)?)?;
        let attempt = artifact.get("attempt").unwrap_or(&artifact);
        self.navigation_delivered(
            navigation_id,
            attempt["request_json"]
                .as_str()
                .context("retained request missing")?,
            attempt["response_json"]
                .as_str()
                .context("retained response missing")?,
        )
    }

    /// Prepare a page without claiming it was delivered.
    /// # Errors
    /// Returns source, navigation, or checkpoint errors without resetting progress.
    pub fn prepare(&self, command: Command) -> Result<StudyOutput> {
        let _lock = self.lock()?;
        let mut state = self.load()?;
        self.hydrate(&mut state)?;
        self.recover(&mut state)?;
        let mut page = match command {
            Command::Map { topic, page } => {
                let text = self.map(&state, &topic, page)?;
                return self.output(&mut state, text, None);
            },
            Command::Find { query, page } => {
                return self.output(&mut state, self.catalog.find(&query, page)?, None);
            },
            Command::Open { source, line } => {
                Page::read(&self.catalog.resolve(&source)?, None, line, None)?
            },
            Command::Resume { source } => {
                let source = self.catalog.resolve(&source)?;
                if let Some(page) = state
                    .pending
                    .as_ref()
                    .filter(|page| page.source == source.id)
                {
                    let page = page.clone();
                    return self.output(&mut state, page.text.clone(), Some(page));
                }
                if let Some(last) = state.bookmarks.get(&source.id) {
                    if last.eof {
                        Page::read(&source, Some(&last.end), 1, Some(&last.revision.sha256))?;
                        return self.output(&mut state, format!(
                            "End of {}. Use SELF_STUDY OPEN {} 1 to reread or SELF_STUDY MAP to choose another source.", source.id, source.id), None);
                    }
                    Page::read(&source, Some(&last.end), 1, Some(&last.revision.sha256))?
                } else {
                    Page::read(&source, None, 1, None)?
                }
            },
            Command::Continue => {
                if let Some(page) = state.pending.clone() {
                    return self.output(&mut state, page.text.clone(), Some(page));
                }
                let Some(last) = state
                    .current
                    .as_ref()
                    .and_then(|id| state.bookmarks.get(id))
                else {
                    let text = self.map(&state, "", 1)?;
                    return self.output(&mut state, text, None);
                };
                if last.eof {
                    Page::read(
                        &self.catalog.resolve(&last.source)?,
                        Some(&last.end),
                        1,
                        Some(&last.revision.sha256),
                    )?;
                    let text = format!(
                        "End of {} at revision {}. {} Choose SELF_STUDY MAP, FIND, or OPEN to deliberately reread.",
                        last.source,
                        last.revision.sha256,
                        state
                            .progress
                            .as_ref()
                            .and_then(|p| p.get(&last.source))
                            .map_or_else(
                                || "Delivery status unavailable.".into(),
                                progress::SourceProgress::label
                            )
                    );
                    return self.output(&mut state, text, None);
                }
                Page::read(
                    &self.catalog.resolve(&last.source)?,
                    Some(&last.end),
                    1,
                    Some(&last.revision.sha256),
                )?
            },
        };
        state.sequence = state
            .sequence
            .checked_add(1)
            .context("source-study sequence exhausted")?;
        let identity = digest(format!("{}:{}", page.id, state.sequence));
        page.text = page.text.replace(&page.id, &identity);
        page.id = identity;
        state.pending = Some(page.clone());
        self.save(&state)?;
        self.output(&mut state, page.text.clone(), Some(page))
    }

    /// Retain the actual provider wire bodies and advance only after verifying
    /// that a complete prepared page reached a successful generation request.
    /// This host API is deliberately not part of the Being's Action vocabulary.
    /// # Errors
    /// Returns an error for incomplete delivery, a stale page, or failed persistence.
    pub fn delivered(
        &self,
        page_id: &str,
        request_json: &str,
        response_json: &str,
    ) -> Result<DeliveryReceipt> {
        let _lock = self.lock()?;
        let mut state = self.load()?;
        self.hydrate(&mut state)?;
        if let Some(receipt) = state.receipts.get(page_id).cloned() {
            if receipt.request_sha256 != digest(request_json)
                || receipt.response_sha256 != digest(response_json)
            {
                bail!("source page was already delivered with different wire evidence");
            }
            if state
                .pending
                .as_ref()
                .is_some_and(|page| page.id == page_id)
            {
                let page = state.pending.take().context("pending page")?;
                verify_wire(&page, request_json, response_json)?;
                record_study(&mut state, &page, response_json)?;
                state.current = Some(page.source.clone());
                state.bookmarks.insert(page.source.clone(), page);
                self.save(&state)?;
            }
            return Ok(receipt);
        }
        let page = state
            .pending
            .as_ref()
            .filter(|p| p.id == page_id)
            .context("delivery does not match the pending source page")?;
        verify_wire(page, request_json, response_json)?;
        let request_sha256 = digest(request_json);
        let response_sha256 = digest(response_json);
        let artifact = serde_json::to_vec_pretty(&serde_json::json!({
            "schema": "source_study_delivery_v1", "page": page,
            "request_json": request_json, "response_json": response_json,
        }))?;
        let artifact_sha256 = digest(&artifact);
        let artifact_path = self
            .directory
            .join("deliveries")
            .join(page_id)
            .join(format!("{artifact_sha256}.json"));
        fs::create_dir_all(artifact_path.parent().context("artifact directory")?)?;
        atomic_write(&artifact_path, &artifact)?;
        let receipt = DeliveryReceipt {
            page_id: page_id.into(),
            request_sha256,
            response_sha256,
            artifact_path,
            artifact_sha256,
        };
        let page = state.pending.take().context("pending page")?;
        record_study(&mut state, &page, response_json)?;
        state.current = Some(page.source.clone());
        state.bookmarks.insert(page.source.clone(), page);
        state.receipts.insert(page_id.into(), receipt.clone());
        self.save(&state)?;
        Ok(receipt)
    }

    /// Import an already retained bridge receipt, preserving its exact wire bodies.
    /// # Errors
    /// Returns an error if retained wire bodies are missing or fail verification.
    pub fn delivered_artifact(&self, page_id: &str, path: &Path) -> Result<DeliveryReceipt> {
        let artifact: Value = serde_json::from_slice(&fs::read(path)?)?;
        let attempt = artifact.get("attempt").unwrap_or(&artifact);
        self.delivered(
            page_id,
            attempt["request_json"]
                .as_str()
                .context("retained request missing")?,
            attempt["response_json"]
                .as_str()
                .context("retained response missing")?,
        )
    }

    fn recover(&self, state: &mut State) -> Result<()> {
        self.recover_navigation(state)?;
        let Some(page) = state.pending.as_ref() else {
            return Ok(());
        };
        let directory = self.directory.join("deliveries").join(&page.id);
        if !directory.exists() {
            return Ok(());
        }
        for entry in fs::read_dir(directory)? {
            let path = entry?.path();
            if path.extension().is_none_or(|extension| extension != "json") {
                continue;
            }
            let bytes = fs::read(&path)?;
            let artifact: Value = serde_json::from_slice(&bytes)?;
            let hash = digest(&bytes);
            if path.file_stem().and_then(|name| name.to_str()) != Some(hash.as_str()) {
                bail!("retained source delivery artifact was modified");
            }
            let recorded: Page = serde_json::from_value(artifact["page"].clone())?;
            if &recorded != page {
                bail!("retained source delivery page mismatch");
            }
            let request = artifact["request_json"]
                .as_str()
                .context("retained request")?;
            let response = artifact["response_json"]
                .as_str()
                .context("retained response")?;
            verify_wire(page, request, response)?;
            let receipt = DeliveryReceipt {
                page_id: page.id.clone(),
                request_sha256: digest(request),
                response_sha256: digest(response),
                artifact_path: path,
                artifact_sha256: hash,
            };
            let page = page.clone();
            record_study(state, &page, response)?;
            state.current = Some(page.source.clone());
            state.bookmarks.insert(page.source.clone(), page.clone());
            state.receipts.insert(page.id.clone(), receipt);
            state.pending = None;
            self.save(state)?;
            break;
        }
        Ok(())
    }

    fn recover_navigation(&self, state: &mut State) -> Result<()> {
        let Some(output) = state.pending_navigation.as_ref() else {
            return Ok(());
        };
        let id = output
            .navigation_id
            .as_ref()
            .context("pending navigation identity missing")?;
        let directory = self.directory.join("navigation").join(id);
        if !directory.exists() {
            return Ok(());
        }
        for entry in fs::read_dir(directory)? {
            let path = entry?.path();
            if path.extension().is_none_or(|extension| extension != "json") {
                continue;
            }
            let bytes = fs::read(&path)?;
            let hash = digest(&bytes);
            let value: Value = serde_json::from_slice(&bytes)?;
            let recorded: StudyOutput = serde_json::from_value(value["output"].clone())?;
            if path.file_stem().and_then(|name| name.to_str()) != Some(hash.as_str())
                || &recorded != output
            {
                bail!("retained navigation artifact mismatch");
            }
            let request = value["request_json"]
                .as_str()
                .context("retained navigation request")?;
            let response = value["response_json"]
                .as_str()
                .context("retained navigation response")?;
            verify_text(&output.text, request, response)?;
            state.last_navigation = Some(DeliveryReceipt {
                page_id: id.clone(),
                request_sha256: digest(request),
                response_sha256: digest(response),
                artifact_path: path,
                artifact_sha256: hash,
            });
            state
                .notebook
                .record(response, &completion_text(response)?, None);
            state.pending_navigation = None;
            self.save(state)?;
            break;
        }
        Ok(())
    }

    fn lock(&self) -> Result<File> {
        fs::create_dir_all(&self.directory)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            fs::set_permissions(&self.directory, fs::Permissions::from_mode(0o700))?;
        }
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(self.directory.join("reader.lock"))?;
        file.lock_exclusive()?;
        Ok(file)
    }
    fn load(&self) -> Result<State> {
        let path = self.directory.join("reader-v1.json");
        if !path.exists() {
            return Ok(State {
                version: SCHEMA_VERSION,
                ..State::default()
            });
        }
        let state: State = serde_json::from_slice(&fs::read(path)?).context(
            "source-study state is unreadable; preserving it instead of resetting progress",
        )?;
        if state.version != SCHEMA_VERSION {
            bail!("unsupported source-study checkpoint version");
        }
        Ok(state)
    }
    fn save(&self, state: &State) -> Result<()> {
        atomic_write(
            &self.directory.join("reader-v1.json"),
            &serde_json::to_vec_pretty(state)?,
        )
    }
}

fn verify_wire(page: &Page, request_json: &str, response_json: &str) -> Result<()> {
    verify_text(&page.text, request_json, response_json)
}

fn verify_text(text: &str, request_json: &str, response_json: &str) -> Result<()> {
    let request: Value = serde_json::from_str(request_json)?;
    let response: Value = serde_json::from_str(response_json)?;
    let present = request["messages"].as_array().is_some_and(|messages| {
        messages.iter().any(|m| {
            m["role"] == "user"
                && m["content"]
                    .as_str()
                    .is_some_and(|content| content.contains(text))
        })
    });
    if !present {
        bail!(
            "prepared source page was missing or shortened in the submitted request; bookmark unchanged"
        );
    }
    let completion = response
        .pointer("/choices/0/message/content")
        .or_else(|| response.pointer("/message/content"))
        .and_then(Value::as_str);
    if completion.is_none_or(|text| text.trim().is_empty())
        || response.get("error").is_some()
        || response.get("done") == Some(&Value::Bool(false))
        || response
            .get("done_reason")
            .is_some_and(|reason| reason == "length")
        || response
            .pointer("/choices/0/finish_reason")
            .is_some_and(|reason| reason == "length" || reason == "content_filter")
    {
        bail!("provider did not retain a completed generation; bookmark unchanged");
    }
    Ok(())
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    let temp = path.with_extension(format!("{}.tmp", std::process::id()));
    let mut options = OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600);
    }
    let mut file = options.open(&temp)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    fs::rename(&temp, path)?;
    File::open(path.parent().context("checkpoint directory")?)?.sync_all()?;
    Ok(())
}

fn completion_text(response: &str) -> Result<String> {
    let value: Value = serde_json::from_str(response)?;
    Ok(value
        .pointer("/choices/0/message/content")
        .or_else(|| value.pointer("/message/content"))
        .and_then(Value::as_str)
        .context("completed response text missing")?
        .to_owned())
}

fn record_study(state: &mut State, page: &Page, response: &str) -> Result<()> {
    state.pending_navigation = None;
    progress::record(state.progress.get_or_insert_with(Progress::new), page);
    state
        .notebook
        .record(response, &completion_text(response)?, Some(page));
    Ok(())
}
