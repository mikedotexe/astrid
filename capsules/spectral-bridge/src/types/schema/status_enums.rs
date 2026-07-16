/// Bidirectional connectivity health derived from the two `WebSocket` lanes.
///
/// The bridge tracks `telemetry_connected` (inbound perception, port 7878) and
/// `sensory_connected` (outbound agency, port 7879) as independent booleans.
/// This enum collapses them into a single perceivable state so a one-way
/// "partial-blindness" window is explicit rather than implicit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ConnectivityStatus {
    /// Both lanes live: full perception and agency.
    Bidirectional,
    /// Telemetry only: perceiving minime but unable to influence (mute agency).
    TelemetryOnly,
    /// Sensory only: able to send features but blind to minime's state.
    SensoryOnly,
    /// Neither lane connected.
    #[default]
    Severed,
}

impl ConnectivityStatus {
    /// Derive the connectivity state from the two lane booleans.
    #[must_use]
    pub const fn from_lanes(telemetry_connected: bool, sensory_connected: bool) -> Self {
        match (telemetry_connected, sensory_connected) {
            (true, true) => Self::Bidirectional,
            (true, false) => Self::TelemetryOnly,
            (false, true) => Self::SensoryOnly,
            (false, false) => Self::Severed,
        }
    }

    /// True only when both lanes are live — the ground for confident spectral
    /// maneuvers (both the speaker and the listener are online).
    #[must_use]
    pub const fn is_bidirectional_active(self) -> bool {
        matches!(self, Self::Bidirectional)
    }

    /// True when exactly one lane is live — the "partial-blindness" window.
    #[must_use]
    pub const fn is_partial_blindness(self) -> bool {
        matches!(self, Self::TelemetryOnly | Self::SensoryOnly)
    }
}

/// Spectral safety level determining bridge behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SafetyLevel {
    /// fill < 75%: Normal relay, full throughput.
    Green,
    /// fill 75-85%: Advisory — log warning, no behavioral change.
    Yellow,
    /// fill 85-92%: Advisory — log alert, no message dropping.
    Orange,
    /// fill ≥ 92%: Emergency — suspend outbound, cease bridge traffic.
    Red,
}

impl SafetyLevel {
    /// Determine safety level from eigenvalue fill percentage.
    #[must_use]
    pub fn from_fill(fill_pct: f32) -> Self {
        // Recalibrated 2026-04-02: targeting fill equilibrium ~65-70% under
        // the current lower semantic-gain regime and wider dynamic-rho range.
        // Only Red (≥92%) suspends outbound.
        if fill_pct >= 92.0 {
            Self::Red
        } else if fill_pct >= 85.0 {
            Self::Orange
        } else if fill_pct >= 75.0 {
            Self::Yellow
        } else {
            Self::Green
        }
    }

    /// Returns `true` if outbound messages to minime should be suspended.
    /// Agency-first: only Red (emergency, ≥95%) suspends outbound.
    /// Orange is advisory — the being can still speak.
    #[must_use]
    pub fn should_suspend_outbound(self) -> bool {
        matches!(self, Self::Red)
    }

    /// Returns `true` if all bridge traffic should cease.
    #[must_use]
    pub fn is_emergency(self) -> bool {
        matches!(self, Self::Red)
    }

    /// Stable lowercase representation for logs and JSON sidecars.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Green => "green",
            Self::Yellow => "yellow",
            Self::Orange => "orange",
            Self::Red => "red",
        }
    }
}
