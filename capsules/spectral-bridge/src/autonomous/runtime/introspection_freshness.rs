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

const CANONICAL_INTROSPECTION_STALE_AFTER: std::time::Duration =
    std::time::Duration::from_secs(30 * 60);
const CANONICAL_INTROSPECTION_PREFIXES: &[&str] = &["introspection_"];

fn render_canonical_introspection_freshness_prompt_note_from_dir(
    introspections_dir: &std::path::Path,
    now: std::time::SystemTime,
) -> Option<String> {
    let latest = newest_prefixed_file_mtime(introspections_dir, CANONICAL_INTROSPECTION_PREFIXES)?;
    let age = now.duration_since(latest).unwrap_or_default();
    if age < CANONICAL_INTROSPECTION_STALE_AFTER {
        return None;
    }
    Some(format!(
        "canonical_introspection_freshness_v2 (optional/read-only): latest canonical introspection artifact about {} ago. \
         If useful, routes include INTROSPECT astrid:autonomous, INTROSPECT astrid:llm, or \
         INTROSPECT next-in-rotation. This is not a task and does not infer intent; silence is \
         neutral. May ignore, defer, or decline.",
        compact_duration_age(age)
    ))
}

fn introspection_freshness_prompt_note() -> Option<String> {
    let paths = bridge_paths();
    let now = std::time::SystemTime::now();
    render_canonical_introspection_freshness_prompt_note_from_dir(&paths.introspections_dir(), now)
        .or_else(|| {
            render_introspection_freshness_prompt_note_from_dirs(
                &paths.astrid_journal_dir(),
                &paths.introspections_dir(),
                now,
            )
        })
}

#[cfg(test)]
mod canonical_introspection_freshness_tests {
    use super::*;

    #[test]
    fn stale_canonical_artifact_surfaces_even_when_self_study_is_fresh() {
        let temp = tempfile::tempdir().expect("tempdir");
        let journal_dir = temp.path().join("journal");
        let introspections_dir = temp.path().join("introspections");
        std::fs::create_dir_all(&journal_dir).expect("journal dir");
        std::fs::create_dir_all(&introspections_dir).expect("introspections dir");
        let canonical = introspections_dir.join("introspection_astrid_llm_1.txt");
        let self_study = journal_dir.join("self_study_2.txt");
        std::fs::write(&canonical, "Observed:\ncanonical signal\n").expect("write canonical");
        std::fs::write(&self_study, "Observed:\nfresh self-study\n").expect("write self-study");
        let now = std::time::UNIX_EPOCH + std::time::Duration::from_secs(300_000);
        let stale = now
            .checked_sub(std::time::Duration::from_secs(31 * 60))
            .expect("stale mtime");
        let fresh = now
            .checked_sub(std::time::Duration::from_secs(5 * 60))
            .expect("fresh mtime");
        std::fs::OpenOptions::new()
            .write(true)
            .open(&canonical)
            .expect("open canonical")
            .set_modified(stale)
            .expect("set canonical mtime");
        std::fs::OpenOptions::new()
            .write(true)
            .open(&self_study)
            .expect("open self-study")
            .set_modified(fresh)
            .expect("set self-study mtime");

        let note =
            render_canonical_introspection_freshness_prompt_note_from_dir(&introspections_dir, now)
                .expect("canonical freshness note");

        assert!(note.contains("canonical_introspection_freshness_v2"));
        assert!(note.contains("optional/read-only"));
        assert!(note.contains("silence is neutral"));
        assert!(note.contains("May ignore, defer, or decline"));
        assert!(!note.contains("must"));
    }

    #[test]
    fn recent_canonical_artifact_stays_quiet() {
        let temp = tempfile::tempdir().expect("tempdir");
        let introspections_dir = temp.path().join("introspections");
        std::fs::create_dir_all(&introspections_dir).expect("introspections dir");
        let canonical = introspections_dir.join("introspection_astrid_llm_1.txt");
        std::fs::write(&canonical, "Observed:\ncanonical signal\n").expect("write canonical");
        let now = std::time::UNIX_EPOCH + std::time::Duration::from_secs(300_000);
        let fresh = now
            .checked_sub(std::time::Duration::from_secs(29 * 60))
            .expect("fresh mtime");
        std::fs::OpenOptions::new()
            .write(true)
            .open(&canonical)
            .expect("open canonical")
            .set_modified(fresh)
            .expect("set canonical mtime");

        assert!(
            render_canonical_introspection_freshness_prompt_note_from_dir(
                &introspections_dir,
                now,
            )
            .is_none()
        );
    }
}
