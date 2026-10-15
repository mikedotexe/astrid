/// Request-scoped accounting from actual post-adaptation admission. No prose,
/// request bodies, identifiers or draft state are stored in this tracker.
#[derive(Default)]
struct WritingAdmissionSummary {
    accepted: bool,
    unknown_failure: bool,
    smallest_refusal: Option<astrid_source_study::FinalWritingCapacity>,
}

tokio::task_local! {
    static WRITING_ADMISSION: std::cell::RefCell<WritingAdmissionSummary>;
}

fn final_writing_capacity(
    messages: &[Message], input: &ProtectedDialogueInputV1, limit: usize,
) -> Option<astrid_source_study::FinalWritingCapacity> {
    let heading = protected_source_heading(input, input.source_start_byte.checked_add(input.source_text.len())?)?;
    Some(astrid_source_study::FinalWritingCapacity {
        prepared_input_bytes: input.source_text.len(),
        system_bytes: messages.iter().filter(|m| m.role == "system")
            .try_fold(0usize, |total, m| total.checked_add(m.content.len()))?,
        wrapper_bytes: heading.len().checked_add(protected_source_ending(input).len())?,
        allowance_bytes: limit,
    })
}

fn record_writing_admission(capacity: Option<astrid_source_study::FinalWritingCapacity>, accepted: bool) {
    let _ = WRITING_ADMISSION.try_with(|summary| {
        let mut summary = summary.borrow_mut();
        summary.accepted |= accepted;
        if accepted { return; }
        let Some(capacity) = capacity.filter(|c| c.complete_bytes().is_some_and(|bytes| bytes > c.allowance_bytes)) else {
            summary.unknown_failure = true;
            return;
        };
        let excess = |c: &astrid_source_study::FinalWritingCapacity| c.complete_bytes().unwrap_or(usize::MAX).saturating_sub(c.allowance_bytes);
        if summary.smallest_refusal.as_ref().is_none_or(|prior| excess(&capacity) < excess(prior)) {
            summary.smallest_refusal = Some(capacity);
        }
    });
}

fn writing_admission_failure() -> Option<astrid_source_study::FinalWritingCapacity> {
    WRITING_ADMISSION.try_with(|summary| {
        let summary = summary.borrow();
        // An admitted lane might have executed. Do not turn its transport or
        // retention failure into proof of pre-submission capacity refusal.
        if summary.accepted || summary.unknown_failure { None } else { summary.smallest_refusal.clone() }
    }).ok().flatten()
}
