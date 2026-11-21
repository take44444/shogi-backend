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

#[test]
fn exhausted_state_still_checks_expected_sequence_before_advancing() {
    let repository = store();
    let mut state = State::restore(repository.0.lock().unwrap().clone()).unwrap();
    state.seq = i64::MAX as u64;
    let mut request = receipt();
    request.expected_seq = u64::MAX;
    assert_eq!(
        state.stage(id(), Some(&request), Timestamp::new(0).unwrap()),
        Err(ApplicationError::SequenceMismatch {
            expected: u64::MAX,
            actual: i64::MAX as u64
        })
    );
    request.expected_seq = i64::MAX as u64;
    assert_eq!(
        state.stage(id(), Some(&request), Timestamp::new(0).unwrap()),
        Err(ApplicationError::SequenceExhausted)
    );
    state.seq = i64::MAX as u64 - 1;
    request.expected_seq = state.seq;
    assert_eq!(
        state.stage(id(), Some(&request), Timestamp::new(0).unwrap()),
        Err(ApplicationError::SequenceExhausted)
    );
}
