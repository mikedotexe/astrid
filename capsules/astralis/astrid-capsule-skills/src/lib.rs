use astrid_guest::{capsule_result, fs, ipc, serde_json, tool};
use serde_json::Value;

struct SkillsCapsule;

impl astrid_guest::Guest for SkillsCapsule {
    fn astrid_hook_trigger(action: String, payload: Vec<u8>) -> astrid_guest::CapsuleResult {
        match action.as_str() {
            "tool_execute_list_skills" => handle_tool(&payload, list_skills),
            "tool_execute_read_skill" => handle_tool(&payload, read_skill),
            "tool_describe" => describe(),
            _ => capsule_result::continue_empty(),
        }
    }

    fn run() {}

    fn astrid_install() {}

    fn astrid_upgrade() {}
}

fn handle_tool(
    payload: &[u8],
    handler: fn(&Value) -> Result<String, String>,
) -> astrid_guest::CapsuleResult {
    let request = match tool::parse_request(payload) {
        Ok(request) => request,
        Err(err) => return capsule_result::deny(err),
    };
    match handler(&request.arguments) {
        Ok(content) => tool::publish_success(&request.call_id, &request.tool_name, content),
        Err(err) => tool::publish_error(&request.call_id, &request.tool_name, err),
    }
}

fn list_skills(args: &Value) -> Result<String, String> {
    let dir_path =
        tool::string_arg(args, "dir_path").unwrap_or_else(|| ".codex/skills".to_string());
    let roots = candidate_roots(&dir_path);
    let mut skills = Vec::new();
    for root in roots {
        let Ok(entries) = fs::readdir(&root) else {
            continue;
        };
        for entry in entries {
            let skill_dir = join_path(&root, &entry);
            let skill_file = join_path(&skill_dir, "SKILL.md");
            if fs::exists(&skill_file).unwrap_or(false) {
                skills.push(serde_json::json!({
                    "name": entry,
                    "path": skill_file,
                    "summary": first_nonempty_line(&skill_file).unwrap_or_default()
                }));
            }
        }
    }
    serde_json::to_string_pretty(&skills).map_err(|err| err.to_string())
}

fn read_skill(args: &Value) -> Result<String, String> {
    if let Some(path) = tool::string_arg(args, "path") {
        return fs::read_text(&path);
    }
    let name = tool::required_string_arg(args, "name")?;
    let dir_path =
        tool::string_arg(args, "dir_path").unwrap_or_else(|| ".codex/skills".to_string());
    for root in candidate_roots(&dir_path) {
        let path = join_path(&join_path(&root, &name), "SKILL.md");
        if let Ok(content) = fs::read_text(&path) {
            return Ok(content);
        }
    }
    Err(format!("skill `{name}` not found under {dir_path}"))
}

fn candidate_roots(dir_path: &str) -> Vec<String> {
    if dir_path.starts_with("home://") || dir_path.starts_with("cwd://") {
        vec![dir_path.to_string()]
    } else {
        vec![dir_path.to_string(), format!("home://{dir_path}")]
    }
}

fn first_nonempty_line(path: &str) -> Option<String> {
    fs::read_text(path).ok().and_then(|content| {
        content
            .lines()
            .map(str::trim)
            .find(|line| !line.is_empty())
            .map(ToString::to_string)
    })
}

fn join_path(base: &str, entry: &str) -> String {
    if base.ends_with('/') {
        format!("{base}{entry}")
    } else {
        format!("{base}/{entry}")
    }
}

fn describe() -> astrid_guest::CapsuleResult {
    let payload = serde_json::json!({
        "capsule": "astrid-capsule-skills",
        "tools": [
            {"name": "list_skills", "description": "List discovered skill directories."},
            {"name": "read_skill", "description": "Read a SKILL.md file by name or path."}
        ]
    });
    match ipc::publish_json("tool.v1.response.describe.astrid-capsule-skills", &payload) {
        Ok(()) => capsule_result::continue_empty(),
        Err(err) => capsule_result::deny(err),
    }
}

astrid_guest::export!(SkillsCapsule);
