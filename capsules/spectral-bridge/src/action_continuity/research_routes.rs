//! Read-only source classification uses the same target extraction as dispatch.
use super::*;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum ResearchWebRequest {
    Search(String),
    Browse(String),
}

pub(crate) fn research_url_arg(original: &str, action: &str, fallback: &str) -> Option<String> {
    let stripped = strip_action_arg(original, action);
    let raw_owned = if stripped.is_empty() {
        fallback.trim().to_string()
    } else {
        stripped
    };
    let raw = raw_owned
        .trim()
        .trim_matches(['"', '\'', '<', '>', '[', ']']);
    let url = raw
        .split(['<', '>', '[', ']', ' ', '\n'])
        .next()
        .unwrap_or(raw)
        .trim_end_matches(|c: char| !c.is_alphanumeric() && !"/-_~%?=&#".contains(c));
    url.starts_with("http").then(|| url.to_string())
}

pub(crate) fn research_web_request(action: &str) -> Result<Option<ResearchWebRequest>> {
    let canonical = crate::autonomous::canonicalize_next_action_text(action);
    let base = base_action(&canonical);
    if matches!(base.as_str(), "SEARCH" | "RESEARCH") {
        let topic =
            crate::autonomous::next_action::extract_search_topic(&canonical).unwrap_or_default();
        anyhow::ensure!(
            !topic.is_empty(),
            "bound search requires an exact authored query"
        );
        return Ok(Some(ResearchWebRequest::Search(topic)));
    }
    if matches!(base.as_str(), "BROWSE" | "EXAMINE") {
        if let Some(url) = research_url_arg(&canonical, &base, &canonical) {
            let parsed = reqwest::Url::parse(&url)?;
            anyhow::ensure!(
                matches!(parsed.scheme(), "http" | "https") && parsed.host_str().is_some(),
                "unsupported web source"
            );
            return Ok(Some(ResearchWebRequest::Browse(url)));
        }
        anyhow::ensure!(base != "BROWSE", "bound browse requires an exact URL");
    }
    Ok(None)
}
