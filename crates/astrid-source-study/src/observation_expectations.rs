//! Optional authored numerical expectations, separate from descriptive execution.
use crate::{
    geometry::Snapshot,
    recurrence::{Analysis, ResultRecord},
};
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Expectation {
    pub analysis: Analysis,
    pub comparison: Comparison,
    pub value: f64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Comparison {
    AtLeast,
    AtMost,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub(crate) struct Evaluation {
    pub numerical_expectation_matched: Option<bool>,
    pub evidence_predates_expectation: bool,
    pub interpretation: String,
}
impl Expectation {
    pub fn validate(&self, at: u64, get: impl Fn(&str) -> Result<Snapshot>) -> Result<()> {
        ensure!(
            self.value.is_finite() && (0.0..=1.0).contains(&self.value),
            "expected fraction/similarity must be in [0,1]"
        );
        let captures = match &self.analysis {
            Analysis::StateReturn { window, .. } => vec![&window.capture],
            Analysis::CovarianceShape { first, second } => vec![&first.capture, &second.capture],
        };
        for id in captures {
            ensure!(
                get(id)?.captured_at_unix_ms <= at,
                "expectation clock precedes selected frozen evidence"
            );
        }
        // Qualify recipe and intervals, without recording an execution result as an authored prediction.
        crate::recurrence::run(&self.analysis, get)?;
        Ok(())
    }
    pub fn evaluate(&self, analysis: &Analysis, result: &ResultRecord) -> Result<Evaluation> {
        ensure!(
            serde_json::to_value(&self.analysis)? == serde_json::to_value(analysis)?,
            "expectation recipe or intervals differ from execution"
        );
        let metric = match result {
            ResultRecord::StateReturn {
                near_state_fraction,
                degenerate,
                ..
            } => {
                if *degenerate {
                    None
                } else {
                    *near_state_fraction
                }
            },
            ResultRecord::CovarianceShape {
                normalized_frobenius_similarity,
                ..
            } => *normalized_frobenius_similarity,
        };
        Ok(Evaluation {numerical_expectation_matched:metric.map(|v|match self.comparison{Comparison::AtLeast=>v>=self.value,Comparison::AtMost=>v<=self.value}),
            evidence_predates_expectation:true,
            interpretation:"Historical numerical comparison only, not confirmed recurrence. Commitment preceded this execution, not the evidence or necessarily prior knowledge. No automatic inquiry resolution or experiential inference.".into()})
    }
}
