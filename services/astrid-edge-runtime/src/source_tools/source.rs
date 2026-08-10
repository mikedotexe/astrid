//! Bounded, read-only views over an already attested source snapshot.

use super::{
    fs::SourceIndex,
    types::{
        BrokerError, BrokerResult, ListSourceRequest, ListSourceResult, MAX_CHUNK_CHARS,
        MAX_CHUNK_LINES, MAX_EXCERPT_CHARS, MAX_LIST_RESULTS, MAX_SEARCH_FILES, MAX_SEARCH_MATCHES,
        MAX_SEARCH_QUERY_CHARS, NumberedLine, ReadSourceChunkRequest, ReadSourceChunkResult,
        SearchMatch, SearchSourceRequest, SearchSourceResult,
    },
};

pub(crate) fn list_source(
    source: &SourceIndex,
    request: &ListSourceRequest,
) -> BrokerResult<ListSourceResult> {
    if request.limit == 0 || request.limit > MAX_LIST_RESULTS {
        return Err(BrokerError::LimitExceeded("source list result count"));
    }
    if request.cursor > source.files.len() {
        return Err(BrokerError::InvalidInput(
            "source list cursor is out of range",
        ));
    }
    let end = request
        .cursor
        .saturating_add(request.limit)
        .min(source.files.len());
    let entries = source.files[request.cursor..end]
        .iter()
        .map(super::fs::IndexedSourceFile::public_entry)
        .collect();
    Ok(ListSourceResult {
        source_id: source.source_id.clone(),
        manifest_sha256: source.manifest_sha256.clone(),
        entries,
        next_cursor: (end < source.files.len()).then_some(end),
    })
}

pub(crate) fn search_source(
    source: &SourceIndex,
    request: &SearchSourceRequest,
) -> BrokerResult<SearchSourceResult> {
    let query = request.query.trim();
    let query_chars = query.chars().count();
    if query_chars == 0 || query_chars > MAX_SEARCH_QUERY_CHARS {
        return Err(BrokerError::LimitExceeded("source search query characters"));
    }
    if request.max_files == 0 || request.max_files > MAX_SEARCH_FILES {
        return Err(BrokerError::LimitExceeded("source search file count"));
    }
    if request.max_matches == 0 || request.max_matches > MAX_SEARCH_MATCHES {
        return Err(BrokerError::LimitExceeded("source search match count"));
    }
    if request.cursor > source.files.len() {
        return Err(BrokerError::InvalidInput(
            "source search cursor is out of range",
        ));
    }

    let query_folded = query.to_lowercase();
    let end = request
        .cursor
        .saturating_add(request.max_files)
        .min(source.files.len());
    let mut matches = Vec::new();
    let mut files_considered = 0_usize;
    let mut next_cursor = None;
    for (offset, file) in source.files[request.cursor..end].iter().enumerate() {
        files_considered = files_considered.saturating_add(1);
        let text = source.read_file(file)?;
        for (line_index, line) in text.lines().enumerate() {
            if line.to_lowercase().contains(&query_folded) {
                matches.push(SearchMatch {
                    source_file_id: file.source_file_id.clone(),
                    basename: file.basename.clone(),
                    line_number: line_index.saturating_add(1),
                    excerpt: bounded_excerpt(line),
                });
                if matches.len() == request.max_matches {
                    let absolute_index = request.cursor.saturating_add(offset);
                    next_cursor = (absolute_index.saturating_add(1) < source.files.len())
                        .then_some(absolute_index.saturating_add(1));
                    break;
                }
            }
        }
        if matches.len() == request.max_matches {
            break;
        }
    }
    if matches.len() < request.max_matches && end < source.files.len() {
        next_cursor = Some(end);
    }

    Ok(SearchSourceResult {
        source_id: source.source_id.clone(),
        manifest_sha256: source.manifest_sha256.clone(),
        matches,
        next_cursor,
        files_considered,
    })
}

pub(crate) fn read_source_chunk(
    source: &SourceIndex,
    request: &ReadSourceChunkRequest,
) -> BrokerResult<ReadSourceChunkResult> {
    if request.start_line == 0 {
        return Err(BrokerError::InvalidInput(
            "source line numbers are one-based",
        ));
    }
    if request.max_lines == 0 || request.max_lines > MAX_CHUNK_LINES {
        return Err(BrokerError::LimitExceeded("source chunk line count"));
    }
    let file = source.file(&request.source_file_id)?;
    let text = source.read_file(file)?;
    let all_lines = text.lines().collect::<Vec<_>>();
    let start = request.start_line.saturating_sub(1);
    if start > all_lines.len() {
        return Err(BrokerError::InvalidInput(
            "source start line is out of range",
        ));
    }

    let requested_end = start.saturating_add(request.max_lines).min(all_lines.len());
    let mut lines = Vec::new();
    let mut characters = 0_usize;
    let mut truncated_by_character_limit = false;
    for (offset, line) in all_lines[start..requested_end].iter().enumerate() {
        let available = MAX_CHUNK_CHARS.saturating_sub(characters);
        if available == 0 {
            truncated_by_character_limit = true;
            break;
        }
        let line_chars = line.chars().count();
        let displayed = take_characters(line, available);
        characters = characters.saturating_add(displayed.chars().count());
        lines.push(NumberedLine {
            line_number: start.saturating_add(offset).saturating_add(1),
            text: displayed,
        });
        if line_chars > available {
            truncated_by_character_limit = true;
            break;
        }
    }

    let consumed = lines.len();
    let next_index = start.saturating_add(consumed);
    let next_line = (next_index < all_lines.len()).then_some(next_index.saturating_add(1));
    Ok(ReadSourceChunkResult {
        source_file_id: file.source_file_id.clone(),
        basename: file.basename.clone(),
        sha256: file.sha256.clone(),
        lines,
        next_line,
        truncated_by_character_limit,
    })
}

fn bounded_excerpt(line: &str) -> String {
    let normalized = line.split_whitespace().collect::<Vec<_>>().join(" ");
    take_characters(&normalized, MAX_EXCERPT_CHARS)
}

pub(crate) fn take_characters(value: &str, maximum: usize) -> String {
    value.chars().take(maximum).collect()
}
