crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "Actor state is directly set to maximum sequence for error precedence.",
        reason: "Synthetic exhausted actor distinguishes stale SequenceMismatch from SequenceExhausted.",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[],
    };
#[tokio::test]
async fn exhausted_state_still_checks_expected_sequence_before_advancing() {
    let repository = store();
    let mut actor = Actor::new(id(), repository.clone());
    actor.refresh().await.unwrap();
    actor.state.as_mut().unwrap().seq = i64::MAX as u64;
    repository.0.lock().unwrap().version = i64::MAX as u64;
    let mut request = receipt();
    request.expected_seq = u64::MAX;
    assert_eq!(
        actor.apply(request.clone()).await,
        Err(ApplicationError::SequenceMismatch {
            expected: u64::MAX,
            actual: i64::MAX as u64
        })
    );
    request.expected_seq = i64::MAX as u64;
    assert_eq!(
        actor.apply(request).await,
        Err(ApplicationError::SequenceExhausted)
    );
}
}
