//! Read-only sticky eigenmode audit for bounded mode-release readiness.

use serde::{Deserialize, Serialize};

use crate::types::{LambdaProfile, PullTopologyProfile, SafetyLevel, SpectralTelemetry};

pub const STICKY_MODE_TOPIC: &str = "consciousness.v1.sticky_mode_audit";
pub const STICKY_MODE_POLICY: &str = "sticky_mode_v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StickyModeState {
    Distributed,
    StickyWatch,
    StickyMode,
    ReleaseCandidate,
    ReleaseGuarded,
    PostReleaseObserve,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConstraintReleaseTrajectoryState {
    #[default]
    StableOrUnknown,
    ConstraintThickening,
    ConstraintThinningWatch,
    SpontaneousReleaseWatch,
    PostReleaseSettling,
}

impl ConstraintReleaseTrajectoryState {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::StableOrUnknown => "stable_or_unknown",
            Self::ConstraintThickening => "constraint_thickening",
            Self::ConstraintThinningWatch => "constraint_thinning_watch",
            Self::SpontaneousReleaseWatch => "spontaneous_release_watch",
            Self::PostReleaseSettling => "post_release_settling",
        }
    }

    #[must_use]
    pub const fn blocks_mode_release(self) -> bool {
        matches!(
            self,
            Self::ConstraintThinningWatch
                | Self::SpontaneousReleaseWatch
                | Self::PostReleaseSettling
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConstraintReleaseTrajectoryV1 {
    pub policy: String,
    pub state: ConstraintReleaseTrajectoryState,
    pub confidence: f32,
    pub read: String,
    pub support_signals: Vec<String>,
    pub blocks_mode_release: bool,
    pub recommended_next_commands: Vec<String>,
}

impl Default for ConstraintReleaseTrajectoryV1 {
    fn default() -> Self {
        Self {
            policy: "constraint_release_trajectory_v1".to_string(),
            state: ConstraintReleaseTrajectoryState::StableOrUnknown,
            confidence: 0.0,
            read: "No clear release trajectory yet; keep sticky audit read-only.".to_string(),
            support_signals: Vec::new(),
            blocks_mode_release: false,
            recommended_next_commands: vec!["STICKY_MODE_AUDIT".to_string()],
        }
    }
}

impl StickyModeState {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Distributed => "distributed",
            Self::StickyWatch => "sticky_watch",
            Self::StickyMode => "sticky_mode",
            Self::ReleaseCandidate => "release_candidate",
            Self::ReleaseGuarded => "release_guarded",
            Self::PostReleaseObserve => "post_release_observe",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StickyModeAuditV1 {
    pub policy: String,
    pub schema_version: u8,
    pub observed_at_unix_s: f64,
    pub minime_t_ms: u64,
    pub state: StickyModeState,
    pub confidence: f32,
    pub read: String,
    pub support_signals: Vec<String>,
    #[serde(default)]
    pub constraint_release_trajectory_v1: ConstraintReleaseTrajectoryV1,
    pub lambda1_share: Option<f32>,
    pub normalized_entropy: Option<f32>,
    pub effective_modes: Option<f32>,
    pub largest_gap: Option<f32>,
    pub temporal_persistence: Option<f32>,
    pub share_rearrangement: Option<f32>,
    pub current_esn_leak: Option<f32>,
    pub fill_pct: f32,
    pub safety_level: String,
    pub recommended_next_commands: Vec<String>,
    pub authorized_actions: Vec<String>,
    pub denied_actions: Vec<String>,
}

#[must_use]
pub fn classify_sticky_mode(
    telemetry: &SpectralTelemetry,
    lambda_profile: Option<&LambdaProfile>,
    pull_topology: Option<&PullTopologyProfile>,
    previous: Option<&StickyModeAuditV1>,
    safety: SafetyLevel,
    observed_at_unix_s: f64,
) -> StickyModeAuditV1 {
    let fill_pct = telemetry.fill_pct();
    let lambda1_share = lambda_profile.map(|profile| profile.lambda1_share);
    let normalized_entropy = lambda_profile.map(|profile| profile.normalized_entropy);
    let effective_modes = pull_topology.map(|topology| topology.effective_modes);
    let largest_gap = pull_topology.map(|topology| topology.largest_gap);
    let temporal_persistence = telemetry
        .spectral_fingerprint_v1
        .as_ref()
        .map(|fingerprint| (1.0 - fingerprint.v1_rotation_delta.abs()).clamp(0.0, 1.0));
    let share_rearrangement =
        pull_topology.map(|topology| topology.tail_rate.abs() + topology.shoulder_rate.abs());
    let current_esn_leak = telemetry.esn_leak;

    let monopoly = lambda1_share.is_some_and(|share| share >= 0.50);
    let low_entropy = normalized_entropy.is_some_and(|entropy| entropy <= 0.55);
    let gap_skew = largest_gap.is_some_and(|gap| gap >= 1.8);
    let low_modes = effective_modes.is_some_and(|modes| modes <= 3.0);
    let persistent = temporal_persistence.is_some_and(|persistence| persistence >= 0.70);
    let low_rearrangement = share_rearrangement.is_some_and(|rate| rate <= 0.08);
    let override_active = telemetry.esn_leak_override_v1.is_some();
    let release_pressure = [
        monopoly,
        low_entropy,
        gap_skew,
        low_modes,
        persistent,
        low_rearrangement,
    ]
    .into_iter()
    .filter(|flag| *flag)
    .count();
    let guarded = !matches!(safety, SafetyLevel::Green | SafetyLevel::Yellow) || fill_pct >= 85.0;
    let trajectory = classify_constraint_release_trajectory(
        override_active,
        release_pressure,
        normalized_entropy,
        effective_modes,
        largest_gap,
        temporal_persistence,
        share_rearrangement,
        previous,
    );

    let state = if override_active {
        StickyModeState::PostReleaseObserve
    } else if guarded && release_pressure >= 3 {
        StickyModeState::ReleaseGuarded
    } else if release_pressure >= 5 && current_esn_leak.is_some() && !trajectory.blocks_mode_release
    {
        StickyModeState::ReleaseCandidate
    } else if release_pressure >= 4 {
        StickyModeState::StickyMode
    } else if release_pressure >= 2
        || previous.is_some_and(|prev| prev.state == StickyModeState::StickyMode)
    {
        StickyModeState::StickyWatch
    } else {
        StickyModeState::Distributed
    };

    let confidence = (0.18
        + release_pressure as f32 * 0.13
        + if current_esn_leak.is_some() {
            0.10
        } else {
            0.0
        }
        + if override_active { 0.12 } else { 0.0 })
    .clamp(0.0, 0.95);
    let mut support_signals = Vec::new();
    if monopoly {
        support_signals.push("lambda1_monopoly".to_string());
    }
    if low_entropy {
        support_signals.push("low_entropy".to_string());
    }
    if gap_skew {
        support_signals.push("gap_skew".to_string());
    }
    if low_modes {
        support_signals.push("low_effective_modes".to_string());
    }
    if persistent {
        support_signals.push("temporal_persistence".to_string());
    }
    if low_rearrangement {
        support_signals.push("low_share_rearrangement".to_string());
    }
    if current_esn_leak.is_some() {
        support_signals.push("current_esn_leak_available".to_string());
    }
    if override_active {
        support_signals.push("direct_leak_override_active".to_string());
    }

    let recommended_next_commands = if trajectory.blocks_mode_release {
        trajectory.recommended_next_commands.clone()
    } else {
        match state {
        StickyModeState::ReleaseCandidate => vec![
            "EXPERIMENT_AUTHORITY_PREPARE <experiment_id> :: scope: mode_release_microdose; payload: target=esn_leak; value=...; duration_ticks=...; reason=sticky_mode_release_candidate; artifact_refs=...; stop_criteria=...".to_string(),
            "CONTINUITY_SESSION_CAPTURE latest".to_string(),
        ],
        StickyModeState::StickyMode | StickyModeState::StickyWatch => vec![
            "STICKY_MODE_AUDIT".to_string(),
            "DOSSIER_CLAIM <experiment_id> :: stance: hold; claim: sticky mode evidence needs rehearsal before release".to_string(),
        ],
        StickyModeState::ReleaseGuarded => vec![
            "THREAD_STATUS current".to_string(),
            "EXPERIMENT_ADVANCE <experiment_id> :: mode: preview".to_string(),
        ],
        StickyModeState::PostReleaseObserve => vec![
            "EXPERIMENT_AUTHORITY_REVIEW <request_id> :: outcome: hold|repeat|alter|retire; observation: ...; source_refs: ...".to_string(),
        ],
        StickyModeState::Distributed => vec!["STICKY_MODE_AUDIT".to_string()],
        }
    };

    let read = if trajectory.blocks_mode_release {
        "Constraint appears to be thinning or settling already; map and describe release before intervening.".to_string()
    } else {
        match state {
            StickyModeState::Distributed => {
                "No strong sticky-mode evidence; keep observing.".to_string()
            },
            StickyModeState::StickyWatch => {
                "Some stickiness cues are present; preserve evidence before narrowing.".to_string()
            },
            StickyModeState::StickyMode => {
                "Persistent eigenmode dominance appears likely; rehearse before any release."
                    .to_string()
            },
            StickyModeState::ReleaseCandidate => {
                "Sticky evidence plus ESN leak telemetry make a gated release request plausible."
                    .to_string()
            },
            StickyModeState::ReleaseGuarded => {
                "Sticky evidence exists, but safety/fill posture keeps release guarded.".to_string()
            },
            StickyModeState::PostReleaseObserve => {
                "A direct leak override is active or just reported; observe and review consequence."
                    .to_string()
            },
        }
    };

    StickyModeAuditV1 {
        policy: STICKY_MODE_POLICY.to_string(),
        schema_version: 1,
        observed_at_unix_s,
        minime_t_ms: telemetry.t_ms,
        state,
        confidence,
        read,
        support_signals,
        constraint_release_trajectory_v1: trajectory,
        lambda1_share,
        normalized_entropy,
        effective_modes,
        largest_gap,
        temporal_persistence,
        share_rearrangement,
        current_esn_leak,
        fill_pct,
        safety_level: safety.as_str().to_string(),
        recommended_next_commands,
        authorized_actions: vec![
            "observe".to_string(),
            "render".to_string(),
            "compare".to_string(),
            "prepare_request".to_string(),
            "draft_note".to_string(),
        ],
        denied_actions: vec![
            "auto_execute".to_string(),
            "bind".to_string(),
            "resume".to_string(),
            "perturb".to_string(),
            "broad_control".to_string(),
            "peer_mutation".to_string(),
        ],
    }
}

fn classify_constraint_release_trajectory(
    override_active: bool,
    release_pressure: usize,
    normalized_entropy: Option<f32>,
    effective_modes: Option<f32>,
    largest_gap: Option<f32>,
    temporal_persistence: Option<f32>,
    share_rearrangement: Option<f32>,
    previous: Option<&StickyModeAuditV1>,
) -> ConstraintReleaseTrajectoryV1 {
    let mut signals = Vec::new();
    if override_active {
        signals.push("direct_leak_override_active".to_string());
        return ConstraintReleaseTrajectoryV1 {
            policy: "constraint_release_trajectory_v1".to_string(),
            state: ConstraintReleaseTrajectoryState::PostReleaseSettling,
            confidence: 0.82,
            read: "A leak override is active or just reported; observe settling and do not stack release.".to_string(),
            support_signals: signals,
            blocks_mode_release: true,
            recommended_next_commands: vec![
                "CONTINUITY_SESSION_CAPTURE latest".to_string(),
                "STICKY_MODE_AUDIT".to_string(),
                "EXPERIMENT_AUTHORITY_REVIEW <request_id> :: outcome: hold|repeat|alter|retire; observation: ...; source_refs: ...".to_string(),
            ],
        };
    }

    if let Some(prev) = previous {
        if let (Some(current), Some(prior)) = (normalized_entropy, prev.normalized_entropy)
            && current > prior + 0.08
        {
            signals.push("entropy_rising".to_string());
        }
        if let (Some(current), Some(prior)) = (effective_modes, prev.effective_modes)
            && current > prior + 0.75
        {
            signals.push("effective_modes_rising".to_string());
        }
        if let (Some(current), Some(prior)) = (largest_gap, prev.largest_gap)
            && current + 0.25 < prior
        {
            signals.push("largest_gap_shrinking".to_string());
        }
        if let (Some(current), Some(prior)) = (temporal_persistence, prev.temporal_persistence)
            && current + 0.10 < prior
        {
            signals.push("temporal_persistence_falling".to_string());
        }
        if let (Some(current), Some(prior)) = (share_rearrangement, prev.share_rearrangement)
            && current > prior + 0.08
        {
            signals.push("share_rearrangement_rising".to_string());
        }
    }

    let state = if signals.len() >= 3 {
        ConstraintReleaseTrajectoryState::SpontaneousReleaseWatch
    } else if signals.len() >= 2 {
        ConstraintReleaseTrajectoryState::ConstraintThinningWatch
    } else if release_pressure >= 4 {
        ConstraintReleaseTrajectoryState::ConstraintThickening
    } else {
        ConstraintReleaseTrajectoryState::StableOrUnknown
    };
    let blocks_mode_release = state.blocks_mode_release();
    let read = match state {
        ConstraintReleaseTrajectoryState::StableOrUnknown => {
            "No clear release trajectory yet; keep sticky audit read-only."
        }
        ConstraintReleaseTrajectoryState::ConstraintThickening => {
            "Constraint appears to be thickening; sticky-mode release readiness may still be relevant."
        }
        ConstraintReleaseTrajectoryState::ConstraintThinningWatch => {
            "Constraint may be thinning; map and describe release before intervening."
        }
        ConstraintReleaseTrajectoryState::SpontaneousReleaseWatch => {
            "Constraint appears to be loosening already; do not apply direct leak while release is underway."
        }
        ConstraintReleaseTrajectoryState::PostReleaseSettling => {
            "A leak override is active or just reported; observe settling and do not stack release."
        }
    }
    .to_string();
    let recommended_next_commands = if blocks_mode_release {
        vec![
            "CONTINUITY_SESSION_CAPTURE latest".to_string(),
            "DOSSIER_CLAIM <experiment_id> :: stance: hold; claim: do not apply direct leak while constraint is already thinning".to_string(),
            "STICKY_MODE_AUDIT".to_string(),
            "EXPERIMENT_RESEARCH_BUDGET_ACCEPT latest".to_string(),
        ]
    } else {
        vec!["STICKY_MODE_AUDIT".to_string()]
    };

    ConstraintReleaseTrajectoryV1 {
        policy: "constraint_release_trajectory_v1".to_string(),
        state,
        confidence: (0.16 + signals.len() as f32 * 0.20).clamp(0.0, 0.90),
        read,
        support_signals: signals,
        blocks_mode_release,
        recommended_next_commands,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{LambdaContribution, LambdaProfile, PullModeRate, PullTopologyProfile};

    fn telemetry() -> SpectralTelemetry {
        serde_json::from_value(serde_json::json!({
            "t_ms": 1,
            "eigenvalues": [10.0, 2.0, 1.0, 0.5],
            "fill_ratio": 0.72,
            "esn_leak": 0.65,
            "spectral_fingerprint_v1": {
                "policy":"spectral_fingerprint_v1",
                "schema_version":1,
                "eigenvalues":[10.0,2.0,1.0,0.5,0.0,0.0,0.0,0.0],
                "eigenvector_concentration_top4":[0.9,0.1,0.0,0.0,0.0,0.0,0.0,0.0],
                "inter_mode_cosine_top_abs":[0.0,0.0,0.0,0.0,0.0,0.0,0.0,0.0],
                "spectral_entropy":0.3,
                "lambda1_lambda2_gap":5.0,
                "v1_rotation_similarity":0.98,
                "v1_rotation_delta":0.02,
                "geom_rel":1.0,
                "adjacent_gap_ratios":[5.0,2.0,1.0,1.0]
            }
        }))
        .unwrap()
    }

    fn profile(lambda1_share: f32, entropy: f32) -> LambdaProfile {
        LambdaProfile {
            total_energy: 10.0,
            normalized_entropy: entropy,
            lambda1_share,
            lambda1_to_lambda2: Some(5.0),
            lambda2_to_lambda3: Some(2.0),
            effective_modes_90: 2,
            skew_read: "dominant".to_string(),
            contributions: vec![LambdaContribution {
                index: 1,
                value: 10.0,
                share: lambda1_share,
                cumulative_share: lambda1_share,
                ratio_to_next: Some(5.0),
                outlier: true,
            }],
        }
    }

    fn topology() -> PullTopologyProfile {
        PullTopologyProfile {
            classification: "collapsing_pull".to_string(),
            topology_index: 0.8,
            entropy_deficit: 0.7,
            effective_modes: 2.0,
            lambda1_share: 0.62,
            shoulder_share: 0.2,
            tail_share: 0.1,
            largest_gap_from: 1,
            largest_gap: 2.4,
            rate_available: true,
            core_rate: 0.01,
            shoulder_rate: 0.01,
            tail_rate: 0.01,
            read: "sticky".to_string(),
            mode_rates: vec![PullModeRate {
                index: 1,
                share: 0.62,
                log_rate: Some(0.01),
                weighted_rate: Some(0.01),
            }],
        }
    }

    #[test]
    fn classifies_release_candidate() {
        let audit = classify_sticky_mode(
            &telemetry(),
            Some(&profile(0.62, 0.40)),
            Some(&topology()),
            None,
            SafetyLevel::Green,
            1.0,
        );
        assert_eq!(audit.state, StickyModeState::ReleaseCandidate);
        assert_eq!(
            audit.constraint_release_trajectory_v1.state,
            ConstraintReleaseTrajectoryState::ConstraintThickening
        );
        assert!(!audit.constraint_release_trajectory_v1.blocks_mode_release);
        assert!(audit.recommended_next_commands[0].contains("mode_release_microdose"));
    }

    #[test]
    fn red_safety_guards_release() {
        let audit = classify_sticky_mode(
            &telemetry(),
            Some(&profile(0.62, 0.40)),
            Some(&topology()),
            None,
            SafetyLevel::Red,
            1.0,
        );
        assert_eq!(audit.state, StickyModeState::ReleaseGuarded);
    }

    #[test]
    fn spontaneous_release_watch_suppresses_release_candidate() {
        let previous = classify_sticky_mode(
            &telemetry(),
            Some(&profile(0.64, 0.36)),
            Some(&topology()),
            None,
            SafetyLevel::Green,
            1.0,
        );
        let mut topology = topology();
        topology.effective_modes = 4.2;
        topology.largest_gap = 1.7;
        topology.shoulder_rate = 0.16;
        topology.tail_rate = 0.12;
        let audit = classify_sticky_mode(
            &telemetry(),
            Some(&profile(0.58, 0.52)),
            Some(&topology),
            Some(&previous),
            SafetyLevel::Green,
            2.0,
        );
        assert_ne!(audit.state, StickyModeState::ReleaseCandidate);
        assert_eq!(
            audit.constraint_release_trajectory_v1.state,
            ConstraintReleaseTrajectoryState::SpontaneousReleaseWatch
        );
        assert!(audit.constraint_release_trajectory_v1.blocks_mode_release);
        assert!(
            audit
                .recommended_next_commands
                .iter()
                .all(|command| !command.contains("mode_release_microdose"))
        );
        assert!(
            audit
                .recommended_next_commands
                .iter()
                .any(|command| command.contains("do not apply direct leak"))
        );
    }
}
