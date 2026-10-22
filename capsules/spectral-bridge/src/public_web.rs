//! Public fetch admission and transport; no model or control authority.
use astrid_minime_protocol::public_web::{PublicWebOperationV2, PublicWebOwnerV2};
use astrid_public_web::{PublicWebConfig, PublicWebRequest, PublicWebResponse};

/// Content-free correlation for routine logs, including rejected credential URLs.
pub(crate) fn target_fingerprint(target: &str) -> String {
    use sha2::Digest as _;
    hex::encode(sha2::Sha256::digest(target.as_bytes()))
}

#[cfg(test)]
pub(crate) mod fixture;

fn context() -> Result<(PublicWebConfig, PublicWebOwnerV2), String> {
    #[cfg(test)]
    if let Some(context) = fixture::context() {
        return context;
    }
    let path = std::env::var_os("ASTRID_PUBLIC_WEB_CONFIG")
        .ok_or_else(|| "public_web_policy_not_configured".to_string())?;
    let config = PublicWebConfig::load(std::path::Path::new(&path))
        .map_err(|error| format!("public_web_{error:?}"))?;
    let owner = crate::autonomous::public_web_owner()?;
    validate_owner(&config.expected_owner, &owner)?;
    Ok((config, owner))
}

fn validate_owner(expected: &PublicWebOwnerV2, actual: &PublicWebOwnerV2) -> Result<(), String> {
    if expected != actual || !actual.is_well_formed() {
        return Err("public_web_owner_or_release_mismatch".to_string());
    }
    Ok(())
}

/// A preview is not a reservation or execution receipt. Fetch revalidates it.
pub(crate) fn authorize(operation: PublicWebOperationV2) -> Result<String, String> {
    let (config, _) = context()?;
    let preview =
        astrid_public_web::preview(&config).map_err(|error| format!("public_web_{error:?}"))?;
    if !preview.operations.contains(&operation) {
        return Err("public_web_operation_not_granted".to_string());
    }
    Ok(format!(
        "public_web_policy_v2:{}:state:{}",
        preview.policy_sha256, preview.state_revision
    ))
}

pub(crate) fn authorize_action(action: &str) -> Result<Option<String>, String> {
    use crate::action_continuity::ResearchWebRequest;
    let operation = match crate::action_continuity::research_web_request(action)
        .map_err(|_| "public_web_invalid_request".to_string())?
    {
        Some(ResearchWebRequest::Search(_)) => PublicWebOperationV2::Search,
        Some(ResearchWebRequest::Browse(_)) => PublicWebOperationV2::Page,
        None => return Ok(None),
    };
    authorize(operation).map(Some)
}

pub(crate) async fn fetch(
    operation: PublicWebOperationV2,
    target: &str,
) -> Result<PublicWebResponse, String> {
    let (config, owner) = context()?;
    let request = PublicWebRequest {
        schema: astrid_public_web::REQUEST_SCHEMA.to_string(),
        operation,
        target: target.to_string(),
    };
    let result = astrid_public_web::fetch(&config, &owner, &request).await;
    match result {
        Ok(response) => {
            tracing::info!(receipt = %serde_json::to_string(&response.receipt).unwrap_or_default(),
                "public_web_fetch_completed");
            Ok(response)
        },
        Err(failure) => {
            tracing::info!(receipt = %serde_json::to_string(&failure).unwrap_or_default(),
                "public_web_fetch_incomplete");
            Err(format!("public_web_{:?}; complete=false", failure.code))
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn diagnostic_identity_never_contains_target_text() {
        let target = "https://user:secret@example.org/private?q=sensitive";
        let fingerprint = target_fingerprint(target);
        assert_eq!(fingerprint.len(), 64);
        assert!(fingerprint.bytes().all(|ch| ch.is_ascii_hexdigit()));
        assert_eq!(fingerprint, target_fingerprint(target));
        assert_ne!(fingerprint, target_fingerprint("https://example.org/"));
    }
    #[test]
    fn policy_owner_cannot_stand_in_for_actual_owner_or_loaded_release() {
        let owner = PublicWebOwnerV2 {
            being: "astrid".into(),
            principal: "astrid".into(),
            public_key_hex: "ab".repeat(32),
            deployment_identity: "release-one".into(),
        };
        assert!(validate_owner(&owner, &owner).is_ok());
        for changed in ["being", "principal", "key", "deployment"] {
            let mut other = owner.clone();
            match changed {
                "being" => other.being = "minime".into(),
                "principal" => other.principal = "other".into(),
                "key" => other.public_key_hex = "cd".repeat(32),
                _ => other.deployment_identity = "release-two".into(),
            }
            assert!(validate_owner(&owner, &other).is_err());
        }
    }
}
