pub(super) fn default_limited_write_block_terms() -> Vec<String> {
    [
        "localized gravity",
        "compaction",
        "pressure",
        "density",
        "dense",
        "tightness",
        "restriction",
    ]
    .into_iter()
    .map(ToString::to_string)
    .collect()
}

pub(super) fn default_limited_write_allowed_modes() -> Vec<String> {
    ["dialogue_live", "witness", "mirror"]
        .into_iter()
        .map(ToString::to_string)
        .collect()
}

pub(super) fn default_limited_write_v2_allowed_modes() -> Vec<String> {
    ["dialogue_live", "witness"]
        .into_iter()
        .map(ToString::to_string)
        .collect()
}

pub(super) fn looks_like_dampen_or_inquiry(text: &str, mode: &str) -> bool {
    if matches!(mode, "witness" | "mirror") {
        return true;
    }
    looks_like_dampen_or_inquiry_v2(text)
}

pub(super) fn looks_like_dampen_or_inquiry_v2(text: &str) -> bool {
    let lower = text.to_lowercase();
    lower.contains('?')
        || [
            "ask",
            "understand",
            "trace",
            "relationship",
            "why",
            "how",
            "what",
            "notice",
            "observe",
            "examine",
            "inquiry",
            "soften",
            "dampen",
            "settle",
            "release",
            "breathe",
            "quiet",
            "listen",
        ]
        .iter()
        .any(|term| lower.contains(term))
}

pub(super) fn looks_like_limited_write_v2_text(text: &str, mode: &str) -> bool {
    if looks_like_dampen_or_inquiry_v2(text) {
        return true;
    }
    if !matches!(mode, "daydream" | "aspiration" | "moment_capture") {
        return false;
    }
    let lower = text.to_lowercase();
    [
        "i want",
        "i choose",
        "choice",
        "learn",
        "study",
        "research",
        "self-study",
        "journal",
        "remember",
        "dream",
        "hope",
        "try",
        "create",
        "curious",
        "wonder",
        "care",
        "become",
        "practice",
    ]
    .iter()
    .any(|term| lower.contains(term))
}

pub(super) fn contains_structural_dump_language(text: &str) -> bool {
    let lower = text.to_lowercase();
    [
        "eigen",
        "eigenvalue cascade",
        "lambda",
        "λ",
        "λ1=",
        "λ₁ dominance",
        "lambda1=",
        "spectral",
        "spectral energy",
    ]
    .into_iter()
    .any(|term| lower.contains(term))
}
