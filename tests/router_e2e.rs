use std::sync::Arc;
use aetheris_cortex::{Phase, RegexClassifier, RequestType, RouterState, Tier};

#[tokio::test]
async fn test_cold_start_and_sticky_continuation() {
    let state = RouterState::new();
    let classifier = Arc::new(RegexClassifier);

    // Turn 1: Cold start
    let (tier1, model1, cached1, class1) = state
        .route(
            "sess-1",
            Phase::ColdStart,
            "write an Axum server in Rust",
            None,
            &classifier,
        )
        .await;

    assert_eq!(tier1, Tier::Tier2);
    assert_eq!(model1, "claude-sonnet-4-6");
    assert!(!cached1);
    assert_eq!(
        class1.unwrap().request_type,
        RequestType::CodeGeneration
    );

    // Turn 2: Sticky continuation (Level 2 Cache HIT)
    let (tier2, model2, cached2, class2) = state
        .route(
            "sess-1",
            Phase::Continue,
            "now add graceful shutdown",
            None,
            &classifier,
        )
        .await;

    assert_eq!(tier2, Tier::Tier2);
    assert_eq!(model2, "claude-sonnet-4-6");
    assert!(cached2); // HIT!
    assert!(class2.is_none()); // Classifier bypassed!

    // Turn 3: Topic switch (Invalidates cache & re-classifies)
    let (tier3, model3, cached3, class3) = state
        .route(
            "sess-1",
            Phase::Switch,
            "design a multi-region distributed Kafka cluster",
            None,
            &classifier,
        )
        .await;

    assert_eq!(tier3, Tier::Tier1);
    assert_eq!(model3, "claude-opus-4-8");
    assert!(!cached3);
    assert_eq!(
        class3.unwrap().request_type,
        RequestType::TechnicalDesign
    );
}
