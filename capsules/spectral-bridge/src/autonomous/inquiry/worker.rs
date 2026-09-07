//! Ownership of admitted inquiry workers during operator maintenance.
use super::*;

pub(super) fn spawn_inquiry_job(inquiry_id: String) {
    let active = ACTIVE_INQUIRY_JOBS.get_or_init(|| Mutex::new(HashSet::new()));
    let Ok(mut guard) = active.lock() else {
        return;
    };
    if !guard.insert(inquiry_id.clone()) {
        return;
    }
    drop(guard);
    let worker_inquiry_id = inquiry_id.clone();
    let spawn_result = crate::lifecycle::spawn_background_thread(
        format!("owner-inquiry-{inquiry_id}"),
        move || {
            let result = run_inquiry_job(&worker_inquiry_id);
            if let Ok(mut active) = ACTIVE_INQUIRY_JOBS
                .get_or_init(|| Mutex::new(HashSet::new()))
                .lock()
            {
                active.remove(&worker_inquiry_id);
            }
            match result {
                Ok(activated) => spawn_activated_jobs(&activated),
                Err(error) => {
                    let now = volition::now_unix_ms();
                    let _ = research::mark_failed(&worker_inquiry_id, &error, now);
                    if let Ok(mut runtime) = read_runtime(&worker_inquiry_id)
                        && runtime.status != OwnerInquiryStatusV1::Cancelled
                    {
                        runtime.status = OwnerInquiryStatusV1::Failed;
                        runtime.failure = Some(error.clone());
                        runtime.updated_at_unix_ms = now;
                        let _ = write_runtime(&runtime);
                        let _ = concern_queue::transition_owner_inquiry(
                            &worker_inquiry_id,
                            BeingConcernStatusV1::Blocked,
                            now,
                        );
                    }
                },
            }
        },
    );
    if spawn_result.is_err()
        && let Ok(mut active) = ACTIVE_INQUIRY_JOBS
            .get_or_init(|| Mutex::new(HashSet::new()))
            .lock()
    {
        active.remove(&inquiry_id);
    }
}
