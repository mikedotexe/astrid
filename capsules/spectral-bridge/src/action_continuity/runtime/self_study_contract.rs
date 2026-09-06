// Offline foundation only: no Action dispatch or control-plane route is registered.
// Records extend the authoritative dossier; they are not a second belief store.

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SelfStudyRecipe {
    SyntheticRetrieval,
    SyntheticReservoirHistory,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SelfStudyPrediction {
    pub recipe: SelfStudyRecipe,
    /// Digest of the full fixture manifest, including offsets/input conditions,
    /// not merely the file being read. Runner custody is not attested here.
    pub assets_sha256: String,
    pub expected_above: f64,
    pub authored_expectation: String,
}

fn self_study_digest(value: &impl Serialize) -> Result<String> {
    use sha2::{Digest, Sha256};
    Ok(format!("{:x}", Sha256::digest(serde_json::to_vec(value)?)))
}

impl ActionContinuityStore {
    fn lock_self_study(&self) -> Result<fs::File> {
        use fs2::FileExt;
        let lock = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(self.root().join("self_study.lock"))?;
        lock.try_lock_exclusive()
            .context("another self-study writer is active")?;
        Ok(lock)
    }

    fn strict_dossier_records(&self, thread_id: &str) -> Result<Vec<Value>> {
        let path = self.dossier_path(thread_id);
        if !path.exists() {
            return Ok(Vec::new());
        }
        if fs::metadata(&path)?.len() > 8_388_608 {
            anyhow::bail!("dossier exceeds the bounded contract reader; no record written");
        }
        fs::read_to_string(path)?
            .lines()
            .filter(|line| !line.trim().is_empty())
            .map(|line| serde_json::from_str(line).context("invalid dossier history"))
            .collect()
    }

    fn self_study_target(
        &self,
        experiment_id: &str,
        record_id: &str,
    ) -> Result<(ResearchThread, Value)> {
        let thread = self
            .current_thread()?
            .ok_or_else(|| anyhow!("an existing inquiry is required"))?;
        let experiment = self.resolve_experiment(&thread, Some(experiment_id))?;
        if experiment.experiment_id != experiment_id {
            anyhow::bail!("exact experiment identity required, not a selector alias");
        }
        let record = self
            .strict_dossier_records(&thread.thread_id)?
            .into_iter()
            .find(|row| {
                row["record_id"] == record_id
                    && row["being"] == SYSTEM
                    && row["experiment_id"] == experiment.experiment_id
            })
            .ok_or_else(|| anyhow!("record not found in this owner's experiment"))?;
        Ok((thread, record))
    }

    fn append_self_study_record(&self, thread: &ResearchThread, mut row: Value) -> Result<String> {
        let kind = row["record_type"]
            .as_str()
            .ok_or_else(|| anyhow!("record type required"))?;
        let id = self.unique_dossier_record_id(kind)?;
        row["record_schema"] = json!("research_dossier_v1");
        row["schema_version"] = json!(SCHEMA_VERSION);
        row["record_id"] = json!(id);
        row["thread_id"] = json!(thread.thread_id);
        row["being"] = json!(SYSTEM);
        row["created_at"] = json!(iso_now());
        row["authority_change"] = json!(false);
        row["automatic_return"] = json!(false);
        row["live_intervention"] = json!(false);
        row["deployment_established"] = json!(false);
        row["felt_state_inferred"] = json!(false);
        self.append_jsonl(&self.dossier_path(&thread.thread_id), &row)?;
        OpenOptions::new()
            .write(true)
            .open(self.dossier_path(&thread.thread_id))?
            .sync_all()?;
        Ok(id)
    }

    /// Commit an expectation before this contract accepts its outcome. This does
    /// not attest that the author has never previously seen the fixture/outcome.
    pub fn self_study_predict(
        &self,
        experiment_id: &str,
        claim_id: &str,
        spec: &SelfStudyPrediction,
    ) -> Result<String> {
        let _lock = self.lock_self_study()?;
        if !spec.expected_above.is_finite()
            || spec.expected_above < 0.
            || spec.authored_expectation.trim().is_empty()
            || spec.authored_expectation.len() > 4096
            || spec.assets_sha256.len() != 64
            || !spec.assets_sha256.bytes().all(|b| b.is_ascii_hexdigit())
        {
            anyhow::bail!("invalid bounded prediction or asset digest");
        }
        let (thread, claim) = self.self_study_target(experiment_id, claim_id)?;
        if !matches!(
            claim["record_type"].as_str(),
            Some("claim" | "claim_revision")
        ) {
            anyhow::bail!("prediction requires an existing owned claim");
        }
        self.append_self_study_record(&thread, json!({
            "record_type":"study_prediction", "experiment_id":experiment_id, "claim_id":claim_id,
            "prediction":spec, "prediction_sha256":self_study_digest(spec)?,
            "comparison":"strictly_greater_than", "native_lane":"authored_prediction",
            "prior_outcome_ignorance_attested":false,
        }))
    }

    /// Runner-side API, not an LLM Action. One immutable outcome per prediction.
    /// Missing data is insufficient, never a failed expectation or a felt verdict.
    pub fn self_study_evaluate(
        &self,
        experiment_id: &str,
        prediction_id: &str,
        recipe: SelfStudyRecipe,
        assets_sha256: &str,
        measured: Option<f64>,
        evidence_ref: &str,
    ) -> Result<String> {
        let _lock = self.lock_self_study()?;
        let (thread, prediction) = self.self_study_target(experiment_id, prediction_id)?;
        if prediction["record_type"] != "study_prediction" {
            anyhow::bail!("a committed prediction is required before an outcome");
        }
        let spec: SelfStudyPrediction = serde_json::from_value(prediction["prediction"].clone())?;
        if prediction["prediction_sha256"] != self_study_digest(&spec)?
            || spec.recipe != recipe
            || spec.assets_sha256 != assets_sha256
            || evidence_ref.trim().is_empty()
            || evidence_ref.len() > 4096
            || measured.is_some_and(|value| !value.is_finite() || value < 0.)
            || (recipe == SelfStudyRecipe::SyntheticRetrieval
                && measured.is_some_and(|value| value != 0. && value != 1.))
        {
            anyhow::bail!("prediction, assets, recipe or measurement mismatch");
        }
        if self
            .strict_dossier_records(&thread.thread_id)?
            .iter()
            .any(|row| {
                row["record_type"] == "study_evaluation" && row["prediction_id"] == prediction_id
            })
        {
            anyhow::bail!("outcome already recorded; retain it and make a new prediction");
        }
        let outcome = match measured {
            None => "insufficient",
            Some(value) if value > spec.expected_above => "matched",
            Some(_) => "not_matched",
        };
        self.append_self_study_record(
            &thread,
            json!({
                "record_type":"study_evaluation", "experiment_id":experiment_id,
                "prediction_id":prediction_id, "claim_id":prediction["claim_id"],
                "prediction_sha256":prediction["prediction_sha256"], "assets_sha256":assets_sha256,
                "measured":measured, "outcome":outcome, "evidence_ref":evidence_ref,
                "native_lane":"runner_measurement", "causal_interpretation":null,
                "runner_identity_attested":false,
            }),
        )
    }

    /// Explicit authored amendment. The old claim and all counterevidence remain.
    pub fn self_study_revise(
        &self,
        experiment_id: &str,
        claim_id: &str,
        evaluation_id: &str,
        relationship: &str,
        claim: &str,
        reason: &str,
    ) -> Result<String> {
        let _lock = self.lock_self_study()?;
        if !matches!(relationship, "qualifies" | "challenges" | "supersedes")
            || claim.trim().is_empty()
            || reason.trim().is_empty()
            || claim.len() > 4096
            || reason.len() > 4096
        {
            anyhow::bail!("bounded authored claim, reason, and revision relationship required");
        }
        let (thread, original) = self.self_study_target(experiment_id, claim_id)?;
        let (_, evidence) = self.self_study_target(experiment_id, evaluation_id)?;
        if !matches!(
            original["record_type"].as_str(),
            Some("claim" | "claim_revision")
        ) || evidence["record_type"] != "study_evaluation"
            || evidence["claim_id"] != claim_id
        {
            anyhow::bail!("revision evidence must evaluate this exact owned claim");
        }
        self.append_self_study_record(
            &thread,
            json!({
                "record_type":"claim_revision", "experiment_id":experiment_id,
                "prior_claim_id":claim_id, "relationship":relationship,
                "claim":claim, "reason":reason, "evidence_ids":[evaluation_id],
                "native_lane":"authored_interpretation", "belief_change_inferred":false,
            }),
        )
    }
}
