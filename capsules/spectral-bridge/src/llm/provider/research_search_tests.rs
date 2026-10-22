#[cfg(test)]
mod research_search_delivery_tests {
    use super::*;
    use serde_json::json;

    const MEDIA: &str = "application/vnd.astrid.search+json";
    const QUERY: &str = "  \"CONTROL Vec<T>\"  \u{03bb} & q=other;\nexact  ";

    fn envelope() -> serde_json::Value {
        json!({"schema":"astrid.search.results.v1", "provider":"brave_web_v1",
            "query": QUERY, "results":[{"title":"Synthetic title", "url":"https://example.org/",
                "snippet":"Synthetic provider snippet, not page evidence"}]})
    }

    #[test]
    fn normalized_search_preserves_query_and_candidates() {
        let body = serde_json::to_vec(&envelope()).unwrap();
        let hits = search_hits_from_body(QUERY, MEDIA, &body).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].title, "Synthetic title");
        assert_eq!(hits[0].url, "https://example.org/");
        assert_eq!(
            hits[0].snippet,
            "Synthetic provider snippet, not page evidence"
        );
        assert!(search_hits_from_body(QUERY.trim(), MEDIA, &body).is_err());
    }

    #[tokio::test]
    async fn explicit_empty_search_completes_without_model_or_fallback() {
        let mut value = envelope();
        value["results"] = json!([]);
        let result = search_from_body(
            QUERY,
            "retained anchor",
            MEDIA,
            &serde_json::to_vec(&value).unwrap(),
        )
        .await
        .unwrap();
        assert!(result.hits.is_empty());
        assert!(result.raw_text.is_empty());
        assert_eq!(result.anchor, "retained anchor");
        assert!(result.prompt_body().contains("zero results"));
        assert!(!result.prompt_body().contains("Best next move"));
    }

    #[test]
    fn invalid_shared_search_envelopes_fail_without_echoing_payload() {
        let mut invalid = vec![json!({"web":{"results":[]}})];
        for (key, bad) in [
            ("schema", json!("PRIVATE_SCHEMA")),
            ("provider", json!("other")),
            ("query", json!("PRIVATE_QUERY")),
            ("results", json!(null)),
            (
                "results",
                json!([{"title":"PRIVATE_BODY", "url":3, "snippet":"x"}]),
            ),
            (
                "results",
                json!([{"title":"x", "url":"https://example.org/"}]),
            ),
            (
                "results",
                json!([{"title":"x", "url":"https://example.org/", "snippet":"x", "extra":1}]),
            ),
            ("extra", json!(true)),
        ] {
            let mut value = envelope();
            value[key] = bad;
            invalid.push(value);
        }
        let mut too_many = envelope();
        too_many["results"] = json!(vec![envelope()["results"][0].clone(); 6]);
        invalid.push(too_many);
        for value in invalid {
            let error = search_hits_from_body(QUERY, MEDIA, &serde_json::to_vec(&value).unwrap())
                .unwrap_err();
            assert_eq!(error, "public_web_search_response_invalid");
        }
        for body in [
            b"PRIVATE_BODY".as_slice(),
            b"\xff",
            br#"{"schema":"x","schema":"y"}"#,
        ] {
            assert_eq!(
                search_hits_from_body(QUERY, MEDIA, body).unwrap_err(),
                "public_web_search_response_invalid"
            );
        }
    }

    #[test]
    fn legacy_html_keeps_its_hit_and_empty_semantics() {
        let html = br#"<a class="result__a" href="https://example.org/">Synthetic title</a>
            <span class="result__snippet">A sufficiently long synthetic snippet.</span>"#;
        let hits = search_hits_from_body(QUERY, "text/html", html).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].title, "Synthetic title");
        assert_eq!(
            search_hits_from_body(QUERY, "text/html", b"<html></html>").unwrap_err(),
            "public_web_no_readable_search_results"
        );
    }
}
