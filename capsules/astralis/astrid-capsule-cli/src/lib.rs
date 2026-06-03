use astrid_guest::{capsule_result, ipc, sys, uplink};

struct CliCapsule;

impl astrid_guest::Guest for CliCapsule {
    fn astrid_hook_trigger(_action: String, _payload: Vec<u8>) -> astrid_guest::CapsuleResult {
        capsule_result::continue_empty()
    }

    fn run() {
        let _ = uplink::register("cli-compat", "cli", "bridge");
        sys::log_info(
            "astrid-capsule-cli is running as an optional compatibility uplink; native socket daemon management is canonical",
        );
        let handle = ipc::subscribe("astrid.v1.capsules_loaded").ok();
        sys::signal_ready();
        loop {
            if let Some(handle_id) = handle {
                let _ = ipc::recv(handle_id, 60_000);
            }
        }
    }

    fn astrid_install() {}

    fn astrid_upgrade() {}
}

astrid_guest::export!(CliCapsule);
