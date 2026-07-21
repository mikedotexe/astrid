use serde::{Deserialize, Serialize};

pub const PROTOCOL_NAME: &str = "astrid_minime";
pub const PROTOCOL_MAJOR: u16 = 1;
pub const PROTOCOL_MINOR: u16 = 2;
pub const TELEMETRY_PROTOCOL_MINOR: u16 = 0;
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolHeaderV1 {
    pub name: String,
    pub major: u16,
    pub minor: u16,
}

impl Default for ProtocolHeaderV1 {
    fn default() -> Self {
        current_protocol()
    }
}

#[must_use]
pub fn current_protocol() -> ProtocolHeaderV1 {
    ProtocolHeaderV1 {
        name: PROTOCOL_NAME.to_string(),
        major: PROTOCOL_MAJOR,
        minor: PROTOCOL_MINOR,
    }
}

#[must_use]
pub fn telemetry_protocol() -> ProtocolHeaderV1 {
    ProtocolHeaderV1 {
        name: PROTOCOL_NAME.to_string(),
        major: PROTOCOL_MAJOR,
        minor: TELEMETRY_PROTOCOL_MINOR,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompatibilityStatus {
    Current,
    CompatibleMinor,
    LegacyUnversioned,
    UnsupportedName,
    UnsupportedMajor,
}

impl CompatibilityStatus {
    #[must_use]
    pub const fn is_compatible(self) -> bool {
        matches!(
            self,
            Self::Current | Self::CompatibleMinor | Self::LegacyUnversioned
        )
    }
}

#[must_use]
pub fn classify_protocol(header: Option<&ProtocolHeaderV1>) -> CompatibilityStatus {
    let Some(header) = header else {
        return CompatibilityStatus::LegacyUnversioned;
    };
    if header.name != PROTOCOL_NAME {
        return CompatibilityStatus::UnsupportedName;
    }
    if header.major != PROTOCOL_MAJOR {
        return CompatibilityStatus::UnsupportedMajor;
    }
    if header.minor <= PROTOCOL_MINOR {
        CompatibilityStatus::Current
    } else {
        CompatibilityStatus::CompatibleMinor
    }
}
