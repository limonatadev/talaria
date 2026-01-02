use talaria_core::models::*;

#[test]
fn hsuf_enrich_request_roundtrip() {
    let req = HsufEnrichRequest {
        images: vec!["https://example.com/img.jpg".into()],
        sku: Some("sku-123".into()),
    };
    let json = serde_json::to_string(&req).unwrap();
    let de: HsufEnrichRequest = serde_json::from_str(&json).unwrap();
    assert_eq!(de.images.len(), 1);
    assert_eq!(de.sku.unwrap(), "sku-123");
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
