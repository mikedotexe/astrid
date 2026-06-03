use astrid_guest::{capsule_result, fs, ipc, serde_json, tool};
use serde_json::Value;

struct FsCapsule;

type ToolHandler = fn(&Value) -> Result<String, String>;

impl astrid_guest::Guest for FsCapsule {
    fn astrid_hook_trigger(action: String, payload: Vec<u8>) -> astrid_guest::CapsuleResult {
        match action.as_str() {
            "tool_execute_read_file" => handle_tool(&payload, read_file),
            "tool_execute_write_file" => handle_tool(&payload, write_file),
            "tool_execute_replace_in_file" => handle_tool(&payload, replace_in_file),
            "tool_execute_list_directory" => handle_tool(&payload, list_directory),
            "tool_execute_grep_search" => handle_tool(&payload, grep_search),
            "tool_execute_create_directory" => handle_tool(&payload, create_directory),
            "tool_execute_delete_file" => handle_tool(&payload, delete_file),
            "tool_execute_move_file" => handle_tool(&payload, move_file),
            "tool_describe" => describe(),
            _ => capsule_result::continue_empty(),
        }
    }

    fn run() {}

    fn astrid_install() {}

    fn astrid_upgrade() {}
}

fn handle_tool(payload: &[u8], handler: ToolHandler) -> astrid_guest::CapsuleResult {
    let request = match tool::parse_request(payload) {
        Ok(request) => request,
        Err(err) => return capsule_result::deny(err),
    };
    match handler(&request.arguments) {
        Ok(content) => tool::publish_success(&request.call_id, &request.tool_name, content),
        Err(err) => tool::publish_error(&request.call_id, &request.tool_name, err),
    }
}

fn read_file(args: &Value) -> Result<String, String> {
    let path = tool::required_string_arg(args, "path")?;
    let content = fs::read_text(&path)?;
    let start = tool::u64_arg(args, "start_line")
        .or_else(|| tool::u64_arg(args, "start"))
        .unwrap_or(1);
    let end = tool::u64_arg(args, "end_line").or_else(|| tool::u64_arg(args, "end"));
    if start <= 1 && end.is_none() {
        return Ok(content);
    }
    let selected = content
        .lines()
        .enumerate()
        .filter_map(|(idx, line)| {
            let line_no = idx as u64 + 1;
            let within_start = line_no >= start;
            let within_end = end.is_none_or(|max| line_no <= max);
            (within_start && within_end).then(|| format!("{line_no}: {line}"))
        })
        .collect::<Vec<_>>()
        .join("\n");
    Ok(selected)
}

fn write_file(args: &Value) -> Result<String, String> {
    let path = tool::required_string_arg(args, "path")?;
    let content = tool::string_arg(args, "content")
        .or_else(|| tool::string_arg(args, "text"))
        .ok_or_else(|| "missing string argument `content`".to_string())?;
    fs::write_text(&path, &content)?;
    Ok(format!("Wrote {} bytes to {path}", content.len()))
}

fn replace_in_file(args: &Value) -> Result<String, String> {
    let path = tool::required_string_arg(args, "path")?;
    let old_text = tool::required_string_arg(args, "old_text")?;
    let new_text = tool::required_string_arg(args, "new_text")?;
    let content = fs::read_text(&path)?;
    if !content.contains(&old_text) {
        return Err("old_text was not found".to_string());
    }
    let replaced = content.replace(&old_text, &new_text);
    fs::write_text(&path, &replaced)?;
    Ok(format!("Updated {path}"))
}

fn list_directory(args: &Value) -> Result<String, String> {
    let path = tool::string_arg(args, "path").unwrap_or_else(|| ".".to_string());
    let mut entries = fs::readdir(&path)?;
    entries.sort();
    Ok(entries.join("\n"))
}

fn grep_search(args: &Value) -> Result<String, String> {
    let pattern = tool::required_string_arg(args, "pattern")?;
    let path = tool::string_arg(args, "path").unwrap_or_else(|| ".".to_string());
    let max_results = tool::u64_arg(args, "max_results").unwrap_or(50) as usize;
    let mut results = Vec::new();
    grep_path(&path, &pattern, max_results, &mut results);
    Ok(results.join("\n"))
}

fn create_directory(args: &Value) -> Result<String, String> {
    let path = tool::required_string_arg(args, "path")?;
    fs::mkdir(&path)?;
    Ok(format!("Created directory {path}"))
}

fn delete_file(args: &Value) -> Result<String, String> {
    let path = tool::required_string_arg(args, "path")?;
    fs::unlink(&path)?;
    Ok(format!("Deleted {path}"))
}

fn move_file(args: &Value) -> Result<String, String> {
    let from = tool::string_arg(args, "from")
        .or_else(|| tool::string_arg(args, "source"))
        .ok_or_else(|| "missing string argument `from`".to_string())?;
    let to = tool::string_arg(args, "to")
        .or_else(|| tool::string_arg(args, "destination"))
        .ok_or_else(|| "missing string argument `to`".to_string())?;
    let content = fs::read_text(&from)?;
    fs::write_text(&to, &content)?;
    fs::unlink(&from)?;
    Ok(format!("Moved {from} to {to}"))
}

fn grep_path(path: &str, pattern: &str, max_results: usize, results: &mut Vec<String>) {
    if results.len() >= max_results {
        return;
    }
    match fs::is_dir(path) {
        Ok(true) => {
            let Ok(entries) = fs::readdir(path) else {
                return;
            };
            for entry in entries {
                if matches!(entry.as_str(), ".git" | "target" | "node_modules") {
                    continue;
                }
                let child = join_path(path, &entry);
                grep_path(&child, pattern, max_results, results);
                if results.len() >= max_results {
                    return;
                }
            }
        },
        Ok(false) => {
            let Ok(content) = fs::read_text(path) else {
                return;
            };
            for (idx, line) in content.lines().enumerate() {
                if line.contains(pattern) {
                    results.push(format!("{}:{}: {}", path, idx + 1, line));
                    if results.len() >= max_results {
                        return;
                    }
                }
            }
        },
        Err(_) => {},
    }
}

fn join_path(base: &str, entry: &str) -> String {
    if base == "." || base.is_empty() {
        entry.to_string()
    } else if base.ends_with('/') {
        format!("{base}{entry}")
    } else {
        format!("{base}/{entry}")
    }
}

fn describe() -> astrid_guest::CapsuleResult {
    let payload = serde_json::json!({
        "capsule": "astrid-capsule-fs",
        "tools": [
            {"name": "read_file", "description": "Read a text file from the workspace or allowed home scope."},
            {"name": "write_file", "description": "Write a text file inside the workspace."},
            {"name": "replace_in_file", "description": "Replace text in a workspace file."},
            {"name": "list_directory", "description": "List entries in a directory."},
            {"name": "grep_search", "description": "Search text files for a literal pattern."},
            {"name": "create_directory", "description": "Create a workspace directory."},
            {"name": "delete_file", "description": "Delete a workspace file."},
            {"name": "move_file", "description": "Move a workspace file."}
        ]
    });
    match ipc::publish_json("tool.v1.response.describe.astrid-capsule-fs", &payload) {
        Ok(()) => capsule_result::continue_empty(),
        Err(err) => capsule_result::deny(err),
    }
}

astrid_guest::export!(FsCapsule);
