use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

pub const MINIME_MEMORY_BANK_PATH: &str =
    "/Users/v/other/minime/workspace/spectral_memory_bank.json";
pub const MINIME_MEMORY_REQUESTS_DIR: &str = "/Users/v/other/minime/workspace/memory_requests";
const ROLE_ORDER: [&str; 5] = ["latest", "stable", "expanding", "contracting", "transition"];

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct RemoteMemorySummary {
    pub id: String,
    pub role: String,
    pub timestamp_ms: u64,
    pub spectral_glimpse_12d: Vec<f32>,
    pub fill_pct: f32,
    pub lambda1_rel: f32,
    pub spread: f32,
    pub geom_rel: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct RemoteMemoryBankFile {
    #[serde(default)]
    pub version: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_memory_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_memory_role: Option<String>,
    #[serde(default)]
    pub entries: Vec<RemoteMemoryEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct RemoteMemoryEntry {
    pub id: String,
    pub role: String,
    pub timestamp_ms: u64,
    #[serde(default)]
    pub spectral_glimpse_12d: Vec<f32>,
    #[serde(default)]
    pub spectral_fingerprint: Vec<f32>,
    pub fill_pct: f32,
    pub lambda1_rel: f32,
    pub spread: f32,
    pub geom_rel: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryRecallRequest {
    pub request_id: String,
    pub requested_by: String,
    pub requested_at_unix: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory_id: Option<String>,
}

fn role_rank(role: &str) -> usize {
    ROLE_ORDER
        .iter()
        .position(|candidate| *candidate == role)
        .unwrap_or(ROLE_ORDER.len())
}

fn summarize_glimpse(glimpse: &[f32]) -> String {
    if glimpse.len() < 12 {
        return "shape unavailable".to_string();
    }
    format!(
        "dominant {:.2}, shoulder {:.2}, tail {:.2}, entropy {:.2}, gap {:.2}, rotation {:.2}, geom {:.2}",
        glimpse[0], glimpse[1], glimpse[2], glimpse[7], glimpse[8], glimpse[9], glimpse[10],
    )
}

pub fn read_remote_memory_bank() -> Vec<RemoteMemorySummary> {
    let path = Path::new(MINIME_MEMORY_BANK_PATH);
    let mut entries: Vec<RemoteMemorySummary> = fs::read_to_string(path)
        .ok()
        .and_then(|json| serde_json::from_str::<RemoteMemoryBankFile>(&json).ok())
        .map(|bank| {
            bank.entries
                .into_iter()
                .map(|entry| RemoteMemorySummary {
                    id: entry.id,
                    role: entry.role,
                    timestamp_ms: entry.timestamp_ms,
                    spectral_glimpse_12d: entry.spectral_glimpse_12d,
                    fill_pct: entry.fill_pct,
                    lambda1_rel: entry.lambda1_rel,
                    spread: entry.spread,
                    geom_rel: entry.geom_rel,
                })
                .collect()
        })
        .unwrap_or_default();
    entries.sort_by_key(|entry| role_rank(&entry.role));
    entries
}

pub fn format_memory_listing(
    entries: &[RemoteMemorySummary],
    selected_id: Option<&str>,
    selected_role: Option<&str>,
) -> String {
    if entries.is_empty() {
        return "[Minime vague-memory bank]\n  No remote memory entries are available yet."
            .to_string();
    }

    let selected_header = match (selected_role, selected_id) {
        (Some(role), Some(id)) => format!("Selected: {role} ({id})"),
        (Some(role), None) => format!("Selected: {role}"),
        (None, Some(id)) => format!("Selected id: {id}"),
        (None, None) => "Selected: (none)".to_string(),
    };

    let listing = entries
        .iter()
        .enumerate()
        .map(|(index, entry)| {
            let marker = if selected_id.is_some_and(|id| id == entry.id)
                || selected_role.is_some_and(|role| role == entry.role)
            {
                " [selected]"
            } else {
                ""
            };
            format!(
                "  {}. {} ({}){} — fill {:.1}%, λ₁_rel {:.2}, geom {:.2}, {}\n     id: {}",
                index.saturating_add(1),
                entry.role,
                entry.timestamp_ms,
                marker,
                entry.fill_pct,
                entry.lambda1_rel,
                entry.geom_rel,
                summarize_glimpse(&entry.spectral_glimpse_12d),
                entry.id,
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        "[Minime vague-memory bank]\n{selected_header}\n\n{listing}\n\nUse RECALL <role-or-id> to request one for the next restart."
    )
}

pub fn write_recall_request(requested_by: &str, target: &str) -> io::Result<PathBuf> {
    let requests_dir = Path::new(MINIME_MEMORY_REQUESTS_DIR);
    fs::create_dir_all(requests_dir)?;
    let requested_at_unix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let normalized = target.trim();
    let normalized_lower = normalized.to_lowercase();
    let request = MemoryRecallRequest {
        request_id: format!("recall_{requested_at_unix}"),
        requested_by: requested_by.to_string(),
        requested_at_unix,
        role: ROLE_ORDER
            .iter()
            .find(|role| **role == normalized_lower.as_str())
            .map(|role| (*role).to_string()),
        memory_id: if ROLE_ORDER
            .iter()
            .any(|role| *role == normalized_lower.as_str())
        {
            None
        } else {
            Some(normalized.to_string())
        },
    };

    let timestamped_path = requests_dir.join(format!("request_{requested_at_unix}.json"));
    let request_json = serde_json::to_string_pretty(&request).map_err(io::Error::other)?;
    fs::write(&timestamped_path, &request_json)?;
    fs::write(requests_dir.join("pending_recall.json"), request_json)?;
    Ok(timestamped_path)
}

pub fn format_glimpse_for_prompt(glimpse: &[f32], role: Option<&str>) -> Option<String> {
    if glimpse.len() < 12 {
        return None;
    }
    let prefix = role
        .map(|role| format!("12D quick-look ({role} memory)"))
        .unwrap_or_else(|| "12D quick-look".to_string());
    Some(format!("{prefix}: {}", summarize_glimpse(glimpse)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn listing_marks_selected_entry() {
        let entries = vec![RemoteMemorySummary {
            id: "memory_stable_1".to_string(),
            role: "stable".to_string(),
            timestamp_ms: 1,
            spectral_glimpse_12d: vec![0.1; 12],
            fill_pct: 20.0,
            lambda1_rel: 1.0,
            spread: 10.0,
            geom_rel: 0.9,
        }];
        let text = format_memory_listing(&entries, Some("memory_stable_1"), Some("stable"));
        assert!(text.contains("[selected]"));
        assert!(text.contains("RECALL <role-or-id>"));
    }
}
