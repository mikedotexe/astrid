use super::*;
use crate::geometry::{GeometryRequest, Operation};

impl Reader {
    // Caller already holds the same cross-process transaction as every reader/question writer.
    pub(super) fn prepare_geometry(&self, state: &mut State, value: Value) -> Result<StudyOutput> {
        let request: GeometryRequest = serde_json::from_value(value)?;
        if state.questions.active.as_deref() != Some(&request.question) {
            bail!("select the question explicitly before GEOMETRY; no checkpoint mutation saved");
        }
        let runtime = self
            .runtime
            .as_ref()
            .context("host-configured owner required")?;
        if matches!(request.operation, Operation::Export)
            && state.questions.has_observations(&request.question)
        {
            let bytes = state
                .questions
                .observation_export(&runtime.being, &request.question)?;
            let path = self
                .directory
                .join("geometry-exports")
                .join(format!("{}.json", digest(&bytes)));
            atomic_write(&path, &bytes)?;
            return self.output(state,format!("Complete inquiry-observations-v2 export (includes legacy geometry and confirmed observations): {} SHA256 {}",path.display(),digest(&bytes)),None,InputKind::Geometry);
        }
        let (question, history) = state.questions.geometry(&request.question)?;
        history.check_owner(&runtime.being)?;
        history.validate()?;
        let text = match &request.operation {
            Operation::Status => history.status(),
            Operation::Show { id } => history.show(id)?,
            Operation::Export => {
                let bytes = history.export(&runtime.being, &request.question, question)?;
                let directory = self.directory.join("geometry-exports");
                fs::create_dir_all(&directory)?;
                let path = directory.join(format!("{}.json", digest(&bytes)));
                if path.exists() {
                    ensure_same_export(&path, &bytes)?;
                } else {
                    atomic_write(&path, &bytes)?;
                }
                format!(
                    "Explicit local export only: {}\nSHA256 {}\nContains only this question and its chosen geometry records, not private drafts, unrelated notebooks or journals. Not delivered to a peer or published.",
                    path.display(),
                    digest(&bytes)
                )
            },
            _ => {
                let root = self
                    .catalog
                    .roots
                    .get("minime")
                    .context("trusted Minime source root required")?;
                let now = u64::try_from(
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)?
                        .as_millis(),
                )?;
                history.apply(&runtime.being, &request, root, now)?
            },
        };
        self.output(state, text, None, InputKind::Geometry)
    }
}

fn ensure_same_export(path: &Path, bytes: &[u8]) -> Result<()> {
    if fs::symlink_metadata(path)?.file_type().is_symlink() || fs::read(path)? != bytes {
        bail!("existing export does not match; preserving it");
    }
    Ok(())
}
