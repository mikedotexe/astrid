use astrid_guest::{capsule_result, ipc, process, serde_json, tool};
use serde_json::Value;

struct ShellCapsule;

impl astrid_guest::Guest for ShellCapsule {
    fn astrid_hook_trigger(action: String, payload: Vec<u8>) -> astrid_guest::CapsuleResult {
        match action.as_str() {
            "tool_execute_run_shell_command" => handle_tool(&payload, run_shell_command),
            "tool_execute_spawn_background_process" => {
                handle_tool(&payload, spawn_background_process)
            },
            "tool_execute_read_process_logs" => handle_tool(&payload, read_process_logs),
            "tool_execute_kill_process" => handle_tool(&payload, kill_process),
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

fn run_shell_command(args: &Value) -> Result<String, String> {
    let command = tool::string_arg(args, "command")
        .or_else(|| tool::string_arg(args, "cmd"))
        .ok_or_else(|| "missing string argument `command`".to_string())?;
    let proc_args = vec!["-lc".to_string(), command];
    let output = process::spawn("bash", &proc_args)?;
    let payload = serde_json::json!({
        "stdout": output.stdout,
        "stderr": output.stderr,
        "exit_code": output.exit_code,
    });
    serde_json::to_string_pretty(&payload).map_err(|err| err.to_string())
}

fn spawn_background_process(args: &Value) -> Result<String, String> {
    let command = tool::string_arg(args, "command")
        .or_else(|| tool::string_arg(args, "cmd"))
        .ok_or_else(|| "missing string argument `command`".to_string())?;
    let proc_args = vec!["-lc".to_string(), command];
    let output = process::spawn_background("bash", &proc_args)?;
    Ok(output.id.to_string())
}

fn read_process_logs(args: &Value) -> Result<String, String> {
    let id = process_id(args)?;
    let logs = process::read_logs(id)?;
    let payload = serde_json::json!({
        "stdout": logs.stdout,
        "stderr": logs.stderr,
        "running": logs.running,
        "exit_code": logs.exit_code,
    });
    serde_json::to_string_pretty(&payload).map_err(|err| err.to_string())
}

fn kill_process(args: &Value) -> Result<String, String> {
    let id = process_id(args)?;
    let result = process::kill(id)?;
    let payload = serde_json::json!({
        "killed": result.killed,
        "exit_code": result.exit_code,
        "stdout": result.stdout,
        "stderr": result.stderr,
    });
    serde_json::to_string_pretty(&payload).map_err(|err| err.to_string())
}

fn process_id(args: &Value) -> Result<u64, String> {
    tool::u64_arg(args, "process_id")
        .or_else(|| tool::u64_arg(args, "id"))
        .ok_or_else(|| "missing numeric argument `process_id`".to_string())
}

fn describe() -> astrid_guest::CapsuleResult {
    let payload = serde_json::json!({
        "capsule": "astrid-capsule-shell",
        "tools": [
            {"name": "run_shell_command", "description": "Run a shell command in the workspace sandbox."},
            {"name": "spawn_background_process", "description": "Start a background shell command."},
            {"name": "read_process_logs", "description": "Read logs for a background process."},
            {"name": "kill_process", "description": "Kill a background process."}
        ]
    });
    match ipc::publish_json("tool.v1.response.describe.astrid-capsule-shell", &payload) {
        Ok(()) => capsule_result::continue_empty(),
        Err(err) => capsule_result::deny(err),
    }
}

astrid_guest::export!(ShellCapsule);
