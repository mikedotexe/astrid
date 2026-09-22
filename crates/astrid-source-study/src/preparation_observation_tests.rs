use super::*;
use serde_json::{Value, json};

#[test]
fn confirmed_disclosure_recovers_both_native_stores_once_after_every_redo_boundary() {
    for point in 0..3 {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        fs::create_dir_all(root.join("minime/workspace/runtime")).unwrap();
        let now = u64::try_from(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis(),
        )
        .unwrap();
        fs::write(root.join("minime/workspace/runtime/esn_activation_trace_v1.json"),json!({"policy":"esn_activation_trace_v1","reservoir_dim":128,"sample_interval_ms":1000,"retained_secs":180,"updated_at_unix_ms":now,"frames":[{"t_ms":1000,"wall_clock_unix_ms":now,"activations":vec![0.0;128],"summary":{"finite_fraction":1.0}}]}).to_string()).unwrap();
        let reader = crate::Reader::new(
            crate::Catalog::new(BTreeMap::from([("minime".into(), root.join("minime"))])).unwrap(),
            root.join("reader"),
        )
        .with_runtime_workspace(root.join("workspace"), "minime");
        reader.prepare_action("WRITE START private").unwrap();
        let writer = crate::writing::Writer::new(root.join("reader/writing"));
        let action = |id: &str, operation: Value, present: bool| {
            let (revision, _, history) = writer.observation_context("d1").unwrap();
            format!(
                "WRITE OBSERVE {}",
                json!({"owner":"minime","draft":"d1","revision":revision,"request_id":id,"expected_head":history.head(),"operation":operation,"present":present})
            )
        };
        let capture = action("capture", json!({"kind":"capture","seconds":1}), false);
        reader
            .prepare_once("capture", &reader.preparation_revision().unwrap(), &capture)
            .unwrap();
        let (_, _, history) = writer.observation_context("d1").unwrap();
        let preview = action(
            "preview",
            json!({"kind":"link_preview","captures":[history.head()],"destination":{"kind":"new","question":"Chosen question"}}),
            true,
        );
        let output = reader
            .prepare_once("preview", &reader.preparation_revision().unwrap(), &preview)
            .unwrap();
        reader
            .navigation_delivered(
                output.navigation_id.as_ref().unwrap(),
                &json!({"messages":[{"role":"user","content":output.text}]}).to_string(),
                &json!({"message":{"content":"The exact preview was supplied."},"done":true})
                    .to_string(),
            )
            .unwrap();
        let (_, _, history) = writer.observation_context("d1").unwrap();
        let confirm = action(
            "confirm",
            json!({"kind":"link_confirm","preview":history.head()}),
            false,
        );
        let revision = reader.preparation_revision().unwrap();
        FAIL_AT.with(|v| v.set(Some(point)));
        assert!(reader.prepare_once("confirm", &revision, &confirm).is_err());
        let first = reader.prepare_once("confirm", &revision, &confirm).unwrap();
        assert_eq!(
            first,
            reader.prepare_once("confirm", &revision, &confirm).unwrap()
        );
        let state: Value =
            serde_json::from_slice(&fs::read(root.join("reader/reader-v1.json")).unwrap()).unwrap();
        assert_eq!(state["questions"]["entries"].as_object().unwrap().len(), 1);
        assert!(state["questions"]["active"].is_null());
        let (_, _, history) = writer.observation_context("d1").unwrap();
        assert_eq!(history.records.len(), 3);
        assert!(matches!(
            history.records.last().unwrap().entry().unwrap(),
            crate::observations::Entry::Confirmed { .. }
        ));
    }
}
