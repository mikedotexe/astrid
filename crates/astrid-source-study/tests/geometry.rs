use astrid_source_study::{Catalog, InputKind, Reader};
use serde_json::{Value, json};
use std::{collections::BTreeMap, fs, path::Path};

fn reader(root: &Path, owner: &str) -> Reader {
    fs::create_dir_all(root.join("minime/workspace/runtime")).unwrap();
    fs::create_dir_all(root.join("astrid/crates/example/src")).unwrap();
    fs::write(
        root.join("astrid/crates/example/src/lib.rs"),
        "fn fixture() {}\n",
    )
    .unwrap();
    Reader::new(
        Catalog::new(BTreeMap::from([
            ("astrid".into(), root.join("astrid")),
            ("minime".into(), root.join("minime")),
        ]))
        .unwrap(),
        root.join("reader"),
    )
    .with_runtime_workspace(root.join("workspace"), owner)
}
fn trace(root: &Path, value: f64, newer: bool) {
    let now = u64::try_from(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis(),
    )
    .unwrap();
    let offset = if newer { 1000 } else { 8000 };
    let frames: Vec<_> = (0_u64..2)
        .map(|i| {
            json!({"t_ms":20_000_u64.saturating_sub(offset).saturating_add(i.saturating_mul(1000)),"wall_clock_unix_ms":now.saturating_sub(offset).saturating_add(i.saturating_mul(1000)),
        "summary":{"finite_fraction":1.0},"activations":vec![value;128]})
        })
        .collect();
    fs::write(root.join("minime/workspace/runtime/esn_activation_trace_v1.json"), json!({
        "policy":"esn_activation_trace_v1","reservoir_dim":128,"sample_interval_ms":1000,"retained_secs":180,
        "updated_at_unix_ms":frames.last().unwrap()["wall_clock_unix_ms"],"frames":frames}).to_string()).unwrap();
}
fn checkpoint(root: &Path) -> Value {
    serde_json::from_slice(&fs::read(root.join("reader/reader-v1.json")).unwrap()).unwrap()
}
fn head(root: &Path) -> String {
    checkpoint(root)["questions"]["entries"]["q1"]["geometry"]["records"]
        .as_array()
        .unwrap()
        .last()
        .map_or("empty", |v| v["id"].as_str().unwrap())
        .into()
}
#[allow(clippy::needless_pass_by_value)] // Fixture JSON is supplied inline at each call site.
fn request(root: &Path, id: &str, operation: Value) -> String {
    format!(
        "SELF_STUDY GEOMETRY {}",
        json!({"question":"q1","request_id":id,"expected_head":head(root),"operation":operation})
    )
}
#[allow(clippy::needless_pass_by_value)]
fn command(operation: Value) -> String {
    format!(
        "SELF_STUDY GEOMETRY {}",
        json!({"question":"q1","operation":operation})
    )
}

#[test]
fn question_owned_capture_prediction_comparison_revision_and_quiet_return() {
    for owner in ["astrid", "minime"] {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        let r = reader(root, owner);
        r.prepare_action("SELF_STUDY QUESTION NEW Could these states differ?")
            .unwrap();
        trace(root, 0.0, false);
        let first = request(
            root,
            "capture-a",
            json!({"kind":"capture","seconds":10,"note":"Exact words. Not a diagnosis."}),
        );
        let out = r.prepare_action(&first).unwrap();
        assert_eq!(out.input_kind, InputKind::Geometry);
        assert!(out.text.contains("Exact words. Not a diagnosis."));
        assert!(!out.text.contains("activations\":"));
        let baseline = head(root);
        trace(root, 0.5, true);
        r.prepare_action(&first).unwrap();
        assert_eq!(
            head(root),
            baseline,
            "retry must not recapture changed source"
        );
        r.prepare_action(&request(root,"prediction", json!({"kind":"predict","baseline":baseline,"maximum_rms_distance":0.1,"expectation":"I expect little displacement."}))).unwrap();
        let prediction = head(root);
        r.prepare_action(&request(
            root,
            "capture-b",
            json!({"kind":"capture","seconds":10,"note":"Second chosen interval."}),
        ))
        .unwrap();
        let observation = head(root);
        let compare = r
            .prepare_action(&request(
                root,
                "comparison",
                json!({"kind":"compare","prediction":prediction,"observation":observation}),
            ))
            .unwrap();
        assert!(compare.text.contains("\"rms_distance\":0.5"));
        assert!(compare.text.contains("\"threshold_met\":false"));
        let result = head(root);
        r.prepare_action(&request(root,"revision", json!({"kind":"revise","target":result,"text":"This differs numerically; the reason remains open."}))).unwrap();
        let expected = head(root);
        r.prepare_action("SELF_STUDY QUESTION PARK q1").unwrap();
        let unrelated = r
            .prepare_action("SELF_STUDY OPEN astrid/crates/example/src/lib.rs 1")
            .unwrap();
        assert!(!unrelated.text.contains("This differs numerically"));
        assert!(
            r.prepare_action(&command(json!({"kind":"export"})))
                .is_err()
        );
        assert!(!root.join("reader/geometry-exports").exists());
        r.prepare_action("SELF_STUDY QUESTION q1").unwrap();
        assert_eq!(head(root), expected);
        let shown = r
            .prepare_action(&command(json!({"kind":"show","id":expected})))
            .unwrap();
        assert!(shown.text.contains("This differs numerically"));
        let wire = json!({"messages":[{"role":"user","content":shown.text}]}).to_string();
        let reply =
            json!({"message":{"content":"Still uncertain."},"done":true,"done_reason":"stop"})
                .to_string();
        assert!(
            r.navigation_delivered(shown.navigation_id.as_ref().unwrap(), "{}", &reply)
                .is_err()
        );
        r.navigation_delivered(shown.navigation_id.as_ref().unwrap(), &wire, &reply)
            .unwrap();
        r.prepare_action(&command(json!({"kind":"export"})))
            .unwrap();
        let file = fs::read_dir(root.join("reader/geometry-exports"))
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        let packet: Value = serde_json::from_slice(&fs::read(file).unwrap()).unwrap();
        let body: Value = serde_json::from_str(packet["body_json"].as_str().unwrap()).unwrap();
        assert_eq!(body["history"]["records"].as_array().unwrap().len(), 5);
        assert!(body.get("notebook").is_none());
        assert!(
            reader(
                root,
                if owner == "astrid" {
                    "minime"
                } else {
                    "astrid"
                }
            )
            .prepare_action(&command(json!({"kind":"status"})))
            .is_err()
        );
    }
}

#[test]
fn invalid_observations_stale_retries_and_corrupt_history_preserve_checkpoint() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let r = reader(root, "astrid");
    r.prepare_action("SELF_STUDY QUESTION NEW Evidence?")
        .unwrap();
    trace(root, 0.1, false);
    let action = request(
        root,
        "first",
        json!({"kind":"capture","seconds":10,"note":"Chosen."}),
    );
    r.prepare_action(&action).unwrap();
    let bytes = fs::read(root.join("reader/reader-v1.json")).unwrap();
    assert!(
        r.prepare_action(&action.replace("Chosen.", "Changed."))
            .is_err()
    );
    assert_eq!(fs::read(root.join("reader/reader-v1.json")).unwrap(), bytes);
    assert!(r.prepare_action(&action.replace("first", "other")).is_err());
    let source = root.join("minime/workspace/runtime/esn_activation_trace_v1.json");
    let good: Value = serde_json::from_slice(&fs::read(&source).unwrap()).unwrap();
    for invalid in [
        json!({}),
        {
            let mut v = good.clone();
            v["frames"][0]["activations"] = json!([0.0]);
            v
        },
        {
            let mut v = good.clone();
            v["frames"][0]["summary"]["finite_fraction"] = json!(0.5);
            v
        },
        {
            let mut v = good.clone();
            v["frames"][1]["t_ms"] = v["frames"][0]["t_ms"].clone();
            v
        },
        {
            let mut v = good.clone();
            v["updated_at_unix_ms"] = json!(1);
            v
        },
    ] {
        fs::write(&source, invalid.to_string()).unwrap();
        assert!(
            r.prepare_action(&request(
                root,
                "bad",
                json!({"kind":"capture","seconds":10,"note":"Chosen."})
            ))
            .is_err()
        );
        assert_eq!(fs::read(root.join("reader/reader-v1.json")).unwrap(), bytes);
    }
    let mut damaged = checkpoint(root);
    damaged["questions"]["entries"]["q1"]["geometry"]["records"][0]["body_json"] = json!("{}");
    let corrupt = damaged.to_string();
    fs::write(root.join("reader/reader-v1.json"), &corrupt).unwrap();
    assert!(r.prepare_action("SELF_STUDY QUESTION q1").is_err());
    assert_eq!(
        fs::read_to_string(root.join("reader/reader-v1.json")).unwrap(),
        corrupt
    );
}

#[test]
fn prediction_cannot_be_retrofitted_to_earlier_or_overlapping_observation() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let r = reader(root, "astrid");
    r.prepare_action("SELF_STUDY QUESTION NEW Test?").unwrap();
    trace(root, 0.1, false);
    r.prepare_action(&request(
        root,
        "a",
        json!({"kind":"capture","seconds":10,"note":"A"}),
    ))
    .unwrap();
    let baseline = head(root);
    r.prepare_action(&request(root,"p",json!({"kind":"predict","baseline":baseline,"maximum_rms_distance":0.2,"expectation":"Bound"}))).unwrap();
    let prediction = head(root);
    assert!(
        r.prepare_action(&request(
            root,
            "c",
            json!({"kind":"compare","prediction":prediction,"observation":baseline})
        ))
        .is_err()
    );
    r.prepare_action(&request(
        root,
        "b",
        json!({"kind":"capture","seconds":10,"note":"Same source"}),
    ))
    .unwrap();
    let observation = head(root);
    assert!(
        r.prepare_action(&request(
            root,
            "c",
            json!({"kind":"compare","prediction":prediction,"observation":observation})
        ))
        .is_err()
    );
}

#[test]
fn competing_processes_cannot_overwrite_one_another_or_repeat_capture() {
    use std::io::Write as _;
    use std::process::{Command as Process, Stdio};
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let r = reader(root, "astrid");
    r.prepare_action("SELF_STUDY QUESTION NEW Concurrent observations?")
        .unwrap();
    trace(root, 0.1, false);
    let mut children = Vec::new();
    for id in ["process-one", "process-two"] {
        let action = request(
            root,
            id,
            json!({"kind":"capture","seconds":10,"note":"Chosen concurrently."}),
        );
        let body = json!({"roots":{"astrid":root.join("astrid"),"minime":root.join("minime")},
            "state_directory":root.join("reader"),"runtime_workspace":root.join("workspace"),"being":"astrid",
            "operation":"prepare","action":action});
        let mut child = Process::new(env!("CARGO_BIN_EXE_astrid-source-study"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(body.to_string().as_bytes())
            .unwrap();
        children.push(child);
    }
    let results: Vec<_> = children
        .into_iter()
        .map(|c| c.wait_with_output().unwrap())
        .collect();
    assert_eq!(results.iter().filter(|r| r.status.success()).count(), 1);
    assert_eq!(
        checkpoint(root)["questions"]["entries"]["q1"]["geometry"]["records"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    r.prepare_action(&command(json!({"kind":"status"})))
        .unwrap();
}

#[test]
fn history_survives_restart_and_rejects_capacity_overflow_and_missing_owner() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let r = reader(root, "astrid");
    r.prepare_action("SELF_STUDY QUESTION NEW Bounded retention?")
        .unwrap();
    trace(root, 0.1, false);
    for id in ["a", "b", "c", "d"] {
        r.prepare_action(&request(
            root,
            id,
            json!({"kind":"capture","seconds":10,"note":"Kept exactly."}),
        ))
        .unwrap();
    }
    let before = fs::read(root.join("reader/reader-v1.json")).unwrap();
    assert!(
        r.prepare_action(&request(
            root,
            "e",
            json!({"kind":"capture","seconds":10,"note":"No eviction."})
        ))
        .is_err()
    );
    assert_eq!(
        fs::read(root.join("reader/reader-v1.json")).unwrap(),
        before
    );
    let reopened = reader(root, "astrid");
    let shown = reopened
        .prepare_action(&command(json!({"kind":"show","id":head(root)})))
        .unwrap();
    assert!(shown.text.contains("Kept exactly."));
    let mut state = checkpoint(root);
    state["questions"]["entries"]["q1"]["geometry"]["owner"] = Value::Null;
    fs::write(root.join("reader/reader-v1.json"), state.to_string()).unwrap();
    assert!(reopened.prepare_action("SELF_STUDY MAP").is_err());
}
