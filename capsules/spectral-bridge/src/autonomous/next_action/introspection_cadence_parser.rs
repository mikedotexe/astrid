use super::{IntrospectionCadenceTargetV1, MAX_CADENCE_EXCHANGES, MIN_CADENCE_EXCHANGES};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum CadenceCommandV1 {
    Status,
    Off,
    Every {
        every_exchanges: u16,
        target: Option<IntrospectionCadenceTargetV1>,
    },
}

pub(super) fn parse_command(original: &str) -> Result<CadenceCommandV1, String> {
    let args = super::super::strip_action(original, "INTROSPECTION_CADENCE");
    let mut parts = args.split_whitespace();
    let Some(command) = parts.next() else {
        return Err("choose STATUS, OFF, or EVERY <4..256> [target [offset]]".to_string());
    };
    if command.eq_ignore_ascii_case("STATUS") {
        if parts.next().is_some() {
            return Err("STATUS takes no arguments".to_string());
        }
        return Ok(CadenceCommandV1::Status);
    }
    if command.eq_ignore_ascii_case("OFF") {
        if parts.next().is_some() {
            return Err("OFF takes no arguments".to_string());
        }
        return Ok(CadenceCommandV1::Off);
    }
    if !command.eq_ignore_ascii_case("EVERY") {
        return Err("choose STATUS, OFF, or EVERY <4..256> [target [offset]]".to_string());
    }

    let interval_text = parts
        .next()
        .ok_or_else(|| "EVERY requires an exchange interval from 4 through 256".to_string())?;
    let every_exchanges = interval_text.parse::<u16>().map_err(|_| {
        "EVERY requires an integer exchange interval from 4 through 256".to_string()
    })?;
    if !(MIN_CADENCE_EXCHANGES..=MAX_CADENCE_EXCHANGES).contains(&every_exchanges) {
        return Err(format!(
            "cadence interval must be between {MIN_CADENCE_EXCHANGES} and {MAX_CADENCE_EXCHANGES} exchanges"
        ));
    }

    let target_label = parts.next();
    let target = if target_label.is_none_or(|label| {
        matches!(
            label.to_ascii_lowercase().as_str(),
            "rotation" | "next-in-rotation" | "next_in_rotation"
        )
    }) {
        None
    } else {
        let label = target_label.unwrap_or_default().to_string();
        let offset = parts
            .next()
            .map(str::parse::<usize>)
            .transpose()
            .map_err(|_| "target offset must be a non-negative integer".to_string())?;
        Some(IntrospectionCadenceTargetV1 { label, offset })
    };
    if parts.next().is_some() {
        return Err("EVERY accepts at most one target and one numeric offset".to_string());
    }
    Ok(CadenceCommandV1::Every {
        every_exchanges,
        target,
    })
}
