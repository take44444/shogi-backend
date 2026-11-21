use super::*;

#[test]
fn sequence_cap_is_portable() {
    assert_eq!(next_sequence(i64::MAX as u64 - 1), Ok(i64::MAX as u64));
    assert_eq!(
        next_sequence(i64::MAX as u64),
        Err(ApplicationError::SequenceExhausted)
    );
    assert_eq!(
        next_sequence(u64::MAX),
        Err(ApplicationError::SequenceExhausted)
    );
}

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
