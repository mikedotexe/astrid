// Mechanical reader progress extends the existing authoritative session record.
// Delivery evidence comes from the final completion adapter, not this offer builder.

fn reader_bookmark_from_record(record: &Value) -> Result<Option<ReaderBookmark>> {
    let Some(value) = record
        .get("reader_bookmark_v1")
        .filter(|value| !value.is_null())
    else {
        return Ok(None);
    };
    let bookmark: ReaderBookmark = serde_json::from_value(value.clone())?;
    if bookmark.schema_version != 1
        || record.get("session_id").and_then(Value::as_str) != Some(bookmark.session_id.as_str())
    {
        return Err(anyhow!("reader bookmark schema/session mismatch"));
    }
    Ok(Some(bookmark))
}

fn reader_record_id(record: &Value) -> Result<&str> {
    record
        .get("record_id")
        .and_then(Value::as_str)
        .filter(|id| !id.is_empty())
        .ok_or_else(|| anyhow!("continuity session has no record identity"))
}

fn reader_check_version(
    record: &Value,
    bookmark: &ReaderBookmark,
    expected: &ReaderBookmarkVersion,
) -> Result<()> {
    if reader_record_id(record)? != expected.session_record_id
        || bookmark.revision != expected.revision
    {
        return Err(anyhow!(
            "stale reader revision: inspect the current session before changing it"
        ));
    }
    Ok(())
}

fn reader_is_active(record: &Value) -> bool {
    matches!(
        record.get("status").and_then(Value::as_str),
        Some("active" | "summarized")
    )
}

impl ActionContinuityStore {
    /// Read the selected session across its complete log without creating a thread,
    /// changing foreground, advancing a cursor, or scheduling a return.
    pub fn reader_bookmark_preview(
        &self,
        thread_id: &str,
        session_id: &str,
    ) -> Result<Option<ReaderBookmarkPreview>> {
        let Some(log) =
            reader_bookmark_io::ReaderSessionLog::open(self, thread_id, session_id, None)?
        else {
            return Ok(None);
        };
        let Some(record) = log.latest.as_ref() else {
            return Ok(None);
        };
        let bookmark = reader_bookmark_from_record(record)?;
        let mut source_comparison = None;
        let mut offered_text = None;
        if let Some(bookmark) = bookmark.as_ref() {
            let retained = reader_bookmark_io::retained_text(self, &bookmark.source);
            let current = reader_bookmark_io::read_utf8(&bookmark.source.original_path)
                .ok()
                .map(|text| reader_bookmark_io::digest(text.as_bytes()));
            source_comparison = Some(ReaderSourceComparison {
                retained_source_available: retained.is_ok(),
                retained_source_error: retained.as_ref().err().map(ToString::to_string),
                current_status: match current.as_deref() {
                    Some(hash) if hash == bookmark.source.sha256 => {
                        ReaderCurrentSourceStatus::Unchanged
                    },
                    Some(_) => ReaderCurrentSourceStatus::Changed,
                    None => ReaderCurrentSourceStatus::Unavailable,
                },
                current_sha256: current,
            });
            if let (Ok(text), Some(passage)) = (retained, bookmark.offered_passage.as_ref()) {
                offered_text = Some(reader_bookmark_io::passage_text(&text, passage)?.to_string());
            }
        }
        Ok(Some(ReaderBookmarkPreview {
            session_id: session_id.to_string(),
            session_record_id: reader_record_id(record)?.to_string(),
            session_status: record
                .get("status")
                .and_then(Value::as_str)
                .unwrap_or("unknown")
                .to_string(),
            bookmark,
            source_comparison,
            offered_text,
        }))
    }

    /// Attach an immutable source to an existing explicitly selected active session.
    pub fn reader_bookmark_create(
        &self,
        thread_id: &str,
        session_id: &str,
        expected_session_record_id: &str,
        source_path: &Path,
        operation_id: &str,
    ) -> Result<ReaderBookmarkReceipt> {
        let request = json!({"kind":"create", "expected_session_record_id":expected_session_record_id, "source_path":source_path});
        self.reader_bookmark_mutate(
            thread_id,
            session_id,
            operation_id,
            request,
            |record, prior| {
                if reader_record_id(record)? != expected_session_record_id {
                    return Err(anyhow!("stale reader session: source was not attached"));
                }
                if prior.is_some() {
                    return Err(anyhow!("session already has a reader bookmark"));
                }
                if !reader_is_active(record) {
                    return Err(anyhow!(
                        "explicitly resume the session before choosing a source"
                    ));
                }
                let source = reader_bookmark_io::retain_source(self, source_path)?;
                Ok((
                    ReaderBookmark {
                        schema_version: 1,
                        activity_id: format!(
                            "reader_{}",
                            reader_bookmark_io::digest(
                                format!("{thread_id}\0{session_id}").as_bytes()
                            )
                        ),
                        session_id: session_id.to_string(),
                        revision: 0,
                        disposition: ReaderDisposition::Active,
                        source,
                        cursor: ReaderUtf8Cursor { next_byte: 0 },
                        last_committed_passage: None,
                        offered_passage: None,
                        authored_focus: record
                            .get("focus")
                            .and_then(Value::as_str)
                            .filter(|value| !value.is_empty())
                            .map(str::to_string),
                        stopping_note: None,
                    },
                    None,
                ))
            },
        )
    }

    /// Offer bounded UTF-8 bytes. Reoffering pending bytes never skips them.
    pub fn reader_bookmark_offer(
        &self,
        thread_id: &str,
        session_id: &str,
        expected: &ReaderBookmarkVersion,
        max_bytes: usize,
        operation_id: &str,
    ) -> Result<ReaderBookmarkReceipt> {
        let request = json!({"kind":"offer", "expected":expected, "max_bytes":max_bytes});
        self.reader_bookmark_mutate(
            thread_id,
            session_id,
            operation_id,
            request,
            |record, bookmark| {
                let mut bookmark =
                    bookmark.ok_or_else(|| anyhow!("session has no reader bookmark"))?;
                reader_check_version(record, &bookmark, expected)?;
                if !reader_is_active(record) || bookmark.disposition != ReaderDisposition::Active {
                    return Err(anyhow!(
                        "parked reader remains quiet until explicitly resumed"
                    ));
                }
                if max_bytes == 0 || max_bytes > READER_PASSAGE_MAX_BYTES {
                    return Err(anyhow!(
                        "reader passage byte budget must be within 1..={READER_PASSAGE_MAX_BYTES}"
                    ));
                }
                let text = reader_bookmark_io::retained_text(self, &bookmark.source)?;
                if let Some(offered) = &bookmark.offered_passage {
                    reader_bookmark_io::passage_text(&text, offered)?;
                    return Ok((bookmark, None));
                }
                let start = usize::try_from(bookmark.cursor.next_byte)?;
                if start >= text.len() || !text.is_char_boundary(start) {
                    return Err(anyhow!(
                        "reader is at end of source or has an invalid UTF-8 cursor"
                    ));
                }
                let mut end = start.saturating_add(max_bytes).min(text.len());
                while !text.is_char_boundary(end) {
                    end = end.saturating_sub(1);
                }
                if end == start {
                    return Err(anyhow!(
                        "byte budget cannot fit the next complete UTF-8 character"
                    ));
                }
                bookmark.offered_passage = Some(ReaderPassage {
                    offer_id: format!(
                        "offer_{}",
                        reader_bookmark_io::digest(
                            format!("{thread_id}\0{session_id}\0{operation_id}").as_bytes()
                        )
                    ),
                    source_sha256: bookmark.source.sha256.clone(),
                    start_byte: u64::try_from(start)?,
                    end_byte: u64::try_from(end)?,
                    sha256: reader_bookmark_io::digest(&text.as_bytes()[start..end]),
                });
                Ok((bookmark, None))
            },
        )
    }

    /// Commit only an adapter-attested contiguous prefix of the current offer.
    /// A partial delivery leaves its undelivered suffix explicitly offered.
    pub fn reader_bookmark_commit(
        &self,
        thread_id: &str,
        session_id: &str,
        expected: &ReaderBookmarkVersion,
        evidence: &ReaderDeliveryEvidence,
        operation_id: &str,
    ) -> Result<ReaderBookmarkReceipt> {
        let request = json!({"kind":"commit", "expected":expected, "evidence":evidence});
        self.reader_bookmark_mutate(thread_id, session_id, operation_id, request, |record, bookmark| {
            let mut bookmark = bookmark.ok_or_else(|| anyhow!("session has no reader bookmark"))?;
            reader_check_version(record, &bookmark, expected)?;
            let offered = bookmark.offered_passage.clone().ok_or_else(|| anyhow!("reader has no offered passage"))?;
            if evidence.offer_id != offered.offer_id || offered.source_sha256 != bookmark.source.sha256
                || evidence.supplied_start_byte != bookmark.cursor.next_byte
                || evidence.supplied_start_byte != offered.start_byte
                || evidence.supplied_end_byte <= evidence.supplied_start_byte
                || evidence.supplied_end_byte > offered.end_byte
                || !reader_bookmark_io::hash_shape(&evidence.final_request_sha256)
                || !reader_bookmark_io::hash_shape(&evidence.retained_output_sha256)
                || evidence.retained_output_ref.trim().is_empty()
            {
                return Err(anyhow!("delivery evidence does not identify the current offered passage and retained completion"));
            }
            let text = reader_bookmark_io::retained_text(self, &bookmark.source)?;
            reader_bookmark_io::passage_text(&text, &offered)?;
            let committed = ReaderPassage {
                offer_id: offered.offer_id.clone(), source_sha256: bookmark.source.sha256.clone(),
                start_byte: evidence.supplied_start_byte, end_byte: evidence.supplied_end_byte,
                sha256: evidence.supplied_sha256.clone(),
            };
            reader_bookmark_io::passage_text(&text, &committed)?;
            bookmark.cursor.next_byte = committed.end_byte;
            bookmark.last_committed_passage = Some(ReaderCommittedPassage { passage: committed, delivery: evidence.clone() });
            bookmark.offered_passage = if evidence.supplied_end_byte < offered.end_byte {
                let start = usize::try_from(evidence.supplied_end_byte)?;
                let end = usize::try_from(offered.end_byte)?;
                Some(ReaderPassage { start_byte: evidence.supplied_end_byte,
                    sha256: reader_bookmark_io::digest(&text.as_bytes()[start..end]), ..offered })
            } else { None };
            // Mechanical delivery does not undo an ordinary later park/finish decision.
            bookmark.disposition = match record.get("status").and_then(Value::as_str) {
                Some("parked" | "held") => ReaderDisposition::Parked,
                Some("complete") => ReaderDisposition::Complete,
                Some("abandoned") => ReaderDisposition::Abandoned,
                _ => bookmark.disposition,
            };
            Ok((bookmark, None))
        })
    }

    /// Explicit lifecycle choice. It restores no command and requires no report.
    pub fn reader_bookmark_transition(
        &self,
        thread_id: &str,
        session_id: &str,
        expected: &ReaderBookmarkVersion,
        disposition: ReaderDisposition,
        operation_id: &str,
    ) -> Result<ReaderBookmarkReceipt> {
        let request = json!({"kind":"transition", "expected":expected, "disposition":disposition});
        self.reader_bookmark_mutate(
            thread_id,
            session_id,
            operation_id,
            request,
            |record, bookmark| {
                let mut bookmark =
                    bookmark.ok_or_else(|| anyhow!("session has no reader bookmark"))?;
                reader_check_version(record, &bookmark, expected)?;
                if disposition == ReaderDisposition::Active {
                    reader_bookmark_io::retained_text(self, &bookmark.source)?;
                }
                bookmark.disposition = disposition;
                Ok((bookmark, Some(disposition.session_status())))
            },
        )
    }

    fn reader_bookmark_mutate(
        &self,
        thread_id: &str,
        session_id: &str,
        operation_id: &str,
        request: Value,
        apply: impl FnOnce(
            &Value,
            Option<ReaderBookmark>,
        ) -> Result<(ReaderBookmark, Option<&'static str>)>,
    ) -> Result<ReaderBookmarkReceipt> {
        let mut log = reader_bookmark_io::ReaderSessionLog::open(
            self,
            thread_id,
            session_id,
            Some(operation_id),
        )?
        .ok_or_else(|| anyhow!("selected continuity session does not exist"))?;
        let fingerprint = reader_bookmark_io::digest(&serde_json::to_vec(&json!({
            "thread_id":thread_id, "session_id":session_id, "request":request,
        }))?);
        if let Some(prior) = &log.prior_operation {
            if prior.get("fingerprint").and_then(Value::as_str) != Some(fingerprint.as_str()) {
                return Err(anyhow!(
                    "reader operation identity reused with different inputs"
                ));
            }
            let mut receipt: ReaderBookmarkReceipt = serde_json::from_value(
                prior
                    .get("receipt")
                    .cloned()
                    .ok_or_else(|| anyhow!("saved reader operation has no receipt"))?,
            )?;
            log.sync().map_err(|error| ReaderPersistenceError {
                operation_id: operation_id.to_string(),
                record_id: receipt.record_id.clone(),
                stage: ReaderAppendStage::AppendedNotSynced,
                detail: error.to_string(),
            })?;
            receipt.duplicate = true;
            return Ok(receipt);
        }
        let mut record = log
            .latest
            .clone()
            .ok_or_else(|| anyhow!("selected continuity session does not exist"))?;
        if record.get("thread_id").and_then(Value::as_str) != Some(thread_id) {
            return Err(anyhow!("continuity session thread identity mismatch"));
        }
        let prior = reader_bookmark_from_record(&record)?;
        let revision = prior
            .as_ref()
            .map_or(0, |bookmark| bookmark.revision)
            .checked_add(1)
            .ok_or_else(|| anyhow!("reader revision exhausted"))?;
        let (mut bookmark, status) = apply(&record, prior)?;
        bookmark.revision = revision;
        let record_id = format!(
            "cs_astrid_reader_{}",
            reader_bookmark_io::digest(
                format!("{thread_id}\0{session_id}\0{operation_id}").as_bytes()
            )
        );
        let receipt = ReaderBookmarkReceipt {
            operation_id: operation_id.to_string(),
            record_id: record_id.clone(),
            version: ReaderBookmarkVersion {
                revision,
                session_record_id: record_id.clone(),
            },
            bookmark: bookmark.clone(),
            duplicate: false,
            primary_record_durable: true,
            derived_projection_required: false,
        };
        record["record_id"] = json!(record_id);
        record["record_type"] = json!("session_reader_bookmark");
        record["updated_at"] = json!(iso_now());
        record["reader_bookmark_v1"] = serde_json::to_value(bookmark)?;
        record["reader_operation_v1"] =
            json!({"operation_id":operation_id,"fingerprint":fingerprint,"receipt":receipt});
        if let Some(status) = status {
            record["status"] = json!(status);
            record["automatic_return"] = json!(false);
        }
        // A previous command's CAS guard is not inherited into this new operation.
        if let Some(fields) = record.as_object_mut() {
            fields.remove("expected_session_record_id");
        }
        log.append(&record, operation_id, &receipt.record_id)?;
        Ok(receipt)
    }
}
