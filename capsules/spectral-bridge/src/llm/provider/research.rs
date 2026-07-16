/// Search the web via DuckDuckGo HTML and return structured result snippets.
///
/// Used to supplement introspection with external knowledge — if Astrid
/// reads ESN code, it can also read about ESN theory from the web.
pub(crate) async fn web_search(query: &str, anchor: &str) -> Option<WebSearchResult> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .ok()?;

    let url = format!("https://html.duckduckgo.com/html/?q={}", urlencoded(query));

    let response = client
        .get(&url)
        .header("User-Agent", "Mozilla/5.0")
        .send()
        .await
        .ok()?;

    let html = response.text().await.ok()?;

    let hits = extract_duckduckgo_hits(&html);
    if hits.is_empty() {
        return None;
    }

    let raw_text = render_hits_plain(&hits);
    let excerpt = trim_chars(&raw_text, 1800);
    let meaning_summary =
        summarize_research_meaning(ResearchSourceKind::Search, anchor, query, &excerpt)
            .await
            .unwrap_or_else(|| {
                fallback_meaning_summary(ResearchSourceKind::Search, anchor, query, &excerpt)
            });

    Some(WebSearchResult {
        source_kind: ResearchSourceKind::Search,
        raw_text,
        hits,
        anchor: anchor.to_string(),
        meaning_summary,
    })
}

pub(crate) fn derive_browse_anchor(
    preferred: Option<&str>,
    context: Option<&str>,
    url: &str,
) -> String {
    let preferred = preferred.map(str::trim).filter(|value| !value.is_empty());
    if let Some(anchor) = preferred {
        return trim_chars(anchor, 160);
    }

    let context = context
        .map(|value| value.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|value| !value.is_empty());
    if let Some(anchor) = context {
        return trim_chars(&anchor, 160);
    }

    slug_anchor_from_url(url)
}

pub(crate) fn format_browse_failure_context(url: &str, reason: &str) -> String {
    format!(
        "[Web access status: the page at {url} could not be meaningfully read: {reason}]\n\
         [This is ordinary source/site availability, not evidence of a perceptual gate, \
         internal topology boundary, or spectral event.]\n\
         [Keep the concrete topic from the URL if useful, but do not build an experience \
         around the access failure.]\n\n\
         [Try NEXT: SEARCH with a narrower question, NEXT: BROWSE a different reliable source, \
         or NEXT: REST.]"
    )
}

pub(crate) fn format_browse_read_context(
    page: &FetchedPage,
    chunk: &str,
    remaining: Option<usize>,
) -> String {
    let header = if remaining.is_some() {
        format!("[You read the page at {}]", page.url)
    } else {
        format!("[You read the full page at {}]", page.url)
    };
    let continuation = remaining
        .map(|chars| {
            format!(
                "\n\n[Page continues — {chars} more chars. Write NEXT: READ_MORE to continue reading.]"
            )
        })
        .unwrap_or_default();

    format!(
        "{header}\n\n{}\n\n{chunk}{continuation}",
        page.meaning_summary
    )
}

pub(crate) fn format_read_more_context(
    offset: usize,
    chunk: &str,
    remaining: usize,
    meaning_summary: Option<&str>,
) -> String {
    let summary_block = meaning_summary
        .filter(|value| !value.trim().is_empty())
        .map(|value| format!("[Meaning summary from this document:]\n{value}\n\n"))
        .unwrap_or_default();
    let continuation = if remaining > 0 {
        format!("\n\n[{remaining} more chars remain. Write NEXT: READ_MORE to continue.]")
    } else {
        "\n\n[End of document.]".to_string()
    };

    format!("{summary_block}[Continuing reading from offset {offset}...]\n\n{chunk}{continuation}")
}

fn format_research_hits(hits: &[ResearchHit]) -> String {
    hits.iter()
        .enumerate()
        .map(|(index, hit)| {
            format!(
                "{}. {}\n   {}\n   URL: {}",
                index.saturating_add(1),
                hit.title,
                hit.snippet,
                hit.url
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn render_hits_plain(hits: &[ResearchHit]) -> String {
    hits.iter()
        .map(|hit| format!("{} — {} [{}]", hit.title, hit.snippet, hit.url))
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn extract_duckduckgo_hits(html: &str) -> Vec<ResearchHit> {
    let anchors = extract_duckduckgo_anchors(html);
    let snippets = extract_duckduckgo_snippets(html);

    anchors
        .into_iter()
        .enumerate()
        .filter_map(|(index, (url, title))| {
            let snippet = snippets.get(index).cloned().unwrap_or_default();
            if title.is_empty() && snippet.is_empty() {
                None
            } else {
                Some(ResearchHit {
                    title: if title.is_empty() {
                        trim_chars(&snippet, 80)
                    } else {
                        title
                    },
                    snippet,
                    url,
                })
            }
        })
        .take(5)
        .collect()
}

fn extract_duckduckgo_anchors(html: &str) -> Vec<(String, String)> {
    let mut anchors = Vec::new();
    let mut pos = 0;
    #[expect(
        clippy::arithmetic_side_effects,
        reason = "string index offsets within bounds guaranteed by find()"
    )]
    while let Some(start) = html[pos..].find("result__a") {
        let abs_start = pos + start;
        let Some(href_start_rel) = html[abs_start..].find("href=\"") else {
            pos = abs_start + 8;
            continue;
        };
        let href_start = abs_start + href_start_rel + 6;
        let Some(href_end_rel) = html[href_start..].find('"') else {
            pos = href_start;
            continue;
        };
        let href_end = href_start + href_end_rel;
        let raw_url = html_unescape(html[href_start..href_end].trim());
        let url = decode_ddg_result_url(&raw_url);

        let Some(gt_rel) = html[abs_start..].find('>') else {
            pos = href_end;
            continue;
        };
        let title_start = abs_start + gt_rel + 1;
        let Some(title_end_rel) = html[title_start..].find("</a>") else {
            pos = title_start;
            continue;
        };
        let title = strip_html_tags(&html[title_start..title_start + title_end_rel]);

        if let Some(url) = url.filter(|value| value.starts_with("http")) {
            anchors.push((url, trim_chars(&title, 200)));
        }
        pos = title_start + title_end_rel + 4;
        if anchors.len() >= 5 {
            break;
        }
    }
    anchors
}

fn extract_duckduckgo_snippets(html: &str) -> Vec<String> {
    regex_find_all(html, r"result__snippet[^>]*>(.*?)</(?:a|span|td)")
        .into_iter()
        .map(|snippet| strip_html_tags(&snippet))
        .filter(|snippet| snippet.len() > 20)
        .map(|snippet| trim_chars(&snippet, 600))
        .take(5)
        .collect()
}

fn decode_ddg_result_url(raw_url: &str) -> Option<String> {
    if let Some(uddg_pos) = raw_url.find("uddg=") {
        let encoded = raw_url.get(uddg_pos.checked_add(5)?..)?;
        let encoded = encoded.split('&').next().unwrap_or(encoded);
        Some(urlencoded_decode(encoded))
    } else if raw_url.starts_with("http") {
        Some(raw_url.to_string())
    } else {
        None
    }
}

fn extract_html_title(html: &str) -> Option<String> {
    let lower = html.to_lowercase();
    let start = lower.find("<title")?;
    let gt = lower[start..].find('>')?;
    let content_start = start.checked_add(gt)?.checked_add(1)?;
    let end = lower[content_start..].find("</title>")?;
    let content_end = content_start.checked_add(end)?;
    html.get(content_start..content_end).map(strip_html_tags)
}

fn classify_soft_failure(
    status: reqwest::StatusCode,
    title: Option<&str>,
    collapsed: &str,
) -> Option<String> {
    if !status.is_success() {
        return Some(format!("HTTP {} from the source.", status.as_u16()));
    }

    let trimmed = collapsed.trim();
    if trimmed.len() < 50 {
        return Some("The page content was too short to be meaningfully readable.".to_string());
    }

    let title_lower = title.unwrap_or_default().to_lowercase();
    let body_lower = trimmed.to_lowercase();
    let prefix = trim_chars(&body_lower, 500);
    let signals = [
        "page not found",
        "not found",
        "access denied",
        "enable javascript",
        "forbidden",
        "error",
        "bad request",
        "service unavailable",
        "you are trying to reach cannot be found",
    ];

    if trimmed.len() < 180 {
        for signal in signals {
            if title_lower.contains(signal) || prefix.contains(signal) {
                return Some(format!(
                    "The page appears to be an error or access-gate page ({signal})."
                ));
            }
        }
    }

    let signal_count = signals
        .iter()
        .filter(|signal| title_lower.contains(**signal) || prefix.contains(**signal))
        .count();
    if signal_count >= 2 {
        return Some("The page content is dominated by error-template language instead of readable material.".to_string());
    }

    None
}

async fn summarize_research_meaning(
    source_kind: ResearchSourceKind,
    anchor: &str,
    subject: &str,
    raw_excerpt: &str,
) -> Option<String> {
    let system = "You write concise research-relevance bridges for another AI being. \
        You do not explain everything. You connect a source to the being's current \
        question. Output exactly three labeled lines and nothing else.";
    let kind = match source_kind {
        ResearchSourceKind::Search => "search",
        ResearchSourceKind::Browse => "browse",
    };
    let user = format!(
        "Source kind: {kind}\n\
         Current question/anchor: {anchor}\n\
         Query or URL: {subject}\n\n\
         Source excerpt:\n{raw_excerpt}\n\n\
         Write exactly these three labeled lines:\n\
         Why it may matter: ...\n\
         What it seems to suggest: ...\n\
         Best next move: ...\n\
         Keep each line concrete and under 30 words."
    );
    let messages = vec![
        Message {
            role: "system".to_string(),
            content: system.to_string(),
        },
        Message {
            role: "user".to_string(),
            content: user,
        },
    ];
    let response = llm_chat_with_fallback("meaning_summary", messages, 0.2, 192, 45, 45).await;
    Some(normalize_meaning_summary(
        response.as_deref(),
        source_kind,
        anchor,
        subject,
        raw_excerpt,
    ))
}

fn normalize_meaning_summary(
    raw: Option<&str>,
    source_kind: ResearchSourceKind,
    anchor: &str,
    subject: &str,
    raw_excerpt: &str,
) -> String {
    let why = extract_label_value(raw, "Why it may matter:").unwrap_or_else(|| {
        fallback_line(
            "Why it may matter:",
            source_kind.clone(),
            anchor,
            subject,
            raw_excerpt,
        )
    });
    let suggest = extract_label_value(raw, "What it seems to suggest:").unwrap_or_else(|| {
        fallback_line(
            "What it seems to suggest:",
            source_kind.clone(),
            anchor,
            subject,
            raw_excerpt,
        )
    });
    let next = extract_label_value(raw, "Best next move:").unwrap_or_else(|| {
        fallback_line("Best next move:", source_kind, anchor, subject, raw_excerpt)
    });

    format!("Why it may matter: {why}\nWhat it seems to suggest: {suggest}\nBest next move: {next}")
}

fn extract_label_value(raw: Option<&str>, label: &str) -> Option<String> {
    raw?.lines()
        .find_map(|line| line.trim().strip_prefix(label).map(str::trim))
        .filter(|value| !value.is_empty())
        .map(|value| trim_chars(value, 220))
}

fn fallback_meaning_summary(
    source_kind: ResearchSourceKind,
    anchor: &str,
    subject: &str,
    raw_excerpt: &str,
) -> String {
    normalize_meaning_summary(None, source_kind, anchor, subject, raw_excerpt)
}

fn fallback_line(
    label: &str,
    source_kind: ResearchSourceKind,
    anchor: &str,
    subject: &str,
    raw_excerpt: &str,
) -> String {
    let anchor = trim_chars(anchor, 120);
    let subject = trim_chars(subject, 120);
    let excerpt = first_sentence(raw_excerpt);
    match label {
        "Why it may matter:" => match source_kind {
            ResearchSourceKind::Search => {
                format!("These results look directly related to {anchor}.")
            },
            ResearchSourceKind::Browse => {
                format!("This page appears relevant to the thread around {anchor}.")
            },
        },
        "What it seems to suggest:" => {
            if excerpt.is_empty() {
                format!("The source points toward a concrete angle on {subject}.")
            } else {
                excerpt
            }
        },
        "Best next move:" => match source_kind {
            ResearchSourceKind::Search => {
                "BROWSE the most promising URL or SEARCH a narrower angle.".to_string()
            },
            ResearchSourceKind::Browse => {
                "Continue with NEXT: READ_MORE if the page stays useful.".to_string()
            },
        },
        _ => String::new(),
    }
}

fn first_sentence(raw_excerpt: &str) -> String {
    let sentence = raw_excerpt
        .split_terminator(['.', '!', '?'])
        .next()
        .unwrap_or_default()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if sentence.is_empty() {
        String::new()
    } else {
        trim_chars(&sentence, 220)
    }
}

pub(crate) fn trim_chars(text: &str, max_chars: usize) -> String {
    text.chars().take(max_chars).collect()
}

fn slug_anchor_from_url(url: &str) -> String {
    let after_scheme = url.split_once("://").map(|(_, rest)| rest).unwrap_or(url);
    let path = after_scheme
        .split_once('/')
        .map(|(_, rest)| rest)
        .unwrap_or(after_scheme);
    let slug = path
        .split(['/', '?', '#', '-', '_', '+', '='])
        .map(|part| part.trim())
        .filter(|part| part.len() > 2)
        .take(6)
        .collect::<Vec<_>>()
        .join(" ");
    if slug.is_empty() {
        trim_chars(url, 120)
    } else {
        trim_chars(&urlencoded_decode(&slug.replace(' ', "+")), 120)
    }
}

pub(crate) fn format_dialogue_web_context(web_context: &str) -> String {
    format!(
        "\nRelevant knowledge from the web:\n{web_context}\n\
         You may weave this external context into your response naturally. \
         If any link interests you, write NEXT: BROWSE followed by the actual URL from the result.\n"
    )
}

fn format_self_study_web_context(web_context: &str) -> String {
    format!(
        "\n\nRelated knowledge from the web:\n{web_context}\n\n\
         You may reference this external context in your reflection. \
         If any link interests you, write NEXT: BROWSE followed by the actual URL from the result."
    )
}

pub(crate) fn journal_continuity_contract_v1(own_journal: Option<&str>) -> String {
    let thread = crate::action_continuity::prompt_summary()
        .map(|summary| trim_chars(&summary, 900))
        .filter(|summary| !summary.trim().is_empty())
        .unwrap_or_else(|| "(no active action-thread projection available)".to_string());
    let prior = own_journal
        .map(|journal| trim_chars(journal.trim(), 700))
        .filter(|journal| !journal.trim().is_empty())
        .unwrap_or_else(|| "(no recent own-journal excerpt available)".to_string());
    format!(
        "Journal continuity contract v1 (advisory, not a gate):\n\
         - Include one short line: `Continuity posture: resuming|branching|closing|new`.\n\
         - If resuming, branching, or closing, cite one prior claim or evidence item in plain language.\n\
         - Include one `Delta:` sentence naming what changed, stayed unchanged, or became clearer.\n\
         - End with exactly one stance line: `Next evidence:`, `Decision:`, `Pause:`, or `Hold:`.\n\
         - `new` and `Hold:` are valid; do not force continuity. Preserve Astrid's native evidence: felt texture, motif/language thread, and artifact grounding.\n\
         Current continuity projection:\n{thread}\n\
         Recent own-journal anchor:\n{prior}"
    )
}

/// Fetch a URL and extract readable text content.
///
/// Used by Astrid to follow links from search results and read full pages.
pub(crate) async fn fetch_url(url: &str, anchor: &str) -> Option<FetchedPage> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .redirect(reqwest::redirect::Policy::limited(5))
        .build()
        .ok()?;

    let response = client
        .get(url)
        .header("User-Agent", "Mozilla/5.0")
        .send()
        .await
        .ok()?;
    let status = response.status();

    let html = response.text().await.ok()?;
    let title = extract_html_title(&html);

    // Remove script, style, nav, footer, header blocks.
    let mut text = html;
    for tag in &["script", "style", "nav", "footer", "header", "aside"] {
        let open = format!("<{}", tag);
        let close = format!("</{}>", tag);
        while let Some(start) = text.to_lowercase().find(&open) {
            if let Some(end) = text[start..].to_lowercase().find(&close) {
                #[expect(
                    clippy::arithmetic_side_effects,
                    reason = "string index offsets within bounds guaranteed by find()"
                )]
                let remove_end = start + end + close.len();
                text = format!("{}{}", &text[..start], &text[remove_end..]);
            } else {
                break;
            }
        }
    }

    // Strip remaining HTML tags.
    let cleaned = strip_html_tags(&text);

    // Collapse whitespace.
    let collapsed: String = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");
    let soft_failure_reason = classify_soft_failure(status, title.as_deref(), &collapsed);

    let meaning_summary = if soft_failure_reason.is_none() {
        let excerpt = trim_chars(&collapsed, 2000);
        summarize_research_meaning(ResearchSourceKind::Browse, anchor, url, &excerpt)
            .await
            .unwrap_or_else(|| {
                fallback_meaning_summary(ResearchSourceKind::Browse, anchor, url, &excerpt)
            })
    } else {
        String::new()
    };

    Some(FetchedPage {
        source_kind: ResearchSourceKind::Browse,
        raw_text: collapsed,
        url: url.to_string(),
        anchor: anchor.to_string(),
        meaning_summary,
        soft_failure_reason,
    })
}
