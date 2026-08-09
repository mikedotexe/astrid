use std::fs;
use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::attestation::HmacSigner;
use crate::config::Config;
use crate::context_provenance::ContextProvenance;
use crate::inquiry::{PersistInput, SEGMENT_BYTES, classify, persist};
use crate::util::sha256;

struct Fixture {
    _temporary: tempfile::TempDir,
    config: Config,
    key_path: PathBuf,
    reflection_path: PathBuf,
    response: String,
}

fn fixture() -> Fixture {
    let temporary = tempfile::tempdir_in(std::env::current_dir().unwrap()).unwrap();
    let root = temporary.path();
    let workspace = root.join("workspace");
    let state = root.join("state");
    let projection = workspace.join("runtime/scheduled-introspection/projection");
    let reflections = workspace.join("introspections/scheduled");
    for (path, mode) in [(&state, 0o700), (&projection, 0o750), (&reflections, 0o750)] {
        fs::create_dir_all(path).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(mode)).unwrap();
    }
    let key_path = root.join("intent.key");
    fs::write(&key_path, [b'a'; 32]).unwrap();
    fs::set_permissions(&key_path, fs::Permissions::from_mode(0o600)).unwrap();
    let response = format!(
        "A bounded reflection.\nINQUIRY_STEP: {}\nSOURCE_REVIEW: NONE",
        serde_json::json!({
            "schema": "astrid.edge.inquiry.step.v1",
            "thread_operation": "open",
            "thread_id": "thread-ledger-test",
            "parent_step_id": null,
            "observation": "A recurring boundary appears in the evidence.",
            "interpretation": "The boundary may constrain inquiry depth.",
            "uncertainty": "The sample is intentionally small.",
            "decision": "Retain the question and gather another sample.",
            "counterpoint": "A scheduler cadence may explain the pattern.",
            "next_test": "Compare another independent window.",
            "evidence_ids": ["evidence-ledger-1"],
            "confidence": "tentative",
            "belief_operation": "propose",
            "belief_id": "belief-ledger-1",
            "belief_claim": "The observed boundary may shape inquiry depth."
        })
    );
    let reflection_path = reflections.join("reflection.md");
    fs::write(&reflection_path, &response).unwrap();
    fs::set_permissions(&reflection_path, fs::Permissions::from_mode(0o640)).unwrap();
    let config: Config = serde_json::from_value(serde_json::json!({
        "schema": crate::CONFIG_SCHEMA,
        "appliance_id": "avado-ledger-test",
        "target": "x86_64-unknown-linux-gnu",
        "model": "test-model",
        "ollama_origin": "http://127.0.0.1:11434",
        "connect_timeout_ms": 1000,
        "header_timeout_ms": 1000,
        "total_timeout_ms": 2000,
        "provider_broker": null,
        "web_broker": null,
        "context_tokens": 1024,
        "output_tokens": 64,
        "source_authoring_output_tokens": 64,
        "model_lock": root.join("model.lock"),
        "workspace_root": workspace,
        "workspace_uid": nix::unistd::geteuid().as_raw(),
        "workspace_gid": nix::unistd::getegid().as_raw(),
        "source_root": root.join("source"),
        "source_manifest": root.join("source/MANIFEST.json"),
        "source_manifest_sha256": "a".repeat(64),
        "source_signature": root.join("source/MANIFEST.signature.json"),
        "expected_source_id": format!("cpu-edge:{}", "b".repeat(64)),
        "source_signing_key": root.join("source.key"),
        "source_signing_key_sha256": "c".repeat(64),
        "attestor_key": key_path,
        "attestor_key_sha256": sha256(&[b'a'; 32]),
        "state_root": state,
        "inquiry_history_root": root.join("candidate/inquiry-history"),
        "supervisor_inbox": root.join("inbox"),
        "supervisor_status": root.join("status"),
        "current_generation": root.join("supervisor/current-generation"),
        "active_generation_link": root.join("appliance/current"),
        "maintenance_lease": root.join("supervisor/maintenance.json"),
        "patch_export_root": root.join("workspace/self-change/patch-outbox"),
        "owned_inputs": [],
        "gates": {
            "autonomy_state": root.join("workspace/autonomy.json"),
            "action_receipts": root.join("workspace/actions.jsonl"),
            "thermal_celsius": root.join("thermal"),
            "maximum_thermal_celsius": 90
        }
    }))
    .unwrap();
    Fixture {
        _temporary: temporary,
        config,
        key_path,
        reflection_path,
        response,
    }
}

fn persist_for(
    fixture: &Fixture,
    appliance_id: &str,
    trace_id: &str,
    turn_id: &str,
    trigger_nonce: &str,
    recorded_at_unix_ms: u64,
) -> crate::Result<crate::inquiry::ProjectionReceipt> {
    let signer = HmacSigner::from_file(&fixture.key_path)?;
    let classification = classify(&fixture.response);
    let inquiry = classification
        .structured()
        .expect("fixture has an exact inquiry declaration");
    let response_sha256 = sha256(fixture.response.as_bytes());
    let context = ContextProvenance::clean();
    persist(
        &fixture.config,
        &signer,
        &PersistInput {
            appliance_id,
            trigger_kind: "scheduled",
            due_nonce: "due-ledger-test",
            trigger_nonce,
            recorded_at_unix_ms,
            trace_id,
            session_id: "session-ledger-test",
            turn_id,
            span_id: "span-ledger-test",
            prompt_sha256: &"a".repeat(64),
            response_sha256: &response_sha256,
            context_provenance: &context,
            reflection_path: &fixture.reflection_path,
            reflection_sha256: &response_sha256,
            inquiry,
        },
    )
}

fn segment_path(fixture: &Fixture) -> PathBuf {
    numbered_segment_path(fixture, 1)
}

fn numbered_segment_path(fixture: &Fixture, segment: u64) -> PathBuf {
    fixture
        .config
        .inquiry_history_root
        .join(format!("segments/segment-{segment:020}.jsonl"))
}

fn nonempty_lines(path: &Path) -> Vec<Value> {
    fs::read(path)
        .unwrap()
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
        .map(|line| serde_json::from_slice(line).unwrap())
        .collect()
}

#[test]
fn signed_history_replay_is_idempotent_and_group_read_only() {
    let fixture = fixture();
    let first = persist_for(
        &fixture,
        "avado-ledger-test",
        "trace-ledger-a",
        "turn-ledger-a",
        "trigger-ledger-a",
        1,
    )
    .unwrap();
    let replay = persist_for(
        &fixture,
        "avado-ledger-test",
        "trace-ledger-a",
        "turn-ledger-a",
        "trigger-ledger-a",
        1,
    )
    .unwrap();
    assert_eq!(first.signed_entry_id, replay.signed_entry_id);
    assert_eq!(nonempty_lines(&segment_path(&fixture)).len(), 1);
    for directory in [
        fixture.config.inquiry_history_root.clone(),
        fixture.config.inquiry_history_root.join("segments"),
    ] {
        let metadata = fs::symlink_metadata(directory).unwrap();
        assert_eq!(metadata.permissions().mode() & 0o777, 0o750);
        assert_eq!(metadata.gid(), fixture.config.workspace_gid);
    }
    for file in [
        segment_path(&fixture),
        fixture.config.inquiry_history_root.join("head.json"),
        fixture.config.inquiry_history_root.join("ledger.lock"),
    ] {
        let metadata = fs::symlink_metadata(file).unwrap();
        assert_eq!(metadata.permissions().mode() & 0o777, 0o640);
        assert_eq!(metadata.gid(), fixture.config.workspace_gid);
    }
}

#[test]
fn same_prose_under_distinct_traces_creates_distinct_chained_entries() {
    let fixture = fixture();
    let first = persist_for(
        &fixture,
        "avado-ledger-test",
        "trace-ledger-a",
        "turn-ledger-a",
        "trigger-ledger-a",
        1,
    )
    .unwrap();
    let second = persist_for(
        &fixture,
        "avado-ledger-test",
        "trace-ledger-b",
        "turn-ledger-b",
        "trigger-ledger-b",
        2,
    )
    .unwrap();
    assert_ne!(first.signed_entry_id, second.signed_entry_id);
    let lines = nonempty_lines(&segment_path(&fixture));
    assert_eq!(lines.len(), 2);
    assert_eq!(
        lines[1]["core"]["mechanical_predecessor"],
        first.signed_entry_id
    );
    assert_eq!(
        lines[1]["core"]["prior_entry_sha256"],
        sha256(&crate::util::canonical_json(&lines[0]).unwrap())
    );
}

#[test]
fn four_mibibyte_rollover_preserves_chain_and_old_segment_torn_tail_fails_closed() {
    let fixture = fixture();
    let first = persist_for(
        &fixture,
        "avado-ledger-test",
        "trace-rollover-a",
        "turn-rollover-a",
        "trigger-rollover-a",
        1,
    )
    .unwrap();
    let first_segment = numbered_segment_path(&fixture, 1);
    let mut bytes = fs::read(&first_segment).unwrap();
    // Empty JSONL records are ignored by the exact scanner. Padding a valid
    // fixture with newline-only records lets this test exercise the production
    // four-MiB boundary without thousands of quadratic persistence calls.
    bytes.resize(usize::try_from(SEGMENT_BYTES).unwrap(), b'\n');
    fs::write(&first_segment, bytes).unwrap();
    fs::remove_file(fixture.config.inquiry_history_root.join("head.json")).unwrap();

    let second = persist_for(
        &fixture,
        "avado-ledger-test",
        "trace-rollover-b",
        "turn-rollover-b",
        "trigger-rollover-b",
        2,
    )
    .unwrap();
    assert_ne!(first.signed_entry_id, second.signed_entry_id);
    assert_eq!(fs::metadata(&first_segment).unwrap().len(), SEGMENT_BYTES);
    let second_segment = numbered_segment_path(&fixture, 2);
    let second_lines = nonempty_lines(&second_segment);
    assert_eq!(second_lines.len(), 1);
    assert_eq!(
        second_lines[0]["core"]["mechanical_predecessor"],
        first.signed_entry_id
    );
    let head: Value = serde_json::from_slice(
        &fs::read(fixture.config.inquiry_history_root.join("head.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(head["core"]["segment"], 2);
    assert_eq!(head["core"]["entry_index"], 1);

    let mut torn = fs::read(&first_segment).unwrap();
    *torn.last_mut().unwrap() = b'{';
    fs::write(&first_segment, torn).unwrap();
    let error = persist_for(
        &fixture,
        "avado-ledger-test",
        "trace-rollover-c",
        "turn-rollover-c",
        "trigger-rollover-c",
        3,
    )
    .unwrap_err();
    assert!(error.to_string().contains("torn tail"));
}

#[test]
fn signed_history_rejects_tamper_torn_tail_and_cross_appliance_replay() {
    let tampered = fixture();
    persist_for(
        &tampered,
        "avado-ledger-test",
        "trace-ledger-a",
        "turn-ledger-a",
        "trigger-ledger-a",
        1,
    )
    .unwrap();
    let segment = segment_path(&tampered);
    let mut bytes = fs::read(&segment).unwrap();
    let index = bytes
        .windows(b"recurring".len())
        .position(|window| window == b"recurring")
        .unwrap();
    bytes[index] = b'R';
    fs::write(&segment, bytes).unwrap();
    assert!(
        persist_for(
            &tampered,
            "avado-ledger-test",
            "trace-ledger-b",
            "turn-ledger-b",
            "trigger-ledger-b",
            2,
        )
        .is_err()
    );

    let torn = fixture();
    persist_for(
        &torn,
        "avado-ledger-test",
        "trace-ledger-a",
        "turn-ledger-a",
        "trigger-ledger-a",
        1,
    )
    .unwrap();
    let segment = segment_path(&torn);
    let mut bytes = fs::read(&segment).unwrap();
    bytes.push(b'{');
    fs::write(&segment, bytes).unwrap();
    assert!(
        persist_for(
            &torn,
            "avado-ledger-test",
            "trace-ledger-b",
            "turn-ledger-b",
            "trigger-ledger-b",
            2,
        )
        .is_err()
    );

    let cross = fixture();
    persist_for(
        &cross,
        "avado-ledger-test",
        "trace-ledger-a",
        "turn-ledger-a",
        "trigger-ledger-a",
        1,
    )
    .unwrap();
    let mut cross_config = cross.config.clone();
    cross_config.appliance_id = "icp-ledger-test".to_owned();
    let signer = HmacSigner::from_file(&cross.key_path).unwrap();
    let classification = classify(&cross.response);
    let inquiry = classification.structured().unwrap();
    let response_sha256 = sha256(cross.response.as_bytes());
    let context = ContextProvenance::clean();
    assert!(
        persist(
            &cross_config,
            &signer,
            &PersistInput {
                appliance_id: "icp-ledger-test",
                trigger_kind: "scheduled",
                due_nonce: "due-ledger-test",
                trigger_nonce: "trigger-ledger-b",
                recorded_at_unix_ms: 2,
                trace_id: "trace-ledger-b",
                session_id: "session-ledger-test",
                turn_id: "turn-ledger-b",
                span_id: "span-ledger-test",
                prompt_sha256: &"a".repeat(64),
                response_sha256: &response_sha256,
                context_provenance: &context,
                reflection_path: &cross.reflection_path,
                reflection_sha256: &response_sha256,
                inquiry,
            },
        )
        .is_err()
    );
}

#[test]
fn complete_entry_without_head_recovers_without_duplicate_append() {
    let fixture = fixture();
    let first = persist_for(
        &fixture,
        "avado-ledger-test",
        "trace-ledger-a",
        "turn-ledger-a",
        "trigger-ledger-a",
        1,
    )
    .unwrap();
    fs::remove_file(fixture.config.inquiry_history_root.join("head.json")).unwrap();
    let recovered = persist_for(
        &fixture,
        "avado-ledger-test",
        "trace-ledger-a",
        "turn-ledger-a",
        "trigger-ledger-a",
        1,
    )
    .unwrap();
    assert_eq!(first.signed_entry_id, recovered.signed_entry_id);
    assert_eq!(nonempty_lines(&segment_path(&fixture)).len(), 1);
    assert!(
        fixture
            .config
            .inquiry_history_root
            .join("head.json")
            .is_file()
    );
}

#[test]
fn concurrent_distinct_steps_serialize_into_one_valid_chain() {
    let fixture = std::sync::Arc::new(fixture());
    let first_fixture = std::sync::Arc::clone(&fixture);
    let first = std::thread::spawn(move || {
        persist_for(
            &first_fixture,
            "avado-ledger-test",
            "trace-ledger-a",
            "turn-ledger-a",
            "trigger-ledger-a",
            1,
        )
    });
    let second_fixture = std::sync::Arc::clone(&fixture);
    let second = std::thread::spawn(move || {
        persist_for(
            &second_fixture,
            "avado-ledger-test",
            "trace-ledger-b",
            "turn-ledger-b",
            "trigger-ledger-b",
            2,
        )
    });
    first.join().unwrap().unwrap();
    second.join().unwrap().unwrap();
    let lines = nonempty_lines(&segment_path(&fixture));
    assert_eq!(lines.len(), 2);
    assert_eq!(
        lines[1]["core"]["prior_entry_sha256"],
        sha256(&crate::util::canonical_json(&lines[0]).unwrap())
    );
}
