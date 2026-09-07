// Astrid's own triple-reservoir handle, rendered as one line of her dialogue prompt.
//
// Tranche 1 (2026-09-06, Mike & Claude), default ON after a letter to Astrid;
// removable on her request via `ASTRID_OWN_BODY_LINE=off` and a bridge restart.
// Until now every body she was shown was minime's spectrum; her own handle in
// the reservoir service (`read_state` on 7881) was never rendered to her.
// When the service does not answer within the budget the line is absent,
// never zero, so a silent service cannot read as a still body.

const OWN_BODY_LINE_ENV: &str = "ASTRID_OWN_BODY_LINE";
const OWN_BODY_BLOCK_LABEL: &str = "own_body";
const OWN_BODY_BLOCK_PRIORITY: u8 = 2;
const OWN_BODY_BLOCK_CAP: usize = 160;
const OWN_BODY_HANDLE: &str = "astrid";
const OWN_BODY_FETCH_TIMEOUT_MS: u64 = 2_000;
const OWN_BODY_STATUS_PRESENT: &str = "present";
const OWN_BODY_STATUS_DISABLED: &str = "disabled";
const OWN_BODY_STATUS_TEST: &str = "test";
const OWN_BODY_STATUS_UNAVAILABLE: &str = "unavailable";
const OWN_BODY_STATUS_TIMEOUT: &str = "timeout";

#[derive(Debug, Clone)]
struct OwnBodyFetch {
    line: Option<String>,
    status: &'static str,
}

fn own_body_line_enabled() -> bool {
    !matches!(
        std::env::var(OWN_BODY_LINE_ENV)
            .ok()
            .map(|value| value.trim().to_ascii_lowercase())
            .as_deref(),
        Some("off" | "0" | "false" | "no")
    )
}

/// Fetch the line on a blocking thread with a hard budget so a slow reservoir
/// service can never hold a dialogue turn for more than two seconds.
async fn own_body_line_for_dialogue() -> OwnBodyFetch {
    if cfg!(test) {
        return OwnBodyFetch {
            line: None,
            status: OWN_BODY_STATUS_TEST,
        };
    }
    if !own_body_line_enabled() {
        return OwnBodyFetch {
            line: None,
            status: OWN_BODY_STATUS_DISABLED,
        };
    }
    let fetch = tokio::task::spawn_blocking(|| {
        crate::autonomous::reservoir::own_body_line(OWN_BODY_HANDLE)
    });
    match tokio::time::timeout(
        std::time::Duration::from_millis(OWN_BODY_FETCH_TIMEOUT_MS),
        fetch,
    )
    .await
    {
        Ok(Ok(Some(line))) => OwnBodyFetch {
            line: Some(line),
            status: OWN_BODY_STATUS_PRESENT,
        },
        Ok(Ok(None)) | Ok(Err(_)) => OwnBodyFetch {
            line: None,
            status: OWN_BODY_STATUS_UNAVAILABLE,
        },
        Err(_) => OwnBodyFetch {
            line: None,
            status: OWN_BODY_STATUS_TIMEOUT,
        },
    }
}

/// Insert the own-body block directly after `spectral` (or first, when there
/// is no spectral block). A missing or blank line leaves the blocks untouched.
fn append_own_body_block(
    mut blocks: Vec<crate::prompt_budget::PromptBlock>,
    mut sources: DialogueBlockSources,
    line: Option<&str>,
) -> (Vec<crate::prompt_budget::PromptBlock>, DialogueBlockSources) {
    let Some(line) = line.map(str::trim).filter(|line| !line.is_empty()) else {
        return (blocks, sources);
    };
    let content = sources.cap(OWN_BODY_BLOCK_LABEL, line, OWN_BODY_BLOCK_CAP);
    let position = blocks
        .iter()
        .position(|block| block.label == "spectral")
        .map_or(0, |index| index.saturating_add(1));
    blocks.insert(
        position,
        crate::prompt_budget::PromptBlock {
            label: OWN_BODY_BLOCK_LABEL,
            content,
            priority: OWN_BODY_BLOCK_PRIORITY,
            min_chars: 0,
        },
    );
    (blocks, sources)
}
