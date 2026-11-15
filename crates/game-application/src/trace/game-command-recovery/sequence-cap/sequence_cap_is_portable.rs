crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "Direct sequence helper, synthetic maximum; no rules execution or DB.",
        reason: "Direct helper asserts portable maximum and exhausted boundary.",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[],
    };
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
}
