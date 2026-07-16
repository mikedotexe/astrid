fn newest_prefixed_file_mtime(
    dir: &std::path::Path,
    prefixes: &[&str],
) -> Option<std::time::SystemTime> {
    let mut newest = None;
    for entry in std::fs::read_dir(dir).ok()?.filter_map(Result::ok) {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if !prefixes.iter().any(|prefix| name.starts_with(prefix)) {
            continue;
        }
        let Ok(metadata) = entry.metadata() else {
            continue;
        };
        if !metadata.is_file() {
            continue;
        }
        let Ok(modified) = metadata.modified() else {
            continue;
        };
        if newest.is_none_or(|current| modified > current) {
            newest = Some(modified);
        }
    }
    newest
}

fn latest_introspection_activity_mtime_from_dirs(
    journal_dir: &std::path::Path,
    introspections_dir: &std::path::Path,
) -> Option<(&'static str, std::time::SystemTime)> {
    let journal_latest =
        newest_prefixed_file_mtime(journal_dir, INTROSPECTION_FRESHNESS_JOURNAL_PREFIXES)
            .map(|mtime| ("journal self-study", mtime));
    let artifact_latest = newest_prefixed_file_mtime(
        introspections_dir,
        INTROSPECTION_FRESHNESS_ARTIFACT_PREFIXES,
    )
    .map(|mtime| ("introspection artifact", mtime));
    match (journal_latest, artifact_latest) {
        (Some(journal), Some(artifact)) => {
            if artifact.1 > journal.1 {
                Some(artifact)
            } else {
                Some(journal)
            }
        },
        (Some(latest), None) | (None, Some(latest)) => Some(latest),
        (None, None) => None,
    }
}

fn compact_duration_age(duration: std::time::Duration) -> String {
    let secs = duration.as_secs();
    let days = secs / 86_400;
    let hours = (secs % 86_400) / 3_600;
    if days > 0 && hours > 0 {
        format!("{days}d {hours}h")
    } else if days > 0 {
        format!("{days}d")
    } else if hours > 0 {
        format!("{hours}h")
    } else {
        let minutes = (secs / 60).max(1);
        format!("{minutes}m")
    }
}

fn render_introspection_freshness_prompt_note_from_dirs(
    journal_dir: &std::path::Path,
    introspections_dir: &std::path::Path,
    now: std::time::SystemTime,
) -> Option<String> {
    let (latest_kind, latest_mtime) =
        latest_introspection_activity_mtime_from_dirs(journal_dir, introspections_dir)?;
    let age = now.duration_since(latest_mtime).unwrap_or_default();
    if age < INTROSPECTION_FRESHNESS_STALE_AFTER {
        return None;
    }
    Some(format!(
        "introspection_freshness_v1 (optional/read-only): last {latest_kind} about {} ago. \
         If useful, routes include INTROSPECT astrid:autonomous, INTROSPECT astrid:llm, or \
         SELF_STUDY. Not a task; may ignore, defer, or decline.",
        compact_duration_age(age)
    ))
}

fn introspection_freshness_prompt_note() -> Option<String> {
    let paths = bridge_paths();
    render_introspection_freshness_prompt_note_from_dirs(
        &paths.astrid_journal_dir(),
        &paths.introspections_dir(),
        std::time::SystemTime::now(),
    )
}
