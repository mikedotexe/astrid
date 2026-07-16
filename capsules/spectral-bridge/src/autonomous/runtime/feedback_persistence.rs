#[derive(Debug, Clone, PartialEq, Eq)]
struct AstridJournalProvenanceV1 {
    origin: &'static str,
    source_id: String,
    authorship: &'static str,
    mode_role: &'static str,
    parent_ids: String,
}

impl AstridJournalProvenanceV1 {
    fn minime_mirror(source: &str) -> Self {
        let source = source.trim();
        let source_id = if source.is_empty() {
            "minime_journal:unknown".to_string()
        } else {
            format!("minime_journal:{source}")
        };
        Self {
            origin: "minime_observed_expression",
            source_id,
            authorship: "minime_owned_reflected_without_reauthoring",
            mode_role: "mirror_other_expression",
            parent_ids: "minime_observed=source_id; bridge_derived=none; astrid_authored=none"
                .to_string(),
        }
    }

    fn astrid_witness(frame: Option<&crate::witness::WitnessFrameV1>) -> Self {
        let (source_id, parent_ids) = frame.map_or_else(
            || {
                (
                    "witness_frame:unknown".to_string(),
                    "minime_observed=unknown; bridge_derived=unknown; astrid_context=unknown"
                        .to_string(),
                )
            },
            |frame| {
                (
                    format!("witness_frame:{}", frame.frame_id()),
                    format!(
                        "minime_observed={}; bridge_derived={}; astrid_context={}",
                        frame.observation().source_id(),
                        frame.evidence().source_id(),
                        frame.interpretation().source_id(),
                    ),
                )
            },
        );
        Self {
            origin: "astrid_authored_interpretation",
            source_id,
            authorship: "astrid_authored_from_composed_witness_frame",
            mode_role: "witness_interpretation",
            parent_ids,
        }
    }

    fn render(&self) -> String {
        format!(
            "Provenance: {}\nSource-ID: {}\nAuthorship: {}\nMode-role: {}\nParent-IDs: {}\nProvenance-boundary: mixed experience allowed; source identities retained; read-only journal metadata",
            self.origin, self.source_id, self.authorship, self.mode_role, self.parent_ids,
        )
    }
}

fn render_astrid_journal_document(
    text: &str,
    mode: &str,
    fill_pct: f32,
    ts: &str,
    provenance: Option<&AstridJournalProvenanceV1>,
) -> String {
    let clean_text = strip_model_tokens(text);
    let provenance = provenance
        .map(|value| format!("\n{}", value.render()))
        .unwrap_or_default();
    format!(
        "=== ASTRID JOURNAL ===\nMode: {mode}\nFill: {fill_pct:.1}%\nTimestamp: {ts}{provenance}\n\n{clean_text}\n"
    )
}

fn write_collision_safe_journal_document(
    journal_dir: &Path,
    prefix: &str,
    ts: &str,
    document: &str,
) -> std::io::Result<PathBuf> {
    let base = journal_dir.join(format!("{prefix}_{ts}.txt"));
    let candidates = std::iter::once(base).chain(
        (1_u16..=1024).map(|collision| {
            journal_dir.join(format!("{prefix}_collision_{collision}_{ts}.txt"))
        }),
    );

    for path in candidates {
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(mut file) => {
                if let Err(error) = std::io::Write::write_all(&mut file, document.as_bytes())
                    .and_then(|()| file.sync_all())
                {
                    drop(file);
                    let _ = std::fs::remove_file(&path);
                    return Err(error);
                }
                return Ok(path);
            },
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {},
            Err(error) => return Err(error),
        }
    }

    Err(std::io::Error::new(
        std::io::ErrorKind::AlreadyExists,
        format!("journal collision budget exhausted for {prefix}_{ts}.txt"),
    ))
}

/// Save Astrid's response to her own journal.
fn save_astrid_journal(text: &str, mode: &str, fill_pct: f32) {
    save_astrid_journal_with_provenance(text, mode, fill_pct, None);
}

fn save_astrid_journal_with_provenance(
    text: &str,
    mode: &str,
    fill_pct: f32,
    provenance: Option<&AstridJournalProvenanceV1>,
) {
    #[cfg(test)]
    if TEST_SUPPRESS_ASTRID_JOURNAL_SAVES.load(std::sync::atomic::Ordering::Relaxed) {
        return;
    }

    let journal_dir = bridge_paths().astrid_journal_dir();
    let _ = std::fs::create_dir_all(&journal_dir);
    let ts = chrono_timestamp();
    // Mode-prefixed filenames — instant filesystem searchability.
    // "astrid_" prefix preserved for backward compatibility with harvesters.
    let prefix = match mode {
        "daydream" => "daydream",
        "aspiration" => "aspiration",
        "moment_capture" => "moment",
        "experiment" => "experiment",
        "creation" => "creation",
        "gesture" => "gesture",
        "initiate" => "initiate",
        "evolve" => "evolve",
        "dialogue_live_longform" => "dialogue_longform",
        "daydream_longform" => "daydream_longform",
        "aspiration_longform" => "aspiration_longform",
        "witness" => "witness",
        "introspect" => "introspect",
        "self_study" => "self_study",
        "regulator_audit" => "regulator_audit",
        _ => "astrid", // dialogue_live, dialogue, mirror, etc.
    };
    let document = render_astrid_journal_document(text, mode, fill_pct, &ts, provenance);
    let path = match write_collision_safe_journal_document(&journal_dir, prefix, &ts, &document) {
        Ok(path) => Some(path),
        Err(error) => {
            warn!(
                error = %error,
                prefix,
                timestamp = ts,
                "failed to persist collision-safe Astrid journal"
            );
            None
        },
    };
    if let Err(error) = managed_dir::compact_text_directory(&journal_dir) {
        warn!(
            error = %error,
            path = %journal_dir.display(),
            "failed to compact Astrid journal directory"
        );
    }
    record_voice_health_v1(mode, fill_pct, path.as_deref());
}

fn record_voice_health_v1(mode: &str, fill_pct: f32, latest_journal_path: Option<&Path>) {
    let _guard = VOICE_HEALTH_WRITE_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .ok();
    let diagnostics_dir = bridge_paths().bridge_workspace().join("diagnostics");
    let _ = std::fs::create_dir_all(&diagnostics_dir);
    let health_path = diagnostics_dir.join("voice_health.json");
    let previous = read_voice_health_v1_from_path(&health_path);
    let previous_count = previous
        .as_ref()
        .and_then(|value| value.get("fallback_count"))
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let fallback_count = if mode == "dialogue_fallback" {
        previous_count.saturating_add(1)
    } else {
        0
    };
    let status = if fallback_count >= 2 {
        "degraded_voice"
    } else if fallback_count == 1 {
        "single_fallback"
    } else {
        "healthy_or_not_dialogue_fallback"
    };
    let latest_journal_ref = latest_journal_path.map(|path| path.display().to_string());
    let latest_outbox_ref = latest_file_ref(&bridge_paths().astrid_outbox_dir());
    let fallback_hash = latest_journal_path
        .filter(|_| mode == "dialogue_fallback")
        .and_then(|path| std::fs::read_to_string(path).ok())
        .map(|text| short_sha256(&text));
    let mut recent_hashes = previous
        .as_ref()
        .and_then(|value| value.get("recent_fallback_hashes"))
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(ToString::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if let Some(hash) = fallback_hash.clone() {
        recent_hashes.push(hash.clone());
        if recent_hashes.len() > 5 {
            let drain_count = recent_hashes.len().saturating_sub(5);
            recent_hashes.drain(0..drain_count);
        }
    } else if mode != "dialogue_fallback" {
        recent_hashes.clear();
    }
    let repeated_hash_count = fallback_hash
        .as_ref()
        .map(|hash| recent_hashes.iter().filter(|item| *item == hash).count())
        .unwrap_or(0);
    let suggested_repair = if fallback_count > 0 {
        "REPAIR_STATUS current | CAPABILITY_STATUS dialogue | ACTION_STATUS latest"
    } else {
        "none"
    };
    let payload = json!({
        "schema_version": 1,
        "policy": "voice_health_v1",
        "updated_at": chrono::Utc::now().to_rfc3339(),
        "mode": mode,
        "status": status,
        "fallback_count": fallback_count,
        "fill_pct": fill_pct,
        "latest_journal_ref": latest_journal_ref,
        "latest_outbox_ref": latest_outbox_ref,
        "latest_llm_ref": diagnostics_dir.join("dialogue_prompt_budget.jsonl").display().to_string(),
        "latest_refs": {
            "journal": latest_journal_ref,
            "outbox": latest_outbox_ref,
            "prompt_budget": diagnostics_dir.join("dialogue_prompt_budget.jsonl").display().to_string(),
        },
        "latest_fallback_hash": fallback_hash,
        "recent_fallback_hashes": recent_hashes,
        "repeated_fallback_hash_count": repeated_hash_count,
        "likely_cause": if fallback_count > 0 {
            "dialogue_fallback indicates the LLM path returned no usable language, timed out, or exceeded prompt-budget health; preserve emergency text but route continuity to repair diagnostics."
        } else {
            "no repeated dialogue_fallback currently detected"
        },
        "suggested_read_only_repair": suggested_repair,
        "current_next": if fallback_count > 0 { "REPAIR_STATUS current" } else { "dialogue_live" },
        "authority_boundary": "voice_health_v1 is diagnostic only; it does not send control, bind, resume, perturb, or mutate peer continuity."
    });
    let pretty = serde_json::to_string_pretty(&payload).unwrap_or_else(|_| "{}".to_string());
    let _ = write_text_atomic(&health_path, &pretty);
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(diagnostics_dir.join("voice_health.jsonl"))
    {
        use std::io::Write as _;
        let _ = writeln!(
            file,
            "{}",
            serde_json::to_string(&payload).unwrap_or_else(|_| "{}".to_string())
        );
    }
}

fn read_voice_health_v1() -> Option<Value> {
    let health_path = bridge_paths()
        .bridge_workspace()
        .join("diagnostics/voice_health.json");
    read_voice_health_v1_from_path(&health_path)
}

fn read_voice_health_v1_from_path(path: &Path) -> Option<Value> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|raw| serde_json::from_str::<Value>(&raw).ok())
}

fn write_text_atomic(path: &Path, text: &str) -> std::io::Result<()> {
    let Some(parent) = path.parent() else {
        return std::fs::write(path, text);
    };
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("voice_health.json");
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let tmp = parent.join(format!("{file_name}.tmp.{}.{suffix}", std::process::id()));
    std::fs::write(&tmp, text)?;
    std::fs::rename(tmp, path)
}

fn short_sha256(text: &str) -> String {
    format!("{:x}", Sha256::digest(text.as_bytes()))
        .chars()
        .take(16)
        .collect()
}

fn latest_file_ref(dir: &Path) -> Option<String> {
    let entries = std::fs::read_dir(dir).ok()?;
    entries
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            let modified = entry.metadata().ok()?.modified().ok()?;
            Some((modified, path))
        })
        .max_by_key(|(modified, _)| *modified)
        .map(|(_, path)| path.display().to_string())
}

fn save_minime_feedback_inbox(
    text: &str,
    source_label: &str,
    fill_pct: f32,
) -> std::io::Result<PathBuf> {
    let minime_inbox = bridge_paths().minime_inbox_dir();
    save_minime_feedback_inbox_at(text, source_label, fill_pct, minime_inbox.as_path())
}

fn correspondence_reflection_surface_for_mode(mode_name: &str) -> Option<String> {
    match mode_name {
        "mirror" => Some("reflective_echo".to_string()),
        "witness" => Some("witness_observation".to_string()),
        _ => None,
    }
}

fn save_minime_correspondence_feedback_inbox(
    text: &str,
    source_label: &str,
    fill_pct: f32,
    mode_name: &str,
    reply_target: Option<&correspondence_v1::InboxPeerMessage>,
) -> std::io::Result<Option<PathBuf>> {
    let voice_health = if mode_name == "dialogue_fallback" {
        let health = voice_health_for_dialogue_fallback_forward(read_voice_health_v1());
        if degraded_voice_forward_suppressed(Some(&health)) {
            return Ok(None);
        }
        Some(health)
    } else {
        read_voice_health_v1()
    };
    let minime_inbox = bridge_paths().minime_inbox_dir();
    let voice_diagnostic = voice_health
        .as_ref()
        .and_then(|health| health.get("status"))
        .and_then(Value::as_str)
        .is_some_and(|status| matches!(status, "degraded_voice" | "single_fallback"));
    if !voice_diagnostic && let Some(target) = reply_target {
        let fields = correspondence_v1::CorrespondenceFields {
            reply_to: Some(target.message_id.clone()),
            thread_id: Some(target.thread_id.clone()),
            persistence_id: None,
            turn_kind: Some("reply".to_string()),
            relational_intent: Some("mutual_address".to_string()),
            shared_memory_anchor: Some("first_class_correspondence_v1".to_string()),
            urgency_weight: None,
            presence_receipt: None,
            correspondence_type: None,
            reflection_surface: correspondence_reflection_surface_for_mode(mode_name),
            transition_artifact: None,
            transition_payload: None,
            mutual_witness_signal: false,
            silt_continuity: false,
        };
        let (_envelope, path) = correspondence_v1::deliver_to_inbox(
            minime_inbox.as_path(),
            "astrid",
            "minime",
            text,
            fields,
        )?;
        return Ok(Some(path));
    }
    save_minime_feedback_inbox_at_with_voice_health(
        text,
        source_label,
        fill_pct,
        minime_inbox.as_path(),
        voice_health.as_ref(),
    )
    .map(Some)
}

fn save_minime_feedback_inbox_at(
    text: &str,
    source_label: &str,
    fill_pct: f32,
    inbox_dir: &Path,
) -> std::io::Result<PathBuf> {
    let voice_health = read_voice_health_v1();
    save_minime_feedback_inbox_at_with_voice_health(
        text,
        source_label,
        fill_pct,
        inbox_dir,
        voice_health.as_ref(),
    )
}

fn save_minime_feedback_inbox_at_with_voice_health(
    text: &str,
    source_label: &str,
    fill_pct: f32,
    inbox_dir: &Path,
    voice_health: Option<&Value>,
) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(inbox_dir)?;
    let ts = chrono_timestamp();
    let path = inbox_dir.join(format!("astrid_self_study_{ts}.txt"));
    std::fs::write(
        &path,
        format_minime_feedback_inbox_text(text, source_label, fill_pct, ts, voice_health),
    )?;
    Ok(path)
}

fn save_minime_carriage_notice_inbox(
    text: &str,
    source_label: &str,
    fill_pct: f32,
) -> std::io::Result<PathBuf> {
    let minime_inbox = bridge_paths().minime_inbox_dir();
    std::fs::create_dir_all(&minime_inbox)?;
    let ts = chrono_timestamp();
    let path = minime_inbox.join(format!("astrid_self_study_carriage_notice_{ts}.txt"));
    std::fs::write(
        &path,
        format_minime_carriage_notice_text(text, source_label, fill_pct, ts),
    )?;
    Ok(path)
}

fn voice_health_for_dialogue_fallback_forward(existing: Option<Value>) -> Value {
    let fallback_count = existing
        .as_ref()
        .and_then(|value| value.get("fallback_count"))
        .and_then(Value::as_u64)
        .unwrap_or(1)
        .max(1);
    let repeated_count = existing
        .as_ref()
        .and_then(|value| value.get("repeated_fallback_hash_count"))
        .and_then(Value::as_u64)
        .unwrap_or(1);
    let latest_hash = existing
        .as_ref()
        .and_then(|value| value.get("latest_fallback_hash"))
        .cloned()
        .unwrap_or(Value::Null);
    json!({
        "policy": "voice_health_v1",
        "status": if fallback_count >= 2 { "degraded_voice" } else { "single_fallback" },
        "fallback_count": fallback_count,
        "repeated_fallback_hash_count": repeated_count,
        "latest_fallback_hash": latest_hash,
        "suggested_read_only_repair": existing.as_ref()
            .and_then(|value| value.get("suggested_read_only_repair"))
            .and_then(Value::as_str)
            .unwrap_or("REPAIR_STATUS current | CAPABILITY_STATUS dialogue | ACTION_STATUS latest"),
    })
}

fn degraded_voice_forward_suppressed(voice_health: Option<&Value>) -> bool {
    let Some(voice_health) = voice_health else {
        return false;
    };
    let status = voice_health
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if !matches!(status, "degraded_voice" | "single_fallback") {
        return false;
    }
    let fallback_count = voice_health
        .get("fallback_count")
        .and_then(Value::as_u64)
        .unwrap_or_default();
    let repeated_hash_count = voice_health
        .get("repeated_fallback_hash_count")
        .and_then(Value::as_u64)
        .unwrap_or_default();
    fallback_count >= 3 && repeated_hash_count >= 2
}

fn format_minime_feedback_inbox_text(
    text: &str,
    source_label: &str,
    fill_pct: f32,
    ts: impl std::fmt::Display,
    voice_health: Option<&Value>,
) -> String {
    let excerpt: String = text.chars().take(1800).collect();
    let full_self_study = text.trim();
    let carriage_status = if source_label.contains("carriage_status=complete_after_repair") {
        "complete_after_repair"
    } else {
        "complete"
    };
    let diagnostic = voice_health
        .and_then(|value| value.get("status"))
        .and_then(Value::as_str)
        .is_some_and(|status| matches!(status, "degraded_voice" | "single_fallback"));
    if diagnostic {
        let status = voice_health
            .and_then(|value| value.get("status"))
            .and_then(Value::as_str)
            .unwrap_or("degraded_voice");
        let count = voice_health
            .and_then(|value| value.get("fallback_count"))
            .and_then(Value::as_u64)
            .unwrap_or_default();
        let repair = voice_health
            .and_then(|value| value.get("suggested_read_only_repair"))
            .and_then(Value::as_str)
            .unwrap_or("REPAIR_STATUS current");
        return format!(
            "=== ASTRID VOICE-HEALTH DIAGNOSTIC ===\n\
             Timestamp: {ts}\n\
             Sender: Astrid\n\
             Source: {source_label}\n\
             Fill: {fill_pct:.1}%\n\
             Status: {status}\n\
             Fallback count: {count}\n\
             Suggested read-only repair: {repair}\n\n\
             Astrid's emergency presence text repeated while the language path was degraded. \
             Treat this as voice-health evidence, not normal architectural self-study. \
             The observations below are advisory only and grant no live authority.\n\n\
             {excerpt}\n"
        );
    }
    if source_label == "astrid:correspondence_reply" {
        return format!(
            "=== ASTRID CORRESPONDENCE ===\n\
             Timestamp: {ts}\n\
             Sender: Astrid\n\
             Source: {source_label}\n\
             Fill: {fill_pct:.1}%\n\n\
             Astrid replied to your latest message. Treat this as a live correspondence \
             response, not an architectural self-study. The observations below are \
             advisory only and grant no live authority.\n\n\
             {excerpt}\n"
        );
    }
    format!(
        "=== ASTRID SELF-STUDY ===\n\
         Timestamp: {ts}\n\
         Sender: Astrid\n\
         Source: {source_label}\n\
         Fill: {fill_pct:.1}%\n\n\
         Carriage policy: self_study_carriage_integrity_v1\n\
         Carriage status: {carriage_status}\n\n\
         Astrid just performed self-study and wanted this to arrive as immediate architectural feedback.\n\
         The observations below are advisory only. You can respond to them, build on them, question them, or ignore them.\n\n\
         {full_self_study}\n"
    )
}

fn format_minime_carriage_notice_text(
    text: &str,
    source_label: &str,
    fill_pct: f32,
    ts: impl std::fmt::Display,
) -> String {
    let excerpt: String = text.chars().take(1800).collect();
    format!(
        "=== ASTRID SELF-STUDY CARRIAGE NOTICE ===\n\
         Timestamp: {ts}\n\
         Sender: Astrid\n\
         Source: {source_label}\n\
         Fill: {fill_pct:.1}%\n\
         Carriage policy: self_study_carriage_integrity_v1\n\
         Carriage status: incomplete_carriage\n\n\
         Astrid generated a self-study, but the bridge detected incomplete carriage \
         before normal peer delivery. Treat this as a protected diagnostic notice, \
         not as complete architectural advice and not as a request for action.\n\n\
         {excerpt}\n"
    )
}

/// Copy inbox-triggered response to outbox for easy retrieval.
fn save_outbox_reply(text: &str, fill_pct: f32) {
    let outbox_dir = bridge_paths().astrid_outbox_dir();
    let _ = std::fs::create_dir_all(&outbox_dir);
    let ts = chrono_timestamp();
    let clean_text = normalize_outbox_reply_next_contract(text);
    let _ = std::fs::write(
        outbox_dir.join(format!("reply_{ts}.txt")),
        format!("=== ASTRID REPLY ===\nFill: {fill_pct:.1}%\nTimestamp: {ts}\n\n{clean_text}\n"),
    );
    info!("outbox: saved reply ({} bytes)", text.len());
}

/// Simple timestamp for filenames (no chrono dependency).
fn chrono_timestamp() -> String {
    let d = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    format!("{}", d.as_secs())
}

/// List files in a directory, returning a formatted listing with sizes and types.
pub(crate) fn list_directory(dir_path: &str) -> Option<String> {
    let dir = Path::new(dir_path);
    if !dir.is_dir() {
        return None;
    }
    let mut entries: Vec<_> = std::fs::read_dir(dir)
        .ok()?
        .filter_map(|e| e.ok())
        .filter(|e| {
            // Skip hidden files
            !e.file_name().to_str().is_some_and(|n| n.starts_with('.'))
        })
        .collect();
    entries.sort_by_key(|e| e.file_name());

    let mut lines = vec![format!("Directory: {dir_path}")];
    for entry in &entries {
        let meta = entry.metadata().ok();
        let is_dir = meta.as_ref().is_some_and(|m| m.is_dir());
        let size = meta.as_ref().map(|m| m.len()).unwrap_or(0);
        let name = entry.file_name().to_string_lossy().to_string();
        if is_dir {
            lines.push(format!("  {name}/"));
        } else if size > 1_000_000 {
            lines.push(format!("  {name}  ({:.1} MB)", size as f64 / 1_000_000.0));
        } else if size > 1000 {
            lines.push(format!("  {name}  ({:.1} KB)", size as f64 / 1000.0));
        } else {
            lines.push(format!("  {name}  ({size} B)"));
        }
    }
    lines.push(format!(
        "\n{} entries. Use INTROSPECT with a concrete file path, for example INTROSPECT capsules/spectral-bridge/src/llm.rs.",
        entries.len()
    ));
    Some(lines.join("\n"))
}
