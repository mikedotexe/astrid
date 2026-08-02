use super::*;

fn set_owner(path: &Path, directory: bool) {
    #[cfg(unix)]
    fs::set_permissions(
        path,
        fs::Permissions::from_mode(if directory { 0o700 } else { 0o600 }),
    )
    .expect("owner permissions");
    let _ = directory;
}

fn write_fixture(path: &Path, bytes: &[u8]) {
    fs::create_dir_all(path.parent().expect("parent")).expect("create fixture parent");
    set_owner(path.parent().expect("parent"), true);
    fs::write(path, bytes).expect("write fixture");
    set_owner(path, false);
}

fn authority_state() -> serde_json::Value {
    serde_json::json!({
        "schema": "artifact_authority_state_v1",
        "schema_version": 1,
        "state": "evidence_only",
        "witness_only": true
    })
}

fn projected_fixture(
    workspace: &Path,
    card_count: usize,
) -> (String, PathBuf, String, Vec<String>) {
    let label = "astrid:llm".to_string();
    let source_path = PathBuf::from(
        "/private/fixture/capsules/spectral-bridge/src/llm/provider/dialogue_runtime.rs",
    );
    let locator = source_locator(&source_path).expect("locator");
    let stable = stable_source_identity(&label, locator.as_deref());
    let source_sha = "e".repeat(64);
    let session_id = "f".repeat(64);
    let mut index_entries = Vec::new();
    let mut card_ids = Vec::new();
    for index in 0..card_count {
        let digit = char::from_digit(u32::try_from(index + 1).expect("small index"), 16)
            .expect("hex digit");
        let card_id = format!("icv1_{}", digit.to_string().repeat(64));
        let introspection_id = format!("introspection_fixture_{}", 100 + index);
        let report_sha = "a".repeat(64);
        let card = serde_json::json!({
            "schema": CARD_SCHEMA,
            "schema_version": 1,
            "card_id": card_id,
            "introspection_id": introspection_id,
            "captured_at_unix": 100 + index,
            "canonical_report": {
                "path": format!("introspections/prior_{index}.txt"),
                "sha256": report_sha,
                "lived_state_witness_id": null
            },
            "source": {
                "label": label,
                "stable_identity": stable,
                "locator": locator,
                "sha256": source_sha,
                "read_session_id": session_id
            },
            "claims": [{
                "claim_id": format!("claim_{index}"),
                "summary": format!("bounded summary {index}"),
                "classification": "verified_source",
                "disposition": "addressed_change",
                "grounded_disposition": "verified by bounded source and tests",
                "evidence_refs": [{
                    "kind": "test",
                    "target": "scripts/test_introspection_continuity.py"
                }],
                "omitted_evidence_ref_count": 0,
                "authority_wait": null
            }],
            "claim_count": 1,
            "remaining_gaps": [],
            "authority_waits": [],
            "right_to_ignore": true,
            "silence_is_neutral": true,
            "mechanical_evidence_only": true,
            "felt_closure_inferred": false,
            "consent_inferred": false,
            "no_authority": true,
            "authority_boundary": "mechanical_evidence_not_felt_closure_control_approval_or_activation",
            "artifact_authority_state_v1": authority_state()
        });
        let mut card_raw = serde_json::to_vec_pretty(&card).expect("card JSON");
        card_raw.push(b'\n');
        let card_relative = format!("{STATE_RELATIVE}/cards/{card_id}.json");
        write_fixture(&workspace.join(&card_relative), &card_raw);
        index_entries.push(serde_json::json!({
            "card_id": card_id,
            "card_path": card_relative,
            "card_sha256": sha256_bytes(&card_raw),
            "introspection_id": introspection_id,
            "captured_at_unix": 100 + index,
            "report_sha256": report_sha,
            "stable_source_identity": stable,
            "source_sha256": source_sha,
            "read_session_id": session_id
        }));
        card_ids.push(card_id);
    }
    let index = serde_json::json!({
        "schema": INDEX_SCHEMA,
        "schema_version": 1,
        "cards": index_entries,
        "card_count": card_count,
        "right_to_ignore": true,
        "silence_is_neutral": true,
        "mechanical_evidence_only": true,
        "artifact_authority_state_v1": authority_state()
    });
    let mut index_raw = serde_json::to_vec_pretty(&index).expect("index JSON");
    index_raw.push(b'\n');
    write_fixture(
        &workspace.join(STATE_RELATIVE).join("index.json"),
        &index_raw,
    );
    (label, source_path, source_sha, card_ids)
}

#[test]
fn loader_selects_latest_three_exact_source_cards() {
    let temp = tempfile::tempdir().expect("tempdir");
    let (label, source_path, source_sha, cards) = projected_fixture(temp.path(), 4);
    let context = load_prior_evidence_v1(temp.path(), &label, &source_path, &source_sha)
        .expect("load cards")
        .expect("matching cards");

    assert_eq!(context.bindings.len(), 3);
    assert!(!context.prompt_context.contains(&cards[0]));
    for card_id in &cards[1..] {
        assert!(context.prompt_context.contains(card_id));
    }
    assert!(context.prompt_context.contains("Right to ignore: yes"));
    assert!(context.prompt_context.contains("Silence is neutral"));
    assert!(context.prompt_context.contains("not source bytes"));
}

#[test]
fn loader_rejects_tampered_card_bytes() {
    let temp = tempfile::tempdir().expect("tempdir");
    let (label, source_path, source_sha, cards) = projected_fixture(temp.path(), 1);
    let card_path = temp
        .path()
        .join(STATE_RELATIVE)
        .join("cards")
        .join(format!("{}.json", cards[0]));
    fs::write(&card_path, b"{}\n").expect("tamper card");
    set_owner(&card_path, false);

    let error = load_prior_evidence_v1(temp.path(), &label, &source_path, &source_sha)
        .expect_err("tampered card must fail closed");
    assert!(error.contains("hash mismatch"), "{error}");
}

#[test]
fn response_writer_accepts_only_exact_lines_and_never_copies_prose() {
    let temp = tempfile::tempdir().expect("tempdir");
    let (label, source_path, source_sha, cards) = projected_fixture(temp.path(), 1);
    let context = load_prior_evidence_v1(temp.path(), &label, &source_path, &source_sha)
        .expect("load cards")
        .expect("matching card");
    let artifact = temp
        .path()
        .join("introspections/introspection_current_900.txt");
    let unknown = format!("icv1_{}", "9".repeat(64));
    let response = format!(
        "Observed:\nprivate felt prose\nPrior Evidence: {} :: still_friction\n Prior Evidence: {} :: contradicted\nPrior Evidence: {} :: contradicted\nPrior Evidence: {unknown} :: not_assessed\n",
        cards[0], cards[0], cards[0]
    );
    write_fixture(
        &artifact,
        format!("private report prose must stay here\n{response}").as_bytes(),
    );

    let written =
        record_prior_evidence_responses_v1(temp.path(), &artifact, &response, &context)
            .expect("write receipts");
    assert_eq!(written.len(), 2);
    let receipts = written
        .iter()
        .map(|path| {
            let raw = fs::read_to_string(path).expect("receipt text");
            assert!(!raw.contains("private felt prose"));
            assert!(!raw.contains("private report prose"));
            serde_json::from_str::<serde_json::Value>(&raw).expect("receipt JSON")
        })
        .collect::<Vec<_>>();
    let bound = receipts
        .iter()
        .find(|receipt| receipt["card_id"] == cards[0])
        .expect("bound receipt");
    assert_eq!(bound["assessment_status"], "still_friction");
    assert_eq!(bound["bound"], true);
    assert_eq!(bound["linked_claim_ids"], serde_json::json!(["claim_0"]));
    assert_eq!(
        receipts
            .iter()
            .filter(|receipt| receipt["card_id"] == cards[0])
            .count(),
        1
    );
    let unbound = receipts
        .iter()
        .find(|receipt| receipt["card_id"] == unknown)
        .expect("unbound receipt");
    assert_eq!(unbound["bound"], false);
    assert_eq!(unbound["linked_claim_ids"], serde_json::json!([]));
}
