use mediaatlas_application::review_queue::ReviewQueueView;

#[test]
fn shared_frontend_fixture_matches_actual_rust_dto_serialization() {
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/review_queue.json")).unwrap();
    let view: ReviewQueueView = serde_json::from_value(fixture.clone()).unwrap();
    assert_eq!(serde_json::to_value(view).unwrap(), fixture);
}
