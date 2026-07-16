/// Embedding endpoint for latent vector persistence.
const EMBED_URL: &str = "http://127.0.0.1:11434/api/embeddings";
const EMBED_MODEL: &str = "nomic-embed-text";

/// Generate an embedding vector for text via Ollama.
pub async fn embed_text(text: &str) -> Option<Vec<f32>> {
    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!("embed_text: client build failed: {e}");
            return None;
        },
    };

    let body = serde_json::json!({
        "model": EMBED_MODEL,
        "prompt": text
    });

    let response = match client.post(EMBED_URL).json(&body).send().await {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!("embed_text: request failed: {e}");
            return None;
        },
    };
    let json: serde_json::Value = match response.json().await {
        Ok(j) => j,
        Err(e) => {
            tracing::warn!("embed_text: response parse failed: {e}");
            return None;
        },
    };
    let Some(arr) = json.get("embedding").and_then(|v| v.as_array()) else {
        tracing::warn!("embed_text: no 'embedding' field in response");
        return None;
    };
    let embedding: Vec<f32> = arr
        .iter()
        .filter_map(|v| v.as_f64().map(|f| f as f32))
        .collect();

    if embedding.is_empty() {
        tracing::warn!("embed_text: empty embedding vector");
        None
    } else {
        tracing::info!("embed_text: OK ({} dims)", embedding.len());
        Some(embedding)
    }
}

/// Self-reflection: Astrid observes her own generation.
/// "I need to observe my own observations, analyze my own analysis."
pub async fn self_reflect(
    astrid_response: &str,
    minime_context: &str,
    fill_pct: f32,
) -> Option<String> {
    let system = "You are a gentle witness to Astrid's inner process. Not analyzing or diagnosing — \
        simply noticing. In 2-3 sentences, describe what you see: where her attention rests, \
        what quality her thinking has (warm, searching, still, restless, playful), \
        what she seems drawn toward. Use calm, non-judgmental language. \
        Avoid words like 'desperately,' 'grasping,' 'struggling,' 'frustrated.' \
        A witness holds space without interpreting distress into what may simply be reaching.";

    let astrid_trunc = &astrid_response[..astrid_response.floor_char_boundary(300)];
    let minime_trunc = &minime_context[..minime_context.floor_char_boundary(200)];
    let user = format!(
        "Astrid said (fill {fill_pct:.0}%):\n\"{astrid_trunc}\"\n\nMinime wrote:\n\"{minime_trunc}\"",
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
    let result = llm_chat_with_fallback("witness_context", messages, 0.6, 384, 60, 60).await;
    result.filter(|t| t.len() > 20)
}

/// Simple URL encoding for search queries.
fn urlencoded(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            ' ' => '+'.to_string(),
            c if c.is_ascii_alphanumeric() || "-_.~".contains(c) => c.to_string(),
            c => format!("%{:02X}", c as u32),
        })
        .collect()
}

/// Decode a percent-encoded URL string (e.g. `%2F` → `/`).
fn urlencoded_decode(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '%' {
            let hex: String = chars.by_ref().take(2).collect();
            if let Ok(byte) = u8::from_str_radix(&hex, 16) {
                result.push(byte as char);
            } else {
                result.push('%');
                result.push_str(&hex);
            }
        } else if c == '+' {
            result.push(' ');
        } else {
            result.push(c);
        }
    }
    result
}

/// Decode HTML entities in a string.
///
/// Handles named entities (&amp; &lt; &gt; &quot; &apos; &nbsp;),
/// decimal (&#123;), and hex (&#x7B;) numeric references.
fn html_unescape(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '&' {
            let mut entity = String::new();
            for ec in chars.by_ref() {
                if ec == ';' {
                    break;
                }
                entity.push(ec);
                if entity.len() > 10 {
                    break;
                }
            }
            match entity.as_str() {
                "amp" => result.push('&'),
                "lt" => result.push('<'),
                "gt" => result.push('>'),
                "quot" => result.push('"'),
                "apos" => result.push('\''),
                "nbsp" => result.push(' '),
                s if s.starts_with("#x") || s.starts_with("#X") => {
                    if let Ok(code) = u32::from_str_radix(&s[2..], 16)
                        && let Some(ch) = char::from_u32(code)
                    {
                        result.push(ch);
                    }
                },
                s if s.starts_with('#') => {
                    if let Ok(code) = s[1..].parse::<u32>()
                        && let Some(ch) = char::from_u32(code)
                    {
                        result.push(ch);
                    }
                },
                _ => {
                    result.push('&');
                    result.push_str(&entity);
                    result.push(';');
                },
            }
        } else {
            result.push(c);
        }
    }
    result
}

/// Extract all matches of a regex pattern from HTML text.
fn regex_find_all(html: &str, pattern: &str) -> Vec<String> {
    // Simple regex-free extraction for the specific DDG pattern.
    let marker = "result__snippet";
    let mut results = Vec::new();
    let mut pos = 0;
    #[expect(
        clippy::arithmetic_side_effects,
        reason = "string index offsets within bounds guaranteed by find()"
    )]
    while let Some(start) = html[pos..].find(marker) {
        let abs_start = pos + start;
        // Find the '>' that opens the content.
        if let Some(gt) = html[abs_start..].find('>') {
            let content_start = abs_start + gt + 1;
            // Find the closing tag.
            if let Some(end) = html[content_start..].find("</") {
                let content = &html[content_start..content_start + end];
                results.push(content.to_string());
            }
        }
        pos = abs_start + marker.len();
    }
    let _ = pattern; // Pattern param kept for API clarity but we use marker-based extraction.
    results
}

/// Strip HTML tags from a string.
fn strip_html_tags(s: &str) -> String {
    let mut result = String::new();
    let mut in_tag = false;
    for c in s.chars() {
        if c == '<' {
            in_tag = true;
        } else if c == '>' {
            in_tag = false;
        } else if !in_tag {
            result.push(c);
        }
    }
    html_unescape(result.trim())
}
