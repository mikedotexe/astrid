use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};

use super::{NextActionContext, bridge_paths, truncate_str};

const STATUS_FILE: &str = "resource_governor_status.json";
const HEALTHY_SAMPLE_SECS: f64 = 600.0;

#[derive(Debug, Clone)]
pub(super) struct ResourceGovernorStatus {
    pub allowed_live: bool,
    pub primary_block_reason: Option<String>,
    pub block_reasons: Vec<String>,
    pub warnings: Vec<String>,
    pub memory_free_pct: Option<f64>,
    pub memory_free_delta: Option<f64>,
    pub used_mem_gb: Option<f64>,
    pub swapouts: Option<u64>,
    pub swapouts_delta: u64,
    pub runaway_git_add: Vec<String>,
    pub shadow_log_mb: Option<f64>,
}

impl ResourceGovernorStatus {
    pub(super) fn summary_line(&self) -> String {
        format!(
            "Resource governor: allowed_live={} block={} free={:?}% free_delta={:?} used_gb={:?} swapouts={:?} swapouts_delta={} warnings={:?} blocks={:?}",
            self.allowed_live,
            self.primary_block_reason.as_deref().unwrap_or("none"),
            self.memory_free_pct,
            self.memory_free_delta,
            self.used_mem_gb,
            self.swapouts,
            self.swapouts_delta,
            self.warnings,
            self.block_reasons,
        )
    }
}

pub(super) fn status(ctx: &NextActionContext<'_>, write: bool) -> ResourceGovernorStatus {
    let workspace = minime_workspace(ctx);
    let budget = read_json(&workspace.join("resource_budget.json")).unwrap_or(Value::Null);
    let previous = read_json(&workspace.join(STATUS_FILE)).unwrap_or(Value::Null);
    let memory_pressure = command_text("memory_pressure", &[]);
    let vm_stat = command_text("vm_stat", &[]);
    let swapusage = command_text("sysctl", &["-n", "vm.swapusage"]);
    let total_mem = command_text("sysctl", &["-n", "hw.memsize"]);
    let pgrep = command_text("pgrep", &["-fl", "git add"]);

    let memory_free_pct = parse_memory_free_pct(&memory_pressure);
    let swapouts = parse_vm_stat_count(&vm_stat, "Swapouts");
    let pageouts = parse_vm_stat_count(&vm_stat, "Pageouts");
    let total_mem_gb = total_mem
        .trim()
        .parse::<f64>()
        .ok()
        .map(|bytes| bytes / 1_073_741_824.0);
    let used_mem_gb = total_mem_gb
        .zip(memory_free_pct)
        .map(|(total, free)| ((total * (1.0 - (free / 100.0))) * 1000.0).round() / 1000.0);
    let now = unix_now();
    let previous_fresh = previous
        .get("timestamp_unix_s")
        .and_then(Value::as_f64)
        .is_some_and(|then| now >= then && now - then <= HEALTHY_SAMPLE_SECS);
    let previous_swapouts = previous_fresh
        .then(|| previous.get("swapouts").and_then(Value::as_u64))
        .flatten();
    let swapouts_delta = swapouts
        .zip(previous_swapouts)
        .map_or(0, |(current, prior)| current.saturating_sub(prior));
    let previous_free = previous_fresh
        .then(|| previous.get("memory_free_pct").and_then(Value::as_f64))
        .flatten();
    let memory_free_delta = memory_free_pct
        .zip(previous_free)
        .map(|(current, prior)| ((current - prior) * 1000.0).round() / 1000.0);

    let soft_cap = budget
        .get("soft_ram_cap_gb")
        .and_then(Value::as_f64)
        .unwrap_or(48.0);
    let hard_cap = budget
        .get("hard_ram_cap_gb")
        .and_then(Value::as_f64)
        .unwrap_or(56.0);
    let log_cap = budget
        .get("jsonl_log_cap_mb")
        .and_then(Value::as_f64)
        .unwrap_or(256.0);
    let shadow_log = Path::new("/Users/v/other/neural-triple-reservoir/state/shadow_metrics.jsonl");
    let shadow_log_mb = std::fs::metadata(shadow_log)
        .ok()
        .map(|meta| (meta.len() as f64 / 1_048_576.0 * 1000.0).round() / 1000.0);
    let runaway_git_add = runaway_git_add_lines(&pgrep);
    let pressure_lower = memory_pressure.to_ascii_lowercase();

    let mut block_reasons = Vec::new();
    let mut warnings = Vec::new();
    if !runaway_git_add.is_empty() {
        push_unique(&mut block_reasons, "runaway_git_add");
    }
    if shadow_log_mb.is_some_and(|mb| mb >= log_cap) {
        push_unique(&mut block_reasons, "shadow_log_over_cap");
    }
    if used_mem_gb.is_some_and(|used| used >= hard_cap) {
        push_unique(&mut block_reasons, "hard_ram_cap");
    } else if used_mem_gb.is_some_and(|used| used >= soft_cap) {
        push_unique(&mut warnings, "soft_ram_cap");
    }
    if memory_free_pct.is_some_and(|free| free <= 8.0) {
        push_unique(&mut block_reasons, "memory_pressure_critical");
    } else if memory_free_pct.is_some_and(|free| free <= 20.0)
        && memory_free_delta.is_some_and(|delta| delta <= -5.0)
    {
        push_unique(&mut block_reasons, "memory_free_falling");
    }
    if pressure_lower.contains("critical") || pressure_lower.contains("urgent") {
        push_unique(&mut block_reasons, "memory_pressure_critical");
    } else if pressure_lower.contains("warning") {
        push_unique(&mut warnings, "memory_pressure_warning");
    }
    if swapouts_delta > 0 {
        push_unique(&mut block_reasons, "swapouts_rising");
    }

    let allowed_live = block_reasons.is_empty();
    let primary_block_reason = block_reasons.first().cloned();
    let json = json!({
        "schema_version": 1,
        "policy": "m4_resource_governor_v1",
        "timestamp_unix_s": now,
        "posture": budget.get("posture").and_then(Value::as_str).unwrap_or("ambitious_lab"),
        "allowed_live": allowed_live,
        "primary_block_reason": primary_block_reason.clone(),
        "block_reasons": block_reasons,
        "warnings": warnings,
        "memory_free_pct": memory_free_pct,
        "memory_free_delta": memory_free_delta,
        "used_mem_gb": used_mem_gb,
        "soft_ram_cap_gb": soft_cap,
        "hard_ram_cap_gb": hard_cap,
        "swapouts": swapouts,
        "swapouts_delta": swapouts_delta,
        "pageouts": pageouts,
        "swapusage": swapusage,
        "runaway_git_add": runaway_git_add,
        "shadow_metrics_jsonl_mb": shadow_log_mb,
        "jsonl_log_cap_mb": log_cap,
        "live_influence_policy": budget.get("live_influence_policy").and_then(Value::as_str),
    });
    if write {
        let path = workspace.join(STATUS_FILE);
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(text) = serde_json::to_string_pretty(&json) {
            let _ = std::fs::write(path, format!("{text}\n"));
        }
    }
    ResourceGovernorStatus {
        allowed_live,
        primary_block_reason,
        block_reasons: json
            .get("block_reasons")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default(),
        warnings: json
            .get("warnings")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default(),
        memory_free_pct,
        memory_free_delta,
        used_mem_gb,
        swapouts,
        swapouts_delta,
        runaway_git_add: json
            .get("runaway_git_add")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default(),
        shadow_log_mb,
    }
}

pub(super) fn audit_text(ctx: &NextActionContext<'_>) -> String {
    let workspace = minime_workspace(ctx);
    let budget = read_json(&workspace.join("resource_budget.json")).unwrap_or(Value::Null);
    let governor = status(ctx, true);
    let rollup =
        Path::new("/Users/v/other/neural-triple-reservoir/state/shadow_metrics_rollup.json");
    let rollup_kb = std::fs::metadata(rollup)
        .ok()
        .map(|meta| meta.len() as f64 / 1024.0);
    let memory = command_text("memory_pressure", &[]);
    format!(
        "M4 resource status:\n  Budget: {}\n  {}\n  Reservoir shadow log MB: {:?}\n  Reservoir shadow rollup KB: {:?}\n  Runaway git add: {}\n\nMemory pressure excerpt:\n{}\n\nPolicy: ambitious lab, but no new live influence if swapouts, runaway jobs, hard RAM pressure, or log caps are rising.",
        truncate_str(&budget.to_string(), 600),
        governor.summary_line(),
        governor.shadow_log_mb,
        rollup_kb,
        if governor.runaway_git_add.is_empty() {
            "none".to_string()
        } else {
            governor.runaway_git_add.join(" | ")
        },
        truncate_str(&memory, 1200),
    )
}

fn minime_workspace(ctx: &NextActionContext<'_>) -> PathBuf {
    ctx.workspace.map_or_else(
        || bridge_paths().minime_workspace().to_path_buf(),
        Path::to_path_buf,
    )
}

fn read_json(path: &Path) -> Option<Value> {
    let text = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&text).ok()
}

fn command_text(program: &str, args: &[&str]) -> String {
    for candidate in command_candidates(program) {
        if candidate.starts_with('/') && !Path::new(candidate).exists() {
            continue;
        }
        let Some(text) = run_command_text(candidate, args) else {
            continue;
        };
        return text;
    }
    String::new()
}

fn command_candidates(program: &str) -> Vec<&'static str> {
    match program {
        "memory_pressure" => vec!["/usr/bin/memory_pressure", "memory_pressure"],
        "vm_stat" => vec!["/usr/bin/vm_stat", "vm_stat"],
        "sysctl" => vec!["/usr/sbin/sysctl", "sysctl"],
        "pgrep" => vec!["/usr/bin/pgrep", "pgrep"],
        _ => Vec::new(),
    }
}

fn run_command_text(program: &str, args: &[&str]) -> Option<String> {
    let out = Command::new(program).args(args).output().ok()?;
    let stdout = String::from_utf8_lossy(&out.stdout);
    let text = if stdout.trim().is_empty() {
        String::from_utf8_lossy(&out.stderr).to_string()
    } else {
        stdout.to_string()
    };
    Some(text.trim().to_string())
}

fn parse_memory_free_pct(text: &str) -> Option<f64> {
    let marker = "System-wide memory free percentage:";
    let after = text.split(marker).nth(1)?;
    let value = after.split('%').next()?.trim();
    value.parse::<f64>().ok()
}

fn parse_vm_stat_count(text: &str, key: &str) -> Option<u64> {
    for line in text.lines() {
        let trimmed = line.trim();
        let Some(rest) = trimmed.strip_prefix(key) else {
            continue;
        };
        let rest = rest.strip_prefix(':')?.trim();
        let digits = rest
            .chars()
            .take_while(|ch| ch.is_ascii_digit() || *ch == ',')
            .filter(|ch| *ch != ',')
            .collect::<String>();
        if !digits.is_empty() {
            return digits.parse::<u64>().ok();
        }
    }
    None
}

fn runaway_git_add_lines(text: &str) -> Vec<String> {
    text.lines()
        .map(str::trim)
        .filter(|line| {
            let lower = line.to_ascii_lowercase();
            !line.is_empty() && lower.contains("git add") && !lower.contains("pgrep")
        })
        .map(str::to_string)
        .collect()
}

fn push_unique(items: &mut Vec<String>, value: &str) {
    if !items.iter().any(|item| item == value) {
        items.push(value.to_string());
    }
}

fn unix_now() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0.0, |duration| duration.as_secs_f64())
}

#[cfg(test)]
mod tests {
    use super::{
        command_candidates, parse_memory_free_pct, parse_vm_stat_count, runaway_git_add_lines,
    };

    #[test]
    fn parses_memory_pressure_free_percentage() {
        let text = "System-wide memory free percentage: 69%";
        assert_eq!(parse_memory_free_pct(text), Some(69.0));
    }

    #[test]
    fn parses_vm_stat_counts() {
        let text = "Swapouts: 636.\nPageouts: 1,003,469.";
        assert_eq!(parse_vm_stat_count(text, "Swapouts"), Some(636));
        assert_eq!(parse_vm_stat_count(text, "Pageouts"), Some(1_003_469));
    }

    #[test]
    fn filters_runaway_git_add_lines() {
        let lines =
            runaway_git_add_lines("123 /usr/bin/git add -A\n456 pgrep -fl git add\n789 git status");
        assert_eq!(lines, vec!["123 /usr/bin/git add -A"]);
    }

    #[test]
    fn command_candidates_prefer_launchd_safe_paths() {
        assert_eq!(
            command_candidates("sysctl").first(),
            Some(&"/usr/sbin/sysctl")
        );
        assert_eq!(
            command_candidates("memory_pressure").first(),
            Some(&"/usr/bin/memory_pressure")
        );
        assert!(command_candidates("unknown").is_empty());
    }
}
