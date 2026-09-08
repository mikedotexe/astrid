use crate::{Catalog, Command, Page, SCHEMA_VERSION, digest};
use anyhow::{Context as _, Result, bail};
use fs2::FileExt as _;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::io::Write as _;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StudyOutput {
    pub system_prompt: String,
    pub text: String,
    pub page: Option<Page>,
}
impl StudyOutput {
    fn navigation(text: String) -> Self {
        Self {
            system_prompt: crate::STUDY_PROMPT.into(),
            text,
            page: None,
        }
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

    /// Prepare a page without claiming it was delivered.
    /// # Errors
    /// Returns source, navigation, or checkpoint errors without resetting progress.
    pub fn prepare(&self, command: Command) -> Result<StudyOutput> {
        let _lock = self.lock()?;
        let mut state = self.load()?;
        self.recover(&mut state)?;
        let mut page = match command {
            Command::Map { topic, page } => {
                return Ok(StudyOutput::navigation(self.catalog.map(&topic, page)?));
            },
            Command::Find { query, page } => {
                return Ok(StudyOutput::navigation(self.catalog.find(&query, page)?));
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
                    return Ok(StudyOutput {
                        system_prompt: crate::STUDY_PROMPT.into(),
                        text: page.text.clone(),
                        page: Some(page.clone()),
                    });
                }
                if let Some(last) = state.bookmarks.get(&source.id) {
                    if last.eof {
                        Page::read(&source, Some(&last.end), 1, Some(&last.revision.sha256))?;
                        return Ok(StudyOutput {
                            system_prompt: crate::STUDY_PROMPT.into(),
                            text: format!(
                                "End of {}. Use SELF_STUDY OPEN {} 1 to reread or SELF_STUDY MAP to choose another source.",
                                source.id, source.id
                            ),
                            page: None,
                        });
                    }
                    Page::read(&source, Some(&last.end), 1, Some(&last.revision.sha256))?
                } else {
                    Page::read(&source, None, 1, None)?
                }
            },
            Command::Continue => {
                if let Some(page) = state.pending {
                    return Ok(StudyOutput {
                        system_prompt: crate::STUDY_PROMPT.into(),
                        text: page.text.clone(),
                        page: Some(page),
                    });
                }
                let Some(last) = state
                    .current
                    .as_ref()
                    .and_then(|id| state.bookmarks.get(id))
                else {
                    return Ok(StudyOutput {
                        system_prompt: crate::STUDY_PROMPT.into(),
                        text: self.catalog.map("", 1)?,
                        page: None,
                    });
                };
                if last.eof {
                    Page::read(
                        &self.catalog.resolve(&last.source)?,
                        Some(&last.end),
                        1,
                        Some(&last.revision.sha256),
                    )?;
                    return Ok(StudyOutput {
                        system_prompt: crate::STUDY_PROMPT.into(),
                        text: format!(
                            "End of {} at revision {}. All pages marked delivered have retained request evidence; this does not establish understanding. Choose SELF_STUDY MAP, FIND, or OPEN.",
                            last.source, last.revision.sha256
                        ),
                        page: None,
                    });
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
        Ok(StudyOutput {
            system_prompt: crate::STUDY_PROMPT.into(),
            text: page.text.clone(),
            page: Some(page),
        })
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
        if let Some(receipt) = state.receipts.get(page_id).cloned() {
            if state
                .pending
                .as_ref()
                .is_some_and(|page| page.id == page_id)
            {
                let page = state.pending.take().context("pending page")?;
                verify_wire(&page, request_json, response_json)?;
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
            state.current = Some(page.source.clone());
            state.bookmarks.insert(page.source.clone(), page.clone());
            state.receipts.insert(page.id.clone(), receipt);
            state.pending = None;
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
    let request: Value = serde_json::from_str(request_json)?;
    let response: Value = serde_json::from_str(response_json)?;
    let present = request["messages"].as_array().is_some_and(|messages| {
        messages.iter().any(|m| {
            m["role"] == "user"
                && m["content"]
                    .as_str()
                    .is_some_and(|content| content.contains(&page.text))
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
