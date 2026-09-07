#[derive(Debug)]
pub struct ContinuityInputError(pub &'static str);

impl std::fmt::Display for ContinuityInputError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0)
    }
}

impl std::error::Error for ContinuityInputError {}

fn session_is_quiet(session: &Value) -> bool {
    matches!(
        session.get("status").and_then(Value::as_str),
        Some("parked" | "held" | "complete" | "abandoned")
    )
}

fn latest_active_session(rows: &[Value]) -> Option<&Value> {
    let mut seen = HashSet::new();
    rows.iter().rev().find(|row| {
        row.get("session_id")
            .and_then(Value::as_str)
            .is_some_and(|id| seen.insert(id))
            && matches!(
                row.get("status").and_then(Value::as_str),
                Some("active" | "summarized")
            )
    })
}

fn resolve_session_reference<'a>(rows: &'a [Value], target: &str) -> Option<&'a Value> {
    let record = rows.iter().rev().find(|row| {
        row.get("session_id").and_then(Value::as_str) == Some(target)
            || row.get("record_id").and_then(Value::as_str) == Some(target)
    })?;
    let Some(session_id) = record.get("session_id").and_then(Value::as_str) else {
        return Some(record);
    };
    // A historical record identifies the session without restoring its old state.
    rows.iter()
        .rev()
        .find(|row| row.get("session_id").and_then(Value::as_str) == Some(session_id))
}

fn session_authored_summary(payload: &str) -> Result<String> {
    let summary = dossier_field(payload, &["summary", "note", "memory"]).or_else(|| {
        let has_fields = payload.split([';', '\n']).any(|part| {
            part.trim_start().split_once(':').is_some_and(|(label, _)| {
                !label.is_empty()
                    && label
                        .chars()
                        .all(|c| c.is_ascii_alphabetic() || matches!(c, '_' | '-' | ' '))
            })
        });
        (!has_fields).then(|| payload.trim().to_string())
    });
    match summary {
        Some(summary)
            if !matches!(
                summary.trim(),
                "" | "..." | "\u{2026}" | "<summary>" | "<note>"
            ) =>
        {
            Ok(summary)
        },
        _ => Err(
            ContinuityInputError("An authored summary is required. Nothing was captured.").into(),
        ),
    }
}

fn session_preserved_list(
    payload: &str,
    labels: &[&str],
    session: &Value,
    key: &str,
) -> Vec<String> {
    let values = dossier_list_field(payload, labels);
    if values.is_empty() {
        value_string_list(session.get(key))
    } else {
        values
    }
}

fn session_preserved_next(payload: &str, session: &Value) -> Option<String> {
    dossier_field(payload, &["next", "next_safe_command"]).or_else(|| {
        session
            .get("suggested_next")
            .and_then(Value::as_str)
            .map(str::to_string)
    })
}

impl NextActionOutcome {
    pub fn continuity_result(result: Result<String>, visibility: &str) -> Self {
        match result {
            Ok(message) => Self::handled("action_continuity", message)
                .with_stage_visibility("read_only", visibility),
            Err(error) => {
                let needs_input = error.downcast_ref::<ContinuityInputError>().is_some();
                let mut outcome = Self::blocked("action_continuity", format!("{error:#}"))
                    .with_stage_visibility(
                        if needs_input {
                            "needs_input"
                        } else {
                            "blocked"
                        },
                        visibility,
                    );
                if needs_input {
                    outcome.status = "needs_input".to_string();
                }
                outcome
            },
        }
    }
}
