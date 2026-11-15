crate::trace_support::case! {
    crate::trace_support::Case {
        scope: "Pure scalar storage-boundary cases.",
        reason: "Direct scalar conversion assertions reject values outside PostgreSQL signed-integer storage bounds.",
        relationship: crate::trace_support::Relationship::Asserts,
        also: &[],
    };
#[test]
    fn record_scalars_reject_values_outside_storage_bounds() {
        for value in [0u64, 1, i64::MAX as u64] {
            assert_eq!(OperationSequence::try_from(value).unwrap().get(), value);
            assert_eq!(MovePly::try_from(value).unwrap().get(), value);
        }
        for value in [i64::MAX as u64 + 1, u64::MAX] {
            assert_eq!(OperationSequence::try_from(value), Err(Error::Corrupt));
            assert_eq!(MovePly::try_from(value), Err(Error::Corrupt));
        }
        for value in [i64::MIN, -1] {
            assert_eq!(OperationSequence::try_from(value), Err(Error::Corrupt));
            assert_eq!(MovePly::try_from(value), Err(Error::Corrupt));
            assert_eq!(PreparedFinishTime::try_from(value), Err(Error::Corrupt));
        }
        for value in [0i64, 1, i64::MAX] {
            assert_eq!(
                OperationSequence::try_from(value).unwrap().get(),
                value as u64
            );
            assert_eq!(MovePly::try_from(value).unwrap().get(), value as u64);
            assert_eq!(PreparedFinishTime::try_from(value).unwrap().get(), value);
        }
    }
}
