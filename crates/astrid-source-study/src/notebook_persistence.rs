//! Additive findings survive an older helper rewriting its known reader fields.
//! The caller holds reader.lock throughout restoration, mutation and both saves.
use crate::notebook_findings::Findings;
use anyhow::{Context as _, Result, bail};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{collections::BTreeMap, fs, io::Read as _, path::Path};

const FILE: &str = "source-findings-v1.json";
const SCHEMA: &str = "source_findings_sidecar_v1";
const MAX_BYTES: u64 = 4 * 1024 * 1024;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Sidecar {
    schema: String,
    /// Null is an explicit empty value, not an invitation to revive inline data.
    notebooks: BTreeMap<String, Value>,
}

/// Restore only source findings; authored notes, prose, choices and offers stay intact.
pub(crate) fn restore(directory: &Path, state: &mut Value) -> Result<()> {
    let Some(sidecar) = read(directory)? else {
        return Ok(());
    };
    let current = extract(state)?;
    // QUESTION NEW can save a null owner here just before its reader checkpoint
    // fails. That owner has no findings to protect: ignore it and prune on save.
    for (owner, findings) in &sidecar.notebooks {
        if !current.contains_key(owner) && !findings.is_null() {
            bail!("source findings name a missing inquiry; preserving both checkpoints");
        }
    }
    let active = active(state)?.map(str::to_owned);
    if let Some(questions) = state.get_mut("questions") {
        if let Some(entries) = questions.get_mut("entries").and_then(Value::as_object_mut) {
            for (id, inquiry) in entries {
                if let Some(findings) = sidecar.notebooks.get(id) {
                    set_findings(
                        inquiry
                            .get_mut("notebook")
                            .context("inquiry notebook missing")?,
                        findings,
                    )?;
                }
            }
        }
        if let Some(home) = questions
            .get_mut("unthreaded")
            .filter(|value| value.is_object())
            && let Some(findings) = sidecar.notebooks.get("home")
        {
            set_findings(home, findings)?;
        }
    }
    // The global notebook is a display/current-recording mirror of its owner.
    // Even a newly introduced inquiry without a sidecar entry must not receive
    // findings from the formerly active question or from home.
    let findings = if let Some(id) = active {
        state
            .get("questions")
            .and_then(|questions| questions.get("entries"))
            .and_then(|entries| entries.get(&id))
            .and_then(|inquiry| inquiry.get("notebook"))
            .and_then(|notebook| notebook.get("source_findings"))
            .cloned()
            .unwrap_or(Value::Null)
    } else {
        sidecar
            .notebooks
            .get("home")
            .cloned()
            .unwrap_or_else(|| current["home"].clone())
    };
    if let Some(notebook) = state.get_mut("notebook") {
        set_findings(notebook, &findings)?;
    } else if !findings.is_null() {
        bail!("source findings have no home notebook; preserving both checkpoints");
    }
    Ok(())
}

/// Persist the new findings before replacing reader-v1.json, under the same lock.
/// An older writer cannot erase this file. A failed subsequent reader save leaves
/// these receipt-gated findings authoritative, including explicit removals.
pub(crate) fn save(directory: &Path, state: &Value) -> Result<()> {
    let notebooks = extract(state)?;
    if let Some(previous) = read(directory)?
        && previous
            .notebooks
            .iter()
            .any(|(id, findings)| !notebooks.contains_key(id) && !findings.is_null())
    {
        bail!("source findings inquiry disappeared; preserving both checkpoints");
    }
    let sidecar = Sidecar {
        schema: SCHEMA.into(),
        notebooks,
    };
    let bytes = serde_json::to_vec_pretty(&sidecar)?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > MAX_BYTES {
        bail!("source findings sidecar exceeds its bound; checkpoint unchanged");
    }
    crate::store::atomic_write(&directory.join(FILE), &bytes)
}

fn active(state: &Value) -> Result<Option<&str>> {
    match state
        .get("questions")
        .and_then(|questions| questions.get("active"))
    {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(id)) if valid_owner(id) && id != "home" => Ok(Some(id)),
        _ => bail!("invalid active inquiry in source findings checkpoint"),
    }
}

fn extract(state: &Value) -> Result<BTreeMap<String, Value>> {
    let active = active(state)?;
    let mut notebooks = BTreeMap::new();
    let home = if active.is_some() {
        state
            .get("questions")
            .and_then(|questions| questions.get("unthreaded"))
    } else {
        state.get("notebook")
    };
    notebooks.insert("home".into(), finding_value(home)?);
    if let Some(entries) = state
        .get("questions")
        .and_then(|questions| questions.get("entries"))
    {
        let entries = entries.as_object().context("invalid inquiry entries")?;
        if entries.len() > 32 {
            bail!("source findings exceed the retained inquiry bound");
        }
        for (id, inquiry) in entries {
            if !valid_owner(id) || id == "home" {
                bail!("invalid source findings inquiry identity");
            }
            notebooks.insert(id.clone(), finding_value(inquiry.get("notebook"))?);
        }
    }
    if let Some(id) = active
        && !notebooks.contains_key(id)
    {
        bail!("active source findings inquiry does not exist");
    }
    Ok(notebooks)
}

fn finding_value(notebook: Option<&Value>) -> Result<Value> {
    let findings = notebook
        .and_then(|notebook| notebook.get("source_findings"))
        .cloned()
        .unwrap_or(Value::Null);
    validate_findings(&findings)?;
    Ok(findings)
}

fn validate_findings(value: &Value) -> Result<()> {
    if !value.is_null() {
        let _: Findings = serde_json::from_value(value.clone())
            .context("invalid persisted source findings; preserving checkpoint")?;
    }
    Ok(())
}

fn set_findings(notebook: &mut Value, findings: &Value) -> Result<()> {
    let notebook = notebook
        .as_object_mut()
        .context("invalid persisted notebook")?;
    if findings.is_null() {
        notebook.remove("source_findings");
    } else {
        notebook.insert("source_findings".into(), findings.clone());
    }
    Ok(())
}

fn valid_owner(owner: &str) -> bool {
    owner == "home"
        || owner.strip_prefix('q').is_some_and(|digits| {
            !digits.is_empty()
                && digits.len() <= 20
                && digits.bytes().all(|byte| byte.is_ascii_digit())
        })
}

fn read(directory: &Path) -> Result<Option<Sidecar>> {
    let path = directory.join(FILE);
    let metadata = match fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    if !metadata.file_type().is_file() || metadata.len() > MAX_BYTES {
        bail!("source findings sidecar is nonregular or oversized; preserving checkpoint");
    }
    let mut bytes = Vec::new();
    fs::File::open(&path)?
        .take(MAX_BYTES.saturating_add(1))
        .read_to_end(&mut bytes)?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > MAX_BYTES {
        bail!("source findings sidecar grew beyond its bound; preserving checkpoint");
    }
    let sidecar: Sidecar = serde_json::from_slice(&bytes)
        .context("source findings sidecar is unreadable; preserving checkpoint")?;
    if sidecar.schema != SCHEMA
        || sidecar.notebooks.len() > 33
        || !sidecar.notebooks.contains_key("home")
    {
        bail!("unsupported source findings sidecar; preserving checkpoint");
    }
    for (owner, findings) in &sidecar.notebooks {
        if !valid_owner(owner) {
            bail!("invalid source findings owner; preserving checkpoint");
        }
        validate_findings(findings)?;
    }
    Ok(Some(sidecar))
}
