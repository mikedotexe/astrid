//! Offline matched-input contrast. No production recall policy or delivery is changed.
use astrid_source_study::{Catalog, InputKind, Reader, StudyOutput};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fmt::Write as _, fs};

const SOURCE: &str = "astrid/crates/example/src/lib.rs";
const OPEN: &str = "SELF_STUDY OPEN astrid/crates/example/src/lib.rs 1";
const HEADER: &str = "\n\nRECALLED ACCOUNT \u{2014}";
const FOOTER: &str = "\nEnd of study notebook.\n";

fn digest(bytes: impl AsRef<[u8]>) -> String {
    format!("{:x}", Sha256::digest(bytes.as_ref()))
}

// This operates only on a copy of an already prepared synthetic input. It is
// deliberately not a reader option, provider adapter, or stored-state migration.
fn latest_account_only(input: &StudyOutput) -> StudyOutput {
    let mut selected = input.clone();
    let Some(start) = input.text.find(HEADER) else {
        return selected;
    };
    let after_header = start.checked_add(HEADER.len()).unwrap();
    let body_start = after_header
        .checked_add(input.text[after_header..].find('\n').unwrap())
        .and_then(|offset| offset.checked_add(1))
        .unwrap();
    let body_end = body_start
        .checked_add(input.text[body_start..].find(FOOTER).unwrap())
        .unwrap();
    let original_body = &input.text[body_start..body_end];
    let mut notebook: Value = serde_json::from_str(original_body).unwrap();
    let expected_fields = notebook.as_object().unwrap().clone();
    notebook["recent"] = json!([]);
    for (key, value) in expected_fields {
        if key != "recent" {
            assert_eq!(notebook[&key], value, "unselected field {key}");
        }
    }
    selected
        .text
        .replace_range(body_start..body_end, &notebook.to_string());
    assert_eq!(&selected.text[..body_start], &input.text[..body_start]);
    assert!(selected.text.ends_with(&input.text[body_end..]));
    let mut restored = serde_json::to_value(&selected).unwrap();
    restored["text"] = input.text.clone().into();
    assert_eq!(restored, serde_json::to_value(input).unwrap());
    selected
}

fn deliver(reader: &Reader, input: &StudyOutput, prose: &str) {
    reader
        .delivered(
            &input.page.as_ref().unwrap().id,
            &json!({"messages":[{"role":"user","content":input.text}]}).to_string(),
            &json!({"message":{"content":prose},"done":true}).to_string(),
        )
        .unwrap();
}

fn synthetic_account(index: usize, fixture: &str) -> String {
    let mut prose = format!(
        "ACCOUNT_{index}\n{}\nNEXT: REST",
        "Synthetic interpretation, explicitly unverified. ".repeat(40)
    );
    if index == 0 && matches!(fixture, "older_counterexample" | "saved_counterexample") {
        prose.insert_str(
            0,
            "OLDER_COUNTEREXAMPLE: the observed synthetic failure continued.\n",
        );
        if fixture == "saved_counterexample" {
            write!(
                prose,
                "\nSTUDY_FINDING: {SOURCE}:1 | OLDER_COUNTEREXAMPLE: retain this contrary observation."
            )
            .unwrap();
        }
    }
    if index == 3 && fixture == "latest_qualification" {
        prose.insert_str(
            0,
            "LATEST_QUALIFICATION: I withdraw my earlier enforcement interpretation.\n",
        );
    }
    prose
}

fn report(
    owner: &str,
    fixture: &str,
    route: &str,
    baseline: &StudyOutput,
    selected: &StudyOutput,
) -> Value {
    json!({
        "owner":owner, "fixture":fixture, "route":route, "input_kind":baseline.input_kind,
        "baseline_user_bytes":baseline.text.len(), "selected_user_bytes":selected.text.len(),
        "baseline_sha256":digest(&baseline.text), "selected_sha256":digest(&selected.text),
        "same_system":baseline.system_prompt == selected.system_prompt,
        "same_page":baseline.page == selected.page,
        "same_delivery_identity":baseline.navigation_id == selected.navigation_id,
        "older_counterexample_in_baseline":baseline.text.contains("OLDER_COUNTEREXAMPLE"),
        "older_counterexample_in_selected":selected.text.contains("OLDER_COUNTEREXAMPLE"),
        "latest_revision_in_baseline":baseline.text.contains("LATEST_QUALIFICATION"),
        "latest_revision_in_selected":selected.text.contains("LATEST_QUALIFICATION"),
        "latest_retained":selected.text.contains("ACCOUNT_3"),
    })
}

#[test]
fn matched_recall_comparison_preserves_source_state_quiet_parking_and_exposes_omission_risk() {
    let mut results = Vec::new();
    for owner in ["astrid", "minime"] {
        for fixture in [
            "ordinary_history",
            "older_counterexample",
            "saved_counterexample",
            "latest_qualification",
        ] {
            let temp = tempfile::tempdir().unwrap();
            let path = temp.path().join(SOURCE);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(&path, "pub fn diagnostic_label() -> bool { false }\n").unwrap();
            let reader = Reader::new(
                Catalog::new(BTreeMap::from([(
                    "astrid".into(),
                    temp.path().join("astrid"),
                )]))
                .unwrap(),
                temp.path().join("state"),
            )
            .with_runtime_workspace(temp.path().join("workspace"), owner);
            reader
                .prepare_action("SELF_STUDY QUESTION NEW Does a diagnostic label enforce anything?")
                .unwrap();
            for index in 0..4 {
                let input = reader.prepare_action(OPEN).unwrap();
                deliver(&reader, &input, &synthetic_account(index, fixture));
            }
            let baseline = reader.prepare_action(OPEN).unwrap();
            let state_path = temp.path().join("state/reader-v1.json");
            let before = fs::read(&state_path).unwrap();
            let selected = latest_account_only(&baseline);
            assert_eq!(baseline.input_kind, InputKind::SourcePage);
            assert!(selected.text.len() < baseline.text.len());
            assert!(baseline.text.contains("ACCOUNT_0"));
            assert!(!selected.text.contains("ACCOUNT_0"));
            assert!(selected.text.contains("ACCOUNT_3"));
            assert_eq!(fs::read(&state_path).unwrap(), before);
            // The retained offer is still the original, not our offline contrast.
            assert_eq!(
                reader.prepare_action("SELF_STUDY CONTINUE").unwrap(),
                baseline
            );
            if fixture == "older_counterexample" {
                assert!(baseline.text.contains("OLDER_COUNTEREXAMPLE"));
                assert!(!selected.text.contains("OLDER_COUNTEREXAMPLE"));
            } else if fixture == "saved_counterexample" {
                assert!(selected.text.contains("OLDER_COUNTEREXAMPLE"));
            } else if fixture == "latest_qualification" {
                assert!(selected.text.contains("LATEST_QUALIFICATION"));
            }
            results.push(report(owner, fixture, "source-page", &baseline, &selected));

            reader
                .prepare_action("SELF_STUDY QUESTION PARK q1")
                .unwrap();
            for action in [
                "SELF_STUDY MAP",
                "INTROSPECT",
                "SELF_STUDY QUESTION REVIEW q1",
            ] {
                let input = reader.prepare_action(action).unwrap();
                let before = fs::read(&state_path).unwrap();
                let selected = latest_account_only(&input);
                assert_eq!(selected, input);
                assert_eq!(fs::read(&state_path).unwrap(), before);
                if action != "SELF_STUDY QUESTION REVIEW q1" {
                    assert!(!input.text.contains("ACCOUNT_"));
                }
                results.push(report(owner, fixture, action, &input, &selected));
            }
            let state: Value = serde_json::from_slice(&fs::read(&state_path).unwrap()).unwrap();
            assert!(state["questions"]["active"].is_null());
            reader.prepare_action("SELF_STUDY QUESTION q1").unwrap();
            let returned = reader.prepare_action("SELF_STUDY CONTINUE").unwrap();
            assert!(returned.text.contains("ACCOUNT_0"));
            assert!(returned.text.contains("ACCOUNT_3"));
        }
    }
    if let Some(path) = std::env::var_os("ASTRID_SELECTIVE_RECALL_REPORT") {
        let report = json!({
            "schema":"offline-selective-recall-comparison-v1",
            "intervention":"remove only the recent array from a copy of an existing synthetic recalled-account JSON block",
            "scope":"matched input/preservation comparison; no inference, actual delivery, policy change or causal verdict",
            "source_sha256":{
                "notebook.rs":digest(include_str!("../src/notebook.rs")),
                "store_navigation.rs":digest(include_str!("../src/store_navigation.rs")),
                "prompt.txt":digest(astrid_source_study::STUDY_PROMPT)
            },
            "results":results
        });
        fs::write(path, serde_json::to_vec_pretty(&report).unwrap()).unwrap();
    }
}
