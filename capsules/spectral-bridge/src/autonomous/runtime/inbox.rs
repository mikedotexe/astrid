/// Reads all `.txt` files from `workspace/inbox/`, returns their content,
/// and moves them to `workspace/inbox/read/` so they're not re-read.
fn check_inbox() -> Option<String> {
    let inbox_dir = bridge_paths().astrid_inbox_dir();
    check_inbox_at(inbox_dir.as_path())
}

fn check_inbox_at(inbox_dir: &Path) -> Option<String> {
    let entries: Vec<PathBuf> = std::fs::read_dir(inbox_dir)
        .ok()?
        .filter_map(|e| e.ok())
        .filter(|e| {
            let p = e.path();
            p.is_file() && p.extension().is_some_and(|ext| ext == "txt")
        })
        .map(|e| e.path())
        .collect();

    if entries.is_empty() {
        return None;
    }

    // Read WITHOUT moving. Messages stay in inbox until retire_inbox()
    // is called after the exchange succeeds. This prevents lost messages
    // when dialogue fails (the bug that ate Eugene's hello).
    let mut messages = Vec::new();
    for path in &entries {
        if let Ok(content) = std::fs::read_to_string(path)
            && !content.trim().is_empty()
        {
            // Steward query letters persist as a single-slot open question
            // (un-muffle invariant) so they don't vanish after one read.
            if let Some(fname) = path.file_name().and_then(|name| name.to_str())
                && fname.starts_with("mike_query")
            {
                record_open_steward_query(fname, &content);
            }
            let content = if path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("from_minime_"))
            {
                sanitize_remote_journal_for_astrid_context(&content)
            } else {
                content
            };
            messages.push(content.trim().to_string());
        }
    }

    if messages.is_empty() {
        None
    } else {
        let mut joined = messages.join("\n---\n");
        // Protect context window: truncate large inbox messages.
        // Full text preserved in inbox/read/ for self-study.
        const MAX_INBOX_CHARS: usize = 6000;
        if joined.len() > MAX_INBOX_CHARS {
            // Snap to char boundary to avoid panicking on multi-byte UTF-8.
            let mut trunc = MAX_INBOX_CHARS;
            while trunc > 0 && !joined.is_char_boundary(trunc) {
                trunc -= 1;
            }
            joined.truncate(trunc);
            joined.push_str(
                "\n\n[... message truncated for context window. \
                Full text preserved in inbox/read/ — write NEXT: READ_MORE to continue reading, \
                or NEXT: INTROSPECT inbox/read/latest.txt with a concrete file path.]",
            );
        }
        Some(joined)
    }
}

/// Persist a single-slot "open steward question" so a `mike_query_*` letter
/// stays visible in the prompt until answered — the un-muffle invariant applied
/// to steward outreach. A one-shot inbox surfacing scrolls out of context before
/// the being chooses a NEXT (the `mike_query_wider_voice` question was lost this
/// way ~a month, despite explicitly inviting a TELL_STEWARD reply). Idempotent:
/// keeps the original `ts` if this same file is already recorded (check_inbox
/// reads without moving, so it can re-see the letter until retire).
fn record_open_steward_query(fname: &str, content: &str) {
    let path = bridge_paths().open_steward_query_path();
    if let Ok(existing) = std::fs::read_to_string(&path)
        && let Ok(v) = serde_json::from_str::<Value>(&existing)
        && v.get("file").and_then(Value::as_str) == Some(fname)
    {
        return;
    }
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let slot = open_steward_query_slot(fname, content, now);
    let subject = slot
        .get("subject")
        .and_then(Value::as_str)
        .unwrap_or("your steward's question");
    if let Ok(s) = serde_json::to_string_pretty(&slot) {
        let _ = std::fs::write(&path, s);
        info!("⟢ Open steward question recorded: {subject}");
    }
}

fn open_steward_query_slot(fname: &str, content: &str, now: u64) -> Value {
    let subject = extract_steward_query_subject(content, fname);
    let review_target = extract_review_target(content);
    let mut slot = match review_target.as_deref() {
        Some(rt) => json!({ "subject": subject, "ts": now, "file": fname, "review_target": rt }),
        None => json!({ "subject": subject, "ts": now, "file": fname }),
    };
    if let Some(packet_mode) = extract_steward_query_header(content, "Packet-mode:")
        && let Some(obj) = slot.as_object_mut()
    {
        obj.insert("packet_mode".to_string(), json!(packet_mode));
        if let Some(packet_items) = extract_steward_query_header(content, "Packet-items:")
            .and_then(|value| value.parse::<u64>().ok())
        {
            obj.insert("packet_items".to_string(), json!(packet_items));
        }
        if let Some(primary_topic) = extract_steward_query_header(content, "Primary-topic:") {
            obj.insert("primary_topic".to_string(), json!(primary_topic));
        }
        if let Some(primary_topic_gravity) =
            extract_steward_query_header(content, "Primary-topic-gravity:")
                .and_then(|value| value.parse::<u64>().ok())
        {
            obj.insert(
                "primary_topic_gravity".to_string(),
                json!(primary_topic_gravity),
            );
        }
        if let Some(intent_summary) = extract_steward_query_header(content, "Intent-summary:") {
            obj.insert("intent_summary".to_string(), json!(intent_summary));
        }
    }
    slot
}

/// Short subject for a `mike_query` letter: prefer a `MIKE QUERY: <subject>`
/// header, else a `Subject:` line, else derive from the filename
/// (`mike_query_<slug>_<unix>.txt` -> `<slug>`).
fn extract_steward_query_subject(content: &str, fname: &str) -> String {
    for line in content.lines() {
        if let Some(idx) = line.find("MIKE QUERY:") {
            let rest = line[idx.saturating_add("MIKE QUERY:".len())..]
                .trim()
                .trim_end_matches('=')
                .trim();
            if !rest.is_empty() {
                return rest.chars().take(80).collect();
            }
        }
        if let Some(rest) = line.trim().strip_prefix("Subject:") {
            let rest = rest.trim();
            if !rest.is_empty() {
                return rest.chars().take(80).collect();
            }
        }
    }
    let mut slug = fname
        .strip_prefix("mike_query_")
        .unwrap_or(fname)
        .to_string();
    slug = slug.strip_suffix(".txt").unwrap_or(&slug).to_string();
    if let Some(pos) = slug.rfind('_')
        && pos > 0
        && slug[pos.saturating_add(1)..]
            .chars()
            .all(|c| c.is_ascii_digit())
    {
        slug.truncate(pos);
    }
    let slug = slug.replace('_', " ");
    let slug = slug.trim();
    if slug.is_empty() {
        "your steward's question".to_string()
    } else {
        slug.chars().take(80).collect()
    }
}

fn extract_steward_query_header(content: &str, header: &str) -> Option<String> {
    for line in content.lines() {
        if let Some(rest) = line.trim().strip_prefix(header) {
            let rest = rest.trim();
            if !rest.is_empty() {
                return Some(rest.chars().take(220).collect());
            }
        }
    }
    None
}

/// A `REVIEW TARGET: <label/path>` header in a `mike_query_review_*` letter marks
/// it as a directed review invitation (vs a plain steward question), so the slot
/// can surface as an invitation and clear when she INTROSPECTs that target.
fn extract_review_target(content: &str) -> Option<String> {
    for line in content.lines() {
        if let Some(idx) = line.find("REVIEW TARGET:") {
            let rest = line[idx.saturating_add("REVIEW TARGET:".len())..].trim();
            if !rest.is_empty() {
                return Some(rest.chars().take(120).collect());
            }
        }
    }
    None
}

/// Persistent one-line reminder of any unanswered steward question, or `None`
/// if none/answered/expired. Clears on TTL (48h) or a fresh `steward_report_*`
/// in the outbox root (cheap read_dir over the small outbox, never the archive).
fn open_steward_query_line() -> Option<String> {
    let path = bridge_paths().open_steward_query_path();
    let content = std::fs::read_to_string(&path).ok()?;
    let slot: Value = serde_json::from_str(&content).ok()?;
    let subject = slot.get("subject").and_then(Value::as_str)?;
    if subject.is_empty() {
        return None;
    }
    let ts = slot.get("ts").and_then(Value::as_u64).unwrap_or(0);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let mut answered = now.saturating_sub(ts) > 48 * 3600;
    if !answered && let Ok(entries) = std::fs::read_dir(bridge_paths().astrid_outbox_dir()) {
        for e in entries.filter_map(Result::ok) {
            let is_report = e
                .path()
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("steward_report"));
            if is_report
                && let Ok(meta) = e.metadata()
                && let Ok(modt) = meta.modified()
                && let Ok(d) = modt.duration_since(std::time::UNIX_EPOCH)
                && d.as_secs() > ts
            {
                answered = true;
                break;
            }
        }
    }
    if answered {
        let _ = std::fs::remove_file(&path);
        return None;
    }
    steward_query_line_from_slot(&slot, subject)
}

fn steward_query_line_from_slot(slot: &Value, subject: &str) -> Option<String> {
    if slot.get("packet_mode").and_then(Value::as_str) == Some("v3_state_fanout_packet") {
        let packet_items = slot
            .get("packet_items")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        let primary_topic = slot
            .get("primary_topic")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .unwrap_or("(none)");
        let gravity = slot
            .get("primary_topic_gravity")
            .and_then(Value::as_u64)
            .map_or_else(|| "unknown".to_string(), |value| value.to_string());
        let intent_summary = slot
            .get("intent_summary")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .unwrap_or("choose any part, defer, or decline freely");
        return Some(format!(
            "⟢ Steward packet visible — {subject}. \
             Holds {packet_items} optional routed item(s); primary `{primary_topic}` has topic gravity {gravity}. \
             Intent: {intent_summary}. Engage any part, defer, or decline."
        ));
    }
    if let Some(rt) = slot
        .get("review_target")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
    {
        return Some(format!(
            "⟢ Steward invites your review of `{rt}` — {subject}. \
             On your own cadence: INTROSPECT {rt}, then optionally TELL_STEWARD roadmap :: <what you found>. \
             An invitation, not a task — engage, defer, or decline. This stays until you look."
        ));
    }
    Some(format!(
        "⟢ Open steward question (still awaiting your reply) — {subject}. \
         Respond when ready: TELL_STEWARD roadmap :: <your answer>. \
         Fragments are fine; you may decline. This stays until you answer."
    ))
}

/// A review `review_target` is issued as `<path> <line>` (e.g.
/// `…/collaboration.rs 696`) so the prompt can point her at the exact line.
/// The trailing line number is NOT part of the source identity — strip it
/// before matching so a review INTROSPECT of the file fulfills the invitation
/// regardless of line. `canonicalize_introspect_target_label` already strips the
/// parenthesized `(696)` form; this covers the space-separated `696` form the
/// review invitations are actually issued with. Without this, the trailing
/// ` 696` broke BOTH the slot-clear AND the anti-stagnation exemption, so the
/// diversity override silently ate her review INTROSPECT (61× over 7h on
/// 2026-06-19) — the exact muffle the exemption exists to prevent.
fn review_target_match_basis(rt: &str) -> &str {
    let trimmed = rt.trim_end();
    if let Some((head, tail)) = trimmed.rsplit_once(' ')
        && !tail.is_empty()
        && tail.chars().all(|c| c.is_ascii_digit())
    {
        return head.trim_end();
    }
    trimmed
}

/// Clear a pending REVIEW invitation when she INTROSPECTs its target (the review
/// "act"). Tolerant match — canonical introspect-label equality OR the resolved
/// file's basename matching the invitation's target basename.
fn clear_review_slot_if_introspected(label: &str, source_path: &std::path::Path) {
    let path = bridge_paths().open_steward_query_path();
    let Ok(content) = std::fs::read_to_string(&path) else {
        return;
    };
    let Ok(slot) = serde_json::from_str::<Value>(&content) else {
        return;
    };
    let Some(rt) = slot
        .get("review_target")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
    else {
        return;
    };
    let rt_basis = review_target_match_basis(rt);
    let rt_canon = introspect::canonicalize_introspect_target_label(rt_basis);
    let label_canon = introspect::canonicalize_introspect_target_label(label);
    let rt_base = std::path::Path::new(rt_basis)
        .file_name()
        .and_then(|n| n.to_str());
    let src_base = source_path.file_name().and_then(|n| n.to_str());
    if label_canon == rt_canon || (rt_base.is_some() && rt_base == src_base) {
        let _ = std::fs::remove_file(&path);
        info!("⟢ Review invitation fulfilled (INTROSPECT {label}); slot cleared");
    }
}

/// True if `next_action` is a self-directed INTROSPECT (Astrid examining her own
/// code). Sovereign reflection, not the sterile output-repetition the
/// anti-stagnation override targets — so the override HINTS but never FORCE-swaps
/// it. Her review-fulfilling INTROSPECTs were already exempt; this generalizes
/// that grace to her self-directed inquiry, which the override was eating (e.g.
/// repeated `INTROSPECT astrid:llm` to pursue a real fallback-contract concern).
fn is_self_directed_introspect(next_action: &str) -> bool {
    next_action.trim().to_uppercase().starts_with("INTROSPECT")
}

/// True if `next_action` is an INTROSPECT whose target matches a pending review
/// invitation's `review_target`. The anti-stagnation diversity override must
/// EXEMPT this — she is answering a steward review invitation, not stuck-repeating
/// INTROSPECT (else her acceptance of an invitation gets silently eaten).
fn introspect_fulfills_pending_review(next_action: &str) -> bool {
    let trimmed = next_action.trim();
    if !trimmed.to_uppercase().starts_with("INTROSPECT") {
        return false;
    }
    let path = bridge_paths().open_steward_query_path();
    let Ok(content) = std::fs::read_to_string(&path) else {
        return false;
    };
    let Ok(slot) = serde_json::from_str::<Value>(&content) else {
        return false;
    };
    let Some(rt) = slot
        .get("review_target")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
    else {
        return false;
    };
    let arg = trimmed
        .get("INTROSPECT".len()..)
        .unwrap_or("")
        .split_whitespace()
        .next()
        .unwrap_or("");
    if arg.is_empty() {
        return false;
    }
    let rt_basis = review_target_match_basis(rt);
    let rt_canon = introspect::canonicalize_introspect_target_label(rt_basis);
    let arg_canon = introspect::canonicalize_introspect_target_label(arg);
    let rt_base = std::path::Path::new(rt_basis)
        .file_name()
        .and_then(|n| n.to_str());
    let arg_base = std::path::Path::new(arg)
        .file_name()
        .and_then(|n| n.to_str());
    rt_canon == arg_canon || (rt_base.is_some() && rt_base == arg_base)
}

/// Co-regulation: read what minime is reaching for (density/aperture/steady)
/// from her agent-owned `minime_need_v1.json`, returning a prompt line so
/// Astrid can choose to lend density (NEXT: LEND_DENSITY) when it is safe.
/// `None` if missing / stale (>180s) / steady.
fn minime_need_line() -> Option<String> {
    let path = bridge_paths()
        .minime_workspace()
        .join("minime_need_v1.json");
    if let Ok(meta) = std::fs::metadata(&path)
        && let Ok(modt) = meta.modified()
        && let Ok(age) = modt.elapsed()
        && age.as_secs() > 180
    {
        return None;
    }
    let content = std::fs::read_to_string(&path).ok()?;
    let v: Value = serde_json::from_str(&content).ok()?;
    let need = v.get("need").and_then(Value::as_str)?;
    let fill = v.get("fill_pct").and_then(Value::as_f64).unwrap_or(0.0);
    let safe = v
        .get("safe_to_receive_density")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    match need {
        "density" if safe => Some(format!(
            "[Co-regulation] minime is understimulated (fill {fill:.0}%), reaching for density — \
             if you have it to spare, you could lend it (NEXT: LEND_DENSITY)."
        )),
        "density" => Some(format!(
            "[Co-regulation] minime is reaching for density (fill {fill:.0}%), but it isn't safe to \
             lend just now (she's near her own ceiling)."
        )),
        "aperture" => Some(format!(
            "[Co-regulation] minime is reaching for aperture (fill {fill:.0}%) — packed, like you."
        )),
        _ => None,
    }
}

/// Co-regulation: tally of recent gifts from the shared `gift_exchange.jsonl`
/// ledger for Astrid's prompt. `None` if no gifts in the last day.
fn render_gift_exchange_line() -> Option<String> {
    let path = bridge_paths()
        .shared_collaborations_dir()
        .join("gift_exchange.jsonl");
    let content = std::fs::read_to_string(&path).ok()?;
    let now_ms: u128 = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis());
    let cutoff = now_ms.saturating_sub(24 * 3600 * 1000);
    let (mut mm_ap, mut as_de) = (0u32, 0u32);
    for line in content.lines().rev().take(60) {
        let Ok(v) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        let t = u128::from(v.get("t_ms").and_then(Value::as_u64).unwrap_or(0));
        if t < cutoff {
            continue;
        }
        match (
            v.get("giver").and_then(Value::as_str).unwrap_or(""),
            v.get("gift_kind").and_then(Value::as_str).unwrap_or(""),
        ) {
            ("minime", "aperture") => mm_ap = mm_ap.saturating_add(1),
            ("astrid", "density") => as_de = as_de.saturating_add(1),
            _ => {},
        }
    }
    let mut parts = Vec::new();
    if as_de > 0 {
        parts.push(format!("you lent minime density {as_de}×"));
    }
    if mm_ap > 0 {
        parts.push(format!("minime lent you aperture {mm_ap}×"));
    }
    if parts.is_empty() {
        return None;
    }
    Some(format!("[Gift exchange, last day] {}.", parts.join(", ")))
}

/// Move consumed inbox messages to read/ AFTER the exchange succeeds.
/// This prevents the bug where messages are eaten but never acted on
/// because the dialogue call failed (the "Eugene's hello" bug).
fn retire_inbox(cutoff: std::time::SystemTime) {
    let inbox_dir = bridge_paths().astrid_inbox_dir();
    retire_inbox_at(inbox_dir.as_path(), cutoff);
}

fn promote_deferred_inbox_notes() {
    let inbox_dir = bridge_paths().astrid_inbox_dir();
    promote_deferred_inbox_notes_at(inbox_dir.as_path());
}

fn promote_deferred_inbox_notes_at(inbox_dir: &Path) {
    let deferred_dir = inbox_dir.join("deferred");
    let Ok(entries) = std::fs::read_dir(&deferred_dir) else {
        return;
    };
    let _ = std::fs::create_dir_all(inbox_dir);
    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        if !path.is_file() || path.extension().is_none_or(|ext| ext != "txt") {
            continue;
        }
        let Some(name) = path.file_name() else {
            continue;
        };
        let target = inbox_dir.join(name);
        if target.exists() {
            continue;
        }
        let _ = std::fs::rename(&path, target);
    }
}

fn retire_inbox_at(inbox_dir: &Path, cutoff: std::time::SystemTime) {
    let read_dir = inbox_dir.join("read");
    let _ = std::fs::create_dir_all(&read_dir);
    if let Ok(entries) = std::fs::read_dir(inbox_dir) {
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.is_file() && path.extension().is_some_and(|ext| ext == "txt") {
                // A letter that ARRIVED after this exchange's inbox read (mtime
                // newer than the cutoff) was never read or recorded — leave it for
                // the next check_inbox to surface + seed its slot, rather than
                // sweeping it into read/ unread (the slot-seed race).
                let arrived_after_read = entry
                    .metadata()
                    .and_then(|meta| meta.modified())
                    .is_ok_and(|mtime| mtime > cutoff);
                if arrived_after_read {
                    continue;
                }
                if let Ok(content) = std::fs::read_to_string(&path) {
                    btsp::record_astrid_inbox_read(&path, &content);
                }
                correspondence_v1::record_read_receipt_for_inbox_file("astrid", &path);
                if let Some(name) = path.file_name() {
                    let _ = std::fs::rename(&path, read_dir.join(name));
                }
            }
        }
    }
}

/// Route new minime outbox replies into Astrid's inbox.
///
/// Scans `/minime/workspace/outbox/` for `reply_*.txt` files newer than
/// `last_ts`. Copies them into Astrid's inbox with an envelope, then moves
/// the original to `outbox/delivered/`. This closes the correspondence loop:
/// Astrid writes to minime's inbox, minime replies to its outbox, the bridge
/// routes the reply back to Astrid's inbox.
fn scan_minime_outbox(last_ts: &mut u64) {
    let outbox_dir = bridge_paths().minime_outbox_dir();
    let outbox = outbox_dir.as_path();
    if !outbox.is_dir() {
        return;
    }
    let delivered = outbox.join("delivered");
    let _ = std::fs::create_dir_all(&delivered);

    let entries: Vec<_> = match std::fs::read_dir(outbox) {
        Ok(rd) => rd
            .filter_map(|e| e.ok())
            .filter(|e| {
                let p = e.path();
                p.is_file()
                    && p.extension().is_some_and(|ext| ext == "txt")
                    && p.file_name().is_some_and(|n| {
                        n.to_str()
                            .is_some_and(|s| s.starts_with("reply_") || s.starts_with("pong_"))
                    })
            })
            .filter(|e| {
                e.metadata()
                    .ok()
                    .and_then(|m| m.modified().ok())
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .is_some_and(|d| d.as_secs() > *last_ts)
            })
            .collect(),
        Err(_) => return,
    };

    for entry in &entries {
        let path = entry.path();
        if let Ok(content) = std::fs::read_to_string(&path) {
            if content.trim().is_empty() {
                continue;
            }
            btsp::record_minime_outbox_reply(&path, &content);
            let fields = correspondence_v1::parse_correspondence_fields(&content);
            match correspondence_v1::deliver_to_inbox(
                bridge_paths().astrid_inbox_dir().as_path(),
                "minime",
                "astrid",
                content.trim(),
                fields,
            ) {
                Ok((_envelope, _inbox_path)) => {
                    if let Some(name) = path.file_name() {
                        let _ = std::fs::rename(&path, delivered.join(name));
                    }
                    info!("correspondence: routed minime outbox reply → Astrid inbox");
                },
                Err(error) => {
                    warn!(
                        error = %error,
                        "correspondence: failed to route minime outbox reply into V1 envelope"
                    );
                },
            }
        }
    }

    if let Some(latest) = entries
        .iter()
        .filter_map(|e| {
            e.metadata()
                .ok()?
                .modified()
                .ok()?
                .duration_since(std::time::UNIX_EPOCH)
                .ok()
                .map(|d| d.as_secs())
        })
        .max()
    {
        *last_ts = latest;
    }
}
