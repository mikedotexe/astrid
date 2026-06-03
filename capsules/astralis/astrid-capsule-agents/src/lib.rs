use astrid_guest::{capsule_result, fs, serde_json, sys};

struct AgentsCapsule;

impl astrid_guest::Guest for AgentsCapsule {
    fn astrid_hook_trigger(action: String, _payload: Vec<u8>) -> astrid_guest::CapsuleResult {
        match action.as_str() {
            "on_before_prompt_build" => before_prompt_build(),
            _ => capsule_result::continue_empty(),
        }
    }

    fn run() {}

    fn astrid_install() {}

    fn astrid_upgrade() {}
}

fn before_prompt_build() -> astrid_guest::CapsuleResult {
    match read_agents_file() {
        Some(content) if !content.trim().is_empty() => {
            let response = serde_json::json!({
                "appendSystemContext": format!(
                    "Project instructions from AGENTS.md:\n\n{}",
                    content.trim()
                )
            });
            capsule_result::continue_json(&response)
        },
        _ => capsule_result::continue_empty(),
    }
}

fn read_agents_file() -> Option<String> {
    if let Ok(content) = fs::read_text("AGENTS.md") {
        return Some(content);
    }

    let cwd_dir = sys::get_config("cwd_dir").unwrap_or_else(|_| ".astrid".to_string());
    let path = format!("{cwd_dir}/AGENTS.md");
    fs::read_text(&path).ok()
}

astrid_guest::export!(AgentsCapsule);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ignores_unknown_actions() {
        let result = AgentsCapsule::astrid_hook_trigger("other".to_string(), Vec::new());
        assert_eq!(result.action, "continue");
        assert!(result.data.is_none());
    }
}
