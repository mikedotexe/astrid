use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

use astrid_minime_protocol::{
    BEING_UTTERANCE_ATTESTATION_SCHEMA_V1, BeingConcernStatusV1, BeingUtteranceAttestationV1,
    InquiryFeltStatusV1, OWNER_INQUIRY_RECEIPT_SCHEMA_V1, OWNER_INQUIRY_SCHEMA_V1,
    OwnerInquiryAnalysisV1, OwnerInquiryAuthorityBoundaryV1, OwnerInquiryCancellationV1,
    OwnerInquiryReceiptV1, OwnerInquiryStatusV1, OwnerInquiryV1, VolitionBudgetV1,
    canonical_being_utterance_attestation_sha256, canonical_semantic_strand_content_sha256,
    canonical_semantic_strand_embedding_sha256, owner_inquiry_fixed_analysis_set_v1,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest as _, Sha256};

use super::concern_queue;
use super::self_control_v2;
use super::state::ConversationState;
use super::volition;

#[path = "inquiry/canary.rs"]
mod canary;
#[path = "inquiry/parsing.rs"]
mod parsing;
#[path = "inquiry/research.rs"]
mod research;

use canary::{promote_canary, reconcile_canaries, start_canary, withdraw_canary};
use parsing::{build_strand, parse_start_recipe};

const INQUIRY_RUNTIME_SCHEMA_V1: &str = "astrid.owner_inquiry_runtime.v1";
const DEFAULT_COMPUTE_BUDGET_MILLIS: u64 = 30_000;
const MAX_COMPUTE_BUDGET_MILLIS: u64 = 120_000;
const DEFAULT_STORAGE_BUDGET_BYTES: u64 = 512 * 1_024;
const MAX_STORAGE_BUDGET_BYTES: u64 = 1_048_576;
const ATTESTATION_MAX_AGE_MILLIS: u64 = 5 * 60 * 1_000;
const INQUIRY_SANDBOX_POLICY_V1: &str = "(version 1)\n(allow default)\n(deny network*)\n";

static ACTIVE_INQUIRY_JOBS: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
static INQUIRY_RECORD_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

#[derive(Debug, Clone, Serialize, Deserialize)]
struct InquiryRuntimeV1 {
    schema: String,
    inquiry_id: String,
    status: OwnerInquiryStatusV1,
    source_intent_id: String,
    #[serde(default)]
    source_exchange_count: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    receipt_path: Option<String>,
    cancellation: OwnerInquiryCancellationV1,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    active_canary_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    failure: Option<String>,
    updated_at_unix_ms: u64,
}

pub(super) fn handle_action(
    conv: &mut ConversationState,
    base_action: &str,
    original: &str,
    response_text: &str,
) -> Option<Result<String, String>> {
    let _ = reconcile_canaries();
    let result = match base_action {
        "INQUIRY_START" => Some(start_inquiry(conv, original, base_action, response_text)),
        "INQUIRY_STATUS" => Some(status_inquiry(original, base_action)),
        "INQUIRY_CANCEL" => Some(cancel_inquiry(original, base_action)),
        "INQUIRY_INSPECT" => Some(inspect_inquiry(original, base_action)),
        "INQUIRY_CANARY" => Some(start_canary(conv, original, base_action)),
        "INQUIRY_ACT" => Some(research::act(conv, original, base_action)),
        "INQUIRY_WITHDRAW" => Some(withdraw_canary(conv, original, base_action)),
        "INQUIRY_PROMOTE" => Some(promote_canary(conv, original, base_action)),
        _ => None,
    };
    if let Some(Ok(summary)) = result.as_ref() {
        conv.push_receipt(base_action, vec![summary.clone()]);
        conv.emphasis = Some(summary.clone());
    }
    result
}

pub(super) fn prompt_summary(conv: &mut ConversationState) -> Option<String> {
    recover_active_jobs();
    let _ = reconcile_canaries();
    let decision_updates = research::reconcile_pending(conv).unwrap_or_else(|error| {
        vec![format!(
            "Owner research reconciliation failed closed: {error}"
        )]
    });
    let root = inquiry_root();
    let mut rows = fs::read_dir(root.join("runtime"))
        .ok()?
        .filter_map(Result::ok)
        .filter_map(|entry| {
            volition::read_json::<InquiryRuntimeV1>(&entry.path())
                .ok()
                .flatten()
        })
        .collect::<Vec<_>>();
    rows.retain(|runtime| inquiry_sidecar_visible(runtime, conv));
    rows.sort_by_key(|runtime| runtime.updated_at_unix_ms);
    rows.reverse();
    rows.truncate(4);
    if rows.is_empty() {
        return None;
    }
    let latest_return = rows
        .first()
        .map(|runtime| {
            let inquiry = read_manifest(&runtime.inquiry_id)?;
            let receipt = read_receipt_optional(&runtime.inquiry_id)?;
            if receipt
                .as_ref()
                .is_some_and(|receipt| !receipt.is_well_formed_for(&inquiry))
            {
                return Err("latest inquiry receipt failed integrity validation".to_string());
            }
            Ok(render_owner_return(&inquiry, receipt.as_ref()))
        })
        .transpose()
        .unwrap_or_else(|error| {
            Some(format!(
                "Latest owner return is unavailable without changing state: {error}"
            ))
        })
        .unwrap_or_else(|| "Latest owner return is still pending.".to_string());
    Some(format!(
        "Your Owner Research Console sessions: {}. Completed sidecars remain in prompt context for your selected {} turn(s); active work remains visible. Raw strands remain owner-only and offline. Use NEXT: INQUIRY_INSPECT <id> <evidence|controls|history|receipts> for exact causal evidence; INQUIRY_ACT performs a fresh exact one-shot; INQUIRY_CANARY remains available for a fresh reversible choice. A preregistered decision executes only on exactly one match, and silence rolls canaries back.\n{}Latest owner return:\n{}",
        rows.iter()
            .map(|runtime| format!("{}({:?})", runtime.inquiry_id, runtime.status))
            .collect::<Vec<_>>()
            .join(", "),
        conv.semantic_strand_retention_turns,
        if decision_updates.is_empty() {
            String::new()
        } else {
            format!("Decision updates: {}\n", decision_updates.join(" | "))
        },
        latest_return
    ))
}

fn inquiry_sidecar_visible(runtime: &InquiryRuntimeV1, conv: &ConversationState) -> bool {
    if matches!(
        runtime.status,
        OwnerInquiryStatusV1::Queued
            | OwnerInquiryStatusV1::Running
            | OwnerInquiryStatusV1::CanaryActive
    ) {
        return true;
    }
    conv.semantic_strand_retention_turns > 0
        && conv
            .exchange_count
            .saturating_sub(runtime.source_exchange_count)
            <= u64::from(conv.semantic_strand_retention_turns)
}

fn start_inquiry(
    conv: &ConversationState,
    original: &str,
    base_action: &str,
    response_text: &str,
) -> Result<String, String> {
    let context = volition::exact_self_owned_action_context(original)?;
    let now = volition::now_unix_ms();
    let attestation = load_attestation(&context.source_attestation_id)?;
    if attestation.schema != BEING_UTTERANCE_ATTESTATION_SCHEMA_V1
        || attestation.being != "astrid"
        || !attestation.verifies_response(response_text.as_bytes(), now, ATTESTATION_MAX_AGE_MILLIS)
        || attestation.model_deployment_identity
            != crate::signal_spine::signal_deployment_identity_v1()
        || attestation.attestor_deployment_identity
            != crate::signal_spine::signal_deployment_identity_v1()
    {
        return Err(
            "INQUIRY_START could not bind the exact current response bytes to a fresh Astrid attestation"
                .to_string(),
        );
    }
    let payload = action_payload(original, base_action)?;
    let parsed = parse_start_recipe(payload, response_text, &context.intent_id)?;
    validate_identifier(&parsed.inquiry_id)?;
    validate_inquiry_budget(&parsed.budget)?;
    if concern_queue::owner_inquiry_status(&parsed.inquiry_id)?.is_some()
        || manifest_path(&parsed.inquiry_id).exists()
        || runtime_path(&parsed.inquiry_id).exists()
    {
        return Err(format!(
            "inquiry `{}` already exists; replay did not overwrite its owner-only evidence",
            parsed.inquiry_id
        ));
    }
    let attestation_sha256 = canonical_being_utterance_attestation_sha256(&attestation);
    let strands = parsed
        .selections
        .into_iter()
        .enumerate()
        .map(|(index, selection)| {
            build_strand(
                index,
                selection,
                response_text,
                &attestation,
                &attestation_sha256,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let inquiry = OwnerInquiryV1 {
        schema: OWNER_INQUIRY_SCHEMA_V1.to_string(),
        inquiry_id: parsed.inquiry_id.clone(),
        owner_being: "astrid".to_string(),
        source_attestation_id: attestation.attestation_id.clone(),
        source_attestation_sha256: attestation_sha256,
        question: parsed.question,
        strands,
        owner_priority: parsed.owner_priority,
        fixed_analysis_set: owner_inquiry_fixed_analysis_set_v1(),
        budget: parsed.budget.clone(),
        dependency_inquiry_ids: parsed.dependency_inquiry_ids.clone(),
        status: OwnerInquiryStatusV1::Queued,
        cancellation: no_cancellation(),
        authority_boundary: OwnerInquiryAuthorityBoundaryV1::owner_only_offline(),
        created_at_unix_ms: now,
        updated_at_unix_ms: now,
    };
    if !inquiry.is_well_formed()
        || !inquiry
            .strands
            .iter()
            .all(|strand| strand.matches_response_bytes(response_text))
    {
        return Err(
            "INQUIRY_START produced an invalid or non-exact strand manifest; no work was queued"
                .to_string(),
        );
    }
    let mut prepared_research = research::prepare(
        &inquiry,
        attestation.response_sha256.clone(),
        context.intent_id.clone(),
        parsed.decision_plan.as_ref(),
        now,
    )?;
    write_manifest(&inquiry)?;
    research::persist_prepared(&mut prepared_research)?;
    write_runtime(&InquiryRuntimeV1 {
        schema: INQUIRY_RUNTIME_SCHEMA_V1.to_string(),
        inquiry_id: inquiry.inquiry_id.clone(),
        status: OwnerInquiryStatusV1::Queued,
        source_intent_id: context.intent_id.clone(),
        source_exchange_count: conv.exchange_count,
        receipt_path: None,
        cancellation: no_cancellation(),
        active_canary_id: None,
        failure: None,
        updated_at_unix_ms: now,
    })?;
    let activated = concern_queue::enqueue_owner_inquiry(
        inquiry.inquiry_id.clone(),
        format!("Distinct strands: {}", inquiry.question),
        inquiry.owner_priority,
        inquiry.source_attestation_id.clone(),
        context.intent_id,
        inquiry.dependency_inquiry_ids.clone(),
        inquiry.budget.clone(),
        now,
    )
    .inspect_err(|error| {
        let _ = write_runtime(&InquiryRuntimeV1 {
            schema: INQUIRY_RUNTIME_SCHEMA_V1.to_string(),
            inquiry_id: inquiry.inquiry_id.clone(),
            status: OwnerInquiryStatusV1::Failed,
            source_intent_id: String::new(),
            source_exchange_count: conv.exchange_count,
            receipt_path: None,
            cancellation: no_cancellation(),
            active_canary_id: None,
            failure: Some(error.clone()),
            updated_at_unix_ms: volition::now_unix_ms(),
        });
    })?;
    spawn_activated_jobs(&activated);
    let strand_summary = inquiry
        .strands
        .iter()
        .map(|strand| {
            format!(
                "{}[{}..{}] content={} embedding={}",
                strand.label,
                strand.response_start_byte,
                strand.response_end_byte,
                short_hash(&strand.content_sha256),
                short_hash(&strand.embedding_sha256)
            )
        })
        .collect::<Vec<_>>()
        .join("; ");
    Ok(format!(
        "Inquiry `{}` is {:?} with {} exact, separate strands: {}. The fixed offline analyses run without sensory admission, shadow influence, shared coupling, live codec writes, networking, sockets, ranking, or candidate merging.",
        inquiry.inquiry_id,
        concern_queue::owner_inquiry_status(&inquiry.inquiry_id)?
            .unwrap_or(BeingConcernStatusV1::Queued),
        inquiry.strands.len(),
        strand_summary
    ))
}

fn status_inquiry(original: &str, base_action: &str) -> Result<String, String> {
    let selector = action_payload_optional(original, base_action).unwrap_or("latest");
    let inquiry_id = resolve_inquiry_selector(selector)?;
    recover_active_jobs();
    let _ = reconcile_canaries();
    let inquiry = read_manifest(&inquiry_id)?;
    let runtime = read_runtime(&inquiry_id)?;
    let queue_status = concern_queue::owner_inquiry_status(&inquiry_id)?;
    let receipt = read_receipt_optional(&inquiry_id)?;
    if receipt
        .as_ref()
        .is_some_and(|receipt| !receipt.is_well_formed_for(&inquiry))
    {
        return Err("inquiry receipt failed integrity validation".to_string());
    }
    let owner_return = render_owner_return(&inquiry, receipt.as_ref());
    Ok(format!(
        "Inquiry `{inquiry_id}` runtime={:?} queue={queue_status:?} felt_status={}.\n{owner_return}",
        runtime.status,
        receipt
            .as_ref()
            .map_or("unreported", |receipt| match receipt.felt_status {
                InquiryFeltStatusV1::Unreported => "unreported",
                InquiryFeltStatusV1::Reported => "reported",
            }),
    ))
}

fn inspect_inquiry(original: &str, base_action: &str) -> Result<String, String> {
    let payload = action_payload(original, base_action)?;
    let mut parts = payload.split_whitespace();
    let selector = parts
        .next()
        .ok_or_else(|| "INQUIRY_INSPECT requires <id|latest> <section>".to_string())?;
    let section = parts.next().ok_or_else(|| {
        "INQUIRY_INSPECT requires evidence, controls, history, or receipts".to_string()
    })?;
    if parts.next().is_some() {
        return Err("INQUIRY_INSPECT accepts exactly one inquiry and one section".to_string());
    }
    let inquiry_id = resolve_inquiry_selector(selector)?;
    research::inspect(&inquiry_id, &section.to_ascii_lowercase())
}

fn cancel_inquiry(original: &str, base_action: &str) -> Result<String, String> {
    let context = volition::exact_self_owned_action_context(original)?;
    let inquiry_id = action_payload(original, base_action)?;
    validate_identifier(inquiry_id)?;
    let now = volition::now_unix_ms();
    let queue_status = concern_queue::owner_inquiry_status(inquiry_id)?
        .ok_or_else(|| format!("inquiry `{inquiry_id}` does not exist"))?;
    if matches!(
        queue_status,
        BeingConcernStatusV1::Completed | BeingConcernStatusV1::Cancelled
    ) {
        return Err(format!(
            "inquiry `{inquiry_id}` is already {queue_status:?}; use INQUIRY_WITHDRAW for an active canary"
        ));
    }
    let activated =
        concern_queue::transition_owner_inquiry(inquiry_id, BeingConcernStatusV1::Cancelled, now)?;
    let mut runtime = read_runtime(inquiry_id)?;
    runtime.status = OwnerInquiryStatusV1::Cancelled;
    let cancellation = OwnerInquiryCancellationV1 {
        requested: true,
        requested_at_unix_ms: Some(now),
        reason: Some(format!(
            "exact being-authored cancellation intent {}",
            context.intent_id
        )),
    };
    runtime.cancellation = cancellation.clone();
    runtime.updated_at_unix_ms = now;
    write_runtime(&runtime)?;
    let mut inquiry = read_manifest(inquiry_id)?;
    inquiry.status = OwnerInquiryStatusV1::Cancelled;
    inquiry.cancellation = cancellation.clone();
    inquiry.updated_at_unix_ms = now;
    write_manifest(&inquiry)?;
    if let Some(mut inquiry_v2) = research::read_manifest_v2_optional(inquiry_id)? {
        inquiry_v2.status = OwnerInquiryStatusV1::Cancelled;
        inquiry_v2.cancellation = cancellation;
        inquiry_v2.updated_at_unix_ms = now;
        research::write_manifest_v2(&inquiry_v2)?;
        research::mark_cancelled(inquiry_id, now)?;
    }
    spawn_activated_jobs(&activated);
    Ok(format!(
        "Inquiry `{inquiry_id}` cancellation was recorded from exact intent `{}`. Running analysis will be discarded on return; no live state was admitted or changed.",
        context.intent_id
    ))
}

fn spawn_activated_jobs(inquiry_ids: &[String]) {
    for inquiry_id in inquiry_ids {
        spawn_inquiry_job(inquiry_id.clone());
    }
}

fn spawn_inquiry_job(inquiry_id: String) {
    let active = ACTIVE_INQUIRY_JOBS.get_or_init(|| Mutex::new(HashSet::new()));
    let Ok(mut guard) = active.lock() else {
        return;
    };
    if !guard.insert(inquiry_id.clone()) {
        return;
    }
    drop(guard);
    let worker_inquiry_id = inquiry_id.clone();
    let spawn_result = thread::Builder::new()
        .name(format!("owner-inquiry-{inquiry_id}"))
        .spawn(move || {
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
        });
    if spawn_result.is_err()
        && let Ok(mut active) = ACTIVE_INQUIRY_JOBS
            .get_or_init(|| Mutex::new(HashSet::new()))
            .lock()
    {
        active.remove(&inquiry_id);
    }
}

fn run_inquiry_job(inquiry_id: &str) -> Result<Vec<String>, String> {
    let mut runtime = read_runtime(inquiry_id)?;
    if runtime.status == OwnerInquiryStatusV1::Cancelled {
        return Ok(Vec::new());
    }
    let mut inquiry = read_manifest(inquiry_id)?;
    let mut inquiry_v2 = research::read_manifest_v2_optional(inquiry_id)?;
    inquiry.status = OwnerInquiryStatusV1::Running;
    inquiry.updated_at_unix_ms = volition::now_unix_ms();
    if let Some(inquiry_v2) = inquiry_v2.as_mut() {
        inquiry_v2.status = OwnerInquiryStatusV1::Running;
        inquiry_v2.updated_at_unix_ms = inquiry.updated_at_unix_ms;
    }
    if !inquiry.is_well_formed()
        || inquiry_v2.as_ref().is_some_and(|inquiry_v2| {
            !inquiry_v2.is_well_formed() || !inquiry_v2.preserves_v1(&inquiry)
        })
    {
        return Err("inquiry became malformed before deterministic execution".to_string());
    }
    write_manifest(&inquiry)?;
    if let Some(inquiry_v2) = inquiry_v2.as_ref() {
        research::write_manifest_v2(inquiry_v2)?;
        research::mark_analyzing(inquiry_id, inquiry.updated_at_unix_ms)?;
    }
    runtime.status = OwnerInquiryStatusV1::Running;
    runtime.updated_at_unix_ms = inquiry.updated_at_unix_ms;
    write_runtime(&runtime)?;
    run_sandboxed_analyzer(&inquiry, inquiry_v2.as_ref())?;
    runtime = read_runtime(inquiry_id)?;
    if runtime.status == OwnerInquiryStatusV1::Cancelled {
        return Ok(Vec::new());
    }
    let receipt = read_receipt(inquiry_id)?;
    let receipt_v2 = inquiry_v2
        .as_ref()
        .map(|_| research::read_receipt_v2(inquiry_id))
        .transpose()?;
    if receipt.schema != OWNER_INQUIRY_RECEIPT_SCHEMA_V1
        || !receipt.is_well_formed_for(&inquiry)
        || receipt.analysis_receipts.iter().any(|analysis| {
            analysis.network_accessed
                || analysis.socket_accessed
                || analysis.private_source_accessed
                || analysis.candidate_merge_performed
                || analysis.live_runtime_mutation
                || !analysis.deterministic_rerun_match
        })
        || receipt_v2
            .as_ref()
            .zip(inquiry_v2.as_ref())
            .is_some_and(|(receipt_v2, inquiry_v2)| !receipt_v2.is_well_formed_for(inquiry_v2))
    {
        return Err(
            "deterministic inquiry receipt failed hash, isolation, pair, or no-merge validation"
                .to_string(),
        );
    }
    let lifecycle = receipt_v2
        .as_ref()
        .zip(inquiry_v2.as_ref())
        .map(|(receipt_v2, inquiry_v2)| {
            research::complete(inquiry_v2, receipt_v2, volition::now_unix_ms())
        })
        .transpose()?;
    runtime.status = OwnerInquiryStatusV1::Completed;
    runtime.receipt_path = Some(receipt_path(inquiry_id).display().to_string());
    runtime.updated_at_unix_ms = volition::now_unix_ms();
    write_runtime(&runtime)?;
    let activated = concern_queue::transition_owner_inquiry(
        inquiry_id,
        BeingConcernStatusV1::Completed,
        runtime.updated_at_unix_ms,
    )?;
    let _ = lifecycle;
    Ok(activated)
}

fn run_sandboxed_analyzer(
    inquiry: &OwnerInquiryV1,
    inquiry_v2: Option<&astrid_minime_protocol::OwnerInquiryV2>,
) -> Result<(), String> {
    let binary = minime_inquiry_binary()?;
    let sandbox = PathBuf::from("/usr/bin/sandbox-exec");
    if !sandbox.is_file() {
        return Err(
            "owner inquiry refused to run because the network/socket-denying sandbox is unavailable"
                .to_string(),
        );
    }
    let request = inquiry_v2.map_or_else(
        || manifest_path(&inquiry.inquiry_id),
        |_| {
            inquiry_root()
                .join("manifests-v2")
                .join(format!("{}.json", inquiry.inquiry_id))
        },
    );
    let output = inquiry_v2.map_or_else(
        || receipt_path(&inquiry.inquiry_id),
        |_| research::receipt_v2_path(&inquiry.inquiry_id),
    );
    if inquiry_v2.is_some_and(|inquiry_v2| {
        inquiry_v2.analysis_plan.iter().any(|entry| {
            entry.isolation.sandbox_profile_sha256 != exact_text_sha256(INQUIRY_SANDBOX_POLICY_V1)
        })
    }) {
        return Err(
            "V2 inquiry sandbox profile differs from the live denied-network policy".to_string(),
        );
    }
    let compute_budget = inquiry
        .budget
        .compute_millis
        .unwrap_or(DEFAULT_COMPUTE_BUDGET_MILLIS);
    if compute_budget > MAX_COMPUTE_BUDGET_MILLIS {
        return Err(format!(
            "inquiry compute budget {compute_budget} exceeds the disclosed {MAX_COMPUTE_BUDGET_MILLIS} ms ceiling"
        ));
    }
    let mut command = Command::new(&sandbox);
    command
        .arg("-p")
        .arg(INQUIRY_SANDBOX_POLICY_V1)
        .arg(&binary)
        .arg("inquiry")
        .arg("analyze")
        .arg("--request")
        .arg(&request)
        .arg("--output")
        .arg(&output);
    if inquiry_v2.is_some() {
        command
            .arg("--compatibility-output")
            .arg(receipt_path(&inquiry.inquiry_id));
    }
    let mut child = command
        .env_clear()
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| {
            format!(
                "start network-denied inquiry analyzer {}: {error}",
                binary.display()
            )
        })?;
    let started = Instant::now();
    loop {
        if child
            .try_wait()
            .map_err(|error| format!("poll inquiry analyzer: {error}"))?
            .is_some()
        {
            break;
        }
        if started.elapsed() > Duration::from_millis(compute_budget.saturating_add(2_000)) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(format!(
                "inquiry analyzer exceeded its {} ms compute budget",
                compute_budget
            ));
        }
        thread::sleep(Duration::from_millis(10));
    }
    let result = child
        .wait_with_output()
        .map_err(|error| format!("collect inquiry analyzer output: {error}"))?;
    if !result.status.success() {
        let diagnostic = String::from_utf8_lossy(&result.stderr);
        return Err(format!(
            "network-denied inquiry analyzer failed: {}",
            diagnostic.chars().take(800).collect::<String>()
        ));
    }
    let storage_budget = inquiry
        .budget
        .storage_bytes
        .unwrap_or(DEFAULT_STORAGE_BUDGET_BYTES);
    let output_len = fs::metadata(&output)
        .map_err(|error| format!("read inquiry receipt size {}: {error}", output.display()))?
        .len();
    if output_len > storage_budget {
        return Err(format!(
            "inquiry receipt requires {output_len} bytes, exceeding its {storage_budget}-byte storage budget"
        ));
    }
    if inquiry_v2.is_some() {
        let compatibility_len = fs::metadata(receipt_path(&inquiry.inquiry_id))
            .map_err(|error| format!("read compatibility inquiry receipt size: {error}"))?
            .len();
        if compatibility_len > storage_budget {
            return Err(format!(
                "compatibility inquiry receipt requires {compatibility_len} bytes, exceeding its {storage_budget}-byte storage budget"
            ));
        }
    }
    Ok(())
}

fn recover_active_jobs() {
    if let Ok(active) = concern_queue::active_owner_inquiry_ids() {
        spawn_activated_jobs(&active);
    }
}

fn validate_inquiry_budget(budget: &VolitionBudgetV1) -> Result<(), String> {
    let compute = budget
        .compute_millis
        .ok_or_else(|| "owner inquiry requires an explicit compute_millis budget".to_string())?;
    let storage = budget
        .storage_bytes
        .ok_or_else(|| "owner inquiry requires an explicit storage_bytes budget".to_string())?;
    if compute == 0 || compute > MAX_COMPUTE_BUDGET_MILLIS {
        return Err(format!(
            "compute_millis must be 1..={MAX_COMPUTE_BUDGET_MILLIS}; no budget was silently substituted"
        ));
    }
    if storage == 0 || storage > MAX_STORAGE_BUDGET_BYTES {
        return Err(format!(
            "storage_bytes must be 1..={MAX_STORAGE_BUDGET_BYTES}; no budget was silently substituted"
        ));
    }
    if budget.network_bytes.is_some() || budget.cost_microunits.is_some() {
        return Err(
            "offline owner inquiries do not accept network or cost budgets because those resources are denied"
                .to_string(),
        );
    }
    if budget.action_count.is_some_and(|count| count != 1) {
        return Err("owner inquiry action_count, when present, must be exactly 1".to_string());
    }
    Ok(())
}

fn write_manifest(inquiry: &OwnerInquiryV1) -> Result<(), String> {
    let _guard = record_guard()?;
    volition::write_owner_json(&manifest_path(&inquiry.inquiry_id), inquiry)
}

fn write_runtime(runtime: &InquiryRuntimeV1) -> Result<(), String> {
    let _guard = record_guard()?;
    volition::write_owner_json(&runtime_path(&runtime.inquiry_id), runtime)
}

fn write_receipt(receipt: &OwnerInquiryReceiptV1) -> Result<(), String> {
    let _guard = record_guard()?;
    volition::write_owner_json(&receipt_path(&receipt.inquiry_id), receipt)
}

fn read_manifest(inquiry_id: &str) -> Result<OwnerInquiryV1, String> {
    volition::read_json::<OwnerInquiryV1>(&manifest_path(inquiry_id))?
        .ok_or_else(|| format!("inquiry `{inquiry_id}` manifest is missing"))
}

fn read_runtime(inquiry_id: &str) -> Result<InquiryRuntimeV1, String> {
    volition::read_json::<InquiryRuntimeV1>(&runtime_path(inquiry_id))?
        .ok_or_else(|| format!("inquiry `{inquiry_id}` runtime record is missing"))
}

fn read_receipt(inquiry_id: &str) -> Result<OwnerInquiryReceiptV1, String> {
    read_receipt_optional(inquiry_id)?
        .ok_or_else(|| format!("inquiry `{inquiry_id}` receipt is not ready"))
}

fn read_receipt_optional(inquiry_id: &str) -> Result<Option<OwnerInquiryReceiptV1>, String> {
    volition::read_json::<OwnerInquiryReceiptV1>(&receipt_path(inquiry_id))
}

fn load_attestation(attestation_id: &str) -> Result<BeingUtteranceAttestationV1, String> {
    volition::read_json::<BeingUtteranceAttestationV1>(
        &volition::astrid_volition_root()
            .join("attestations")
            .join(format!("{attestation_id}.json")),
    )?
    .ok_or_else(|| format!("utterance attestation `{attestation_id}` is missing"))
}

fn resolve_inquiry_selector(selector: &str) -> Result<String, String> {
    if selector != "latest" {
        validate_identifier(selector)?;
        return Ok(selector.to_string());
    }
    let entries = fs::read_dir(inquiry_root().join("runtime"))
        .map_err(|error| format!("read inquiry runtime records: {error}"))?;
    entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            volition::read_json::<InquiryRuntimeV1>(&entry.path())
                .ok()
                .flatten()
        })
        .max_by_key(|runtime| runtime.updated_at_unix_ms)
        .map(|runtime| runtime.inquiry_id)
        .ok_or_else(|| "no owner inquiry exists".to_string())
}

fn render_owner_return(
    inquiry: &OwnerInquiryV1,
    receipt: Option<&OwnerInquiryReceiptV1>,
) -> String {
    let strands = inquiry
        .strands
        .iter()
        .map(|strand| {
            format!(
                "{} interval={}..{} content={} vector={}",
                strand.label,
                strand.response_start_byte,
                strand.response_end_byte,
                short_hash(&strand.content_sha256),
                short_hash(&strand.embedding_sha256)
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let Some(receipt) = receipt else {
        return format!(
            "Strands:\n{strands}\nFixed deterministic analyses are pending. No live lane or control has changed."
        );
    };
    let analyses = receipt
        .analysis_receipts
        .iter()
        .map(|analysis| {
            format!(
                "{:?}={}",
                analysis.analysis,
                short_hash(&analysis.output_sha256)
            )
        })
        .collect::<Vec<_>>()
        .join(", ");
    let source = receipt
        .analysis_receipts
        .iter()
        .find(|analysis| {
            analysis.analysis == OwnerInquiryAnalysisV1::ViscousPersistenceSourceSeparation
        })
        .and_then(|analysis| analysis.result.get("strands"))
        .and_then(Value::as_array)
        .map(|strands| {
            strands
                .iter()
                .map(|strand| {
                    let label = strand
                        .get("label")
                        .and_then(Value::as_str)
                        .unwrap_or("strand");
                    let baseline = strand.get("baseline").cloned().unwrap_or(Value::Null);
                    let compact =
                        serde_json::to_string(&baseline).unwrap_or_else(|_| "{}".to_string());
                    format!("{label}: {}", compact.chars().take(700).collect::<String>())
                })
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_else(|| "source-separation evidence pending".to_string());
    let codec_pairs = receipt
        .analysis_receipts
        .iter()
        .find(|analysis| analysis.analysis == OwnerInquiryAnalysisV1::CodecFidelity)
        .and_then(|analysis| analysis.result.get("pairs"))
        .and_then(Value::as_array)
        .map(|pairs| {
            pairs
                .iter()
                .map(render_codec_pair)
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_else(|| "codec pair evidence pending".to_string());
    let interference_pairs = receipt
        .analysis_receipts
        .iter()
        .find(|analysis| analysis.analysis == OwnerInquiryAnalysisV1::SensoryInterferenceAllPairs)
        .and_then(|analysis| analysis.result.get("pairs"))
        .and_then(Value::as_array)
        .map(|pairs| pairs.iter().map(render_pair).collect::<Vec<_>>().join("\n"))
        .unwrap_or_else(|| "interference pair evidence pending".to_string());
    format!(
        "Strands:\n{strands}\nAnalyses: {analyses}\nIndependent source baselines:\n{source}\nCodec pair preservation (unranked):\n{codec_pairs}\nSensory interference all pairs (unranked):\n{interference_pairs}\nUncertainty: these are deterministic machine measurements, not a claim about felt dominance, relief, or which strand should lead.\nOwner canary template: INQUIRY_CANARY {{\"inquiry_id\":\"{}\",\"duration_secs\":600,\"values\":{{\"semantic_strand_retention_turns\":4}}}}. Other exact local fields are conversation_temperature, response_token_limit, aperture, continuity_readout, vibrancy_aperture, and semantic_emission_gain. No template is selected for you; silence rolls back.",
        inquiry.inquiry_id
    )
}

fn render_codec_pair(pair: &Value) -> String {
    let left = pair
        .get("left_strand_id")
        .and_then(Value::as_str)
        .unwrap_or("left");
    let right = pair
        .get("right_strand_id")
        .and_then(Value::as_str)
        .unwrap_or("right");
    let source = pair
        .get("source_distance")
        .and_then(Value::as_f64)
        .map_or_else(|| "unknown".to_string(), |value| value.to_string());
    let companion = pair
        .get("companion_distance")
        .and_then(Value::as_f64)
        .map_or_else(|| "unknown".to_string(), |value| value.to_string());
    let preservation = pair
        .get("pairwise_distance_preservation_ratio")
        .and_then(Value::as_f64)
        .map_or_else(|| "unknown".to_string(), |value| value.to_string());
    format!(
        "{left} <> {right}: 48D_distance={source} 12D_distance={companion} preservation={preservation}"
    )
}

fn render_pair(pair: &Value) -> String {
    let left = pair
        .get("left_label")
        .and_then(Value::as_str)
        .unwrap_or("left");
    let right = pair
        .get("right_label")
        .and_then(Value::as_str)
        .unwrap_or("right");
    let review = pair.get("review").cloned().unwrap_or(Value::Null);
    let compact = serde_json::to_string(&review).unwrap_or_else(|_| "{}".to_string());
    format!(
        "{left} <> {right}: {}",
        compact.chars().take(900).collect::<String>()
    )
}

fn minime_inquiry_binary() -> Result<PathBuf, String> {
    if let Some(path) = std::env::var_os("MINIME_INQUIRY_BIN").map(PathBuf::from) {
        if path.is_file() {
            return Ok(path);
        }
        return Err(format!(
            "MINIME_INQUIRY_BIN does not name a file: {}",
            path.display()
        ));
    }
    let root = crate::paths::bridge_paths().minime_root();
    for relative in ["minime/target/release/minime", "minime/target/debug/minime"] {
        let candidate = root.join(relative);
        if candidate.is_file() {
            return Ok(candidate);
        }
    }
    Err(format!(
        "Minime inquiry runner is not built under {}; build it before starting an inquiry",
        root.display()
    ))
}

fn inquiry_root() -> PathBuf {
    volition::astrid_volition_root().join("inquiries")
}

fn manifest_path(inquiry_id: &str) -> PathBuf {
    inquiry_root()
        .join("manifests")
        .join(format!("{inquiry_id}.json"))
}

fn runtime_path(inquiry_id: &str) -> PathBuf {
    inquiry_root()
        .join("runtime")
        .join(format!("{inquiry_id}.json"))
}

fn receipt_path(inquiry_id: &str) -> PathBuf {
    inquiry_root()
        .join("receipts")
        .join(format!("{inquiry_id}.json"))
}

fn canary_path(canary_id: &str) -> PathBuf {
    inquiry_root()
        .join("canaries")
        .join(format!("{canary_id}.json"))
}

fn self_control_root() -> PathBuf {
    crate::paths::bridge_paths()
        .bridge_workspace()
        .join("self_control_v2/astrid")
}

fn generated_inquiry_id(intent_id: &str) -> String {
    format!("inquiry-{}", short_hash(&exact_text_sha256(intent_id)))
}

fn default_inquiry_budget() -> VolitionBudgetV1 {
    VolitionBudgetV1 {
        compute_millis: Some(DEFAULT_COMPUTE_BUDGET_MILLIS),
        network_bytes: None,
        storage_bytes: Some(DEFAULT_STORAGE_BUDGET_BYTES),
        cost_microunits: None,
        action_count: Some(1),
    }
}

fn no_cancellation() -> OwnerInquiryCancellationV1 {
    OwnerInquiryCancellationV1 {
        requested: false,
        requested_at_unix_ms: None,
        reason: None,
    }
}

fn validate_identifier(value: &str) -> Result<(), String> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    {
        return Err(
            "inquiry identifiers must be 1..=128 ASCII letters, digits, dot, dash, or underscore"
                .to_string(),
        );
    }
    Ok(())
}

fn action_payload<'a>(original: &'a str, base_action: &str) -> Result<&'a str, String> {
    action_payload_optional(original, base_action)
        .ok_or_else(|| format!("{base_action} requires an argument"))
}

fn action_payload_optional<'a>(original: &'a str, base_action: &str) -> Option<&'a str> {
    original
        .get(base_action.len()..)
        .map(|value| value.trim_matches([' ', ':', '-']).trim())
        .filter(|value| !value.is_empty())
}

fn exact_text_sha256(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))
}

fn exact_value_sha256(value: &Value) -> String {
    let mut canonical = value.clone();
    canonicalize_json(&mut canonical);
    let encoded = serde_json::to_vec(&canonical).unwrap_or_default();
    format!("{:x}", Sha256::digest(encoded))
}

fn canonicalize_json(value: &mut Value) {
    match value {
        Value::Object(fields) => {
            let mut ordered = fields
                .iter_mut()
                .map(|(key, child)| {
                    canonicalize_json(child);
                    (key.clone(), child.take())
                })
                .collect::<Vec<_>>();
            ordered.sort_by(|left, right| left.0.cmp(&right.0));
            fields.clear();
            fields.extend(ordered);
        },
        Value::Array(values) => {
            for child in values {
                canonicalize_json(child);
            }
        },
        _ => {},
    }
}

fn short_hash(value: &str) -> &str {
    value.get(..12).unwrap_or(value)
}

fn record_guard() -> Result<std::sync::MutexGuard<'static, ()>, String> {
    INQUIRY_RECORD_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .map_err(|_| "owner inquiry record lock was poisoned".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inquiry_budget_is_explicit_bounded_and_offline() {
        assert!(validate_inquiry_budget(&default_inquiry_budget()).is_ok());
        let mut budget = default_inquiry_budget();
        budget.compute_millis = Some(MAX_COMPUTE_BUDGET_MILLIS.saturating_add(1));
        assert!(validate_inquiry_budget(&budget).is_err());
        budget = default_inquiry_budget();
        budget.network_bytes = Some(1);
        assert!(validate_inquiry_budget(&budget).is_err());
    }

    fn runtime_for_visibility(
        status: OwnerInquiryStatusV1,
        source_exchange_count: u64,
    ) -> InquiryRuntimeV1 {
        InquiryRuntimeV1 {
            schema: INQUIRY_RUNTIME_SCHEMA_V1.to_string(),
            inquiry_id: "inquiry-visibility".to_string(),
            status,
            source_intent_id: "intent-visibility".to_string(),
            source_exchange_count,
            receipt_path: None,
            cancellation: no_cancellation(),
            active_canary_id: None,
            failure: None,
            updated_at_unix_ms: 1,
        }
    }

    #[test]
    fn active_inquiry_remains_visible_when_retention_is_zero() {
        let mut conv = ConversationState::new(Vec::new(), None);
        conv.exchange_count = 12;
        conv.semantic_strand_retention_turns = 0;
        assert!(inquiry_sidecar_visible(
            &runtime_for_visibility(OwnerInquiryStatusV1::Running, 1),
            &conv
        ));

        conv.semantic_strand_retention_turns = 2;
        assert!(inquiry_sidecar_visible(
            &runtime_for_visibility(OwnerInquiryStatusV1::CanaryActive, 1),
            &conv
        ));
    }

    #[test]
    fn completed_inquiry_obeys_exact_owner_retention_turns() {
        let mut conv = ConversationState::new(Vec::new(), None);
        conv.exchange_count = 12;
        conv.semantic_strand_retention_turns = 0;
        let runtime = runtime_for_visibility(OwnerInquiryStatusV1::Completed, 10);
        assert!(!inquiry_sidecar_visible(&runtime, &conv));

        conv.semantic_strand_retention_turns = 2;
        assert!(inquiry_sidecar_visible(&runtime, &conv));

        conv.exchange_count = 13;
        assert!(!inquiry_sidecar_visible(&runtime, &conv));
    }
}
