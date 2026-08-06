use std::fs;
use std::path::Path;

use super::catalog::sha256_bytes;
use super::{
    ClaimChallengeReportV2, ClaimKindV2, ClaimSupportRefV2, ClaimSupportStateV2, SourceEvidenceV3,
};

const ABSENCE_MARKERS: &[&str] = &[
    " is absent",
    " are absent",
    " is missing",
    " are missing",
    " does not implement",
    " doesn't implement",
    " not implemented",
    " lacks ",
    " no mechanism",
    " no schema",
    " no function",
];
const NEW_IMPLEMENTATION_MARKERS: &[&str] = &[
    " now implements",
    " newly implements",
    " has been implemented",
    " was implemented",
    " was added",
    " newly added",
    " introduced ",
];
const SOURCE_ATTRIBUTION_MARKERS: &[&str] = &[
    "source defines",
    "source code defines",
    "code defines",
    "source establishes",
    "source code establishes",
    "code establishes",
    "in the source code `",
    "source shows",
    "code shows",
    "source contains",
    "code contains",
    "source captures",
    "code captures",
    "source exposes",
    "code exposes",
    "source implements",
    "code implements",
    "source declares",
    "code declares",
    "thing i notice is the",
    " is instantiated",
    "i can see the ",
    "i also see the `",
    "code identifies",
    "in the source,",
    "file defines",
    "file establishes",
    "file describes",
    "file labels",
    "documentation defines",
    "documentation establishes",
    "documentation describes",
    "documentation labels",
    "code describes",
    "code labels",
    "i see the definition of",
    " defines `",
    " defines the `",
    "i see the `",
    " as a struct",
    " as a function",
    " flow shows ",
];
const HYPOTHETICAL_MARKERS: &[&str] = &[
    "i suggest",
    "suggested",
    "i propose",
    "proposed",
    "recommend",
    "should add",
    "could add",
    "would add",
];

pub(super) fn challenge_response_claims_v3(
    response: &str,
    path: &Path,
    evidence: &SourceEvidenceV3,
) -> ClaimChallengeReportV2 {
    let claims = detected_claims(response);
    let source = fs::read(path).ok();
    let source_hash_current = source
        .as_deref()
        .is_some_and(|bytes| sha256_bytes(bytes) == evidence.source_map_v3.source_sha256);
    let source_text = source
        .as_deref()
        .and_then(|bytes| std::str::from_utf8(bytes).ok());
    let support_refs = claims
        .into_iter()
        .map(|(kind, claim)| {
            challenge_claim(kind, &claim, source_text, source_hash_current, evidence)
        })
        .collect::<Vec<_>>();
    ClaimChallengeReportV2 {
        schema: "claim_challenge_report_v2".to_string(),
        schema_version: 2,
        all_supported: support_refs
            .iter()
            .all(|support| support.support_state == ClaimSupportStateV2::Supported),
        challenged_claim_count: support_refs.len(),
        support_refs,
    }
}

fn challenge_claim(
    kind: ClaimKindV2,
    claim: &str,
    source: Option<&str>,
    source_hash_current: bool,
    evidence: &SourceEvidenceV3,
) -> ClaimSupportRefV2 {
    let identifiers = backticked_identifiers(claim);
    let (support_state, reason) = if !source_hash_current {
        (
            ClaimSupportStateV2::Rejected,
            "source bytes no longer match the mapped source hash".to_string(),
        )
    } else if identifiers.is_empty() {
        (
            ClaimSupportStateV2::Rejected,
            "claim has no exact backticked identifier to challenge".to_string(),
        )
    } else {
        match (kind, source) {
            (ClaimKindV2::Absence, Some(source)) => {
                if identifiers
                    .iter()
                    .all(|identifier| token_lines(source, identifier).is_empty())
                {
                    (
                        ClaimSupportStateV2::Supported,
                        "whole-source exact-token challenge found no named identifier".to_string(),
                    )
                } else {
                    (
                        ClaimSupportStateV2::Rejected,
                        "whole-source exact-token challenge found a named identifier".to_string(),
                    )
                }
            },
            (ClaimKindV2::NewImplementation, Some(source)) => {
                let present = identifiers
                    .iter()
                    .all(|identifier| !token_lines(source, identifier).is_empty());
                let read = identifiers.iter().all(|identifier| {
                    token_lines(source, identifier).iter().any(|line| {
                        evidence
                            .read_session_checkpoint_v3
                            .included_intervals
                            .iter()
                            .any(|interval| interval.contains_line(*line))
                    })
                });
                if !present {
                    (
                        ClaimSupportStateV2::Rejected,
                        "whole-source exact-token challenge did not find every named identifier"
                            .to_string(),
                    )
                } else if !read {
                    (
                        ClaimSupportStateV2::Rejected,
                        "named implementation exists only outside the included read intervals"
                            .to_string(),
                    )
                } else {
                    (
                        ClaimSupportStateV2::RequiresChangeEvidence,
                        "a current source snapshot proves presence, not that an implementation is new"
                            .to_string(),
                    )
                }
            },
            (ClaimKindV2::SourceAttribution, Some(source)) => {
                let requires_included_bytes = source_attribution_requires_included_bytes(claim);
                let unsupported = identifiers
                    .iter()
                    .filter(|identifier| {
                        !source_attribution_supported(
                            identifier,
                            source,
                            evidence,
                            requires_included_bytes,
                        )
                    })
                    .cloned()
                    .collect::<Vec<_>>();
                if unsupported.is_empty() {
                    (
                        ClaimSupportStateV2::Supported,
                        if requires_included_bytes {
                            "every named identifier is present in the included source bytes named by the claim"
                                .to_string()
                        } else {
                            "every named identifier is present in included source bytes or the whole-source structural map"
                                .to_string()
                        },
                    )
                } else {
                    (
                        ClaimSupportStateV2::Rejected,
                        if requires_included_bytes {
                            format!(
                                "named identifiers lack included-byte support for the source window asserted by the claim: {}",
                                unsupported.join(", ")
                            )
                        } else {
                            format!(
                                "named identifiers lack included-byte or structural-map support: {}",
                                unsupported.join(", ")
                            )
                        },
                    )
                }
            },
            (_, None) => (
                ClaimSupportStateV2::Rejected,
                "source could not be read as UTF-8 for an exact challenge".to_string(),
            ),
        }
    };
    ClaimSupportRefV2 {
        schema: "claim_support_ref_v2".to_string(),
        schema_version: 2,
        claim_sha256: sha256_bytes(claim.as_bytes()),
        claim_kind: kind,
        exact_identifiers: identifiers,
        source_identity: evidence.source_map_v3.source_identity.clone(),
        source_sha256: evidence.source_map_v3.source_sha256.clone(),
        structural_map_sha256: evidence.source_map_v3.structural_map_sha256.clone(),
        read_session_id: evidence.read_session_checkpoint_v3.read_session_id.clone(),
        whole_source_challenge_performed: source.is_some(),
        source_hash_current,
        support_state,
        reason,
        artifact_authority: "claim_support_evidence_not_runtime_or_deployment_proof".to_string(),
    }
}

fn detected_claims(response: &str) -> Vec<(ClaimKindV2, String)> {
    let mut claims = Vec::new();
    for line in response
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
    {
        let lower = line.to_ascii_lowercase();
        let absence = ABSENCE_MARKERS.iter().any(|marker| lower.contains(marker));
        let new_implementation = NEW_IMPLEMENTATION_MARKERS
            .iter()
            .any(|marker| lower.contains(marker));
        if absence {
            claims.push((ClaimKindV2::Absence, line.to_string()));
        }
        if new_implementation {
            claims.push((ClaimKindV2::NewImplementation, line.to_string()));
        }
        if !absence
            && !new_implementation
            && SOURCE_ATTRIBUTION_MARKERS
                .iter()
                .any(|marker| lower.contains(marker))
            && !HYPOTHETICAL_MARKERS
                .iter()
                .any(|marker| lower.contains(marker))
        {
            claims.push((ClaimKindV2::SourceAttribution, line.to_string()));
        }
    }
    claims.sort_by(|left, right| {
        format!("{:?}\0{}", left.0, left.1).cmp(&format!("{:?}\0{}", right.0, right.1))
    });
    claims.dedup();
    claims
}

fn source_attribution_supported(
    identifier: &str,
    source: &str,
    evidence: &SourceEvidenceV3,
    requires_included_bytes: bool,
) -> bool {
    let lines = token_lines(source, identifier);
    if lines.is_empty() {
        return false;
    }
    let included = lines.iter().any(|line| {
        evidence
            .read_session_checkpoint_v3
            .included_intervals
            .iter()
            .any(|interval| interval.contains_line(*line))
    });
    included
        || (!requires_included_bytes
            && evidence
                .source_map_v3
                .entries
                .iter()
                .any(|entry| entry.label == identifier))
}

fn source_attribution_requires_included_bytes(claim: &str) -> bool {
    let lower = claim.to_ascii_lowercase();
    lower.contains("source window")
        || lower.contains("included source")
        || lower.contains("included lines")
        || lower.contains("early on")
        || (lower.contains("first ") && lower.contains(" lines"))
        || (lower.contains("initial ") && lower.contains(" lines"))
}

fn backticked_identifiers(response: &str) -> Vec<String> {
    let mut values = Vec::new();
    let mut rest = response;
    while let Some((_, tail)) = rest.split_once('`') {
        let Some((candidate, after)) = tail.split_once('`') else {
            break;
        };
        rest = after;
        let candidate = candidate.trim();
        let without_kind = [
            "fn ", "struct ", "enum ", "trait ", "class ", "def ", "schema ",
        ]
        .iter()
        .find_map(|prefix| candidate.strip_prefix(prefix))
        .unwrap_or(candidate);
        let identifier = without_kind
            .split(['(', '<', ' ', ':'])
            .next()
            .unwrap_or("")
            .trim_matches(|character: char| {
                !(character.is_ascii_alphanumeric() || character == '_')
            });
        if identifier.len() >= 3
            && identifier
                .chars()
                .all(|character| character.is_ascii_alphanumeric() || character == '_')
        {
            values.push(identifier.to_string());
        }
    }
    values.sort();
    values.dedup();
    values
}

fn token_lines(source: &str, identifier: &str) -> Vec<usize> {
    source
        .lines()
        .enumerate()
        .filter_map(|(index, line)| {
            contains_exact_token(line, identifier).then_some(index.saturating_add(1))
        })
        .collect()
}

fn contains_exact_token(line: &str, identifier: &str) -> bool {
    line.match_indices(identifier).any(|(start, _)| {
        let end = start.saturating_add(identifier.len());
        let before = line[..start].chars().next_back();
        let after = line[end..].chars().next();
        !before.is_some_and(is_identifier_character) && !after.is_some_and(is_identifier_character)
    })
}

fn is_identifier_character(character: char) -> bool {
    character.is_ascii_alphanumeric() || character == '_'
}
