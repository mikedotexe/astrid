struct MinimePeerText<'a> {
    provenance: Option<&'a str>,
    body: &'a str,
}

fn minime_peer_text(text: &str) -> MinimePeerText<'_> {
    // Recognize the complete public source-study envelope, not individual
    // header-like lines that may be authored prose. Never rewrite the archive.
    let envelope = text
        .strip_prefix(crate::journal::MINIME_STUDY_ACCOUNT_PREFIX)
        .unwrap_or(text);
    let prefix_len = text.len().saturating_sub(envelope.len());
    let split = ["\n\n", "\r\n\r\n"]
        .into_iter()
        .filter_map(|separator| envelope.split_once(separator))
        .min_by_key(|(header, _)| header.len());
    if let Some((header, body)) = split {
        let mut lines = header.lines();
        let revision = lines.next().unwrap_or_default();
        let evidence = lines.next().unwrap_or_default();
        let account = lines.next().unwrap_or_default();
        let delivery = lines.next().unwrap_or_default();
        if revision
            .strip_prefix("Source revision: ")
            .is_some_and(|value| !value.trim().is_empty())
            && evidence
                .strip_prefix("Input evidence: ")
                .is_some_and(|value| !value.trim().is_empty())
            && account
                == "Account: Minime\u{2019}s response to this input, not independently verified code facts."
            && matches!(
                delivery,
                "Delivery: verified input delivery; response claims and understanding not verified"
                    | "Delivery: unverified; bookmark unchanged"
            )
            && lines.next().is_none()
        {
            return MinimePeerText {
                provenance: Some(&text[..prefix_len.saturating_add(header.len())]),
                body,
            };
        }
    }
    MinimePeerText {
        provenance: None,
        body: text,
    }
}

/// Retain the recognized envelope separately from a character-bounded body.
/// Call before storing history; an already truncated header cannot recover prose.
pub(crate) fn minime_history_context(text: &str, prose_chars: usize) -> String {
    let peer = minime_peer_text(text);
    let body: String = peer.body.chars().take(prose_chars).collect();
    if let Some(provenance) = peer.provenance {
        format!("{provenance}\n\n{body}")
    } else {
        body
    }
}

fn minime_peer_excerpt(text: &str, prose_chars: usize, provenance_chars: usize) -> String {
    if text.is_empty() {
        return "Peer context: no new journal excerpt supplied. Previous entries remain available through explicit reading or MIRROR; no reply to an earlier entry is required.".into();
    }
    let peer = minime_peer_text(text);
    let mut omitted_actions = false;
    let body = peer
        .body
        .split_inclusive('\n')
        .filter(|line| {
            let omit = is_peer_action_directive_line(line);
            omitted_actions |= omit;
            !omit
        })
        .collect::<String>();
    let excerpt: String = body.chars().take(prose_chars).collect();
    let label = if peer.provenance.is_some() {
        "Minime authored excerpt"
    } else {
        "Minime journal excerpt (unclassified source text)"
    };
    let mut rendered = format!("{label}:\n{excerpt}");
    if excerpt.len() < body.len() {
        rendered.push_str("\n[Peer prose excerpt trimmed.]");
    }
    if let Some(provenance) = peer.provenance {
        if provenance_chars == 0 {
            rendered.push_str("\n\nRuntime provenance: recognized source-study envelope; claims unverified; header retained in archive.");
        } else {
            rendered.push_str(
                "\n\nRuntime provenance (recognized envelope; claims not independently verified):\n",
            );
            let excerpt: String = provenance.chars().take(provenance_chars).collect();
            rendered.push_str(&excerpt);
            if excerpt.len() < provenance.len() {
                rendered.push_str(
                    "\n[Runtime provenance excerpt trimmed; archival original unchanged.]",
                );
            }
        }
    }
    if omitted_actions {
        rendered.push_str("\n\n[Minime peer action/status line omitted; choose your own listed Astrid NEXT action.]");
    }
    rendered
}

#[cfg(test)]
#[path = "dialogue_peer_context_tests.rs"]
mod dialogue_peer_context_tests;
