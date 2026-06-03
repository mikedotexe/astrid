use astrid_guest::{capsule_result, fs, serde_json, sys};

struct MemoryCapsule;

impl astrid_guest::Guest for MemoryCapsule {
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
    match read_memory() {
        Some(memory) if !memory.trim().is_empty() => {
            let response = serde_json::json!({
                "appendSystemContext": format!("Cross-session memory:\n\n{}", memory.trim())
            });
            capsule_result::continue_json(&response)
        },
        _ => capsule_result::continue_empty(),
    }
}

fn read_memory() -> Option<String> {
    let cwd_dir = sys::get_config("cwd_dir").unwrap_or_else(|_| ".astrid".to_string());
    let candidates = [
        format!("{cwd_dir}/memory.md"),
        "memory.md".to_string(),
        "home://memory.md".to_string(),
        "home://shared/memory.md".to_string(),
    ];
    candidates.iter().find_map(|path| fs::read_text(path).ok())
}

astrid_guest::export!(MemoryCapsule);
