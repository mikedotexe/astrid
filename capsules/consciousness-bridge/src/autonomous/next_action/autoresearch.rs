use tracing::{info, warn};

use super::{ConversationState, NextActionContext};
use crate::autoresearch as bridge_autoresearch;
use crate::paths::bridge_paths;

pub(super) fn handle_action(
    conv: &mut ConversationState,
    base_action: &str,
    original: &str,
    _ctx: &mut NextActionContext<'_>,
) -> bool {
    if !bridge_autoresearch::is_autoresearch_action(base_action) {
        return false;
    }

    match bridge_autoresearch::run_action(
        original,
        bridge_paths().autoresearch_root(),
        &bridge_paths().research_dir(),
        true,
    ) {
        Ok(result) => {
            conv.pending_file_listing = Some(result.display_text);
            if let Some(offset) = result.next_offset {
                conv.last_read_path = Some(result.saved_path.to_string_lossy().into_owned());
                conv.last_read_offset = offset;
            } else {
                conv.last_read_path = None;
                conv.last_read_offset = 0;
            }
            conv.last_read_meaning_summary = None;
            info!("Astrid ran autoresearch action: {base_action}");
        },
        Err(error) => {
            conv.pending_file_listing = Some(format!("[Autoresearch error] {error}"));
            conv.last_read_path = None;
            conv.last_read_offset = 0;
            conv.last_read_meaning_summary = None;
            warn!("Autoresearch action failed: {base_action}: {error}");
        },
    }

    true
}
