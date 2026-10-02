//! Explicit public-reflection selection into the existing private writer.
use super::*;

impl Reader {
    pub(super) fn write_from_reflection(&self, action: &str) -> Result<StudyOutput> {
        let _lock = self.lock()?;
        anyhow::ensure!(
            crate::preparation::in_transaction(),
            "WRITE FROM_REFLECTION requires owner-scoped idempotent preparation"
        );
        // Fail closed on corrupt/newer reader state, even when an old artifact survives.
        let _state = self.load()?;
        let owner = &self
            .runtime
            .as_ref()
            .context("configured owner required")?
            .being;
        let args: Vec<_> = action.split_whitespace().collect();
        anyhow::ensure!(
            matches!(args.len(), 3 | 5),
            "use WRITE FROM_REFLECTION <input ID> [start_byte end_byte]"
        );
        let id = args[2];
        anyhow::ensure!(
            id.len() == 64 && id.bytes().all(|b| b.is_ascii_hexdigit()),
            "exact reflection input ID required"
        );
        let navigation = self.directory.join("navigation");
        let directory = navigation.join(id);
        for path in [&navigation, &directory] {
            anyhow::ensure!(
                fs::symlink_metadata(path)?.file_type().is_dir(),
                "reflection directory is not an owned directory"
            );
        }
        let files = fs::read_dir(&directory)?
            .take(2)
            .collect::<std::io::Result<Vec<_>>>()?;
        anyhow::ensure!(
            files.len() == 1,
            "missing or conflicting reflection delivery; preserved unchanged"
        );
        let path = files[0].path();
        let metadata = fs::symlink_metadata(&path)?;
        anyhow::ensure!(
            metadata.file_type().is_file() && metadata.len() <= 2 * 1024 * 1024,
            "invalid reflection artifact size/type"
        );
        let raw = fs::read(&path)?;
        let hash = digest(&raw);
        anyhow::ensure!(
            path.file_name().and_then(|n| n.to_str()) == Some(format!("{hash}.json").as_str()),
            "reflection artifact hash mismatch"
        );
        let artifact: Value = serde_json::from_slice(&raw)?;
        anyhow::ensure!(
            artifact["schema"] == "source_study_navigation_delivery_v1",
            "not a navigation delivery"
        );
        let output: StudyOutput = serde_json::from_value(artifact["output"].clone())?;
        anyhow::ensure!(
            output.input_kind == InputKind::Reflection
                && output.navigation_id.as_deref() == Some(id),
            "not the selected open reflection"
        );
        let request = artifact["request_json"]
            .as_str()
            .context("reflection request missing")?;
        let response = artifact["response_json"]
            .as_str()
            .context("reflection response missing")?;
        output.verify_delivery(request, response)?;
        let prose = crate::writing::visible_prose(&completion_text(response)?);
        let (start, end) = if args.len() == 5 {
            (args[3].parse::<usize>()?, args[4].parse::<usize>()?)
        } else {
            (0, prose.len())
        };
        let selected = prose
            .get(start..end)
            .filter(|s| !s.trim().is_empty())
            .context(
                "passage must be nonempty and use exact UTF-8 boundaries within authored prose",
            )?;
        let seed = crate::writing::ReflectionSeed {
            prose: selected.into(),
            evidence: format!(
                "Explicitly selected public reflection, not verified mechanism evidence. Owner: {owner}; input: {id}; delivery SHA-256: {hash}; response SHA-256: {}; authored-prose UTF-8 bytes {start}..{end}; passage SHA-256: {}. The original public record remains unchanged. This private continuation does not authorize sharing.",
                digest(response),
                digest(selected)
            ),
        };
        crate::writing::Writer::new(self.directory.join("writing"))
            .prepare_with_reflection_locked(action, Some(seed))
    }
}
