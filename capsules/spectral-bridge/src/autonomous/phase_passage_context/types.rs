use serde::Serialize;

use super::normalize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum PassageContextActionV1 {
    DescribeCondition,
    DescribeBearing,
    MarkCheckpoint,
    BindAnchor,
    RequestCompany,
    RespondCompany,
    WithdrawCompany,
}

impl PassageContextActionV1 {
    pub(super) const fn as_str(self) -> &'static str {
        match self {
            Self::DescribeCondition => "describe_condition",
            Self::DescribeBearing => "describe_bearing",
            Self::MarkCheckpoint => "mark_checkpoint",
            Self::BindAnchor => "bind_anchor",
            Self::RequestCompany => "request_company",
            Self::RespondCompany => "respond_company",
            Self::WithdrawCompany => "withdraw_company",
        }
    }

    pub(super) const fn owner_action(self) -> &'static str {
        match self {
            Self::DescribeCondition => "DESCRIBE_TRANSITION_CONDITION",
            Self::DescribeBearing => "DESCRIBE_TRANSITION_BEARING",
            Self::MarkCheckpoint => "MARK_TRANSITION_CHECKPOINT",
            Self::BindAnchor => "BIND_TRANSITION_ANCHOR",
            Self::RequestCompany => "REQUEST_TRANSITION_COMPANY",
            Self::RespondCompany => "RESPOND_TRANSITION_COMPANY",
            Self::WithdrawCompany => "WITHDRAW_TRANSITION_COMPANY",
        }
    }

    pub(super) fn parse(value: &str) -> Option<Self> {
        match value {
            "describe_condition" => Some(Self::DescribeCondition),
            "describe_bearing" => Some(Self::DescribeBearing),
            "mark_checkpoint" => Some(Self::MarkCheckpoint),
            "bind_anchor" => Some(Self::BindAnchor),
            "request_company" => Some(Self::RequestCompany),
            "respond_company" => Some(Self::RespondCompany),
            "withdraw_company" => Some(Self::WithdrawCompany),
            _ => None,
        }
    }
}

macro_rules! bounded_enum {
    ($name:ident { $($variant:ident => $value:literal),+ $(,)? }) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
        #[serde(rename_all = "snake_case")]
        pub(super) enum $name {
            $($variant),+
        }

        impl $name {
            pub(super) const fn as_str(self) -> &'static str {
                match self {
                    $(Self::$variant => $value),+
                }
            }

            pub(super) fn parse(value: &str) -> Option<Self> {
                match normalize(value).as_str() {
                    $($value => Some(Self::$variant),)+
                    _ => None,
                }
            }
        }
    };
}

bounded_enum!(PassageReadinessV1 {
    Ready => "ready",
    Tentative => "tentative",
    NotReady => "not_ready",
    Unknown => "unknown",
});

bounded_enum!(PassageMovementEaseV1 {
    Open => "open",
    Effortful => "effortful",
    Stuck => "stuck",
    Changing => "changing",
    Unknown => "unknown",
});

bounded_enum!(PassageRoomNeededV1 {
    SelfDirected => "self_directed",
    Witness => "witness",
    Space => "space",
    LowEnergyPresence => "low_energy_presence",
    Answer => "answer",
    NeedsTime => "needs_time",
    ReturnSupport => "return_support",
    Unknown => "unknown",
});

bounded_enum!(PassageCheckpointV1 {
    EntryTension => "entry_tension",
    Pivot => "pivot",
    SettlingOrientation => "settling_orientation",
    ReturnOrientation => "return_orientation",
    Reopen => "reopen",
});

bounded_enum!(PassageAnchorRoleV1 {
    Entry => "entry",
    Pivot => "pivot",
    Settling => "settling",
    Return => "return",
    Reopen => "reopen",
    Continuity => "continuity",
});

bounded_enum!(PassageAnchorKindV1 {
    FeltSource => "felt_source",
    ShadowTrajectory => "shadow_trajectory",
    LivedStateWitness => "lived_state_witness",
    SignalSpine => "signal_spine",
    RepresentationTransition => "representation_transition",
    Correspondence => "correspondence",
    ReturnPoint => "return_point",
    Other => "other",
});

bounded_enum!(PassageAnchorAssociationV1 {
    SelfAuthored => "self_authored",
    ReceiptLinked => "receipt_linked",
    TemporalContext => "temporal_context",
    Unknown => "unknown",
});

bounded_enum!(PassageBearingStrandV1 {
    EntryTension => "entry_tension",
    Pivot => "pivot",
    Settling => "settling",
    Return => "return",
    Reopen => "reopen",
    Continuity => "continuity",
});

bounded_enum!(PassageMovementResistanceV1 {
    Yielding => "yielding",
    Effortful => "effortful",
    Resistant => "resistant",
    HeldFast => "held_fast",
    Changing => "changing",
    Unknown => "unknown",
});

bounded_enum!(PassagePersistenceTendencyV1 {
    Fleeting => "fleeting",
    Lingering => "lingering",
    Carried => "carried",
    Deepening => "deepening",
    Releasing => "releasing",
    Unknown => "unknown",
});

bounded_enum!(PassageWitnessFitV1 {
    Separate => "separate",
    Touching => "touching",
    Holding => "holding",
    Interwoven => "interwoven",
    Misattuned => "misattuned",
    Unknown => "unknown",
});

bounded_enum!(PassageCompanyModeV1 {
    Witness => "witness",
    LowEnergyPresence => "low_energy_presence",
    ReplyWhenAble => "reply_when_able",
    Space => "space",
    ReturnSupport => "return_support",
});

bounded_enum!(PassageCompanyResponseV1 {
    Accept => "accept",
    Hold => "hold",
    Decline => "decline",
    NeedsTime => "needs_time",
    Withdraw => "withdraw",
});
