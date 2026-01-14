use talaria_core::models::*;

#[test]
fn hsuf_enrich_request_roundtrip() {
    let req = HsufEnrichRequest {
        images: vec!["https://example.com/img.jpg".into()],
        sku: Some("sku-123".into()),
        context_text: Some("notes about the item".into()),
        prompt_rules: Some("grid squares are 0.5in".into()),
        llm_ingest: Some(LlmStageOptions {
            model: LlmModel::Gpt5Mini,
            reasoning: Some(true),
            web_search: Some(false),
        }),
    };
    let json = serde_json::to_string(&req).unwrap();
    let de: HsufEnrichRequest = serde_json::from_str(&json).unwrap();
    assert_eq!(de.images.len(), 1);
    assert_eq!(de.sku.unwrap(), "sku-123");
    assert_eq!(de.context_text.as_deref(), Some("notes about the item"));
    assert_eq!(de.prompt_rules.as_deref(), Some("grid squares are 0.5in"));
    let llm = de.llm_ingest.expect("llm_ingest");
    assert!(matches!(llm.model, LlmModel::Gpt5Mini));
    assert_eq!(llm.reasoning, Some(true));
    assert_eq!(llm.web_search, Some(false));
}

#[test]
fn api_error_parse() {
    let raw = r#"{"code":"bad_request","detail":null,"error":"Invalid input","fields":null,"request_id":"req-123"}"#;
    let parsed: ApiError = serde_json::from_str(raw).unwrap();
    assert_eq!(parsed.code.as_deref(), Some("bad_request"));
    assert_eq!(parsed.request_id.as_deref(), Some("req-123"));
}

#[test]
fn job_state_completed_roundtrip() {
    let state = JobState::Completed {
        result: ListingResponse {
            listing_id: "abc".into(),
            stages: vec![],
        },
    };
    let json = serde_json::to_string(&state).unwrap();
    let de: JobState = serde_json::from_str(&json).unwrap();
    match de {
        JobState::Completed { result } => assert_eq!(result.listing_id, "abc"),
        _ => panic!("unexpected variant"),
    }
}
