fn peer_recent_runs(path: &Path, experiment_id: &str) -> Vec<String> {
    let Ok(raw) = fs::read_to_string(path) else {
        return Vec::new();
    };
    let mut rows = raw
        .lines()
        .rev()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter(|value| value.get("experiment_id").and_then(Value::as_str) == Some(experiment_id))
        .take(3)
        .map(|value| {
            format!(
                "- {} [{} / {}]: {}",
                value
                    .get("action_text")
                    .and_then(Value::as_str)
                    .unwrap_or("(unknown action)"),
                value
                    .get("stage")
                    .and_then(Value::as_str)
                    .unwrap_or("(unknown stage)"),
                value
                    .get("status")
                    .and_then(Value::as_str)
                    .unwrap_or("(unknown status)"),
                value
                    .get("result_summary")
                    .and_then(Value::as_str)
                    .unwrap_or("")
            )
        })
        .collect::<Vec<_>>();
    rows.reverse();
    rows
}

fn iso_now() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

fn now_millis() -> u64 {
    u64::try_from(chrono::Utc::now().timestamp_millis()).unwrap_or_default()
}

fn peer_system_from_experiment_id(experiment_id: &str) -> String {
    if experiment_id.starts_with("exp_minime_") {
        "minime".to_string()
    } else if experiment_id.starts_with("exp_astrid_") {
        "astrid".to_string()
    } else {
        "peer".to_string()
    }
}

fn peer_workspace_dir(peer_system: &str) -> PathBuf {
    if peer_system == "minime" {
        bridge_paths().minime_workspace().to_path_buf()
    } else {
        bridge_paths().bridge_workspace().to_path_buf()
    }
}

fn shared_investigation_lane(system: &str) -> &'static str {
    match system {
        "astrid" => "felt texture, motif continuity, language thread, artifact grounding",
        "minime" => {
            "spectral condition, fill/pressure state, recurrence pattern, artifact grounding"
        },
        _ => "native evidence lane",
    }
}

fn shared_investigation_authority_boundary() -> &'static str {
    "read-mostly shared continuity; allowed local lifecycle decisions are pause, hold, and charter_repair; no peer mutation, bind, resume, perturb, sensory, or control authority"
}

fn shared_investigation_sort_ts(row: &Value) -> u64 {
    row.get("updated_t_ms")
        .or_else(|| row.get("created_t_ms"))
        .and_then(Value::as_u64)
        .unwrap_or_default()
}

fn local_participant_for_investigation(investigation: &Value, system: &str) -> Option<Value> {
    investigation
        .get("participants")
        .and_then(Value::as_array)?
        .iter()
        .find(|participant| participant.get("being").and_then(Value::as_str) == Some(system))
        .cloned()
}

fn parse_shared_investigation_decision(raw: &str) -> (String, String) {
    let text = raw.trim();
    let lowered = text.to_ascii_lowercase();
    let decision = if lowered.starts_with("charter_repair") || lowered.starts_with("charter repair")
    {
        "charter_repair"
    } else if lowered.starts_with("hold") {
        "hold"
    } else {
        "pause"
    };
    let reason = text
        .trim_start_matches("charter_repair")
        .trim_start_matches("charter repair")
        .trim_start_matches("pause")
        .trim_start_matches("hold")
        .trim_start()
        .strip_prefix("because")
        .unwrap_or(text)
        .trim()
        .to_string();
    (
        decision.to_string(),
        if reason.is_empty() {
            "shared investigation decision".to_string()
        } else {
            reason
        },
    )
}

fn push_recent(recent: &mut VecDeque<String>, thread_id: String) {
    recent.retain(|existing| existing != &thread_id);
    recent.push_front(thread_id);
    while recent.len() > 16 {
        let _ = recent.pop_back();
    }
}

fn sanitize_slug(input: &str) -> String {
    let mut out = String::new();
    let mut last_dash = false;
    for ch in input.chars().flat_map(char::to_lowercase) {
        if ch.is_ascii_alphanumeric() {
            out.push(ch);
            last_dash = false;
        } else if !last_dash && !out.is_empty() {
            out.push('-');
            last_dash = true;
        }
        if out.len() >= 48 {
            break;
        }
    }
    while out.ends_with('-') {
        let _ = out.pop();
    }
    if out.is_empty() {
        "untitled".to_string()
    } else {
        out
    }
}
